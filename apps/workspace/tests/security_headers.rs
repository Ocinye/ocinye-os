//! Os cabeçalhos de segurança são servidos, e o transporte é exigido em produção.
//!
//! # Porque isto não existia
//!
//! A auditoria pós-Boot encontrou o conjunto completo — CSP sem `inline`,
//! `frame-ancestors 'none'`, `nosniff`, `Referrer-Policy`, `Permissions-Policy`,
//! `no-store` — e **zero cobertura**. Nada notaria se uma directiva caísse numa
//! refactorização, e a CSP é o tipo de linha que alguém relaxa «só para
//! experimentar» e nunca volta a apertar.
//!
//! Estes testes conduzem o router verdadeiro. Não é preciso um Core de pé: os
//! cabeçalhos aplicam-se a todas as respostas, incluindo a de uma rota que não
//! existe.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ocinye_workspace::config::WorkspaceConfig;
use ocinye_workspace::session::SessionStore;
use ocinye_workspace::{routes, WorkspaceState};
use std::time::Duration;
use tower::ServiceExt;

/// O Workspace, com a configuração que se pedir.
fn estado(producao: bool) -> WorkspaceState {
    let publica = if producao {
        "https://ocinye.example"
    } else {
        "http://127.0.0.1:8090"
    };
    WorkspaceState {
        config: std::sync::Arc::new(WorkspaceConfig {
            core_transport: ocinye_workspace::config::CoreTransport::Roteavel,
            bind_address: "127.0.0.1:0".to_owned(),
            core_url: "http://127.0.0.1:1".to_owned(),
            public_url: publica.to_owned(),
            session_ttl: Duration::from_secs(3600),
            cookie_secure: producao,
            log_level: "warn".to_owned(),
            log_format: "pretty".to_owned(),
            is_production: producao,
            static_dir: format!("{}/static", env!("CARGO_MANIFEST_DIR")),
        }),
        sessions: SessionStore::new(),
        http: reqwest::Client::new(),
    }
}

/// Os cabeçalhos de uma resposta qualquer.
async fn cabecalhos(producao: bool) -> axum::http::HeaderMap {
    let resposta = routes::router(estado(producao))
        .oneshot(
            Request::builder()
                .uri("/uma-rota-que-nao-existe")
                // Com marcador de arranque: sem ele, o portão encaminha para
                // `/boot` e mediríamos os cabeçalhos de um encaminhamento em
                // vez dos do caminho comum.
                .header("cookie", "oc_boot=1")
                .body(Body::empty())
                .expect("pedido"),
        )
        .await
        .expect("resposta");
    assert_eq!(
        resposta.status(),
        StatusCode::NOT_FOUND,
        "esperava-se que esta rota não existisse: se passar a existir, o teste \
         deixa de estar a medir o caminho comum"
    );
    resposta.headers().clone()
}

fn valor(mapa: &axum::http::HeaderMap, nome: &str) -> String {
    mapa.get(nome)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

/// Toda a resposta leva o conjunto completo.
#[tokio::test]
async fn toda_a_resposta_leva_os_cabecalhos_de_seguranca() {
    let cabecalhos = cabecalhos(false).await;

    for (nome, esperado) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "same-origin"),
        ("cross-origin-opener-policy", "same-origin"),
        ("cache-control", "no-store"),
    ] {
        assert_eq!(
            valor(&cabecalhos, nome),
            esperado,
            "o cabeçalho `{nome}` mudou ou desapareceu"
        );
    }
}

/// A política de conteúdo não abre portas que estavam fechadas.
///
/// Cada uma destas directivas foi escolhida, e a lista é o contrato. Um `'self'`
/// a mais em `default-src`, ou um `'unsafe-inline'` em `script-src`, são as duas
/// formas mais comuns de uma CSP deixar de servir para alguma coisa.
#[tokio::test]
async fn a_politica_de_conteudo_continua_fechada() {
    let politica = valor(&cabecalhos(false).await, "content-security-policy");
    assert!(!politica.is_empty(), "não há política de conteúdo nenhuma");

    for exigido in [
        "default-src 'none'",
        "script-src 'self'",
        "form-action 'self'",
        "base-uri 'none'",
        "frame-ancestors 'none'",
    ] {
        assert!(
            politica.contains(exigido),
            "a política perdeu `{exigido}`: {politica}"
        );
    }

    for proibido in ["unsafe-inline", "unsafe-eval", "*"] {
        assert!(
            !politica.contains(proibido),
            "a política ganhou `{proibido}`, que a abre: {politica}"
        );
    }
}

/// O transporte é exigido em produção, e não em desenvolvimento.
///
/// As duas metades importam. Sem a primeira, o primeiro pedido de cada pessoa
/// pode acontecer em claro. Com ela em desenvolvimento, o browser de quem
/// trabalha aqui recusa-se a falar com `localhost` em claro durante um ano — e
/// não há como desfazer isso a não ser limpando o estado do browser.
#[tokio::test]
async fn o_transporte_seguro_e_exigido_em_producao_e_so_ai() {
    let producao = valor(&cabecalhos(true).await, "strict-transport-security");
    assert!(
        producao.contains("max-age=31536000") && producao.contains("includeSubDomains"),
        "produção não exige transporte seguro: «{producao}»"
    );

    let desenvolvimento = valor(&cabecalhos(false).await, "strict-transport-security");
    assert!(
        desenvolvimento.is_empty(),
        "desenvolvimento corre em claro e recebeu `Strict-Transport-Security`: \
         isto tranca o `localhost` do browser de quem desenvolve"
    );
}
