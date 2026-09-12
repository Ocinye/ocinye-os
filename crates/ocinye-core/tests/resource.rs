//! Resource governance — entitlement resolution and its authority boundary.
//!
//! Access authority and resource entitlement are different systems: a member
//! reads their own entitlement without any permission, an override replaces the
//! profile base, a temporary grant adds on top and expires, and seeing another
//! member's entitlement takes `resources.view`. None of it grants access.
//!
//! Skips when `OCINYE_TEST_DATABASE_URL` is unset; fails if set-but-unreachable.

use ocinye_contracts::{ResourceScopeType, ResourceType, TechnicalRole};
use ocinye_core::modules::resource;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

const GIB: i64 = 1024 * 1024 * 1024;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: a governança de recursos ficaria por verificar"
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

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("r{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn member(pool: &PgPool, organisation_id: Uuid, roles: &[TechnicalRole]) -> Principal {
    let handle = format!("m{}", Uuid::new_v4().simple());
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
    for role in roles {
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
            .bind(person_id)
            .bind(role.as_str())
            .execute(pool)
            .await
            .expect("papel");
    }
    let pessoa = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &pessoa)
        .await
        .expect("principal")
}

/// Seed the default profile and its storage rule for an organisation, as
/// provisioning will. Returns the profile id.
async fn default_profile(pool: &PgPool, organisation_id: Uuid, storage_bytes: i64) -> Uuid {
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
    profile_id
}

async fn add_allocation(
    pool: &PgPool,
    organisation_id: Uuid,
    person_id: Uuid,
    source: &str,
    quantity: i64,
    starts_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
) {
    sqlx::query(
        "INSERT INTO resource_allocations
             (organisation_id, resource_type, unit, scope_type, scope_id, quantity,
              source, starts_at, expires_at, reason)
         VALUES ($1, 'persistent_storage', 'bytes', 'member', $2, $3, $4, $5, $6, $7)",
    )
    .bind(organisation_id)
    .bind(person_id)
    .bind(quantity)
    .bind(source)
    .bind(starts_at)
    .bind(expires_at)
    .bind(format!("prova {source}"))
    .execute(pool)
    .await
    .expect("alocação");
}

/// With no allocation of their own, a member resolves the institution's default
/// profile — and the number can be explained.
#[tokio::test]
async fn um_membro_sem_alocacao_resolve_o_perfil_por_omissao() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;
    let quem = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let entitlement = resource::resolve_entitlement(
        &pool,
        org,
        ResourceScopeType::Member,
        quem.person_id,
        ResourceType::PersistentStorage,
    )
    .await
    .expect("resolver");

    assert_eq!(
        entitlement.quantity,
        10 * GIB,
        "não resolveu o perfil por omissão"
    );
    assert_eq!(
        entitlement.parts.len(),
        1,
        "a explicação devia ter uma parte"
    );
    assert!(
        entitlement.parts[0].note.contains("MEMBER_STANDARD"),
        "a parte não nomeia o perfil: {:?}",
        entitlement.parts[0].note
    );
}

/// A temporary grant adds on top of the base, and an expired one does not count.
#[tokio::test]
async fn uma_alocacao_temporaria_soma_e_expira() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;
    let quem = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let agora = chrono::Utc::now();
    let futuro = agora + chrono::Duration::days(30);
    // A viva: começou e ainda não expirou. A expirada: começou há dois dias e
    // expirou ontem — `expires_at > starts_at`, como o esquema exige.
    add_allocation(
        &pool,
        org,
        quem.person_id,
        "temporary",
        5 * GIB,
        agora,
        Some(futuro),
    )
    .await;
    add_allocation(
        &pool,
        org,
        quem.person_id,
        "temporary",
        3 * GIB,
        agora - chrono::Duration::days(2),
        Some(agora - chrono::Duration::days(1)),
    )
    .await;

    let entitlement = resource::resolve_entitlement(
        &pool,
        org,
        ResourceScopeType::Member,
        quem.person_id,
        ResourceType::PersistentStorage,
    )
    .await
    .expect("resolver");

    // 10 (perfil) + 5 (temporária viva); a expirada não conta.
    assert_eq!(
        entitlement.quantity,
        15 * GIB,
        "a temporária viva não somou, ou a expirada contou"
    );
}

/// An override replaces the profile base; temporaries still add.
#[tokio::test]
async fn um_override_substitui_a_base_do_perfil() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;
    let quem = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let agora = chrono::Utc::now();
    add_allocation(
        &pool,
        org,
        quem.person_id,
        "override",
        20 * GIB,
        agora,
        None,
    )
    .await;
    add_allocation(
        &pool,
        org,
        quem.person_id,
        "temporary",
        5 * GIB,
        agora,
        Some(agora + chrono::Duration::days(30)),
    )
    .await;

    let entitlement = resource::resolve_entitlement(
        &pool,
        org,
        ResourceScopeType::Member,
        quem.person_id,
        ResourceType::PersistentStorage,
    )
    .await
    .expect("resolver");

    // 20 (override, não 10 do perfil) + 5 (temporária).
    assert_eq!(
        entitlement.quantity,
        25 * GIB,
        "o override não substituiu a base"
    );
}

/// A member sees their own entitlement without any permission; seeing another's
/// requires `resources.view`.
#[tokio::test]
async fn ver_o_recurso_de_outro_exige_permissao() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;
    let quem = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let outro = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    // O próprio: sem permissão nenhuma.
    resource::member_entitlement(
        &pool,
        &quem,
        quem.person_id,
        ResourceType::PersistentStorage,
    )
    .await
    .expect("o próprio vê o seu recurso");

    // O de outro: recusado a um membro comum.
    assert!(
        resource::member_entitlement(
            &pool,
            &quem,
            outro.person_id,
            ResourceType::PersistentStorage
        )
        .await
        .is_err(),
        "um membro comum viu o recurso de outro"
    );

    // Um administrador com resources.view vê.
    let admin = member(&pool, org, &[TechnicalRole::OrganisationAdmin]).await;
    resource::member_entitlement(
        &pool,
        &admin,
        quem.person_id,
        ResourceType::PersistentStorage,
    )
    .await
    .expect("o administrador vê o recurso do membro");
}

/// `list_profiles` is governed by `resources.view`.
#[tokio::test]
async fn listar_perfis_exige_resources_view() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;

    let comum = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    assert!(
        resource::list_profiles(&pool, &comum).await.is_err(),
        "um membro comum listou os perfis"
    );

    let admin = member(&pool, org, &[TechnicalRole::OrganisationAdmin]).await;
    let perfis = resource::list_profiles(&pool, &admin)
        .await
        .expect("listar");
    assert!(
        perfis.iter().any(|p| p.profile.code == "MEMBER_STANDARD"),
        "o perfil por omissão não apareceu"
    );
}

/// Seed an additional named profile with a storage rule. Returns its id.
async fn named_profile(
    pool: &PgPool,
    organisation_id: Uuid,
    code: &str,
    storage_bytes: i64,
) -> Uuid {
    let profile_id: Uuid = sqlx::query_scalar(
        "INSERT INTO resource_profiles (organisation_id, code, name)
         VALUES ($1, $2, $2) RETURNING id",
    )
    .bind(organisation_id)
    .bind(code)
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
    profile_id
}

/// Assigning a profile changes the member's resolved entitlement — and it is a
/// resource operation, not an access one.
#[tokio::test]
async fn atribuir_um_perfil_muda_a_resolucao() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    default_profile(&pool, org, 10 * GIB).await;
    let pesado = named_profile(&pool, org, "COMPUTE_HEAVY", 50 * GIB).await;

    let quem = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let admin = member(&pool, org, &[TechnicalRole::OrganisationAdmin]).await;
    let ids = CorrelationIds::generate();

    // Antes: resolve o perfil por omissão (10 GiB).
    let antes = resource::resolve_entitlement(
        &pool,
        org,
        ResourceScopeType::Member,
        quem.person_id,
        ResourceType::PersistentStorage,
    )
    .await
    .expect("antes");
    assert_eq!(antes.quantity, 10 * GIB);

    // O administrador atribui o perfil pesado.
    resource::assign_member_profile(&pool, &admin, &ids, quem.person_id, Some(pesado))
        .await
        .expect("atribuir");

    let depois = resource::resolve_entitlement(
        &pool,
        org,
        ResourceScopeType::Member,
        quem.person_id,
        ResourceType::PersistentStorage,
    )
    .await
    .expect("depois");
    assert_eq!(
        depois.quantity,
        50 * GIB,
        "a atribuição não mudou a resolução"
    );

    // Um membro comum não atribui perfis.
    assert!(
        resource::assign_member_profile(&pool, &quem, &ids, quem.person_id, Some(pesado))
            .await
            .is_err(),
        "um membro comum atribuiu um perfil"
    );
}

/// A profile from another institution cannot be assigned by naming it.
#[tokio::test]
async fn atribuir_perfil_de_outra_org_e_recusado() {
    let Some(pool) = pool().await else { return };
    let org_a = organisation(&pool).await;
    let org_b = organisation(&pool).await;
    default_profile(&pool, org_a, 10 * GIB).await;
    let perfil_b = named_profile(&pool, org_b, "OUTRO", 99 * GIB).await;

    let admin_a = member(&pool, org_a, &[TechnicalRole::OrganisationAdmin]).await;
    let membro_a = member(&pool, org_a, &[TechnicalRole::ResearchMember]).await;
    let ids = CorrelationIds::generate();

    assert!(
        resource::assign_member_profile(&pool, &admin_a, &ids, membro_a.person_id, Some(perfil_b))
            .await
            .is_err(),
        "um perfil de outra instituição foi atribuído"
    );
}

/// Ensuring the default profile is idempotent — one default per organisation.
#[tokio::test]
async fn ensure_default_profile_e_idempotente() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;

    let a = resource::ensure_default_profile(&pool, org)
        .await
        .expect("primeira");
    let b = resource::ensure_default_profile(&pool, org)
        .await
        .expect("segunda");
    assert_eq!(a, b, "a segunda chamada criou um perfil diferente");

    let defaults: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM resource_profiles WHERE organisation_id = $1 AND is_default",
    )
    .bind(org)
    .fetch_one(&pool)
    .await
    .expect("contagem");
    assert_eq!(defaults, 1, "há mais do que um perfil por omissão");

    // E resolve os 10 GiB por omissão.
    let quem = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let ent = resource::resolve_entitlement(
        &pool,
        org,
        ResourceScopeType::Member,
        quem.person_id,
        ResourceType::PersistentStorage,
    )
    .await
    .expect("resolver");
    assert_eq!(ent.quantity, resource::DEFAULT_STORAGE_QUOTA_BYTES);
}
