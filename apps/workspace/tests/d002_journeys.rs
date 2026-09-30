//! As viagens da integração Claude Design D002 — o Gestor de Janelas e os
//! painéis — pelo caminho real:
//!
//! ```text
//! pedido HTTP → rota do Workspace → motor de janelas → HTTP → Core → PostgreSQL
//! ```
//!
//! O que se prova aqui é o contrato que o `wm-engine.js` usa (`/wm`, `409`,
//! `?close=`, `?frame=1`) e o HTML que uma pessoa recebe. O arrastar e a
//! apresentação por largura (tablet, telemóvel) medem-se no browser; aqui
//! fica o contrato CSS que os decide (ver o fim do ficheiro).
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida — e diz que saltou;
//! em CI, falha.

mod common;

use common::*;
use ocinye_contracts::TechnicalRole;
use serde_json::Value;
use uuid::Uuid;

/// O estado das janelas desta sessão (WM-1).
async fn janelas(s: &Sistema, cookie: &str) -> Value {
    let r = s
        .get("/wm", cookie)
        .header("accept", "application/json")
        .send()
        .await
        .expect("GET /wm");
    assert_eq!(r.status().as_u16(), 200, "GET /wm");
    r.json().await.expect("json")
}

/// Uma operação sobre uma janela, como o motor a pede (JSON).
async fn operar(s: &Sistema, cookie: &str, id: &str, form: &[(&str, &str)]) -> (u16, Value) {
    let r = s
        .escrever(reqwest::Method::POST, &format!("/wm/{id}"), cookie)
        .header("accept", "application/json")
        .form(form)
        .send()
        .await
        .expect("POST /wm/{id}");
    let status = r.status().as_u16();
    (status, r.json().await.unwrap_or(Value::Null))
}

fn ids(v: &Value) -> Vec<String> {
    v["windows"]
        .as_array()
        .expect("windows")
        .iter()
        .map(|w| w["id"].as_str().expect("id").to_owned())
        .collect()
}

fn janela<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["windows"]
        .as_array()
        .expect("windows")
        .iter()
        .find(|w| w["id"] == id)
        .unwrap_or_else(|| panic!("janela {id} em falta: {v}"))
}

fn activa(v: &Value) -> String {
    v["active"].as_str().unwrap_or_default().to_owned()
}

/// Um membro de investigação com sessão.
async fn membro(s: &Sistema) -> (Uuid, String) {
    s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await
}

/// Uma administradora, depois do segundo factor (a administração exige-o).
async fn admin(s: &Sistema) -> (Uuid, String) {
    let (id, email, password) = s.pessoa(&[TechnicalRole::PlatformAdmin]).await;
    s.totp_confirmado(id).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    let r = s
        .escrever(reqwest::Method::POST, "/mfa/challenge", &c)
        .form(&[("code", codigo_totp(SEMENTE_MFA).as_str())])
        .send()
        .await
        .expect("desafio");
    let novo = r
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with(&format!("{}=", ocinye_workspace::session::COOKIE_NAME)))
        .map(|c| c.split(';').next().unwrap_or_default().to_owned())
        .expect("sessão depois do MFA");
    (id, novo)
}

// ── Política de lançamento ─────────────────────────────────────────────────

#[tokio::test]
async fn uma_aplicacao_de_uma_janela_abre_uma_e_volta_a_focar_a_mesma() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;

    // 1 · Abrir: uma janela, com o estado honesto do `app_pending` dentro
    // (o Terminal: Meus Recursos ganhou o seu ecrã na D007).
    let (status, html) = s.html("/terminal", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"data-oc="wm-layer""#) && html.contains(r#"data-oc="win""#));
    assert!(
        html.contains("oc-pending oc-win__state"),
        "a janela não tem o app_pending"
    );
    assert!(
        html.contains("/static/wm-engine.js"),
        "o motor não foi carregado"
    );
    let v = janelas(&s, &c).await;
    assert_eq!(ids(&v).len(), 1);
    let w1 = ids(&v)[0].clone();

    // 2 · Lançar de novo, pela rota ou pelo motor: foca a mesma.
    let (status, _) = s.html("/terminal", &c).await;
    assert_eq!(status, 200);
    let r = s
        .escrever(reqwest::Method::POST, "/wm", &c)
        .header("accept", "application/json")
        .form(&[
            ("app_id", "terminal"),
            ("href", "/terminal"),
            ("window", "new"),
        ])
        .send()
        .await
        .expect("POST /wm");
    assert_eq!(r.status().as_u16(), 200);
    let aberto: Value = r.json().await.expect("json");
    assert_eq!(aberto["existing"], true, "{aberto}");
    assert_eq!(aberto["id"], w1.as_str());
    assert_eq!(ids(&janelas(&s, &c).await), [w1]);
}

#[tokio::test]
async fn uma_aplicacao_de_varias_janelas_abre_outra_e_recarregar_nao_abre_mais() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;
    let (status, html) = s.html("/notes", &c).await;
    assert_eq!(status, 200);
    // O Design só oferece «Nova janela» onde o registo deixa: aqui, sim.
    assert!(
        !html.contains("oc-chooser__new"),
        "com uma janela não há escolha"
    );

    // 3 · «Nova janela»: uma segunda janela, e o endereço passa a ser o dela.
    let r = s.get("/notes?window=new", &c).send().await.expect("GET");
    assert_eq!(r.status().as_u16(), 303);
    assert_eq!(location(&r), "/notes");
    let v = janelas(&s, &c).await;
    assert_eq!(ids(&v).len(), 2);
    let (_, html) = s.html("/notes", &c).await;
    assert_eq!(
        ids(&janelas(&s, &c).await).len(),
        2,
        "recarregar abriu outra"
    );
    assert!(
        html.contains(r#"data-oc="chooser""#) && html.contains("oc-chooser__new"),
        "duas janelas: a barra de aplicações escolhe, e há «Nova janela»"
    );
    assert!(html.contains(r#"data-windows="2""#) && html.contains(r#"data-n="2""#));
}

// ── Ciclo de vida ──────────────────────────────────────────────────────────

#[tokio::test]
async fn minimizar_restaurar_maximizar_encaixar_e_fechar() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;
    s.html("/notes", &c).await;
    s.html("/my-work", &c).await;
    let v = janelas(&s, &c).await;
    let (notas, trabalho) = (ids(&v)[0].clone(), ids(&v)[1].clone());
    assert_eq!(activa(&v), trabalho, "a última aberta é a activa");

    // 4 · Minimizar: o foco passa à seguinte, e a janela continua ao alcance —
    // na barra de aplicações e no alternador (D003: já não há prateleira).
    let (status, v) = operar(&s, &c, &trabalho, &[("op", "minimize")]).await;
    assert_eq!(status, 200);
    assert_eq!(janela(&v, &trabalho)["state"], "minimized");
    assert_eq!(activa(&v), notas);
    assert_eq!(v["href"], "/notes", "o endereço segue a janela activa");
    let (_, html) = s.html("/notes", &c).await;
    assert!(!html.contains("oc-shelf"), "a prateleira voltou");
    assert!(
        html.contains(&format!(
            r#"data-oc="wm-focus" data-win="{trabalho}" data-part="switcher-item""#
        )),
        "o alternador não mostra a minimizada"
    );
    let app = janela(&v, &trabalho)["app_id"]
        .as_str()
        .expect("aplicação")
        .to_owned();
    assert!(
        html.contains(&format!(r#"data-app="{app}" data-windows="1""#)),
        "a barra de aplicações não diz que a minimizada está aberta"
    );

    // 5 · Restaurar: volta, à frente e activa.
    let (_, v) = operar(&s, &c, &trabalho, &[("op", "restore")]).await;
    assert_eq!(janela(&v, &trabalho)["state"], "normal");
    assert_eq!(activa(&v), trabalho);

    // 6 · Maximizar e restaurar.
    let (_, v) = operar(&s, &c, &notas, &[("op", "maximize")]).await;
    assert_eq!(janela(&v, &notas)["state"], "maximized");
    assert_eq!(activa(&v), notas);
    let (_, v) = operar(&s, &c, &notas, &[("op", "restore")]).await;
    assert_eq!(janela(&v, &notas)["state"], "normal");

    // 7 · Encaixar à esquerda e à direita (sem quartos).
    let (_, v) = operar(&s, &c, &notas, &[("op", "snap"), ("zone", "left")]).await;
    assert_eq!(janela(&v, &notas)["state"], "snap-left");
    let (_, v) = operar(&s, &c, &trabalho, &[("op", "snap"), ("zone", "right")]).await;
    assert_eq!(janela(&v, &trabalho)["state"], "snap-right");
    let (status, _) = operar(&s, &c, &notas, &[("op", "snap"), ("zone", "top-left")]).await;
    assert_eq!(status, 422, "um quarto não é uma zona");

    // 8 · Fechar uma janela limpa: fecha já.
    let (status, v) = operar(&s, &c, &trabalho, &[("op", "close")]).await;
    assert_eq!(status, 200);
    assert_eq!(ids(&v), vec![notas.clone()]);

    // Sem JavaScript, cada controlo é um formulário que volta à janela activa.
    let r = s
        .escrever(reqwest::Method::POST, &format!("/wm/{notas}"), &c)
        .form(&[("op", "minimize")])
        .send()
        .await
        .expect("POST");
    assert_eq!(r.status().as_u16(), 303);
    assert_eq!(location(&r), "/", "sem janela activa, o Desktop");

    // Fechar a última deixa a casca D001, sem camada nem prateleira.
    operar(&s, &c, &notas, &[("op", "close")]).await;
    let (_, html) = s.html("/", &c).await;
    assert!(
        !html.contains(r#"data-oc="wm-layer""#)
            && !html.contains(r#"data-oc="shelf""#)
            && !html.contains("data-wm")
    );
    assert!(!html.contains("wm-engine.js"));
}

#[tokio::test]
async fn a_ordem_e_o_foco_sao_do_servidor_e_so_ha_uma_activa() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;
    s.html("/notes", &c).await;
    s.html("/files", &c).await;
    s.html("/my-work", &c).await;
    let v = janelas(&s, &c).await;
    let [a, b, _] = [ids(&v)[0].clone(), ids(&v)[1].clone(), ids(&v)[2].clone()];
    let (_, v) = operar(&s, &c, &a, &[("op", "focus")]).await;
    assert_eq!(activa(&v), a);
    let activas = v["windows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["active"] == true)
        .count();
    assert_eq!(activas, 1);
    // A página desenha a mesma ordem: a activa à frente.
    let (_, html) = s.html("/notes", &c).await;
    assert!(html.contains(&format!(r#"data-win="{a}" data-app="notes""#)));
    // Minimizada não é activa, mesmo sendo a de ordem mais alta.
    operar(&s, &c, &a, &[("op", "minimize")]).await;
    let v = janelas(&s, &c).await;
    assert_ne!(activa(&v), a);
    let _ = b;
}

// ── Fechar com trabalho por guardar ────────────────────────────────────────

#[tokio::test]
async fn fechar_com_trabalho_por_guardar_pede_a_decisao_e_executa_a_confirmada() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;
    s.html("/notes", &c).await;
    let w = ids(&janelas(&s, &c).await)[0].clone();
    let relatar = |dirty: &'static str, can_save: &'static str| {
        s.escrever(reqwest::Method::POST, &format!("/wm/{w}/state"), &c)
            .header("accept", "application/json")
            .form(&[("dirty", dirty), ("can_save", can_save)])
            .send()
    };
    assert_eq!(
        relatar("true", "false").await.unwrap().status().as_u16(),
        200
    );

    // 9 · Fechar pede a decisão: 409 com o endereço do diálogo (JS) ou 303 (sem JS).
    let (status, recusa) = operar(&s, &c, &w, &[("op", "close")]).await;
    assert_eq!((status, recusa["reason"].as_str()), (409, Some("dirty")));
    let confirm = recusa["confirm"].as_str().unwrap().to_owned();
    assert_eq!(confirm, format!("/notes?close={w}"));
    let (_, html) = s.html(&confirm, &c).await;
    assert!(
        html.contains(r#"role="alertdialog""#)
            && html.contains(&format!(r#"action="/wm/{w}/close""#))
    );
    assert!(html.contains(r#"value="cancel""#) && html.contains(r#"value="discard""#));
    // Sem poder guardar, «Guardar» está indisponível e diz porquê.
    assert!(
        html.contains(r#"aria-describedby="oc-dirty-nosave""#) && !html.contains(r#"value="save""#)
    );

    let decidir = |d: &'static str| {
        s.escrever(reqwest::Method::POST, &format!("/wm/{w}/close"), &c)
            .header("accept", "application/json")
            .form(&[("decision", d)])
            .send()
    };
    // Cancelar: fica, por guardar.
    assert_eq!(decidir("cancel").await.unwrap().status().as_u16(), 200);
    assert_eq!(janela(&janelas(&s, &c).await, &w)["dirty"], true);
    // Guardar sem poder: recusado, nada muda.
    let r = decidir("save").await.unwrap();
    assert_eq!(r.status().as_u16(), 409);
    assert_eq!(ids(&janelas(&s, &c).await), vec![w.clone()]);
    // Uma decisão que não existe não é «Não guardar».
    assert_eq!(decidir("maybe").await.unwrap().status().as_u16(), 422);
    assert_eq!(ids(&janelas(&s, &c).await), vec![w.clone()]);

    // Guardar podendo: espera que a aplicação guarde, e fecha quando ela diz.
    relatar("true", "true").await.unwrap();
    assert_eq!(decidir("save").await.unwrap().status().as_u16(), 200);
    assert_eq!(
        ids(&janelas(&s, &c).await),
        vec![w.clone()],
        "fechou antes de guardar"
    );
    relatar("false", "true").await.unwrap();
    assert!(
        ids(&janelas(&s, &c).await).is_empty(),
        "guardou e não fechou"
    );

    // Não guardar: fecha, com trabalho por guardar e tudo.
    s.html("/notes", &c).await;
    let w2 = ids(&janelas(&s, &c).await)[0].clone();
    s.escrever(reqwest::Method::POST, &format!("/wm/{w2}/state"), &c)
        .form(&[("dirty", "true"), ("can_save", "true")])
        .send()
        .await
        .unwrap();
    let r = s
        .escrever(reqwest::Method::POST, &format!("/wm/{w2}/close"), &c)
        .form(&[("decision", "discard")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);
    assert!(ids(&janelas(&s, &c).await).is_empty());
}

/// O fim do `<div>` que abre em `start` (etiquetas equilibradas).
fn end_of_div(html: &str, start: usize) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    loop {
        let open = html[i..].find("<div").map(|p| p + i);
        let close = html[i..].find("</div>").map(|p| p + i).expect("</div>");
        match open {
            Some(o) if o < close => {
                depth += 1;
                i = o + 4;
            }
            _ => {
                depth -= 1;
                i = close + 6;
                if depth == 0 {
                    return i;
                }
            }
        }
    }
}

/// O diálogo de fechar é uma camada global: desenhado depois da casca, fora
/// do `.oc-desk` (`isolation: isolate`) e da camada das janelas, cobre tudo
/// (D002.1 · DIRTY_CLOSE_GLOBAL_LAYER = VERIFIED_ALREADY_GLOBAL).
#[tokio::test]
async fn o_dialogo_de_fechar_fica_fora_do_desktop_e_das_janelas() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;
    s.html("/notes", &c).await;
    let w = ids(&janelas(&s, &c).await)[0].clone();
    s.escrever(reqwest::Method::POST, &format!("/wm/{w}/state"), &c)
        .form(&[("dirty", "true"), ("can_save", "false")])
        .send()
        .await
        .unwrap();
    let (_, html) = s.html(&format!("/notes?close={w}"), &c).await;
    let shell_start = html[..html.find(r#"class="oc-shell""#).unwrap()]
        .rfind("<div")
        .unwrap();
    let shell_end = end_of_div(&html, shell_start);
    let dialog = html.find(r#"data-oc="dirty-close""#).expect("diálogo");
    assert!(dialog > shell_end, "o diálogo entrou na casca");
    assert_eq!(html.matches(r#"data-oc="dirty-close""#).count(), 1);
}

// ── Autoridade e isolamento ───────────────────────────────────────────────

#[tokio::test]
async fn uma_aplicacao_que_o_membro_nao_abre_nunca_ganha_janela() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    // Um membro sem `members.manage`: a Administração está escondida.
    let (_, c) = membro(&s).await;
    // 10 · Pela rota: 404, como uma rota que não existe, e nenhuma janela.
    let (status, _) = s.html("/admin", &c).await;
    assert_eq!(status, 404);
    assert!(ids(&janelas(&s, &c).await).is_empty());
    // Pelo motor: a mesma resposta, e nenhuma janela.
    let r = s
        .escrever(reqwest::Method::POST, "/wm", &c)
        .header("accept", "application/json")
        .form(&[("app_id", "administration"), ("href", "/admin")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 404);
    assert!(ids(&janelas(&s, &c).await).is_empty());
    // 11 · Continua escondida: nem no lançador, nem na barra de aplicações.
    let (_, html) = s.html("/", &c).await;
    assert!(!html.contains(r#"href="/admin""#));
    // Uma rota de outra aplicação não entra numa janela desta.
    let r = s
        .escrever(reqwest::Method::POST, "/wm", &c)
        .header("accept", "application/json")
        .form(&[("app_id", "notes"), ("href", "/admin")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 422);
    for mau in [
        "//evil.example/notes",
        "/notes/../admin",
        "https://evil.example",
    ] {
        let r = s
            .escrever(reqwest::Method::POST, "/wm", &c)
            .header("accept", "application/json")
            .form(&[("app_id", "notes"), ("href", mau)])
            .send()
            .await
            .unwrap();
        assert_eq!(r.status().as_u16(), 422, "{mau}");
    }
}

#[tokio::test]
async fn uma_permissao_retirada_fecha_a_janela_na_pagina_seguinte() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (id, c) = admin(&s).await;
    let (status, _) = s.html("/admin", &c).await;
    assert_eq!(status, 200);
    assert_eq!(ids(&janelas(&s, &c).await).len(), 1);
    sqlx::query("DELETE FROM person_roles WHERE person_id = $1 AND role = $2")
        .bind(id)
        .bind(TechnicalRole::PlatformAdmin.as_str())
        .execute(&s.pool)
        .await
        .unwrap();
    // 15 · Refrescar não reabre: a página seguinte fecha-a antes de desenhar.
    let (_, html) = s.html("/", &c).await;
    assert!(!html.contains(r#"data-app="administration""#));
    assert!(ids(&janelas(&s, &c).await).is_empty());
}

#[tokio::test]
async fn as_janelas_de_uma_sessao_nao_sao_de_outra() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, ana) = membro(&s).await;
    let (_, rui) = membro(&s).await;
    s.html("/notes", &ana).await;
    let w = ids(&janelas(&s, &ana).await)[0].clone();
    // O identificador de uma janela não é um token: noutra sessão não existe.
    let (status, recusa) = operar(&s, &rui, &w, &[("op", "close")]).await;
    assert_eq!(
        (status, recusa["reason"].as_str()),
        (404, Some("no_such_window"))
    );
    assert!(ids(&janelas(&s, &rui).await).is_empty());
    assert_eq!(ids(&janelas(&s, &ana).await), [w]);
    // Sem sessão, não há mesa.
    let r = s
        .http
        .get(format!("{}/wm", s.url))
        .header("accept", "application/json")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 401);
}

#[tokio::test]
async fn numeros_que_nao_sao_numeros_e_geometria_fora_da_area_sao_recusados_ou_limitados() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;
    s.html("/notes", &c).await;
    let w = ids(&janelas(&s, &c).await)[0].clone();
    for mau in [
        vec![("op", "move"), ("x", "NaN"), ("y", "0")],
        vec![("op", "move"), ("x", "1e9"), ("y", "0")],
        vec![
            ("op", "resize"),
            ("x", "0"),
            ("y", "0"),
            ("w", "-5"),
            ("h", "300"),
        ],
        vec![("op", "move"), ("x", "0")],
        vec![("op", "explode")],
        vec![
            ("op", "move"),
            ("x", "0"),
            ("y", "0"),
            ("area_w", "10"),
            ("area_h", "10"),
        ],
    ] {
        let (status, _) = operar(&s, &c, &w, &mau).await;
        assert_eq!(status, 422, "{mau:?}");
    }
    // Um número válido mas fora da área é aproximado: a pega fica ao alcance.
    let (status, v) = operar(
        &s,
        &c,
        &w,
        &[
            ("op", "move"),
            ("x", "-99999"),
            ("y", "-50"),
            ("area_w", "1400"),
            ("area_h", "800"),
        ],
    )
    .await;
    assert_eq!(status, 200);
    let j = janela(&v, &w);
    assert_eq!(
        (j["x"].as_i64(), j["y"].as_i64()),
        (Some(-(760 - 120)), Some(0))
    );
    let (_, v) = operar(
        &s,
        &c,
        &w,
        &[
            ("op", "resize"),
            ("x", "0"),
            ("y", "0"),
            ("w", "99999"),
            ("h", "10"),
            ("area_w", "1400"),
            ("area_h", "800"),
        ],
    )
    .await;
    let j = janela(&v, &w);
    assert_eq!((j["w"].as_u64(), j["h"].as_u64()), (Some(1400), Some(240)));
}

// ── Refrescar, sessão, frame ───────────────────────────────────────────────

#[tokio::test]
async fn refrescar_desenha_o_mesmo_estado_e_uma_sessao_nova_comeca_sem_janelas() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    s.html("/notes", &c).await;
    s.html("/files", &c).await;
    let v = janelas(&s, &c).await;
    let notas = ids(&v)[0].clone();
    operar(&s, &c, &notas, &[("op", "snap"), ("zone", "left")]).await;
    let (_, a) = s.html("/notes", &c).await;
    let (_, b) = s.html("/notes", &c).await;
    let camada = |h: &str| {
        let i = h.find(r#"data-oc="wm-layer""#).unwrap();
        h[i..i + h[i..].find(r#"data-part="wm-live""#).unwrap()].to_owned()
    };
    assert_eq!(camada(&a), camada(&b), "refrescar mudou as janelas");
    assert!(a.contains(r#"data-state="snap-left""#));

    // WM-4: só o corpo da janela, sem casca.
    let (status, frame) = s.html("/files?frame=1", &c).await;
    assert_eq!(status, 200);
    assert!(
        frame.starts_with(r#"<template data-part="win-title">"#),
        "{frame}"
    );
    assert!(!frame.contains("oc-top") && !frame.contains("<html"));
    assert_eq!(
        ids(&janelas(&s, &c).await).len(),
        2,
        "um frame não abre janelas"
    );

    // Sair acaba com a sessão e com as janelas dela.
    s.escrever(reqwest::Method::POST, "/logout", &c)
        .send()
        .await
        .unwrap();
    let (_, _, c2) = s.entrar(&email, &password).await;
    assert!(ids(&janelas(&s, &c2).await).is_empty());
}

// ── Painéis e menu do Desktop ─────────────────────────────────────────────

#[tokio::test]
async fn o_painel_do_estado_so_conta_o_core_e_so_o_admin_ve_o_detalhe() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;
    let (_, html) = s.html("/", &c).await;
    // 13 · O painel abre da pastilha CORE·IA e não inventa estados.
    assert!(html.contains(r#"class="oc-menu oc-panel-menu""#));
    assert!(
        html.contains(r#"<li data-state="ok" class="oc-cap">"#),
        "o Core"
    );
    assert!(
        html.contains(r#"data-state="unknown""#),
        "cópias sem registo"
    );
    assert!(
        html.contains("oc-panel__note"),
        "sem IA, a nota de que o Ocinye continua"
    );
    assert!(
        !html.contains(r#"href="/admin/monitor""#),
        "o detalhe é só de administração"
    );
    let (_, a) = admin(&s).await;
    let (_, html) = s.html("/", &a).await;
    assert!(html.contains(r#"<a href="/admin/monitor" class="oc-panel__foot">"#));
}

#[tokio::test]
async fn as_notificacoes_de_um_membro_nao_sao_as_de_outro() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (ana_id, ana) = membro(&s).await;
    let (rui_id, rui) = membro(&s).await;
    for (quem, titulo) in [(ana_id, "Para a Ana"), (rui_id, "Para o Rui")] {
        sqlx::query(
            "INSERT INTO notifications (organisation_id, recipient_id, kind, title)
             VALUES ($1, $2, 'reminder', $3)",
        )
        .bind(s.organisation_id)
        .bind(quem)
        .bind(titulo)
        .execute(&s.pool)
        .await
        .unwrap();
    }
    // 14 · Cada um vê só as suas.
    let (_, html) = s.html("/", &ana).await;
    assert!(html.contains("Para a Ana") && !html.contains("Para o Rui"));
    assert!(html.contains("oc-note__dot") && html.contains(r#"action="/notifications/read-all""#));
    // «Marcar todas como lidas» volta à página e só toca nas da Ana.
    let r = s
        .escrever(reqwest::Method::POST, "/notifications/read-all", &ana)
        .header("referer", format!("{}/notes", s.url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        (r.status().as_u16(), location(&r).as_str()),
        (303, "/notes")
    );
    let (_, html) = s.html("/", &ana).await;
    assert!(!html.contains("oc-note__dot"), "ficou por ler");
    let (_, html) = s.html("/", &rui).await;
    assert!(
        html.contains("Para o Rui") && html.contains("oc-note__dot"),
        "leu as do Rui"
    );
    // Um `Referer` de fora não é para onde se volta.
    let r = s
        .escrever(reqwest::Method::POST, "/notifications/read-all", &rui)
        .header("referer", "https://evil.example/x")
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/");
}

#[tokio::test]
async fn o_relogio_mostra_o_mes_e_a_agenda_de_hoje() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;
    let (_, html) = s.html("/", &c).await;
    assert!(html.contains(r#"<table class="oc-month">"#));
    assert!(
        html.contains(r#"aria-current="date""#),
        "hoje não está marcado"
    );
    assert!(html.contains(r#"<a href="/calendar" class="oc-panel__foot">"#));
}

#[tokio::test]
async fn o_menu_do_desktop_e_so_do_desktop_e_segue_a_politica() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = membro(&s).await;
    let (_, html) = s.html("/", &c).await;
    // 12 · O menu entra no Desktop e acciona os controlos D001 que já existem.
    assert!(html.contains(r#"data-oc="desk-ctx""#));
    for proxy in ["desk-lib-open", "desk-bg-open", "desk-edit"] {
        assert!(
            html.contains(&format!(r#"data-proxy="{proxy}""#)),
            "{proxy}"
        );
        assert!(
            html.contains(&format!(r#"data-oc="{proxy}""#)),
            "o controlo D001 de {proxy} não existe"
        );
    }
    // Numa janela de aplicação, não há menu do Desktop.
    let (_, html) = s.html("/notes", &c).await;
    assert!(!html.contains(r#"data-oc="desk-ctx""#));
}

// ── Contrato CSS: tablet e telemóvel são apresentações do mesmo estado ────

const WM_CSS: &str = include_str!("../static/oc-wm.css");

#[test]
fn abaixo_de_1100px_as_janelas_estao_sempre_maximizadas_e_sem_gestos() {
    // 16 · A 924×540, e em todo o tablet: sem arrastar, redimensionar nem
    // encaixar, e sem o controlo de maximizar, que seria um controlo morto.
    let tablet = WM_CSS
        .split("@media (max-width: 1099px) {")
        .nth(1)
        .expect("a regra do tablet desapareceu");
    let tablet = &tablet[..tablet.find("\n}").unwrap()];
    assert!(tablet
        .contains(r#".oc-win:not([data-state="minimized"]) { left: 8px; top: 8px; right: 8px;"#));
    assert!(tablet
        .contains(r#".oc-win__rz, .oc-snap, .oc-win [data-part="win-max"] { display: none; }"#));
    // E o motor só arrasta a partir de 1100px (oc-wm.js · free()).
    assert!(include_str!("../static/oc-wm.js").contains("(min-width: 1100px)"));
}

#[test]
fn ate_640px_ha_uma_aplicacao_activa_em_ecra_cheio() {
    // 17 · Uma superfície de cada vez, «Voltar ao Desktop» e o alternador; o
    // estado das janelas é o mesmo, só a apresentação muda. (Desde a D003 não
    // há prateleira em nenhuma largura.)
    let mobile = WM_CSS
        .split("@media (max-width: 640px) {")
        .nth(1)
        .expect("a regra do telemóvel desapareceu");
    let mobile = &mobile[..mobile.find("\n}").unwrap()];
    assert!(mobile.contains(".oc-win:not([data-active]) { display: none !important; }"));
    assert!(mobile.contains(".oc-win[data-active] { inset: 0;"));
    assert!(mobile.contains(
        ".oc-win__back, .oc-win__switch { display: inline-flex; width: 44px; height: 44px; }"
    ));
    assert!(!WM_CSS.contains(".oc-shelf"), "a prateleira voltou ao CSS");
}

#[test]
fn o_movimento_reduzido_tira_todas_as_animacoes_das_janelas() {
    let reduzido = WM_CSS
        .split("@media (prefers-reduced-motion: reduce) {")
        .nth(1)
        .expect("a regra de movimento reduzido desapareceu");
    assert!(reduzido.contains("animation: none") && reduzido.contains("transition: none"));
}
