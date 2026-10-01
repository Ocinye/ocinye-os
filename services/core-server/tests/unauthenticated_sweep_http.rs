//! Nenhuma rota do Core responde a quem não tem sessão (Parte 12 da
//! generalização, fronteiras de confiança).
//!
//! O inventário sai do código das rotas — não de uma lista mantida à mão —, e
//! cada rota é pedida, com o seu método real e identificadores que não existem,
//! sem credencial nenhuma. A resposta tem de ser `401`. As excepções são poucas,
//! têm nome e razão, e as que se autenticam por outra credencial (um nó, o
//! próprio início de sessão) têm de recusar à mesma.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use ocinye_core::authn::TokenVerifier;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::{Authenticator, Throttle};
use ocinye_core::modules::mail::provider::UnconfiguredProvider;
use ocinye_core::password::{Hasher, HashingParams, Secret};
use ocinye_core_server::routes;
use ocinye_core_server::state::AppState;
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
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations");
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

/// O Core desta instalação: sem armazenamento, sem embeddings, e — o que
/// importa aqui — com o fornecedor de inferência por omissão, `NoProvider`. Zero
/// modelos, zero nós. É o estado que M5.1 tem de servir sem falhar.
fn nucleo(pool: PgPool, organisation_id: Uuid) -> AppState {
    nucleo_com(
        pool,
        organisation_id,
        Arc::new(ocinye_core::modules::intelligence::NoProvider),
    )
}

/// O mesmo, com um fornecedor de inferência à escolha — para provar que o
/// caminho de execução roteia quando um fornecedor serve (M5.2, e a base do
/// hot-plug de M5.3).
fn nucleo_com(
    pool: PgPool,
    organisation_id: Uuid,
    inference: Arc<dyn ocinye_core::modules::intelligence::InferenceProvider>,
) -> AppState {
    let mut config = config();
    // A raiz de selagem desta prova: gerada aqui, nunca uma chave real.
    config.sealing_key = Some(
        ocinye_core::password::sealed::SealingKey::from_base64(
            &ocinye_core::password::sealed::SealingKey::generate(),
        )
        .expect("raiz de teste"),
    );
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
        inference,
        mail_registry,
        realtime: Arc::new(ocinye_core::realtime::Realtime::ausente()),
        mail_probe: Arc::new(SondaDoHarness),
        capabilities: Arc::new(
            ocinye_core::capabilities::Capabilities::empty().expect("motor de capacidades"),
        ),
        organisation_id,
    }
}

async fn organisation(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("swp-{}", Uuid::new_v4().simple()))
        .bind("Instituição da varredura")
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn pedido(
    state: &AppState,
    token: Option<&Secret>,
    no: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.expose()));
    }
    if let Some(no) = no {
        builder = builder.header("x-ocinye-node-token", no);
    }
    let request = match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).expect("json"))),
        None => builder.body(Body::empty()),
    }
    .expect("pedido");
    let response = routes::router(state.clone())
        .oneshot(request)
        .await
        .expect("resposta");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("corpo");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// As rotas que respondem sem sessão, cada uma com a razão. Tudo o resto exige
/// uma — e é isso que o teste abaixo prova, rota a rota.
const PUBLICAS: &[(&str, &str, &str)] = &[
    (
        "GET",
        "/health",
        "liveness: não diz nada sobre a instituição",
    ),
    (
        "GET",
        "/ready",
        "readiness: estado de componentes, sem dados institucionais",
    ),
    (
        "GET",
        "/instance/branding",
        "a página de entrada mostra o nome da Instância antes de haver sessão (ADR-0017)",
    ),
    (
        "GET",
        "/instance/logo",
        "o logótipo da porta de entrada; mudá-lo exige administração",
    ),
    (
        "GET",
        "/access/resolve",
        "D010 · o Workspace resolve o anfitrião antes de haver sessão: devolve só o \
         destino e o nome da Instância que a entrada mostra; desconhecido é 404 (ADR-0020)",
    ),
];

/// As rotas que se autenticam por outra coisa que não uma sessão de membro.
/// Sem essa credencial, recusam — e recusam sem ser por `401` de sessão.
const OUTRA_CREDENCIAL: &[(&str, &str)] = &[
    (
        "/auth/login",
        "é onde a sessão nasce: recusa credenciais inválidas",
    ),
    (
        "/compute/enroll",
        "o nó autentica-se com o token de enrolamento",
    ),
    (
        "/compute/heartbeat",
        "o nó autentica-se com a sua credencial de máquina",
    ),
    (
        "/invitations/accept",
        "um convite aceita-se com o seu token, antes de haver sessão",
    ),
];

/// Cada `.route("…", método(…))` dos ficheiros de rotas do Core, com os métodos.
fn inventario() -> Vec<(String, Vec<&'static str>)> {
    let pasta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/routes");
    let mut rotas = Vec::new();
    for entrada in std::fs::read_dir(&pasta).expect("rotas do Core") {
        let codigo = std::fs::read_to_string(entrada.expect("ficheiro").path()).expect("ler");
        for argumento in argumentos_de_route(&codigo) {
            let Some(resto) = argumento.trim_start().strip_prefix('"') else {
                continue;
            };
            let Some((rota, metodos_texto)) = resto.split_once('"') else {
                continue;
            };
            let mut metodos = Vec::new();
            for (marca, metodo) in [
                ("get(", "GET"),
                ("post(", "POST"),
                ("put(", "PUT"),
                ("patch(", "PATCH"),
                ("delete(", "DELETE"),
            ] {
                // A palavra inteira — `budget(` não é um `get(` —, como o
                // `repository-facts.sh` a conta: os dois inventários são o mesmo.
                let inteira = metodos_texto.match_indices(marca).any(|(i, _)| {
                    !metodos_texto[..i]
                        .chars()
                        .next_back()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_')
                });
                if inteira {
                    metodos.push(metodo);
                }
            }
            if !metodos.is_empty() {
                rotas.push((rota.to_owned(), metodos));
            }
        }
    }
    rotas
}

/// O argumento exacto de cada `.route(…)`, com os parênteses equilibrados e sem
/// contar os que estão dentro de texto — o mesmo recorte do
/// `scripts/repository-facts.sh`.
fn argumentos_de_route(fonte: &str) -> Vec<String> {
    let marca = ".route(";
    let bytes = fonte.as_bytes();
    let mut argumentos = Vec::new();
    let mut inicio = 0;
    while let Some(posicao) = fonte[inicio..].find(marca) {
        let comeco = inicio + posicao + marca.len();
        let (mut j, mut nivel, mut texto, mut escapado) = (comeco, 1_i32, false, false);
        while j < bytes.len() && nivel > 0 {
            let c = bytes[j];
            if escapado {
                escapado = false;
            } else if c == b'\\' {
                escapado = true;
            } else if c == b'"' {
                texto = !texto;
            } else if !texto {
                nivel += i32::from(c == b'(') - i32::from(c == b')');
            }
            j += 1;
        }
        argumentos.push(fonte[comeco..j.saturating_sub(1)].to_owned());
        inicio = j;
    }
    argumentos
}

/// Um caminho concreto: cada parâmetro vira um identificador que não existe.
fn concreto(rota: &str) -> String {
    rota.split('/')
        .map(|segmento| {
            if segmento.starts_with('{') {
                Uuid::new_v4().to_string()
            } else {
                segmento.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[tokio::test]
async fn nenhuma_rota_do_core_responde_a_quem_nao_tem_sessao() {
    let pool = pool!();
    let org = organisation(&pool).await;
    let state = nucleo(pool.clone(), org);

    let rotas = inventario();
    assert!(
        rotas.len() > 150,
        "o inventário encontrou só {} rotas",
        rotas.len()
    );
    // O mesmo número que `repository-facts.sh` publica na Secção 1: se os dois
    // divergirem, a varredura está a olhar para menos rotas do que existem.
    let operacoes: usize = rotas.iter().map(|(_, m)| m.len()).sum();
    let factos = std::process::Command::new("sh")
        .arg("-c")
        .arg("./scripts/repository-facts.sh | awk '/operacoes-core/ {print $2}'")
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .output()
        .expect("repository-facts");
    let publicadas: usize = String::from_utf8_lossy(&factos.stdout)
        .trim()
        .parse()
        .expect("operacoes-core");
    assert_eq!(
        operacoes, publicadas,
        "a varredura vê {operacoes} operações e os factos do repositório dizem {publicadas}"
    );
    let mut verificadas = 0;
    let mut abertas = Vec::new();
    for (rota, metodos) in &rotas {
        for metodo in metodos {
            if PUBLICAS.iter().any(|(m, p, _)| m == metodo && p == rota) {
                continue;
            }
            let caminho = if rota.starts_with("/api/") {
                concreto(rota)
            } else {
                format!("/api/v1{}", concreto(rota))
            };
            let corpo = (*metodo != "GET" && *metodo != "DELETE").then(|| json!({}));
            let (status, _) = pedido(&state, None, None, metodo, &caminho, corpo).await;
            verificadas += 1;
            let outra = OUTRA_CREDENCIAL.iter().any(|(p, _)| p == rota);
            let recusou = status == StatusCode::UNAUTHORIZED
                || (outra && (status.is_client_error() || status == StatusCode::FORBIDDEN));
            if !recusou {
                abertas.push(format!("{metodo} {rota} → {status}"));
            }
        }
    }
    assert!(
        abertas.is_empty(),
        "{} de {verificadas} pedidos sem sessão não foram recusados:\n  {}",
        abertas.len(),
        abertas.join("\n  ")
    );
    eprintln!("{verificadas} pedidos sem sessão, todos recusados");
}

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
