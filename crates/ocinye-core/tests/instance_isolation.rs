//! Uma instância não vê os recursos de outra, mesmo na mesma base de dados.
//!
//! O modelo por omissão do Ocinye OS é uma instância por instalação, e o domínio
//! inteiro já se delimita por `organisation_id`. Mas `ai_models` não tem
//! organização própria: pertence ao nó que o reporta. Até à Parte 1 da
//! generalização, o Model Router lia a tabela inteira, e um nó enrolado numa
//! instância servia os pedidos de outra (F-08 da linha de base).
//!
//! Estas provas fixam a fronteira pelo sítio onde ela passa: o nó.

use std::collections::BTreeMap;

use ocinye_contracts::{AiCapability, AiReasonCode};
use ocinye_core::config::AiConfig;
use ocinye_core::modules::intelligence::{self, ModelResolution};
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: o isolamento entre instâncias ficaria por verificar"
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

async fn instancia(pool: &PgPool) -> Uuid {
    let slug = format!("i{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("instância")
}

/// Um nó vivo na instância, a reportar um modelo que serve `GENERAL`.
async fn no_com_modelo(pool: &PgPool, organisation_id: Uuid) -> String {
    let identifier = format!("N{}", &Uuid::new_v4().simple().to_string()[..12]);
    let node_id: Uuid = sqlx::query_scalar(
        "INSERT INTO compute_nodes (organisation_id, identifier, display_name, status, last_seen_at)
         VALUES ($1, $2, $2, 'online', now()) RETURNING id",
    )
    .bind(organisation_id)
    .bind(&identifier)
    .fetch_one(pool)
    .await
    .expect("nó");
    let model_name = format!("modelo-{identifier}");
    sqlx::query(
        "INSERT INTO ai_models
             (provider_kind, provider_name, node_id, model_name, capabilities, status, reported_at)
         VALUES ('ocinye_node', 'teste', $1, $2, '[\"GENERAL\"]'::jsonb, 'available', now())",
    )
    .bind(node_id)
    .bind(&model_name)
    .execute(pool)
    .await
    .expect("modelo");
    model_name
}

fn ai_config() -> AiConfig {
    AiConfig {
        capability_map: BTreeMap::new(),
        allow_external_providers: false,
        embedding_provider: "none".to_owned(),
    }
}

#[tokio::test]
async fn um_no_de_outra_instancia_nao_serve_os_pedidos_desta() {
    let Some(pool) = pool().await else { return };
    let a = instancia(&pool).await;
    let b = instancia(&pool).await;
    let modelo_de_a = no_com_modelo(&pool, a).await;

    // A instância dona do nó resolve o modelo do seu nó.
    match intelligence::resolve_capability(&pool, a, &ai_config(), AiCapability::General)
        .await
        .expect("resolução em A")
    {
        ModelResolution::Resolved(model) => assert_eq!(model.model_name, modelo_de_a),
        ModelResolution::NoCandidate(reason) => {
            panic!("A devia resolver o modelo do seu nó; veio {reason:?}")
        }
    }

    // A outra não o vê — nem como candidato, nem como «existe inventário».
    match intelligence::resolve_capability(&pool, b, &ai_config(), AiCapability::General)
        .await
        .expect("resolução em B")
    {
        ModelResolution::NoCandidate(reason) => assert_eq!(
            reason,
            AiReasonCode::AiNoProviderAvailable,
            "para B não há inventário nenhum: a razão é «nenhum fornecedor», não «nenhum compatível»"
        ),
        ModelResolution::Resolved(model) => panic!(
            "o modelo {} do nó de A serviu um pedido de B",
            model.model_name
        ),
    }
}
