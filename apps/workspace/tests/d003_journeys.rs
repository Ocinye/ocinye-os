//! As viagens da Nye (Claude Design D003) sem inferência — o estado real
//! desta Instância e de qualquer instalação sem nó de IA.
//!
//! ```text
//! pedido HTTP → Workspace → POST /agentic/invoke · /ai/prompt · /ai/conversations → Core → PostgreSQL
//! ```
//!
//! A invariante constitucional (ADR-0619): **sem IA, a Nye continua a
//! pesquisar, abrir e navegar**; perguntar e agir dizem porque não podem, e
//! nada é inventado. As viagens de proposta, confirmação e execução, que
//! precisam de um Planner, estão em `d003_act_journeys.rs`.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida — e diz que saltou;
//! em CI, falha.

mod common;

use common::*;
use ocinye_contracts::TechnicalRole;
use serde_json::Value;
use uuid::Uuid;

/// Uma sessão do Core para a mesma pessoa (para criar dados pelo caminho real).
async fn token(s: &Sistema, email: &str, password: &str) -> String {
    s.http
        .post(format!("{}/api/v1/auth/login", s.core_url))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("login no Core")
        .json::<Value>()
        .await
        .expect("sessão do Core")["session_token"]
        .as_str()
        .expect("token")
        .to_owned()
}

/// Uma nota da pessoa, pela API do Core, com o título dado.
async fn nota(s: &Sistema, token: &str, titulo: &str) -> String {
    let r = s
        .http
        .post(format!("{}/api/v1/me/notes", s.core_url))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "title": titulo,
            "document": { "schema_version": 1, "blocks": [{ "type": "paragraph", "content": [] }] }
        }))
        .send()
        .await
        .expect("criar nota");
    assert!(r.status().is_success(), "{}", r.status());
    r.json::<Value>().await.expect("nota")["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

/// Um membro de investigação (tem `ai.use`) com sessão no Workspace e no Core.
async fn membro(s: &Sistema) -> (Uuid, String, String) {
    let (id, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, cookie) = s.entrar(&email, &password).await;
    let t = token(s, &email, &password).await;
    (id, cookie, t)
}

fn enc(q: &str) -> String {
    q.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                char::from(b).to_string()
            }
            b' ' => "+".to_owned(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

// ── A superfície universal ────────────────────────────────────────────────

#[tokio::test]
async fn a_superficie_esta_em_todas_as_paginas_fechada_e_honesta() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, _) = membro(&s).await;
    let (status, html) = s.html("/", &c).await;
    assert_eq!(status, 200);
    // A paleta D001 é agora a superfície da Nye: a mesma âncora, os mesmos
    // ganchos, o pedido vai para /ask.
    assert!(html.contains(r#"id="oc-palette""#) && html.contains("data-nye"));
    assert!(html.contains(r#"action="/ask""#) && html.contains(r#"data-part="palette-q""#));
    assert!(
        !html.contains(
            r#"data-open="" role="dialog" aria-modal="true" aria-labelledby="oc-nye-title""#
        ),
        "aberta sem pedido"
    );
    // Perguntar e agir não estão disponíveis, com a razão; pesquisar está.
    assert!(html.contains(r#"value="ask""#) && html.contains(r#"value="act""#));
    // O microfone da barra de cima não parece funcionar: sem voz, o controlo D001.
    assert!(html.contains(r#"aria-describedby="oc-voice-pending""#));
    assert!(!html.contains(r#"href="/ai/prompt?voice=1""#));
}

#[tokio::test]
async fn sem_ia_a_pesquisa_encontra_e_abre_so_o_que_o_membro_pode_ver() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, t) = membro(&s).await;
    let (_, outro_c, outro_t) = membro(&s).await;
    let marca = format!("solander{}", Uuid::new_v4().simple());
    let minha = nota(&s, &t, &format!("Ata {marca}")).await;
    let alheia = nota(&s, &outro_t, &format!("Segredo {marca}")).await;

    // 10 · Sem nenhum fornecedor de IA, a pesquisa funciona.
    let (status, html) = s
        .html(&format!("/ask?q={}&intent=search", enc(&marca)), &c)
        .await;
    assert_eq!(status, 200);
    assert!(
        html.contains(&format!(r#"href="/notes/{minha}""#)),
        "a nota do membro não apareceu"
    );
    // 1 · Um recurso escondido não aparece na pesquisa da Nye.
    assert!(
        !html.contains(&alheia) && !html.contains("Segredo"),
        "a nota de outra pessoa apareceu"
    );
    // A ligação leva a uma rota canónica, e abre (janela gerida).
    let (status, _) = s.html(&format!("/notes/{minha}"), &c).await;
    assert_eq!(status, 200);
    // O outro membro vê a sua, e não a minha.
    let (_, html) = s
        .html(&format!("/ask?q={}&intent=search", enc(&marca)), &outro_c)
        .await;
    assert!(html.contains(&alheia) && !html.contains(&minha));
}

#[tokio::test]
async fn perguntar_sem_inferencia_diz_porque_e_continua_a_pesquisar() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, t) = membro(&s).await;
    let marca = format!("tagus{}", Uuid::new_v4().simple());
    let minha = nota(&s, &t, &format!("Relatório {marca}")).await;
    let (status, html) = s
        .html(&format!("/ask?q={}&intent=ask", enc(&marca)), &c)
        .await;
    assert_eq!(status, 200);
    // O estado sem inferência, nunca «offline» nem uma resposta inventada.
    assert!(
        html.contains(crate_t("nye.reason.no_inference")),
        "sem a razão tipada"
    );
    assert!(!html.to_lowercase().contains("offline"));
    assert!(
        !html.contains(r#"data-role="nye""#),
        "uma resposta da Nye sem inferência"
    );
    // A mesma pergunta continua a encontrar o que o membro tem.
    assert!(html.contains(&format!(r#"href="/notes/{minha}""#)));
}

#[tokio::test]
async fn agir_sem_planner_nao_cria_nenhum_plano() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (id, c, _) = membro(&s).await;
    let (status, _) = s
        .html(
            &format!("/ask?q={}", enc("cria uma nota sobre o sensor")),
            &c,
        )
        .await;
    assert_eq!(status, 200);
    let planos: i64 =
        sqlx::query_scalar("SELECT count(*) FROM action_plans WHERE requested_by = $1")
            .bind(id)
            .fetch_one(&s.pool)
            .await
            .expect("planos");
    assert_eq!(planos, 0, "um plano apareceu sem Planner");
}

// ── A aplicação Nye ───────────────────────────────────────────────────────

#[tokio::test]
async fn a_nye_e_uma_janela_so_com_o_nome_nye() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, _) = membro(&s).await;
    let (status, html) = s.html("/ai/prompt", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"data-app="prompt""#) && html.contains(r#"data-oc="nye-app""#));
    assert!(
        html.contains(r#"<h2 id="win-w1-title" class="oc-win__title">Nye</h2>"#),
        "o título da janela não é Nye"
    );
    assert!(!html.contains("Prompt Ocinye"));
    // Sem inferência, o composer diz porquê.
    assert!(html.contains(crate_t("nye.reason.no_inference")));
    // Abrir de novo foca a mesma janela.
    s.html("/ai/prompt?panel=sources", &c).await;
    let wm = s
        .get("/wm", &c)
        .header("accept", "application/json")
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let nye = wm["windows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["app_id"] == "prompt")
        .count();
    assert_eq!(nye, 1, "a Nye abriu duas janelas");
    // O lançador mostra Nye.
    let (_, html) = s.html("/", &c).await;
    assert!(html.contains(">Nye<"));
}

#[tokio::test]
async fn as_conversas_sao_do_membro_e_continuam() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, _) = membro(&s).await;
    let (_, outro, _) = membro(&s).await;
    // Criar: o pedido vai ao Core, que o guarda na conversa (com a conclusão
    // degradada, porque não há inferência).
    let r = s
        .escrever(reqwest::Method::POST, "/ai/prompt", &c)
        .form(&[("prompt", "Qual é o estado do projecto Solander?")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);
    let destino = location(&r);
    assert!(destino.starts_with("/ai/prompt?c="), "{destino}");
    let conversa = destino.trim_start_matches("/ai/prompt?c=").to_owned();
    // Continuar na mesma conversa.
    let r = s
        .escrever(reqwest::Method::POST, &destino, &c)
        .form(&[("prompt", "E das tarefas?")])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), destino);
    // Abrir e listar: os dois pedidos, e a razão tipada em vez do texto do Core.
    let (_, html) = s.html(&destino, &c).await;
    assert!(
        html.contains("Qual é o estado do projecto Solander?") && html.contains("E das tarefas?")
    );
    assert!(
        html.contains(&format!(r#"href="/ai/prompt?c={conversa}""#)),
        "a conversa não está na lista"
    );
    assert!(
        !html.contains("Não existe nenhum nó"),
        "o texto de sistema do Core apareceu"
    );
    // Refrescar mostra o mesmo.
    let (_, again) = s.html(&destino, &c).await;
    assert_eq!(
        again.matches("E das tarefas?").count(),
        html.matches("E das tarefas?").count()
    );
    // 7 · A conversa de outro membro não abre, nem aparece na lista.
    let (status, html) = s.html(&destino, &outro).await;
    assert_eq!(status, 200);
    assert!(
        !html.contains("Qual é o estado do projecto Solander?"),
        "a conversa de outra pessoa abriu"
    );
    assert!(!html.contains(&conversa));
}

#[tokio::test]
async fn a_voz_diz_que_nao_esta_disponivel_e_nao_finge() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, _) = membro(&s).await;
    let (_, html) = s.html("/ai/prompt?voice=1", &c).await;
    assert!(html.contains(r#"data-oc="nye-voice""#));
    assert!(html.contains(crate_t("nye.reason.voice_unavailable")));
    // Nenhum botão de falar activo sem voz: o controlo existe, indisponível.
    assert!(
        !html.contains(r#"data-oc="nye-ptt""#),
        "premir para falar activo sem voz"
    );
    assert!(html.contains(r#"aria-disabled="true" aria-describedby="oc-nye-voice-privacy""#));
}

#[tokio::test]
async fn sem_ai_use_nao_ha_nye_nem_perguntas() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, email, password) = s.pessoa(&[TechnicalRole::Auditor]).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    // 2 · A aplicação Nye não abre por quem não a pode usar, e não ganha janela.
    let (status, _) = s.html("/ai/prompt", &c).await;
    assert_eq!(status, 404);
    // A pesquisa continua; perguntar diz que é uma questão de permissão.
    let (status, html) = s.html("/ask?q=teste&intent=ask", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(crate_t("nye.reason.permission_denied")));
}

#[tokio::test]
async fn nenhum_nome_de_fornecedor_nas_paginas_normais() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, _) = membro(&s).await;
    for rota in ["/", "/ai/prompt", "/ask?q=teste&intent=ask"] {
        for locale in ["pt", "en", "fr"] {
            let (_, html) = s.html(rota, &format!("{c}; oc_locale={locale}")).await;
            let lower = html.to_lowercase();
            for marca in [
                "qwen",
                "deepseek",
                "openai",
                "anthropic",
                "claude",
                "llama",
                "fixture",
            ] {
                assert!(!lower.contains(marca), "{marca} em {rota} ({locale})");
            }
        }
    }
}

/// A frase de uma chave do catálogo, no idioma canónico (pt).
fn crate_t(key: &str) -> &'static str {
    ocinye_workspace::i18n::t(key)
}
