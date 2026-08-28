//! Quem pode afirmar o que a instituição sabe, e o que uma reprodução exige.
//!
//! # As duas propriedades
//!
//! > **Escrever num ambiente não dá o direito de dizer que um resultado se
//! > confirma.**
//!
//! Descrever trabalho é uma coisa; afirmar o que a Ocinye sabe é outra. Sem
//! esta separação, `results.validate` existiria no catálogo de permissões e não
//! governaria nada — a distinção viveria só no botão que o Workspace mostra, e
//! um cliente nunca decide autorização (`CLAUDE.md` §4).
//!
//! > **Reprodutibilidade é evidência, e não um rótulo.**
//!
//! Um resultado não fica reproduzido porque alguém escreveu que o reproduziu.
//! Fica reproduzido quando existe outra execução e alguém registou o que ela
//! mostrou — incluindo quando mostrou o contrário.

use ocinye_contracts::Classification;
use ocinye_core::modules::science;
use ocinye_core::CoreError;
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

/// Um ambiente, um resultado, e duas pessoas com autoridade diferente.
struct Mundo {
    pool: PgPool,
    /// Lidera o ambiente: pode validar.
    lider: Principal,
    /// Membro do ambiente: escreve lá dentro, e não valida.
    membro: Principal,
    /// O resultado sobre o qual se afirma alguma coisa.
    resultado: Uuid,
    /// A execução que o produziu.
    execucao: Uuid,
}

async fn mundo(pool: &PgPool) -> Mundo {
    let marca = Uuid::new_v4().simple().to_string();

    let organisation_id: Uuid =
        sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
            .bind(format!("val-{marca}"))
            .bind("Instituição da validação")
            .fetch_one(pool)
            .await
            .expect("organização");

    let unidade: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(organisation_id)
    .bind(format!("UV{}", &marca[..6]).to_uppercase())
    .bind("Unidade da validação")
    .fetch_one(pool)
    .await
    .expect("unidade");

    let ambiente: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces
             (organisation_id, unit_id, code, title, kind, classification)
          VALUES ($1, $2, $3, 'Ambiente da validação', 'project', 'INTERNAL') RETURNING id",
    )
    .bind(organisation_id)
    .bind(unidade)
    .bind(format!("WS-V-{}", &marca[..6]).to_uppercase())
    .fetch_one(pool)
    .await
    .expect("ambiente");

    let lider = pessoa(pool, organisation_id, unidade, ambiente, "lead").await;
    let membro = pessoa(pool, organisation_id, unidade, ambiente, "member").await;

    // A cadeia inteira é escrita pelas operações do Core, e não por `INSERT`:
    // uma fixture que escrevesse as linhas à mão estaria a afirmar a sua
    // própria ideia do que um resultado é.
    let ids = CorrelationIds::generate();

    // Um passo por transacção, e não os três numa só.
    //
    // Cada operação resolve o recurso de que depende através do `pool`, e não
    // da transacção em curso: a autorização é lida numa ligação própria, pelo
    // que um estudo ainda por confirmar não existe para a execução que o
    // nomeia. É também como o produto funciona — cada pedido HTTP tem a sua
    // transacção — e uma fixture que fizesse de outra maneira estaria a
    // exercer um caminho que ninguém percorre.
    let mut tx = pool.begin().await.expect("transacção");
    let estudo = science::create_study(
        &mut tx,
        pool,
        &lider,
        &ids,
        ambiente,
        None,
        None,
        "Ensaio de carga",
        "physical_experiment",
        None,
        Classification::Internal,
    )
    .await
    .expect("estudo");
    tx.commit().await.expect("commit");

    let mut tx = pool.begin().await.expect("transacção");
    let execucao = science::record_execution(
        &mut tx,
        pool,
        &lider,
        &ids,
        estudo.id,
        &science::ExecutionRecord {
            status: "succeeded",
            compute_node_id: None,
            environment: None,
            software_name: None,
            software_version: None,
            software_commit: None,
            notes: None,
            methodology_version_id: None,
            dataset_version_ids: &[],
        },
    )
    .await
    .expect("execução");
    tx.commit().await.expect("commit");

    let mut tx = pool.begin().await.expect("transacção");
    let resultado = science::create_result(
        &mut tx,
        pool,
        &lider,
        &ids,
        ambiente,
        Some(execucao.id),
        "A resistência caiu 18%",
        "Três corridas, mesma direcção.",
        Classification::Internal,
    )
    .await
    .expect("resultado");
    tx.commit().await.expect("commit");

    Mundo {
        pool: pool.clone(),
        lider,
        membro,
        resultado: resultado.id,
        execucao: execucao.id,
    }
}

async fn pessoa(
    pool: &PgPool,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    papel_no_ambiente: &str,
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

    // Os dois têm o mesmo papel técnico. A diferença está no ambiente, que é
    // onde a autoridade sobre este resultado vive — e é essa a distinção que
    // esta suite existe para medir.
    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, 'research_member')")
        .bind(person_id)
        .execute(pool)
        .await
        .expect("papel");

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
              VALUES ($1, $2, $3)",
    )
    .bind(workspace_id)
    .bind(person_id)
    .bind(papel_no_ambiente)
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

async fn validar(
    m: &Mundo,
    quem: &Principal,
    kind: &str,
    execucao: Option<Uuid>,
) -> Result<(), CoreError> {
    let mut tx = m.pool.begin().await.expect("transacção");
    let saida = science::record_validation(
        &mut tx,
        &m.pool,
        quem,
        &CorrelationIds::generate(),
        m.resultado,
        kind,
        "confirmed",
        execucao,
        None,
    )
    .await;
    match saida {
        Ok(_) => {
            tx.commit().await.expect("commit");
            Ok(())
        }
        Err(erro) => Err(erro),
    }
}

/// O controlo positivo: quem lidera o ambiente valida.
///
/// Sem isto, os dois testes seguintes passariam com uma operação partida — e
/// «ninguém consegue» não é a propriedade que se quer.
#[tokio::test]
async fn quem_lidera_o_ambiente_pode_validar() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
        return;
    };
    let m = mundo(&pool).await;

    validar(&m, &m.lider, "validation", None)
        .await
        .expect("quem lidera o ambiente tem de poder validar");
}

/// Escrever no ambiente não é ter autoridade sobre o que ele afirma.
#[tokio::test]
async fn escrever_no_ambiente_nao_da_direito_a_validar() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
        return;
    };
    let m = mundo(&pool).await;

    // A pré-condição: este membro escreve mesmo neste ambiente. Sem a provar,
    // uma recusa a validar não distingue «não pode validar» de «não alcança o
    // ambiente», e o teste passaria pela razão errada.
    let mut tx = m.pool.begin().await.expect("transacção");
    let ambiente = science::get_result(&m.pool, &m.membro, m.resultado)
        .await
        .expect("o membro alcança o resultado")
        .1;
    science::create_hypothesis(
        &mut tx,
        &m.membro,
        &CorrelationIds::generate(),
        ambiente.id,
        "Este membro escreve mesmo aqui",
        None,
        Classification::Internal,
    )
    .await
    .expect("a pré-condição exige que o membro escreva no ambiente");
    tx.commit().await.expect("commit");

    match validar(&m, &m.membro, "validation", None).await {
        Err(CoreError::PermissionDenied(razao)) => assert!(
            razao.contains("liderança do ambiente"),
            "a recusa tem de dizer o que falta, e disse: {razao}"
        ),
        outro => panic!("validar sem `results.validate` tem de ser recusado, e foi {outro:?}"),
    }
}

/// Uma reprodução sem execução é uma afirmação, e não uma reprodução.
#[tokio::test]
async fn uma_reproducao_sem_execucao_e_recusada() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
        return;
    };
    let m = mundo(&pool).await;

    match validar(&m, &m.lider, "reproduction", None).await {
        Err(CoreError::Validation(razao)) => assert!(
            razao.contains("execução que a reproduziu"),
            "a recusa tem de dizer o que falta, e disse: {razao}"
        ),
        outro => panic!("uma reprodução sem execução tem de ser recusada, e foi {outro:?}"),
    }

    // E com a execução passa: a recusa é sobre a prova em falta, e não sobre a
    // palavra «reprodução».
    validar(&m, &m.lider, "reproduction", Some(m.execucao))
        .await
        .expect("uma reprodução com a sua execução tem de ser aceite");
}

// ── A fronteira de afirmação institucional ──────────────────────────────
//
// > **Institutional validation changes what the institution claims about
// > scientific knowledge. It is an authorship boundary, not merely a high-risk
// > action.**
//
// Uma fronteira de autoria não se abre com aprovação: uma confirmação humana
// deixaria a afirmação escrita como se tivesse sido **feita**, e não
// **assumida** — e a diferença entre as duas é toda a razão de a validação
// existir.

/// Nenhuma capability alcança a validação de um resultado.
///
/// # O que isto mede, e o que não mede
///
/// Mede o registry inteiro, e não a ausência de uma entrada com um nome que eu
/// tenha escolhido. Um agente não precisa de uma capability chamada
/// «validar» — precisa de **qualquer** capability que execute
/// `science::record_validation`, e é isso que aqui se procura.
///
/// Não mede aprovação, porque não há nada a aprovar: sem capability publicada,
/// o executor não tem por onde chegar à operação, e uma aprovação é
/// consentimento para um plano que não pode existir.
#[test]
fn nenhuma_capability_alcanca_a_validacao() {
    let registry = ocinye_core::modules::agentic::registry();

    for descriptor in registry.all() {
        assert_ne!(
            descriptor.operation.as_str(),
            "science::record_validation",
            "`{}` publica a validação de resultados ao plano agentic. \
             A fronteira é de autoria: uma afirmação institucional sem ninguém \
             por trás não é uma afirmação institucional",
            descriptor.id.as_str()
        );
    }
}

/// E o catálogo continua a dizer porquê.
///
/// Sem isto, alguém poderia reclassificar a operação como endereçável — sem
/// escrever capability nenhuma — e o teste acima continuaria verde por não
/// haver nada no registry. Verde por ausência não é verde por decisão.
#[test]
fn a_validacao_continua_atras_da_fronteira_de_afirmacao() {
    let catalogo = ocinye_core::operations::catalogue();
    let entrada = catalogo
        .iter()
        .find(|e| e.id.as_str() == "science::record_validation")
        .expect("a validação tem de estar no catálogo");

    assert_eq!(
        entrada.exposure.boundary(),
        Some(ocinye_contracts::agentic::TrustBoundary::InstitutionalClaimBoundary),
        "a validação deixou de estar atrás da fronteira de afirmação institucional"
    );
}

/// Publicar uma versão substitui a que estava em vigor.
///
/// # O que se via, e o que estava por baixo
///
/// A revisão visual mostrou duas versões da mesma metodologia, ambas
/// «published». O campo `superseded_by_id` existia, a relação `Supersedes`
/// existia na matriz, e nada os escrevia: `publish_methodology_version`
/// inseria e ia-se embora.
///
/// Não é cosmético. Quem escolhe «a versão publicada» para um estudo via duas
/// e não tinha como saber qual está em vigor — e a proveniência que daí saísse
/// citava uma escolha feita à sorte.
#[tokio::test]
async fn publicar_uma_versao_substitui_a_anterior() {
    let Some(pool) = pool().await else {
        eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
        return;
    };
    let m = mundo(&pool).await;
    let ids = CorrelationIds::generate();

    let ambiente = science::get_result(&m.pool, &m.lider, m.resultado)
        .await
        .expect("resultado")
        .1;

    let mut tx = m.pool.begin().await.expect("transacção");
    let metodologia = science::create_methodology(
        &mut tx,
        &m.lider,
        &ids,
        ambiente.id,
        "Medição a quatro pontas",
        None,
        Classification::Internal,
    )
    .await
    .expect("metodologia");
    tx.commit().await.expect("commit");

    let publicar = |etiqueta: &'static str, resumo: &'static str| {
        let pool = m.pool.clone();
        let lider = m.lider.clone();
        let ids = ids.clone();
        let metodologia_id = metodologia.id;
        async move {
            let mut tx = pool.begin().await.expect("transacção");
            let v = science::publish_methodology_version(
                &mut tx,
                &pool,
                &lider,
                &ids,
                metodologia_id,
                etiqueta,
                resumo,
                None,
            )
            .await
            .expect("versão");
            tx.commit().await.expect("commit");
            v
        }
    };

    let v1 = publicar("v1", "Corrente de 10 mA.").await;
    // A primeira está em vigor: nada a substituiu ainda.
    assert!(
        v1.superseded_by_id.is_none(),
        "a primeira versão nasceu substituída"
    );

    let v2 = publicar("v2", "Corrente reduzida para 1 mA.").await;

    let versoes = science::list_methodology_versions(&m.pool, &m.lider, metodologia.id)
        .await
        .expect("versões");
    let lida = |id: uuid::Uuid| {
        versoes
            .iter()
            .find(|v| v.id == id)
            .unwrap_or_else(|| panic!("versão {id} desapareceu"))
    };

    assert_eq!(
        lida(v1.id).superseded_by_id,
        Some(v2.id),
        "publicar a segunda não substituiu a primeira"
    );
    assert_eq!(
        lida(v1.id).status_label(),
        "Substituída",
        "a versão substituída continua a ler-se como se estivesse em vigor"
    );
    assert_eq!(
        lida(v2.id).status_label(),
        "Em vigor",
        "a versão nova não está em vigor"
    );

    // E a anterior **fica**: não se apaga o que a proveniência já cita.
    assert_eq!(versoes.len(), 2, "publicar apagou a versão anterior");
}
