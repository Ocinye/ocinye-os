//! Perfis de Instância e activação de aplicações (ADR-0014).
//!
//! Duas famílias de prova:
//!
//! - **uma Instância nova por perfil**, cada uma numa base só sua — o conjunto de
//!   aplicações por omissão e a estrutura inicial vêm do perfil escolhido;
//! - **activar e desactivar** numa Instância, na base partilhada dos testes: uma
//!   organização de fixture por prova, que é o que a activação delimita.


use ocinye_contracts::{ApplicationId, InstanceProfile, TechnicalRole};
use ocinye_core::modules::organisation;
use ocinye_core::CoreError;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use sqlx::migrate::Migrator;
use sqlx::PgPool;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

fn ids() -> CorrelationIds {
    CorrelationIds::generate()
}

fn url() -> Option<String> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: os perfis ficariam por verificar"
        );
        return None;
    };
    Some(url)
}

async fn partilhada() -> Option<PgPool> {
    let pool = PgPool::connect(&url()?)
        .await
        .expect("OCINYE_TEST_DATABASE_URL está definida mas a base não responde");
    MIGRATOR.run(&pool).await.expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

/// Uma base só desta prova: `(admin, nome, pool)`.
async fn propria() -> Option<(PgPool, String, PgPool)> {
    let url = url()?;
    let admin = PgPool::connect(&url).await.expect("base de teste");
    let nome = format!("ocinye_perfil_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE {nome}"))
        .execute(&admin)
        .await
        .expect("criar a base");
    let (base, consulta) = url.split_once('?').map_or((url.as_str(), ""), |(b, q)| (b, q));
    let raiz = base.rsplit_once('/').map_or(base, |(raiz, _)| raiz);
    let alvo = if consulta.is_empty() {
        format!("{raiz}/{nome}")
    } else {
        format!("{raiz}/{nome}?{consulta}")
    };
    let pool = PgPool::connect(&alvo).await.expect("ligar");
    MIGRATOR.run(&pool).await.expect("migrations");
    Some((admin, nome, pool))
}

async fn apagar(admin: PgPool, nome: String, pool: PgPool) {
    pool.close().await;
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {nome} WITH (FORCE)"))
        .execute(&admin)
        .await;
}

async fn organizacao(pool: &PgPool) -> Uuid {
    let slug = format!("p{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn pessoa(pool: &PgPool, organisation_id: Uuid, role: TechnicalRole) -> Principal {
    let handle = format!("m{}", Uuid::new_v4().simple());
    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
         VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@exemplo.org"))
    .fetch_one(pool)
    .await
    .expect("pessoa");
    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
        .bind(person_id)
        .bind(role.as_str())
        .execute(pool)
        .await
        .expect("papel");
    let registo = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &registo)
        .await
        .expect("principal")
}

async fn activa(pool: &PgPool, organisation_id: Uuid, app: ApplicationId) -> bool {
    !organisation::inactive_applications(pool, organisation_id)
        .await
        .expect("inactivas")
        .contains(&app)
}

// ── Uma Instância nova por perfil ───────────────────────────────────────

#[tokio::test]
async fn cada_perfil_nasce_com_as_suas_aplicacoes_e_a_sua_estrutura() {
    for perfil in InstanceProfile::ALL {
        let Some((admin, nome, pool)) = propria().await else { return };
        let criada = organisation::resolve_instance(
            &pool,
            Some(&format!("instancia-{}", perfil.as_str())),
            None,
            Some(perfil),
            &ids(),
        )
        .await
        .expect("criar a instância");

        assert_eq!(organisation::profile_of(&pool, criada.id).await.expect("perfil"), perfil);
        for app in ApplicationId::ALL {
            assert_eq!(
                activa(&pool, criada.id, app).await,
                perfil.activates(app),
                "{app} no perfil {perfil:?}"
            );
        }

        let unidades: i64 =
            sqlx::query_scalar("SELECT count(*) FROM units WHERE organisation_id = $1")
                .bind(criada.id)
                .fetch_one(&pool)
                .await
                .expect("unidades");
        if perfil.seeds_initial_units() {
            assert!(unidades > 0, "o perfil de investigação nasce com unidades");
        } else {
            assert_eq!(unidades, 0, "{perfil:?} nasce sem estrutura de investigação");
        }
        apagar(admin, nome, pool).await;
    }
}

#[tokio::test]
async fn criar_uma_instancia_sem_perfil_e_recusado() {
    let Some((admin, nome, pool)) = propria().await else { return };
    let resultado =
        organisation::resolve_instance(&pool, Some("sem-perfil"), None, None, &ids()).await;
    assert!(
        matches!(resultado, Err(CoreError::Configuration(_))),
        "não há perfil por omissão; veio {resultado:?}"
    );
    let criadas: i64 = sqlx::query_scalar("SELECT count(*) FROM organisations")
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(criadas, 0, "a recusa não deixa organização nenhuma");
    apagar(admin, nome, pool).await;
}

// ── Activar e desactivar ────────────────────────────────────────────────

/// A instalação que existia é investigação, e tem tudo activo — o
/// comportamento de antes dos perfis.
#[tokio::test]
async fn uma_organizacao_existente_e_investigacao_com_tudo_activo() {
    let Some(pool) = partilhada().await else { return };
    let org = organizacao(&pool).await;
    assert_eq!(
        organisation::profile_of(&pool, org).await.expect("perfil"),
        InstanceProfile::Research
    );
    assert!(organisation::inactive_applications(&pool, org)
        .await
        .expect("inactivas")
        .is_empty());
}

#[tokio::test]
async fn desactivar_esconde_reactivar_devolve_e_repor_segue_o_perfil() {
    let Some(pool) = partilhada().await else { return };
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, TechnicalRole::PlatformAdmin).await;

    organisation::set_application_active(&pool, &admin, ApplicationId::Notes, Some(false), &ids())
        .await
        .expect("desactivar");
    assert!(!activa(&pool, org, ApplicationId::Notes).await);

    organisation::set_application_active(&pool, &admin, ApplicationId::Notes, Some(true), &ids())
        .await
        .expect("reactivar");
    assert!(activa(&pool, org, ApplicationId::Notes).await);

    // Um perfil sem Ideias; a decisão explícita sobre Notas mantém-se.
    organisation::set_profile(&pool, &admin, InstanceProfile::Business, &ids())
        .await
        .expect("mudar de perfil");
    assert!(!activa(&pool, org, ApplicationId::Ideas).await, "empresa não traz Ideias");
    assert!(activa(&pool, org, ApplicationId::Notes).await);

    // Uma decisão explícita sobrepõe o perfil, e «repor» volta a ele.
    organisation::set_application_active(&pool, &admin, ApplicationId::Ideas, Some(true), &ids())
        .await
        .expect("activar ideias");
    assert!(activa(&pool, org, ApplicationId::Ideas).await);
    organisation::set_application_active(&pool, &admin, ApplicationId::Ideas, None, &ids())
        .await
        .expect("repor");
    assert!(!activa(&pool, org, ApplicationId::Ideas).await);

    // Cada mudança ficou auditada.
    let auditadas: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events
          WHERE actor_person_id = $1 AND resource_type IN ('instance_application', 'instance_profile')",
    )
    .bind(admin.person_id)
    .fetch_one(&pool)
    .await
    .expect("auditoria");
    assert_eq!(auditadas, 5);
}

#[tokio::test]
async fn uma_aplicacao_essencial_nao_se_desactiva() {
    let Some(pool) = partilhada().await else { return };
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, TechnicalRole::PlatformAdmin).await;
    for app in ApplicationId::ALL.into_iter().filter(|a| !a.is_optional()) {
        let resultado =
            organisation::set_application_active(&pool, &admin, app, Some(false), &ids()).await;
        assert!(matches!(resultado, Err(CoreError::Conflict(_))), "{app}: {resultado:?}");
        assert!(activa(&pool, org, app).await);
    }
}

#[tokio::test]
async fn so_quem_governa_a_instancia_a_configura() {
    let Some(pool) = partilhada().await else { return };
    let org = organizacao(&pool).await;
    let membro = pessoa(&pool, org, TechnicalRole::ResearchMember).await;

    let desactivar =
        organisation::set_application_active(&pool, &membro, ApplicationId::Notes, Some(false), &ids())
            .await;
    assert!(matches!(desactivar, Err(CoreError::PermissionDenied(_))), "{desactivar:?}");
    let perfil = organisation::set_profile(&pool, &membro, InstanceProfile::Personal, &ids()).await;
    assert!(matches!(perfil, Err(CoreError::PermissionDenied(_))), "{perfil:?}");
    assert!(activa(&pool, org, ApplicationId::Notes).await);
    assert_eq!(
        organisation::profile_of(&pool, org).await.expect("perfil"),
        InstanceProfile::Research
    );
}

/// A decisão de uma Instância não toca noutra.
#[tokio::test]
async fn a_activacao_e_de_cada_instancia() {
    let Some(pool) = partilhada().await else { return };
    let a = organizacao(&pool).await;
    let b = organizacao(&pool).await;
    let admin_a = pessoa(&pool, a, TechnicalRole::PlatformAdmin).await;
    organisation::set_application_active(&pool, &admin_a, ApplicationId::Mail, Some(false), &ids())
        .await
        .expect("desactivar em A");
    assert!(!activa(&pool, a, ApplicationId::Mail).await);
    assert!(activa(&pool, b, ApplicationId::Mail).await, "B não herda a decisão de A");
}
