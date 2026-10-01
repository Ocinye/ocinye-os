//! D010 · Acesso, várias Distribuições e superfície do sistema — os
//! percursos A–I do pacote (`D010_SCREEN_MATRIX.md`) e as fronteiras de
//! segurança dos pontos de acesso, por HTTP contra um Core real.
//!
//! Cada percurso começa no mesmo sítio que um browser: um `Host`, a porta, um
//! formulário. As Distribuições activadas e o acesso de cada membro escrevem-se
//! na base de teste (o ecrã da Administração que os muda tem as suas provas);
//! o que se prova aqui é o que o Workspace faz com eles.

mod common;

use common::*;
use ocinye_contracts::{Locale, TechnicalRole};
use ocinye_workspace::i18n::t_in;
use serde_json::Value;
use uuid::Uuid;

fn pt(key: &str, distribution: &str) -> String {
    t_in(Locale::Pt, key).replace(
        "{distribution}",
        t_in(Locale::Pt, &format!("dist.{distribution}")),
    )
}

/// Activa uma Distribuição na organização do sistema (como S26 faria).
async fn activar(s: &Sistema, d: &str) {
    sqlx::query(
        "INSERT INTO instance_distributions (organisation_id, distribution, state, enabled_at)
         VALUES ($1, $2, 'enabled', now())
         ON CONFLICT (organisation_id, distribution) DO UPDATE SET state = 'enabled', disabled_at = NULL",
    )
    .bind(s.organisation_id)
    .bind(d)
    .execute(&s.pool)
    .await
    .expect("activar");
}

async fn desactivar(s: &Sistema, d: &str) {
    sqlx::query(
        "UPDATE instance_distributions SET state = 'disabled', disabled_at = now()
          WHERE organisation_id = $1 AND distribution = $2",
    )
    .bind(s.organisation_id)
    .bind(d)
    .execute(&s.pool)
    .await
    .expect("desactivar");
}

async fn dar(s: &Sistema, person: Uuid, d: &str) {
    sqlx::query(
        "INSERT INTO member_distribution_access (organisation_id, person_id, distribution)
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(s.organisation_id)
    .bind(person)
    .bind(d)
    .execute(&s.pool)
    .await
    .expect("dar acesso");
}

async fn tirar(s: &Sistema, person: Uuid, d: &str) {
    sqlx::query(
        "DELETE FROM member_distribution_access WHERE person_id = $1 AND distribution = $2",
    )
    .bind(person)
    .bind(d)
    .execute(&s.pool)
    .await
    .expect("tirar acesso");
}

/// A Distribuição activa no distintivo da casca (o `<summary class="oc-dist">`).
fn distintivo(html: &str) -> Option<String> {
    html.split("<summary").skip(1).find_map(|tag| {
        let tag = tag.split('>').next()?;
        if !tag.contains(r#"class="oc-dist""#) {
            return None;
        }
        tag.split(r#"data-distribution=""#)
            .nth(1)?
            .split('"')
            .next()
            .map(str::to_owned)
    })
}

async fn janelas(s: &Sistema, cookie: &str) -> Value {
    s.get("/wm", cookie)
        .header("accept", "application/json")
        .send()
        .await
        .expect("GET /wm")
        .json()
        .await
        .expect("json")
}

fn ids(v: &Value) -> Vec<String> {
    v["windows"]
        .as_array()
        .map(|l| {
            l.iter()
                .filter_map(|w| w["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

// ── A · ponto genérico, uma acessível: entra directamente ────────────────

#[tokio::test]
async fn a_uma_acessivel_entra_directamente() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (status, html) = s.html("/", &c).await;
    assert_eq!(status, 200);
    assert_eq!(distintivo(&html).as_deref(), Some("research"));
    assert!(!html.contains(r#"data-part="dist-selector""#));
}

// ── B · ponto genérico, várias: escolhe (S09) ─────────────────────────────

#[tokio::test]
async fn b_varias_acessiveis_escolhe_e_entra() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    activar(&s, "business").await;
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    dar(&s, id, "research").await;
    dar(&s, id, "business").await;
    let (_, _, c) = s.entrar(&email, &pw).await;
    let r = s.get("/", &c).send().await.unwrap();
    assert_eq!(
        location(&r),
        "/distribution",
        "várias acessíveis: o seleccionador"
    );
    let (status, html) = s.html("/distribution", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"data-part="dist-selector""#));
    assert!(html.contains(r#"value="research""#) && html.contains(r#"value="business""#));
    // Só Distribuições: nem unidades, nem projectos, nem espaço pessoal.
    assert!(!html.contains(r#"value="personal""#) && !html.contains(r#"value="education""#));

    // Escolher uma que não é sua é recusado pelo Core.
    let r = s
        .escrever(reqwest::Method::POST, "/distribution", &c)
        .form(&[("distribution", "education")])
        .send()
        .await
        .unwrap();
    assert_ne!(location(&r), "/");
    let r = s
        .escrever(reqwest::Method::POST, "/distribution", &c)
        .form(&[("distribution", "business")])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/");
    let (_, html) = s.html("/", &c).await;
    assert_eq!(distintivo(&html).as_deref(), Some("business"));
}

// ── C · ponto fixo: entra nele ────────────────────────────────────────────

#[tokio::test]
async fn c_ponto_fixo_entra_na_sua_distribuicao() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    activar(&s, "business").await;
    let fixo = s.ponto(Some("business")).await;
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    dar(&s, id, "research").await;
    dar(&s, id, "business").await;
    let (_, destino, c) = s.entrar_em(&fixo, &email, &pw).await;
    assert_eq!(destino, "/");
    let r = s.get_em(&fixo, "/", &c).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 200, "um ponto fixo não pergunta");
    let html = r.text().await.unwrap();
    assert_eq!(distintivo(&html).as_deref(), Some("business"));
    // Mudar localmente não se oferece nem se aceita (ADR-0625 §4).
    let html = s
        .get_em(&fixo, "/?switch=research", &c)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!html.contains(r#"data-part="dist-switch-confirm""#));
    let r = s
        .escrever_em(&fixo, reqwest::Method::POST, "/distribution/switch", &c)
        .form(&[("to", "research")])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/");
    let html = s
        .get_em(&fixo, "/", &c)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(distintivo(&html).as_deref(), Some("business"));
}

// ── D · ponto fixo sem acesso (S11) e fixo numa desactivada (S12) ─────────

#[tokio::test]
async fn d_ponto_fixo_sem_acesso_recusa_sem_alternativa() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    activar(&s, "business").await;
    let fixo = s.ponto(Some("business")).await;
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    dar(&s, id, "research").await;
    let (_, _, c) = s.entrar_em(&fixo, &email, &pw).await;
    let r = s.get_em(&fixo, "/", &c).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 403);
    let html = r.text().await.unwrap();
    assert!(html.contains(&pt("dist.bound.noaccess.title", "business")));
    // Nunca a cai para a outra Distribuição do membro.
    assert!(!html.contains(r#"data-part="desk""#));

    // S12: o ponto continua fixo, a Distribuição deixou de estar activada.
    desactivar(&s, "business").await;
    tokio::time::sleep(std::time::Duration::from_millis(5200)).await;
    let r = s.get_em(&fixo, "/", &c).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 403);
    assert!(r
        .text()
        .await
        .unwrap()
        .contains(&pt("dist.bound.disabled.title", "business")));
}

// ── E · anfitrião desconhecido (S13), e o cabeçalho forjado ───────────────

#[tokio::test]
async fn e_anfitriao_desconhecido_e_cabecalho_forjado() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let r = s
        .get_em("desconhecido.exemplo.test", "/login", "")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 404);
    let html = r.text().await.unwrap();
    assert!(html.contains(r#"data-part="host""#) && html.contains("desconhecido.exemplo.test"));
    assert!(
        !html.contains(r#"name="password""#),
        "S13 não oferece entrada"
    );

    // Sem proxy de confiança, `X-Forwarded-Host` não escolhe o anfitrião: um
    // desconhecido continua desconhecido, e um conhecido não se troca.
    let r = s
        .get_em("desconhecido.exemplo.test", "/login", "")
        .header("x-forwarded-host", &s.host)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        404,
        "X-Forwarded-Host forjado foi aceite"
    );
    let fixo = s.ponto(Some("research")).await;
    let r = s
        .get("/login", "")
        .header("x-forwarded-host", &fixo)
        .send()
        .await
        .unwrap();
    let html = r.text().await.unwrap();
    assert!(
        !html.contains(&fixo),
        "X-Forwarded-Host forjado escolheu o ponto"
    );
}

// ── F · mudar de Distribuição (S15/S16), com trabalho por guardar ─────────

#[tokio::test]
async fn f_mudar_pede_confirmacao_e_cancelar_janela_por_guardar_aborta() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    activar(&s, "business").await;
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    dar(&s, id, "research").await;
    dar(&s, id, "business").await;
    let (_, _, c) = s.entrar(&email, &pw).await;
    s.escrever(reqwest::Method::POST, "/distribution", &c)
        .form(&[("distribution", "research")])
        .send()
        .await
        .unwrap();

    // S16: a folha diz quantas janelas fecham.
    s.html("/notes", &c).await;
    let w = ids(&janelas(&s, &c).await)[0].clone();
    let (_, html) = s.html("/?switch=business", &c).await;
    assert!(html.contains(r#"data-part="dist-switch-confirm""#));
    assert!(html.contains(r#"data-part="dist-switch-windows""#));

    // Uma janela por guardar: a mudança passa pelo diálogo do D002, e
    // Cancelar aborta a mudança inteira.
    s.escrever(reqwest::Method::POST, &format!("/wm/{w}/state"), &c)
        .header("accept", "application/json")
        .form(&[("dirty", "true"), ("can_save", "false")])
        .send()
        .await
        .unwrap();
    let r = s
        .escrever(reqwest::Method::POST, "/distribution/switch", &c)
        .form(&[("to", "business")])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), format!("/?close={w}&after=switch:business"));
    let r = s
        .escrever(reqwest::Method::POST, &format!("/wm/{w}/close"), &c)
        .form(&[("decision", "cancel"), ("after", "switch:business")])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/?switched=aborted");
    let (_, html) = s.html("/", &c).await;
    assert_eq!(
        distintivo(&html).as_deref(),
        Some("research"),
        "cancelar mudou na mesma"
    );
    assert_eq!(
        ids(&janelas(&s, &c).await),
        vec![w.clone()],
        "cancelar fechou a janela"
    );

    // Sem nada por guardar: muda, e as janelas da anterior fecham.
    s.escrever(reqwest::Method::POST, &format!("/wm/{w}/state"), &c)
        .header("accept", "application/json")
        .form(&[("dirty", "false"), ("can_save", "false")])
        .send()
        .await
        .unwrap();
    let r = s
        .escrever(reqwest::Method::POST, "/distribution/switch", &c)
        .form(&[("to", "business"), ("return", "//evil.example/x")])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/", "o regresso aceitou um endereço de fora");
    let (_, html) = s.html("/", &c).await;
    assert_eq!(distintivo(&html).as_deref(), Some("business"));
    assert!(
        ids(&janelas(&s, &c).await).is_empty(),
        "as janelas de Research ficaram abertas"
    );
}

// ── G · contexto (S19), reposto ao mudar ─────────────────────────────────

#[tokio::test]
async fn g_contexto_escolhe_se_e_repoe_se_ao_mudar() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    activar(&s, "business").await;
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    dar(&s, id, "research").await;
    dar(&s, id, "business").await;
    let (_, _, c) = s.entrar(&email, &pw).await;
    s.escrever(reqwest::Method::POST, "/distribution", &c)
        .form(&[("distribution", "research")])
        .send()
        .await
        .unwrap();
    let (_, html) = s.html("/", &c).await;
    assert!(html.contains(r#"data-part="ctx-switcher""#));
    s.escrever(reqwest::Method::POST, "/context", &c)
        .form(&[("kind", "organisation")])
        .send()
        .await
        .unwrap();
    let k: Option<String> = sqlx::query_scalar(
        "SELECT active_context_kind FROM sessions WHERE person_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .fetch_one(&s.pool)
    .await
    .unwrap();
    assert_eq!(k.as_deref(), Some("organisation"));
    s.escrever(reqwest::Method::POST, "/distribution/switch", &c)
        .form(&[("to", "business")])
        .send()
        .await
        .unwrap();
    let k: Option<String> = sqlx::query_scalar(
        "SELECT active_context_kind FROM sessions WHERE person_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .fetch_one(&s.pool)
    .await
    .unwrap();
    assert_eq!(k, None, "o contexto atravessou a mudança de Distribuição");
}

// ── H · desactivada a meio da sessão (S39); acesso retirado (S18) ─────────

#[tokio::test]
async fn h_desactivada_ou_retirada_a_meio_da_sessao_fecha_tudo() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    activar(&s, "business").await;
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    dar(&s, id, "research").await;
    dar(&s, id, "business").await;
    let (_, _, c) = s.entrar(&email, &pw).await;
    s.escrever(reqwest::Method::POST, "/distribution", &c)
        .form(&[("distribution", "business")])
        .send()
        .await
        .unwrap();
    s.html("/notes", &c).await;
    assert!(!ids(&janelas(&s, &c).await).is_empty());

    desactivar(&s, "business").await;
    let (status, html) = s.html("/", &c).await;
    assert_eq!(status, 403);
    assert!(html.contains(&pt("dist.disabled_live.title", "business")));
    assert!(
        !html.contains(r#"data-part="desk""#),
        "o Desktop ficou à vista"
    );

    // S18: noutra sessão, o acesso a Research retirado.
    let (_, _, c2) = s.entrar(&email, &pw).await;
    let r = s.get("/", &c2).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 200, "uma só acessível: entra");
    tirar(&s, id, "research").await;
    let (status, html) = s.html("/", &c2).await;
    assert_eq!(status, 403);
    assert!(html.contains(&pt("dist.revoked.title", "research")));
}

// ── I · contexto retirado (S21) ───────────────────────────────────────────

#[tokio::test]
async fn i_contexto_retirado_e_reposto_e_dito() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let unidade: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name, status)
         VALUES ($1, $2, $2, 'active') RETURNING id",
    )
    .bind(s.organisation_id)
    .bind(format!("U{}", &Uuid::new_v4().simple().to_string()[..8]))
    .fetch_one(&s.pool)
    .await
    .expect("unidade");
    sqlx::query(
        "INSERT INTO unit_memberships (unit_id, person_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(unidade)
    .bind(id)
    .execute(&s.pool)
    .await
    .expect("pertença");
    let (_, _, c) = s.entrar(&email, &pw).await;
    s.html("/", &c).await;
    let unidade_s = unidade.to_string();
    s.escrever(reqwest::Method::POST, "/context", &c)
        .form(&[("kind", "unit"), ("id", unidade_s.as_str())])
        .send()
        .await
        .unwrap();
    sqlx::query(
        "UPDATE unit_memberships SET revoked_at = now() WHERE unit_id = $1 AND person_id = $2",
    )
    .bind(unidade)
    .bind(id)
    .execute(&s.pool)
    .await
    .expect("retirar");
    let (_, html) = s.html("/", &c).await;
    assert!(
        html.contains(r#"data-part="ctx-revoked""#),
        "S21 não foi dito"
    );
    let k: Option<String> = sqlx::query_scalar(
        "SELECT active_context_kind FROM sessions WHERE person_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .fetch_one(&s.pool)
    .await
    .unwrap();
    assert_eq!(k, None);
}

// ── Fronteiras dos pontos de acesso ───────────────────────────────────────

/// A sessão é do anfitrião: o cookie não leva `Domain`, e uma sessão de um
/// ponto não serve noutro; a origem de uma escrita é a do ponto.
#[tokio::test]
async fn a_sessao_e_a_origem_sao_do_ponto() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let outro = s.ponto(None).await;
    let (_, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let r = s
        .escrever(reqwest::Method::POST, "/login", "")
        .form(&[("email", email.as_str()), ("password", pw.as_str())])
        .send()
        .await
        .unwrap();
    let cookies: Vec<String> = r
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_owned))
        .collect();
    let sessao = cookies
        .iter()
        .find(|c| c.starts_with(&format!("{}=", ocinye_workspace::session::COOKIE_NAME)))
        .expect("cookie de sessão");
    assert!(
        !sessao.to_ascii_lowercase().contains("domain="),
        "cookie com Domain: {sessao}"
    );
    let c = sessao.split(';').next().unwrap().to_owned();

    // A mesma sessão noutro anfitrião não entra.
    let r = s.get_em(&outro, "/", &c).send().await.unwrap();
    assert!(
        location(&r).starts_with("/login"),
        "a sessão atravessou anfitriões"
    );

    // Uma escrita com a origem de outro ponto é recusada.
    let r = s
        .http
        .post(format!("{}/context", s.url))
        .header("origin", s.origin_de(&outro))
        .header("cookie", format!("oc_boot=1; {c}"))
        .form(&[("kind", "organisation")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 403, "origem de outro ponto aceite");
}

/// Um ponto desactivado a meio da sessão termina-a (S40).
#[tokio::test]
async fn ponto_desactivado_termina_a_sessao() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let outro = s.ponto(None).await;
    let (_, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, c) = s.entrar_em(&outro, &email, &pw).await;
    assert_eq!(
        s.get_em(&outro, "/", &c)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        200
    );
    sqlx::query("UPDATE access_endpoints SET state = 'disabled' WHERE hostname = $1")
        .bind(&outro)
        .execute(&s.pool)
        .await
        .unwrap();
    // A resolução fica em cache 5 s no Workspace (um nome, muitos pedidos).
    tokio::time::sleep(std::time::Duration::from_millis(5200)).await;
    let r = s.get_em(&outro, "/", &c).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 410);
    let r = s.get_em(&outro, "/", &c).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 404, "a sessão sobreviveu ao ponto");
}

/// Bloquear (S22): a sessão fica, o trabalho fica, e nada se vê até a
/// palavra-passe voltar.
#[tokio::test]
async fn bloquear_e_desbloquear() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, c) = s.entrar(&email, &pw).await;
    s.html("/notes", &c).await;
    let antes = ids(&janelas(&s, &c).await);
    let r = s
        .escrever(reqwest::Method::POST, "/lock", &c)
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/lock");
    let r = s.get("/", &c).send().await.unwrap();
    assert_eq!(
        location(&r),
        "/lock",
        "bloqueado, a casca continuou à vista"
    );
    let r = s
        .escrever(reqwest::Method::POST, "/unlock", &c)
        .form(&[("password", "errada-de-proposito")])
        .send()
        .await
        .unwrap();
    assert!(location(&r).starts_with("/lock"));
    let r = s
        .escrever(reqwest::Method::POST, "/unlock", &c)
        .form(&[("password", pw.as_str())])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/");
    assert_eq!(
        ids(&janelas(&s, &c).await),
        antes,
        "desbloquear perdeu as janelas"
    );
}
