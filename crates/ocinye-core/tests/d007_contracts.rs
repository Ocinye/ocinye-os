//! D007 · Os contratos que a conclusão das aplicações pediu ao Core, cada um
//! provado contra o PostgreSQL real.
//!
//! - Um envio de mensagem com a mesma chave de idempotência, em paralelo, é
//!   **um** envio: o segundo espera pelo primeiro e devolve a mesma mensagem.
//! - As instruções de um agente só chegam a quem o criou.
//! - Os âmbitos onde o actor pode criar um agente são perguntados ao Core, pela
//!   mesma política da criação.
//! - O estado da IA diz, por capacidade, o motivo tipado de não servir — o
//!   mesmo que um pedido receberia.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida.

use std::collections::BTreeMap;
use std::time::Duration;

use ocinye_contracts::{AiCapability, AiReasonCode, Classification};
use ocinye_core::config::AiConfig;
use ocinye_core::modules::intelligence::agents::{self, AgentScope, NewAgent};
use ocinye_core::modules::messaging::{self, Outgoing};
use ocinye_core::realtime::Realtime;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL está definida mas a base não responde");
    ocinye_core::db::migrate(&pool).await.expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("d7{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organisation")
}

async fn pessoa(pool: &PgPool, org: Uuid, roles: &[&str]) -> ocinye_domain::Principal {
    let handle = format!("p{}", Uuid::new_v4().simple());
    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
              VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(org)
    .bind(&handle)
    .bind(format!("{handle}@ocinye.com"))
    .fetch_one(pool)
    .await
    .expect("person");
    for role in roles {
        sqlx::query(
            "INSERT INTO person_roles (person_id, role, granted_by_id) VALUES ($1, $2, $1)",
        )
        .bind(person_id)
        .bind(role)
        .execute(pool)
        .await
        .expect("papel");
    }
    principal(pool, person_id).await
}

async fn principal(pool: &PgPool, person_id: Uuid) -> ocinye_domain::Principal {
    ocinye_core::modules::identity::principal_for_person(
        pool,
        &ocinye_core::modules::identity::person_by_id(pool, person_id)
            .await
            .expect("ler")
            .expect("existe"),
    )
    .await
    .expect("principal")
}

// ── Mensagens ────────────────────────────────────────────────────────────

/// Dois envios com a mesma chave, em paralelo: o segundo espera pela tranca
/// do primeiro e devolve a mensagem que ele escreveu. Sem a tranca, o segundo
/// lia «não existe» e batia no índice único com um erro interno.
#[tokio::test]
async fn dois_envios_paralelos_com_a_mesma_chave_sao_um() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ana = pessoa(&pool, org, &["research_member"]).await;
    let dario = pessoa(&pool, org, &["research_member"]).await;
    let ids = CorrelationIds::generate();
    let conversa = messaging::open_direct(&pool, &ana, dario.person_id, &ids)
        .await
        .expect("abrir");

    // Uma transacção segura a tranca desta chave, como um primeiro envio a
    // meio: o segundo tem de esperar.
    let chave = "paralela";
    let mut primeiro = pool.begin().await.expect("tx");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!(
            "ocinye.messaging.send.{conversa}.{}.{chave}",
            ana.person_id
        ))
        .execute(&mut *primeiro)
        .await
        .expect("tranca");

    let (p, a) = (pool.clone(), ana.clone());
    let segundo = tokio::spawn(async move {
        let envio = Outgoing {
            body: "só uma vez",
            reply_to: None,
            mentions: &[],
            idempotency_key: Some("paralela"),
        };
        messaging::send(
            &p,
            &a,
            &Realtime::ausente(),
            conversa,
            &envio,
            &CorrelationIds::generate(),
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !segundo.is_finished(),
        "o segundo envio não esperou pela tranca"
    );

    // O primeiro escreve com a chave e confirma.
    let escrita: Uuid = sqlx::query_scalar(
        "INSERT INTO messages (conversation_id, author_id, body, idempotency_key)
         VALUES ($1, $2, 'só uma vez', $3) RETURNING id",
    )
    .bind(conversa)
    .bind(ana.person_id)
    .bind(chave)
    .fetch_one(&mut *primeiro)
    .await
    .expect("primeira escrita");
    primeiro.commit().await.expect("commit");

    let devolvida = segundo
        .await
        .expect("tarefa")
        .expect("o segundo envio falhou");
    assert_eq!(
        devolvida, escrita,
        "o segundo envio escreveu outra mensagem"
    );
    let quantas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM messages WHERE conversation_id = $1")
            .bind(conversa)
            .fetch_one(&pool)
            .await
            .expect("contagem");
    assert_eq!(quantas, 1);
}

// ── Agentes ──────────────────────────────────────────────────────────────

fn novo(nome: &str, scope: AgentScope, scope_id: Option<Uuid>) -> NewAgent {
    NewAgent {
        name: nome.to_owned(),
        purpose: Some("Resumir".to_owned()),
        instructions: Some("INSTRUCOES-DO-AUTOR".to_owned()),
        capability: AiCapability::General,
        scope,
        scope_id,
        max_classification: Classification::Internal,
        uses_bibliography: false,
        uses_documents: false,
        uses_datasets: false,
    }
}

async fn caps(pool: &PgPool, org: Uuid) -> ocinye_contracts::SystemCapabilities {
    ocinye_core::modules::platform::system_capabilities(
        pool,
        org,
        &ocinye_core::config::CoreConfig::from_env().unwrap_or_else(|_| {
            unsafe { std::env::set_var("OCINYE_DATABASE_URL", "postgres://x/x") };
            ocinye_core::config::CoreConfig::from_env().expect("config")
        }),
        false,
        ocinye_contracts::MailReachability::NotConfigured,
    )
    .await
    .expect("capacidades")
}

/// Ver um agente não é ler como o criador lhe disse que se comportasse.
#[tokio::test]
async fn as_instrucoes_so_chegam_a_quem_criou_o_agente() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let admin = pessoa(&pool, org, &["platform_admin"]).await;
    let outra = pessoa(&pool, org, &["research_member"]).await;
    let ids = CorrelationIds::generate();
    let id = agents::create(
        &pool,
        &admin,
        &novo("Institucional", AgentScope::Institutional, None),
        &ids,
    )
    .await
    .expect("criar");
    let c = caps(&pool, org).await;

    let visto_pelo_autor = agents::get(&pool, &admin, id, &c).await.expect("autor");
    assert_eq!(
        visto_pelo_autor.instructions.as_deref(),
        Some("INSTRUCOES-DO-AUTOR")
    );

    let visto_por_outra = agents::get(&pool, &outra, id, &c).await.expect("outra");
    assert_eq!(visto_por_outra.name, "Institucional");
    assert!(
        visto_por_outra.instructions.is_none(),
        "as instruções saíram"
    );
    let listados = agents::list(&pool, &outra, &c).await.expect("lista");
    let na_lista = listados.iter().find(|a| a.id == id).expect("na lista");
    assert!(
        na_lista.instructions.is_none(),
        "as instruções saíram pela lista"
    );
}

/// Os âmbitos oferecidos são os que a criação aceitaria — e só esses.
#[tokio::test]
async fn os_ambitos_criaveis_sao_os_da_politica_da_criacao() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();

    let membro = pessoa(&pool, org, &["research_member"]).await;
    let s = agents::creatable_scopes(&membro);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].scope, AgentScope::Personal);

    // Um gestor de unidade cria também para a unidade que gere — e só essa.
    let unidade: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, 'U') RETURNING id",
    )
    .bind(org)
    .bind(format!("U{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase())
    .fetch_one(&pool)
    .await
    .expect("unidade");
    let gestor = pessoa(&pool, org, &["research_member"]).await;
    sqlx::query(
        "INSERT INTO unit_memberships (unit_id, person_id, role) VALUES ($1, $2, 'manager')",
    )
    .bind(unidade)
    .bind(gestor.person_id)
    .execute(&pool)
    .await
    .expect("pertença");
    let gestor = principal(&pool, gestor.person_id).await;
    let s = agents::creatable_scopes(&gestor);
    let unit = s
        .iter()
        .find(|x| x.scope == AgentScope::Unit)
        .expect("unidade oferecida");
    assert_eq!(unit.targets, vec![unidade]);
    assert!(!s.iter().any(|x| x.scope == AgentScope::Institutional));

    // A plataforma cria institucionais.
    let admin = pessoa(&pool, org, &["platform_admin"]).await;
    let s = agents::creatable_scopes(&admin);
    assert!(s.iter().any(|x| x.scope == AgentScope::Institutional));

    // E cada âmbito oferecido passa a criação; um não oferecido não passa.
    for x in agents::creatable_scopes(&gestor) {
        let alvo = x.targets.first().copied();
        agents::create(
            &pool,
            &gestor,
            &novo(&format!("A {:?}", x.scope), x.scope, alvo),
            &ids,
        )
        .await
        .unwrap_or_else(|e| panic!("{:?} oferecido e recusado: {e}", x.scope));
    }
    assert!(agents::create(
        &pool,
        &membro,
        &novo("Forjado", AgentScope::Institutional, None),
        &ids
    )
    .await
    .is_err());
}

// ── Estado da IA ─────────────────────────────────────────────────────────

fn ai_config() -> AiConfig {
    AiConfig {
        capability_map: BTreeMap::new(),
        allow_external_providers: false,
        embedding_provider: "none".to_owned(),
    }
}

/// «Nada reportado» e «nada serve esta capacidade» são motivos diferentes, e
/// o estado diz o mesmo que um pedido receberia.
#[tokio::test]
async fn o_estado_da_ia_diz_o_motivo_de_cada_capacidade() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let membro = pessoa(&pool, org, &["research_member"]).await;
    let config = ai_config();

    let st = ocinye_core::modules::intelligence::intelligence_status(&pool, &membro, &config)
        .await
        .expect("estado");
    assert!(!st.available);
    assert_eq!(st.providers, 0);
    for c in &st.capabilities {
        assert!(!c.available);
        assert_eq!(
            c.reason,
            Some(AiReasonCode::AiNoProviderAvailable),
            "{:?}",
            c.capability
        );
    }

    // Um nó com dois modelos que servem GENERAL: um fornecedor, e as outras
    // capacidades sem modelo compatível.
    let identifier = format!("N{}", &Uuid::new_v4().simple().to_string()[..12]);
    let node: Uuid = sqlx::query_scalar(
        "INSERT INTO compute_nodes (organisation_id, identifier, display_name, status, last_seen_at)
         VALUES ($1, $2, $2, 'online', now()) RETURNING id",
    )
    .bind(org)
    .bind(&identifier)
    .fetch_one(&pool)
    .await
    .expect("nó");
    for m in ["modelo-a", "modelo-b"] {
        sqlx::query(
            "INSERT INTO ai_models
                 (provider_kind, provider_name, node_id, model_name, capabilities, status, reported_at)
             VALUES ('ocinye_node', $1, $2, $3, '[\"GENERAL\"]'::jsonb, 'available', now())",
        )
        .bind(&identifier)
        .bind(node)
        .bind(m)
        .execute(&pool)
        .await
        .expect("modelo");
    }
    let st = ocinye_core::modules::intelligence::intelligence_status(&pool, &membro, &config)
        .await
        .expect("estado");
    assert!(st.available);
    assert_eq!(
        st.providers, 1,
        "dois modelos do mesmo nó são um fornecedor"
    );
    for c in &st.capabilities {
        if c.capability == AiCapability::General {
            assert!(c.available && c.reason.is_none());
        } else {
            assert!(!c.available);
            assert_eq!(
                c.reason,
                Some(AiReasonCode::AiNoCompatibleModel),
                "{:?}",
                c.capability
            );
        }
    }
}
