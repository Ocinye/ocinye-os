//! Duas entradas, uma operação.
//!
//! # O que aqui se prova
//!
//! ADR-0307 afirma que uma acção do Workspace e a sua equivalente agentic
//! convergem na **mesma operação determinista do Core**. Essa afirmação tem uma
//! parte que o `ocinye-core` não pode verificar sozinho: a entrada do Workspace
//! não chama o Core directamente — submete um formulário que vira `POST` numa
//! rota HTTP deste crate, e só depois disso é que a operação corre.
//!
//! Por isso cada teste aqui conduz **as duas entradas a sério**: o router real,
//! com o middleware real e os extractores reais, e o executor de capabilities
//! real. Nenhuma das duas é imitada. Uma reconstrução do router seria livre de
//! divergir dele, e a divergência é exactamente o que se está a medir.
//!
//! # Como se observa a convergência
//!
//! Pelo rasto de auditoria. A entrada de auditoria é escrita **dentro** da
//! operação do Core, e não no handler HTTP nem na capability. Se as duas
//! entradas produzem auditorias indistinguíveis — mesma acção, mesmo tipo de
//! recurso, mesmo actor — então passaram as duas pelo mesmo código. Uma rota
//! com escrita paralela própria seria visível aqui, porque o par deixaria de
//! coincidir.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida; **falha** quando
//! está e a base não responde.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ocinye_contracts::agentic::{
    CapabilityId, CapabilityRequest, CapabilityResult, ExecutionStatus,
    ResourceKind as AgenticKind, ResourceRef,
};
use ocinye_contracts::avatar::AvatarChoice;
use ocinye_contracts::{SessionState, TechnicalRole};
use ocinye_core::authn::TokenVerifier;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::agentic::{self, registry::registry, runtime};
use ocinye_core::modules::identity::{self, Authenticator, Throttle};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::password::{Hasher, HashingParams, Secret};
use ocinye_core_server::routes;
use ocinye_core_server::state::AppState;
use ocinye_domain::{Principal, ResourceContext, ResourceKind};
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

// ── As duas entradas ────────────────────────────────────────────────────

/// The Workspace entry: a real `POST` on the real router.
async fn via_workspace(
    state: &AppState,
    token: &Secret,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token.expose()))
        .body(Body::from(body.to_string()))
        .expect("pedido");

    let response = routes::router(state.clone())
        .oneshot(request)
        .await
        .expect("resposta");

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("corpo");
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

    (status, value)
}

/// The agentic entry: the real executor, on the real registry.
async fn via_agent(
    pool: &PgPool,
    actor: &Principal,
    ctx: &ResourceContext,
    capability: &str,
    input: Value,
    resources: Vec<ResourceRef>,
) -> CapabilityResult {
    agentic::execute(
        pool,
        capacidades(),
        actor,
        &runtime::main_agent_boundary(),
        None,
        &CapabilityRequest {
            capability: CapabilityId::parse(capability)
                .unwrap_or_else(|| panic!("`{capability}` não é um identificador de capability")),
            input,
            resources,
            dry_run: false,
        },
        ctx,
        // Aprovado: o que aqui se mede é a convergência, não o portão de
        // aprovação — esse tem os seus próprios testes.
        true,
        &CorrelationIds::generate(),
    )
    .await
    .expect("o executor devolve um resultado")
}

// ── Observação ──────────────────────────────────────────────────────────

/// The audit trail of one resource, as the Core wrote it.
async fn trilho(pool: &PgPool, resource_id: Uuid) -> Vec<(String, String, Option<Uuid>)> {
    sqlx::query_as(
        "SELECT action, resource_type, actor_person_id FROM audit_events
          WHERE resource_id = $1 ORDER BY occurred_at",
    )
    .bind(resource_id)
    .fetch_all(pool)
    .await
    .expect("auditoria")
}

/// Assert that the two entries left the same institutional trace.
///
/// Not «both worked»: **the same operation ran**. A handler with a private
/// audit write, or a capability that reimplemented a domain rule instead of
/// calling it, breaks here and nowhere else.
async fn convergem(
    pool: &PgPool,
    operacao: &str,
    (ui, ui_actor): (Uuid, Uuid),
    (agente, agente_actor): (Uuid, Uuid),
) {
    let pela_ui = trilho(pool, ui).await;
    let pelo_agente = trilho(pool, agente).await;

    assert!(
        !pela_ui.is_empty(),
        "{operacao}: a entrada do Workspace não deixou auditoria nenhuma"
    );

    let acoes = |t: &[(String, String, Option<Uuid>)]| -> Vec<(String, String)> {
        t.iter().map(|(a, r, _)| (a.clone(), r.clone())).collect()
    };

    assert_eq!(
        acoes(&pela_ui),
        acoes(&pelo_agente),
        "{operacao}: as duas entradas escreveram auditorias diferentes, logo não \
         passaram pelo mesmo código"
    );

    assert_eq!(
        pela_ui.first().and_then(|(_, _, actor)| *actor),
        Some(ui_actor),
        "{operacao}: a auditoria da entrada do Workspace não regista quem submeteu"
    );
    assert_eq!(
        pelo_agente.first().and_then(|(_, _, actor)| *actor),
        Some(agente_actor),
        "{operacao}: a auditoria da entrada agentic não regista a pessoa em nome de \
         quem o agente agiu"
    );
}

/// The capability the registry says implements this operation.
///
/// Looked up by `OperationId`, never by name resemblance: the whole point is
/// that the pairing is declared in the type, not inferred from a string that
/// happens to read similarly.
fn capability_de(operacao: &str) -> String {
    registry()
        .all()
        .into_iter()
        .find(|d| d.operation.as_str() == operacao)
        .unwrap_or_else(|| panic!("nenhuma capability declara `{operacao}`"))
        .id
        .as_str()
        .to_owned()
}

fn identificador(valor: &Value, caminho: &[&str]) -> Uuid {
    let mut cursor = valor;
    for passo in caminho {
        cursor = cursor
            .get(passo)
            .unwrap_or_else(|| panic!("resposta sem `{passo}`: {valor}"));
    }
    cursor
        .as_str()
        .and_then(|raw| Uuid::parse_str(raw).ok())
        .unwrap_or_else(|| panic!("`{caminho:?}` não é um identificador: {valor}"))
}

fn criado(resultado: &CapabilityResult) -> Uuid {
    resultado
        .resources
        .first()
        .unwrap_or_else(|| panic!("a capability não nomeou o que criou: {}", resultado.detail))
        .id
}

// ── Os cinco pares ──────────────────────────────────────────────────────
//
// Cada teste cria a **mesma coisa duas vezes**, uma por cada entrada, e depois
// pergunta se o Core registou as duas da mesma maneira. As duas pessoas são
// distintas de propósito: se a auditoria coincidisse por acaso — por exemplo
// porque ninguém a escreve — o par actor/actor apanhava isso.

fn instituicao(organisation_id: Uuid) -> ResourceContext {
    ResourceContext::organisation(ResourceKind::Person, organisation_id)
}

/// Criar uma unidade científica.
#[tokio::test]
async fn organisation_create_unit_tem_as_duas_entradas() {
    let (pool, org, state) = institution!();
    let (_, token) = member(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let (agente, _) = member(&pool, org, &[TechnicalRole::PlatformAdmin]).await;

    let (status, corpo) = via_workspace(
        &state,
        &token,
        "/api/v1/units",
        json!({"code": format!("U{}", &Uuid::new_v4().simple().to_string()[..6]), "name": "Unidade pela interface"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a rota do Workspace recusou: {corpo}"
    );
    let pela_ui = identificador(&corpo, &["id"]);
    let ui_actor = quem_submeteu(&pool, &token).await;

    let resultado = via_agent(
        &pool,
        &agente,
        &instituicao(org),
        &capability_de("organisation::create_unit"),
        json!({"code": format!("A{}", &Uuid::new_v4().simple().to_string()[..6]), "name": "Unidade pelo agente"}),
        Vec::new(),
    )
    .await;
    assert_eq!(
        resultado.status,
        ExecutionStatus::Succeeded,
        "a capability recusou: {}",
        resultado.detail
    );

    convergem(
        &pool,
        "organisation::create_unit",
        (pela_ui, ui_actor),
        (criado(&resultado), agente.person_id),
    )
    .await;
}

/// Criar uma Ideia numa unidade.
#[tokio::test]
async fn research_create_idea_tem_as_duas_entradas() {
    let (pool, org, state) = institution!();
    let (unidade, admin) = unidade(&pool, org, &state).await;

    let (submissor, token) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let (agente, _) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_da_unidade(&pool, &admin, unidade, submissor.person_id).await;
    membro_da_unidade(&pool, &admin, unidade, agente.person_id).await;
    // Relido: a pertença acabou de mudar o que esta pessoa pode fazer, e um
    // principal em cache continuaria a dizer que não pode.
    let agente = principal(&pool, agente.person_id).await;

    let (status, corpo) = via_workspace(
        &state,
        &token,
        "/api/v1/ideas",
        json!({"unit_id": unidade, "title": "Ideia pela interface"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a rota do Workspace recusou: {corpo}"
    );
    let pela_ui = identificador(&corpo, &["idea", "id"]);

    let resultado = via_agent(
        &pool,
        &agente,
        &instituicao(org),
        &capability_de("research::create_idea"),
        json!({"title": "Ideia pelo agente"}),
        vec![ResourceRef {
            kind: AgenticKind::Unit,
            id: unidade,
            label: None,
        }],
    )
    .await;
    assert_eq!(
        resultado.status,
        ExecutionStatus::Succeeded,
        "a capability recusou: {}",
        resultado.detail
    );

    convergem(
        &pool,
        "research::create_idea",
        (pela_ui, submissor.person_id),
        (criado(&resultado), agente.person_id),
    )
    .await;
}

/// Criar uma tarefa num Research Workspace.
#[tokio::test]
async fn collaboration_create_task_tem_as_duas_entradas() {
    let (pool, org, state) = institution!();
    let (unidade, admin) = unidade(&pool, org, &state).await;

    let (submissor, token) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_da_unidade(&pool, &admin, unidade, submissor.person_id).await;
    let submissor = principal(&pool, submissor.person_id).await;
    let workspace = workspace(&pool, unidade, &submissor).await;
    // Criar o workspace tornou esta pessoa membro dele, e é essa pertença que
    // dá `tasks.create`. A entrada HTTP relê o principal a cada pedido; a
    // entrada agentic recebe o que lhe derem, por isso é aqui que se relê.
    let submissor = principal(&pool, submissor.person_id).await;

    let (status, corpo) = via_workspace(
        &state,
        &token,
        &format!("/api/v1/workspaces/{workspace}/tasks"),
        json!({"title": "Tarefa pela interface"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a rota do Workspace recusou: {corpo}"
    );
    let pela_ui = identificador(&corpo, &["id"]);

    let resultado = via_agent(
        &pool,
        &submissor,
        &instituicao(org),
        &capability_de("collaboration::create_task"),
        json!({"title": "Tarefa pelo agente"}),
        vec![ResourceRef {
            kind: AgenticKind::Workspace,
            id: workspace,
            label: None,
        }],
    )
    .await;
    assert_eq!(
        resultado.status,
        ExecutionStatus::Succeeded,
        "a capability recusou: {}",
        resultado.detail
    );

    convergem(
        &pool,
        "collaboration::create_task",
        (pela_ui, submissor.person_id),
        (criado(&resultado), submissor.person_id),
    )
    .await;
}

/// Criar um dataset — metadados; os ficheiros não são delegáveis.
#[tokio::test]
async fn data_create_dataset_tem_as_duas_entradas() {
    let (pool, org, state) = institution!();
    let (unidade, admin) = unidade(&pool, org, &state).await;

    let (submissor, token) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    membro_da_unidade(&pool, &admin, unidade, submissor.person_id).await;
    let submissor = principal(&pool, submissor.person_id).await;
    let workspace = workspace(&pool, unidade, &submissor).await;
    // Criar o workspace tornou esta pessoa membro dele, e é essa pertença que
    // dá `tasks.create`. A entrada HTTP relê o principal a cada pedido; a
    // entrada agentic recebe o que lhe derem, por isso é aqui que se relê.
    let submissor = principal(&pool, submissor.person_id).await;

    let (status, corpo) = via_workspace(
        &state,
        &token,
        &format!("/api/v1/workspaces/{workspace}/datasets"),
        json!({"code": "DS-UI", "title": "Dataset pela interface"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a rota do Workspace recusou: {corpo}"
    );
    let pela_ui = identificador(&corpo, &["id"]);

    let resultado = via_agent(
        &pool,
        &submissor,
        &instituicao(org),
        &capability_de("data::create_dataset"),
        json!({"code": "DS-AG", "title": "Dataset pelo agente"}),
        vec![ResourceRef {
            kind: AgenticKind::Workspace,
            id: workspace,
            label: None,
        }],
    )
    .await;
    assert_eq!(
        resultado.status,
        ExecutionStatus::Succeeded,
        "a capability recusou: {}",
        resultado.detail
    );

    convergem(
        &pool,
        "data::create_dataset",
        (pela_ui, submissor.person_id),
        (criado(&resultado), submissor.person_id),
    )
    .await;
}

/// Marcar um compromisso.
///
/// # A prova que fecha o Calendário
///
/// A interface e o agente marcam o mesmo tipo de coisa, cada um pelo seu
/// caminho, e o rasto de auditoria que deixam tem de ser indistinguível — porque
/// a auditoria é escrita **dentro** da operação do Core. Se divergisse, uma das
/// entradas teria ganho lógica própria.
#[tokio::test]
async fn calendar_create_event_tem_as_duas_entradas() {
    let (pool, org, state) = institution!();
    let (submissor, token) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let amanha = (chrono::Utc::now() + chrono::Duration::days(1)).date_naive();
    let (status, corpo) = via_workspace(
        &state,
        &token,
        "/api/v1/calendar/events",
        json!({
            "scope": "personal",
            "title": "Pela interface",
            "occurrence": {
                "kind": "timed",
                "starts_at": amanha.and_hms_opt(9, 0, 0).expect("hora"),
                "ends_at": amanha.and_hms_opt(10, 0, 0).expect("hora"),
                "timezone": "Europe/Lisbon"
            }
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a rota do Workspace recusou: {corpo}"
    );
    let pela_ui = identificador(&corpo, &["id"]);

    let resultado = via_agent(
        &pool,
        &submissor,
        &instituicao(org),
        &capability_de("calendar::create_event"),
        json!({
            "title": "Pelo agente",
            "starts_at": format!("{amanha}T14:00"),
            "ends_at": format!("{amanha}T15:00"),
            "timezone": "Europe/Lisbon"
        }),
        Vec::new(),
    )
    .await;
    assert_eq!(
        resultado.status,
        ExecutionStatus::Succeeded,
        "a capability recusou: {}",
        resultado.detail
    );

    convergem(
        &pool,
        "calendar::create_event",
        (pela_ui, submissor.person_id),
        (criado(&resultado), submissor.person_id),
    )
    .await;
}

/// Self-service: escolher um avatar do catálogo Ocinye.
///
/// O par aqui não se observa por identificador de recurso criado — nada é
/// criado —, mas pelo estado que fica na pessoa. É a mesma pergunta: as duas
/// entradas deixam a instituição no mesmo sítio?
#[tokio::test]
async fn identity_choose_preset_tem_as_duas_entradas() {
    let (pool, org, state) = institution!();
    let (pela_interface, token) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let (pelo_agente, _) = member(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let preset = ocinye_contracts::avatar::AVATAR_PRESETS[1].0;

    let (status, corpo) = via_workspace(
        &state,
        &token,
        "/api/v1/me/avatar/preset",
        json!({"preset": preset}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a rota do Workspace recusou: {corpo}"
    );

    let resultado = via_agent(
        &pool,
        &pelo_agente,
        &instituicao(org),
        &capability_de("identity::choose_preset"),
        json!({"preset": preset}),
        Vec::new(),
    )
    .await;
    assert_eq!(
        resultado.status,
        ExecutionStatus::Succeeded,
        "a capability recusou: {}",
        resultado.detail
    );

    let pela_ui = avatar(&pool, pela_interface.person_id).await;
    let pelo_agente = avatar(&pool, pelo_agente.person_id).await;

    // Primeiro que *alguma coisa* mudou, e só depois que mudou igual.
    //
    // A comparação sozinha satisfazia-se com as duas entradas a não fazerem
    // nada: `Initials == Initials` passa, e passaria para sempre. Já me
    // aconteceu escrever um teste assim e chamar-lhe verde.
    assert_eq!(
        pela_ui,
        AvatarChoice::Preset {
            preset: preset.to_owned()
        },
        "identity::choose_preset: a entrada do Workspace não deixou o avatar escolhido"
    );
    assert_eq!(
        pela_ui, pelo_agente,
        "identity::choose_preset: as duas entradas deixaram avatares diferentes"
    );
}

// ── Apoio ───────────────────────────────────────────────────────────────

/// A unit, created through the route a member would use.
async fn unidade(pool: &PgPool, org: Uuid, state: &AppState) -> (Uuid, Principal) {
    let (admin, token) = member(pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let (status, corpo) = via_workspace(
        state,
        &token,
        "/api/v1/units",
        json!({
            "code": format!("F{}", &Uuid::new_v4().simple().to_string()[..6]),
            "name": "Unidade de apoio"
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a fixture não conseguiu criar a unidade: {corpo}"
    );
    (identificador(&corpo, &["id"]), admin)
}

/// Membership, through the operation that grants it.
///
/// Not an `INSERT`. Membership is authorisation-relevant state — belonging to a
/// unit is what makes `ideas.create` reachable — and a fixture that wrote the
/// row directly would be asserting its own idea of what membership means
/// instead of the institution's.
///
/// `Manager`, not `Member`: the permission table lists `IdeasCreate` for both,
/// but `may_write_in_context` is a separate and narrower gate that requires
/// management of the unit, and it is the one that decides.
async fn membro_da_unidade(pool: &PgPool, admin: &Principal, unit_id: Uuid, person_id: Uuid) {
    let mut tx = pool.begin().await.expect("transacção");
    ocinye_core::modules::organisation::add_unit_member(
        &mut tx,
        admin,
        &CorrelationIds::generate(),
        unit_id,
        person_id,
        ocinye_contracts::UnitRole::Manager,
    )
    .await
    .expect("membro da unidade");
    tx.commit().await.expect("commit");
}

/// A research workspace, born of an idea, as one always is.
async fn workspace(pool: &PgPool, unit_id: Uuid, actor: &Principal) -> Uuid {
    let mut principal = actor.clone();
    let mut tx = pool.begin().await.expect("transacção");
    let (_, workspace) = ocinye_core::modules::research::create_idea(
        &mut tx,
        &mut principal,
        &CorrelationIds::generate(),
        ocinye_core::modules::research::NewIdea {
            unit_id,
            title: "Ideia de apoio".to_owned(),
            summary: None,
            research_question: None,
            hypothesis: None,
            motivation: None,
            keywords: Vec::new(),
            classification: None,
        },
    )
    .await
    .expect("ideia de apoio");
    tx.commit().await.expect("commit");
    workspace.id
}

/// What avatar a person is left with.
async fn avatar(pool: &PgPool, person_id: Uuid) -> AvatarChoice {
    identity::own_avatar(pool, &principal(pool, person_id).await)
        .await
        .expect("avatar")
}

/// Whose session this token is.
async fn quem_submeteu(pool: &PgPool, token: &Secret) -> Uuid {
    identity::find_session(pool, token)
        .await
        .expect("consulta")
        .expect("sessão viva")
        .person_id
}

/// O Capability Runtime, com os componentes desta árvore.
fn capacidades() -> &'static ocinye_core::capabilities::Capabilities {
    use std::sync::OnceLock;
    static UM: OnceLock<ocinye_core::capabilities::Capabilities> = OnceLock::new();
    UM.get_or_init(|| {
        ocinye_core::capabilities::Capabilities::load(&format!(
            "{}/../../target/wasm32-wasip1/release",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("motor de capacidades")
    })
}
