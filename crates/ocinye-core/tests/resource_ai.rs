//! AI request admission against the resource-governance entitlement (ADR-0108).
//!
//! A member's AI usage is measured from the append-only ledger; the limit is
//! their `model_access` entitlement; a request over the limit is refused
//! fail-closed; with no entitlement configured the limit is zero and everything
//! is admitted; and the SQL limit the admission uses equals the Rust resolver,
//! so the two never disagree — the same discipline storage already proves.
//!
//! Skips when `OCINYE_TEST_DATABASE_URL` is unset; fails if set-but-unreachable.

use ocinye_contracts::{ResourceScopeType, ResourceType};
use ocinye_core::modules::resource;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: a admissão de IA ficaria por verificar"
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
    let slug = format!("a{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn member(pool: &PgPool, organisation_id: Uuid) -> Uuid {
    let handle = format!("p{}", Uuid::new_v4().simple());
    sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
         VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@ocinye.com"))
    .fetch_one(pool)
    .await
    .expect("pessoa")
}

/// Grant a `model_access` allocation of the given source and quantity.
async fn allocate(
    pool: &PgPool,
    organisation_id: Uuid,
    person_id: Uuid,
    source: &str,
    quantity: i64,
) {
    sqlx::query(
        "INSERT INTO resource_allocations
             (organisation_id, resource_type, unit, scope_type, scope_id, quantity, source, reason)
         VALUES ($1, 'model_access', 'count', 'member', $2, $3, $4, 'prova')",
    )
    .bind(organisation_id)
    .bind(person_id)
    .bind(quantity)
    .bind(source)
    .execute(pool)
    .await
    .expect("alocação");
}

/// The admission refuses above the quota and admits at or below it, fail-closed.
#[tokio::test]
async fn a_admissao_recusa_acima_da_quota() {
    let Some(pool) = pool().await else { return };
    let organisation_id = organisation(&pool).await;
    let person_id = member(&pool, organisation_id).await;
    allocate(&pool, organisation_id, person_id, "override", 3).await;

    // Zero used: a single access is admitted.
    let mut tx = pool.begin().await.expect("tx");
    assert!(resource::admit_ai_access(&mut tx, person_id, 1)
        .await
        .expect("admissão"));
    drop(tx);

    // Record two accesses through the ledger.
    for _ in 0..2 {
        resource::record_ai_access(&pool, organisation_id, person_id, None, None, None, "t", 1)
            .await
            .expect("uso");
    }
    assert_eq!(
        resource::ai_access_used(&pool, person_id)
            .await
            .expect("uso"),
        2
    );

    // 2 + 1 <= 3 is admitted; 2 + 2 > 3 is refused.
    let mut tx = pool.begin().await.expect("tx");
    assert!(resource::admit_ai_access(&mut tx, person_id, 1)
        .await
        .expect("admissão"));
    assert!(!resource::admit_ai_access(&mut tx, person_id, 2)
        .await
        .expect("admissão"));
    drop(tx);
}

/// With no `model_access` entitlement, the limit is zero and all is admitted.
#[tokio::test]
async fn sem_entitlement_admite_tudo() {
    let Some(pool) = pool().await else { return };
    let organisation_id = organisation(&pool).await;
    let person_id = member(&pool, organisation_id).await;

    assert_eq!(
        resource::ai::ai_access_limit(&pool, person_id)
            .await
            .expect("limite"),
        0,
        "sem entitlement, o limite é zero (ilimitado)"
    );
    let mut tx = pool.begin().await.expect("tx");
    assert!(
        resource::admit_ai_access(&mut tx, person_id, 1_000_000)
            .await
            .expect("admissão"),
        "limite zero admite tudo, como o storage"
    );
    drop(tx);
}

/// The SQL limit the admission uses equals the Rust resolver used elsewhere.
#[tokio::test]
async fn o_limite_sql_iguala_o_resolvedor() {
    let Some(pool) = pool().await else { return };
    let organisation_id = organisation(&pool).await;
    let person_id = member(&pool, organisation_id).await;

    // An override (base) plus a live temporary sum on top: 10 + 5 = 15.
    allocate(&pool, organisation_id, person_id, "override", 10).await;
    allocate(&pool, organisation_id, person_id, "temporary", 5).await;

    let sql = resource::ai::ai_access_limit(&pool, person_id)
        .await
        .expect("limite sql");
    let resolved = resource::resolve_entitlement(
        &pool,
        organisation_id,
        ResourceScopeType::Member,
        person_id,
        ResourceType::ModelAccess,
    )
    .await
    .expect("resolvedor");

    assert_eq!(sql, 15, "override 10 + temporário 5");
    assert_eq!(
        sql, resolved.quantity,
        "o SQL e o resolvedor têm de concordar"
    );
}
