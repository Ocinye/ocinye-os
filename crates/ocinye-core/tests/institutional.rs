//! Uma instituição a nascer, a trabalhar, e a ser observada por vários actores.
//!
//! # Porque este ficheiro existe
//!
//! Os outros testes provam que cada módulo está certo por si. Este prova que
//! **o sistema é um só**: que uma unidade criada num sítio serve de âmbito
//! noutro, que uma ideia promovida deixa de ser candidata em todo o lado, e que
//! um dataset visível numa superfície é visível em todas as outras que o
//! mostrem — e invisível em todas quando não deve ser visto.
//!
//! Um Workspace pode ter todos os ecrãs correctos e continuar a ser um conjunto
//! de ilhas. É esta a diferença que se mede aqui.
//!
//! # Sem SQL de atalho
//!
//! Tudo o que a instituição contém é criado pelas operações reais do Core, e
//! não por `INSERT`. É deliberado: o índice de pesquisa e o feed de actividade
//! são alimentados pelos serviços, e não pela base de dados. Uma fixture escrita
//! em SQL passaria por cima deles e depois observaria zero — um zero sem
//! significado nenhum, porque nada lá tinha chegado.
//!
//! As pessoas e os papéis são a excepção: nascem por `INSERT` porque criá-las
//! exige um administrador que ainda não existe quando a instituição está vazia.

use ocinye_contracts::{Classification, IdeaState, PageRequest, TechnicalRole};
use ocinye_core::modules::collaboration::TaskPriority;
use ocinye_core::modules::{collaboration, data, identity, knowledge, organisation, research};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations must apply to the test database");

    // Antes da primeira escrita, e não depois: falhar depois de escrever
    // não é uma guarda, é um relatório de estragos.
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

macro_rules! skip_without_database {
    () => {
        match pool().await {
            Some(pool) => pool,
            None => {
                eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
                return;
            }
        }
    };
}

/// Uma organização vazia: sem unidades, sem workspaces, sem nada.
async fn organizacao(pool: &PgPool, etiqueta: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("{etiqueta}-{}", Uuid::new_v4().simple()))
        .bind("Instituição de teste")
        .fetch_one(pool)
        .await
        .expect("organização")
}

/// Uma pessoa com os papéis indicados.
async fn pessoa(pool: &PgPool, organisation_id: Uuid, roles: &[TechnicalRole]) -> Principal {
    let handle = format!("p{}", Uuid::new_v4().simple());

    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
              VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@ocinye.com"))
    .fetch_one(pool)
    .await
    .expect("pessoa");

    for role in roles {
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
            .bind(person_id)
            .bind(role.as_str())
            .execute(pool)
            .await
            .expect("papel");
    }

    let record = identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

/// Relê o principal, para que memberships criadas entretanto contem.
async fn refrescar(pool: &PgPool, principal: &Principal) -> Principal {
    let record = identity::person_by_id(pool, principal.person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

fn ids() -> CorrelationIds {
    CorrelationIds::generate()
}

/// Um sufixo curto e único, para códigos institucionais.
fn tag() -> String {
    Uuid::new_v4().simple().to_string()[..6].to_uppercase()
}

// ── 1 e 2: a instituição nasce vazia e produz o seu primeiro projecto ───

/// Uma instituição vazia chega a projecto sem uma linha de SQL.
///
/// # O que este teste prova
///
/// Que o Ocinye OS se arranca a si próprio. Uma organização sem unidades não é
/// um estado inválido a corrigir à mão na base de dados: é o primeiro dia, e o
/// produto tem de saber sair dele.
///
/// A cadeia é a do trabalho científico, e cada passo é uma operação real:
///
/// ```text
/// organização vazia
/// → primeira Unidade
/// → primeira Ideia
/// → candidata a projecto
/// → promoção
/// → Projecto
/// ```
///
/// E em cada passo verifica-se o **efeito**, não o retorno: uma operação que
/// devolve `Ok` e não aparece na lista seguinte não fez o que diz.
#[tokio::test]
async fn uma_instituicao_vazia_chega_a_projecto_sem_sql() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "fresh").await;
    let admin = pessoa(
        &pool,
        organisation_id,
        &[
            TechnicalRole::OrganisationAdmin,
            TechnicalRole::ResearchLead,
        ],
    )
    .await;
    let marca = tag();

    // Dia zero: nada.
    let unidades = organisation::list_units(&pool, &admin, false)
        .await
        .expect("listar unidades");
    assert!(
        unidades.is_empty(),
        "a instituição nasceu com unidades que ninguém criou"
    );

    // A primeira unidade.
    let mut tx = pool.begin().await.expect("tx");
    let unidade = organisation::create_unit(
        &mut tx,
        &admin,
        &ids(),
        organisation::NewUnit {
            code: format!("ENG{marca}"),
            name: "Engenharia Computacional".to_owned(),
            description: None,
            research_areas: vec!["computação".to_owned()],
        },
    )
    .await
    .expect("criar unidade");
    tx.commit().await.expect("commit");

    let unidades = organisation::list_units(&pool, &admin, false)
        .await
        .expect("listar unidades");
    assert_eq!(unidades.len(), 1, "a unidade criada não aparece na lista");

    // A primeira ideia. `create_idea` recebe o principal em `&mut` porque a
    // criação torna quem cria membro do workspace — e o principal em memória
    // tem de saber disso para o passo seguinte.
    let mut autor = admin.clone();
    let mut tx = pool.begin().await.expect("tx");
    let (ideia, _workspace) = research::create_idea(
        &mut tx,
        &mut autor,
        &ids(),
        research::NewIdea {
            unit_id: unidade.id,
            title: "Escalonamento de tarefas em L40S".to_owned(),
            summary: Some("Primeira ideia da instituição.".to_owned()),
            research_question: None,
            hypothesis: None,
            motivation: None,
            keywords: vec!["computação".to_owned()],
            classification: Some(Classification::Internal),
        },
    )
    .await
    .expect("criar ideia");
    tx.commit().await.expect("commit");

    let autor = refrescar(&pool, &autor).await;

    // A ideia aparece em Ideias, e não em Projectos.
    let ideias = research::list_workspaces(
        &pool,
        &autor,
        research::WorkspaceQuery {
            kind: Some(ocinye_contracts::research::WorkspaceKind::Idea),
            ..Default::default()
        },
        PageRequest::default(),
    )
    .await
    .expect("listar ideias");
    assert_eq!(ideias.1, 1, "a ideia criada não aparece em Ideias");

    let projectos = research::list_workspaces(
        &pool,
        &autor,
        research::WorkspaceQuery {
            kind: Some(ocinye_contracts::research::WorkspaceKind::Project),
            ..Default::default()
        },
        PageRequest::default(),
    )
    .await
    .expect("listar projectos");
    assert_eq!(
        projectos.1, 0,
        "uma ideia apareceu em Projectos antes de ser promovida"
    );

    // Ainda não é candidata: o selector de promoção não a deve oferecer.
    let promovíveis = research::list_workspaces(
        &pool,
        &autor,
        research::WorkspaceQuery {
            promotable_only: true,
            ..Default::default()
        },
        PageRequest::default(),
    )
    .await
    .expect("listar promovíveis");
    assert_eq!(
        promovíveis.1, 0,
        "uma ideia em exploração já se oferece para promoção"
    );

    // Até candidata, pelo caminho que o domínio define.
    //
    // Não há salto de `discovery` para `project_candidate`: uma ideia é
    // explorada, toma forma, é revista, e só então se torna candidata. O
    // percurso é a substância do trabalho científico, e o teste segue-o em vez
    // de o contornar com um `UPDATE`.
    for estado in [
        IdeaState::Exploration,
        IdeaState::Concept,
        IdeaState::Review,
        IdeaState::ProjectCandidate,
    ] {
        let mut tx = pool.begin().await.expect("tx");
        research::transition_idea(&mut tx, &autor, &ids(), ideia.id, estado, None)
            .await
            .unwrap_or_else(|e| panic!("transitar para {estado:?}: {e:?}"));
        tx.commit().await.expect("commit");
    }

    let promovíveis = research::list_workspaces(
        &pool,
        &autor,
        research::WorkspaceQuery {
            promotable_only: true,
            ..Default::default()
        },
        PageRequest::default(),
    )
    .await
    .expect("listar promovíveis");
    assert_eq!(
        promovíveis.1, 1,
        "uma candidata não é oferecida para promoção"
    );

    // A promoção.
    let mut tx = pool.begin().await.expect("tx");
    let projecto = research::promote_idea(
        &mut tx,
        &autor,
        &ids(),
        ideia.id,
        research::Promotion {
            code: format!("PRJ{marca}"),
            title: None,
            objectives: Some("Primeiro projecto da instituição.".to_owned()),
            responsible_person_id: None,
        },
    )
    .await
    .expect("promover");
    tx.commit().await.expect("commit");

    // E o efeito atravessa as três superfícies ao mesmo tempo.
    let projectos = research::list_workspaces(
        &pool,
        &autor,
        research::WorkspaceQuery {
            kind: Some(ocinye_contracts::research::WorkspaceKind::Project),
            ..Default::default()
        },
        PageRequest::default(),
    )
    .await
    .expect("listar projectos");
    assert_eq!(projectos.1, 1, "o projecto promovido não aparece");

    let promovíveis = research::list_workspaces(
        &pool,
        &autor,
        research::WorkspaceQuery {
            promotable_only: true,
            ..Default::default()
        },
        PageRequest::default(),
    )
    .await
    .expect("listar promovíveis");
    assert_eq!(
        promovíveis.1, 0,
        "a ideia promovida continua a oferecer-se para promoção"
    );

    assert!(
        research::get_project(&pool, &autor, projecto.id)
            .await
            .is_ok(),
        "o projecto não é legível por quem o promoveu"
    );
}

// ── A instituição povoada, com vários actores ──────────────────────────

/// Uma instituição com trabalho a sério, e gente que a vê de sítios diferentes.
struct Instituicao {
    /// A organização onde tudo isto vive.
    ///
    /// Guardada porque identifica a fixture, e porque um teste que precise de
    /// perguntar «a que instituição pertence isto» não deve ter de o deduzir.
    #[allow(
        dead_code,
        reason = "identifica a fixture; nem todos os testes precisam dela"
    )]
    organisation_id: Uuid,
    /// Duas unidades, para provar que não há «unidade principal».
    unidade_a: Uuid,
    unidade_b: Uuid,
    /// Membro de ambas as unidades e do workspace de A.
    membro: Principal,
    /// Da mesma organização, sem participação nenhuma em A.
    forasteiro: Principal,
    /// De outra organização.
    estrangeiro: Principal,
    /// O workspace onde o membro participa.
    workspace_a: Uuid,
    /// Um workspace que o membro não alcança.
    workspace_fechado: Uuid,
    /// Um dataset INTERNAL dentro do workspace inacessível.
    dataset_escondido: Uuid,
    /// Um dataset dentro do workspace do membro.
    dataset_visivel: Uuid,
    /// Uma referência com um termo único, para a sonda de pesquisa.
    termo_unico: String,
}

/// Constrói a instituição por operações reais.
async fn instituicao(pool: &PgPool) -> Instituicao {
    let organisation_id = organizacao(pool, "rica").await;
    let outra = organizacao(pool, "outra").await;
    let marca = tag();

    let admin = pessoa(
        pool,
        organisation_id,
        &[
            TechnicalRole::OrganisationAdmin,
            TechnicalRole::ResearchLead,
        ],
    )
    .await;

    // Duas unidades.
    let mut tx = pool.begin().await.expect("tx");
    let unidade_a = organisation::create_unit(
        &mut tx,
        &admin,
        &ids(),
        organisation::NewUnit {
            code: format!("ENA{marca}"),
            name: "Energia".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("unidade A");
    let unidade_b = organisation::create_unit(
        &mut tx,
        &admin,
        &ids(),
        organisation::NewUnit {
            code: format!("SIS{marca}"),
            name: "Sistemas Digitais".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("unidade B");
    tx.commit().await.expect("commit");

    // O membro pertence às **duas** unidades: é o caso que proíbe inventar uma
    // unidade principal.
    //
    // Gestor na primeira e membro na segunda, e a diferença não é decorativa.
    // `may_write_in_context` exige gestão da unidade para escrever recursos com
    // âmbito de unidade — criar uma ideia é isso. A tabela de permissões lista
    // `IdeasCreate` para `UnitRole::Member`, mas a regra de escrita é uma porta
    // separada e mais estreita, e é ela que decide.
    //
    // Descobri-o aqui: uma fixture que desse gestão a toda a gente nunca teria
    // encontrado a diferença, e um teste que só usasse administradores estaria
    // a provar o sistema no seu caso mais fácil.
    let mut membro = pessoa(pool, organisation_id, &[TechnicalRole::ResearchMember]).await;
    for (unidade, papel) in [
        (unidade_a.id, ocinye_contracts::UnitRole::Manager),
        (unidade_b.id, ocinye_contracts::UnitRole::Member),
    ] {
        let mut tx = pool.begin().await.expect("tx");
        organisation::add_unit_member(&mut tx, &admin, &ids(), unidade, membro.person_id, papel)
            .await
            .expect("membro da unidade");
        tx.commit().await.expect("commit");
    }
    membro = refrescar(pool, &membro).await;

    // O workspace do membro, criado por ele.
    let mut tx = pool.begin().await.expect("tx");
    let (_, workspace_a) = research::create_idea(
        &mut tx,
        &mut membro,
        &ids(),
        research::NewIdea {
            unit_id: unidade_a.id,
            title: "Rede de sensores".to_owned(),
            summary: None,
            research_question: None,
            hypothesis: None,
            motivation: None,
            keywords: Vec::new(),
            classification: Some(Classification::Internal),
        },
    )
    .await
    .expect("ideia do membro");
    tx.commit().await.expect("commit");
    membro = refrescar(pool, &membro).await;

    // Um workspace RESTRITO na unidade B, criado por outra pessoa: o membro
    // pertence à unidade, mas não participa neste workspace.
    let dono = pessoa(pool, organisation_id, &[TechnicalRole::ResearchLead]).await;
    let mut tx = pool.begin().await.expect("tx");
    organisation::add_unit_member(
        &mut tx,
        &admin,
        &ids(),
        unidade_b.id,
        dono.person_id,
        ocinye_contracts::UnitRole::Manager,
    )
    .await
    .expect("gestor da unidade B");
    tx.commit().await.expect("commit");
    let mut dono_mut = refrescar(pool, &dono).await;
    let mut tx = pool.begin().await.expect("tx");
    let (_, workspace_fechado) = research::create_idea(
        &mut tx,
        &mut dono_mut,
        &ids(),
        research::NewIdea {
            unit_id: unidade_b.id,
            title: "Trabalho reservado".to_owned(),
            summary: None,
            research_question: None,
            hypothesis: None,
            motivation: None,
            keywords: Vec::new(),
            // Nasce INTERNAL de propósito. A classificação do conteúdo é
            // fixada pela do contentor no momento da criação: um dataset criado
            // num workspace RESTRITO nasce RESTRITO, e então as duas condições
            // negam ao mesmo tempo — o que faz o teste passar mesmo sem o
            // predicado do contentor, e portanto não prova nada.
            //
            // Aqui as duas condições são separadas: o dataset fica INTERNAL, o
            // workspace fecha-se depois, e o que o esconde passa a ser apenas o
            // contentor.
            classification: Some(Classification::Internal),
        },
    )
    .await
    .expect("ideia reservada");
    tx.commit().await.expect("commit");
    let dono = refrescar(pool, &dono_mut).await;

    // Um dataset INTERNAL dentro do workspace inacessível.
    //
    // A classificação do dataset **não** o esconde: INTERNAL é legível por
    // qualquer membro activo. O que o esconde é o workspace que o contém — e é
    // exactamente essa a distinção que `SB1-FU-01` existe para provar.
    let mut tx = pool.begin().await.expect("tx");
    let dataset_escondido = data::create_dataset(
        &mut tx,
        &dono,
        &ids(),
        workspace_fechado.id,
        data::NewDataset {
            code: format!("DSX{marca}"),
            title: "Leituras reservadas".to_owned(),
            description: None,
            origin: data::DatasetOrigin::CollectedByOcinye,
            licence: None,
            usage_restrictions: None,
            responsible_person_id: None,
            acquisition_date: None,
            keywords: Vec::new(),
            classification: Some(Classification::Internal),
        },
    )
    .await
    .expect("dataset escondido");
    tx.commit().await.expect("commit");

    // Agora o workspace fecha-se, pela operação real. `reclassify_workspace`
    // não propaga aos conteúdos — e é exactamente por isso que a invariante do
    // contentor tem de existir: sem ela, um artefacto INTERNAL continuaria a
    // aparecer nas vistas agregadas depois de o seu ambiente ter sido fechado.
    let mut tx = pool.begin().await.expect("tx");
    research::reclassify_workspace(
        &mut tx,
        &dono,
        &ids(),
        workspace_fechado.id,
        Classification::Restricted,
        "trabalho reservado a partir de agora",
    )
    .await
    .expect("reclassificar");
    tx.commit().await.expect("commit");

    let mut tx = pool.begin().await.expect("tx");
    let dataset_visivel = data::create_dataset(
        &mut tx,
        &membro,
        &ids(),
        workspace_a.id,
        data::NewDataset {
            code: format!("DSV{marca}"),
            title: "Leituras da rede".to_owned(),
            description: None,
            origin: data::DatasetOrigin::CollectedByOcinye,
            licence: None,
            usage_restrictions: None,
            responsible_person_id: None,
            acquisition_date: None,
            keywords: Vec::new(),
            classification: Some(Classification::Internal),
        },
    )
    .await
    .expect("dataset visível");
    tx.commit().await.expect("commit");

    // Uma referência com um termo que não existe em mais lado nenhum: é o
    // controlo positivo da sonda de pesquisa.
    let termo_unico = format!("ZXQV{marca}");
    let mut tx = pool.begin().await.expect("tx");
    knowledge::create_source(
        &mut tx,
        &membro,
        &ids(),
        workspace_a.id,
        knowledge::NewSource {
            source_type: None,
            title: format!("Estudo {termo_unico}"),
            authors: Vec::new(),
            year: None,
            container_title: None,
            publisher: None,
            doi: None,
            isbn: None,
            url: None,
            abstract_text: None,
            keywords: Vec::new(),
            licence: None,
            content_right: None,
            origin: None,
            citation_key: None,
            classification: Some(Classification::Internal),
            raw_metadata: None,
        },
    )
    .await
    .expect("referência");
    tx.commit().await.expect("commit");

    Instituicao {
        organisation_id,
        unidade_a: unidade_a.id,
        unidade_b: unidade_b.id,
        membro,
        forasteiro: pessoa(pool, organisation_id, &[TechnicalRole::ResearchMember]).await,
        estrangeiro: pessoa(pool, outra, &[TechnicalRole::OrganisationAdmin]).await,
        workspace_a: workspace_a.id,
        workspace_fechado: workspace_fechado.id,
        dataset_escondido: dataset_escondido.id,
        dataset_visivel: dataset_visivel.id,
        termo_unico,
    }
}

// ── 8: as três invariantes de segurança, dentro da fixture integrada ────

/// `SB1-FU-01` continua fechado: a agregação exige o contentor visível.
///
/// # Os dois testes têm de estar separados
///
/// O dataset escondido é `INTERNAL` — legível por qualquer membro activo pela
/// sua própria classificação. O que o esconde é o **workspace** que o contém.
///
/// Uma fixture onde as duas condições negam ao mesmo tempo não prova nada: o
/// recurso não apareceria de qualquer maneira, e remover o predicado do
/// contentor deixaria o teste verde. É por isso que este dataset é o mais
/// permissivo que pode ser sem deixar de estar fechado.
///
/// # Controlo positivo primeiro
///
/// O dataset **visível** tem de aparecer. Sem isso, o zero adversarial pode ser
/// só uma consulta que não vê nada — e um zero que não distingue «não deve» de
/// «não consegue» não é um resultado de segurança.
#[tokio::test]
async fn sb1_fu_01_a_agregacao_exige_o_contentor_visivel() {
    let pool = skip_without_database!();
    let inst = instituicao(&pool).await;

    let (datasets, total) = data::list_datasets(&pool, &inst.membro, None, PageRequest::default())
        .await
        .expect("listar datasets");

    // Controlo positivo: a fixture e o caminho de observação funcionam.
    assert!(
        datasets.iter().any(|d| d.id == inst.dataset_visivel),
        "controlo positivo falhou: o dataset do próprio membro não aparece na \
         listagem institucional, e por isso um zero adversarial não diria nada"
    );

    // E o adversarial: o contido num workspace inacessível não aparece.
    assert!(
        !datasets.iter().any(|d| d.id == inst.dataset_escondido),
        "um dataset INTERNAL dentro de um workspace inacessível apareceu na \
         listagem institucional"
    );

    // A contagem concorda com a lista: as duas usam a mesma autorização.
    assert_eq!(
        total,
        i64::try_from(datasets.len()).unwrap_or(i64::MAX),
        "a contagem e a lista discordam sob a mesma consulta"
    );

    // E o caminho directo diz o mesmo que a agregação.
    assert!(
        data::get_dataset(&pool, &inst.membro, inst.dataset_visivel)
            .await
            .is_ok(),
        "o dataset visível na lista não é legível pelo caminho directo"
    );
    assert!(
        data::get_dataset(&pool, &inst.membro, inst.dataset_escondido)
            .await
            .is_err(),
        "o dataset ausente da lista é legível pelo caminho directo"
    );
}

/// `SB1-FU-02` continua fechado: um âmbito do cliente não cria autoridade.
///
/// > **A client-supplied scope identifier may narrow an already-authorised
/// > operation; it never establishes authority.**
///
/// O identificador do workspace fechado é conhecido — está aqui, na fixture.
/// Conhecê-lo não pode ser o suficiente.
#[tokio::test]
async fn sb1_fu_02_um_ambito_do_cliente_nao_cria_autoridade() {
    let pool = skip_without_database!();
    let inst = instituicao(&pool).await;

    // Controlo positivo: com o workspace a que pertence, o âmbito funciona.
    let proprio = data::list_datasets(
        &pool,
        &inst.membro,
        Some(inst.workspace_a),
        PageRequest::default(),
    )
    .await
    .expect("o âmbito do próprio workspace devia funcionar");
    assert!(
        proprio.0.iter().any(|d| d.id == inst.dataset_visivel),
        "controlo positivo falhou: o âmbito não devolve o que lá está"
    );

    // Adversarial: o mesmo pedido, com o UUID de um workspace alheio.
    let alheio = data::list_datasets(
        &pool,
        &inst.membro,
        Some(inst.workspace_fechado),
        PageRequest::default(),
    )
    .await;
    match alheio {
        Err(_) => {}
        Ok((linhas, total)) => {
            assert!(
                linhas.is_empty() && total == 0,
                "escrever o UUID de um workspace alheio devolveu {} datasets",
                linhas.len()
            );
        }
    }

    // O mesmo para tarefas, que é onde a falha original era mais grave.
    let tarefas = collaboration::list_tasks(
        &pool,
        &inst.membro,
        Some(inst.workspace_fechado),
        None,
        false,
        PageRequest::default(),
    )
    .await;
    match tarefas {
        Err(_) => {}
        Ok((linhas, total)) => {
            assert!(
                linhas.is_empty() && total == 0,
                "um âmbito alheio devolveu {} tarefas",
                linhas.len()
            );
        }
    }
}

/// Outra organização nunca aparece, mesmo com identificadores na mão.
#[tokio::test]
async fn outra_organizacao_permanece_impossivel() {
    let pool = skip_without_database!();
    let inst = instituicao(&pool).await;

    // O estrangeiro é administrador — da organização dele.
    let (datasets, total) =
        data::list_datasets(&pool, &inst.estrangeiro, None, PageRequest::default())
            .await
            .expect("listar");
    assert_eq!(
        total, 0,
        "um actor de outra organização vê {total} datasets"
    );
    assert!(datasets.is_empty());

    assert!(
        data::get_dataset(&pool, &inst.estrangeiro, inst.dataset_visivel)
            .await
            .is_err(),
        "um actor de outra organização alcançou um dataset pelo identificador"
    );
    assert!(
        research::get_workspace(&pool, &inst.estrangeiro, inst.workspace_a)
            .await
            .is_err(),
        "um actor de outra organização alcançou um workspace pelo identificador"
    );
}

// ── 9: o mesmo facto, visto de superfícies diferentes ───────────────────

/// Um recurso tem o mesmo resultado em todas as superfícies que o expõem.
///
/// > **A resource has the same visibility outcome regardless of which Workspace
/// > surface exposes it.**
///
/// # A propriedade é a concordância, e não um veredicto fixo
///
/// A primeira versão deste teste afirmava que o forasteiro não veria o dataset
/// visível. Estava errada — e a expectativa é que estava errada, não o código:
/// o dataset é `INTERNAL` dentro de um workspace `INTERNAL`, e `INTERNAL` é
/// legível por qualquer membro activo da organização (ADR-0100). Não é preciso
/// participar para ler; é preciso participar para *aparecer em O Meu Trabalho*,
/// que é outra coisa.
///
/// O que tem de ser verdade não é «este actor vê» ou «não vê» — é que as três
/// superfícies **digam o mesmo**. `Dados` a mostrar e o caminho directo a
/// recusar seria um sistema que discorda de si próprio, e quem o usa não sabe
/// qual das duas acreditar.
#[tokio::test]
async fn o_mesmo_recurso_tem_o_mesmo_resultado_em_todas_as_superficies() {
    let pool = skip_without_database!();
    let inst = instituicao(&pool).await;

    for (quem, actor) in [
        ("o membro", &inst.membro),
        ("o forasteiro", &inst.forasteiro),
        ("o estrangeiro", &inst.estrangeiro),
    ] {
        for (recurso, id) in [
            ("o dataset acessível", inst.dataset_visivel),
            ("o dataset no workspace fechado", inst.dataset_escondido),
        ] {
            let (datasets, _) = data::list_datasets(&pool, actor, None, PageRequest::default())
                .await
                .expect("listar datasets");
            let na_lista = datasets.iter().any(|d| d.id == id);

            let directo = data::get_dataset(&pool, actor, id).await.is_ok();

            let com_ambito =
                data::list_datasets(&pool, actor, Some(inst.workspace_a), PageRequest::default())
                    .await
                    .is_ok_and(|(linhas, _)| linhas.iter().any(|d| d.id == id));

            assert_eq!(
                na_lista, directo,
                "{quem} · {recurso}: a listagem diz {na_lista} e o caminho directo diz {directo}"
            );

            // O âmbito explícito só pode concordar quando é o âmbito certo: um
            // recurso de outro workspace não aparece neste, e isso não é
            // discordância — é o filtro a filtrar.
            if id == inst.dataset_visivel {
                assert_eq!(
                    na_lista, com_ambito,
                    "{quem} · {recurso}: o âmbito explícito discorda da listagem"
                );
            } else {
                assert!(
                    !com_ambito,
                    "{quem} · {recurso}: apareceu no âmbito de outro workspace"
                );
            }
        }
    }

    // E os dois extremos, onde o veredicto é conhecido e não apenas coerente.
    let (do_membro, _) = data::list_datasets(&pool, &inst.membro, None, PageRequest::default())
        .await
        .expect("listar");
    assert!(
        do_membro.iter().any(|d| d.id == inst.dataset_visivel),
        "quem criou o dataset não o vê"
    );
    assert!(
        !do_membro.iter().any(|d| d.id == inst.dataset_escondido),
        "o dataset do workspace fechado aparece a quem não participa nele"
    );

    let (do_estrangeiro, _) =
        data::list_datasets(&pool, &inst.estrangeiro, None, PageRequest::default())
            .await
            .expect("listar");
    assert!(
        do_estrangeiro.is_empty(),
        "outra organização vê datasets desta"
    );
}

// ── 3 e 8: pesquisa, com o positivo antes do negativo ──────────────────

/// A pesquisa devolve o que é do actor e omite o que não é.
///
/// # A ordem importa
///
/// > **A negative security result is meaningful only after a positive control
/// > proves the fixture and observation path work.**
///
/// O termo é único e foi indexado por uma operação real — `create_source`, e
/// não um `INSERT`, porque é o serviço que alimenta o índice. Se a fixture
/// tivesse escrito a linha directamente na base de dados, a pesquisa devolveria
/// zero para toda a gente e o teste adversarial passaria sem provar nada.
#[tokio::test]
async fn a_pesquisa_encontra_o_autorizado_e_omite_o_resto() {
    let pool = skip_without_database!();
    let inst = instituicao(&pool).await;

    // Controlo positivo: quem criou encontra.
    let (achados, _) = ocinye_core::modules::search::search(
        &pool,
        &inst.membro,
        &inst.termo_unico,
        None,
        None,
        PageRequest::default(),
    )
    .await
    .expect("pesquisar");
    assert_eq!(
        achados.len(),
        1,
        "controlo positivo falhou: o termo único não foi indexado, e por isso um \
         zero adversarial não diria nada sobre autorização"
    );

    // Adversarial: outra organização não encontra o mesmo termo.
    let (alheios, _) = ocinye_core::modules::search::search(
        &pool,
        &inst.estrangeiro,
        &inst.termo_unico,
        None,
        None,
        PageRequest::default(),
    )
    .await
    .expect("pesquisar");
    assert!(
        alheios.is_empty(),
        "outra organização encontrou {} resultados desta",
        alheios.len()
    );
}

// ── 4: trabalho atribuído, e o que «participar» significa ──────────────

/// Uma tarefa aparece a quem a tem, sem transformar «visível» em «participado».
///
/// # A distinção
///
/// Ler um workspace e participar nele são coisas diferentes. `O Meu Trabalho`
/// promete participação, e por isso não pode encher-se de tudo o que o membro
/// consegue ler — seria a instituição inteira com o nome de «meu».
#[tokio::test]
async fn uma_tarefa_atribuida_aparece_a_quem_a_tem() {
    let pool = skip_without_database!();
    let inst = instituicao(&pool).await;

    let mut tx = pool.begin().await.expect("tx");
    let tarefa = collaboration::create_task(
        &mut tx,
        &inst.membro,
        &ids(),
        collaboration::NewTask {
            workspace_id: inst.workspace_a,
            title: "Calibrar sensores".to_owned(),
            description: None,
            priority: TaskPriority::Normal,
            assignee_id: Some(inst.membro.person_id),
            due_on: None,
        },
    )
    .await
    .expect("criar tarefa");
    tx.commit().await.expect("commit");

    // Controlo positivo: o encarregado vê-a no seu trabalho.
    let (minhas, total) = collaboration::list_tasks(
        &pool,
        &inst.membro,
        None,
        Some(inst.membro.person_id),
        false,
        PageRequest::default(),
    )
    .await
    .expect("listar tarefas");
    assert!(
        minhas.iter().any(|t| t.id == tarefa.id),
        "controlo positivo falhou: quem tem a tarefa não a vê"
    );
    assert_eq!(
        total,
        i64::try_from(minhas.len()).unwrap_or(i64::MAX),
        "a contagem e a lista discordam"
    );

    // O forasteiro não a tem atribuída, e por isso não a vê como sua — mesmo
    // que o workspace lhe seja legível.
    let (do_forasteiro, _) = collaboration::list_tasks(
        &pool,
        &inst.forasteiro,
        None,
        Some(inst.forasteiro.person_id),
        false,
        PageRequest::default(),
    )
    .await
    .expect("listar tarefas");
    assert!(
        !do_forasteiro.iter().any(|t| t.id == tarefa.id),
        "uma tarefa de outra pessoa apareceu como trabalho do forasteiro: \
         «visível» foi confundido com «participado»"
    );

    // E outra organização não a alcança de maneira nenhuma.
    let (do_estrangeiro, _) = collaboration::list_tasks(
        &pool,
        &inst.estrangeiro,
        None,
        None,
        false,
        PageRequest::default(),
    )
    .await
    .expect("listar tarefas");
    assert!(
        !do_estrangeiro.iter().any(|t| t.id == tarefa.id),
        "outra organização alcançou uma tarefa desta"
    );
}

// ── 6 e a regressão de paginação + autorização ─────────────────────────

/// A paginação percorre o conjunto autorizado, e não o conjunto todo.
///
/// # A armadilha
///
/// > **Pagination operates inside the authorised result set.**
///
/// Paginar antes de autorizar tem duas consequências, e as duas são erradas de
/// maneiras diferentes:
///
/// - a página pode **preencher o buraco** com o recurso proibido, e então ele
///   aparece;
/// - ou pode devolver uma página com menos linhas do que devia, e então o
///   recurso autorizado que ficou de fora perde-se — não está nesta página nem
///   na seguinte, e ninguém dá por isso.
///
/// A fixture põe um dataset inacessível **no meio** da ordenação, com
/// autorizados antes e depois, precisamente para que qualquer uma das duas
/// falhas apareça.
///
/// # E a linha 51
///
/// Com mais recursos do que cabem numa página, o teste percorre todas as
/// páginas e junta o que encontrou. O conjunto tem de ser exactamente o
/// autorizado: sem repetições, sem faltas.
#[tokio::test]
async fn a_paginacao_percorre_o_conjunto_autorizado_sem_faltas_nem_repeticoes() {
    let pool = skip_without_database!();
    let inst = instituicao(&pool).await;
    let marca = tag();

    // Doze datasets autorizados, para atravessar páginas de cinco.
    let mut esperados = Vec::new();
    for n in 0..12 {
        let mut tx = pool.begin().await.expect("tx");
        let d = data::create_dataset(
            &mut tx,
            &inst.membro,
            &ids(),
            inst.workspace_a,
            data::NewDataset {
                code: format!("PG{marca}{n:02}"),
                title: format!("Série {n:02}"),
                description: None,
                origin: data::DatasetOrigin::CollectedByOcinye,
                licence: None,
                usage_restrictions: None,
                responsible_person_id: None,
                acquisition_date: None,
                keywords: Vec::new(),
                classification: Some(Classification::Internal),
            },
        )
        .await
        .expect("dataset");
        tx.commit().await.expect("commit");
        esperados.push(d.id);
    }

    // O conjunto autorizado inclui também os datasets que a fixture já criou.
    let (tudo, total) = data::list_datasets(&pool, &inst.membro, None, PageRequest::default())
        .await
        .expect("listar tudo");
    let autorizado: std::collections::BTreeSet<Uuid> = tudo.iter().map(|d| d.id).collect();
    assert!(
        total > 5,
        "a fixture não produziu mais do que uma página: {total}"
    );
    assert!(
        !autorizado.contains(&inst.dataset_escondido),
        "o conjunto autorizado inclui o dataset do workspace fechado"
    );
    for id in &esperados {
        assert!(
            autorizado.contains(id),
            "um dataset criado não é autorizado"
        );
    }

    // Percorre página a página, com páginas pequenas de propósito.
    let tamanho = 5;
    let mut visto: Vec<Uuid> = Vec::new();
    let mut pagina = 1;
    loop {
        let (linhas, contagem) = data::list_datasets(
            &pool,
            &inst.membro,
            None,
            PageRequest {
                page: pagina,
                page_size: tamanho,
            },
        )
        .await
        .expect("listar página");

        assert_eq!(
            contagem, total,
            "a contagem mudou entre páginas: {total} na primeira, {contagem} na {pagina}.ª"
        );
        assert!(
            !linhas.iter().any(|d| d.id == inst.dataset_escondido),
            "a página {pagina} preencheu-se com o dataset do workspace fechado"
        );

        if linhas.is_empty() {
            break;
        }
        visto.extend(linhas.iter().map(|d| d.id));
        pagina += 1;
        assert!(pagina < 100, "a paginação não termina");
    }

    // Nem repetições…
    let unicos: std::collections::BTreeSet<Uuid> = visto.iter().copied().collect();
    assert_eq!(
        unicos.len(),
        visto.len(),
        "a paginação devolveu {} linhas para {} recursos distintos",
        visto.len(),
        unicos.len()
    );

    // …nem faltas. É aqui que a «linha 51» se prova alcançável.
    assert_eq!(
        unicos, autorizado,
        "o que a paginação percorreu não é o conjunto autorizado"
    );
    assert_eq!(
        i64::try_from(unicos.len()).unwrap_or(i64::MAX),
        total,
        "a contagem não corresponde ao que a paginação consegue alcançar"
    );
}

// ── 5: duas unidades, e nenhuma delas é «a principal» ──────────────────

/// Um membro de duas unidades não recebe uma unidade principal inventada.
///
/// > **The Workspace never invents a primary Unit.**
///
/// # O que o domínio garante, e o que garante a interface
///
/// Aqui prova-se a metade de baixo: o Core aceita `unit_id` como recorte, os
/// conjuntos são realmente diferentes conforme a unidade, e nenhuma consulta
/// escolhe uma por si — sem `unit_id`, devolve as duas.
///
/// A metade de cima — a tab «Da Unidade» exigir escolha explícita quando há
/// várias — está provada nos testes do Workspace, onde a decisão vive.
#[tokio::test]
async fn duas_unidades_nao_produzem_uma_unidade_principal() {
    let pool = skip_without_database!();
    let inst = instituicao(&pool).await;
    let marca = tag();

    // Uma ideia em cada unidade, para que os conjuntos sejam distinguíveis.
    let mut autor = inst.membro.clone();
    for (unidade, titulo) in [
        (inst.unidade_a, "Trabalho na Energia"),
        (inst.unidade_b, "Trabalho nos Sistemas"),
    ] {
        let mut tx = pool.begin().await.expect("tx");
        let r = research::create_idea(
            &mut tx,
            &mut autor,
            &ids(),
            research::NewIdea {
                unit_id: unidade,
                title: format!("{titulo} {marca}"),
                summary: None,
                research_question: None,
                hypothesis: None,
                motivation: None,
                keywords: Vec::new(),
                classification: Some(Classification::Internal),
            },
        )
        .await;
        // Na unidade B o membro é apenas membro, e escrever recursos com âmbito
        // de unidade exige gestão. A recusa é o comportamento certo, e o teste
        // não a contorna: o que interessa é o recorte, não quem criou.
        if r.is_ok() {
            tx.commit().await.expect("commit");
        } else {
            let _ = tx.rollback().await;
        }
    }
    let membro = refrescar(&pool, &autor).await;

    // O membro pertence às duas.
    assert_eq!(
        membro.unit_roles.len(),
        2,
        "a fixture deixou de ter um membro em duas unidades, e é esse o caso \
         que proíbe inventar uma unidade principal"
    );

    // Sem recorte: as duas unidades juntas.
    let (_todas, total_todas) = research::list_workspaces(
        &pool,
        &membro,
        research::WorkspaceQuery::default(),
        PageRequest::default(),
    )
    .await
    .expect("listar sem recorte");

    // Com recorte: cada unidade dá o seu conjunto, e nenhum é o total.
    let mut soma = 0;
    for unidade in [inst.unidade_a, inst.unidade_b] {
        let (linhas, total) = research::list_workspaces(
            &pool,
            &membro,
            research::WorkspaceQuery {
                unit_id: Some(unidade),
                ..Default::default()
            },
            PageRequest::default(),
        )
        .await
        .expect("listar com recorte");

        assert!(
            linhas.iter().all(|w| w.unit_id == unidade),
            "o recorte por unidade devolveu workspaces de outra unidade"
        );
        assert!(
            total <= total_todas,
            "uma unidade sozinha devolve mais do que as duas juntas"
        );
        soma += total;
    }

    assert_eq!(
        soma, total_todas,
        "as duas unidades somadas não dão o conjunto sem recorte: {soma} contra {total_todas}"
    );
    assert!(
        total_todas > 0,
        "controlo positivo falhou: o membro não vê workspace nenhum, e por isso \
         a comparação entre recortes não diria nada"
    );

    // E o âmbito de outra organização não alcança nada.
    let (alheios, _) = research::list_workspaces(
        &pool,
        &inst.estrangeiro,
        research::WorkspaceQuery {
            unit_id: Some(inst.unidade_a),
            ..Default::default()
        },
        PageRequest::default(),
    )
    .await
    .expect("listar");
    assert!(
        alheios.is_empty(),
        "escrever o UUID de uma unidade de outra organização devolveu {} workspaces",
        alheios.len()
    );
}

// ── 4 e 7: participar não é o mesmo que conseguir ler ──────────────────

/// Um conjunto de participações vazio estreita para zero.
///
/// > **An empty membership set narrows to zero; it never removes the filter.**
///
/// # Porque é a falha mais fácil de escrever
///
/// Em SQL, um filtro por lista vazia parece não ter nada para filtrar, e a
/// tentação é omiti-lo — `IN ()` não é sequer válido em Postgres, e o caminho de
/// menor resistência é não gerar a cláusula. O resultado é o pior possível: quem
/// não participa em nada passa a ver **tudo**, e um ecrã chamado «O Meu
/// Trabalho» enche-se com a instituição inteira.
///
/// O forasteiro é o caso: consegue **ler** os workspaces INTERNAL da sua
/// organização — ADR-0100 — e não participa em nenhum. As duas metades da
/// resposta têm de ser diferentes.
#[tokio::test]
async fn um_conjunto_de_participacoes_vazio_estreita_para_zero() {
    let pool = skip_without_database!();
    let inst = instituicao(&pool).await;

    // Controlo positivo: o forasteiro **consegue ler** — logo, um zero no
    // recorte de participação não é a consulta a não ver nada.
    let (legiveis, total_legiveis) = research::list_workspaces(
        &pool,
        &inst.forasteiro,
        research::WorkspaceQuery::default(),
        PageRequest::default(),
    )
    .await
    .expect("listar legíveis");
    assert!(
        total_legiveis > 0,
        "controlo positivo falhou: o forasteiro não lê workspace nenhum, e por \
         isso um zero no recorte de participação não diria nada"
    );
    assert!(!legiveis.is_empty());

    // O forasteiro não participa em nada.
    assert!(
        inst.forasteiro.workspace_roles.is_empty(),
        "a fixture deixou de ter um actor sem participações"
    );

    // E o recorte de participação devolve zero — não tudo.
    let membros_de: Vec<Uuid> = inst.forasteiro.workspace_roles.keys().copied().collect();
    let (minhas, total_minhas) = research::list_workspaces(
        &pool,
        &inst.forasteiro,
        research::WorkspaceQuery {
            member_of: Some(&membros_de),
            ..Default::default()
        },
        PageRequest::default(),
    )
    .await
    .expect("listar participadas");

    assert_eq!(
        total_minhas, 0,
        "um conjunto de participações vazio devolveu {total_minhas} workspaces: \
         o filtro foi removido em vez de estreitar"
    );
    assert!(minhas.is_empty());

    // E o membro, que participa, continua a ver o que é dele.
    let seus: Vec<Uuid> = inst.membro.workspace_roles.keys().copied().collect();
    assert!(
        !seus.is_empty(),
        "a fixture deixou de ter um actor com participações"
    );
    let (dele, total_dele) = research::list_workspaces(
        &pool,
        &inst.membro,
        research::WorkspaceQuery {
            member_of: Some(&seus),
            ..Default::default()
        },
        PageRequest::default(),
    )
    .await
    .expect("listar participadas");
    assert!(
        total_dele > 0,
        "quem participa não vê nada no recorte de participação"
    );
    assert!(
        dele.iter().all(|w| seus.contains(&w.id)),
        "o recorte de participação devolveu um workspace onde o membro não participa"
    );
    assert!(
        total_dele < total_legiveis || total_legiveis == total_dele,
        "participar não pode devolver mais do que conseguir ler"
    );
}

// ── A pergunta que decide a classificação de `add_unit_member` ─────────

/// Pertencer a uma unidade **expande o acesso efectivo**.
///
/// # Porque este teste existe
///
/// A classificação agentic de `organisation::add_unit_member` depende de uma
/// pergunta de facto: filiação numa unidade é metadado organizacional, ou muda o
/// que a pessoa passa a alcançar?
///
/// Se for a segunda, a operação é uma **mutação da fronteira de autoridade** e
/// não é delegável — pela mesma regra que fecha `grant_role` e `create_grant`,
/// e não por ser arriscada.
///
/// O teste mede em vez de assumir. Compara o que a mesma pessoa consegue antes
/// e depois de ser acrescentada a uma unidade, sem lhe tocar em papel técnico
/// nenhum: se a única coisa que mudou foi a filiação e o acesso mudou com ela, a
/// resposta está dada.
#[tokio::test]
async fn pertencer_a_uma_unidade_expande_o_acesso_efectivo() {
    use ocinye_contracts::Permission;
    use ocinye_domain::{ResourceContext, ResourceKind};

    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "membership").await;
    let admin = pessoa(
        &pool,
        organisation_id,
        &[
            TechnicalRole::OrganisationAdmin,
            TechnicalRole::ResearchLead,
        ],
    )
    .await;
    let marca = tag();

    let mut tx = pool.begin().await.expect("tx");
    let unidade = organisation::create_unit(
        &mut tx,
        &admin,
        &ids(),
        organisation::NewUnit {
            code: format!("MEM{marca}"),
            name: "Unidade de prova".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("unidade");
    tx.commit().await.expect("commit");

    // Uma pessoa sem filiação nenhuma, e sem papel técnico que lhe dê nada.
    let antes = pessoa(&pool, organisation_id, &[TechnicalRole::ResearchMember]).await;
    let contexto = ResourceContext::unit(ResourceKind::Idea, organisation_id, unidade.id);

    // O que consegue antes.
    let podia_criar_ideias =
        ocinye_domain::can(&antes, Permission::IdeasCreate, &contexto, None).allowed;
    let podia_ver_datasets =
        ocinye_domain::can(&antes, Permission::DatasetsView, &contexto, None).allowed;

    // A única coisa que muda é a filiação.
    let mut tx = pool.begin().await.expect("tx");
    organisation::add_unit_member(
        &mut tx,
        &admin,
        &ids(),
        unidade.id,
        antes.person_id,
        ocinye_contracts::UnitRole::Member,
    )
    .await
    .expect("filiação");
    tx.commit().await.expect("commit");

    let depois = refrescar(&pool, &antes).await;

    // Nenhum papel técnico mudou: só a filiação.
    assert_eq!(
        antes.roles, depois.roles,
        "a fixture alterou papéis técnicos e deixou de medir o efeito da filiação"
    );

    let pode_criar_ideias =
        ocinye_domain::can(&depois, Permission::IdeasCreate, &contexto, None).allowed;
    let pode_ver_datasets =
        ocinye_domain::can(&depois, Permission::DatasetsView, &contexto, None).allowed;

    // O veredicto.
    assert!(
        !podia_criar_ideias && pode_criar_ideias,
        "controlo: antes {podia_criar_ideias}, depois {pode_criar_ideias}"
    );
    assert!(
        !podia_ver_datasets && pode_ver_datasets,
        "controlo: antes {podia_ver_datasets}, depois {pode_ver_datasets}"
    );

    // Portanto: filiação numa unidade **não** é metadado organizacional. É uma
    // mutação da fronteira de autoridade, e `organisation::add_unit_member`
    // classifica-se pela mesma regra que fecha `grant_role` e `create_grant`.
}
