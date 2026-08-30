//! Governação de acesso: bootstrap de autoridade e gestão de pertenças.
//!
//! > **A criação de um recurso estabelece a autoridade mínima legítima de que o
//! > criador precisa para continuar a operá-lo.**
//!
//! Nunca auto-elevação arbitrária: o que se estabelece é o que o domínio exige
//! para o recurso não nascer ingovernável.

use ocinye_contracts::{TechnicalRole, UnitRole};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        // Em CI, a ausência da dependência não pode converter o teste em verde
        // por skip. Um teste que se ignora a si próprio passa, e `cargo test`
        // esconde a saída de quem passa: o skip seria invisível.
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: a governação de acesso \
             ficaria por verificar e a suite reportaria verde"
        );
        return None;
    };
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL está definida mas a base não responde");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("g{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn person(pool: &PgPool, organisation_id: Uuid, roles: &[TechnicalRole]) -> Principal {
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

    relido(pool, person_id).await
}

/// Relê o principal da base, como o executor faz antes de cada operação.
async fn relido(pool: &PgPool, person_id: Uuid) -> Principal {
    let pessoa = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &pessoa)
        .await
        .expect("principal")
}

// ── Bootstrap de unidade ────────────────────────────────────────────────

/// Criar uma unidade deixa-a governável por quem a criou.
///
/// # O beco sem saída que este teste fecha
///
/// Criar uma unidade não criava pertença nenhuma. O resultado era uma unidade
/// que existia e que **ninguém** podia gerir: acrescentar membros exige
/// `ManageMembers` no contexto da unidade, e esse direito vem de ser Manager
/// dela. Quem a criava ficava de fora do que acabara de criar, e a única saída
/// era escrever na base por fora — que é exactamente o que não pode acontecer.
///
/// Isto **não** é auto-elevação: é a autoridade mínima que o domínio exige para
/// o recurso não nascer ingovernável. Quem não pode criar unidades continua sem
/// poder criar nenhuma.
#[tokio::test]
async fn quem_cria_uma_unidade_pode_continuar_a_geri_la() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();

    let admin = person(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let outro = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let unidade = ocinye_core::modules::organisation::create_unit(
        &mut tx,
        &admin,
        &ids,
        ocinye_core::modules::organisation::NewUnit {
            code: format!("U{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase(),
            name: "Unidade de prova".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("um administrador devia poder criar uma unidade");
    tx.commit().await.expect("commit");

    // É Manager, e é-o na base — não só na memória de quem criou.
    let papel: Option<String> = sqlx::query_scalar(
        "SELECT role FROM unit_memberships WHERE unit_id = $1 AND person_id = $2",
    )
    .bind(unidade.id)
    .bind(admin.person_id)
    .fetch_optional(&pool)
    .await
    .expect("consulta");
    assert_eq!(
        papel.as_deref(),
        Some("manager"),
        "quem criou a unidade não ficou a poder geri-la"
    );

    // E consegue mesmo continuar: acrescentar alguém exige `ManageMembers`.
    let admin = relido(&pool, admin.person_id).await;
    let mut tx = pool.begin().await.expect("tx");
    ocinye_core::modules::organisation::add_unit_member(
        &mut tx,
        &admin,
        &ids,
        unidade.id,
        outro.person_id,
        UnitRole::Member,
    )
    .await
    .expect("quem criou a unidade não consegue acrescentar membros a ela");
    tx.commit().await.expect("commit");
}

/// Quem não pode criar unidades continua sem poder.
#[tokio::test]
async fn o_bootstrap_nao_e_uma_porta_para_criar_unidades() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();

    let membro = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let mut tx = pool.begin().await.expect("tx");
    let recusa = ocinye_core::modules::organisation::create_unit(
        &mut tx,
        &membro,
        &ids,
        ocinye_core::modules::organisation::NewUnit {
            code: format!("U{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase(),
            name: "Não devia existir".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await;

    assert!(
        recusa.is_err(),
        "um membro de investigação criou uma unidade"
    );
}

/// Ninguém se acrescenta a uma unidade alheia, nem se promove.
///
/// Conhecer o identificador de uma unidade não é autoridade sobre ela.
#[tokio::test]
async fn ninguem_se_acrescenta_nem_se_promove_numa_unidade_alheia() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();

    let admin = person(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let mut tx = pool.begin().await.expect("tx");
    let unidade = ocinye_core::modules::organisation::create_unit(
        &mut tx,
        &admin,
        &ids,
        ocinye_core::modules::organisation::NewUnit {
            code: format!("U{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase(),
            name: "Unidade fechada".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("unidade");
    tx.commit().await.expect("commit");

    let estranho = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    // Acrescentar-se a si próprio, conhecendo o identificador.
    let mut tx = pool.begin().await.expect("tx");
    let recusa = ocinye_core::modules::organisation::add_unit_member(
        &mut tx,
        &estranho,
        &ids,
        unidade.id,
        estranho.person_id,
        UnitRole::Member,
    )
    .await;
    assert!(
        recusa.is_err(),
        "alguém acrescentou-se a uma unidade alheia"
    );

    // E promover-se a Manager também não.
    let recusa = ocinye_core::modules::organisation::add_unit_member(
        &mut tx,
        &estranho,
        &ids,
        unidade.id,
        estranho.person_id,
        UnitRole::Manager,
    )
    .await;
    assert!(
        recusa.is_err(),
        "alguém promoveu-se a gestor de uma unidade alheia"
    );
}

// ── Bootstrap de ambiente ───────────────────────────────────────────────

/// Criar uma ideia deixa o ambiente utilizável por quem a criou.
///
/// Esta propriedade **já existia** no domínio: `create_idea` torna o criador
/// `Lead` e actualiza o principal na mesma operação. O teste existe para que
/// continue a existir — foi o único bootstrap que estava feito, e é o modelo do
/// que faltava à unidade.
#[tokio::test]
async fn quem_cria_uma_ideia_pode_continuar_a_usar_o_ambiente() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();

    let admin = person(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let mut tx = pool.begin().await.expect("tx");
    let unidade = ocinye_core::modules::organisation::create_unit(
        &mut tx,
        &admin,
        &ids,
        ocinye_core::modules::organisation::NewUnit {
            code: format!("U{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase(),
            name: "Unidade".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("unidade");
    tx.commit().await.expect("commit");

    let mut admin = relido(&pool, admin.person_id).await;
    let mut tx = pool.begin().await.expect("tx");
    let (_, ambiente) = ocinye_core::modules::research::create_idea(
        &mut tx,
        &mut admin,
        &ids,
        ocinye_core::modules::research::NewIdea {
            unit_id: unidade.id,
            title: "Ideia de prova".to_owned(),
            summary: None,
            research_question: None,
            hypothesis: None,
            motivation: None,
            keywords: Vec::new(),
            classification: None,
        },
    )
    .await
    .expect("criar ideia");
    tx.commit().await.expect("commit");

    let papel: Option<String> = sqlx::query_scalar(
        "SELECT role FROM workspace_memberships WHERE workspace_id = $1 AND person_id = $2",
    )
    .bind(ambiente.id)
    .bind(admin.person_id)
    .fetch_optional(&pool)
    .await
    .expect("consulta");
    assert_eq!(
        papel.as_deref(),
        Some("lead"),
        "quem criou a ideia não ficou a liderar o ambiente"
    );

    // E alcança-o de facto: ler o ambiente é a operação seguinte.
    let admin = relido(&pool, admin.person_id).await;
    ocinye_core::modules::research::get_workspace(&pool, &admin, ambiente.id)
        .await
        .expect("quem criou a ideia não alcança o ambiente dela");
}

// ── A matriz de personas ────────────────────────────────────────────────
//
// Dois eixos, e nunca uma tabela só.
//
//   relevância    «este módulo pertence ao trabalho desta pessoa?»
//   autorização   «esta pessoa pode ver ou fazer isto, aqui?»
//
// Uma tabela única onde «Ficheiros = true» seria lida como ACL daqui a seis
// meses, e é exactamente esse o erro que esta milestone existe para corrigir.

use ocinye_domain::policy::relevance::{is_relevant, Module};

/// Quem faz investigação conhece o espaço onde ela acontece — tenha ou não
/// trabalho atribuído hoje.
///
/// O principal é construído pelo caminho real (`relido`), e não à mão: um
/// principal sintético provaria a tabela deste ficheiro, não o sistema.
#[tokio::test]
async fn a_relevancia_deriva_do_papel_e_nao_da_pertenca() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;

    let casos: &[(TechnicalRole, bool)] = &[
        (TechnicalRole::ResearchMember, true),
        (TechnicalRole::ResearchLead, true),
        (TechnicalRole::UnitManager, true),
        (TechnicalRole::OrganisationAdmin, true),
        // Administrar a plataforma não é fazer investigação. É o mesmo
        // princípio que impede um papel administrativo de ler RESTRICTED.
        (TechnicalRole::PlatformAdmin, false),
        (TechnicalRole::Collaborator, false),
        (TechnicalRole::ExternalCollaborator, false),
        (TechnicalRole::Auditor, false),
    ];

    for (papel, esperado) in casos {
        let principal = person(&pool, org, &[*papel]).await;

        for modulo in [
            Module::Files,
            Module::Knowledge,
            Module::Bibliography,
            Module::Datasets,
        ] {
            assert_eq!(
                is_relevant(&principal, modulo),
                *esperado,
                "{}: {} devia ser {}",
                papel.as_str(),
                modulo.as_str(),
                if *esperado {
                    "relevante"
                } else {
                    "irrelevante"
                }
            );
        }
    }
}

/// A relevância não muda quando a pertença muda.
///
/// É a metade que impede o eixo de colapsar no outro: se acrescentar uma
/// pertença mudasse a relevância, os dois eixos seriam o mesmo com nomes
/// diferentes — e a navegação voltaria a ser uma ACL.
#[tokio::test]
async fn a_relevancia_nao_muda_quando_a_pertenca_muda() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();

    let admin = person(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    // A persona tem de ser alguém para quem a relevância é `false`. Com um
    // membro de investigação — relevante desde o início — o colapso dos dois
    // eixos passaria despercebido: `true` continuaria `true`.
    let membro = person(&pool, org, &[TechnicalRole::Collaborator]).await;
    assert!(
        !is_relevant(&membro, Module::Files),
        "a persona escolhida já era relevante; o teste não conseguiria ver o colapso"
    );

    let antes: Vec<_> = Module::all()
        .into_iter()
        .map(|m| (m, is_relevant(&membro, m)))
        .collect();

    // Uma unidade, e a pessoa lá dentro.
    let mut tx = pool.begin().await.expect("tx");
    let unidade = ocinye_core::modules::organisation::create_unit(
        &mut tx,
        &admin,
        &ids,
        ocinye_core::modules::organisation::NewUnit {
            code: format!("U{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase(),
            name: "Unidade".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("unidade");
    tx.commit().await.expect("commit");

    let admin = relido(&pool, admin.person_id).await;
    let mut tx = pool.begin().await.expect("tx");
    ocinye_core::modules::organisation::add_unit_member(
        &mut tx,
        &admin,
        &ids,
        unidade.id,
        membro.person_id,
        UnitRole::Member,
    )
    .await
    .expect("pertença");
    tx.commit().await.expect("commit");

    // A autorização mudou: agora tem `DocumentsView` no contexto da unidade.
    let membro = relido(&pool, membro.person_id).await;
    assert!(
        !membro.unit_roles.is_empty(),
        "a pertença não chegou ao principal"
    );

    // A relevância não mudou. Era relevante antes, e continua a ser.
    let depois: Vec<_> = Module::all()
        .into_iter()
        .map(|m| (m, is_relevant(&membro, m)))
        .collect();
    assert_eq!(
        antes, depois,
        "a relevância mudou com a pertença; os dois eixos colapsaram num só"
    );
}

/// Administrar a plataforma não concede leitura de material RESTRICTED.
///
/// Esta é a política existente, e este teste está aqui para que continue a
/// existir depois de toda esta milestone lhe ter mexido à volta.
#[tokio::test]
async fn administrar_a_plataforma_nao_da_acesso_a_investigacao_restrita() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();

    let dono = person(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let mut tx = pool.begin().await.expect("tx");
    let unidade = ocinye_core::modules::organisation::create_unit(
        &mut tx,
        &dono,
        &ids,
        ocinye_core::modules::organisation::NewUnit {
            code: format!("U{}", &Uuid::new_v4().simple().to_string()[..6]).to_uppercase(),
            name: "Unidade".to_owned(),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("unidade");
    tx.commit().await.expect("commit");

    let mut dono = relido(&pool, dono.person_id).await;
    let mut tx = pool.begin().await.expect("tx");
    let (_, ambiente) = ocinye_core::modules::research::create_idea(
        &mut tx,
        &mut dono,
        &ids,
        ocinye_core::modules::research::NewIdea {
            unit_id: unidade.id,
            title: "Trabalho restrito".to_owned(),
            summary: None,
            research_question: None,
            hypothesis: None,
            motivation: None,
            keywords: Vec::new(),
            classification: Some(ocinye_contracts::Classification::Restricted),
        },
    )
    .await
    .expect("ideia");
    tx.commit().await.expect("commit");

    // Outro administrador de plataforma, sem pertença nenhuma.
    let outro_admin = person(&pool, org, &[TechnicalRole::PlatformAdmin]).await;

    let alcanca = ocinye_core::modules::research::get_workspace(&pool, &outro_admin, ambiente.id)
        .await
        .is_ok();
    assert!(
        !alcanca,
        "um administrador de plataforma alcançou um ambiente RESTRICTED sem pertença"
    );
}
