//! A001 · Auditoria pré-D011: uma prova por defeito corrigido no Workspace,
//! por HTTP contra um Core real. Cada uma falha com a guarda retirada.

mod common;

use common::*;
use ocinye_contracts::TechnicalRole;
use uuid::Uuid;

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

async fn entrar_em(s: &Sistema, cookie: &str, d: &str) {
    s.escrever(reqwest::Method::POST, "/distribution", cookie)
        .form(&[("distribution", d)])
        .send()
        .await
        .expect("entrar");
}

// ── H005 · o bloqueio vale para todas as rotas ────────────────────────────

#[tokio::test]
async fn h005_bloqueado_nenhuma_rota_responde_alem_do_bloqueio() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let r = s
        .escrever(reqwest::Method::POST, "/lock", &c)
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/lock");

    // Uma aplicação por `GET` vai para o bloqueio (antes: abria as Notas).
    let r = s.get("/notes", &c).send().await.unwrap();
    assert_eq!(
        location(&r),
        "/lock",
        "as Notas abriram com o ecrã bloqueado"
    );
    // O terminal, por JSON, é recusado (antes: executava).
    let r = s
        .escrever(reqwest::Method::POST, "/terminal/exec", &c)
        .header("content-type", "application/json")
        .body(r#"{"line":"help"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        423,
        "o terminal correu com o ecrã bloqueado"
    );
    // As janelas também.
    let r = s
        .escrever(reqwest::Method::POST, "/wm", &c)
        .header("accept", "application/json")
        .form(&[("app_id", "notes"), ("href", "/notes")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 423);
    // O próprio ecrã de bloqueio e a saída continuam.
    assert_eq!(
        s.get("/lock", &c).send().await.unwrap().status().as_u16(),
        200
    );
}

// ── H006 · o caminho do avatar não chega a outras rotas do Core ───────────

#[tokio::test]
async fn h006_a_versao_do_avatar_e_um_segmento_e_nada_mais() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    for v in [
        "..%2F..%2Fme%2Fdesktop",
        "..%2F..%2Ffiles%2F00000000-0000-0000-0000-000000000000%2Fraw",
        "a%2Fb",
        "%2E%2E",
    ] {
        let r = s.get(&format!("/avatar/me/{v}"), &c).send().await.unwrap();
        assert_eq!(r.status().as_u16(), 404, "{v} chegou ao Core");
        let ct = r
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        assert!(!ct.contains("json"), "{v} devolveu JSON do Core: {ct}");
    }
}

// ── M001 · um TAB não faz um open redirect ────────────────────────────────

#[tokio::test]
async fn m001_o_regresso_recusa_caracteres_de_controlo() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    activar(&s, "business").await;
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    dar(&s, id, "research").await;
    dar(&s, id, "business").await;
    let (_, _, c) = s.entrar(&email, &pw).await;
    entrar_em(&s, &c, "research").await;
    for back in ["/\t/evil.example", "/\n/evil.example", "/\r//evil.example"] {
        let r = s
            .escrever(reqwest::Method::POST, "/distribution/switch", &c)
            .form(&[("to", "business"), ("return", back)])
            .send()
            .await
            .unwrap();
        let loc = location(&r);
        assert!(
            !loc.contains("evil") && !loc.chars().any(char::is_control),
            "{back:?} → {loc:?}"
        );
        // Volta a Research para a próxima tentativa.
        s.escrever(reqwest::Method::POST, "/distribution/switch", &c)
            .form(&[("to", "research")])
            .send()
            .await
            .unwrap();
    }
}

// ── M002 · perder a activa com duas ou mais restantes não prende ──────────

#[tokio::test]
async fn m002_perder_a_activa_deixa_escolher_entre_as_que_restam() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    activar(&s, "business").await;
    activar(&s, "personal").await;
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    for d in ["research", "business", "personal"] {
        dar(&s, id, d).await;
    }
    let (_, _, c) = s.entrar(&email, &pw).await;
    entrar_em(&s, &c, "business").await;
    sqlx::query(
        "UPDATE instance_distributions SET state = 'disabled', disabled_at = now()
          WHERE organisation_id = $1 AND distribution = 'business'",
    )
    .bind(s.organisation_id)
    .execute(&s.pool)
    .await
    .unwrap();
    // A escolha do seleccionador funciona: entra em Research.
    let r = s
        .escrever(reqwest::Method::POST, "/distribution", &c)
        .form(&[("distribution", "research")])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/", "o membro ficou preso na recusa");
    let (status, _) = s.html("/", &c).await;
    assert_eq!(status, 200);
}

// ── M003 · escolher outra com uma activa é mudar (com o diálogo) ──────────

#[tokio::test]
async fn m003_o_seleccionador_nao_fecha_janelas_por_atalho() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    activar(&s, "business").await;
    let (id, email, pw) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    dar(&s, id, "research").await;
    dar(&s, id, "business").await;
    let (_, _, c) = s.entrar(&email, &pw).await;
    entrar_em(&s, &c, "research").await;
    s.html("/notes", &c).await;
    // A mesma: nada muda.
    let r = s
        .escrever(reqwest::Method::POST, "/distribution", &c)
        .form(&[("distribution", "research")])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/");
    // Outra: vai para a mudança (S16), não fecha nada por atalho.
    let r = s
        .escrever(reqwest::Method::POST, "/distribution", &c)
        .form(&[("distribution", "business")])
        .send()
        .await
        .unwrap();
    assert_eq!(location(&r), "/?switch=business");
    let wm: serde_json::Value = s
        .get("/wm", &c)
        .header("accept", "application/json")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        wm["windows"].as_array().is_some_and(|w| !w.is_empty()),
        "as janelas fecharam sem passar pelo diálogo"
    );
}
