//! AI conversation persistence with typed provenance (ADR-0309).
//!
//! A conversation is owner-private: a member sees only their own, and another
//! member's is indistinguishable from absent. The response turn carries typed
//! provenance, and a system turn is never retroactively a model turn.
//!
//! Skips when `OCINYE_TEST_DATABASE_URL` is unset; fails if set-but-unreachable.

use ocinye_contracts::{AiReasonCode, InteractionOrigin, InteractionStatus, TechnicalRole};
use ocinye_core::modules::intelligence::{self, ResponseTurn};
use ocinye_domain::Principal;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: a proveniência de conversas ficaria por verificar"
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
    let slug = format!("c{}", Uuid::new_v4().simple());
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

fn system_degraded() -> ResponseTurn<'static> {
    ResponseTurn {
        origin: InteractionOrigin::System,
        status: InteractionStatus::Degraded,
        reason_code: Some(AiReasonCode::AiNoProviderAvailable),
        model: None,
        provider: None,
        compute_node_id: None,
        content: "Nenhuma capacidade de inferência está disponível.",
    }
}

async fn record(
    pool: &PgPool,
    principal: &Principal,
    prompt: &str,
    turn: &ResponseTurn<'_>,
) -> Uuid {
    let mut tx = pool.begin().await.expect("tx");
    let id = intelligence::record_interaction(&mut tx, principal, None, None, prompt, turn)
        .await
        .expect("gravar");
    tx.commit().await.expect("commit");
    id
}

/// An interaction stores the member's prompt and the response, with provenance.
#[tokio::test]
async fn uma_interacao_grava_o_prompt_e_a_resposta_com_proveniencia() {
    let Some(pool) = pool().await else { return };
    let organisation_id = organisation(&pool).await;
    let quem = member(&pool, organisation_id).await;

    let id = record(&pool, &quem, "Cria uma função Rust.", &system_degraded()).await;

    let view = intelligence::get_conversation(&pool, &quem, id)
        .await
        .expect("consulta")
        .expect("conversa");

    assert_eq!(view.turns.len(), 2, "o prompt e a resposta");
    assert_eq!(view.turns[0].role, "member");
    assert_eq!(view.turns[0].content, "Cria uma função Rust.");
    assert_eq!(view.turns[1].role, "system");
    assert_eq!(view.turns[1].status.as_deref(), Some("degraded"));
    assert_eq!(
        view.turns[1].reason_code.as_deref(),
        Some("AI_NO_PROVIDER_AVAILABLE")
    );
    assert!(
        view.turns[1].model.is_none(),
        "sem modelo numa resposta de sistema"
    );
    // O título deriva do primeiro prompt.
    assert_eq!(view.title, "Cria uma função Rust.");
}

/// A system turn stays a system turn — never retroactively a model response.
#[tokio::test]
async fn a_verdade_historica_mantem_se() {
    let Some(pool) = pool().await else { return };
    let organisation_id = organisation(&pool).await;
    let quem = member(&pool, organisation_id).await;

    // Primeiro: uma resposta de sistema (sem inferência).
    let sistema = record(&pool, &quem, "Resume isto.", &system_degraded()).await;
    // Depois: uma resposta de modelo, noutra conversa.
    let modelo_turn = ResponseTurn {
        origin: InteractionOrigin::Model,
        status: InteractionStatus::Completed,
        reason_code: None,
        model: Some("qwen-coder"),
        provider: Some("ocinye-node-01"),
        compute_node_id: None,
        content: "Aqui está o resumo.",
    };
    let modelo = record(&pool, &quem, "Resume isto.", &modelo_turn).await;

    let v1 = intelligence::get_conversation(&pool, &quem, sistema)
        .await
        .expect("consulta")
        .expect("conversa");
    let v2 = intelligence::get_conversation(&pool, &quem, modelo)
        .await
        .expect("consulta")
        .expect("conversa");

    // A primeira continua a ser do sistema; a segunda é do modelo. A história
    // não é reescrita.
    assert_eq!(v1.turns[1].role, "system");
    assert!(v1.turns[1].model.is_none());
    assert_eq!(v2.turns[1].role, "model");
    assert_eq!(v2.turns[1].model.as_deref(), Some("qwen-coder"));
}

/// Another member cannot see, or even confirm the existence of, the conversation.
#[tokio::test]
async fn outra_pessoa_nao_ve_a_conversa() {
    let Some(pool) = pool().await else { return };
    let organisation_id = organisation(&pool).await;
    let dona = member(&pool, organisation_id).await;
    let outra = member(&pool, organisation_id).await;

    let id = record(&pool, &dona, "Privado.", &system_degraded()).await;

    // A dona vê; a outra recebe `None` — indistinguível de inexistente.
    assert!(intelligence::get_conversation(&pool, &dona, id)
        .await
        .expect("consulta")
        .is_some());
    assert!(
        intelligence::get_conversation(&pool, &outra, id)
            .await
            .expect("consulta")
            .is_none(),
        "IDOR: outra pessoa não pode abrir a conversa"
    );

    // E a lista é owner-scoped.
    let lista_outra = intelligence::list_conversations(&pool, &outra)
        .await
        .expect("lista");
    assert!(
        !lista_outra.iter().any(|c| c.id == id),
        "a conversa da dona não aparece na lista da outra"
    );
}
