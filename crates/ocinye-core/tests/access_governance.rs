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
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
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
