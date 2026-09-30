//! As invariantes de governo da organização: uma unidade não fica sem gestor, e
//! a instituição não fica sem administrador da plataforma capaz de entrar.
//!
//! # O que este ficheiro prova
//!
//! Que as duas invariantes vivem no Core, e não num botão escondido: pelas
//! operações reais, a remoção **e** a despromoção do último gestor são recusadas
//! (U-12), um gestor retirado não conta como gestor, e duas mudanças em
//! simultâneo não conseguem esvaziar nem a unidade nem a plataforma. A recusa
//! leva um motivo estável (`CoreError::Invariant`), que o cliente mapeia sem ler
//! a prosa.
//!
//! As provas de concorrência são deterministas: a primeira transacção fica
//! aberta com a mudança feita, a segunda tenta a mudança simétrica, e só depois
//! a primeira grava. Sem a tranca, a segunda contava com um gestor (ou um
//! administrador) que a primeira já tinha retirado, e as duas gravavam.

use std::time::Duration;

use ocinye_contracts::{TechnicalRole, UnitRole};
use ocinye_core::error::refusal;
use ocinye_core::modules::{identity, organisation};
use ocinye_core::CoreError;
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

async fn organizacao(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
        .bind(format!("inv-{}", Uuid::new_v4().simple()))
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
    reler(pool, person_id).await
}

/// A autoridade relida do Core, como cada pedido a relê.
async fn reler(pool: &PgPool, person_id: Uuid) -> Principal {
    let record = identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

/// Uma unidade criada pelo administrador, que fica o seu único gestor.
async fn unidade(pool: &PgPool, admin: &Principal) -> Uuid {
    let mut tx = pool.begin().await.expect("tx");
    let unit = organisation::create_unit(
        &mut tx,
        admin,
        &ids(),
        organisation::NewUnit {
            code: None,
            name: format!("Unidade {}", &Uuid::new_v4().simple().to_string()[..6]),
            description: None,
            research_areas: Vec::new(),
        },
    )
    .await
    .expect("criar unidade");
    tx.commit().await.expect("commit");
    unit.id
}

async fn papel(
    pool: &PgPool,
    actor: &Principal,
    unit: Uuid,
    person: Uuid,
    role: UnitRole,
) -> Result<(), CoreError> {
    let mut tx = pool.begin().await.expect("tx");
    organisation::add_unit_member(&mut tx, actor, &ids(), unit, person, role).await?;
    tx.commit().await.expect("commit");
    Ok(())
}

async fn retirar(
    pool: &PgPool,
    actor: &Principal,
    unit: Uuid,
    person: Uuid,
) -> Result<(), CoreError> {
    let mut tx = pool.begin().await.expect("tx");
    organisation::revoke_unit_member(&mut tx, actor, &ids(), unit, person).await?;
    tx.commit().await.expect("commit");
    Ok(())
}

async fn gestores_vivos(pool: &PgPool, unit: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM unit_memberships
          WHERE unit_id = $1 AND role = 'manager' AND revoked_at IS NULL",
    )
    .bind(unit)
    .fetch_one(pool)
    .await
    .expect("contar")
}

fn e_motivo(result: Result<(), CoreError>, reason: &str) {
    match result {
        Err(CoreError::Invariant { reason: r, .. }) => assert_eq!(r, reason),
        other => panic!("esperava a recusa `{reason}`, veio {other:?}"),
    }
}

// ── U-12 · o último gestor de uma unidade ────────────────────────────────

#[tokio::test]
async fn o_ultimo_gestor_nao_se_remove() {
    let pool = skip_without_database!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::OrganisationAdmin]).await;
    let unit = unidade(&pool, &admin).await;

    e_motivo(
        retirar(&pool, &admin, unit, admin.person_id).await,
        refusal::LAST_UNIT_MANAGER,
    );
    assert_eq!(gestores_vivos(&pool, unit).await, 1);
}

#[tokio::test]
async fn o_ultimo_gestor_nao_se_despromove() {
    let pool = skip_without_database!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::OrganisationAdmin]).await;
    let unit = unidade(&pool, &admin).await;

    // O `upsert` com o papel de membro é a despromoção: o mesmo beco que a
    // remoção, por outro caminho.
    e_motivo(
        papel(&pool, &admin, unit, admin.person_id, UnitRole::Member).await,
        refusal::LAST_UNIT_MANAGER,
    );
    assert_eq!(gestores_vivos(&pool, unit).await, 1);
    let role: String = sqlx::query_scalar(
        "SELECT role FROM unit_memberships WHERE unit_id = $1 AND person_id = $2",
    )
    .bind(unit)
    .bind(admin.person_id)
    .fetch_one(&pool)
    .await
    .expect("papel");
    assert_eq!(role, "manager", "a despromoção recusada não mudou nada");
}

#[tokio::test]
async fn com_um_segundo_gestor_o_primeiro_sai_ou_passa_a_membro() {
    let pool = skip_without_database!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::OrganisationAdmin]).await;
    let outra = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let terceira = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let unit = unidade(&pool, &admin).await;

    papel(&pool, &admin, unit, outra.person_id, UnitRole::Manager)
        .await
        .expect("segundo gestor");
    // Com outro gestor vivo, despromover é permitido.
    papel(&pool, &admin, unit, admin.person_id, UnitRole::Member)
        .await
        .expect("despromover com outro gestor");
    assert_eq!(gestores_vivos(&pool, unit).await, 1);

    // E remover também: nomeia-se de novo, e retira-se.
    papel(&pool, &admin, unit, terceira.person_id, UnitRole::Manager)
        .await
        .expect("terceiro gestor");
    retirar(&pool, &admin, unit, outra.person_id)
        .await
        .expect("remover com outro gestor");
    assert_eq!(gestores_vivos(&pool, unit).await, 1);

    // O que resta é o último: nem sai, nem passa a membro.
    e_motivo(
        retirar(&pool, &admin, unit, terceira.person_id).await,
        refusal::LAST_UNIT_MANAGER,
    );
    e_motivo(
        papel(&pool, &admin, unit, terceira.person_id, UnitRole::Member).await,
        refusal::LAST_UNIT_MANAGER,
    );
}

/// Um gestor retirado fica na tabela como memória, e não governa nada.
///
/// A contagem antiga não filtrava `revoked_at`: com um gestor retirado no
/// histórico, o último gestor vivo removia-se, e a unidade ficava sem quem a
/// governe.
#[tokio::test]
async fn um_gestor_retirado_nao_conta_como_gestor() {
    let pool = skip_without_database!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::OrganisationAdmin]).await;
    let outra = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let unit = unidade(&pool, &admin).await;

    papel(&pool, &admin, unit, outra.person_id, UnitRole::Manager)
        .await
        .expect("segundo gestor");
    retirar(&pool, &admin, unit, admin.person_id)
        .await
        .expect("o primeiro sai");

    e_motivo(
        retirar(&pool, &admin, unit, outra.person_id).await,
        refusal::LAST_UNIT_MANAGER,
    );
    assert_eq!(gestores_vivos(&pool, unit).await, 1);
}

/// Dois gestores a despromoverem-se um ao outro ao mesmo tempo.
#[tokio::test]
async fn duas_despromocoes_em_simultaneo_nao_esvaziam_a_unidade() {
    let pool = skip_without_database!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::OrganisationAdmin]).await;
    let outra = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let unit = unidade(&pool, &admin).await;
    papel(&pool, &admin, unit, outra.person_id, UnitRole::Manager)
        .await
        .expect("segundo gestor");

    // A primeira despromove a outra pessoa e fica aberta.
    let mut primeira = pool.begin().await.expect("tx");
    organisation::add_unit_member(
        &mut primeira,
        &admin,
        &ids(),
        unit,
        outra.person_id,
        UnitRole::Member,
    )
    .await
    .expect("a primeira despromove");

    // A segunda tenta despromover o administrador, enquanto a primeira está
    // aberta: tem de esperar por ela, e ler o que ela gravou.
    let segunda = {
        let pool = pool.clone();
        let admin = admin.clone();
        tokio::spawn(
            async move { papel(&pool, &admin, unit, admin.person_id, UnitRole::Member).await },
        )
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !segunda.is_finished(),
        "a segunda esperou pela tranca da unidade"
    );
    primeira.commit().await.expect("a primeira grava");

    e_motivo(segunda.await.expect("tarefa"), refusal::LAST_UNIT_MANAGER);
    assert_eq!(gestores_vivos(&pool, unit).await, 1);
}

// ── O último administrador da plataforma ─────────────────────────────────

async fn admins_vivos(pool: &PgPool, org: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM person_roles pr JOIN people p ON p.id = pr.person_id
          WHERE pr.role = 'platform_admin' AND pr.revoked_at IS NULL
            AND p.organisation_id = $1 AND p.status IN ('invited', 'active')",
    )
    .bind(org)
    .fetch_one(pool)
    .await
    .expect("contar")
}

/// Dois administradores a retirarem o papel um ao outro ao mesmo tempo.
///
/// Cada um, sozinho, é permitido — o outro continua. Em simultâneo, sem tranca,
/// cada transacção via o outro ainda administrador, e a plataforma ficava sem
/// nenhum.
#[tokio::test]
async fn duas_revogacoes_em_simultaneo_nao_esvaziam_a_plataforma() {
    let pool = skip_without_database!();
    let org = organizacao(&pool).await;
    let a = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let b = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    assert_eq!(admins_vivos(&pool, org).await, 2);

    let mut primeira = pool.begin().await.expect("tx");
    identity::revoke_role(
        &mut primeira,
        &a,
        &ids(),
        b.person_id,
        TechnicalRole::PlatformAdmin,
    )
    .await
    .expect("A retira o papel a B");

    let segunda = {
        let pool = pool.clone();
        let b = b.clone();
        let alvo = a.person_id;
        tokio::spawn(async move {
            let mut tx = pool.begin().await.expect("tx");
            identity::revoke_role(&mut tx, &b, &ids(), alvo, TechnicalRole::PlatformAdmin).await?;
            tx.commit().await.expect("commit");
            Ok::<(), CoreError>(())
        })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !segunda.is_finished(),
        "a segunda esperou pela tranca da instituição"
    );
    primeira.commit().await.expect("a primeira grava");

    e_motivo(segunda.await.expect("tarefa"), refusal::LAST_PLATFORM_ADMIN);
    assert_eq!(admins_vivos(&pool, org).await, 1);
}

/// O último administrador não se despromove a si próprio, nem é despromovido.
#[tokio::test]
async fn o_ultimo_administrador_nao_perde_o_papel() {
    let pool = skip_without_database!();
    let org = organizacao(&pool).await;
    let a = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;

    let mut tx = pool.begin().await.expect("tx");
    let result = identity::revoke_role(
        &mut tx,
        &a,
        &ids(),
        a.person_id,
        TechnicalRole::PlatformAdmin,
    )
    .await;
    drop(tx);
    e_motivo(result, refusal::LAST_PLATFORM_ADMIN);
    assert_eq!(admins_vivos(&pool, org).await, 1);
}

// ── O ciclo de vida da conta ─────────────────────────────────────────────

async fn estado(pool: &PgPool, person: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM people WHERE id = $1")
        .bind(person)
        .fetch_one(pool)
        .await
        .expect("estado")
}

async fn mudar(
    pool: &PgPool,
    actor: &Principal,
    person: Uuid,
    status: ocinye_contracts::AccountStatus,
) -> Result<(), CoreError> {
    let alvo = identity::person_by_id(pool, person)
        .await
        .expect("consulta")
        .expect("pessoa");
    identity::set_account_status(pool, actor, &alvo, status, "motivo de teste", &ids()).await
}

/// `invited` não é um destino, e `disabled` não tem volta (docs/identity). O
/// formulário de estado aceitava qualquer um dos quatro; agora é o Core que diz
/// não, com a transição tipada, e nada muda na base.
#[tokio::test]
async fn o_estado_da_conta_segue_o_ciclo_documentado() {
    use ocinye_contracts::AccountStatus;
    let pool = skip_without_database!();
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    let alvo = pessoa(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let recusa = mudar(&pool, &admin, alvo.person_id, AccountStatus::Invited).await;
    assert!(
        matches!(
            recusa,
            Err(CoreError::Domain(
                ocinye_domain::DomainError::InvalidTransition {
                    from: "active",
                    to: "invited"
                }
            ))
        ),
        "{recusa:?}"
    );
    assert_eq!(estado(&pool, alvo.person_id).await, "active");

    mudar(&pool, &admin, alvo.person_id, AccountStatus::Suspended)
        .await
        .expect("suspender");
    mudar(&pool, &admin, alvo.person_id, AccountStatus::Active)
        .await
        .expect("reactivar uma suspensa");
    mudar(&pool, &admin, alvo.person_id, AccountStatus::Disabled)
        .await
        .expect("desactivar");

    let recusa = mudar(&pool, &admin, alvo.person_id, AccountStatus::Active).await;
    assert!(
        matches!(
            recusa,
            Err(CoreError::Domain(
                ocinye_domain::DomainError::InvalidTransition {
                    from: "disabled",
                    ..
                }
            ))
        ),
        "{recusa:?}"
    );
    assert_eq!(estado(&pool, alvo.person_id).await, "disabled");
}
