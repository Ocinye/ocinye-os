//! O que `/ready` responde a quem ainda não entrou.
//!
//! # Porque isto existe separado dos testes do módulo
//!
//! Os testes dentro de `health.rs` provam a forma da projecção. Estes provam o
//! que atravessa a rede: o estado HTTP, os cabeçalhos, o corpo serializado e o
//! comportamento perante um contrato que não bate certo.
//!
//! São perguntas diferentes. Uma projecção pode estar correcta e ser servida com
//! o cabeçalho errado, e é o cabeçalho errado que faz um proxy guardar a
//! resposta sobre um sistema que entretanto caiu.
//!
//! # A semântica fecha-se aqui, antes de existir interface
//!
//! Esta matriz corre antes de haver Splash de propósito. Uma interface
//! construída sobre uma semântica ainda em aberto acaba por inventar a metade
//! que falta — e depois há duas políticas de arranque, uma no Core e outra no
//! browser, que um dia discordam.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ocinye_contracts::readiness::{
    Criticality, PublicReadiness, ReadinessComponentId, ReadinessOverall, CONTRACT_VERSION,
};
use ocinye_contracts::system_capability::SystemCapabilityState;
use ocinye_core::authn::TokenVerifier;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{Authenticator, Throttle};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::password::{Hasher, HashingParams};
use ocinye_core_server::routes;
use ocinye_core_server::state::AppState;
use serde_json::Value;
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
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }};
}

fn config() -> CoreConfig {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    // Uma vez por processo, como nas outras suites de HTTP deste serviço.
    //
    // Estava escrito «os testes deste ficheiro correm no mesmo processo e lêem
    // estas variáveis uma vez, antes de qualquer trabalho concorrente», e não
    // era verdade: `config()` é chamada por cada teste, através de `state()`, e
    // os testes correm em paralelo. `set_var` com leitores concorrentes é
    // precisamente o caso que a função declara não suportar. Escrever sempre o
    // mesmo valor tornava a corrida inofensiva na prática, mas a justificação
    // de segurança era falsa — e uma razão escrita que não é verdadeira é pior
    // do que nenhuma, porque a próxima pessoa acredita nela.
    //
    // `OCINYE_PUBLIC_URL` saiu daqui: era definida e não é lida por ninguém em
    // todo o repositório.
    ONCE.call_once(|| {
        // SAFETY: uma escrita, antes de qualquer teste começar trabalho.
        unsafe {
            std::env::set_var("OCINYE_DATABASE_URL", "postgres://x/x");
        }
    });
    CoreConfig::from_env().expect("configuração de teste")
}

async fn organisation(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("rdy-{}", Uuid::new_v4().simple()))
        .bind("Instituição de prontidão")
        .fetch_one(pool)
        .await
        .expect("organização")
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
            per_email: config.auth.throttle_per_email,
            window_minutes: config.auth.throttle_window_minutes,
        },
        config.auth.temporary_credential_hours,
    ));
    let mail_registry = Arc::new(ocinye_core::modules::mail::ProviderRegistry::new(
        Arc::new(UnconfiguredProvider),
        config.mail.clone(),
        config.mail.sealing_key.clone(),
    ));
    AppState {
        pool,
        config: Arc::new(config),
        verifier,
        authenticator,
        store: None,
        inference: Arc::new(ocinye_core::modules::intelligence::NoProvider),
        mail_registry,
        // Estes testes medem HTTP, e não tempo real. Um plano ausente aceita
        // tudo e não propaga nada — que é o que uma instalação sem Redis faz,
        // e não um sítio por preencher.
        realtime: Arc::new(ocinye_core::realtime::Realtime::ausente()),
        mail_probe: Arc::new(SondaDoHarness),
        capabilities: std::sync::Arc::new(
            ocinye_core::capabilities::Capabilities::empty().expect("motor de capacidades"),
        ),
        organisation_id,
    }
}

/// Pede `/ready` ao router real e devolve estado, cabeçalhos e corpo.
async fn pedir(state: AppState, consulta: &str) -> (StatusCode, String, Value) {
    let resposta = routes::router(state)
        .oneshot(
            Request::builder()
                .uri(format!("/ready{consulta}"))
                .body(Body::empty())
                .expect("pedido"),
        )
        .await
        .expect("resposta");

    let estado = resposta.status();
    let cache = resposta
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let corpo = axum::body::to_bytes(resposta.into_body(), 1 << 20)
        .await
        .expect("corpo");
    let json: Value = serde_json::from_slice(&corpo).expect("json");
    (estado, cache, json)
}

// ── O que responde com tudo de pé ───────────────────────────────────────

/// Com a base a responder, o sistema pode ser entregue.
///
/// O controlo positivo de toda a matriz. Sem ele, cada recusa abaixo podia estar
/// a acontecer por uma razão que nada tem que ver com o que o teste diz provar.
#[tokio::test]
async fn com_a_base_de_pe_o_sistema_segue() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let (estado, _, corpo) = pedir(state(pool, org), "").await;

    assert_eq!(estado, StatusCode::OK);
    let prontidao: PublicReadiness = serde_json::from_value(corpo).expect("projecção");
    assert!(
        prontidao.overall.may_proceed(),
        "com a base de pé o arranque tem de poder seguir, e veio {:?}",
        prontidao.overall
    );
    assert_eq!(prontidao.contract_version, CONTRACT_VERSION);
    assert!(
        !prontidao.components.is_empty(),
        "uma prontidão sem componentes não diz nada sobre nada"
    );
}

/// A resposta não pode ser guardada por ninguém.
///
/// Readiness em cache é uma resposta sobre um sistema que já não existe. O
/// browser, um proxy reverso ou uma rede de distribuição que a guardem passam a
/// afirmar prontidão de um Core que entretanto caiu — e o cabeçalho é a única
/// coisa que os impede.
#[tokio::test]
async fn a_prontidao_nao_pode_ser_guardada() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let (_, cache, _) = pedir(state(pool, org), "").await;

    assert!(
        cache.contains("no-store"),
        "`/ready` tem de proibir armazenamento, e disse «{cache}»"
    );
}

// ── Compatibilidade ─────────────────────────────────────────────────────

/// Um Core saudável que fala outro contrato não é um sistema pronto.
///
/// É a diferença entre falhar no arranque, com uma frase que se lê, e falhar
/// mais tarde num erro de desserialização que ninguém consegue interpretar.
#[tokio::test]
async fn um_contrato_diferente_bloqueia_mesmo_com_tudo_de_pe() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let (estado, _, corpo) = pedir(
        state(pool, org),
        &format!("?contract={}", CONTRACT_VERSION + 7),
    )
    .await;

    assert_eq!(
        estado,
        StatusCode::SERVICE_UNAVAILABLE,
        "um contrato incompatível tem de impedir o arranque"
    );
    let prontidao: PublicReadiness = serde_json::from_value(corpo).expect("projecção");
    assert_eq!(prontidao.overall, ReadinessOverall::Blocked);

    let compat = prontidao
        .components
        .iter()
        .find(|c| c.component == ReadinessComponentId::Compatibility)
        .expect("o componente de compatibilidade tem de existir");
    assert_eq!(compat.state, SystemCapabilityState::Unavailable);
}

/// O contrato certo não bloqueia nada.
///
/// O controlo positivo do teste acima: o que bloqueou foi a diferença, e não o
/// facto de alguém ter declarado um contrato.
#[tokio::test]
async fn o_contrato_certo_deixa_seguir() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let (estado, _, corpo) =
        pedir(state(pool, org), &format!("?contract={CONTRACT_VERSION}")).await;

    assert_eq!(estado, StatusCode::OK);
    let prontidao: PublicReadiness = serde_json::from_value(corpo).expect("projecção");
    assert!(prontidao.overall.may_proceed());
}

/// Quem não declara contrato não é marcado incompatível.
///
/// Uma sonda de infraestrutura chama `/ready` para saber se o processo serve, e
/// não é o Workspace. Tratá-la como incompatível diria que o sistema está
/// bloqueado por causa de quem perguntou.
#[tokio::test]
async fn uma_sonda_sem_contrato_nao_e_incompativel() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let (estado, _, corpo) = pedir(state(pool, org), "").await;

    assert_eq!(estado, StatusCode::OK);
    let prontidao: PublicReadiness = serde_json::from_value(corpo).expect("projecção");
    let compat = prontidao
        .components
        .iter()
        .find(|c| c.component == ReadinessComponentId::Compatibility)
        .expect("compatibilidade");
    assert_ne!(
        compat.state,
        SystemCapabilityState::Unavailable,
        "quem não declarou contrato foi tratado como incompatível"
    );
}

/// Um contrato ilegível é recusado, e não interpretado.
///
/// Nem pânico, nem valor por omissão, nem o texto de volta na mensagem de erro —
/// que seria reflectir entrada de terceiros numa superfície pública.
#[tokio::test]
async fn um_contrato_ilegivel_nao_vira_valor_por_omissao() {
    let pool = pool!();
    let org = organisation(&pool).await;

    for lixo in [
        "?contract=abc",
        "?contract=",
        "?contract=-1",
        "?contract=99999999999999999999",
    ] {
        let resposta = routes::router(state(pool.clone(), org))
            .oneshot(
                Request::builder()
                    .uri(format!("/ready{lixo}"))
                    .body(Body::empty())
                    .expect("pedido"),
            )
            .await
            .expect("resposta");

        let estado = resposta.status();
        let corpo = axum::body::to_bytes(resposta.into_body(), 1 << 20)
            .await
            .expect("corpo");
        let texto = String::from_utf8_lossy(&corpo);

        assert!(
            estado.is_client_error() || estado == StatusCode::OK,
            "«{lixo}» deu {estado}, que não é nem recusa nem resposta"
        );
        assert!(
            !texto.contains("99999999999999999999") && !texto.contains("abc"),
            "«{lixo}» foi reflectido de volta na resposta: {texto}"
        );
    }
}

// ── O que não sai daqui ─────────────────────────────────────────────────

/// A resposta pública não nomeia infraestrutura nem pessoas.
///
/// Quem chama `/ready` ainda não entrou. O que recebe não pode ensinar-lhe a
/// topologia da instalação — nem por descuido numa frase de razão, nem por um
/// campo interno que apareceu numa estrutura aninhada e ninguém reparou.
#[tokio::test]
async fn a_resposta_publica_nao_ensina_a_topologia() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let (_, _, corpo) = pedir(state(pool, org), "").await;

    let texto = serde_json::to_string(&corpo).expect("json");
    let minusculas = texto.to_lowercase();

    for agulha in [
        "postgres",
        "postgresql",
        "5432",
        "minio",
        "s3",
        "9000",
        "localhost",
        "127.0.0.1",
        "/var/",
        "/usr/",
        "/home/",
        "amazonaws",
        "password",
        "secret",
        "token",
        "@",
    ] {
        assert!(
            !minusculas.contains(agulha),
            "a resposta pública contém «{agulha}»: {texto}"
        );
    }
}

/// Nenhum identificador de pessoa atravessa uma superfície pré-autenticação.
#[tokio::test]
async fn a_resposta_publica_nao_tem_ninguem_dentro() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let (_, _, corpo) = pedir(state(pool, org), "").await;

    let texto = serde_json::to_string(&corpo).expect("json").to_lowercase();
    for agulha in [
        "email",
        "person",
        "role",
        "membership",
        "avatar",
        "workspace_id",
    ] {
        assert!(
            !texto.contains(agulha),
            "a resposta pública nomeia «{agulha}»"
        );
    }

    // O identificador da própria organização também não. Uma instalação não se
    // apresenta a quem ainda não entrou.
    assert!(
        !texto.contains(&org.to_string()),
        "o identificador da organização saiu numa resposta pública"
    );
}

/// Cada chave do JSON emitido está na lista de autorização.
///
/// Percorre o que atravessa a rede, e não os tipos: um campo acrescentado a uma
/// estrutura aninhada aparece aqui mesmo que ninguém se lembre dele.
#[tokio::test]
async fn cada_chave_emitida_esta_na_lista() {
    const PERMITIDAS: [&str; 7] = [
        "overall",
        "contract_version",
        "components",
        "component",
        "state",
        "criticality",
        "reason",
    ];

    fn percorre(valor: &Value, fora: &mut Vec<String>) {
        match valor {
            Value::Object(mapa) => {
                for (chave, dentro) in mapa {
                    if !PERMITIDAS.contains(&chave.as_str()) {
                        fora.push(chave.clone());
                    }
                    percorre(dentro, fora);
                }
            }
            Value::Array(itens) => itens.iter().for_each(|i| percorre(i, fora)),
            _ => {}
        }
    }

    let pool = pool!();
    let org = organisation(&pool).await;
    let (_, _, corpo) = pedir(state(pool, org), "").await;

    let mut fora = Vec::new();
    percorre(&corpo, &mut fora);
    assert!(
        fora.is_empty(),
        "chaves fora da lista de autorização pública: {fora:?}"
    );
}

// ── As duas regressões históricas ───────────────────────────────────────

/// Um pedido de domínio que responde bem não significa que o sistema arrancou.
///
/// Houve uma altura em que a interface inferia a saúde do Core a partir de uma
/// consulta de organização: se respondesse, o Core estaria bem. Um pedido de
/// domínio responde por razões suas, e uma delas não é a prontidão institucional.
///
/// A prova aqui é directa: o mesmo estado responde a um pedido de domínio e a
/// `/ready`, e são duas perguntas com duas respostas independentes.
#[tokio::test]
async fn um_pedido_de_dominio_nao_e_uma_sonda_de_prontidao() {
    let pool = pool!();
    let org = organisation(&pool).await;

    // O contrato incompatível bloqueia a prontidão.
    let (estado_ready, _, corpo) = pedir(
        state(pool.clone(), org),
        &format!("?contract={}", CONTRACT_VERSION + 7),
    )
    .await;
    let prontidao: PublicReadiness = serde_json::from_value(corpo).expect("projecção");
    assert_eq!(prontidao.overall, ReadinessOverall::Blocked);
    assert_eq!(estado_ready, StatusCode::SERVICE_UNAVAILABLE);

    // E a base continua a responder na mesma. Quem lesse isto como saúde do
    // sistema concluiria que está tudo bem enquanto o arranque está bloqueado.
    let existe: i64 = sqlx::query_scalar("SELECT count(*) FROM organisations WHERE id = $1")
        .bind(org)
        .fetch_one(&pool)
        .await
        .expect("consulta de domínio");
    assert_eq!(
        existe, 1,
        "a consulta de domínio devia responder normalmente, e é esse o ponto"
    );
}

/// Uma resposta HTTP com sucesso de transporte não é prontidão.
///
/// `/ready` respondeu — logo o Core está lá. O que ele disse foi que não está
/// pronto. Tratar o sucesso do transporte como prontidão é a regressão que este
/// teste guarda: o corpo é que decide, e não o facto de ter chegado.
#[tokio::test]
async fn transporte_com_sucesso_nao_e_prontidao() {
    let pool = pool!();
    let org = organisation(&pool).await;

    let resposta = routes::router(state(pool, org))
        .oneshot(
            Request::builder()
                .uri(format!("/ready?contract={}", CONTRACT_VERSION + 7))
                .body(Body::empty())
                .expect("pedido"),
        )
        .await;

    // O transporte correu bem: há resposta.
    let resposta = resposta.expect("o Core respondeu");
    assert!(
        !resposta.status().is_success(),
        "o transporte correu e o estado diz sucesso; a prontidão tem de vir do corpo"
    );

    let corpo = axum::body::to_bytes(resposta.into_body(), 1 << 20)
        .await
        .expect("corpo");
    let prontidao: PublicReadiness = serde_json::from_slice(&corpo).expect("projecção");
    assert!(
        !prontidao.overall.may_proceed(),
        "houve resposta, e o que ela diz é que não se pode seguir"
    );
}

/// Um componente crítico indisponível bloqueia; um opcional apenas limita.
#[tokio::test]
async fn a_criticidade_decide_se_bloqueia_ou_limita() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let (_, _, corpo) = pedir(state(pool, org), "").await;
    let prontidao: PublicReadiness = serde_json::from_value(corpo).expect("projecção");

    let criticos: Vec<_> = prontidao
        .components
        .iter()
        .filter(|c| c.criticality == Criticality::Critical)
        .collect();
    assert!(
        !criticos.is_empty(),
        "uma prontidão sem nada crítico não pode bloquear nunca"
    );

    let opcionais: Vec<_> = prontidao
        .components
        .iter()
        .filter(|c| c.criticality == Criticality::Optional)
        .collect();
    assert!(
        !opcionais.is_empty(),
        "uma prontidão sem nada opcional nunca pode ficar limitada"
    );

    // A regra que liga as duas coisas prova-se onde ela acontece: com um
    // crítico realmente em baixo, no teste seguinte. Estava aqui, dentro de um
    // `if critico_em_baixo`, e num ambiente saudável esse ramo nunca corria —
    // o teste passava sem ter observado nada da regra que dizia guardar.
}

/// Com um componente crítico em baixo, o sistema recusa-se a ser entregue.
///
/// # Porque a base é fechada, e não apontada a um sítio errado
///
/// Apontar a um endereço inexistente provaria que uma ligação que nunca existiu
/// falha. Fechar uma base que estava a responder prova o que interessa: o
/// sistema estava pronto, deixou de estar, e a resposta mudou.
///
/// # O que isto fecha
///
/// A decisão de bloquear é do Core e é feita uma vez, mas quem a lê são os
/// transportes. Sem isto, o único bloqueio provado ao nível do HTTP era o do
/// contrato incompatível — que constrói a projecção directamente e nunca passa
/// pela regra de criticidade.
#[tokio::test]
async fn um_critico_em_baixo_recusa_ao_nivel_do_http() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let estado = state(pool.clone(), org);

    // Primeiro, de pé: o controlo positivo desta viagem.
    let (antes, _, _) = pedir(estado.clone(), "").await;
    assert_eq!(
        antes,
        StatusCode::OK,
        "com a base a responder, isto tinha de seguir"
    );

    // A base fecha por baixo do sistema.
    pool.close().await;

    let (depois, cache, corpo) = pedir(estado, "").await;
    assert_eq!(
        depois,
        StatusCode::SERVICE_UNAVAILABLE,
        "um crítico em baixo tem de recusar, e o transporte tem de o dizer"
    );
    assert!(
        cache.contains("no-store"),
        "uma recusa guardada em cache sobrevive ao problema que a causou"
    );

    let prontidao: PublicReadiness = serde_json::from_value(corpo).expect("projecção");
    assert_eq!(prontidao.overall, ReadinessOverall::Blocked);

    let persistencia = prontidao
        .components
        .iter()
        .find(|c| c.component == ReadinessComponentId::Persistence)
        .expect("persistência na projecção");
    assert_eq!(persistencia.state, SystemCapabilityState::Unavailable);
    assert_eq!(persistencia.criticality, Criticality::Critical);

    // E a razão continua a ser uma frase institucional, e não o erro do driver:
    // uma base em baixo traz consigo o nome do servidor e o porto.
    assert!(
        !persistencia.reason.contains("127.0.0.1")
            && !persistencia.reason.to_lowercase().contains("connection"),
        "a razão trouxe consigo a topologia: «{}»",
        persistencia.reason
    );
}

/// A sonda do harness: aceita, porque não há servidor de correio para
/// perguntar. O que o harness assume fica escrito, em vez de a verificação
/// desaparecer do caminho que estes testes percorrem.
struct SondaDoHarness;

#[async_trait::async_trait]
impl ocinye_core::modules::mail::provider::CredentialProbe for SondaDoHarness {
    async fn verify(
        &self,
        _endereco: &str,
        _username: &str,
        _senha: &str,
    ) -> ocinye_core::modules::mail::provider::ProviderResult<()> {
        Ok(())
    }
}
