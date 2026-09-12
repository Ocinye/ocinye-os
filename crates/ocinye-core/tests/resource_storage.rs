//! Personal storage accounting and server-side enforcement (ADR-0108).
//!
//! Usage is measured from the bytes a member owns; the limit is their
//! entitlement; a write over the limit is refused; reducing a quota below usage
//! never deletes data; freeing space re-admits; and the SQL limit used by the
//! admission equals the Rust resolver used by display, so the two never
//! disagree. The full upload path is proved against real object storage.
//!
//! Skips when `OCINYE_TEST_DATABASE_URL` is unset; fails if set-but-unreachable.

use ocinye_contracts::{ResourceScopeType, ResourceType, StorageState, TechnicalRole};
use ocinye_core::modules::{files, resource};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

const GIB: i64 = 1024 * 1024 * 1024;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: o enforcement de storage ficaria por verificar"
        );
        return None;
    };
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL definida mas a base não responde");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    Some(pool)
}

fn test_store() -> Option<ocinye_core::storage::ObjectStore> {
    ocinye_core::storage::ObjectStore::new(ocinye_core::config::StorageConfig {
        endpoint_url: std::env::var("OCINYE_TEST_STORAGE_ENDPOINT").ok()?,
        region: std::env::var("OCINYE_TEST_STORAGE_REGION")
            .unwrap_or_else(|_| "us-east-1".to_owned()),
        access_key: std::env::var("OCINYE_TEST_STORAGE_ACCESS_KEY").ok()?,
        secret_key: std::env::var("OCINYE_TEST_STORAGE_SECRET_KEY").ok()?,
        bucket: std::env::var("OCINYE_TEST_STORAGE_BUCKET")
            .unwrap_or_else(|_| "ocinye-test-artifacts".to_owned()),
        backend_code: "test".to_owned(),
        location_label: "test".to_owned(),
        residency: ocinye_contracts::storage::Residency::Undeclared,
        max_upload_bytes: 32 * 1024 * 1024,
    })
}

async fn backend_por_omissao(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO storage_backends
             (code, kind, display_name, location_label, bucket, is_default, is_active)
         VALUES ('ocinye-test-default', 's3_compatible', 'Test', 'test', 'prova', TRUE, TRUE)
         ON CONFLICT (code) DO UPDATE
             SET is_default = TRUE, is_active = TRUE, updated_at = now()",
    )
    .execute(pool)
    .await
    .expect("registar armazenamento de teste");
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("s{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn member(pool: &PgPool, organisation_id: Uuid) -> Principal {
    let handle = format!("p{}", Uuid::new_v4().simple());
    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
         VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@ocinye.com"))
    .fetch_one(pool)
    .await
    .expect("pessoa");
    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
        .bind(person_id)
        .bind(TechnicalRole::ResearchMember.as_str())
        .execute(pool)
        .await
        .expect("papel");
    let pessoa = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &pessoa)
        .await
        .expect("principal")
}

async fn default_profile(pool: &PgPool, organisation_id: Uuid, storage_bytes: i64) {
    let profile_id: Uuid = sqlx::query_scalar(
        "INSERT INTO resource_profiles (organisation_id, code, name, is_default)
         VALUES ($1, 'MEMBER_STANDARD', 'Membro padrão', TRUE) RETURNING id",
    )
    .bind(organisation_id)
    .fetch_one(pool)
    .await
    .expect("perfil");
    sqlx::query(
        "INSERT INTO resource_profile_rules (profile_id, resource_type, quantity, unit)
         VALUES ($1, 'persistent_storage', $2, 'bytes')",
    )
    .bind(profile_id)
    .bind(storage_bytes)
    .execute(pool)
    .await
    .expect("regra");
}

async fn override_quota(pool: &PgPool, organisation_id: Uuid, person_id: Uuid, quantity: i64) {
    sqlx::query(
        "INSERT INTO resource_allocations
             (organisation_id, resource_type, unit, scope_type, scope_id, quantity, source, reason)
         VALUES ($1, 'persistent_storage', 'bytes', 'member', $2, $3, 'override', 'prova')",
    )
    .bind(organisation_id)
    .bind(person_id)
    .bind(quantity)
    .execute(pool)
    .await
    .expect("override");
}

/// Insert a stored personal object of the given size directly, returning its id.
async fn stored_object(pool: &PgPool, organisation_id: Uuid, owner_id: Uuid, size: i64) -> Uuid {
    let object_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, owner_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256,
              classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, $4, 'prova.bin', 'application/octet-stream', $5,
                repeat('0', 64), 'INTERNAL', 'stored', $3
           FROM storage_backends b WHERE b.is_default AND b.is_active",
    )
    .bind(object_id)
    .bind(organisation_id)
    .bind(owner_id)
    .bind(format!("k/{object_id}"))
    .bind(size)
    .execute(pool)
    .await
    .expect("objecto");
    object_id
}

/// The SQL limit used by the admission equals the Rust resolver used by display.
#[tokio::test]
async fn o_limite_sql_iguala_o_resolvedor() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;
    let quem = member(&pool, org).await;
    override_quota(&pool, org, quem.person_id, 20 * GIB).await;
    // A temporária soma por cima.
    sqlx::query(
        "INSERT INTO resource_allocations
             (organisation_id, resource_type, unit, scope_type, scope_id, quantity, source, expires_at, reason)
         VALUES ($1, 'persistent_storage', 'bytes', 'member', $2, $3, 'temporary', now() + interval '30 days', 'prova')",
    )
    .bind(org).bind(quem.person_id).bind(5 * GIB)
    .execute(&pool).await.expect("temporária");

    let sql = resource::storage::personal_storage_limit_bytes(&pool, quem.person_id)
        .await
        .expect("sql");
    let resolvido = resource::resolve_entitlement(
        &pool,
        org,
        ResourceScopeType::Member,
        quem.person_id,
        ResourceType::PersistentStorage,
    )
    .await
    .expect("resolvido")
    .quantity;

    assert_eq!(sql, resolvido, "o limite SQL divergiu do resolvedor Rust");
    assert_eq!(sql, 25 * GIB, "20 (override) + 5 (temporária)");
}

/// A write over the quota is refused; one that fits is admitted.
#[tokio::test]
async fn a_admissao_recusa_acima_da_quota() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;
    let quem = member(&pool, org).await;
    stored_object(&pool, org, quem.person_id, 9 * GIB).await;

    assert_eq!(
        resource::personal_usage_bytes(&pool, quem.person_id)
            .await
            .expect("uso"),
        9 * GIB
    );

    // Mais 2 GiB estoura os 10; mais 1 GiB cabe.
    let mut tx = pool.begin().await.expect("tx");
    assert!(
        resource::admit_personal_bytes(&mut tx, quem.person_id, 2 * GIB)
            .await
            .is_err(),
        "admitiu acima da quota"
    );
    assert!(
        resource::admit_personal_bytes(&mut tx, quem.person_id, 1 * GIB)
            .await
            .is_ok(),
        "recusou um ficheiro que cabia"
    );
    tx.rollback().await.expect("rollback");
}

/// Reducing the quota below usage puts the member over quota, keeps the data,
/// and blocks new writes — it never deletes.
#[tokio::test]
async fn reduzir_a_quota_abaixo_do_uso_bloqueia_sem_apagar() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 20 * GIB).await;
    let quem = member(&pool, org).await;
    let obj = stored_object(&pool, org, quem.person_id, 14 * GIB).await;

    // O administrador reduz para 10 GiB (override).
    override_quota(&pool, org, quem.person_id, 10 * GIB).await;

    let estado = resource::personal_storage_status(&pool, &quem, quem.person_id)
        .await
        .expect("estado");
    assert_eq!(
        estado.used_bytes,
        14 * GIB,
        "os bytes existentes desapareceram"
    );
    assert_eq!(estado.limit_bytes, 10 * GIB);
    assert_eq!(estado.state, StorageState::OverQuota);

    // Os bytes continuam lá — nada foi apagado.
    let ainda: i64 = sqlx::query_scalar("SELECT count(*) FROM storage_objects WHERE id = $1")
        .bind(obj)
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(ainda, 1, "um objecto foi apagado ao reduzir a quota");

    // Uma nova escrita, por pequena que seja, é recusada.
    let mut tx = pool.begin().await.expect("tx");
    assert!(
        resource::admit_personal_bytes(&mut tx, quem.person_id, 1)
            .await
            .is_err(),
        "admitiu uma escrita estando acima da quota"
    );
    tx.rollback().await.expect("rollback");
}

/// Freeing space re-admits writes.
#[tokio::test]
async fn apagar_liberta_e_readmite() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;
    let quem = member(&pool, org).await;
    let obj = stored_object(&pool, org, quem.person_id, 10 * GIB).await;

    let mut tx = pool.begin().await.expect("tx");
    assert!(
        resource::admit_personal_bytes(&mut tx, quem.person_id, 1)
            .await
            .is_err(),
        "admitiu estando cheio"
    );
    tx.rollback().await.expect("rollback");

    // Libertar espaço.
    sqlx::query("DELETE FROM storage_objects WHERE id = $1")
        .bind(obj)
        .execute(&pool)
        .await
        .expect("apagar");

    let mut tx = pool.begin().await.expect("tx");
    assert!(
        resource::admit_personal_bytes(&mut tx, quem.person_id, 5 * GIB)
            .await
            .is_ok(),
        "recusou depois de libertar espaço"
    );
    tx.rollback().await.expect("rollback");
}

/// Two uploads racing against a nearly-full quota: exactly one is admitted.
///
/// Each fits on its own, but not both. The per-member advisory lock serialises
/// the admissions, so the second sees the first's committed bytes and is
/// refused — the quota is not bypassed by racing (briefing §10).
#[tokio::test]
async fn duas_admissoes_a_correr_nao_estouram_a_quota() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;
    let quem = member(&pool, org).await;
    // Já usa 6 GiB; cada carregamento de 3 GiB cabe sozinho (9), os dois não (12).
    stored_object(&pool, org, quem.person_id, 6 * GIB).await;

    async fn carregar_com_admissao(
        pool: PgPool,
        org: Uuid,
        person: Uuid,
        size: i64,
    ) -> Result<(), ocinye_core::CoreError> {
        let mut tx = pool.begin().await.expect("tx");
        resource::admit_personal_bytes(&mut tx, person, size).await?;
        // Guardar dentro da mesma transacção, segurando a tranca.
        let object_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO storage_objects
                 (id, backend_id, organisation_id, owner_id, object_key,
                  original_filename, content_type, size_bytes, checksum_sha256,
                  classification, status, created_by_id)
             SELECT $1, b.id, $2, $3, $4, 'r.bin', 'application/octet-stream', $5,
                    repeat('0', 64), 'INTERNAL', 'stored', $3
               FROM storage_backends b WHERE b.is_default AND b.is_active",
        )
        .bind(object_id)
        .bind(org)
        .bind(person)
        .bind(format!("k/{object_id}"))
        .bind(size)
        .execute(&mut *tx)
        .await
        .expect("insert");
        tx.commit().await.expect("commit");
        Ok(())
    }

    let a = tokio::spawn(carregar_com_admissao(
        pool.clone(),
        org,
        quem.person_id,
        3 * GIB,
    ));
    let b = tokio::spawn(carregar_com_admissao(
        pool.clone(),
        org,
        quem.person_id,
        3 * GIB,
    ));
    let (ra, rb) = (a.await.expect("a"), b.await.expect("b"));

    let sucessos = [ra.is_ok(), rb.is_ok()].iter().filter(|x| **x).count();
    assert_eq!(
        sucessos, 1,
        "ambas ou nenhuma admissão passou: {ra:?} {rb:?}"
    );

    // O uso final é 9 GiB — só uma das duas de 3 GiB entrou.
    let uso = resource::personal_usage_bytes(&pool, quem.person_id)
        .await
        .expect("uso");
    assert_eq!(uso, 9 * GIB, "a corrida deixou entrar mais do que a quota");
}

/// End to end against real object storage: personal uploads count, and one that
/// would exceed the quota is refused by the Core, not the client.
#[tokio::test]
async fn o_carregamento_pessoal_e_admitido_contra_a_quota() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    backend_por_omissao(&pool).await;
    let org = organisation(&pool).await;
    // Slug real da organização, para a chave do objecto.
    let slug: String = sqlx::query_scalar("SELECT slug FROM organisations WHERE id = $1")
        .bind(org)
        .fetch_one(&pool)
        .await
        .expect("slug");
    // Quota pequena: 6 MiB, para provar o limite com ficheiros pequenos.
    let quota = 6 * 1024 * 1024;
    default_profile(&pool, org, quota).await;
    let quem = member(&pool, org).await;
    let ids = CorrelationIds::generate();

    let carregar = |dados: Vec<u8>| {
        let store = &store;
        let slug = slug.clone();
        let quem = &quem;
        let ids = &ids;
        let pool = &pool;
        async move {
            let mut tx = pool.begin().await.expect("tx");
            let r = files::create_personal(
                &mut tx,
                quem,
                ids,
                store,
                &slug,
                files::NewFile {
                    filename: format!("{}.txt", Uuid::new_v4().simple()),
                    content_type: "text/plain".to_owned(),
                    data: dados,
                    classification: None,
                },
            )
            .await;
            if r.is_ok() {
                tx.commit().await.expect("commit");
            } else {
                tx.rollback().await.expect("rollback");
            }
            r
        }
    };

    // Dois de 2 MiB cabem (4 MiB de 6).
    carregar(vec![7u8; 2 * 1024 * 1024])
        .await
        .expect("primeiro cabe");
    carregar(vec![7u8; 2 * 1024 * 1024])
        .await
        .expect("segundo cabe");

    // Um terceiro de 4 MiB levaria a 8 MiB > 6 MiB: recusado pelo Core.
    assert!(
        carregar(vec![7u8; 4 * 1024 * 1024]).await.is_err(),
        "o Core admitiu um carregamento acima da quota"
    );

    // E o uso reflecte os 4 MiB guardados, não os 8.
    let uso = resource::personal_usage_bytes(&pool, quem.person_id)
        .await
        .expect("uso");
    assert_eq!(uso, 4 * 1024 * 1024, "o uso contou o ficheiro recusado");
}
