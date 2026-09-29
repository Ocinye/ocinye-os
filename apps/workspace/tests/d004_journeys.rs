//! As viagens das aplicações de produtividade (Claude Design D004) contra um
//! Core real: Notas, Ficheiros, Calendário e Correio.
//!
//! ```text
//! pedido HTTP → Workspace (ui::apps) → Core → PostgreSQL
//! ```
//!
//! O Core governa; as aplicações implementam. Cada viagem prova um contrato
//! que existe de facto — e, onde o Core ou o Design não o têm, que o ecrã o diz
//! em vez de o fingir.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida — e diz que saltou;
//! em CI, falha.

mod common;

use common::*;
use ocinye_contracts::TechnicalRole;
use serde_json::{json, Value};
use uuid::Uuid;

/// Uma sessão do Core para a mesma pessoa (para criar dados pelo caminho real).
async fn token(s: &Sistema, email: &str, password: &str) -> String {
    s.http
        .post(format!("{}/api/v1/auth/login", s.core_url))
        .json(&json!({ "email": email, "password": password }))
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

/// Um membro de investigação com sessão no Workspace e no Core.
async fn membro(s: &Sistema) -> (Uuid, String, String) {
    let (id, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, cookie) = s.entrar(&email, &password).await;
    let t = token(s, &email, &password).await;
    (id, cookie, t)
}

async fn core_post(s: &Sistema, token: &str, path: &str, body: &Value) -> reqwest::Response {
    s.http
        .post(format!("{}/api/v1{path}", s.core_url))
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .expect("pedido ao Core")
}

/// Uma nota da pessoa, pela API do Core: um título e um parágrafo.
async fn nota(s: &Sistema, token: &str, titulo: &str, texto: &str) -> String {
    let r = core_post(
        s,
        token,
        "/me/notes",
        &json!({
            "title": titulo,
            "document": { "schema_version": 1, "blocks": [
                { "type": "paragraph", "content": [{ "type": "text", "text": texto, "marks": [] }] }
            ]}
        }),
    )
    .await;
    assert!(r.status().is_success(), "{}", r.status());
    r.json::<Value>().await.expect("nota")["id"]
        .as_str()
        .expect("id")
        .to_owned()
}

async fn nota_no_core(s: &Sistema, token: &str, id: &str) -> Value {
    s.http
        .get(format!("{}/api/v1/me/notes/{id}", s.core_url))
        .bearer_auth(token)
        .send()
        .await
        .expect("ler nota")
        .json::<Value>()
        .await
        .expect("nota")
}

/// O valor de um atributo `name="…" value="…"` no HTML.
fn valor(html: &str, name: &str) -> String {
    let at = html
        .find(&format!(r#"name="{name}" value=""#))
        .unwrap_or_else(|| panic!("campo {name}"));
    let rest = &html[at + name.len() + 15..];
    rest[..rest.find('"').expect("fim")].to_owned()
}

/// O texto do `<textarea>` do editor.
fn corpo(html: &str) -> String {
    let at = html.find(r#"id="oc-notes-body""#).expect("editor");
    let start = at + html[at..].find('>').expect(">") + 1;
    let end = start + html[start..].find("</textarea>").expect("</textarea>");
    html[start..end].to_owned()
}

async fn gravar(s: &Sistema, cookie: &str, id: &str, form: &[(&str, &str)]) -> reqwest::Response {
    s.escrever(
        reqwest::Method::POST,
        &format!("/notes/{id}/gravar"),
        cookie,
    )
    .form(form)
    .send()
    .await
    .unwrap()
}

// ── Notas ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn notas_lista_abre_grava_e_o_texto_e_o_documento() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, t) = membro(&s).await;
    let marca = format!("vento{}", Uuid::new_v4().simple());
    let id = nota(&s, &t, &format!("Torre {marca}"), "primeiro texto").await;

    // A lista e o editor, na janela das Notas, com o ecrã do Design.
    let (status, html) = s.html("/notes", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"data-app="notes""#) && html.contains(r#"data-oc="win""#));
    assert!(html.contains(&format!(r#"href="/notes/{id}""#)));
    assert!(!html.contains("oc-pending oc-win__state"));

    let (status, html) = s.html(&format!("/notes/{id}"), &c).await;
    assert_eq!(status, 200);
    assert_eq!(corpo(&html), "primeiro texto");
    let base = valor(&html, "base_revision");
    assert!(html.contains(&format!(r#"action="/notes/{id}/gravar""#)));
    assert!(html.contains(&format!(r#"id="oc-notes-doc-{id}""#)));

    // Gravar: o texto do editor vira o documento do Core, estrutura a estrutura.
    let texto = "## Medições\n\n- **forte** a _norte_\n- `v = 12`\n\n[fonte](https://ocinye.com)";
    let r = gravar(
        &s,
        &c,
        &id,
        &[
            ("title", "Torre 2"),
            ("body", texto),
            ("base_revision", &base),
        ],
    )
    .await;
    assert_eq!(r.status().as_u16(), 303);
    assert_eq!(location(&r), format!("/notes/{id}?saved=1"));
    let doc = nota_no_core(&s, &t, &id).await;
    assert_eq!(doc["title"], "Torre 2");
    let blocks = doc["document"]["blocks"].as_array().expect("blocos");
    assert_eq!(blocks[0]["type"], "heading");
    assert_eq!(blocks[1]["type"], "bullet_list");
    assert_eq!(blocks[1]["items"][0][0]["marks"][0], "bold");
    assert_eq!(blocks[2]["content"][0]["type"], "link");

    // Voltar a abrir mostra o mesmo texto, e diz que ficou guardado.
    let (_, html) = s.html(&format!("/notes/{id}?saved=1"), &c).await;
    assert_eq!(corpo(&html), texto);
    assert!(html.contains(r#"data-state="saved""#));
}

#[tokio::test]
async fn notas_uma_revisao_nova_entretanto_e_conflito_e_o_texto_fica() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, t) = membro(&s).await;
    let id = nota(&s, &t, "Relatório", "base").await;
    let (_, html) = s.html(&format!("/notes/{id}"), &c).await;
    let base = valor(&html, "base_revision");
    // Outra sessão grava primeiro.
    let r = gravar(
        &s,
        &c,
        &id,
        &[
            ("title", "Relatório"),
            ("body", "da outra sessão"),
            ("base_revision", &base),
        ],
    )
    .await;
    assert_eq!(r.status().as_u16(), 303);
    // Esta grava com a revisão antiga: conflito tipado, nada sobreposto.
    let r = gravar(
        &s,
        &c,
        &id,
        &[
            ("title", "Relatório"),
            ("body", "o meu texto"),
            ("base_revision", &base),
        ],
    )
    .await;
    assert_eq!(r.status().as_u16(), 409);
    let html = r.text().await.unwrap();
    assert!(html.contains(r#"data-state="failed""#));
    assert_eq!(corpo(&html), "o meu texto", "o texto do membro perdeu-se");
    let doc = nota_no_core(&s, &t, &id).await;
    assert_eq!(
        doc["document"]["blocks"][0]["content"][0]["text"], "da outra sessão",
        "o conflito sobrepôs a outra revisão"
    );
}

#[tokio::test]
async fn notas_criar_pesquisar_e_so_as_proprias() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, t) = membro(&s).await;
    let (_, _, outro_t) = membro(&s).await;
    let marca = format!("solander{}", Uuid::new_v4().simple());
    let minha = nota(&s, &t, &format!("Ata {marca}"), "x").await;
    let alheia = nota(&s, &outro_t, &format!("Segredo {marca}"), "y").await;

    // Criar: POST, e o editor da nota nova.
    let r = s
        .escrever(reqwest::Method::POST, "/notes/new", &c)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);
    assert!(location(&r).starts_with("/notes/"));

    // A pesquisa em Notas encontra a do membro e nunca a de outra pessoa.
    let (_, html) = s.html(&format!("/notes?q={marca}"), &c).await;
    assert!(html.contains(&format!(r#"href="/notes/{minha}""#)));
    assert!(!html.contains(&alheia) && !html.contains("Segredo"));

    // A de outra pessoa, pelo endereço, é «não encontrado» — nunca o conteúdo.
    let (_, html) = s.html(&format!("/notes/{alheia}"), &c).await;
    assert!(html.contains(r#"data-error="app.err.not_found""#));
    assert!(!html.contains("Segredo") && !html.contains(r#"id="oc-notes-body""#));
}

#[tokio::test]
async fn notas_uma_partilha_revogada_deixa_de_mostrar_o_conteudo() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, _, dono_t) = membro(&s).await;
    let (leitor, c, _) = membro(&s).await;
    let id = nota(&s, &dono_t, "Partilhada", "conteúdo reservado").await;
    let r = core_post(
        &s,
        &dono_t,
        &format!("/me/notes/{id}/shares"),
        &json!({ "person_id": leitor, "role": "viewer" }),
    )
    .await;
    assert!(r.status().is_success(), "{}", r.status());

    // Quem só lê vê o texto, sem poder gravar nem apagar.
    let (_, html) = s.html(&format!("/notes/{id}"), &c).await;
    assert!(html.contains("conteúdo reservado"));
    assert!(html.contains(r#"readonly"#) && !html.contains("/apagar"));

    // Revogada, a mesma janela deixa de mostrar o conteúdo.
    let r = s
        .http
        .delete(format!(
            "{}/api/v1/me/notes/{id}/shares/{leitor}",
            s.core_url
        ))
        .bearer_auth(&dono_t)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.status());
    let (_, html) = s.html(&format!("/notes/{id}"), &c).await;
    assert!(
        !html.contains("conteúdo reservado"),
        "conteúdo revogado ainda visível"
    );
    assert!(html.contains(r#"data-state="error""#));
}

#[tokio::test]
async fn notas_fechar_com_alteracoes_e_o_dialogo_d002_com_guardar_da_nota() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, t) = membro(&s).await;
    let id = nota(&s, &t, "Rascunho", "antes").await;
    let (_, html) = s.html(&format!("/notes/{id}"), &c).await;
    // O modelo do fecho: o diálogo D002, inerte, com «Guardar» a submeter o
    // formulário da nota (`then=close`) e «Não guardar» ao gestor de janelas.
    let tpl = &html[html
        .find(r#"<template data-part="app-dirty""#)
        .expect("modelo do fecho")..];
    let tpl = &tpl[..tpl.find("</template>").expect("fim")];
    assert!(tpl.contains(r#"data-oc="dirty-close""#) && tpl.contains(r#"role="alertdialog""#));
    assert!(tpl.contains(&format!(r#"form="oc-notes-doc-{id}""#)));
    assert!(tpl.contains(r#"name="then" value="close""#));
    assert!(tpl.contains(r#"name="decision" value="discard""#));
    let win = {
        let at = html
            .find(r#"<template data-part="app-dirty" data-win=""#)
            .unwrap()
            + 43;
        html[at..at + html[at..].find('"').unwrap()].to_owned()
    };
    // «Guardar» do diálogo: grava e fecha a janela da nota.
    let base = valor(&html, "base_revision");
    let r = gravar(
        &s,
        &c,
        &id,
        &[
            ("title", "Rascunho"),
            ("body", "depois"),
            ("base_revision", &base),
            ("then", "close"),
        ],
    )
    .await;
    assert_eq!(r.status().as_u16(), 303);
    let janelas: Value = s
        .get("/wm", &c)
        .header("accept", "application/json")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        janelas["windows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["id"] != win.as_str()),
        "a janela da nota continua aberta depois de «Guardar»"
    );
    assert_eq!(
        nota_no_core(&s, &t, &id).await["document"]["blocks"][0]["content"][0]["text"],
        "depois"
    );
}

#[tokio::test]
async fn notas_o_conteudo_e_texto_nunca_html() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, t) = membro(&s).await;
    let id = nota(&s, &t, "Hostil", "x").await;
    let (_, html) = s.html(&format!("/notes/{id}"), &c).await;
    let base = valor(&html, "base_revision");
    let hostil =
        "</textarea><script>alert(1)</script><img src=x onerror=alert(2)> [a](javascript:alert(3))";
    let r = gravar(
        &s,
        &c,
        &id,
        &[
            ("title", "Hostil"),
            ("body", hostil),
            ("base_revision", &base),
        ],
    )
    .await;
    assert_eq!(r.status().as_u16(), 303);
    let (_, html) = s.html(&format!("/notes/{id}"), &c).await;
    assert!(!html.contains("<script>alert(1)"), "o texto saiu do campo");
    assert!(!html.contains("<img src=x"));
    assert!(html.contains("&lt;/textarea&gt;&lt;script&gt;"));
    // A ligação `javascript:` nunca se tornou ligação no documento.
    let doc = nota_no_core(&s, &t, &id).await;
    let content = doc["document"]["blocks"][0]["content"].as_array().unwrap();
    assert!(content.iter().all(|i| i["type"] != "link"));
}
