//! As viagens da organização (Claude Design D006) contra um Core real:
//! Unidades e Administração (Membros, Papéis, Instância).
//!
//! ```text
//! pedido HTTP → Workspace (ui::apps) → Core → PostgreSQL
//! ```
//!
//! Cada viagem prova uma fronteira: quem não administra pessoas não vê nada
//! da Administração, nem por endereço; a credencial temporária aparece uma
//! vez, na resposta que a emitiu, e nunca mais; as invariantes (último
//! administrador, último gestor, auto-bloqueio) são do Core, e a vista só as
//! mostra; um formulário forjado não passa; e nada precisa de IA.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida — e diz que saltou;
//! em CI, falha.

mod common;

use common::*;
use ocinye_contracts::TechnicalRole;
use uuid::Uuid;

async fn form(s: &Sistema, cookie: &str, path: &str, campos: &[(&str, &str)]) -> reqwest::Response {
    s.escrever(reqwest::Method::POST, path, cookie)
        .form(campos)
        .send()
        .await
        .expect("POST")
}

/// Uma administração da organização (sem segundo factor: não é plataforma).
async fn admin_org(s: &Sistema) -> (Uuid, String) {
    s.membro_com_sessao(&[TechnicalRole::OrganisationAdmin])
        .await
}

/// Uma administração da plataforma, com o segundo factor confirmado.
async fn admin_plataforma(s: &Sistema) -> (Uuid, String) {
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

async fn nome(s: &Sistema, id: Uuid) -> String {
    sqlx::query_scalar("SELECT full_name FROM people WHERE id = $1")
        .bind(id)
        .fetch_one(&s.pool)
        .await
        .expect("nome")
}

async fn estado(s: &Sistema, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM people WHERE id = $1")
        .bind(id)
        .fetch_one(&s.pool)
        .await
        .expect("estado")
}

async fn unidade(s: &Sistema) -> Uuid {
    let suf = Uuid::new_v4().simple().to_string();
    sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(s.organisation_id)
    .bind(format!("U{}", &suf[..6]).to_uppercase())
    .bind(format!("Unidade {}", &suf[..4]))
    .fetch_one(&s.pool)
    .await
    .expect("unidade")
}

async fn pertence(s: &Sistema, unit: Uuid, person: Uuid, role: &str) {
    sqlx::query("INSERT INTO unit_memberships (unit_id, person_id, role) VALUES ($1, $2, $3)")
        .bind(unit)
        .bind(person)
        .bind(role)
        .execute(&s.pool)
        .await
        .expect("pertença");
}

async fn gestores(s: &Sistema, unit: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM unit_memberships
          WHERE unit_id = $1 AND role = 'manager' AND revoked_at IS NULL",
    )
    .bind(unit)
    .fetch_one(&s.pool)
    .await
    .expect("gestores")
}

// ── A Administração só existe para quem gere pessoas ─────────────────────

#[tokio::test]
async fn a_administracao_nao_existe_para_quem_nao_gere_pessoas() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (alvo, _) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let nome_alvo = nome(&s, alvo).await;
    let (_, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    // Endereço directo, membro conhecido e um identificador inventado: a mesma
    // resposta, sem o nome, sem navegação da Administração.
    for rota in [
        "/admin".to_owned(),
        format!("/admin/members/{alvo}"),
        format!("/admin/members/{}", Uuid::new_v4()),
        "/admin/roles".to_owned(),
        "/admin/instance".to_owned(),
        "/admin/members/new".to_owned(),
        format!("/admin/members/{alvo}?confirm=suspend"),
    ] {
        let (status, html) = s.html(&rota, &c).await;
        assert_eq!(status, 404, "{rota}");
        assert!(!html.contains(&nome_alvo), "{rota} revelou o membro");
        assert!(!html.contains(r#"data-part="member""#), "{rota}");
        assert!(!html.contains("oc-org-confirm"), "{rota}");
    }
    // E uma operação forjada é recusada pelo Core, sem mudar nada.
    let r = form(
        &s,
        &c,
        &format!("/admin/members/{alvo}/status"),
        &[("status", "suspended"), ("reason", "forjado")],
    )
    .await;
    assert_eq!(r.status().as_u16(), 403);
    assert_eq!(estado(&s, alvo).await, "active");
}

#[tokio::test]
async fn o_roster_mostra_os_quatro_estados_e_o_detalhe_se_rele() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = admin_org(&s).await;
    let mut ids = Vec::new();
    for st in ["invited", "active", "suspended", "disabled"] {
        let (id, _, _) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
        sqlx::query("UPDATE people SET status = $2 WHERE id = $1")
            .bind(id)
            .bind(st)
            .execute(&s.pool)
            .await
            .expect("estado");
        ids.push(id);
    }
    let (status, html) = s.html("/admin", &c).await;
    assert_eq!(status, 200);
    for st in ["invited", "active", "suspended", "disabled"] {
        assert!(
            html.contains(&format!(r#"data-status="{st}""#)),
            "o roster não mostra {st}"
        );
    }
    // O detalhe relê do Core: um membro apagado por baixo desaparece, sem
    // confiar no que a lista mostrou.
    let (status, html) = s.html(&format!("/admin/members/{}", ids[1]), &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"data-part="member""#));
    assert!(html.contains(r#"data-part="member-security""#));
    let (status, _) = s
        .html(&format!("/admin/members/{}", Uuid::new_v4()), &c)
        .await;
    assert_eq!(status, 404);
    // Nenhum segredo no detalhe: nem verificador, nem campo de credencial.
    assert!(!html.contains("org-secret") && !html.contains("verifier"));
}

// ── A credencial temporária: uma vez, e nunca mais ───────────────────────

#[tokio::test]
async fn criar_um_membro_mostra_a_credencial_uma_vez_e_nunca_mais() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = admin_org(&s).await;
    let email = format!("n{}@ocinye.com", &Uuid::new_v4().simple().to_string()[..10]);
    let r = form(
        &s,
        &c,
        "/admin/members/new",
        &[
            ("full_name", "Nova Pessoa"),
            ("email", &email),
            ("role", "research_member"),
            ("position", "researcher"),
            ("unit_id", ""),
        ],
    )
    .await;
    // A credencial vem na resposta do POST — não num redireccionamento.
    assert_eq!(r.status().as_u16(), 200);
    assert_eq!(
        r.headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    let html = r.text().await.expect("corpo");
    assert!(html.contains(r#"data-part="org-credential""#));
    let segredo = html
        .split(r#"data-part="org-secret""#)
        .next()
        .and_then(|antes| antes.rsplit("value=\"").next())
        .and_then(|v| v.split('"').next())
        .expect("o segredo no campo")
        .to_owned();
    assert!(segredo.len() >= 12, "segredo curto: {segredo:?}");
    assert!(
        html.contains(r#"type="password""#),
        "o campo começa escondido"
    );

    let id: Uuid = sqlx::query_scalar("SELECT id FROM people WHERE email = $1")
        .bind(&email)
        .fetch_one(&s.pool)
        .await
        .expect("criado");
    assert_eq!(estado(&s, id).await, "invited");

    // Nunca mais: nem no roster, nem no detalhe, nem ao recarregar.
    for rota in ["/admin".to_owned(), format!("/admin/members/{id}")] {
        let (_, html) = s.html(&rota, &c).await;
        assert!(!html.contains(&segredo), "{rota} mostrou a credencial");
        assert!(!html.contains(r#"data-part="org-credential""#), "{rota}");
    }
    // Nem na auditoria, nem em lado nenhum da base: só o verificador existe.
    let fugas: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM audit_events WHERE metadata::text LIKE '%' || $1 || '%')
              + (SELECT count(*) FROM outbox_events WHERE payload::text LIKE '%' || $1 || '%')",
    )
    .bind(&segredo)
    .fetch_one(&s.pool)
    .await
    .expect("auditoria");
    assert_eq!(fugas, 0, "a credencial entrou na auditoria");
}

#[tokio::test]
async fn criar_um_membro_com_um_papel_forjado_nao_cria_nada() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = admin_org(&s).await;
    for papel in ["inventado", "platform_admin"] {
        let email = format!("f{}@ocinye.com", &Uuid::new_v4().simple().to_string()[..10]);
        let r = form(
            &s,
            &c,
            "/admin/members/new",
            &[("full_name", "Forjada"), ("email", &email), ("role", papel)],
        )
        .await;
        assert_eq!(r.status().as_u16(), 422, "{papel}");
        let html = r.text().await.expect("corpo");
        assert!(!html.contains(r#"data-part="org-credential""#), "{papel}");
        let criados: i64 = sqlx::query_scalar("SELECT count(*) FROM people WHERE email = $1")
            .bind(&email)
            .fetch_one(&s.pool)
            .await
            .expect("contar");
        assert_eq!(criados, 0, "{papel} criou uma conta");
    }
}

// ── Estado da conta: confirmação, auto-bloqueio, forja ───────────────────

#[tokio::test]
async fn o_estado_da_conta_passa_pela_confirmacao_e_o_core_decide() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (eu, c) = admin_org(&s).await;
    let (alvo, _) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let nome_alvo = nome(&s, alvo).await;
    let base = format!("/admin/members/{alvo}");

    // A confirmação nomeia o alvo, leva o destino fixo, e o foco vai para
    // «Cancelar».
    let (_, html) = s.html(&format!("{base}?confirm=suspend"), &c).await;
    assert!(html.contains(r#"data-oc="org-confirm""#));
    assert!(html.contains(&nome_alvo));
    assert!(html.contains(&format!(r#"action="{base}/status""#)));
    assert!(html.contains(r#"name="status" value="suspended""#));
    assert!(html.contains(r#"data-oc="org-confirm-cancel" autofocus"#));
    assert!(html.contains(r#"minlength="4""#));

    // Sem razão suficiente, o Core recusa e a confirmação volta.
    let r = form(
        &s,
        &c,
        &format!("{base}/status"),
        &[("status", "suspended"), ("reason", "x")],
    )
    .await;
    assert!(location(&r).contains("confirm=suspend") && location(&r).contains("err=save"));
    assert_eq!(estado(&s, alvo).await, "active");

    let r = form(
        &s,
        &c,
        &format!("{base}/status"),
        &[("status", "suspended"), ("reason", "ausência prolongada")],
    )
    .await;
    assert_eq!(location(&r), format!("{base}?done=status"));
    assert_eq!(estado(&s, alvo).await, "suspended");
    // Repetir a mesma operação não duplica nada nem quebra.
    let (_, html) = s.html(&format!("{base}?done=status"), &c).await;
    assert!(html.contains(r#"data-notice="org.done.status""#));
    // Uma confirmação para uma acção que já não é oferecida não abre.
    let (_, html) = s.html(&format!("{base}?confirm=suspend"), &c).await;
    assert!(!html.contains(r#"data-oc="org-confirm""#));

    // Forja: `invited` não é um destino — nem chega ao Core.
    let r = form(
        &s,
        &c,
        &format!("{base}/status"),
        &[("status", "invited"), ("reason", "forjado agora")],
    )
    .await;
    assert_eq!(location(&r), base);
    assert_eq!(estado(&s, alvo).await, "suspended");
    // Desactivada é final: reactivar é recusado pelo Core.
    form(
        &s,
        &c,
        &format!("{base}/status"),
        &[("status", "disabled"), ("reason", "saiu da instituição")],
    )
    .await;
    assert_eq!(estado(&s, alvo).await, "disabled");
    form(
        &s,
        &c,
        &format!("{base}/status"),
        &[("status", "active"), ("reason", "voltar atrás")],
    )
    .await;
    assert_eq!(estado(&s, alvo).await, "disabled");

    // Auto-bloqueio: a vista não o oferece, e o POST forjado é recusado pelo
    // Core com o motivo tipado.
    let (_, html) = s.html(&format!("/admin/members/{eu}"), &c).await;
    assert!(!html.contains(r#"data-kind="org.act.suspend""#));
    let r = form(
        &s,
        &c,
        &format!("/admin/members/{eu}/status"),
        &[("status", "disabled"), ("reason", "engano meu")],
    )
    .await;
    assert!(
        location(&r).contains("refused=self_lockout"),
        "{}",
        location(&r)
    );
    assert_eq!(estado(&s, eu).await, "active");
    let (_, html) = s.html(&location(&r), &c).await;
    assert!(html.contains(r#"data-refusal="org.refusal.self""#));
}

#[tokio::test]
async fn o_ultimo_administrador_da_plataforma_nao_se_suspende() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = admin_org(&s).await;
    let (pa, _, _) = s.pessoa(&[TechnicalRole::PlatformAdmin]).await;
    let base = format!("/admin/members/{pa}");
    for destino in ["suspended", "disabled"] {
        let r = form(
            &s,
            &c,
            &format!("{base}/status"),
            &[("status", destino), ("reason", "reorganização")],
        )
        .await;
        assert!(
            location(&r).contains("refused=last_platform_admin"),
            "{destino}: {}",
            location(&r)
        );
        assert_eq!(estado(&s, pa).await, "active");
    }
    let r = form(&s, &c, &format!("{base}/delete"), &[]).await;
    assert!(location(&r).contains("refused="), "{}", location(&r));
    assert_eq!(estado(&s, pa).await, "active");
}

// ── Papéis ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn os_papeis_sao_os_do_core_e_um_papel_forjado_nao_passa() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (eu, c) = admin_plataforma(&s).await;
    let (_, html) = s.html("/admin/roles", &c).await;
    // Os oito papéis de sistema, com permissões que o Core diz (não a fixture).
    for papel in [
        "platform_admin",
        "organisation_admin",
        "unit_manager",
        "research_lead",
        "research_member",
        "collaborator",
        "external_collaborator",
        "auditor",
    ] {
        assert!(html.contains(&format!(">{papel}</code>")), "{papel}");
    }
    assert!(html.contains("platform.administer"));

    let (alvo, _) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let base = format!("/admin/members/{alvo}");
    // Conceder: só pela confirmação, com a razão que o Core exige.
    let (_, html) = s
        .html(&format!("{base}?confirm=grant_role&role=auditor"), &c)
        .await;
    assert!(html.contains(r#"name="role" value="auditor""#));
    let r = form(
        &s,
        &c,
        &format!("{base}/roles"),
        &[("role", "auditor"), ("reason", "auditoria anual")],
    )
    .await;
    assert_eq!(location(&r), format!("{base}?done=role_granted"));
    let tem: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM person_roles WHERE person_id = $1 AND role = 'auditor' AND revoked_at IS NULL)",
    )
    .bind(alvo)
    .fetch_one(&s.pool)
    .await
    .expect("papel");
    assert!(tem);
    // Forjado: um papel que o catálogo não tem não chega ao Core.
    let r = form(
        &s,
        &c,
        &format!("{base}/roles"),
        &[("role", "superuser"), ("reason", "forjado")],
    )
    .await;
    assert!(location(&r).contains("refused=option"));
    // O próprio não revoga o seu papel de plataforma pela vista, e o POST
    // forjado encontra a invariante do último administrador.
    let (_, html) = s.html(&format!("/admin/members/{eu}"), &c).await;
    assert!(!html.contains("confirm=revoke_role&amp;role=platform_admin"));
    let r = form(
        &s,
        &c,
        &format!("/admin/members/{eu}/roles/platform_admin/revoke"),
        &[],
    )
    .await;
    assert!(
        location(&r).contains("refused=last_platform_admin"),
        "{}",
        location(&r)
    );

    // Uma administração da organização não concede papéis: o Core recusa.
    let (_, c_org) = admin_org(&s).await;
    let r = form(
        &s,
        &c_org,
        &format!("{base}/roles"),
        &[("role", "collaborator"), ("reason", "tentativa")],
    )
    .await;
    assert_eq!(r.status().as_u16(), 403);
}

// ── Unidades ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_unidade_so_oferece_accoes_com_o_sinal_do_core() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let u = unidade(&s).await;
    let (gestor, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    pertence(&s, u, gestor, "manager").await;
    let (membro_id, _, _) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    pertence(&s, u, membro_id, "member").await;
    let (_, _, cg) = s.entrar(&email, &password).await;
    let (_, cm) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;

    // Quem gere: mudar papel e retirar; o acrescentar explicado e sem rota.
    let (status, html) = s.html(&format!("/units/{u}"), &cg).await;
    assert_eq!(status, 200);
    assert!(html.contains(r#"data-kind="org.act.unit_role""#));
    assert!(html.contains(r#"data-kind="org.act.unit_remove""#));
    assert!(html.contains(r#"data-part="unit-add" data-unavailable"#));
    assert!(
        !html.contains(r#"name="candidate_q""#),
        "o selector sem contrato apareceu"
    );
    // Sem `members.manage`, o nome não liga à Administração.
    assert!(!html.contains(&format!("/admin/members/{membro_id}")));
    let r = form(
        &s,
        &cg,
        &format!("/units/{u}/members"),
        &[("person_id", &membro_id.to_string()), ("role", "member")],
    )
    .await;
    assert!(
        matches!(r.status().as_u16(), 404 | 405),
        "acrescentar tem rota: {}",
        r.status()
    );

    // Quem só lê: sem acções.
    let (_, html) = s.html(&format!("/units/{u}"), &cm).await;
    assert!(!html.contains(r#"data-kind="org.act.unit_role""#));
    assert!(!html.contains(r#"data-kind="org.act.unit_remove""#));

    // A confirmação leva o alvo e o papel fixos.
    let (_, html) = s
        .html(
            &format!("/units/{u}?confirm=unit_role&person={membro_id}"),
            &cg,
        )
        .await;
    assert!(html.contains(&format!(r#"action="/units/{u}/members/role""#)));
    assert!(html.contains(&format!(r#"name="person_id" value="{membro_id}""#)));
    assert!(html.contains(r#"name="role" value="manager""#));
    // Uma confirmação para quem não pertence à unidade não abre.
    let (_, html) = s
        .html(
            &format!("/units/{u}?confirm=unit_remove&person={}", Uuid::new_v4()),
            &cg,
        )
        .await;
    assert!(!html.contains(r#"data-oc="org-confirm""#));
}

#[tokio::test]
async fn o_ultimo_gestor_nao_sai_nem_e_despromovido() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let u = unidade(&s).await;
    let (gestor, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    pertence(&s, u, gestor, "manager").await;
    let (_, _, cg) = s.entrar(&email, &password).await;
    let g = gestor.to_string();

    // Despromover o último gestor (U-12): recusado pelo Core, com o motivo.
    let r = form(
        &s,
        &cg,
        &format!("/units/{u}/members/role"),
        &[("person_id", &g), ("role", "member")],
    )
    .await;
    assert!(
        location(&r).contains("refused=last_unit_manager"),
        "{}",
        location(&r)
    );
    assert_eq!(gestores(&s, u).await, 1);
    let (_, html) = s.html(&location(&r), &cg).await;
    assert!(html.contains(r#"data-refusal="org.refusal.last_manager""#));
    // Retirá-lo: o mesmo.
    let r = form(
        &s,
        &cg,
        &format!("/units/{u}/members/remove"),
        &[("person_id", &g)],
    )
    .await;
    assert!(
        location(&r).contains("refused=last_unit_manager"),
        "{}",
        location(&r)
    );
    assert_eq!(gestores(&s, u).await, 1);

    // Com um segundo gestor, o primeiro passa a membro.
    let (outra, _, _) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    pertence(&s, u, outra, "member").await;
    let r = form(
        &s,
        &cg,
        &format!("/units/{u}/members/role"),
        &[("person_id", &outra.to_string()), ("role", "manager")],
    )
    .await;
    assert_eq!(location(&r), format!("/units/{u}?done=unit_role"));
    let r = form(
        &s,
        &cg,
        &format!("/units/{u}/members/role"),
        &[("person_id", &g), ("role", "member")],
    )
    .await;
    assert_eq!(location(&r), format!("/units/{u}?done=unit_role"));
    assert_eq!(gestores(&s, u).await, 1);
}

#[tokio::test]
async fn quem_gere_uma_unidade_nao_mexe_noutra() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let a = unidade(&s).await;
    let b = unidade(&s).await;
    let (gestor_a, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    pertence(&s, a, gestor_a, "manager").await;
    let (gestor_b, _, _) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    pertence(&s, b, gestor_b, "manager").await;
    let (membro_b, _, _) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    pertence(&s, b, membro_b, "member").await;
    let (_, _, ca) = s.entrar(&email, &password).await;

    let (_, html) = s.html(&format!("/units/{b}"), &ca).await;
    assert!(!html.contains(r#"data-kind="org.act.unit_remove""#));
    for (rota, campos) in [
        (
            format!("/units/{b}/members/remove"),
            vec![("person_id", membro_b.to_string())],
        ),
        (
            format!("/units/{b}/members/role"),
            vec![
                ("person_id", membro_b.to_string()),
                ("role", "manager".to_owned()),
            ],
        ),
    ] {
        let campos: Vec<(&str, &str)> = campos.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let r = form(&s, &ca, &rota, &campos).await;
        assert_eq!(r.status().as_u16(), 403, "{rota}");
    }
    let vivo: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM unit_memberships WHERE unit_id = $1 AND person_id = $2 AND role = 'member' AND revoked_at IS NULL)",
    )
    .bind(b)
    .bind(membro_b)
    .fetch_one(&s.pool)
    .await
    .expect("pertença");
    assert!(vivo, "a pertença na outra unidade mudou");
}

#[tokio::test]
async fn retirar_e_registo_e_nao_apagar() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let u = unidade(&s).await;
    let (gestor, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    pertence(&s, u, gestor, "manager").await;
    let (m, _, _) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    pertence(&s, u, m, "member").await;
    let (_, _, cg) = s.entrar(&email, &password).await;
    let r = form(
        &s,
        &cg,
        &format!("/units/{u}/members/remove"),
        &[("person_id", &m.to_string())],
    )
    .await;
    assert_eq!(location(&r), format!("/units/{u}?done=member_removed"));
    let (linhas, revogada): (i64, bool) = sqlx::query_as(
        "SELECT count(*), bool_and(revoked_at IS NOT NULL) FROM unit_memberships WHERE unit_id = $1 AND person_id = $2",
    )
    .bind(u)
    .bind(m)
    .fetch_one(&s.pool)
    .await
    .expect("linha");
    assert_eq!(
        (linhas, revogada),
        (1, true),
        "a pertença foi apagada em vez de registada"
    );
    // Repetir a remoção não duplica nada: falha em segurança.
    let r = form(
        &s,
        &cg,
        &format!("/units/{u}/members/remove"),
        &[("person_id", &m.to_string())],
    )
    .await;
    assert_eq!(r.status().as_u16(), 404);
}

// ── Instância ────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_instancia_mostra_o_que_o_core_diz_e_activar_nao_toca_nas_fixacoes() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = admin_org(&s).await;
    let (m, cm) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    sqlx::query("INSERT INTO member_app_pins (person_id, pinned_app_ids) VALUES ($1, ARRAY['notes','units'])")
        .bind(m)
        .execute(&s.pool)
        .await
        .ok();
    let pins_antes: Option<Vec<String>> =
        sqlx::query_scalar("SELECT pinned_app_ids FROM member_app_pins WHERE person_id = $1")
            .bind(m)
            .fetch_optional(&s.pool)
            .await
            .expect("pins");

    let (status, html) = s.html("/admin/instance", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains("Instituição da viagem"));
    assert!(html.contains(r#"data-part="admin-apps""#));
    assert!(
        html.contains(r#"name="app:bibliography""#),
        "decisões editáveis"
    );

    let r = form(
        &s,
        &c,
        "/admin/instance",
        &[("app:bibliography", "inactive"), ("profile", "personal")],
    )
    .await;
    assert_eq!(location(&r), "/admin/instance?done=settings");
    let (perfil, activa): (String, Option<bool>) = sqlx::query_as(
        "SELECT o.profile, (SELECT active FROM instance_applications WHERE organisation_id = o.id AND application_id = 'bibliography')
           FROM organisations o WHERE o.id = $1",
    )
    .bind(s.organisation_id)
    .fetch_one(&s.pool)
    .await
    .expect("instância");
    assert_eq!(activa, Some(false));
    assert_eq!(perfil, "research", "a Distribuição mudou pela Instância");
    let pins_depois: Option<Vec<String>> =
        sqlx::query_scalar("SELECT pinned_app_ids FROM member_app_pins WHERE person_id = $1")
            .bind(m)
            .fetch_optional(&s.pool)
            .await
            .expect("pins");
    assert_eq!(pins_antes, pins_depois, "activar tocou nas fixações");

    let (status, _) = s.html("/admin/instance", &cm).await;
    assert_eq!(status, 404);
}

// ── Privilégio perdido · sem IA · por frame ──────────────────────────────

#[tokio::test]
async fn o_privilegio_perdido_a_meio_falha_fechado() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (eu, c) = admin_org(&s).await;
    let (alvo, _) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let nome_alvo = nome(&s, alvo).await;
    let (status, html) = s.html(&format!("/admin/members/{alvo}"), &c).await;
    assert_eq!(status, 200);
    assert!(html.contains(&nome_alvo));

    sqlx::query("UPDATE person_roles SET revoked_at = now() WHERE person_id = $1")
        .bind(eu)
        .execute(&s.pool)
        .await
        .expect("revogar");

    let (status, html) = s.html(&format!("/admin/members/{alvo}"), &c).await;
    assert_eq!(status, 404);
    assert!(!html.contains(&nome_alvo));
    let r = form(
        &s,
        &c,
        &format!("/admin/members/{alvo}/status"),
        &[("status", "suspended"), ("reason", "janela antiga")],
    )
    .await;
    assert_eq!(r.status().as_u16(), 403);
    assert_eq!(estado(&s, alvo).await, "active");
}

#[tokio::test]
async fn unidades_e_administracao_funcionam_sem_ia_e_por_frame() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (_, c) = admin_org(&s).await;
    let u = unidade(&s).await;
    for (rota, app) in [
        ("/admin".to_owned(), "admin"),
        ("/admin/roles".to_owned(), "admin"),
        ("/admin/instance".to_owned(), "admin"),
        ("/units".to_owned(), "units"),
        (format!("/units/{u}"), "units"),
    ] {
        let (status, html) = s.html(&rota, &c).await;
        assert_eq!(status, 200, "{rota}");
        assert!(html.contains(&format!(r#"data-app="{app}""#)), "{rota}");
        let (status, frame) = s
            .html(
                &format!(
                    "{rota}{}frame=1",
                    if rota.contains('?') { '&' } else { '?' }
                ),
                &c,
            )
            .await;
        assert_eq!(status, 200, "{rota} frame");
        assert!(frame.contains(r#"data-part="win-title""#), "{rota} frame");
        assert!(!frame.contains("oc-top"), "{rota}: o frame trouxe a casca");
    }
    // Uma janela de fundo reposta por frame nunca traz a confirmação: ela só
    // pertence à janela do pedido.
    let (alvo, _) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (_, frame) = s
        .html(
            &format!("/admin/members/{alvo}?confirm=suspend&frame=1"),
            &c,
        )
        .await;
    assert!(frame.contains(r#"data-part="member""#));
    assert!(
        !frame.contains(r#"data-oc="org-confirm""#),
        "o frame trouxe a confirmação"
    );
    let (_, pagina) = s
        .html(&format!("/admin/members/{alvo}?confirm=suspend"), &c)
        .await;
    assert!(pagina.contains(r#"data-oc="org-confirm""#));
    // Nenhuma ligação de administração à Nye.
    let (_, html) = s.html("/admin", &c).await;
    assert!(!html.contains("/ai/prompt?ref="));
}

/// A autoridade sobre a Administração não é autoridade sobre cada secção: quem
/// só tem `members.manage` (um grant explícito) vê Membros, e Papéis e
/// Instância nem aparecem na navegação, nem abrem por endereço.
#[tokio::test]
async fn cada_seccao_da_administracao_tem_a_sua_autoridade() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (admin, _) = admin_org(&s).await;
    let (id, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    sqlx::query(
        "INSERT INTO explicit_access_grants (organisation_id, subject_id, permission, scope, reason, granted_by_id)
         VALUES ($1, $2, 'members.manage', 'institution', 'viagem D006: só membros', $3)",
    )
    .bind(s.organisation_id)
    .bind(id)
    .bind(admin)
    .execute(&s.pool)
    .await
    .expect("grant");
    let (_, _, c) = s.entrar(&email, &password).await;

    let (status, html) = s.html("/admin", &c).await;
    assert_eq!(status, 200, "members.manage abre a Administração");
    assert!(html.contains(r#"href="/admin""#));
    assert!(
        !html.contains(r#"href="/admin/roles""#),
        "Papéis na navegação sem roles.view"
    );
    // A Instância abre com `organisation.view` (que um membro tem), só de
    // leitura: sem `organisation.manage`, nenhuma decisão é editável.
    let (status, inst) = s.html("/admin/instance", &c).await;
    assert_eq!(status, 200);
    assert!(inst.contains(r#"data-part="admin-apps""#));
    assert!(
        !inst.contains(r#"name="app:"#),
        "decisões editáveis sem organisation.manage"
    );
    let (status, roles) = s.html("/admin/roles", &c).await;
    assert_eq!(status, 403);
    assert!(!roles.contains("platform.administer"));
    // Sem `members.create`, não há «Novo membro», e a rota recusa.
    assert!(!html.contains(r#"href="/admin/members/new""#));
    let (status, _) = s.html("/admin/members/new", &c).await;
    assert_eq!(status, 403);
}
