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

// ── Calendário ───────────────────────────────────────────────────────────

async fn fuso(s: &Sistema, token: &str) -> String {
    s.http
        .get(format!("{}/api/v1/me", s.core_url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["timezone"]
        .as_str()
        .unwrap_or("UTC")
        .to_owned()
}

#[tokio::test]
async fn calendario_cria_no_fuso_do_membro_abre_edita_e_cancela() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    // Um fuso que não é UTC, para que «o fuso do membro» e «UTC por omissão»
    // não se confundam: 09:30 em Luanda são 08:30 UTC.
    sqlx::query(
        "INSERT INTO instance_settings (organisation_id, timezone) VALUES ($1, 'Africa/Luanda')
         ON CONFLICT (organisation_id) DO UPDATE SET timezone = EXCLUDED.timezone",
    )
    .bind(s.organisation_id)
    .execute(&s.pool)
    .await
    .expect("fuso da Instância");
    let (_, c, t) = membro(&s).await;
    let marca = format!("reuniao{}", Uuid::new_v4().simple());
    // O formulário do Design: nomes `start`/`end`, hora local, âmbito pessoal.
    let (status, html) = s.html("/calendar/events/new?date=2026-10-07", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"name="start""#) && html.contains(r#"name="scope""#));
    let r = s
        .escrever(reqwest::Method::POST, "/calendar/events/new", &c)
        .form(&[
            ("title", marca.as_str()),
            ("start", "2026-10-07T09:30"),
            ("end", "2026-10-07T10:30"),
            ("location", "Sala 2"),
            ("scope", "personal"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        303,
        "{}",
        r.text().await.unwrap_or_default()
    );
    let destino = location(&r);
    let id = destino.trim_start_matches("/calendar/events/").to_owned();
    // No Core: a hora que o membro escreveu, no fuso dele (nunca UTC às cegas).
    let evento: Value = s
        .http
        .get(format!("{}/api/v1/calendar/events/{id}", s.core_url))
        .bearer_auth(&t)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let zona = fuso(&s, &t).await;
    assert_eq!(zona, "Africa/Luanda");
    assert_eq!(evento["timezone"], "Africa/Luanda");
    assert_eq!(
        evento["starts_at"], "2026-10-07T08:30:00Z",
        "a hora local não foi lida no fuso do membro"
    );
    // Mês, semana e dia mostram-no no dia e à hora locais.
    for (vista, marca_vista) in [
        ("month", "oc-cal-month"),
        ("week", "oc-cal-tl"),
        ("day", "oc-cal-tl"),
        ("agenda", "oc-cal-agenda"),
    ] {
        let (status, html) = s
            .html(&format!("/calendar?view={vista}&date=2026-10-07"), &c)
            .await;
        assert_eq!(status, 200, "{vista}");
        assert!(html.contains(marca_vista), "{vista}");
        assert!(html.contains(&marca), "{vista} sem o evento");
    }
    let (_, html) = s.html("/calendar?view=day&date=2026-10-07", &c).await;
    assert!(
        html.contains(r#"data-start="570" data-dur="60""#),
        "não está às 09:30 locais"
    );
    assert!(html.contains(r#"data-scope="personal""#));
    // O detalhe no inspector, com o local e a ligação à Nye.
    let (_, html) = s.html(&destino, &c).await;
    assert!(html.contains(r#"data-part="cal-details""#) && html.contains("Sala 2"));
    assert!(html.contains(&format!(r#"href="/ai/prompt?ref=event:{id}""#)));
    // Editar: a hora muda, o evento é o mesmo.
    let r = s
        .escrever(
            reqwest::Method::POST,
            &format!("/calendar/events/{id}/edit"),
            &c,
        )
        .form(&[
            ("title", marca.as_str()),
            ("start", "2026-10-07T14:00"),
            ("end", "2026-10-07T15:00"),
            ("location", ""),
            ("scope", "personal"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);
    let (_, html) = s.html("/calendar?view=day&date=2026-10-07", &c).await;
    assert!(html.contains(r#"data-start="840" data-dur="60""#));
    // Cancelar: fica, riscado.
    let r = s
        .escrever(
            reqwest::Method::POST,
            &format!("/calendar/events/{id}/cancel"),
            &c,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);
    let (_, html) = s.html("/calendar?view=day&date=2026-10-07", &c).await;
    assert!(html.contains(&marca) && html.contains("data-cancelled"));
}

#[tokio::test]
async fn calendario_datas_invalidas_voltam_ao_formulario_com_o_erro() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, _) = membro(&s).await;
    let r = s
        .escrever(reqwest::Method::POST, "/calendar/events/new", &c)
        .form(&[
            ("title", "Ao contrário"),
            ("start", "2026-10-07T11:00"),
            ("end", "2026-10-07T10:00"),
            ("scope", "personal"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 422);
    let html = r.text().await.unwrap();
    assert!(html.contains(crate_t("prod.cal.err.order")));
    assert!(
        html.contains(r#"value="Ao contrário""#),
        "o que o membro escreveu perdeu-se"
    );
}

#[tokio::test]
async fn calendario_o_evento_pessoal_de_outra_pessoa_nao_existe() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c, _) = membro(&s).await;
    let (_, outro_c, _) = membro(&s).await;
    let r = s
        .escrever(reqwest::Method::POST, "/calendar/events/new", &outro_c)
        .form(&[
            ("title", "Consulta privada"),
            ("start", "2026-10-08T09:00"),
            ("end", "2026-10-08T10:00"),
            ("scope", "personal"),
        ])
        .send()
        .await
        .unwrap();
    let alheio = location(&r);
    let (_, html) = s.html(&alheio, &c).await;
    assert!(!html.contains("Consulta privada"));
    assert!(html.contains(r#"data-error="app.err.not_found""#));
    let (_, html) = s.html("/calendar?view=week&date=2026-10-08", &c).await;
    assert!(!html.contains("Consulta privada"));
}

fn crate_t(key: &str) -> &'static str {
    ocinye_workspace::i18n::t(key)
}

// ── Ficheiros ────────────────────────────────────────────────────────────

use sha2::{Digest, Sha256};

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

async fn com_armazenamento(s: &Sistema) -> bool {
    if armazenamento().is_some() {
        // O Core guarda cada objecto num backend registado; o de teste é o que
        // o `ObjectStore` do harness diz ser.
        sqlx::query(
            "INSERT INTO storage_backends
                 (code, kind, display_name, location_label, bucket, is_default, is_active)
             VALUES ('ocinye-test-default', 's3_compatible', 'Test', 'test', 'ocinye-test-artifacts', TRUE, TRUE)
             ON CONFLICT DO NOTHING",
        )
        .execute(&s.pool)
        .await
        .expect("backend de teste");
        return true;
    }
    assert!(
        std::env::var("CI").is_err(),
        "sem armazenamento, Ficheiros não se prova; defina OCINYE_TEST_STORAGE_ENDPOINT"
    );
    eprintln!("skipping: OCINYE_TEST_STORAGE_ENDPOINT is not set");
    false
}

async fn json_post(s: &Sistema, cookie: &str, path: &str, body: &Value) -> reqwest::Response {
    s.escrever(reqwest::Method::POST, path, cookie)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(body.to_string())
        .send()
        .await
        .unwrap()
}

/// O envio por partes, exactamente como o `files-engine.js` o faz: preflight,
/// sessão pessoal, cada parte com a sua soma, fecho com a soma do todo.
async fn enviar(
    s: &Sistema,
    cookie: &str,
    nome: &str,
    tipo: &str,
    bytes: &[u8],
    pasta: Option<&str>,
) -> Value {
    let r = json_post(
        s,
        cookie,
        "/files/upload-preflight",
        &json!({ "size_bytes": bytes.len() }),
    )
    .await;
    assert!(r.status().is_success(), "preflight {}", r.status());
    let mut abertura = json!({ "filename": nome, "content_type": tipo, "size_bytes": bytes.len() });
    if let Some(p) = pasta {
        abertura["folder_id"] = json!(p);
    }
    let r = json_post(s, cookie, "/files/personal-upload", &abertura).await;
    let status = r.status();
    let corpo = r.text().await.unwrap_or_default();
    assert!(status.is_success(), "abertura {status} {corpo}");
    let sessao: Value = serde_json::from_str(&corpo).unwrap();
    let id = sessao["session_id"].as_str().unwrap().to_owned();
    let parte = usize::try_from(sessao["chunk_size_bytes"].as_u64().unwrap()).unwrap();
    let total = sessao["total_parts"].as_u64().unwrap();
    let mut todo = Sha256::new();
    for (n, pedaco) in bytes.chunks(parte.max(1)).enumerate() {
        todo.update(pedaco);
        let r = s
            .escrever(
                reqwest::Method::PUT,
                &format!(
                    "/files/uploads/{id}/parts/{}?sha256={}",
                    n + 1,
                    hex(&Sha256::digest(pedaco))
                ),
                cookie,
            )
            .header("content-type", "application/octet-stream")
            .body(pedaco.to_vec())
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "parte {} {}", n + 1, r.status());
    }
    let r = json_post(
        s,
        cookie,
        &format!("/files/uploads/{id}/complete"),
        &json!({ "sha256": hex(&todo.finalize()) }),
    )
    .await;
    let status = r.status();
    if !status.is_success() {
        panic!("fecho {status} {}", r.text().await.unwrap_or_default());
    }
    let mut feito: Value = r.json().await.unwrap();
    feito["total_parts"] = json!(total);
    feito
}

/// O identificador opaco de um ficheiro na página (`f.<ficheiro>.<versão>`).
fn referencia(feito: &Value) -> String {
    format!(
        "f.{}.{}",
        feito["file_id"].as_str().unwrap(),
        feito["version_id"].as_str().unwrap()
    )
}

#[tokio::test]
async fn ficheiros_envio_por_partes_sem_limite_fixo_e_com_a_soma_do_todo() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    if !com_armazenamento(&s).await {
        return;
    }
    let (_, c, _) = membro(&s).await;
    // Sem tecto fixo de 512 MB: quem diz o que cabe é o preflight do Core.
    let r = json_post(
        &s,
        &c,
        "/files/upload-preflight",
        &json!({ "size_bytes": 600u64 * 1024 * 1024 }),
    )
    .await;
    let cap: Value = r.json().await.unwrap();
    assert!(
        cap["effective_max_uploadable_bytes"].as_u64().unwrap_or(0) > 512 * 1024 * 1024,
        "{cap}"
    );
    // Um ficheiro de duas partes (as partes do Core são de 32 MiB).
    let bytes: Vec<u8> = (0..(32 * 1024 * 1024 + 4096))
        .map(|i| (i % 251) as u8)
        .collect();
    let nome = format!("serie-{}.txt", Uuid::new_v4().simple());
    let feito = enviar(&s, &c, &nome, "text/plain", &bytes, None).await;
    assert_eq!(feito["total_parts"], 2);
    let (_, html) = s.html("/files", &c).await;
    assert!(
        html.contains(&nome),
        "o ficheiro enviado não aparece nos meus ficheiros"
    );
    assert!(
        html.contains("/static/files-engine.js"),
        "o motor de envio não está na página"
    );
}

#[tokio::test]
async fn ficheiros_cancelar_aborta_e_intencoes_malformadas_sao_recusadas() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    if !com_armazenamento(&s).await {
        return;
    }
    let (_, c, _) = membro(&s).await;
    let (_, outro_c, _) = membro(&s).await;
    let dados = b"linha 1\nlinha 2\n".to_vec();
    let abrir = |nome: &str| json!({ "filename": nome, "content_type": "text/plain", "size_bytes": dados.len() });
    // Cancelar: a sessão aborta, e o fecho depois disso não cria nada.
    let r = json_post(&s, &c, "/files/personal-upload", &abrir("cancelado.txt")).await;
    let id = r.json::<Value>().await.unwrap()["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = s
        .escrever(reqwest::Method::DELETE, &format!("/files/uploads/{id}"), &c)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.status());
    let r = json_post(
        &s,
        &c,
        &format!("/files/uploads/{id}/complete"),
        &json!({ "sha256": hex(&Sha256::digest(&dados)) }),
    )
    .await;
    assert!(!r.status().is_success());
    // Uma parte com a soma errada é recusada.
    let r = json_post(&s, &c, "/files/personal-upload", &abrir("errado.txt")).await;
    let id = r.json::<Value>().await.unwrap()["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = s
        .escrever(
            reqwest::Method::PUT,
            &format!("/files/uploads/{id}/parts/1?sha256={}", "0".repeat(64)),
            &c,
        )
        .header("content-type", "application/octet-stream")
        .body(dados.clone())
        .send()
        .await
        .unwrap();
    assert!(!r.status().is_success(), "parte com soma errada aceite");
    // A soma do todo que não bate recusa o fecho.
    let r = s
        .escrever(
            reqwest::Method::PUT,
            &format!(
                "/files/uploads/{id}/parts/1?sha256={}",
                hex(&Sha256::digest(&dados))
            ),
            &c,
        )
        .header("content-type", "application/octet-stream")
        .body(dados.clone())
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let r = json_post(
        &s,
        &c,
        &format!("/files/uploads/{id}/complete"),
        &json!({ "sha256": "0".repeat(64) }),
    )
    .await;
    assert!(!r.status().is_success(), "fecho com a soma errada aceite");
    // A sessão de outra pessoa não se usa; a pasta de outra pessoa não é destino.
    let r = s
        .escrever(
            reqwest::Method::DELETE,
            &format!("/files/uploads/{id}"),
            &outro_c,
        )
        .send()
        .await
        .unwrap();
    assert!(
        !r.status().is_success(),
        "a sessão de outra pessoa foi cancelada"
    );
    let r = s
        .escrever(reqwest::Method::POST, "/files/folder", &outro_c)
        .form(&[("name", "Dela"), ("parent", "root")])
        .send()
        .await
        .unwrap();
    let pasta_alheia = location(&r).trim_start_matches("/files?folder=").to_owned();
    let mut pedido = abrir("intruso.txt");
    pedido["folder_id"] = json!(pasta_alheia);
    let r = json_post(&s, &c, "/files/personal-upload", &pedido).await;
    assert!(
        !r.status().is_success(),
        "enviou para a pasta de outra pessoa"
    );
    let (_, html) = s.html("/files", &c).await;
    assert!(
        !html.contains("cancelado.txt")
            && !html.contains("errado.txt")
            && !html.contains("intruso.txt")
    );
}

#[tokio::test]
async fn ficheiros_lista_inspector_e_accoes_pelo_core() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    if !com_armazenamento(&s).await {
        return;
    }
    let (_, c, _) = membro(&s).await;
    let texto = enviar(
        &s,
        &c,
        "leituras.txt",
        "text/plain",
        b"vento 12 m/s\n<script>x</script>",
        None,
    )
    .await;
    let png: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    let imagem = enviar(&s, &c, "anemometro.png", "image/png", png, None).await;
    let (rt, ri) = (referencia(&texto), referencia(&imagem));

    // A lista e o inspector: o texto escapado (nunca executado), a imagem pela
    // origem do Workspace, descarregar, e a Nye com a referência do ficheiro.
    let (_, html) = s.html("/files", &c).await;
    assert!(
        html.contains(r#"data-app="files""#)
            && html.contains("leituras.txt")
            && html.contains("anemometro.png")
    );
    assert!(!html.contains("oc-pending oc-win__state"));
    let (_, html) = s.html(&format!("/files?item={rt}"), &c).await;
    assert!(html.contains(r#"data-part="files-details""#) && html.contains("vento 12 m/s"));
    assert!(html.contains("&lt;script&gt;x&lt;/script&gt;") && !html.contains("<script>x"));
    let v = texto["version_id"].as_str().unwrap();
    assert!(html.contains(&format!(r#"href="/me/files/{v}/download""#)));
    assert!(html.contains(&format!(
        "/ai/prompt?ref=file:{}",
        texto["file_id"].as_str().unwrap()
    )));
    assert!(
        html.contains(crate_t("files.versions.none")),
        "versões inventadas"
    );
    let (_, html) = s.html(&format!("/files?item={ri}"), &c).await;
    assert!(html.contains(&format!(
        r#"src="/me/files/{}/inline""#,
        imagem["version_id"].as_str().unwrap()
    )));

    // Pasta nova, mudar o nome.
    let r = s
        .escrever(reqwest::Method::POST, "/files/folder", &c)
        .form(&[("name", "Campanha"), ("parent", "root")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);
    let (_, html) = s.html("/files", &c).await;
    assert!(html.contains("Campanha") && html.contains(r#"data-kind="folder""#));
    let r = s
        .escrever(reqwest::Method::POST, &format!("/files/{rt}/rename"), &c)
        .form(&[("name", "leituras-torre2.txt")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);
    let (_, html) = s.html("/files", &c).await;
    assert!(html.contains("leituras-torre2.txt"));

    // Favorito, lixo e restaurar, pela barra de selecção (cada item pelo Core).
    let sel = |op: &str, item: &str| format!("op={op}&item={item}");
    let post_sel = |body: String| {
        s.escrever(reqwest::Method::POST, "/files/selection", &c)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
    };
    assert_eq!(
        post_sel(sel("favourite", &rt))
            .await
            .unwrap()
            .status()
            .as_u16(),
        303
    );
    let (_, html) = s.html("/files?section=favourites", &c).await;
    assert!(html.contains("leituras-torre2.txt"));
    assert_eq!(
        post_sel(format!("{}&item={ri}", sel("trash", &rt)))
            .await
            .unwrap()
            .status()
            .as_u16(),
        303
    );
    let (_, html) = s.html("/files?section=trash", &c).await;
    assert!(html.contains("leituras-torre2.txt") && html.contains("anemometro.png"));
    assert_eq!(
        post_sel(sel("restore", &ri))
            .await
            .unwrap()
            .status()
            .as_u16(),
        303
    );
    let (_, html) = s.html("/files", &c).await;
    assert!(html.contains("anemometro.png") && !html.contains("leituras-torre2.txt"));

    // Eliminar para sempre: sem confirmação desenhada, recusa-se, e nada se apaga.
    let r = post_sel(sel("purge", &rt)).await.unwrap();
    assert!(!r.status().is_success() && r.status().as_u16() != 303);
    let (_, html) = s.html("/files?section=trash", &c).await;
    assert!(
        html.contains("leituras-torre2.txt"),
        "eliminado sem confirmação"
    );
    // Mover pela barra precisa de destino: recusado, nada muda.
    let r = post_sel(sel("move", &ri)).await.unwrap();
    assert!(!r.status().is_success());
    // Descarregar um: a descarga da versão exacta.
    let r = post_sel(sel("download", &ri)).await.unwrap();
    assert_eq!(
        location(&r),
        format!(
            "/me/files/{}/download",
            imagem["version_id"].as_str().unwrap()
        )
    );
}

#[tokio::test]
async fn ficheiros_os_de_outra_pessoa_nao_se_veem_nem_se_mudam() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    if !com_armazenamento(&s).await {
        return;
    }
    let (_, c, _) = membro(&s).await;
    let (_, outro_c, _) = membro(&s).await;
    let alheio = enviar(
        &s,
        &outro_c,
        "confidencial.txt",
        "text/plain",
        b"segredo",
        None,
    )
    .await;
    let ra = referencia(&alheio);
    // Pelo endereço, com a referência certa: não há inspector, nem conteúdo.
    let (_, html) = s.html(&format!("/files?item={ra}"), &c).await;
    assert!(!html.contains("confidencial.txt") && !html.contains("segredo"));
    assert!(!html.contains(r#"data-part="files-details""#));
    // Mudar-lhe o nome ou pô-lo no lixo: o Core recusa, e o ficheiro fica.
    let r = s
        .escrever(reqwest::Method::POST, &format!("/files/{ra}/rename"), &c)
        .form(&[("name", "meu.txt")])
        .send()
        .await
        .unwrap();
    assert!(!r.status().is_success() && r.status().as_u16() != 303);
    let r = s
        .escrever(reqwest::Method::POST, "/files/selection", &c)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("op=trash&item={ra}"))
        .send()
        .await
        .unwrap();
    assert!(!r.status().is_success() && r.status().as_u16() != 303);
    let (_, html) = s.html("/files", &outro_c).await;
    assert!(
        html.contains("confidencial.txt"),
        "o ficheiro de outra pessoa mudou"
    );
    // Um identificador que não é uma referência (um caminho) é recusado.
    let r = s
        .escrever(
            reqwest::Method::POST,
            "/files/..%2F..%2Fetc%2Fpasswd/rename",
            &c,
        )
        .form(&[("name", "x")])
        .send()
        .await
        .unwrap();
    assert!(!r.status().is_success());
}
