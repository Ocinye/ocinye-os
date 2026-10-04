//! A001 · Auditoria pré-D011: uma prova por defeito corrigido no Core.
//!
//! Cada teste aqui falha com a guarda retirada — é o critério da auditoria
//! (`docs/audits/A001_PRE_D011_FULL_SYSTEM_AUDIT.md`). Base partilhada, uma
//! organização de fixture por prova.

use ocinye_contracts::{AccountStatus, Distribution, TechnicalRole};
use ocinye_core::modules::identity::{self, Authenticator, Throttle};
use ocinye_core::modules::organisation::{distributions, endpoints};
use ocinye_core::password::{Hasher, HashingParams};
use ocinye_core::CoreError;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::migrate::Migrator;
use sqlx::PgPool;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

fn ids() -> CorrelationIds {
    CorrelationIds::generate()
}

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: as provas A001 ficariam por correr"
        );
        return None;
    };
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL está definida mas a base não responde");
    MIGRATOR.run(&pool).await.expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

fn authenticator() -> Authenticator {
    Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: 8 * 1024,
            iterations: 2,
            parallelism: 1,
        }),
        Throttle {
            per_ip: 1_000,
            per_email: 1_000,
            window_minutes: 15,
        },
        24,
    )
}

async fn organizacao(pool: &PgPool) -> Uuid {
    let slug = format!("a{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn pessoa_id(pool: &PgPool, organisation_id: Uuid, roles: &[TechnicalRole]) -> Uuid {
    let handle = format!("m{}", Uuid::new_v4().simple());
    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
         VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@exemplo.test"))
    .fetch_one(pool)
    .await
    .expect("pessoa");
    for role in roles {
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
            .bind(person_id)
            .bind(role.as_str())
            .execute(pool)
            .await
            .expect("papel");
    }
    person_id
}

async fn principal(pool: &PgPool, person_id: Uuid) -> Principal {
    let registo = identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    identity::principal_for_person(pool, &registo)
        .await
        .expect("principal")
}

async fn registo(pool: &PgPool, person_id: Uuid) -> identity::Person {
    identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa")
}

// ── H003 · anexos de rascunho de outra pessoa ─────────────────────────────

#[tokio::test]
async fn h003_os_anexos_de_um_rascunho_alheio_nao_se_alcancam() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let a = principal(
        &pool,
        pessoa_id(&pool, org, &[TechnicalRole::ResearchMember]).await,
    )
    .await;
    let b = principal(
        &pool,
        pessoa_id(&pool, org, &[TechnicalRole::ResearchMember]).await,
    )
    .await;
    let caixa = |dono: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO mailboxes (organisation_id, address, kind, owner_id)
                 VALUES ($1, $2, 'personal', $3) RETURNING id",
            )
            .bind(org)
            .bind(format!("mb{}@exemplo.test", Uuid::new_v4().simple()))
            .bind(dono)
            .fetch_one(&pool)
            .await
            .expect("caixa")
        }
    };
    let caixa_a = caixa(a.person_id).await;
    let caixa_b = caixa(b.person_id).await;
    let rascunho_a = ocinye_core::modules::mail::repository::insert_composer_draft(
        &pool,
        caixa_a,
        a.person_id,
        &["dest@exemplo.test".to_owned()],
        &[],
        &[],
        Some("assunto"),
        "corpo",
        None,
        None,
    )
    .await
    .expect("rascunho");
    // Um armazenamento que nunca é alcançado: a recusa vem antes dos bytes.
    let store = ocinye_core::storage::ObjectStore::new(ocinye_core::config::StorageConfig {
        endpoint_url: "http://127.0.0.1:9".to_owned(),
        region: "us-east-1".to_owned(),
        access_key: "a001".to_owned(),
        secret_key: "a001".to_owned(),
        bucket: "a001".to_owned(),
        backend_code: "test".to_owned(),
        location_label: "test".to_owned(),
        residency: ocinye_contracts::storage::Residency::Undeclared,
        max_upload_bytes: 1024,
    })
    .expect("loja");

    // B, pela sua própria caixa, a nomear o rascunho de A.
    let b_alheio =
        ocinye_core::modules::mail::attachments_for_send(&pool, &store, &b, caixa_b, rascunho_a)
            .await;
    assert!(
        matches!(b_alheio, Err(CoreError::NotFound(_))),
        "o rascunho de outra pessoa foi alcançado: {b_alheio:?}"
    );
    // A, mas por outra caixa que não a do rascunho.
    let a_outra =
        ocinye_core::modules::mail::attachments_for_send(&pool, &store, &a, caixa_b, rascunho_a)
            .await;
    assert!(
        matches!(a_outra, Err(CoreError::NotFound(_))),
        "{a_outra:?}"
    );
    // A, pela caixa certa e sem anexos: lista vazia, sem tocar no armazenamento.
    let a_proprio =
        ocinye_core::modules::mail::attachments_for_send(&pool, &store, &a, caixa_a, rascunho_a)
            .await
            .expect("o seu próprio rascunho");
    assert!(a_proprio.is_empty());
}

// ── H004 · um OrganisationAdmin a agir sobre um PlatformAdmin ─────────────

#[tokio::test]
async fn h004_quem_nao_administra_a_plataforma_nao_age_sobre_quem_a_administra() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let platform = pessoa_id(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let _outra_platform = pessoa_id(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let org_admin = principal(
        &pool,
        pessoa_id(&pool, org, &[TechnicalRole::OrganisationAdmin]).await,
    )
    .await;
    let alvo = registo(&pool, platform).await;

    let reset = identity::reset_password(&pool, &authenticator(), &org_admin, &alvo, &ids()).await;
    assert!(
        matches!(reset, Err(CoreError::PermissionDenied(_))),
        "um OrganisationAdmin recebeu a credencial de um PlatformAdmin: {:?}",
        reset.map(|_| ())
    );
    let suspender = identity::set_account_status(
        &pool,
        &org_admin,
        &alvo,
        AccountStatus::Suspended,
        "x",
        &ids(),
    )
    .await;
    assert!(
        matches!(suspender, Err(CoreError::PermissionDenied(_))),
        "{suspender:?}"
    );

    // E sobre um membro comum, o OrganisationAdmin continua a poder.
    let membro = registo(
        &pool,
        pessoa_id(&pool, org, &[TechnicalRole::ResearchMember]).await,
    )
    .await;
    identity::reset_password(&pool, &authenticator(), &org_admin, &membro, &ids())
        .await
        .expect("um membro comum");
}

// ── H007 · fechar a última via de administração pela conta ────────────────

#[tokio::test]
async fn h007_suspender_o_unico_administrador_que_entra_e_recusado() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    // A entra em Research (o trigger dá-lhe acesso, uma só activada).
    let a = pessoa_id(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let actor = principal(&pool, a).await;
    distributions::enable(&pool, &actor, Distribution::Business, &ids())
        .await
        .expect("activar");
    // B é PlatformAdmin e não entra em Distribuição nenhuma (duas activadas:
    // nenhum acesso automático).
    let b = pessoa_id(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let b_principal = principal(&pool, b).await;
    let alvo = registo(&pool, a).await;

    let r = identity::set_account_status(
        &pool,
        &b_principal,
        &alvo,
        AccountStatus::Suspended,
        "x",
        &ids(),
    )
    .await;
    assert!(
        matches!(&r, Err(CoreError::Invariant { reason, .. }) if *reason == "last_administrator_access"),
        "a Instância ficou sem administrador que entre: {r:?}"
    );

    // Um membro comum suspende-se sem esta guarda o tocar, mesmo numa
    // organização onde ninguém administra.
    let sem_admin = organizacao(&pool).await;
    let quem = principal(
        &pool,
        pessoa_id(&pool, sem_admin, &[TechnicalRole::OrganisationAdmin]).await,
    )
    .await;
    let comum = registo(
        &pool,
        pessoa_id(&pool, sem_admin, &[TechnicalRole::ResearchMember]).await,
    )
    .await;
    identity::set_account_status(&pool, &quem, &comum, AccountStatus::Suspended, "x", &ids())
        .await
        .expect("suspender um membro comum");
}

// ── M004 · autoridade da plataforma por grant explícito ───────────────────

#[test]
fn m004_so_as_permissoes_exclusivas_da_plataforma_contam() {
    use ocinye_contracts::Permission;
    assert!(identity::is_platform_privileged(
        Permission::PlatformAdminister
    ));
    // `members.manage` também é do OrganisationAdmin: não é exclusiva.
    assert!(!identity::is_platform_privileged(Permission::MembersManage));
}

// ── M016 · o canónico é genérico ──────────────────────────────────────────

#[tokio::test]
async fn m016_o_ponto_canonico_nao_se_fixa_numa_distribuicao() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let admin = principal(
        &pool,
        pessoa_id(&pool, org, &[TechnicalRole::PlatformAdmin]).await,
    )
    .await;
    let host = format!(
        "c-{}.exemplo.test",
        &Uuid::new_v4().simple().to_string()[..12]
    );
    endpoints::seed(&pool, org, &format!("https://{host}"), &ids())
        .await
        .expect("semear");
    let canonico = endpoints::list(&pool, &admin)
        .await
        .expect("lista")
        .into_iter()
        .find(|e| e.canonical)
        .expect("canónico");
    let r = endpoints::bind(
        &pool,
        &admin,
        canonico.id,
        Some(Distribution::Research),
        &ids(),
    )
    .await;
    assert!(
        matches!(&r, Err(CoreError::Invariant { reason, .. }) if *reason == "endpoint_canonical_generic"),
        "o canónico foi fixado: {:?}",
        r.map(|_| ())
    );
}

// ── M018 · corrida contra uma restrição única é 409 ───────────────────────

#[tokio::test]
async fn m018_um_duplicado_recusado_pela_base_e_um_conflito() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let host = format!(
        "d-{}.exemplo.test",
        &Uuid::new_v4().simple().to_string()[..12]
    );
    let inserir = || {
        sqlx::query(
            "INSERT INTO access_endpoints (id, organisation_id, hostname, state, canonical)
             VALUES (gen_random_uuid(), $1, $2, 'unverified', false)",
        )
        .bind(org)
        .bind(&host)
        .execute(&pool)
    };
    inserir().await.expect("primeiro");
    let erro = CoreError::from(inserir().await.expect_err("duplicado"));
    assert_eq!(
        erro.code(),
        ocinye_contracts::ErrorCode::Conflict,
        "{erro:?}"
    );
    assert!(
        !erro.public_message().contains("access_endpoints"),
        "nomeia a tabela"
    );
}

// ── L006 · o par do contexto da sessão ────────────────────────────────────

#[tokio::test]
async fn l006_um_identificador_de_contexto_sem_tipo_e_recusado() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let quem = pessoa_id(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let digest = format!("{:064x}", u128::from_le_bytes(*Uuid::new_v4().as_bytes()));
    let r = sqlx::query(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at, active_context_id)
         VALUES ($1, $2, 'active', now() + interval '1 hour', gen_random_uuid())",
    )
    .bind(quem)
    .bind(digest)
    .execute(&pool)
    .await;
    assert!(r.is_err(), "uma sessão guardou um contexto órfão");
}

// ── M007 · a última liderança de um ambiente ──────────────────────────────

#[tokio::test]
async fn m007_despromover_a_ultima_lideranca_e_recusado() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let sufixo = Uuid::new_v4().simple().to_string();
    let unidade: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, $2) RETURNING id",
    )
    .bind(org)
    .bind(format!("U{}", &sufixo[..8]))
    .fetch_one(&pool)
    .await
    .expect("unidade");
    let ambiente: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces (organisation_id, unit_id, code, title)
         VALUES ($1, $2, $3, $3) RETURNING id",
    )
    .bind(org)
    .bind(unidade)
    .bind(format!("W{}", &sufixo[..12]))
    .fetch_one(&pool)
    .await
    .expect("ambiente");
    let lider = pessoa_id(&pool, org, &[TechnicalRole::ResearchMember]).await;
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role) VALUES ($1, $2, 'lead')",
    )
    .bind(ambiente)
    .bind(lider)
    .execute(&pool)
    .await
    .expect("liderança");
    let actor = principal(&pool, lider).await;

    let mut tx = pool.begin().await.expect("tx");
    let r = ocinye_core::modules::research::add_workspace_member(
        &mut tx,
        &actor,
        &ids(),
        ambiente,
        lider,
        ocinye_contracts::WorkspaceRole::Member,
    )
    .await;
    drop(tx);
    assert!(
        matches!(r, Err(CoreError::Conflict(_))),
        "o ambiente ficou sem liderança: {r:?}"
    );
    let papel: String = sqlx::query_scalar(
        "SELECT role FROM workspace_memberships WHERE workspace_id = $1 AND person_id = $2",
    )
    .bind(ambiente)
    .bind(lider)
    .fetch_one(&pool)
    .await
    .expect("papel");
    assert_eq!(papel, "lead");
}
