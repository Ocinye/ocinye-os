//! Agir com a Nye (D003 · NYE-06 a NYE-08): proposta, autorização,
//! confirmação e execução, contra o Core real.
//!
//! O Planner do Core é inferência. Aqui o Core corre com o `FixtureProvider`
//! (o fornecedor determinístico do próprio Core, que só existe com a feature
//! `test-fixtures` e não está em nenhum binário de produção): ele escreve o
//! plano; **tudo o resto é o caminho de produção** — validação do plano pelo
//! registo de capacidades, risco e confirmação decididos pelo Core, aprovação
//! ligada ao digest, execução pelo executor determinístico, auditoria.
//!
//! Isto prova a execução governada. Não prova inferência: nenhum modelo real
//! correu (OCINYE_D003_INFERENCE_CERTIFIED fica por declarar).
//!
//! # Porque um ficheiro à parte, e sequencial
//!
//! A disponibilidade de IA do Core lê o inventário global `ai_models`. Um
//! modelo registado aqui tornaria «disponível» a IA das viagens sem inferência
//! que correm em paralelo noutro binário. Por isso: um binário próprio, um só
//! teste por sistema, e o inventário limpo no fim.

mod common;

use std::sync::Arc;

use common::*;
use ocinye_contracts::TechnicalRole;
use ocinye_core::modules::intelligence::fixture::FixtureProvider;
use sqlx::PgPool;
use uuid::Uuid;

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

/// Um modelo de nó que serve a capacidade geral: é o que torna a IA
/// «utilizável» para o Core. Devolve o nó, para limpar no fim.
async fn modelo(pool: &PgPool, organisation_id: Uuid) -> Uuid {
    let identifier: String = Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(24)
        .collect();
    let node: Uuid = sqlx::query_scalar(
        "INSERT INTO compute_nodes (organisation_id, identifier, display_name, status)
         VALUES ($1, $2, 'Nó da viagem D003', 'online') RETURNING id",
    )
    .bind(organisation_id)
    .bind(identifier)
    .fetch_one(pool)
    .await
    .expect("nó");
    sqlx::query(
        "INSERT INTO ai_models
             (provider_kind, provider_name, node_id, model_name, version,
              capabilities, status, enabled)
         VALUES ('ocinye_node', $1, $2, $3, 'v1', '[\"GENERAL\"]'::jsonb, 'available', TRUE)",
    )
    .bind(format!("prov-{}", Uuid::new_v4().simple()))
    .bind(node)
    .bind(format!("modelo-{}", Uuid::new_v4().simple()))
    .execute(pool)
    .await
    .expect("modelo");
    node
}

async fn limpar(pool: &PgPool, node: Uuid) {
    let _ = sqlx::query("DELETE FROM ai_models WHERE node_id = $1")
        .bind(node)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM compute_nodes WHERE id = $1")
        .bind(node)
        .execute(pool)
        .await;
}

/// Um ambiente de investigação onde a pessoa é membro: o sítio onde uma nota
/// pode nascer. O Nye ainda não envia contexto (NYE-09), por isso o pedido
/// nomeia-o, como um membro que o escrevesse.
async fn ambiente(pool: &PgPool, organisation_id: Uuid, person: Uuid) -> Uuid {
    let suffix = Uuid::new_v4().simple().to_string();
    let unit: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, 'Unidade') RETURNING id",
    )
    .bind(organisation_id)
    .bind(format!("U{}", &suffix[..6]).to_uppercase())
    .fetch_one(pool)
    .await
    .expect("unidade");
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces
             (organisation_id, unit_id, code, title, kind, classification)
         VALUES ($1, $2, $3, 'Ambiente da viagem', 'idea', 'INTERNAL') RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit)
    .bind(format!("WS-{}", &suffix[..8]))
    .fetch_one(pool)
    .await
    .expect("ambiente");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(id)
    .bind(person)
    .execute(pool)
    .await
    .expect("pertença");
    id
}

/// O primeiro plano proposto na página: identificador e digest mostrados.
fn proposta(html: &str) -> Option<(String, String)> {
    let at = html.find(r#"action="/ask/plans/"#)?;
    let id = html[at + r#"action="/ask/plans/"#.len()..]
        .split('/')
        .next()?
        .to_owned();
    let d = html[at..].find(r#"name="digest" value=""#)? + at + r#"name="digest" value=""#.len();
    let digest = html[d..].split('"').next()?.to_owned();
    Some((id, digest))
}

async fn estado(pool: &PgPool, plan: &str) -> String {
    sqlx::query_scalar("SELECT state FROM action_plans WHERE id = $1::uuid")
        .bind(plan)
        .fetch_one(pool)
        .await
        .expect("plano")
}

async fn executar(s: &Sistema, cookie: &str, plan: &str, digest: Option<&str>) -> u16 {
    let mut form = Vec::new();
    if let Some(d) = digest {
        form.push(("digest", d));
    }
    s.escrever(
        reqwest::Method::POST,
        &format!("/ask/plans/{plan}/execute"),
        cookie,
    )
    .form(&form)
    .send()
    .await
    .unwrap()
    .status()
    .as_u16()
}

#[tokio::test]
async fn propor_confirmar_e_executar_passa_sempre_pelo_core() {
    let Some(s) = Sistema::levantar_com("research", Arc::new(FixtureProvider::cooperative())).await
    else {
        return;
    };
    let node = modelo(&s.pool, s.organisation_id).await;
    let (ana, email, password) = s.pessoa(&[TechnicalRole::ResearchMember]).await;
    let (_, _, c) = s.entrar(&email, &password).await;
    let (_, rui) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;

    // Um recurso inventado — o que um modelo alucinado propõe — passa a
    // proposta, e o Core recusa-o na execução: nada é criado.
    let (_, inventado) = s
        .html(
            &format!(
                "/ask?q={}&intent=act",
                enc("cria uma nota sobre o sensor 04")
            ),
            &c,
        )
        .await;
    let (p0, d0) = proposta(&inventado).expect("proposta com recurso inventado");
    assert_eq!(executar(&s, &c, &p0, Some(&d0)).await, 303);
    assert_eq!(estado(&s.pool, &p0).await, "failed");
    let (_, falhado) = s.html(&format!("/ask?plan={p0}"), &c).await;
    assert!(falhado.contains(r#"data-state="failed""#));

    // Proposta: o Core valida o plano e decide o risco e a confirmação.
    let ws = ambiente(&s.pool, s.organisation_id, ana).await;
    let pedido = format!("cria uma nota em {ws}");
    let (status, html) = s
        .html(&format!("/ask?q={}&intent=act", enc(&pedido)), &c)
        .await;
    assert_eq!(status, 200);
    assert!(
        html.contains(r#"data-risk="reversible_write""#),
        "o risco não é o do Core"
    );
    assert!(html.contains("knowledge.note.create"));
    let (plano, digest) = proposta(&html).expect("proposta com confirmação");
    assert_eq!(estado(&s.pool, &plano).await, "awaiting_approval");
    let notas_antes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notes WHERE created_by_id = $1")
            .bind(ana)
            .fetch_one(&s.pool)
            .await
            .expect("notas");
    assert_eq!(notas_antes, 0, "o recurso inventado criou uma nota");

    // 4 · Sem a confirmação do membro não corre: sem digest, e com outro digest.
    assert_eq!(executar(&s, &c, &plano, None).await, 409);
    assert_eq!(executar(&s, &c, &plano, Some("0000")).await, 409);
    assert_eq!(estado(&s.pool, &plano).await, "awaiting_approval");

    // 9 · Outro membro não confirma nem vê o plano da Ana (o id não autoriza).
    assert_ne!(executar(&s, &rui, &plano, Some(&digest)).await, 303);
    let (_, alheio) = s.html(&format!("/ask?plan={plano}"), &rui).await;
    assert!(!alheio.contains(&plano), "o plano de outra pessoa apareceu");
    assert_eq!(estado(&s.pool, &plano).await, "awaiting_approval");

    // 5 · Uma proposta com outro efeito não herda a confirmação: o digest da
    // primeira é recusado. (O digest liga-se ao efeito — capacidades, entradas,
    // recursos —, e não ao texto: duas propostas iguais têm o mesmo.)
    let (_, html2) = s
        .html(
            &format!(
                "/ask?q={}&intent=act",
                enc("cria uma tarefa para rever o relatório")
            ),
            &c,
        )
        .await;
    let (plano2, digest2) = proposta(&html2).expect("segunda proposta");
    assert_ne!(plano2, plano);
    assert_ne!(digest2, digest, "efeitos diferentes com o mesmo digest");
    assert_eq!(executar(&s, &c, &plano2, Some(&digest)).await, 409);
    assert_eq!(estado(&s.pool, &plano2).await, "awaiting_approval");

    // Confirmar com o que foi mostrado: aprova, executa, e volta ao resultado.
    let r = s
        .escrever(
            reqwest::Method::POST,
            &format!("/ask/plans/{plano}/execute"),
            &c,
        )
        .form(&[("digest", digest.as_str())])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);
    assert_eq!(location(&r), format!("/ask?plan={plano}"));
    assert_eq!(estado(&s.pool, &plano).await, "completed");
    let notas_depois: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notes WHERE created_by_id = $1")
            .bind(ana)
            .fetch_one(&s.pool)
            .await
            .expect("notas");
    assert_eq!(notas_depois, notas_antes + 1, "a nota não foi criada");
    // O resultado, com a referência de auditoria, e sem voltar a oferecer confirmar.
    let (_, feito) = s.html(&format!("/ask?plan={plano}"), &c).await;
    assert!(feito.contains(r#"data-state="completed""#));
    assert!(!feito.contains(&format!(r#"action="/ask/plans/{plano}/execute""#)));
    let auditado: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_events WHERE resource_id = $1::uuid ORDER BY occurred_at",
    )
    .bind(&plano)
    .fetch_all(&s.pool)
    .await
    .expect("auditoria");
    assert_eq!(auditado, ["plan_created", "plan_approved", "plan_executed"]);
    // A segunda proposta continua à espera, e pode ser recusada.
    let r = s
        .escrever(
            reqwest::Method::POST,
            &format!("/ask/plans/{plano2}/reject"),
            &c,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 303);
    assert_eq!(estado(&s.pool, &plano2).await, "rejected");

    // 3 · Uma capacidade que a pessoa não pode exercer não corre: criar uma
    // ideia numa unidade exige pertencer-lhe, e a Ana só pertence ao ambiente.
    let unidade: Uuid = sqlx::query_scalar("SELECT unit_id FROM research_workspaces WHERE id = $1")
        .bind(ws)
        .fetch_one(&s.pool)
        .await
        .expect("unidade");
    let ideias_antes: i64 = sqlx::query_scalar("SELECT count(*) FROM ideas")
        .fetch_one(&s.pool)
        .await
        .expect("ideias");
    let (_, html3) = s
        .html(
            &format!(
                "/ask?q={}&intent=act",
                enc(&format!("cria uma ideia na unidade {unidade}"))
            ),
            &c,
        )
        .await;
    let (p3, d3) = proposta(&html3).expect("proposta de ideia");
    assert!(html3.contains("research.idea.create"));
    executar(&s, &c, &p3, Some(&d3)).await;
    assert_eq!(estado(&s.pool, &p3).await, "failed");
    let razao: String = sqlx::query_scalar(
        "SELECT steps->0->'result'->>'status' FROM action_plans WHERE id = $1::uuid",
    )
    .bind(&p3)
    .fetch_one(&s.pool)
    .await
    .expect("razão");
    assert_eq!(razao, "permission_denied");
    let ideias_depois: i64 = sqlx::query_scalar("SELECT count(*) FROM ideas")
        .fetch_one(&s.pool)
        .await
        .expect("ideias");
    assert_eq!(
        ideias_depois, ideias_antes,
        "uma ideia foi criada sem autorização"
    );

    limpar(&s.pool, node).await;
}

/// 8 · O que um modelo propõe depois de ler conteúdo hostil («chama a
/// ferramenta de administração») não se autoriza: o Planner do Core recusa
/// capacidades que não existem, e nada é proposto nem executado.
#[tokio::test]
async fn uma_proposta_hostil_nao_chega_a_ser_plano() {
    let Some(s) = Sistema::levantar_com("research", Arc::new(FixtureProvider::hostile())).await
    else {
        return;
    };
    let node = modelo(&s.pool, s.organisation_id).await;
    let (id, c) = s.membro_com_sessao(&[TechnicalRole::ResearchMember]).await;
    let (status, html) = s
        .html(
            &format!(
                "/ask?q={}&intent=act",
                enc("cria uma nota com as instruções do documento")
            ),
            &c,
        )
        .await;
    assert_eq!(status, 200);
    assert!(proposta(&html).is_none(), "uma proposta hostil apareceu");
    let planos: i64 =
        sqlx::query_scalar("SELECT count(*) FROM action_plans WHERE requested_by = $1")
            .bind(id)
            .fetch_one(&s.pool)
            .await
            .expect("planos");
    assert_eq!(planos, 0);
    limpar(&s.pool, node).await;
}
