//! As viagens da D008 contra um Core real: o Ocinye Terminal (ocsh) e o
//! Ocinye Browser no runtime Web.
//!
//! O Terminal: a linha vai inteira ao Core, que faz o parse que conta; a
//! sintaxe e as palavras do anfitrião são recusadas antes de haver comando; um
//! comando desconhecido é 127 e nunca chega à Nye; o eco é redigido; `|` é
//! composição tipada; a Nye só pela ponte explícita.
//!
//! O Browser: uma moldura isolada, só `https:`, só na resposta de `/browser`;
//! esquemas perigosos bloqueados antes de qualquer navegação.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida — e diz que saltou;
//! em CI, falha.

mod common;

use common::*;
use ocinye_contracts::TechnicalRole;
use serde_json::{json, Value};

fn pt(key: &str) -> &'static str {
    ocinye_workspace::i18n::t_in(ocinye_contracts::Locale::Pt, key)
}

/// Uma linha ao Terminal, como o `oc-terminal.js` a envia.
async fn linha(s: &Sistema, cookie: &str, line: &str) -> Value {
    let r = s
        .escrever(reqwest::Method::POST, "/terminal/exec", cookie)
        .header("accept", "application/json")
        .json(&json!({ "line": line, "context": null }))
        .send()
        .await
        .expect("POST /terminal/exec");
    assert_eq!(r.status().as_u16(), 200, "{line}");
    r.json().await.expect("json")
}

fn kinds_of(v: &Value) -> Vec<String> {
    v["blocks"]
        .as_array()
        .map(|b| {
            b.iter()
                .filter_map(|x| x["kind"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Se alguma moldura tem endereço. A do `<template>` do Design é inerte: o
/// `oc-browser.js` só a usa com um endereço que ele próprio validou.
fn moldura_com_endereco(html: &str) -> bool {
    html.split("<iframe")
        .skip(1)
        .any(|t| t.split('>').next().unwrap_or_default().contains(" src="))
}

fn csp(r: &reqwest::Response) -> String {
    r.headers()
        .get("content-security-policy")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

// ═════════════════════════════════════════════════════════════════════════
// Terminal
// ═════════════════════════════════════════════════════════════════════════

/// O ecrã é o do Design, com o cliente do Terminal e só ele; a descoberta é a
/// ajuda do Core para esta pessoa.
#[tokio::test]
async fn o_terminal_e_real_e_descobre_so_o_que_o_core_mostra() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (status, html) = s.html("/terminal", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"data-oc="term""#), "sem o ecrã do Terminal");
    assert!(!html.contains("oc-pending"), "ainda app_pending");
    assert!(html.contains(r#"<script src="/static/oc-terminal.js" defer>"#));
    assert!(
        !html.contains("oc-browser.js"),
        "o cliente do Browser no Terminal"
    );
    assert!(html.contains(pt("term.statement.2")), "a frase canónica");
    assert!(html.contains(r#"data-core="online""#));

    // A descoberta é a ajuda do Core: os mesmos comandos, nem mais um.
    let help = linha(&s, &c, "help").await;
    let mut do_core: Vec<String> = help["blocks"][0]["entries"]
        .as_array()
        .expect("ajuda")
        .iter()
        .filter_map(|e| e["usage"].as_str())
        .map(|u| {
            u.split_whitespace()
                .take_while(|w| !w.starts_with(['<', '[']))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let registo = html
        .split(r#"data-part="term-registry""#)
        .nth(1)
        .and_then(|r| r.split("</template>").next())
        .expect("registo");
    let mut no_ecra: Vec<String> = registo
        .split("data-cmd=\"")
        .skip(1)
        .filter_map(|x| x.split('"').next())
        .map(str::to_owned)
        .collect();
    do_core.sort();
    no_ecra.sort();
    assert_eq!(no_ecra, do_core, "a descoberta diverge da ajuda do Core");
    assert!(no_ecra.iter().any(|c| c == "whoami"));
    // Os grupos da ajuda vêm localizados, na forma do Design (T-05).
    assert_eq!(
        help["blocks"][0]["entries"][0]["group"],
        pt("ocsh.group.shell")
    );

    // Uma janela de fundo não recebe corpo: o cliente só existe na rota.
    let r = s.get("/terminal?frame=1", &c).send().await.expect("GET");
    assert_eq!(r.status().as_u16(), 204);
}

/// Um comando executa pela capability do Core, e a resposta traz o eco
/// redigido e os blocos que o Design desenha.
#[tokio::test]
async fn um_comando_corre_pela_capability_e_o_eco_e_redigido() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let v = linha(&s, &c, "whoami").await;
    assert_eq!(v["exit"], 0, "{v}");
    assert_eq!(v["capability"], "identity.self.read");
    assert_eq!(v["echo"], "whoami");
    assert_eq!(kinds_of(&v), ["facts"]);
    assert_eq!(v["context"]["label"], pt("terminal.context.personal"));

    // Um segredo escrito na linha nunca volta no eco, nem no percurso sem JS.
    for (l, segredo) in [
        ("whoami --password hunter2-secret", "hunter2-secret"),
        ("whoami --token=tk-9f8e7d", "tk-9f8e7d"),
        ("provider add --key sk-live-abc", "sk-live-abc"),
    ] {
        let v = linha(&s, &c, l).await;
        let texto = v.to_string();
        assert!(!texto.contains(segredo), "{l} → {texto}");
        assert!(v["echo"].as_str().unwrap().contains("••••"), "{l}");
        let r = s
            .escrever(reqwest::Method::POST, "/terminal/exec", &c)
            .form(&[("line", l)])
            .send()
            .await
            .expect("POST form");
        assert_eq!(r.status().as_u16(), 200);
        let html = r.text().await.unwrap();
        assert!(!html.contains(segredo), "sem JS: {l}");
        assert!(html.contains("oc-term-entry"), "sem JS, sem entrada");
    }
    // Sem JavaScript, o comando corre e desenha-se no servidor.
    let r = s
        .escrever(reqwest::Method::POST, "/terminal/exec", &c)
        .form(&[("line", "whoami")])
        .send()
        .await
        .expect("POST form");
    let html = r.text().await.unwrap();
    assert!(html.contains(r#"data-exit="0""#), "whoami sem JS");
    assert!(html.contains("identity.self.read"));

    // Sem sessão, nada vai ao Core; de outra origem, nada é aceite.
    let r = s
        .http
        .post(format!("{}/terminal/exec", s.url))
        .header("origin", &s.url)
        .json(&json!({ "line": "whoami" }))
        .send()
        .await
        .expect("POST");
    assert_eq!(r.status().as_u16(), 401);
    let r = s
        .http
        .post(format!("{}/terminal/exec", s.url))
        .header("origin", "https://evil.example")
        .header("cookie", format!("oc_boot=1; {c}"))
        .json(&json!({ "line": "whoami" }))
        .send()
        .await
        .expect("POST");
    assert_eq!(r.status().as_u16(), 403, "escrita de outra origem aceite");
}

/// A matriz de fuga para a shell do anfitrião: nada disto chega a uma
/// capability, e o código diz porquê.
#[tokio::test]
async fn nada_chega_a_shell_do_anfitriao() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    // Palavras do anfitrião e sintaxe da shell: 126, antes de haver comando.
    for l in [
        "bash",
        "sh -c id",
        "zsh",
        "sudo whoami",
        "su root",
        "ssh host",
        "exec id",
        "eval whoami",
        "whoami; id",
        "whoami && id",
        "whoami || id",
        "whoami & id",
        "whoami > /tmp/x",
        "whoami < /etc/passwd",
        "whoami `id`",
        "whoami $(id)",
        "whoami ${HOME}",
        "whoami $HOME",
    ] {
        let v = linha(&s, &c, l).await;
        assert_eq!(v["exit"], 126, "{l}: {v}");
        assert!(v["capability"].is_null(), "{l} chegou a uma capability");
        assert_eq!(v["blocks"][0]["tone"], "deny", "{l}");
    }
    // Ferramentas do anfitrião: comando desconhecido (127), sem capability.
    for l in [
        "ls -la",
        "cat /etc/passwd",
        "rm -rf /",
        "docker ps",
        "psql",
        "curl https://example.org",
        "env",
        "/bin/sh",
        "../../etc/passwd",
    ] {
        let v = linha(&s, &c, l).await;
        assert_eq!(v["exit"], 127, "{l}: {v}");
        assert!(v["capability"].is_null(), "{l}");
    }
    // `|` não é um pipe da shell: uma etapa que não é do registo tipado é
    // erro de uso, e nenhum programa corre.
    for l in [
        "whoami | sh",
        "context list | bash",
        "context list | grep x",
        "context list | xargs rm",
    ] {
        let v = linha(&s, &c, l).await;
        assert_ne!(v["exit"], 0, "{l}");
        assert!(v["capability"].is_null(), "{l}");
    }
}

/// Um comando desconhecido é 127, nunca vai para a Nye, e só sugere comandos
/// que existem (T-08).
#[tokio::test]
async fn desconhecido_e_127_e_so_sugere_o_que_existe() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let v = linha(&s, &c, "resume este projecto por favor").await;
    assert_eq!(v["exit"], 127);
    assert!(!kinds_of(&v).contains(&"nye".to_owned()), "foi à Nye: {v}");
    let nota = &v["blocks"][0];
    assert!(nota["detail"]
        .as_str()
        .unwrap()
        .contains(pt("term.not_found.body")));

    // `ls` explica o modelo e não sugere `files ls`, que não existe.
    let v = linha(&s, &c, "ls").await;
    assert_eq!(v["blocks"][0]["suggestions"], json!([]), "{v}");
    assert!(!v.to_string().contains("files ls"));
    assert_eq!(v["blocks"][0]["detail"], pt("ocsh.err.posix"));
    // `cls` sugere `clear`, que existe; um erro de dedo sugere a família.
    let v = linha(&s, &c, "cls").await;
    assert_eq!(v["blocks"][0]["suggestions"], json!(["clear"]));
    let v = linha(&s, &c, "whoamj").await;
    assert_eq!(v["blocks"][0]["suggestions"], json!(["whoami"]));
    // Uma família que a ajuda esconde a esta pessoa não se sugere, e escrita
    // à mão é recusada pelo Core (um colaborador não tem Computação).
    let (_, colab) = s.membro_com_sessao(&[TechnicalRole::Collaborator]).await;
    let help = linha(&s, &colab, "help").await.to_string();
    assert!(
        !help.contains("nodes list"),
        "a ajuda mostra nodes a quem não pode"
    );
    let v = linha(&s, &colab, "node").await;
    assert_eq!(v["blocks"][0]["suggestions"], json!([]), "{v}");
    let v = linha(&s, &colab, "nodes list").await;
    assert_eq!(v["exit"], 77, "escondido e executado: {v}");
    // Quem vê a família recebe a sugestão.
    assert!(linha(&s, &c, "help")
        .await
        .to_string()
        .contains("nodes list"));
    let v = linha(&s, &c, "node").await;
    assert_eq!(v["blocks"][0]["suggestions"], json!(["nodes"]), "{v}");
}

/// `|` compõe dados já autorizados, pelo registo fechado de etapas.
#[tokio::test]
async fn a_composicao_tipada_filtra_ordena_corta_e_conta() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let v = linha(&s, &c, "context list | count").await;
    assert_eq!(v["exit"], 0, "{v}");
    assert_eq!(kinds_of(&v), ["facts"]);
    let v = linha(&s, &c, "context list | sort title --desc | head 1").await;
    assert_eq!(v["exit"], 0, "{v}");
    assert_eq!(kinds_of(&v), ["table"]);
    assert_eq!(v["blocks"][0]["columns"][0]["id"], "code");
    let v = linha(&s, &c, "context list | export json").await;
    assert_eq!(kinds_of(&v), ["json"]);
    let v = linha(&s, &c, "context list | sort nao_existe").await;
    assert_eq!(v["exit"], 2);
}

/// A Nye só pela ponte explícita, pelo caminho canónico, e sem inferência é
/// «indisponível» com o motivo tipado — nunca uma resposta inventada.
#[tokio::test]
async fn a_nye_so_pela_ponte_explicita() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    for l in ["nye ask o que tenho hoje", "? o que tenho hoje"] {
        let v = linha(&s, &c, l).await;
        assert_eq!(v["exit"], 69, "{l}: {v}");
        assert_eq!(v["blocks"][0]["kind"], "note");
        assert_eq!(v["blocks"][0]["text"], pt("nye.reason.no_inference"), "{v}");
        assert!(v["capability"].is_null());
    }
    // Um contexto que a pessoa não alcança volta ao pessoal, e diz-se.
    let r = s
        .escrever(reqwest::Method::POST, "/terminal/exec", &c)
        .json(&json!({ "line": "whoami", "context": uuid::Uuid::new_v4().to_string() }))
        .send()
        .await
        .expect("POST");
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["exit"], 77, "{v}");
    assert_eq!(v["blocks"][0]["text"], pt("ocsh.err.context_unreachable"));
}

// ═════════════════════════════════════════════════════════════════════════
// Browser
// ═════════════════════════════════════════════════════════════════════════

/// A Nova aba, o cliente do Browser e só ele, e a política de molduras só
/// nesta resposta.
#[tokio::test]
async fn o_browser_e_real_e_so_ele_aceita_molduras() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::Collaborator]).await;
    let r = s.get("/browser", &c).send().await.expect("GET");
    assert_eq!(r.status().as_u16(), 200);
    let politica = csp(&r);
    assert!(politica.contains("frame-src 'self' https:;"), "{politica}");
    let html = r.text().await.unwrap();
    assert!(html.contains(r#"data-oc="brw""#) && !html.contains("oc-pending"));
    assert!(html.contains(r#"<script src="/static/oc-browser.js" defer>"#));
    assert!(
        !html.contains("oc-terminal.js"),
        "o cliente do Terminal no Browser"
    );
    assert!(html.contains(pt("brw.new.title")), "sem a Nova aba");
    assert!(!moldura_com_endereco(&html), "moldura sem endereço pedido");
    // Todas as outras respostas continuam só com molduras da própria origem.
    for rota in ["/terminal", "/", "/notes"] {
        let r = s.get(rota, &c).send().await.expect("GET");
        let p = csp(&r);
        assert!(p.contains("frame-src 'self';"), "{rota}: {p}");
        assert!(!p.contains("https:"), "{rota}: {p}");
    }
    let r = s.get("/browser?frame=1", &c).send().await.expect("GET");
    assert_eq!(r.status().as_u16(), 204);
}

/// Um endereço `https:` abre numa moldura isolada; o recurso honesto está lá.
#[tokio::test]
async fn um_endereco_abre_numa_moldura_isolada() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (status, html) = s
        .html("/browser?url=https%3A%2F%2Fexample.org%2Fa", &c)
        .await;
    assert_eq!(status, 200);
    let moldura = html
        .split("<iframe")
        .find(|t| t.split('>').next().unwrap_or_default().contains(" src="))
        .and_then(|x| x.split('>').next())
        .expect("moldura");
    assert!(
        moldura.contains(r#"src="https://example.org/a""#),
        "{moldura}"
    );
    assert!(
        moldura.contains(&format!(
            r#"sandbox="{}""#,
            ocinye_workspace::ui::apps::browser::WEB_SANDBOX
        )),
        "{moldura}"
    );
    for proibido in ["allow-same-origin", "allow-top-navigation", "allow-modals"] {
        assert!(!html.contains(proibido), "{proibido}");
    }
    assert!(moldura.contains(r#"referrerpolicy="no-referrer""#) && moldura.contains(r#"allow="""#));
    assert!(html.contains(r#"data-part="brw-edge""#), "sem a fronteira");
    assert!(
        html.contains(r#"rel="noopener noreferrer""#),
        "sem o recurso honesto"
    );
    assert!(html.contains(pt("brw.requested")));
    // Nada do Ocinye dentro da vista externa.
    let vista = html
        .split(r#"class="oc-brw-view""#)
        .nth(1)
        .and_then(|x| x.split("</div>").next())
        .unwrap();
    assert!(!vista.contains("data-oc="), "controlo do Ocinye na vista");
}

/// Esquemas perigosos e texto que não é endereço: nenhuma navegação, e o que
/// foi escrito volta escapado.
#[tokio::test]
async fn esquemas_perigosos_nao_navegam() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    for (url, esquema) in [
        ("javascript%3Aalert(1)", "javascript:"),
        (
            "data%3Atext%2Fhtml%2C%3Cscript%3Ealert(1)%3C%2Fscript%3E",
            "data:",
        ),
        ("file%3A%2F%2F%2Fetc%2Fpasswd", "file:"),
        ("blob%3Ahttps%3A%2F%2Fexample.org%2Fx", "blob:"),
    ] {
        let (status, html) = s.html(&format!("/browser?url={url}"), &c).await;
        assert_eq!(status, 200, "{url}");
        assert!(!moldura_com_endereco(&html), "{url} abriu uma moldura");
        assert!(!html.contains(&format!(r#"src="{esquema}"#)), "{url}");
        assert!(!html.contains(&format!(r#"href="{esquema}"#)), "{url}");
        assert!(!html.contains("<script>alert"), "{url} não escapado");
        assert!(
            html.contains(&pt("brw.scheme.title").replace("{scheme}", esquema)),
            "{url} sem o estado bloqueado"
        );
    }
    // Texto que não é endereço não vai a um motor de pesquisa.
    let (_, html) = s
        .html(
            "/browser?url=%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E%20tempo",
            &c,
        )
        .await;
    assert!(html.contains(pt("brw.invalid.title")));
    assert!(!html.contains("<img src=x"), "texto escrito não escapado");
    // `http:` não entra numa moldura: abre-se fora, pelo recurso honesto.
    let (_, html) = s.html("/browser?url=http%3A%2F%2Fexample.org%2F", &c).await;
    assert!(!moldura_com_endereco(&html), "moldura http:");
    assert!(html.contains(r#"href="http://example.org/""#) && html.contains("noopener noreferrer"));
}
