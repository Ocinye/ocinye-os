//! Um nó de compute diz quem o controla e onde está — dois eixos, não um.
//!
//! O primeiro nó GPU real da Ocinye será software da Ocinye a correr numa
//! máquina alugada numa cloud de terceiros (OVHcloud). Alugar hardware não cede
//! controlo, e o modelo tem de o dizer sem inferir um eixo do outro (ADR-0503):
//! `institutional_control = OCINYE`, `physical_residency = THIRD_PARTY_CLOUD`.

use std::time::Duration;

use ocinye_contracts::{InstitutionalControl, NodeKind, Residency, TechnicalRole};
use ocinye_core::config::ComputeConfig;
use ocinye_core::modules::compute;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: o registo de nós ficaria por verificar"
        );
        return None;
    };
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL está definida mas a base não responde");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("c{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn platform_admin(pool: &PgPool, organisation_id: Uuid) -> Principal {
    let handle = format!("a{}", Uuid::new_v4().simple());
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
        .bind(TechnicalRole::PlatformAdmin.as_str())
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

fn config() -> ComputeConfig {
    ComputeConfig {
        enrollment_token_ttl: Duration::from_secs(3600),
        node_offline_after: Duration::from_secs(120),
    }
}

/// Registar um nó como controlado pela Ocinye numa cloud de terceiros guarda os
/// dois eixos, e lê-os de volta tipados.
#[tokio::test]
async fn um_no_alugado_e_controlado_pela_ocinye_e_reside_em_terceiros() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let admin = platform_admin(&pool, org).await;
    let ids = CorrelationIds::generate();

    let mut tx = pool.begin().await.expect("tx");
    let enrolled = compute::register_node(
        &mut tx,
        &admin,
        &ids,
        &config(),
        compute::NewNode {
            identifier: format!("OVH-L40S-{}", &Uuid::new_v4().simple().to_string()[..6]),
            display_name: "Nó GPU de validação".to_owned(),
            kind: NodeKind::Gpu,
            location_label: Some("OVHcloud Europe".to_owned()),
            institutional_control: InstitutionalControl::Ocinye,
            physical_residency: Residency::ThirdPartyCloud,
        },
    )
    .await
    .expect("regista o nó");
    tx.commit().await.expect("commit");

    // Os dois eixos vieram tipados, e são independentes: controlo da Ocinye,
    // hardware de terceiros. Um não implica o outro.
    let node = enrolled.node;
    assert_eq!(node.institutional_control(), InstitutionalControl::Ocinye);
    assert_eq!(node.physical_residency(), Residency::ThirdPartyCloud);
    assert!(
        !node.physical_residency().is_ocinye_owned(),
        "uma máquina alugada não é infraestrutura da Ocinye"
    );
    assert_eq!(node.location_label.as_deref(), Some("OVHcloud Europe"));
}

/// Sem escolha explícita, os defaults são honestos: controlo da Ocinye (o nó
/// corre o nosso agente), residência **não declarada** (não se afirma o que não
/// se sabe).
#[tokio::test]
async fn os_defaults_de_controlo_e_residencia_sao_honestos() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let admin = platform_admin(&pool, org).await;
    let ids = CorrelationIds::generate();

    let mut tx = pool.begin().await.expect("tx");
    let enrolled = compute::register_node(
        &mut tx,
        &admin,
        &ids,
        &config(),
        compute::NewNode {
            identifier: format!("NODE-{}", &Uuid::new_v4().simple().to_string()[..6]),
            display_name: "Nó sem residência declarada".to_owned(),
            kind: NodeKind::Gpu,
            location_label: None,
            institutional_control: InstitutionalControl::default(),
            physical_residency: Residency::default(),
        },
    )
    .await
    .expect("regista o nó");
    tx.commit().await.expect("commit");

    assert_eq!(
        enrolled.node.institutional_control(),
        InstitutionalControl::Ocinye
    );
    assert_eq!(enrolled.node.physical_residency(), Residency::Undeclared);
}
