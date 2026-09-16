//! Unidades: geração de código, seed institucional, edição e pesquisa.
//!
//! # O que este ficheiro prova
//!
//! Que uma instituição começa com unidades sensatas sem que nada esteja
//! codificado como enum, que o código de uma unidade nova é gerado de forma
//! determinista a partir do nome, que uma unidade que não existia quando o
//! Ocinye foi compilado é tão criável como qualquer outra (§14), e que editar
//! não toca no código. Tudo por operações reais do Core — o índice de pesquisa é
//! alimentado pelos serviços, e um `INSERT` de fixture passaria por cima dele.

use ocinye_contracts::{PageRequest, TechnicalRole, UnitRole};
use ocinye_core::modules::{identity, organisation, search};
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

fn ids() -> CorrelationIds {
    CorrelationIds::generate()
}

async fn organizacao(pool: &PgPool, etiqueta: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("{etiqueta}-{}", Uuid::new_v4().simple()))
        .bind("Instituição de teste")
        .fetch_one(pool)
        .await
        .expect("organização")
}

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

async fn criar(
    pool: &PgPool,
    admin: &Principal,
    code: Option<&str>,
    name: &str,
    areas: &[&str],
) -> organisation::Unit {
    let mut tx = pool.begin().await.expect("tx");
    let unit = organisation::create_unit(
        &mut tx,
        admin,
        &ids(),
        organisation::NewUnit {
            code: code.map(ToOwned::to_owned),
            name: name.to_owned(),
            description: None,
            research_areas: areas.iter().map(|a| (*a).to_owned()).collect(),
        },
    )
    .await
    .expect("criar unidade");
    tx.commit().await.expect("commit");
    unit
}

/// O código é gerado do nome quando omitido, e o número sobe por radical.
///
/// Dois nomes cujo radical coincide (`UCS`) recebem `-001` e `-002`: a alocação
/// conta o que já existe, e não adivinha.
#[tokio::test]
async fn o_codigo_e_gerado_do_nome_e_o_numero_sobe() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "codegen").await;
    let admin = pessoa(&pool, organisation_id, &[TechnicalRole::OrganisationAdmin]).await;

    let primeira = criar(&pool, &admin, None, "Computação e Sistemas", &[]).await;
    assert_eq!(primeira.code, "UCS-001");

    // «Ciência e Sociedade» reduz-se ao mesmo radical UCS.
    let segunda = criar(&pool, &admin, None, "Ciência e Sociedade", &[]).await;
    assert_eq!(segunda.code, "UCS-002");
}

/// Um código explícito é respeitado e normalizado; um repetido é recusado.
#[tokio::test]
async fn um_codigo_explicito_e_respeitado_e_o_repetido_recusado() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "explicit").await;
    let admin = pessoa(&pool, organisation_id, &[TechnicalRole::OrganisationAdmin]).await;

    let unidade = criar(&pool, &admin, Some(" ai "), "Inteligência", &[]).await;
    assert_eq!(
        unidade.code, "AI",
        "o código explícito é aparado e normalizado"
    );

    let mut tx = pool.begin().await.expect("tx");
    let repetido = organisation::create_unit(
        &mut tx,
        &admin,
        &ids(),
        organisation::NewUnit {
            code: Some("AI".to_owned()),
            name: "Outra".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await;
    assert!(repetido.is_err(), "um código repetido tem de ser recusado");
}

/// §14: uma unidade desconhecida na compilação é tão criável como qualquer outra.
///
/// Nada no sistema conhecia «Astrofísica» quando o Ocinye foi compilado. É
/// criada, recebe um código gerado, aparece na lista, e é pesquisável — sem uma
/// única linha de código sobre ela.
#[tokio::test]
async fn uma_unidade_futura_e_criavel_e_pesquisavel() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "futura").await;
    let admin = pessoa(&pool, organisation_id, &[TechnicalRole::OrganisationAdmin]).await;

    let unidade = criar(&pool, &admin, None, "Astrofísica", &["Cosmologia"]).await;
    assert_eq!(unidade.code, "UAST-001");

    let lista = organisation::list_units(&pool, &admin, false)
        .await
        .expect("listar");
    assert!(
        lista.iter().any(|u| u.id == unidade.id),
        "a unidade nova tem de aparecer na lista"
    );

    let (hits, _) = search::search(
        &pool,
        &admin,
        "Astrofísica",
        Some(vec!["unit".to_owned()]),
        None,
        PageRequest::default(),
    )
    .await
    .expect("pesquisar");
    assert!(
        hits.iter()
            .any(|h| h.entity_id == unidade.id && h.entity_type == "unit"),
        "a unidade nova tem de ser pesquisável"
    );
}

/// Semear as unidades iniciais é idempotente, e semeia dados, não um enum.
#[tokio::test]
async fn seed_das_unidades_iniciais_e_idempotente() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "seed").await;
    let admin = pessoa(&pool, organisation_id, &[TechnicalRole::OrganisationAdmin]).await;

    let primeira = organisation::seed_initial_units(&pool, organisation_id, &ids())
        .await
        .expect("seed");
    assert_eq!(primeira, 4, "a primeira seed cria as quatro unidades");

    let segunda = organisation::seed_initial_units(&pool, organisation_id, &ids())
        .await
        .expect("seed de novo");
    assert_eq!(segunda, 0, "semear de novo não duplica nada");

    let lista = organisation::list_units(&pool, &admin, false)
        .await
        .expect("listar");
    let codigos: Vec<&str> = lista.iter().map(|u| u.code.as_str()).collect();
    for esperado in ["UAI-001", "UCS-001", "UDC-001", "UID-001"] {
        assert!(codigos.contains(&esperado), "falta a unidade {esperado}");
    }
    assert_eq!(lista.len(), 4, "só as quatro semeadas");

    // Uma unidade semeada é pesquisável pela sua área.
    let (hits, _) = search::search(
        &pool,
        &admin,
        "Inteligência",
        Some(vec!["unit".to_owned()]),
        None,
        PageRequest::default(),
    )
    .await
    .expect("pesquisar");
    assert!(
        hits.iter().any(|h| h.entity_type == "unit"),
        "a unidade semeada tem de ser pesquisável"
    );
}

/// Editar altera nome, descrição e áreas — nunca o código — e reindexa.
#[tokio::test]
async fn editar_uma_unidade_nao_lhe_muda_o_codigo() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "editar").await;
    let admin = pessoa(&pool, organisation_id, &[TechnicalRole::OrganisationAdmin]).await;

    let unidade = criar(&pool, &admin, None, "Computação e Sistemas", &["Antigo"]).await;
    let codigo_original = unidade.code.clone();

    let mut tx = pool.begin().await.expect("tx");
    let editada = organisation::update_unit(
        &mut tx,
        &admin,
        &ids(),
        unidade.id,
        organisation::UnitEdit {
            name: "Computação Avançada".to_owned(),
            description: Some("Nova descrição".to_owned()),
            research_areas: vec!["Novo".to_owned()],
        },
    )
    .await
    .expect("editar");
    tx.commit().await.expect("commit");

    assert_eq!(editada.code, codigo_original, "o código não pode mudar");
    assert_eq!(editada.name, "Computação Avançada");
    assert_eq!(editada.research_areas, vec!["Novo".to_owned()]);

    // Reindexada: encontra-se pelo nome novo, e a linha continua a ser a unidade.
    let (hits, _) = search::search(
        &pool,
        &admin,
        "Avançada",
        Some(vec!["unit".to_owned()]),
        None,
        PageRequest::default(),
    )
    .await
    .expect("pesquisar");
    assert!(
        hits.iter().any(|h| h.entity_id == unidade.id),
        "o nome novo tem de ser pesquisável"
    );
}

/// Arquivar uma unidade tira-a do índice, mas conserva a linha.
#[tokio::test]
async fn arquivar_uma_unidade_remove_a_do_indice() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "arquivar").await;
    let admin = pessoa(&pool, organisation_id, &[TechnicalRole::OrganisationAdmin]).await;

    let unidade = criar(&pool, &admin, None, "Metalurgia", &[]).await;

    let mut tx = pool.begin().await.expect("tx");
    organisation::archive_unit(&mut tx, &admin, &ids(), unidade.id)
        .await
        .expect("arquivar");
    tx.commit().await.expect("commit");

    let (hits, _) = search::search(
        &pool,
        &admin,
        "Metalurgia",
        Some(vec!["unit".to_owned()]),
        None,
        PageRequest::default(),
    )
    .await
    .expect("pesquisar");
    assert!(
        !hits.iter().any(|h| h.entity_id == unidade.id),
        "uma unidade arquivada não pode continuar no índice"
    );

    // Mas a linha fica: aparece na lista que inclui arquivadas.
    let lista = organisation::list_units(&pool, &admin, true)
        .await
        .expect("listar com arquivadas");
    assert!(
        lista.iter().any(|u| u.id == unidade.id),
        "a unidade arquivada tem de continuar a existir"
    );
}

/// A pré-visualização do código prevê o próximo, e desloca-se com o que existe.
#[tokio::test]
async fn a_pre_visualizacao_do_codigo_preve_o_proximo() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "preview").await;
    let admin = pessoa(&pool, organisation_id, &[TechnicalRole::OrganisationAdmin]).await;

    let sugerido = organisation::suggest_unit_code(&pool, &admin, "Computação e Sistemas")
        .await
        .expect("sugerir");
    assert_eq!(sugerido, "UCS-001");

    criar(&pool, &admin, None, "Computação e Sistemas", &[]).await;

    let seguinte = organisation::suggest_unit_code(&pool, &admin, "Ciência e Sociedade")
        .await
        .expect("sugerir de novo");
    assert_eq!(
        seguinte, "UCS-002",
        "a sugestão desloca-se com o que existe"
    );
}

/// Editar exige autoridade: um membro comum não pode.
#[tokio::test]
async fn editar_uma_unidade_exige_autoridade() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "auth").await;
    let admin = pessoa(&pool, organisation_id, &[TechnicalRole::OrganisationAdmin]).await;
    let membro = pessoa(&pool, organisation_id, &[TechnicalRole::ResearchMember]).await;

    let unidade = criar(&pool, &admin, None, "Robótica", &[]).await;

    let mut tx = pool.begin().await.expect("tx");
    let recusa = organisation::update_unit(
        &mut tx,
        &membro,
        &ids(),
        unidade.id,
        organisation::UnitEdit {
            name: "Roubada".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await;
    assert!(
        recusa.is_err(),
        "um membro comum não pode editar uma unidade"
    );
}

/// Um gestor de unidade pode editá-la, mesmo sem ser administrador.
///
/// Só um administrador cria unidades (o âmbito é a organização); mas depois de
/// nomear um gestor, esse gestor edita a sua unidade sem precisar de autoridade
/// administrativa — a mesma política que o deixa gerir os seus membros.
#[tokio::test]
async fn um_gestor_de_unidade_pode_edita_la() {
    let pool = skip_without_database!();
    let organisation_id = organizacao(&pool, "gestor").await;
    let admin = pessoa(&pool, organisation_id, &[TechnicalRole::OrganisationAdmin]).await;
    let gestor = pessoa(&pool, organisation_id, &[TechnicalRole::ResearchMember]).await;

    let unidade = criar(&pool, &admin, None, "Bioengenharia", &[]).await;

    // O admin nomeia o membro comum gestor da unidade.
    let mut tx = pool.begin().await.expect("tx");
    organisation::add_unit_member(
        &mut tx,
        &admin,
        &ids(),
        unidade.id,
        gestor.person_id,
        UnitRole::Manager,
    )
    .await
    .expect("nomear gestor");
    tx.commit().await.expect("commit");

    // Reler o gestor: a autoridade de unidade só conta depois de conhecida.
    let record = identity::person_by_id(&pool, gestor.person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    let gestor = identity::principal_for_person(&pool, &record)
        .await
        .expect("principal");
    assert_eq!(
        gestor.unit_roles.get(&unidade.id),
        Some(&UnitRole::Manager),
        "o membro nomeado passa a gerir a unidade"
    );

    let mut tx = pool.begin().await.expect("tx");
    let editada = organisation::update_unit(
        &mut tx,
        &gestor,
        &ids(),
        unidade.id,
        organisation::UnitEdit {
            name: "Bioengenharia Aplicada".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("o gestor edita");
    tx.commit().await.expect("commit");
    assert_eq!(editada.name, "Bioengenharia Aplicada");
}
