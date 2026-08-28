//! O calendário através do HTTP real.
//!
//! # Porque isto passa pelo router
//!
//! Porque é por aqui que uma pessoa chega. Um teste que chamasse a operação do
//! Core directamente provaria a operação e não o caminho — e o caminho tem
//! extractores, autenticação, validação de intervalo e tradução de erros
//! temporais, que são exactamente as coisas que podem estar mal sem que o Core
//! dê por isso.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ocinye_contracts::{SessionState, TechnicalRole, UnitRole};
use ocinye_core::authn::TokenVerifier;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{self, Authenticator, Throttle};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::modules::organisation;
use ocinye_core::password::{Hasher, HashingParams, Secret};
use ocinye_core_server::routes;
use ocinye_core_server::state::AppState;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// ── Fixture ─────────────────────────────────────────────────────────────

async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations must apply to the test database");
    Some(pool)
}

macro_rules! institution {
    () => {{
        let Some(pool) = pool().await else {
            eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
            return;
        };
        let organisation_id = organisation(&pool).await;
        let state = state(pool.clone(), organisation_id);
        (pool, organisation_id, state)
    }};
}

/// The configuration a test Core runs on.
///
/// `from_env` requires exactly one variable, and the test database is the right
/// value for it. Everything else takes its documented default, which is what a
/// Core with no storage and no mail is supposed to be.
fn config() -> CoreConfig {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let url = std::env::var("OCINYE_TEST_DATABASE_URL").unwrap_or_default();
        std::env::set_var("OCINYE_DATABASE_URL", url);
    });
    CoreConfig::from_env().expect("configuração de teste")
}

fn state(pool: PgPool, organisation_id: Uuid) -> AppState {
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
            per_username: config.auth.throttle_per_username,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));

    AppState {
        pool,
        config: Arc::new(config),
        verifier,
        authenticator,
        // Nenhuma das cinco operações toca em ficheiros: a paridade que aqui se
        // mede não precisa de armazenamento, e levantá-lo por ritual só tornava
        // o teste dependente de infraestrutura que não exercita.
        store: None,
        inference: Arc::new(ocinye_core::modules::intelligence::NoProvider),
        mail_provider: Arc::new(UnconfiguredProvider),
        capabilities: std::sync::Arc::new(
            ocinye_core::capabilities::Capabilities::empty().expect("motor de capacidades"),
        ),
        organisation_id,
    }
}

async fn organisation(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("par-{}", Uuid::new_v4().simple()))
        .bind("Instituição de paridade")
        .fetch_one(pool)
        .await
        .expect("organização")
}

/// A person, and a live session token for them.
///
/// The token is minted by the same function signing in uses, so what the HTTP
/// entry receives is the same kind of credential a member actually holds.
async fn member(
    pool: &PgPool,
    organisation_id: Uuid,
    roles: &[TechnicalRole],
) -> (Principal, Secret) {
    let handle = format!("m{}", Uuid::new_v4().simple());

    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, username, status)
             VALUES ($1, $2, $3, $2, 'active') RETURNING id",
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

    let token = sessao(pool, person_id).await;

    (principal(pool, person_id).await, token)
}

/// A live session, minted the way the Core mints one.
///
/// The row is written here rather than through `create_session`, which the
/// module does not re-export. The token is still hashed by the Core's own
/// [`identity::session_digest`], so the credential the HTTP entry receives is
/// indistinguishable from one a sign-in produced — a fixture that hashed it
/// some other way would be testing its own arithmetic.
async fn sessao(pool: &PgPool, person_id: Uuid) -> Secret {
    let mut bytes = [0_u8; 32];
    getrandom(&mut bytes);
    let token = Secret::new(
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );

    sqlx::query(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at, user_agent)
             VALUES ($1, $2, $3, now() + interval '1 hour', 'parity-test')",
    )
    .bind(person_id)
    .bind(identity::session_digest(&token))
    .bind(SessionState::Active.as_str())
    .execute(pool)
    .await
    .expect("sessão");

    token
}

/// Enough entropy for a token nobody is meant to guess.
fn getrandom(buffer: &mut [u8; 32]) {
    for (index, byte) in buffer.iter_mut().enumerate() {
        let seed = Uuid::new_v4().as_u128();
        *byte = ((seed >> ((index % 16) * 8)) & 0xff) as u8;
    }
}

async fn principal(pool: &PgPool, person_id: Uuid) -> Principal {
    let record = identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

/// Um pedido autenticado ao router real.
async fn pedir(
    state: &AppState,
    token: &Secret,
    metodo: &str,
    caminho: &str,
    corpo: Option<Value>,
) -> (StatusCode, Value) {
    let mut construtor = Request::builder()
        .method(metodo)
        .uri(caminho)
        .header(header::AUTHORIZATION, format!("Bearer {}", token.expose()));

    let body = match corpo {
        Some(valor) => {
            construtor = construtor.header(header::CONTENT_TYPE, "application/json");
            Body::from(valor.to_string())
        }
        None => Body::empty(),
    };

    let response = routes::router(state.clone())
        .oneshot(construtor.body(body).expect("pedido"))
        .await
        .expect("resposta");

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("corpo");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn amanha_as(hora: u32) -> Value {
    let dia = (chrono::Utc::now() + chrono::Duration::days(1)).date_naive();
    json!({
        "kind": "timed",
        "starts_at": dia.and_hms_opt(hora, 0, 0).expect("hora"),
        "ends_at": dia.and_hms_opt(hora + 1, 0, 0).expect("hora"),
        "timezone": "Europe/Lisbon"
    })
}

fn intervalo_de_uma_semana() -> String {
    let agora = chrono::Utc::now();
    format!(
        "/api/v1/calendar/agenda?from={}&to={}",
        urlencoding(&(agora - chrono::Duration::days(1)).to_rfc3339()),
        urlencoding(&(agora + chrono::Duration::days(7)).to_rfc3339())
    )
}

fn urlencoding(valor: &str) -> String {
    valor.replace(':', "%3A").replace('+', "%2B")
}

async fn unidade_com_gestor(pool: &PgPool, org: Uuid) -> (Uuid, Principal, Secret) {
    let (admin, _) = member(pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let mut tx = pool.begin().await.expect("tx");
    let unidade = organisation::create_unit(
        &mut tx,
        &admin,
        &CorrelationIds::generate(),
        organisation::NewUnit {
            code: format!("H{}", &Uuid::new_v4().simple().to_string()[..5]),
            name: "Unidade".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("unidade");
    tx.commit().await.expect("commit");

    let (gestor, token) = member(pool, org, &[TechnicalRole::ResearchMember]).await;
    let mut tx = pool.begin().await.expect("tx");
    organisation::add_unit_member(
        &mut tx,
        &admin,
        &CorrelationIds::generate(),
        unidade.id,
        gestor.person_id,
        UnitRole::Manager,
    )
    .await
    .expect("membro");
    tx.commit().await.expect("commit");

    (unidade.id, principal(pool, gestor.person_id).await, token)
}

// ── O caminho completo ──────────────────────────────────────────────────

/// Marcar, ler, listar, alterar e cancelar — tudo por HTTP.
#[tokio::test]
async fn o_ciclo_de_um_evento_pelo_http() {
    let (pool, org, state) = institution!();
    let (_, _, token) = unidade_com_gestor(&pool, org).await;

    // Marcar.
    let (status, criado) = pedir(
        &state,
        &token,
        "POST",
        "/api/v1/calendar/events",
        Some(json!({
            "scope": "personal",
            "title": "Consulta",
            "occurrence": amanha_as(14)
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "marcar recusou: {criado}");
    let id = criado["id"].as_str().expect("identificador").to_owned();
    assert_eq!(criado["state"], "scheduled");
    assert_eq!(criado["all_day"], false);
    assert_eq!(criado["timezone"], "Europe/Lisbon");

    // Ler.
    let (status, lido) = pedir(
        &state,
        &token,
        "GET",
        &format!("/api/v1/calendar/events/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "ler recusou: {lido}");
    assert_eq!(lido["title"], "Consulta");

    // Na agenda.
    let (status, agenda) = pedir(&state, &token, "GET", &intervalo_de_uma_semana(), None).await;
    assert_eq!(status, StatusCode::OK, "a agenda recusou: {agenda}");
    let itens = agenda["items"].as_array().expect("itens");
    assert!(
        itens.iter().any(|item| item["id"] == criado["id"]),
        "o evento marcado não aparece na agenda"
    );
    assert_eq!(
        agenda["total"].as_u64().expect("total") as usize,
        itens.iter().filter(|i| i["kind"] == "event").count(),
        "a contagem e a lista discordam"
    );

    // Alterar.
    let (status, alterado) = pedir(
        &state,
        &token,
        "PATCH",
        &format!("/api/v1/calendar/events/{id}"),
        Some(json!({ "title": "Consulta adiada" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "alterar recusou: {alterado}");
    assert_eq!(alterado["title"], "Consulta adiada");
    assert_eq!(
        alterado["scope"], criado["scope"],
        "uma alteração mexeu no âmbito"
    );

    // Cancelar, duas vezes.
    for tentativa in 1..=2 {
        let (status, cancelado) = pedir(
            &state,
            &token,
            "POST",
            &format!("/api/v1/calendar/events/{id}/cancel"),
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "cancelar à {tentativa}.ª vez: {cancelado}"
        );
        assert_eq!(cancelado["state"], "cancelled");
    }
}

/// Uma hora que não existe chega como frase, e não como 500.
#[tokio::test]
async fn as_horas_das_transicoes_chegam_como_validacao() {
    let (pool, org, state) = institution!();
    let (_, _, token) = unidade_com_gestor(&pool, org).await;

    // 2026-03-29, 02:30 em Paris: o relógio salta essa hora.
    let (status, corpo) = pedir(
        &state,
        &token,
        "POST",
        "/api/v1/calendar/events",
        Some(json!({
            "scope": "personal",
            "title": "Hora que não existe",
            "occurrence": {
                "kind": "timed",
                "starts_at": "2026-03-29T02:30:00",
                "ends_at": "2026-03-29T03:30:00",
                "timezone": "Europe/Paris"
            }
        })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "uma hora inexistente devia ser recusada como validação: {corpo}"
    );
    let mensagem = corpo["message"].as_str().unwrap_or_default();
    assert!(
        mensagem.contains("não existe"),
        "a mensagem não explica o que se passou: {mensagem}"
    );

    // Zona inválida, pelo mesmo caminho.
    let (status, corpo) = pedir(
        &state,
        &token,
        "POST",
        "/api/v1/calendar/events",
        Some(json!({
            "scope": "personal",
            "title": "Zona inventada",
            "occurrence": {
                "kind": "timed",
                "starts_at": "2026-06-01T10:00:00",
                "ends_at": "2026-06-01T11:00:00",
                "timezone": "Europa/Paris"
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{corpo}");
}

/// Um intervalo absurdo é recusado antes de chegar à base.
#[tokio::test]
async fn um_intervalo_sem_limite_e_recusado() {
    let (pool, org, state) = institution!();
    let (_, _, token) = unidade_com_gestor(&pool, org).await;

    let (status, corpo) = pedir(
        &state,
        &token,
        "GET",
        "/api/v1/calendar/agenda?from=1900-01-01T00%3A00%3A00Z&to=2500-01-01T00%3A00%3A00Z",
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "um intervalo de seiscentos anos foi aceite: {corpo}"
    );

    // E o inverso: um intervalo que acaba antes de começar.
    let (status, _) = pedir(
        &state,
        &token,
        "GET",
        "/api/v1/calendar/agenda?from=2026-06-02T00%3A00%3A00Z&to=2026-06-01T00%3A00%3A00Z",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

/// O identificador de um evento alheio não abre nada, e não diz que existe.
#[tokio::test]
async fn o_http_nao_e_um_oraculo_de_existencia() {
    let (pool, org, state) = institution!();
    let (_, _, token_alice) = unidade_com_gestor(&pool, org).await;
    let (_, token_bruno) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let (_, criado) = pedir(
        &state,
        &token_alice,
        "POST",
        "/api/v1/calendar/events",
        Some(json!({
            "scope": "personal",
            "title": "Pessoal",
            "occurrence": amanha_as(9)
        })),
    )
    .await;
    let id = criado["id"].as_str().expect("id").to_owned();

    let (existe, _) = pedir(
        &state,
        &token_bruno,
        "GET",
        &format!("/api/v1/calendar/events/{id}"),
        None,
    )
    .await;
    let (inventado, _) = pedir(
        &state,
        &token_bruno,
        "GET",
        &format!("/api/v1/calendar/events/{}", Uuid::new_v4()),
        None,
    )
    .await;

    assert_eq!(existe, StatusCode::NOT_FOUND);
    assert_eq!(
        existe, inventado,
        "«existe mas não é seu» e «não existe» dão respostas distinguíveis"
    );

    // E cancelar também não.
    let (cancelar, _) = pedir(
        &state,
        &token_bruno,
        "POST",
        &format!("/api/v1/calendar/events/{id}/cancel"),
        None,
    )
    .await;
    assert_eq!(cancelar, StatusCode::NOT_FOUND);
}

/// Sem sessão, o calendário não responde.
#[tokio::test]
async fn o_calendario_exige_sessao() {
    let (pool, org, state) = institution!();
    let _ = (pool, org);

    for (metodo, caminho) in [
        (
            "GET",
            "/api/v1/calendar/agenda?from=2026-06-01T00%3A00%3A00Z&to=2026-06-02T00%3A00%3A00Z",
        ),
        ("POST", "/api/v1/calendar/events"),
        ("GET", "/api/v1/notifications"),
    ] {
        let request = Request::builder()
            .method(metodo)
            .uri(caminho)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .expect("pedido");
        let response = routes::router(state.clone())
            .oneshot(request)
            .await
            .expect("resposta");
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "«{metodo} {caminho}» respondeu sem sessão"
        );
    }
}

/// Os lembretes e as notificações têm caminho HTTP real.
#[tokio::test]
async fn lembretes_e_notificacoes_pelo_http() {
    let (pool, org, state) = institution!();
    let (_, quem, token) = unidade_com_gestor(&pool, org).await;

    let daqui_a_uma_hora = chrono::Utc::now() + chrono::Duration::hours(1);
    let (status, lembrete) = pedir(
        &state,
        &token,
        "POST",
        "/api/v1/calendar/reminders",
        Some(json!({ "note": "rever o relatório", "trigger_at": daqui_a_uma_hora })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "criar lembrete: {lembrete}");
    let id = lembrete["id"].as_str().expect("id").to_owned();

    let (status, lista) = pedir(&state, &token, "GET", "/api/v1/calendar/reminders", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        lista
            .as_array()
            .expect("lista")
            .iter()
            .any(|r| r["id"] == lembrete["id"]),
        "o lembrete criado não aparece na lista"
    );

    // Adiar, e depois dispensar.
    let (status, adiado) = pedir(
        &state,
        &token,
        "POST",
        &format!("/api/v1/calendar/reminders/{id}/snooze"),
        Some(json!({ "minutes": 30 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{adiado}");
    assert_eq!(adiado["state"], "snoozed");

    let (status, dispensado) = pedir(
        &state,
        &token,
        "POST",
        &format!("/api/v1/calendar/reminders/{id}/dismiss"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{dispensado}");
    assert_eq!(dispensado["state"], "dismissed");

    // As notificações começam vazias, e o contador diz zero — não «desconhecido».
    let (status, notificacoes) = pedir(&state, &token, "GET", "/api/v1/notifications", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(notificacoes["unread"], 0);
    assert_eq!(
        notificacoes["notifications"]
            .as_array()
            .expect("lista")
            .len(),
        0
    );

    // Uma entrega real produz uma notificação legível por HTTP.
    let mut tx = pool.begin().await.expect("tx");
    let pendente = ocinye_core::modules::calendar::create_reminder(
        &mut tx,
        &quem,
        &CorrelationIds::generate(),
        ocinye_core::modules::calendar::NewReminder {
            event_id: None,
            task_id: None,
            note: Some("agora".to_owned()),
            trigger_at: chrono::Utc::now() - chrono::Duration::minutes(1),
        },
    )
    .await
    .expect("lembrete");
    ocinye_core::modules::calendar::delivery::deliver_in_app(&mut tx, &pendente)
        .await
        .expect("entrega");
    tx.commit().await.expect("commit");

    let (_, notificacoes) = pedir(&state, &token, "GET", "/api/v1/notifications", None).await;
    assert_eq!(
        notificacoes["unread"], 1,
        "a notificação entregue não conta"
    );
    let primeira = &notificacoes["notifications"][0];
    assert_eq!(primeira["read"], false);

    let notificacao_id = primeira["id"].as_str().expect("id").to_owned();
    let (status, _) = pedir(
        &state,
        &token,
        "POST",
        &format!("/api/v1/notifications/{notificacao_id}/read"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, depois) = pedir(&state, &token, "GET", "/api/v1/notifications", None).await;
    assert_eq!(
        depois["unread"], 0,
        "marcar como lida não baixou o contador"
    );
}
