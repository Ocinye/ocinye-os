//! D010 · Uma Instância, várias Distribuições; acesso por membro; pontos de
//! acesso; contexto da sessão (ADR-0019, ADR-0020, ADR-0625).
//!
//! Na base partilhada dos testes, uma organização de fixture por prova. Cada
//! invariante é provada pelo lado que recusa — e as que têm guarda dupla (o
//! Core e a base) pelos dois lados.

use ocinye_contracts::desktop::{DesktopLayout, PlacedWidget};
use ocinye_contracts::{Distribution, TechnicalRole};
use ocinye_core::modules::identity;
use ocinye_core::modules::organisation::{contexts, distributions, endpoints};
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

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: a D010 ficaria por verificar"
        );
        return None;
    };
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL está definida mas a base não responde");
    MIGRATOR.run(&pool).await.expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

/// Uma organização nova: nasce com a sua Distribuição (`research`) activada.
async fn organizacao(pool: &PgPool) -> Uuid {
    let slug = format!("d{}", Uuid::new_v4().simple());
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
    let registo = identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    identity::principal_for_person(pool, &registo)
        .await
        .expect("principal")
}

async fn sessao(pool: &PgPool, person_id: Uuid) -> Uuid {
    let digest = format!("{:064x}", u128::from_le_bytes(*Uuid::new_v4().as_bytes()));
    sqlx::query_scalar(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at)
         VALUES ($1, $2, 'active', now() + interval '1 hour') RETURNING id",
    )
    .bind(person_id)
    .bind(digest)
    .fetch_one(pool)
    .await
    .expect("sessão")
}

fn recusa(r: Result<(), CoreError>) -> &'static str {
    match r {
        Err(CoreError::Invariant { reason, .. }) => reason,
        outro => panic!("esperava uma recusa tipada, veio {outro:?}"),
    }
}

fn layout(kind: &str) -> DesktopLayout {
    DesktopLayout {
        wallpaper: "org".to_owned(),
        fit: "fill".to_owned(),
        dim: 20,
        widgets: vec![PlacedWidget {
            id: kind.to_owned(),
            kind: kind.to_owned(),
            w: 1,
            h: 2,
            minimized: false,
        }],
    }
}

async fn acesso(pool: &PgPool, p: &Principal) -> Vec<Distribution> {
    distributions::member_access(pool, p.organisation_id, p.person_id)
        .await
        .expect("acesso")
        .iter()
        .collect()
}

// ── Activar, desactivar, a última ─────────────────────────────────────────

/// Activar é aditivo: quem activa recebe acesso, os outros não — e o
/// conjunto activado, o acessível e o disponível são três coisas.
#[tokio::test]
async fn activar_da_acesso_so_a_quem_activa() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, TechnicalRole::PlatformAdmin).await;
    let membro = pessoa(&pool, org, TechnicalRole::ResearchMember).await;
    // Uma só activada: um membro novo entra nela sem escolher (trigger 0060).
    assert_eq!(acesso(&pool, &membro).await, vec![Distribution::Research]);

    distributions::enable(&pool, &admin, Distribution::Business, &ids())
        .await
        .expect("activar");
    let activadas: Vec<_> = distributions::enabled(&pool, org)
        .await
        .expect("activadas")
        .iter()
        .collect();
    assert_eq!(
        activadas,
        vec![Distribution::Research, Distribution::Business]
    );
    assert!(acesso(&pool, &admin)
        .await
        .contains(&Distribution::Business));
    assert_eq!(acesso(&pool, &membro).await, vec![Distribution::Research]);

    // Com duas activadas, um membro novo não recebe acesso nenhum: a escolha
    // é de quem administra (S37), não de uma regra escondida.
    let novo = pessoa(&pool, org, TechnicalRole::ResearchMember).await;
    assert!(acesso(&pool, &novo).await.is_empty());

    // Sem `organisation.manage`, nem activar.
    assert!(matches!(
        distributions::enable(&pool, &membro, Distribution::Personal, &ids()).await,
        Err(CoreError::PermissionDenied(_))
    ));
}

/// A última Distribuição activada não se desactiva — no Core e na base.
#[tokio::test]
async fn a_ultima_activada_nao_se_desactiva_nem_pelo_core_nem_pela_base() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, TechnicalRole::PlatformAdmin).await;

    assert_eq!(
        recusa(distributions::disable(&pool, &admin, Distribution::Research, &ids()).await),
        "last_enabled_distribution"
    );
    // Uma actualização que cumpre todas as outras regras da tabela (o estado
    // e a data andam juntos): só o trigger a pode recusar.
    let directo = sqlx::query(
        "UPDATE instance_distributions SET state = 'disabled', disabled_at = now()
          WHERE organisation_id = $1",
    )
    .bind(org)
    .execute(&pool)
    .await;
    match directo {
        Err(sqlx::Error::Database(e)) => assert!(
            e.message().contains("Distribui") || e.message().contains("distribution"),
            "recusada, mas não pelo trigger: {}",
            e.message()
        ),
        outro => panic!("o trigger deixou desactivar a última: {outro:?}"),
    }
    let ainda: Vec<_> = distributions::enabled(&pool, org)
        .await
        .expect("activadas")
        .iter()
        .collect();
    assert_eq!(ainda, vec![Distribution::Research]);
}

/// Desactivar guarda tudo — acesso, disposição, fixações — e a sessão que lá
/// estava deixa de valer no pedido seguinte (S39).
#[tokio::test]
async fn desactivar_guarda_o_estado_e_recusa_a_sessao_seguinte() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, TechnicalRole::PlatformAdmin).await;
    distributions::enable(&pool, &admin, Distribution::Business, &ids())
        .await
        .expect("activar");
    identity::put_desktop(
        &pool,
        &admin,
        0,
        &layout("tasks"),
        true,
        Distribution::Business,
    )
    .await
    .expect("disposição");
    identity::set_app_pins(&pool, &admin, &["files".to_owned()], Distribution::Business)
        .await
        .expect("fixações");

    distributions::disable(&pool, &admin, Distribution::Business, &ids())
        .await
        .expect("desactivar");
    assert_eq!(
        recusa(
            distributions::revalidate(&pool, org, admin.person_id, Distribution::Business).await
        ),
        "distribution_not_enabled"
    );
    assert!(
        acesso(&pool, &admin)
            .await
            .contains(&Distribution::Business),
        "o acesso foi apagado"
    );

    distributions::enable(&pool, &admin, Distribution::Business, &ids())
        .await
        .expect("reactivar");
    let volta = identity::get_desktop(&pool, &admin, Distribution::Business)
        .await
        .expect("ler")
        .expect("a disposição perdeu-se");
    assert_eq!(volta.layout, layout("tasks"));
    assert_eq!(
        identity::list_app_pins(&pool, &admin, Distribution::Business)
            .await
            .expect("ler"),
        Some(vec!["files".to_owned()])
    );
}

// ── Acesso por membro ─────────────────────────────────────────────────────

/// Retirar o acesso ao último administrador que entra é recusado; a outro
/// membro, a sessão dele deixa de valer (S18).
#[tokio::test]
async fn retirar_acesso_protege_o_ultimo_administrador() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, TechnicalRole::PlatformAdmin).await;
    let membro = pessoa(&pool, org, TechnicalRole::ResearchMember).await;

    assert_eq!(
        recusa(
            distributions::revoke(
                &pool,
                &admin,
                admin.person_id,
                Distribution::Research,
                &ids()
            )
            .await
        ),
        "last_administrator_access"
    );
    assert!(
        acesso(&pool, &admin)
            .await
            .contains(&Distribution::Research),
        "a recusa apagou o acesso"
    );

    distributions::revoke(
        &pool,
        &admin,
        membro.person_id,
        Distribution::Research,
        &ids(),
    )
    .await
    .expect("retirar");
    assert_eq!(
        recusa(
            distributions::revalidate(&pool, org, membro.person_id, Distribution::Research).await
        ),
        "distribution_no_access"
    );
    // Dar acesso a uma que não está activada é recusado.
    assert_eq!(
        recusa(
            distributions::grant(
                &pool,
                &admin,
                membro.person_id,
                Distribution::Education,
                &ids()
            )
            .await
        ),
        "distribution_not_enabled"
    );
    distributions::grant(
        &pool,
        &admin,
        membro.person_id,
        Distribution::Research,
        &ids(),
    )
    .await
    .expect("dar");
    assert!(
        distributions::revalidate(&pool, org, membro.person_id, Distribution::Research)
            .await
            .is_ok()
    );
}

/// Uma pessoa de outra Instância não recebe acesso a esta (a FK composta).
#[tokio::test]
async fn acesso_nunca_atravessa_instancias() {
    let Some(pool) = pool().await else { return };
    let a = organizacao(&pool).await;
    let b = organizacao(&pool).await;
    let admin_a = pessoa(&pool, a, TechnicalRole::PlatformAdmin).await;
    let de_b = pessoa(&pool, b, TechnicalRole::ResearchMember).await;
    assert!(matches!(
        distributions::grant(
            &pool,
            &admin_a,
            de_b.person_id,
            Distribution::Research,
            &ids()
        )
        .await,
        Err(CoreError::NotFound(_))
    ));
    let directo = sqlx::query(
        "INSERT INTO member_distribution_access (organisation_id, person_id, distribution)
         VALUES ($1, $2, 'research')",
    )
    .bind(a)
    .bind(de_b.person_id)
    .execute(&pool)
    .await;
    assert!(directo.is_err(), "a base aceitou acesso de outra Instância");
}

// ── Estado por Distribuição ───────────────────────────────────────────────

/// Disposição, fundo e fixações são por (membro, Distribuição): gravar numa
/// não toca na outra.
#[tokio::test]
async fn disposicao_e_fixacoes_sao_por_distribuicao() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, TechnicalRole::PlatformAdmin).await;
    distributions::enable(&pool, &admin, Distribution::Business, &ids())
        .await
        .expect("activar");

    identity::put_desktop(
        &pool,
        &admin,
        0,
        &layout("tasks"),
        true,
        Distribution::Research,
    )
    .await
    .expect("research");
    let mut outra = layout("calendar");
    outra.wallpaper = "module".to_owned();
    identity::put_desktop(&pool, &admin, 0, &outra, true, Distribution::Business)
        .await
        .expect("business");
    let r = identity::get_desktop(&pool, &admin, Distribution::Research)
        .await
        .expect("ler")
        .expect("r");
    let b = identity::get_desktop(&pool, &admin, Distribution::Business)
        .await
        .expect("ler")
        .expect("b");
    assert_eq!(r.layout, layout("tasks"));
    assert_eq!(b.layout.wallpaper, "module", "o fundo é da Distribuição");
    assert_eq!(r.layout.wallpaper, "org");

    identity::set_app_pins(&pool, &admin, &["notes".to_owned()], Distribution::Research)
        .await
        .expect("fixar");
    assert_eq!(
        identity::list_app_pins(&pool, &admin, Distribution::Business)
            .await
            .expect("ler"),
        None,
        "fixar em Research fixou em Business"
    );
}

// ── Sessão: entrar, contexto ──────────────────────────────────────────────

/// Entrar numa Distribuição grava-a na sessão e repõe o contexto; entrar
/// numa sem acesso é recusado, e a sessão fica como estava.
#[tokio::test]
async fn entrar_grava_a_distribuicao_e_repoe_o_contexto() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, TechnicalRole::PlatformAdmin).await;
    let membro = pessoa(&pool, org, TechnicalRole::ResearchMember).await;
    distributions::enable(&pool, &admin, Distribution::Business, &ids())
        .await
        .expect("activar");
    let s = sessao(&pool, membro.person_id).await;

    assert_eq!(
        recusa(distributions::enter(&pool, &membro, s, Distribution::Business, &ids()).await),
        "distribution_no_access"
    );
    distributions::enter(&pool, &membro, s, Distribution::Research, &ids())
        .await
        .expect("entrar");
    contexts::choose(
        &pool,
        &membro,
        s,
        Some(Distribution::Research),
        Some("organisation"),
        None,
        &ids(),
    )
    .await
    .expect("contexto");
    let (d, k): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT active_distribution, active_context_kind FROM sessions WHERE id = $1",
    )
    .bind(s)
    .fetch_one(&pool)
    .await
    .expect("sessão");
    assert_eq!(
        (d.as_deref(), k.as_deref()),
        (Some("research"), Some("organisation"))
    );

    // Um contexto que o membro não tem é recusado.
    assert!(contexts::choose(
        &pool,
        &membro,
        s,
        Some(Distribution::Research),
        Some("unit"),
        Some(Uuid::new_v4()),
        &ids()
    )
    .await
    .is_err());

    distributions::grant(
        &pool,
        &admin,
        membro.person_id,
        Distribution::Business,
        &ids(),
    )
    .await
    .expect("dar");
    distributions::enter(&pool, &membro, s, Distribution::Business, &ids())
        .await
        .expect("mudar");
    let k: Option<String> =
        sqlx::query_scalar("SELECT active_context_kind FROM sessions WHERE id = $1")
            .bind(s)
            .fetch_one(&pool)
            .await
            .expect("sessão");
    assert_eq!(k, None, "mudar de Distribuição transportou o contexto");
}

// ── Pontos de acesso ──────────────────────────────────────────────────────

/// Um ponto nasce por verificar; o nome é validado e único; um ponto fixo só
/// numa Distribuição activada; o canónico não se desactiva.
#[tokio::test]
async fn pontos_de_acesso_validam_e_protegem_o_canonico() {
    let Some(pool) = pool().await else { return };
    let org = organizacao(&pool).await;
    let admin = pessoa(&pool, org, TechnicalRole::PlatformAdmin).await;
    let marca = Uuid::new_v4().simple().to_string();
    let canonico = format!("os-{}.exemplo.test", &marca[..12]);
    assert!(
        endpoints::seed(&pool, org, &format!("https://{canonico}:8443/x"), &ids())
            .await
            .expect("semear")
    );
    assert!(
        !endpoints::seed(&pool, org, "https://outro.exemplo.test", &ids())
            .await
            .expect("semear"),
        "semear duas vezes criou outro canónico"
    );

    for mau in ["localhost", "https://a.exemplo.test/", "a b.test", ""] {
        assert!(
            endpoints::create(&pool, &admin, mau, None, &ids())
                .await
                .is_err(),
            "{mau:?} foi aceite"
        );
    }
    assert!(endpoints::create(&pool, &admin, &canonico, None, &ids())
        .await
        .is_err());
    assert!(
        endpoints::create(
            &pool,
            &admin,
            &format!("b-{canonico}"),
            Some(Distribution::Business),
            &ids()
        )
        .await
        .is_err(),
        "fixo numa Distribuição não activada"
    );
    let novo = endpoints::create(&pool, &admin, &format!("labs-{canonico}"), None, &ids())
        .await
        .expect("criar");
    assert_eq!(novo.state, "unverified");

    let lista = endpoints::list(&pool, &admin).await.expect("lista");
    let c = lista.iter().find(|e| e.canonical).expect("canónico");
    assert!(endpoints::disable(&pool, &admin, c.id, &ids())
        .await
        .is_err());
    endpoints::disable(&pool, &admin, novo.id, &ids())
        .await
        .expect("desactivar");
}

/// Resolver um nome só responde pela Instância que este Core serve.
#[tokio::test]
async fn resolver_e_da_instancia_servida() {
    let Some(pool) = pool().await else { return };
    let a = organizacao(&pool).await;
    let b = organizacao(&pool).await;
    let host = format!(
        "a-{}.exemplo.test",
        &Uuid::new_v4().simple().to_string()[..12]
    );
    endpoints::seed(&pool, a, &format!("https://{host}"), &ids())
        .await
        .expect("semear");
    assert!(endpoints::resolve(&pool, a, &host)
        .await
        .expect("a")
        .is_some());
    assert!(
        endpoints::resolve(&pool, b, &host)
            .await
            .expect("b")
            .is_none(),
        "um Core resolveu o nome de outra Instância"
    );
    assert!(endpoints::resolve(&pool, a, "desconhecido.exemplo.test")
        .await
        .expect("x")
        .is_none());
    assert!(endpoints::resolve(&pool, a, "not a host")
        .await
        .expect("x")
        .is_none());
}
