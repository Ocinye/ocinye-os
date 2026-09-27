//! O Terminal pelo Core: `POST /api/v1/commands/exec` (ADR-0312).
//!
//! # O que estas provas medem
//!
//! Que a linha é lida pelo parser determinístico e corre só por capabilities,
//! com a autoridade da pessoa: `whoami` diz quem é; o contexto de um separador
//! só aponta para um ambiente que a pessoa alcança, e é reautorizado a cada
//! comando; um comando que precisa de ambiente recusa no pessoal; um ambiente
//! alheio não aparece na lista e não pode ser usado; sintaxe de shell do
//! anfitrião é 126, comando desconhecido é 127, e nenhum deles executa nada.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ocinye_contracts::{SessionState, TechnicalRole};
use ocinye_core::authn::TokenVerifier;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{self, Authenticator, Throttle};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::password::{Hasher, HashingParams, Secret};
use ocinye_core_server::routes;
use ocinye_core_server::state::AppState;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// Salta quando não há base de dados; **falha** quando há e algo corre mal.
macro_rules! pool {
    () => {{
        let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
            eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
            return;
        };
        let pool = PgPool::connect(&url).await.expect("base de dados");
        sqlx::migrate!("../../migrations").run(&pool).await.expect("migrations");
        ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
        pool
    }};
}

fn config() -> CoreConfig {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: uma escrita, antes de qualquer teste começar trabalho.
        unsafe {
            std::env::set_var("OCINYE_DATABASE_URL", "postgres://x/x");
        }
    });
    CoreConfig::from_env().expect("configuração de teste")
}

fn nucleo(pool: PgPool, organisation_id: Uuid) -> AppState {
    let config = config();
    let verifier = TokenVerifier::new(config.oidc.clone()).expect("verificador");
    let authenticator = Arc::new(Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: config.auth.argon2_memory_kib,
            iterations: config.auth.argon2_iterations,
            parallelism: config.auth.argon2_parallelism,
        }),
        Throttle {
            per_ip: config.auth.throttle_per_ip,
            per_email: config.auth.throttle_per_email,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));
    let mail_registry = Arc::new(ocinye_core::modules::mail::ProviderRegistry::new(
        Arc::new(UnconfiguredProvider),
        config.mail.clone(),
        config.sealing_key.clone(),
    ));
    AppState {
        pool,
        config: Arc::new(config),
        verifier,
        authenticator,
        store: None,
        embeddings: None,
        inference: Arc::new(ocinye_core::modules::intelligence::NoProvider),
        mail_registry,
        realtime: Arc::new(ocinye_core::realtime::Realtime::ausente()),
        mail_probe: Arc::new(Sonda),
        capabilities: Arc::new(ocinye_core::capabilities::Capabilities::empty().expect("motor")),
        organisation_id,
    }
}

struct Sonda;

#[async_trait::async_trait]
impl ocinye_core::modules::mail::provider::CredentialProbe for Sonda {
    async fn verify(
        &self,
        _endereco: &str,
        _username: &str,
        _senha: &str,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Ok(())
    }
}

async fn organisation(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("trm-{}", Uuid::new_v4().simple()))
        .bind("Instituição do Terminal")
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn member(pool: &PgPool, organisation_id: Uuid, roles: &[TechnicalRole]) -> (Principal, Secret) {
    let handle = format!("t{}", Uuid::new_v4().simple());
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
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let seed = Uuid::new_v4().as_u128();
        *byte = ((seed >> ((index % 16) * 8)) & 0xff) as u8;
    }
    let token = Secret::new(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
    sqlx::query(
        "INSERT INTO sessions (person_id, token_digest, state, mfa_satisfied, expires_at, user_agent)
             VALUES ($1, $2, $3, true, now() + interval '1 hour', 'terminal-http-test')",
    )
    .bind(person_id)
    .bind(identity::session_digest(&token))
    .bind(SessionState::Active.as_str())
    .execute(pool)
    .await
    .expect("sessão");
    let record = identity::person_by_id(pool, person_id).await.expect("consulta").expect("pessoa");
    let principal = identity::principal_for_person(pool, &record).await.expect("principal");
    (principal, token)
}

async fn post(state: &AppState, token: &Secret, path: &str, body: Value) -> (StatusCode, Value) {
    let response = routes::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header(header::AUTHORIZATION, format!("Bearer {}", token.expose()))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .expect("pedido"),
        )
        .await
        .expect("resposta");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.expect("corpo");
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn exec(state: &AppState, token: &Secret, line: &str, context: Option<Uuid>) -> Value {
    let (status, body) = post(
        state,
        token,
        "/api/v1/commands/exec",
        json!({ "line": line, "context": context.map(|c| c.to_string()) }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{line}: {body}");
    body
}

/// Uma unidade e um ambiente nela, criados pelas operações do Core.
async fn ambiente(state: &AppState, pool: &PgPool, org: Uuid) -> (Uuid, Principal, Secret) {
    let (admin, token) = member(pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let (status, unit) = post(
        state,
        &token,
        "/api/v1/units",
        json!({ "code": format!("T{}", &Uuid::new_v4().simple().to_string()[..6]), "name": "Unidade do Terminal" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{unit}");
    let unit_id = Uuid::parse_str(unit["id"].as_str().expect("id")).expect("uuid");
    let mut principal = admin.clone();
    let mut tx = pool.begin().await.expect("tx");
    let (_, workspace) = ocinye_core::modules::research::create_idea(
        &mut tx,
        &mut principal,
        &CorrelationIds::generate(),
        ocinye_core::modules::research::NewIdea {
            unit_id,
            title: "Ideia do Terminal".to_owned(),
            summary: None,
            research_question: None,
            hypothesis: None,
            motivation: None,
            keywords: Vec::new(),
            // Confidencial: só quem tem papel nele o vê. Um ambiente interno é
            // visível a todos os membros da Instância, e não serviria para
            // provar que um alheio não o alcança.
            classification: Some(ocinye_contracts::Classification::Confidential),
        },
    )
    .await
    .expect("ideia");
    tx.commit().await.expect("commit");
    (workspace.id, admin, token)
}

fn note_key(body: &Value) -> &str {
    body["blocks"][0]["key"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn whoami_diz_quem_pede_e_o_contexto() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (me, token) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let body = exec(&state, &token, "whoami", None).await;
    assert_eq!(body["exit"], 0, "{body}");
    assert_eq!(body["capability"], "identity.self.read");
    let rows = body["blocks"][0]["rows"].as_array().expect("factos");
    assert!(rows.iter().any(|r| r[0] == "display_name" && r[1] == me.display_name.as_str()), "{body}");
    assert!(rows.iter().any(|r| r[0] == "instance" && r[1] == "Instituição do Terminal"), "{body}");
    assert!(rows.iter().any(|r| r[0] == "context" && r[1] == "personal"), "{body}");

    let json = exec(&state, &token, "whoami --json", None).await;
    assert_eq!(json["blocks"][0]["kind"], "json", "{json}");
}

#[tokio::test]
async fn o_contexto_so_aponta_para_ambientes_alcancaveis() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (ws, _admin, admin_token) = ambiente(&state, &pool, org).await;
    let (_outro, outro_token) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    // Quem o criou vê-o na lista, entra nele por código, e as tarefas correm lá.
    let list = exec(&state, &admin_token, "context list", None).await;
    assert_eq!(list["exit"], 0, "{list}");
    let code = list["blocks"][0]["rows"][0][0].as_str().expect("código").to_owned();
    let usar = exec(&state, &admin_token, &format!("context use {code}"), None).await;
    assert_eq!(usar["exit"], 0, "{usar}");
    assert_eq!(usar["context"]["workspace_id"], ws.to_string(), "{usar}");
    let tarefas = exec(&state, &admin_token, "tasks list", Some(ws)).await;
    assert_eq!(tarefas["exit"], 0, "{tarefas}");
    assert_eq!(tarefas["blocks"][0]["kind"], "table", "{tarefas}");

    // Sem ambiente, `tasks` diz porquê.
    let pessoal = exec(&state, &admin_token, "tasks list", None).await;
    assert_eq!(pessoal["exit"], 2, "{pessoal}");
    assert_eq!(note_key(&pessoal), "ocsh.err.needs_workspace");

    // Outra pessoa, sem papel no ambiente e sem o ver, não o encontra — nem
    // pelo código, nem pelo identificador enviado como contexto.
    let alheio = exec(&state, &outro_token, &format!("context use {code}"), None).await;
    assert_eq!(alheio["exit"], 1, "{alheio}");
    assert_eq!(note_key(&alheio), "ocsh.err.context_not_found");
    let forjado = exec(&state, &outro_token, "tasks list", Some(ws)).await;
    assert_eq!(forjado["exit"], 77, "{forjado}");
    assert_eq!(note_key(&forjado), "ocsh.err.context_unreachable");
    assert!(forjado["context"]["workspace_id"].is_null(), "o contexto forjado não pode ficar: {forjado}");
    let lista_alheia = exec(&state, &outro_token, "context list", None).await;
    assert!(
        !lista_alheia.to_string().contains(&code),
        "a lista de outra pessoa revela o ambiente: {lista_alheia}"
    );
}

#[tokio::test]
async fn sintaxe_do_anfitriao_e_comandos_desconhecidos_nao_executam() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_me, token) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    for (line, exit) in [
        ("; rm -rf /", 126),
        ("whoami && rm -rf /", 126),
        ("whoami $(id)", 126),
        ("whoami `id`", 126),
        ("whoami > /etc/passwd", 126),
        ("sudo whoami", 126),
        ("bash -c 'id'", 126),
        ("whoami | /bin/sh", 2),
        ("ls -la", 127),
        ("resume o meu trabalho de hoje", 127),
        ("whoami \"aberta", 2),
    ] {
        let body = exec(&state, &token, line, None).await;
        assert_eq!(body["exit"], exit, "{line}: {body}");
        assert!(body["capability"].is_null(), "{line} executou uma capability: {body}");
    }

    // Uma linha desconhecida nunca chega ao Nye: nenhuma interacção de IA fica
    // registada por ela.
    let conversas: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_jobs")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    let _ = exec(&state, &token, "explica-me este projecto", None).await;
    let depois: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_jobs")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    assert_eq!(conversas, depois, "uma linha desconhecida chegou à IA");
}

#[tokio::test]
async fn a_ajuda_so_mostra_o_que_a_pessoa_pode_usar() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (_me, membro) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let (_op, operador) = member(&pool, org, &[TechnicalRole::PlatformAdmin]).await;

    let de = |body: &Value| body["blocks"][0]["entries"].to_string();
    let m = exec(&state, &membro, "help", None).await;
    let o = exec(&state, &operador, "help", None).await;
    assert!(de(&m).contains("whoami") && de(&m).contains("context list"), "{m}");
    assert!(de(&o).contains("nodes list"), "o operador não vê `nodes`: {o}");
    if !de(&m).contains("nodes list") {
        // Escondido na ajuda, e recusado se escrito.
        let n = exec(&state, &membro, "nodes list", None).await;
        assert_eq!(n["exit"], 77, "{n}");
    }
    let h = exec(&state, &membro, "tasks --help", None).await;
    assert_eq!(h["blocks"][0]["topic"], "tasks", "{h}");
}

#[tokio::test]
async fn sem_sessao_nao_ha_terminal() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);
    let (status, _) = post(&state, &Secret::new("x".repeat(64)), "/api/v1/commands/exec", json!({"line": "whoami"})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
