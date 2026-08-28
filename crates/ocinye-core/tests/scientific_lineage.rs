//! A linhagem, e a fronteira que ela não pode revelar.
//!
//! # A propriedade que esta suite existe para guardar
//!
//! > **Uma fronteira de autorização escondida tem de ser indistinguível de uma
//! > folha visível.**
//!
//! Se um recurso da linhagem não é legível por quem percorre, a resposta não
//! pode dizer que ele existe — e «dizer» inclui coisas que não parecem dizer:
//! uma contagem diferente, uma forma diferente, um `truncada` diferente, um
//! marcador de continuação.
//!
//! «Este resultado depende de mais três coisas que não podes ver» já diz que há
//! três coisas, e a que unidade pertencem costuma deduzir-se do resto.

use ocinye_contracts::agentic::{ResourceKind as AgenticKind, ResourceRef};
use ocinye_contracts::provenance::ProvenanceRelation;
use ocinye_contracts::Classification;
use ocinye_core::modules::science::{self, Sentido};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

/// Salta quando não há base de dados; **falha** quando há e algo corre mal.
async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url).await.expect("base de dados");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    Some(pool)
}

/// Duas unidades, dois ambientes, duas pessoas que não se alcançam.
struct Mundo {
    pool: PgPool,
    /// Alcança a unidade A e não a B.
    dentro: Principal,
    /// O ambiente de A.
    ambiente_a: Uuid,
    /// O ambiente de B, que `dentro` não alcança.
    ambiente_b: Uuid,
}

async fn mundo(pool: &PgPool) -> Mundo {
    let marca = Uuid::new_v4().simple().to_string();

    let organisation_id: Uuid =
        sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
            .bind(format!("lin-{marca}"))
            .bind("Instituição da linhagem")
            .fetch_one(pool)
            .await
            .expect("organização");

    let unidade = |sufixo: String| {
        let codigo = format!("U{}{}", sufixo, &marca[..6]).to_uppercase();
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO units (organisation_id, code, name)
                      VALUES ($1, $2, $3) RETURNING id",
            )
            .bind(organisation_id)
            .bind(&codigo)
            .bind(format!("Unidade {sufixo}"))
            .fetch_one(&pool)
            .await
            .expect("unidade")
        }
    };
    let unidade_a = unidade("A".to_owned()).await;
    let unidade_b = unidade("B".to_owned()).await;

    let ambiente = |unit_id: Uuid, sufixo: String| {
        let codigo = format!("WS-{}-{}", sufixo, &marca[..6]).to_uppercase();
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO research_workspaces
                     (organisation_id, unit_id, code, title, kind, classification)
                  VALUES ($1, $2, $3, $4, 'project', 'INTERNAL') RETURNING id",
            )
            .bind(organisation_id)
            .bind(unit_id)
            .bind(&codigo)
            .bind(format!("Ambiente {sufixo}"))
            .fetch_one(&pool)
            .await
            .expect("ambiente")
        }
    };
    let ambiente_a = ambiente(unidade_a, "A".to_owned()).await;
    let ambiente_b = ambiente(unidade_b, "B".to_owned()).await;

    let dentro = pessoa(pool, organisation_id, unidade_a, ambiente_a).await;

    Mundo {
        pool: pool.clone(),
        dentro,
        ambiente_a,
        ambiente_b,
    }
}

async fn pessoa(
    pool: &PgPool,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
) -> Principal {
    let marca = format!("p{}", Uuid::new_v4().simple());

    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
              VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&marca)
    .bind(format!("{marca}@ocinye.com"))
    .fetch_one(pool)
    .await
    .expect("pessoa");

    for papel in ["research_member", "research_lead"] {
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
            .bind(person_id)
            .bind(papel)
            .execute(pool)
            .await
            .expect("papel");
    }

    sqlx::query(
        "INSERT INTO unit_memberships (unit_id, person_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(unit_id)
    .bind(person_id)
    .execute(pool)
    .await
    .expect("unidade");

    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role)
              VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(person_id)
    .execute(pool)
    .await
    .expect("ambiente");

    let registo = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &registo)
        .await
        .expect("principal")
}

/// Uma hipótese num ambiente.
async fn hipotese(m: &Mundo, ambiente: Uuid, quem: &Principal, texto: &str) -> Uuid {
    let mut tx = m.pool.begin().await.expect("transacção");
    let h = science::create_hypothesis(
        &mut tx,
        quem,
        &CorrelationIds::generate(),
        ambiente,
        texto,
        None,
        Classification::Internal,
    )
    .await
    .expect("hipótese");
    tx.commit().await.expect("commit");
    h.id
}

/// Uma aresta escrita directamente, para montar topologias que a operação
/// normal não produziria — incluindo as que atravessam a fronteira.
async fn aresta(
    m: &Mundo,
    organisation_id: Uuid,
    de: (AgenticKind, Uuid),
    verbo: ProvenanceRelation,
    para: (AgenticKind, Uuid),
    quem: Uuid,
) {
    sqlx::query(
        "INSERT INTO research_links
             (organisation_id, workspace_id, source_type_name, source_id, relation,
              target_type_name, target_id, created_by_id, origin)
         VALUES ($1, NULL, $2, $3, $4, $5, $6, $7, 'declared')",
    )
    .bind(organisation_id)
    .bind(de.0.as_str())
    .bind(de.1)
    .bind(verbo.as_str())
    .bind(para.0.as_str())
    .bind(para.1)
    .bind(quem)
    .execute(&m.pool)
    .await
    .expect("aresta");
}

/// Uma fronteira escondida é indistinguível de uma folha.
///
/// # O canal lateral que isto fecha
///
/// Duas topologias, do ponto de vista de quem percorre:
///
/// ```text
///   A  --supports-->  [hipótese de outra unidade]
///   B  (sem nada)
/// ```
///
/// As duas respostas têm de ser **iguais**: mesma contagem de passos, mesmo
/// `truncada`. Se diferirem em qualquer coisa, a forma do grafo passa a dizer
/// que existe um recurso que a política escondeu — e a unidade a que pertence
/// costuma deduzir-se do resto do contexto.
#[tokio::test]
async fn uma_fronteira_escondida_parece_uma_folha() {
    let Some(pool) = pool().await else { return };
    let m = mundo(&pool).await;
    let org = m.dentro.organisation_id;

    // Duas hipóteses no ambiente de quem percorre.
    let com_vizinho = hipotese(&m, m.ambiente_a, &m.dentro, "Tem um vizinho escondido").await;
    let sem_nada = hipotese(&m, m.ambiente_a, &m.dentro, "Não tem vizinho nenhum").await;

    // Uma terceira, escondida de verdade.
    //
    // Não basta estar noutra unidade: material `INTERNAL` é alcançável entre
    // unidades **por desenho**, e há um teste que o afirma. Uma hipótese
    // `RESTRICTED` no ambiente de B é o que a política recusa a quem não tem
    // concessão explícita — e foi este teste que mo mostrou, ao contar um
    // passo onde devia contar zero.
    let unidade_b: Uuid =
        sqlx::query_scalar("SELECT unit_id FROM research_workspaces WHERE id = $1")
            .bind(m.ambiente_b)
            .fetch_one(&pool)
            .await
            .expect("unidade");

    let outra_unidade: Uuid = sqlx::query_scalar(
        "INSERT INTO hypotheses
             (organisation_id, unit_id, workspace_id, statement, classification)
         VALUES ($1, $2, $3, $4, 'RESTRICTED') RETURNING id",
    )
    .bind(org)
    .bind(unidade_b)
    .bind(m.ambiente_b)
    .bind("Invisível para a unidade A")
    .fetch_one(&pool)
    .await
    .expect("hipótese restrita");

    // E, **para lá** da escondida, uma que `dentro` alcançaria.
    //
    // É esta que torna o teste capaz de medir o que afirma. Sem um neto
    // visível, atravessar por trás do nó oculto não mostra nada, e uma
    // travessia que o atravessasse passaria à mesma: o teste dizia «não
    // revela» e o que observava era «não havia nada para revelar».
    let neto_visivel = hipotese(&m, m.ambiente_a, &m.dentro, "Do outro lado da fronteira").await;

    // A primeira aponta para a escondida, e a escondida para o neto.
    aresta(
        &m,
        org,
        (AgenticKind::Hypothesis, com_vizinho),
        ProvenanceRelation::RelatesTo,
        (AgenticKind::Hypothesis, outra_unidade),
        m.dentro.person_id,
    )
    .await;
    aresta(
        &m,
        org,
        (AgenticKind::Hypothesis, outra_unidade),
        ProvenanceRelation::RelatesTo,
        (AgenticKind::Hypothesis, neto_visivel),
        m.dentro.person_id,
    )
    .await;

    let referencia = |id| ResourceRef {
        kind: AgenticKind::Hypothesis,
        id,
        label: None,
    };

    let com = science::percorrer(
        &pool,
        &m.dentro,
        &referencia(com_vizinho),
        Sentido::Montante,
        3,
    )
    .await
    .expect("percorrer");

    let sem = science::percorrer(
        &pool,
        &m.dentro,
        &referencia(sem_nada),
        Sentido::Montante,
        3,
    )
    .await
    .expect("percorrer");

    assert_eq!(
        com.passos.len(),
        sem.passos.len(),
        "a contagem de passos revela que existe um recurso escondido: \
         com vizinho = {}, sem vizinho = {}",
        com.passos.len(),
        sem.passos.len()
    );
    assert_eq!(
        com.truncada, sem.truncada,
        "`truncada` revela que existe alguma coisa para lá da fronteira"
    );

    // E nada sobre o recurso escondido aparece na resposta.
    let texto = serde_json::to_string(&com).expect("serializar");
    assert!(
        !texto.contains(&outra_unidade.to_string()),
        "o identificador do recurso escondido apareceu na resposta"
    );
    assert!(
        !texto.contains("Invisível"),
        "o título do recurso escondido apareceu na resposta"
    );

    // E o que está **para lá** da fronteira também não aparece.
    //
    // Atravessar por trás de um nó oculto para mostrar o que vem depois dele
    // revela que ele existe — «isto depende daquilo, que depende de algo que
    // não te mostro» é a mesma informação por outras palavras.
    assert!(
        !texto.contains(&neto_visivel.to_string()),
        "a travessia passou por trás do nó oculto e mostrou o que vem depois"
    );
}

/// O controlo positivo: a linhagem visível **aparece**.
///
/// Sem ele, uma travessia que devolvesse sempre nada passaria em todos os
/// testes de fuga acima — e a linhagem deixaria de servir para o que existe.
#[tokio::test]
async fn a_linhagem_visivel_aparece() {
    let Some(pool) = pool().await else { return };
    let m = mundo(&pool).await;
    let org = m.dentro.organisation_id;

    let primeira = hipotese(&m, m.ambiente_a, &m.dentro, "A primeira").await;
    let segunda = hipotese(&m, m.ambiente_a, &m.dentro, "A segunda").await;

    aresta(
        &m,
        org,
        (AgenticKind::Hypothesis, primeira),
        ProvenanceRelation::RelatesTo,
        (AgenticKind::Hypothesis, segunda),
        m.dentro.person_id,
    )
    .await;

    let linhagem = science::percorrer(
        &pool,
        &m.dentro,
        &ResourceRef {
            kind: AgenticKind::Hypothesis,
            id: primeira,
            label: None,
        },
        Sentido::Montante,
        3,
    )
    .await
    .expect("percorrer");

    assert_eq!(linhagem.passos.len(), 1, "a relação visível não apareceu");
    assert_eq!(linhagem.passos[0].para.id, segunda);
    assert_eq!(linhagem.passos[0].relacao_legivel, "relaciona-se com");
    assert!(!linhagem.truncada);
}

/// Um ciclo não prende a travessia.
///
/// A proveniência científica forma ciclos legítimos: um resultado sustenta uma
/// hipótese, que gera um estudo, que produz outro resultado. Uma travessia sem
/// memória andaria à volta deles para sempre — e «para sempre» num pedido HTTP
/// é uma indisponibilidade.
#[tokio::test]
async fn um_ciclo_nao_prende_a_travessia() {
    let Some(pool) = pool().await else { return };
    let m = mundo(&pool).await;
    let org = m.dentro.organisation_id;

    let a = hipotese(&m, m.ambiente_a, &m.dentro, "A").await;
    let b = hipotese(&m, m.ambiente_a, &m.dentro, "B").await;

    // A → B e B → A.
    aresta(
        &m,
        org,
        (AgenticKind::Hypothesis, a),
        ProvenanceRelation::RelatesTo,
        (AgenticKind::Hypothesis, b),
        m.dentro.person_id,
    )
    .await;
    aresta(
        &m,
        org,
        (AgenticKind::Hypothesis, b),
        ProvenanceRelation::RelatesTo,
        (AgenticKind::Hypothesis, a),
        m.dentro.person_id,
    )
    .await;

    let linhagem = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        science::percorrer(
            &pool,
            &m.dentro,
            &ResourceRef {
                kind: AgenticKind::Hypothesis,
                id: a,
                label: None,
            },
            Sentido::Montante,
            5,
        ),
    )
    .await
    .expect("a travessia ficou presa num ciclo")
    .expect("percorrer");

    // B aparece uma vez, e A não volta a aparecer.
    assert_eq!(
        linhagem.passos.len(),
        1,
        "o ciclo produziu passos repetidos: {:?}",
        linhagem
            .passos
            .iter()
            .map(|p| p.para.id)
            .collect::<Vec<_>>()
    );
}
