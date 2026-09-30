//! As viagens da D007.1 contra um Core real: o registo completo (27
//! aplicações), o Monitor de Actividade, os Resultados e o Lixo — e o fecho
//! dos caminhos de eliminação definitiva sem confirmação.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida — e diz que saltou;
//! em CI, falha.

mod common;

use common::*;
use ocinye_contracts::{ApplicationId, TechnicalRole};
use serde_json::{json, Value};
use uuid::Uuid;

fn pt(key: &str) -> &'static str {
    ocinye_workspace::i18n::t_in(ocinye_contracts::Locale::Pt, key)
}

fn marca() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_uppercase()
}

async fn form(s: &Sistema, cookie: &str, path: &str, campos: &[(&str, &str)]) -> reqwest::Response {
    s.escrever(reqwest::Method::POST, path, cookie)
        .form(campos)
        .send()
        .await
        .expect("POST")
}

fn regiao<'a>(html: &'a str, inicio: &str, fim: &str) -> &'a str {
    let i = html.find(inicio).unwrap_or_else(|| panic!("sem {inicio}"));
    let j = html[i..].find(fim).map_or(html.len(), |j| i + j);
    &html[i..j]
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

/// Uma pessoa com papéis, com cookie do Workspace e token do Core.
async fn pessoa(s: &Sistema, roles: &[TechnicalRole]) -> (Uuid, String, String) {
    let (id, email, password) = s.pessoa(roles).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    let t = token(s, &email, &password).await;
    (id, c, t)
}

async fn com_mfa(s: &Sistema, roles: &[TechnicalRole]) -> (Uuid, String) {
    let (id, email, password) = s.pessoa(roles).await;
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

// ═════════════════════════════════════════════════════════════════════════
// O registo
// ═════════════════════════════════════════════════════════════════════════

/// O registo tem 27 aplicações, sem ids nem rotas repetidos, sem Tarefas,
/// Histórico ou Browser; e nenhuma, excepto o Terminal, cai em `app_pending`.
#[tokio::test]
async fn o_registo_tem_27_e_so_o_terminal_esta_pendente() {
    let apps = ocinye_workspace::experience::apps::APPLICATIONS;
    assert_eq!(ApplicationId::ALL.len(), 27);
    assert_eq!(apps.len(), 27);
    let mut ids = std::collections::BTreeSet::new();
    let mut rotas = std::collections::BTreeSet::new();
    for a in apps {
        assert!(ids.insert(a.id()), "id repetido: {}", a.id());
        assert!(rotas.insert(a.route()), "rota repetida: {}", a.route());
        for l in [
            ocinye_contracts::Locale::Pt,
            ocinye_contracts::Locale::En,
            ocinye_contracts::Locale::Fr,
        ] {
            let nome = ocinye_workspace::i18n::t_in(l, a.manifest().name_key);
            let desc = ocinye_workspace::i18n::t_in(l, a.manifest().description_key);
            assert!(
                !nome.contains('.') || nome.contains(' '),
                "{} sem nome em {l:?}",
                a.id()
            );
            assert_ne!(
                desc,
                a.manifest().description_key,
                "{} sem descrição em {l:?}",
                a.id()
            );
        }
    }
    for fora in ["tasks", "history", "browser", "teams"] {
        assert!(!ids.contains(fora), "{fora} não é uma aplicação registada");
    }
    for nova in ["monitor", "results", "trash"] {
        assert!(ids.contains(nova), "{nova} por registar");
    }
    assert_eq!(pt("nav.audit"), "Registo de auditoria");
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    // Um perfil que abre tudo: plataforma (com o segundo factor), investigação
    // e auditoria.
    let (_, c) = com_mfa(
        &s,
        &[
            TechnicalRole::PlatformAdmin,
            TechnicalRole::ResearchMember,
            TechnicalRole::Auditor,
        ],
    )
    .await;
    // A Home é o Desktop, não uma janela (antes de haver janelas abertas).
    let (status, html) = s.html("/", &c).await;
    assert_eq!(status, 200);
    assert!(html.contains("oc-desk") && !html.contains("oc-pending oc-win__state"));
    // O Terminal primeiro, numa janela: continua pendente (D008).
    let (_, html) = s.html("/terminal", &c).await;
    assert!(
        html.contains("oc-pending"),
        "o Terminal deixou de estar pendente"
    );
    // As outras 26 pelo corpo da janela (`?frame=1`): a mesa tem um limite de
    // janelas abertas, e o corpo é o que a janela mostraria.
    for a in apps
        .iter()
        .filter(|a| !matches!(a.id(), "terminal" | "home"))
    {
        let mut rota = a.route().to_owned();
        let r = s.get(&rota, &c).send().await.expect("GET");
        // Uma aplicação pode abrir na sua secção por omissão (Conhecimento).
        if r.status().as_u16() == 303 {
            rota = location(&r);
        }
        let sep = if rota.contains('?') { '&' } else { '?' };
        let (status, html) = s.html(&format!("{rota}{sep}frame=1"), &c).await;
        assert_eq!(status, 200, "{rota}");
        // O corpo de uma aplicação do Design (a Nye tem a sua raiz própria).
        assert!(
            html.contains(r#"data-oc="app""#) || html.contains("oc-nye"),
            "{rota} sem ecrã"
        );
        assert!(
            !html.contains("oc-pending") && !html.contains("interface_pending"),
            "{rota} ainda é app_pending"
        );
    }
    // Tarefas continua a ser O Meu Trabalho.
    let r = s.get("/tasks/new", &c).send().await.expect("GET");
    assert_eq!(location(&r), "/my-work/new");
}

// ═════════════════════════════════════════════════════════════════════════
// Monitor
// ═════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn o_monitor_e_da_plataforma_e_nao_inventa_metricas() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let m = marca();
    // Um nó em linha que reporta memória, e outro sem sinal há muito.
    sqlx::query(
        "INSERT INTO compute_nodes (organisation_id, identifier, display_name, kind, status,
                                    last_seen_at, memory_bytes, memory_used_bytes)
         VALUES ($1, $2, $3, 'gpu', 'online', now() + interval '1 hour', 1000, 250),
                ($1, $4, $5, 'cpu', 'online', now() - interval '1 day', 1000, 900)",
    )
    .bind(s.organisation_id)
    .bind(format!("A{}", &m[..6]))
    .bind(format!("Vivo {m}"))
    .bind(format!("B{}", &m[..6]))
    .bind(format!("Antigo {m}"))
    .execute(&s.pool)
    .await
    .expect("nós");
    let (_, c) = com_mfa(&s, &[TechnicalRole::PlatformAdmin]).await;
    let (status, html) = s.html("/admin/monitor", &c).await;
    assert_eq!(status, 200);
    let app = regiao(&html, r#"data-part="monitor""#, "</article>");
    assert!(app.contains(r#"data-part="mon-summary""#));
    // Memória reportada; CPU, rede e GPU nomeadas como não reportadas.
    assert!(app.contains(&format!("Vivo {m}")) && app.contains(r#"data-fresh="live""#));
    assert!(
        app.contains(r#"data-fresh="stale""#),
        "o nó sem sinal não se diz antigo"
    );
    let naoreportado = regiao(app, r#"data-part="mon-unsupported""#, "</p>");
    for p in ["mon.plane.cpu", "mon.plane.network", "mon.plane.gpu"] {
        assert!(naoreportado.contains(pt(p)), "{p} não se diz não reportado");
    }
    assert!(!app.contains(r#"data-plane="cpu""#) && !app.contains(r#"data-plane="gpu""#));
    // Sem inventário de serviços: nada para parar, e nada que execute.
    assert!(app.contains(r#"data-part="mon-no-inventory""#));
    for proibido in ["<form", "stop_service", "confirm=", "<textarea", "<input"] {
        assert!(!app.contains(proibido), "{proibido}");
    }
    // Um plano inventado cai no primeiro reportado.
    let (status, html) = s.html("/admin/monitor?plane=gpu", &c).await;
    assert_eq!(status, 200);
    assert!(
        html.contains(r#"aria-current="page" data-plane="memory""#),
        "um plano inventado não cai no reportado"
    );
    // A administração da organização e um membro não o têm, nem por endereço.
    let (_, co) = s
        .membro_com_sessao(&[TechnicalRole::OrganisationAdmin])
        .await;
    let (_, cm) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    for c in [&co, &cm] {
        for rota in [
            "/admin/monitor",
            "/admin/monitor?frame=1",
            "/admin/monitor?plane=memory",
        ] {
            let (status, html) = s.html(rota, c).await;
            assert_eq!(status, 404, "{rota}");
            assert!(!html.contains(&format!("Vivo {m}")));
        }
        // E a Home não lhes liga ao Monitor.
        let (_, home) = s.html("/", c).await;
        assert!(!home.contains(r#"href="/admin/monitor""#));
    }
    // Não há rota que pare um serviço.
    let r = form(&s, &c, "/admin/monitor/x/stop", &[]).await;
    assert!(matches!(r.status().as_u16(), 404 | 405));
}

// ═════════════════════════════════════════════════════════════════════════
// Resultados
// ═════════════════════════════════════════════════════════════════════════

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

async fn com_papel_na_unidade(s: &Sistema, u: Uuid, role: &str) -> (Uuid, String) {
    let (id, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    sqlx::query("INSERT INTO unit_memberships (unit_id, person_id, role) VALUES ($1, $2, $3)")
        .bind(u)
        .bind(id)
        .bind(role)
        .execute(&s.pool)
        .await
        .expect("pertença");
    let (_, _, c) = s.entrar(&email, &password).await;
    (id, c)
}

async fn resultado(
    s: &Sistema,
    u: Uuid,
    ws: Uuid,
    titulo: &str,
    class: &str,
    sup: Option<Uuid>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO results (organisation_id, unit_id, workspace_id, title, summary, status,
                              classification, superseded_by_id)
         VALUES ($1, $2, $3, $4, 'Conclusão do trabalho.', 'under_review', $5, $6) RETURNING id",
    )
    .bind(s.organisation_id)
    .bind(u)
    .bind(ws)
    .bind(titulo)
    .bind(class)
    .bind(sup)
    .fetch_one(&s.pool)
    .await
    .expect("resultado")
}

#[tokio::test]
async fn um_resultado_e_as_suas_ligacoes_so_se_leem_com_autoridade() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let u = unidade(&s).await;
    let (_, gestor) = com_papel_na_unidade(&s, u, "manager").await;
    let (_, membro) = com_papel_na_unidade(&s, u, "member").await;
    let m = marca();
    // Um ambiente de ideia na unidade.
    let r = form(
        &s,
        &gestor,
        "/ideas/new",
        &[
            ("unit_id", &u.to_string()),
            ("classification", "INTERNAL"),
            ("title", &format!("Ambiente {m}")),
            ("summary", "s"),
            ("research_question", "q?"),
            ("hypothesis", "h"),
            ("motivation", "m"),
            ("keywords", "a"),
        ],
    )
    .await;
    assert_eq!(r.status().as_u16(), 303);
    let ideia = Uuid::parse_str(
        location(&r)
            .trim_start_matches("/ideas/")
            .split(['?', '/'])
            .next()
            .unwrap(),
    )
    .unwrap();
    let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM ideas WHERE id = $1")
        .bind(ideia)
        .fetch_one(&s.pool)
        .await
        .expect("ambiente");
    let restrito = resultado(&s, u, ws, &format!("Restrito {m}"), "RESTRICTED", None).await;
    let visivel = resultado(
        &s,
        u,
        ws,
        &format!("<img src=x onerror=alert(1)> Visível {m}"),
        "INTERNAL",
        Some(restrito),
    )
    .await;
    // O membro vê o interno na lista e no detalhe; o restrito não sai da base,
    // nem pela lista, nem pelo endereço, nem como substituto.
    let (status, html) = s.html("/results", &membro).await;
    assert_eq!(status, 200);
    assert!(html.contains(&format!("Visível {m}")));
    assert!(
        !html.contains(&format!("Restrito {m}")),
        "a lista trouxe um resultado restrito"
    );
    assert!(
        !html.contains("<img src=x"),
        "o título hostil não foi escapado"
    );
    let (status, html) = s.html(&format!("/results/{visivel}"), &membro).await;
    assert_eq!(status, 200);
    let d = regiao(&html, r#"data-part="result""#, "</article>");
    assert!(!d.contains(&format!("Restrito {m}")) && !d.contains(&restrito.to_string()));
    assert!(!d.contains(r#"data-part="result-superseded""#));
    // Sem «Novo resultado», e «Registar validação» sem formulário desenhado.
    assert!(!html.contains("/results/new") && !d.contains(pt("results.validate")));
    for rota in [
        format!("/results/{restrito}"),
        format!("/results/{}", Uuid::new_v4()),
    ] {
        let (status, html) = s.html(&rota, &membro).await;
        assert_eq!(status, 404, "{rota}");
        assert!(!html.contains(&format!("Restrito {m}")));
    }
    // Quem gere a unidade lê os dois, e a substituição aponta para o restrito.
    let (_, html) = s.html(&format!("/results/{visivel}"), &gestor).await;
    let d = regiao(&html, r#"data-part="result""#, "</article>");
    assert!(d.contains(r#"data-part="result-superseded""#));
    assert!(d.contains(&format!(r#"href="/results/{restrito}""#)));
    // Quem não vê projectos não tem Resultados.
    let (_, cc) = s.membro_com_sessao(&[TechnicalRole::Auditor]).await;
    let (status, _) = s.html("/results", &cc).await;
    assert_eq!(status, 404);
}

// ═════════════════════════════════════════════════════════════════════════
// Lixo
// ═════════════════════════════════════════════════════════════════════════

/// Uma nota apagada pelo próprio (pelo Core, com a sessão dele).
async fn nota_apagada(s: &Sistema, token: &str, titulo: &str) -> Uuid {
    let r = s
        .http
        .post(format!("{}/api/v1/me/notes", s.core_url))
        .bearer_auth(token)
        .json(&json!({
            "title": titulo,
            "document": { "schema_version": 1, "blocks": [{ "type": "paragraph", "content": [] }] }
        }))
        .send()
        .await
        .expect("criar nota");
    assert!(r.status().is_success());
    let id = Uuid::parse_str(r.json::<Value>().await.unwrap()["id"].as_str().unwrap()).unwrap();
    let r = s
        .http
        .delete(format!("{}/api/v1/me/notes/{id}", s.core_url))
        .bearer_auth(token)
        .send()
        .await
        .expect("apagar");
    assert!(r.status().is_success());
    id
}

/// Um ficheiro pessoal no Lixo (só as linhas: listar e restaurar não tocam no
/// objecto).
async fn ficheiro_apagado(s: &Sistema, dono: Uuid, nome: &str) -> Uuid {
    let backend: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_backends (code, kind, display_name, location_label, bucket, is_default, is_active)
         VALUES ('ocinye-test-default', 's3_compatible', 'Test', 'test', 'ocinye-test-artifacts', TRUE, TRUE)
         ON CONFLICT (code) DO UPDATE SET code = EXCLUDED.code RETURNING id",
    )
    .fetch_one(&s.pool)
    .await
    .expect("backend");
    let objecto: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_objects (backend_id, organisation_id, object_key, original_filename,
                                      content_type, size_bytes, checksum_sha256)
         VALUES ($1, $2, $3, $4, 'text/plain', 2048, repeat('a', 64)) RETURNING id",
    )
    .bind(backend)
    .bind(s.organisation_id)
    .bind(format!("t/{}", Uuid::new_v4()))
    .bind(nome)
    .fetch_one(&s.pool)
    .await
    .expect("objecto");
    let ficheiro: Uuid = sqlx::query_scalar(
        "INSERT INTO files (organisation_id, owner_id, name, classification, deleted_at)
         VALUES ($1, $2, $3, 'INTERNAL', now()) RETURNING id",
    )
    .bind(s.organisation_id)
    .bind(dono)
    .bind(nome)
    .fetch_one(&s.pool)
    .await
    .expect("ficheiro");
    sqlx::query(
        "INSERT INTO file_versions (file_id, sequence, storage_object_id) VALUES ($1, 1, $2)",
    )
    .bind(ficheiro)
    .bind(objecto)
    .execute(&s.pool)
    .await
    .expect("versão");
    ficheiro
}

#[tokio::test]
async fn o_lixo_e_do_proprio_restaura_pelo_dominio_e_nao_elimina() {
    let Some(s) = Sistema::levantar("research").await else {
        return;
    };
    let (a, ca, ta) = pessoa(&s, &[TechnicalRole::ResearchMember]).await;
    let (b, cb, tb) = pessoa(&s, &[TechnicalRole::ResearchMember]).await;
    let m = marca();
    let nota_a = nota_apagada(&s, &ta, &format!("<script>alert(1)</script> Nota A {m}")).await;
    let nota_b = nota_apagada(&s, &tb, &format!("Nota B {m}")).await;
    let fich_a = ficheiro_apagado(&s, a, &format!("relatorio-{m}.txt")).await;
    let fich_b = ficheiro_apagado(&s, b, &format!("alheio-{m}.txt")).await;
    // Cada um vê só o seu.
    let (status, html) = s.html("/trash", &ca).await;
    assert_eq!(status, 200);
    assert!(html.contains(&format!("Nota A {m}")) && html.contains(&format!("relatorio-{m}.txt")));
    assert!(!html.contains(&format!("Nota B {m}")) && !html.contains(&format!("alheio-{m}.txt")));
    assert!(
        !html.contains("<script>alert(1)"),
        "o título hostil não foi escapado"
    );
    // Nenhum prazo inventado; a eliminação definitiva nunca submete.
    let (status, html) = s.html(&format!("/trash?open=file:{fich_a}"), &ca).await;
    assert_eq!(status, 200);
    let d = regiao(&html, r#"data-part="trash-item""#, "</article>");
    for prazo in ["30 dias", "30 days", "30 jours", "expira", "expires"] {
        assert!(!d.contains(prazo), "prazo inventado: {prazo}");
    }
    assert!(d.contains(r#"data-part="trash-purge-unavailable""#) && d.contains(r#"disabled="""#));
    assert!(
        d.contains(r#"data-part="trash-storage""#),
        "o ficheiro no Lixo conta para o armazenamento"
    );
    // O item de outra pessoa não abre, nem por endereço.
    for alheio in [
        format!("file:{fich_b}"),
        format!("note:{nota_b}"),
        format!("note:{}", Uuid::new_v4()),
    ] {
        let (status, html) = s.html(&format!("/trash?open={alheio}"), &ca).await;
        assert_eq!(status, 404, "{alheio}");
        assert!(
            !html.contains(&format!("Nota B {m}")) && !html.contains(&format!("alheio-{m}.txt"))
        );
    }
    // Restaurar o de outra pessoa: o Core recusa, e continua no Lixo dele.
    let r = form(
        &s,
        &ca,
        "/trash/notes/restore",
        &[("id", &nota_b.to_string())],
    )
    .await;
    assert_eq!(location(&r), "/trash?refused=gone");
    let r = form(
        &s,
        &ca,
        "/trash/files/restore",
        &[("id", &fich_b.to_string())],
    )
    .await;
    assert_eq!(location(&r), "/trash?refused=gone");
    let (_, html) = s.html("/trash", &cb).await;
    assert!(html.contains(&format!("Nota B {m}")) && html.contains(&format!("alheio-{m}.txt")));
    // Restaurar o próprio: pela operação do domínio, com o aviso e o nome.
    let r = form(
        &s,
        &ca,
        "/trash/notes/restore",
        &[("id", &nota_a.to_string())],
    )
    .await;
    let destino = location(&r);
    assert!(destino.starts_with("/trash?done=restored"), "{destino}");
    let (_, html) = s.html(&destino, &ca).await;
    assert!(html.contains(r#"data-notice="trash.done.restored""#));
    assert!(
        !html.contains(&format!("open=note:{nota_a}")),
        "a nota restaurada continua no Lixo"
    );
    let r = form(
        &s,
        &ca,
        "/trash/files/restore",
        &[("id", &fich_a.to_string())],
    )
    .await;
    assert!(location(&r).starts_with("/trash?done=restored"));
    let (_, html) = s.html(&location(&r), &ca).await;
    assert!(
        html.contains(&format!("relatorio-{m}.txt")),
        "o nome do restaurado não se releu"
    );
    let apagado: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM files WHERE id = $1")
            .bind(fich_a)
            .fetch_one(&s.pool)
            .await
            .expect("ficheiro");
    assert!(apagado.is_none());
    // Os caminhos de eliminação definitiva sem confirmação já não existem.
    for (rota, campos) in [
        (format!("/notes/{nota_b}/eliminar"), vec![]),
        (
            "/me/files/purge".to_owned(),
            vec![("file_id", fich_b.to_string())],
        ),
        ("/files/trash/empty".to_owned(), vec![]),
    ] {
        let campos: Vec<(&str, &str)> = campos.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let r = form(&s, &cb, &rota, &campos).await;
        assert!(
            matches!(r.status().as_u16(), 404 | 405),
            "{rota}: {}",
            r.status()
        );
    }
    let (_, html) = s.html("/trash", &cb).await;
    assert!(
        html.contains(&format!("Nota B {m}")) && html.contains(&format!("alheio-{m}.txt")),
        "algo foi eliminado"
    );
    // As apagadas nas Notas abrem no Lixo, não no editor.
    let (_, html) = s.html("/notes/lixo", &cb).await;
    assert!(
        html.contains(&format!("/trash?nav=notes&amp;open=note:{nota_b}")),
        "a apagada abre no editor"
    );
}
