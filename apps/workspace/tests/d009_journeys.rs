//! As viagens da D009 contra um Core real, em Instâncias **novas**: uma base
//! nova por Distribuição, criada pelo caminho de produto
//! (`Sistema::provisionar`, o que o `bootstrap-admin --profile` faz).
//!
//! A Distribuição define o ponto de partida; o Core decide o que o membro
//! pode fazer. Aqui prova-se, por Distribuição: o distintivo à porta e na
//! barra, o fundo, os widgets e as fixações por omissão (e a sua ordem), o
//! lançador filtrado pela autoridade, a activação inalterada, o painel dos
//! primeiros passos, a ausência de dados de exemplo, a proveniência e o
//! «Repor predefinição». E, à parte, que uma fixação sem autorização cai e
//! que um widget escondido sobrevive a uma gravação.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida — e diz que saltou;
//! em CI, falha.

mod common;

use common::*;
use ocinye_contracts::{ApplicationId, InstanceProfile, TechnicalRole};
use serde_json::{json, Value};
use uuid::Uuid;

fn pt(key: &str) -> &'static str {
    ocinye_workspace::i18n::t_in(ocinye_contracts::Locale::Pt, key)
}

/// As fixações desenhadas na barra, pela ordem da barra (antes das que só
/// estão em execução).
fn fixacoes(html: &str) -> Vec<String> {
    let nav = html
        .split(r#"data-oc="dock""#)
        .nth(1)
        .and_then(|x| x.split("</nav>").next())
        .expect("barra de aplicações");
    let fixadas = nav
        .split(r#"data-part="dock-running""#)
        .next()
        .unwrap_or(nav);
    fixadas
        .split("data-app=\"")
        .skip(1)
        .filter_map(|x| x.split('"').next())
        .map(str::to_owned)
        .collect()
}

/// As rotas das fichas do lançador (o Gestor de Aplicações).
fn lancador(html: &str) -> Vec<String> {
    html.split(r#"data-part="launcher-item""#)
        .skip(1)
        .filter_map(|x| x.split("href=\"").nth(1))
        .filter_map(|x| x.split('"').next())
        .map(str::to_owned)
        .collect()
}

/// Os widgets do Desktop: `(id, escondido)`, pela ordem da disposição.
fn widgets(html: &str) -> Vec<(String, bool)> {
    html.split(r#"data-part="desk-widget""#)
        .skip(1)
        .map(|x| {
            let tag = x.split('>').next().unwrap_or_default();
            let id = tag
                .split("data-id=\"")
                .nth(1)
                .and_then(|y| y.split('"').next())
                .unwrap_or_default()
                .to_owned();
            (id, tag.contains("data-withheld"))
        })
        .collect()
}

fn atributo<'a>(html: &'a str, depois: &str, nome: &str) -> &'a str {
    html.split(depois)
        .nth(1)
        .and_then(|x| x.split(&format!("{nome}=\"")).nth(1))
        .and_then(|x| x.split('"').next())
        .unwrap_or_default()
}

/// O corpo que o `oc-desk.js` envia: todos os widgets da grelha, escondidos
/// incluídos, com a versão da página.
fn corpo_do_cliente(html: &str, minimizar: Option<&str>) -> Value {
    let versao: i64 = atributo(html, r#"data-oc="desk""#, "data-version")
        .parse()
        .unwrap_or(0);
    let ws: Vec<Value> = html
        .split(r#"data-part="desk-widget""#)
        .skip(1)
        .map(|x| {
            let tag = x.split('>').next().unwrap_or_default();
            let a = |n: &str| {
                tag.split(&format!("{n}=\""))
                    .nth(1)
                    .and_then(|y| y.split('"').next())
                    .unwrap_or_default()
                    .to_owned()
            };
            let id = a("data-id");
            json!({
                "id": id,
                "kind": a("data-kind"),
                "w": a("data-w").parse::<u8>().unwrap_or(1),
                "h": a("data-h").parse::<u8>().unwrap_or(1),
                "minimized": tag.contains("data-min") || minimizar == Some(id.as_str()),
            })
        })
        .collect();
    json!({
        "version": versao,
        "fit": "fill",
        "wallpaper": atributo(html, "<body", "data-wall"),
        "dim": atributo(html, "<body", "data-dim").parse::<u8>().unwrap_or(20),
        "widgets": ws,
    })
}

async fn gravar(s: &Sistema, c: &str, corpo: &Value) -> u16 {
    s.escrever(reqwest::Method::PUT, "/me/desktop", c)
        .header("accept", "application/json")
        .json(corpo)
        .send()
        .await
        .expect("PUT /me/desktop")
        .status()
        .as_u16()
}

async fn token(s: &Sistema, email: &str, password: &str) -> String {
    s.http
        .post(format!("{}/api/v1/auth/login", s.core_url))
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("login no Core")
        .json::<Value>()
        .await
        .expect("sessão")["session_token"]
        .as_str()
        .expect("token")
        .to_owned()
}

async fn com_mfa(s: &Sistema, roles: &[TechnicalRole]) -> String {
    let (id, email, password) = s.pessoa(roles).await;
    s.totp_confirmado(id).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    let r = s
        .escrever(reqwest::Method::POST, "/mfa/challenge", &c)
        .form(&[("code", codigo_totp(SEMENTE_MFA).as_str())])
        .send()
        .await
        .expect("desafio");
    r.headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with(&format!("{}=", ocinye_workspace::session::COOKIE_NAME)))
        .map(|c| c.split(';').next().unwrap_or_default().to_owned())
        .expect("sessão depois do MFA")
}

/// O que a Design fixou para cada Distribuição (docs/ui/distribution-defaults.md).
struct Esperado {
    perfil: InstanceProfile,
    fundo: &'static str,
    widgets: &'static [&'static str],
    fixacoes: &'static [&'static str],
    icone: &'static str,
}

const ESPERADO: [Esperado; 4] = [
    Esperado {
        perfil: InstanceProfile::Research,
        fundo: "field",
        widgets: &["kpis", "projects"],
        fixacoes: &[
            "work",
            "projects",
            "ideas",
            "datasets",
            "results",
            "knowledge",
            "files",
            "notes",
        ],
        icone: "dist-research",
    },
    Esperado {
        perfil: InstanceProfile::Business,
        fundo: "module",
        widgets: &["tasks", "calendar", "projects", "files"],
        fixacoes: &[
            "work", "calendar", "mail", "messages", "projects", "files", "notes",
        ],
        icone: "dist-business",
    },
    Esperado {
        perfil: InstanceProfile::Personal,
        fundo: "calm",
        widgets: &["notes", "files", "calendar", "storage"],
        fixacoes: &["files", "notes", "calendar", "work", "resources", "trash"],
        icone: "dist-personal",
    },
    Esperado {
        perfil: InstanceProfile::Education,
        fundo: "lattice",
        widgets: &["calendar", "tasks", "projects", "notes"],
        fixacoes: &[
            "work",
            "units",
            "projects",
            "knowledge",
            "bibliography",
            "calendar",
            "files",
            "notes",
        ],
        icone: "dist-education",
    },
];

/// Uma Instância nova numa Distribuição, com um membro comum e um
/// administrador da plataforma (com o segundo factor).
async fn instancia_nova(e: &Esperado) {
    let Some(s) = Sistema::provisionar(e.perfil).await else {
        return;
    };
    let p = e.perfil.as_str();

    // 1 · A Instância nasceu com a Distribuição, e sem dados de exemplo.
    let gravado: String = sqlx::query_scalar("SELECT profile FROM organisations WHERE id = $1")
        .bind(s.organisation_id)
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(gravado, p);
    for tabela in [
        "research_workspaces",
        "ideas",
        "tasks",
        "notes",
        "files",
        "calendar_events",
        "datasets",
        "results",
        "projects",
        "messages",
    ] {
        let existe: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(tabela)
            .fetch_one(&s.pool)
            .await
            .unwrap();
        if existe {
            let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {tabela}"))
                .fetch_one(&s.pool)
                .await
                .unwrap();
            assert_eq!(n, 0, "{p}: {tabela} tem dados de exemplo");
        }
    }
    let unidades: i64 = sqlx::query_scalar("SELECT count(*) FROM units WHERE organisation_id = $1")
        .bind(s.organisation_id)
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(
        unidades > 0,
        e.perfil == InstanceProfile::Research,
        "{p}: unidades iniciais só em Research"
    );

    // 2 · À porta: o ícone da Distribuição, sem as duas letras.
    let r = s
        .http
        .get(format!("{}/login", s.url))
        .header("cookie", "oc_boot=1")
        .send()
        .await
        .unwrap();
    let porta = r.text().await.unwrap();
    assert!(
        porta.contains(&format!(r#"data-distribution="{p}""#)),
        "{p}: porta"
    );
    let codigo = porta
        .split("oc-auth__dist-code")
        .nth(1)
        .and_then(|x| x.split("</span>").next())
        .expect("distintivo à porta");
    assert!(
        codigo.contains(&format!("#{}", e.icone)),
        "{p}: ícone à porta"
    );
    assert!(
        !codigo.contains(">Re<") && !codigo.contains(">Bu<"),
        "{p}: letras à porta"
    );

    // 3 · Um membro comum entra.
    let (_, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    let (status, html) = s.html("/", &c).await;
    assert_eq!(status, 200, "{p}: Desktop");

    // A barra de cima: o ícone, com nome acessível, e o painel a pedido.
    let distintivo = html
        .split(r#"data-distribution=""#)
        .find(|x| x.starts_with(p) && x.contains("</summary>"))
        .unwrap_or_else(|| {
            let i = html.find("oc-dist").unwrap_or(0);
            panic!(
                "distintivo na barra: {}",
                &html[i..(i + 400).min(html.len())]
            )
        });
    assert!(distintivo.starts_with(p), "{p}: distintivo");
    let sumario = distintivo.split("</summary>").next().unwrap();
    assert!(
        sumario.contains(&format!("#{}", e.icone)),
        "{p}: ícone na barra"
    );
    assert!(
        sumario.contains(pt(&format!("dist.{p}"))),
        "{p}: o distintivo sem nome acessível"
    );
    let painel = distintivo.split("</details>").next().unwrap();
    assert!(
        painel.contains(pt(&format!("dist.first.{p}.title"))),
        "{p}: primeiros passos"
    );
    assert!(
        painel.contains(pt("dist.authority")),
        "{p}: a frase de autoridade"
    );
    assert!(
        !html.contains("<details class=\"oc-menu\" data-oc=\"menu\" open"),
        "{p}: o painel abriu sozinho"
    );

    // O fundo e os widgets da Distribuição, todos visíveis a este membro.
    assert_eq!(atributo(&html, "<body", "data-wall"), e.fundo, "{p}: fundo");
    let ws = widgets(&html);
    assert_eq!(
        ws.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        e.widgets,
        "{p}: widgets por omissão"
    );
    assert!(
        ws.iter().all(|(_, escondido)| !escondido),
        "{p}: widget escondido"
    );
    assert!(
        !html.contains(r#"data-kind="notice""#),
        "{p}: avisos por omissão"
    );

    // As fixações, pela ordem da Distribuição.
    assert_eq!(fixacoes(&html), e.fixacoes, "{p}: fixações e ordem");

    // O lançador: sem aplicações de autoridade para um membro comum.
    let apps = lancador(&html);
    for privada in ["/admin", "/audit", "/admin/monitor"] {
        assert!(
            !apps.iter().any(|a| a == privada),
            "{p}: {privada} no lançador"
        );
    }

    // A activação é a do perfil (ADR-0014), a mesma de antes da D009.
    let t = token(&s, &email, &password).await;
    let me: Value = s
        .http
        .get(format!("{}/api/v1/me", s.core_url))
        .bearer_auth(&t)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut inactivas: Vec<String> = me["inactive_applications"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x.as_str().map(str::to_owned))
        .collect();
    inactivas.sort();
    let mut do_perfil: Vec<String> = ApplicationId::ALL
        .into_iter()
        .filter(|a| !e.perfil.activates(*a))
        .map(|a| a.as_str().to_owned())
        .collect();
    do_perfil.sort();
    assert_eq!(inactivas, do_perfil, "{p}: activação");

    // A proveniência: a predefinição da Distribuição, versionada.
    assert!(
        html.contains(r#"data-source="distribution""#),
        "{p}: proveniência"
    );

    // 4 · Personalizar ganha à Distribuição; Repor volta a ela.
    let corpo = json!({
        "version": 0, "fit": "fill", "wallpaper": "dusk", "dim": 30,
        "widgets": [{ "id": "storage", "kind": "storage", "w": 1, "h": 1, "minimized": false }],
    });
    assert_eq!(gravar(&s, &c, &corpo).await, 200, "{p}: gravar");
    let (_, html) = s.html("/", &c).await;
    assert_eq!(atributo(&html, "<body", "data-wall"), "dusk");
    assert_eq!(widgets(&html).len(), 1, "{p}: a disposição do membro");
    let r = s
        .escrever(reqwest::Method::POST, "/me/desktop/restore", &c)
        .header("accept", "application/json")
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{p}: repor");
    let (_, html) = s.html("/", &c).await;
    assert_eq!(
        atributo(&html, "<body", "data-wall"),
        e.fundo,
        "{p}: fundo reposto"
    );
    assert_eq!(
        widgets(&html)
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>(),
        e.widgets,
        "{p}: repor volta à Distribuição (não há predefinição da Instância)"
    );

    // 5 · Um administrador da plataforma vê as aplicações de autoridade — pela
    //     autoridade, e nenhuma fica fixada por omissão.
    let admin = com_mfa(
        &s,
        &[TechnicalRole::PlatformAdmin, TechnicalRole::ResearchMember],
    )
    .await;
    let (_, html) = s.html("/", &admin).await;
    let apps = lancador(&html);
    for privada in ["/admin", "/admin/monitor"] {
        assert!(
            apps.iter().any(|a| a == privada),
            "{p}: {privada} sem autoridade"
        );
    }
    for nunca in [
        "administration",
        "audit",
        "monitor",
        "terminal",
        "browser",
        "prompt",
    ] {
        assert!(
            !fixacoes(&html).iter().any(|a| a == nunca),
            "{p}: {nunca} fixado"
        );
    }
    s.descartar().await;
}

#[tokio::test]
async fn uma_instancia_research_nova() {
    instancia_nova(&ESPERADO[0]).await;
}

#[tokio::test]
async fn uma_instancia_business_nova() {
    instancia_nova(&ESPERADO[1]).await;
}

#[tokio::test]
async fn uma_instancia_personal_nova() {
    instancia_nova(&ESPERADO[2]).await;
}

#[tokio::test]
async fn uma_instancia_education_nova() {
    instancia_nova(&ESPERADO[3]).await;
}

/// Uma fixação da Distribuição a que o membro não chega sai da barra (a
/// seguinte ocupa o lugar), a rota continua recusada, e a predefinição não
/// muda; um widget sem autorização esconde-se.
#[tokio::test]
async fn a_autoridade_filtra_fixacoes_e_widgets() {
    let Some(s) = Sistema::provisionar(InstanceProfile::Education).await else {
        return;
    };
    // Um colaborador: sem Unidades, Projectos nem Conhecimento (Ficheiros
    // pessoais, sim).
    let (id, email, password) = s.pessoa(&[TechnicalRole::Collaborator]).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    let (_, html) = s.html("/", &c).await;
    assert_eq!(fixacoes(&html), ["work", "calendar", "files", "notes"]);
    for rota in ["/projects", "/units", "/knowledge", "/bibliography"] {
        // Uma aplicação pode levar à sua secção por omissão: segue-se.
        let mut destino = rota.to_owned();
        for _ in 0..3 {
            let r = s.get(&destino, &c).send().await.expect("GET");
            if r.status().as_u16() != 303 {
                assert_eq!(
                    r.status().as_u16(),
                    404,
                    "{rota} aberto sem autoridade ({destino})"
                );
                break;
            }
            destino = location(&r);
            assert!(
                destino.starts_with('/') && destino != "/",
                "{rota} → {destino}"
            );
        }
    }
    let ws = widgets(&html);
    let projectos = ws
        .iter()
        .find(|(id, _)| id == "projects")
        .expect("na disposição");
    assert!(
        projectos.1,
        "o widget de Projectos desenhou-se sem autoridade"
    );
    assert!(
        !ws.iter().find(|(id, _)| id == "calendar").unwrap().1,
        "controlo positivo"
    );
    // G9-09 · Tarefas vive de O Meu Trabalho: sem Projectos, continua.
    assert!(
        !ws.iter()
            .find(|(id, _)| id == "tasks")
            .expect("na disposição")
            .1,
        "Tarefas escondeu-se por faltar Projectos"
    );
    // Nada foi gravado em nome do membro, e a predefinição é a mesma.
    let pins: i64 = sqlx::query_scalar("SELECT count(*) FROM member_app_pins WHERE person_id = $1")
        .bind(id)
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(pins, 0, "o filtro gravou-se como fixações do membro");
    assert_eq!(
        ocinye_workspace::experience::apps::default_pins_for(Some("education")),
        ESPERADO[3].fixacoes
    );
    s.descartar().await;
}

/// Um widget na disposição, a autoridade retirada: esconde-se; gravar outra
/// mudança não o apaga; a autoridade de volta, reaparece.
#[tokio::test]
async fn um_widget_escondido_sobrevive_a_gravacao() {
    let Some(s) = Sistema::provisionar(InstanceProfile::Research).await else {
        return;
    };
    let (id, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    // 1 · A disposição do membro tem Projectos e Notas.
    let corpo = json!({
        "version": 0, "fit": "fill", "wallpaper": "field", "dim": 20,
        "widgets": [
            { "id": "projects", "kind": "projects", "w": 2, "h": 1, "minimized": false },
            { "id": "notes", "kind": "notes", "w": 2, "h": 1, "minimized": false },
        ],
    });
    assert_eq!(gravar(&s, &c, &corpo).await, 200);
    // 2 · A autoridade sobre Projectos sai.
    sqlx::query("DELETE FROM person_roles WHERE person_id = $1")
        .bind(id)
        .execute(&s.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, 'collaborator')")
        .bind(id)
        .execute(&s.pool)
        .await
        .unwrap();
    let (_, _, c) = s.entrar(&email, &password).await;
    let (_, html) = s.html("/", &c).await;
    // 3 · Esconde-se, mas continua na página.
    let ws = widgets(&html);
    assert_eq!(
        ws,
        [("projects".to_owned(), true), ("notes".to_owned(), false)]
    );
    // 4 · O membro grava outra mudança, como o cliente a envia.
    assert_eq!(
        gravar(&s, &c, &corpo_do_cliente(&html, Some("notes"))).await,
        200
    );
    let gravada: Value =
        sqlx::query_scalar("SELECT layout FROM member_desktop_layouts WHERE person_id = $1")
            .bind(id)
            .fetch_one(&s.pool)
            .await
            .unwrap();
    let ids: Vec<&str> = gravada["widgets"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|w| w["id"].as_str())
        .collect();
    assert_eq!(
        ids,
        ["projects", "notes"],
        "a gravação apagou o widget escondido"
    );
    assert_eq!(
        gravada["widgets"][1]["minimized"], true,
        "a mudança não se gravou"
    );
    // 5 · A autoridade volta: o widget reaparece.
    sqlx::query("UPDATE person_roles SET role = 'research_member' WHERE person_id = $1")
        .bind(id)
        .execute(&s.pool)
        .await
        .unwrap();
    let (_, _, c) = s.entrar(&email, &password).await;
    let (_, html) = s.html("/", &c).await;
    assert_eq!(
        widgets(&html),
        [("projects".to_owned(), false), ("notes".to_owned(), false)]
    );
    s.descartar().await;
}

/// Sem inferência (o fornecedor por omissão é `NoProvider`): o Desktop, as
/// fixações e os primeiros passos funcionam; nada por omissão depende da IA
/// nem mostra GPU.
#[tokio::test]
async fn sem_ia_nem_gpu_o_ponto_de_partida_funciona() {
    let Some(s) = Sistema::provisionar(InstanceProfile::Research).await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (status, html) = s.html("/", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"data-kind="kpis""#) && html.contains(pt("dist.first.research.title")));
    for fora in [r#"data-kind="health""#, "GPU", r#"data-kind="compute""#] {
        assert!(
            !widgets(&html).iter().any(|(id, _)| id == "health"),
            "{fora}"
        );
    }
    let nodes: i64 = sqlx::query_scalar("SELECT count(*) FROM compute_nodes")
        .fetch_one(&s.pool)
        .await
        .unwrap_or(0);
    assert_eq!(nodes, 0);
    let _ = Uuid::nil();
    s.descartar().await;
}

/// G9-09 · Os Indicadores numa Business (sem Ideias nem Dados activas) mostram
/// Unidades e Projectos; não desaparecem por faltar uma aplicação, e não
/// oferecem ligação a uma que não abre.
#[tokio::test]
async fn os_indicadores_largam_so_as_metricas_que_o_membro_nao_ve() {
    let Some(s) = Sistema::provisionar(InstanceProfile::Business).await else {
        return;
    };
    let (_, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    let corpo = json!({
        "version": 0, "fit": "fill", "wallpaper": "module", "dim": 20,
        "widgets": [
            { "id": "kpis", "kind": "kpis", "w": 4, "h": 1, "minimized": false },
        ],
    });
    assert_eq!(gravar(&s, &c, &corpo).await, 200);
    let (_, html) = s.html("/", &c).await;
    assert_eq!(widgets(&html), [("kpis".to_owned(), false)]);
    let metricas: Vec<&str> = html
        .split(r#"<a href=""#)
        .skip(1)
        .filter(|x| x.split('>').next().unwrap_or_default().contains("oc-kpi\""))
        .filter_map(|x| x.split('"').next())
        .collect();
    assert_eq!(metricas, ["/units", "/projects"]);
    // Um colaborador (sem Unidades nem Projectos) não vê nenhuma: escondido,
    // e continua na disposição.
    let (_, email, password) = s.pessoa(&[TechnicalRole::Collaborator]).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    assert_eq!(gravar(&s, &c, &corpo).await, 200);
    let (_, html) = s.html("/", &c).await;
    assert_eq!(widgets(&html), [("kpis".to_owned(), true)]);
    s.descartar().await;
}

/// G9-09 · Um indicador de uma aplicação que o membro não abre não se desenha,
/// mesmo quando o Core devolve a contagem (a lista de ambientes de um
/// colaborador é autorizada; `/projects` não lhe abre). Nenhuma ligação morta.
#[tokio::test]
async fn um_indicador_sem_aplicacao_visivel_nao_se_desenha() {
    let Some(s) = Sistema::provisionar(InstanceProfile::Research).await else {
        return;
    };
    let (_, email, password) = s.pessoa(&[TechnicalRole::Collaborator]).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    let corpo = json!({
        "version": 0, "fit": "fill", "wallpaper": "field", "dim": 20,
        "widgets": [
            { "id": "kpis", "kind": "kpis", "w": 4, "h": 1, "minimized": false },
        ],
    });
    assert_eq!(gravar(&s, &c, &corpo).await, 200);
    let (_, html) = s.html("/", &c).await;
    let metricas: Vec<&str> = html
        .split(r#"<a href=""#)
        .skip(1)
        .filter(|x| x.split('>').next().unwrap_or_default().contains("oc-kpi\""))
        .filter_map(|x| x.split('"').next())
        .collect();
    // Um colaborador de Research não abre nenhuma das quatro: o widget
    // esconde-se, e fica na disposição.
    assert_eq!(widgets(&html), [("kpis".to_owned(), true)]);
    let lancador = lancador(&html);
    for m in &metricas {
        assert!(
            lancador.iter().any(|a| a == m),
            "o indicador {m} liga a uma aplicação que o membro não abre"
        );
    }
    s.descartar().await;
}
