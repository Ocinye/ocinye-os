//! D011 · O que o executor da instalação pede ao Core: o `endpoint-seed` e as
//! quatro verificações só de leitura (ADR-0022).
//!
//! Cada prova corre numa base **só sua**: o registo da Instância é um
//! singleton, e duas provas a instalar Instâncias na base partilhada pisavam-se.

use ocinye_contracts::{Distribution, InstanceProfile};
use ocinye_core::modules::identity::{self, Authenticator, Throttle};
use ocinye_core::modules::organisation::installation::{self, SeedRefusal};
use ocinye_core::modules::organisation::{self, distributions};
use ocinye_core::password::{Hasher, HashingParams};
use ocinye_observability::CorrelationIds;
use sqlx::migrate::Migrator;
use sqlx::PgPool;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

const INST: &str = "inst-0123456789abcdef";
const CANONICO: &str = "https://os.instalacao.test";

struct Base {
    admin: PgPool,
    nome: String,
    pool: PgPool,
}

impl Base {
    async fn nova() -> Option<Self> {
        let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
            assert!(
                std::env::var("CI").is_err(),
                "OCINYE_TEST_DATABASE_URL em falta em CI: a instalação ficaria por provar"
            );
            return None;
        };
        let admin = PgPool::connect(&url).await.expect("base de testes");
        let nome = format!("ocinye_d011_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE {nome}"))
            .execute(&admin)
            .await
            .expect("criar a base descartável");
        let (base, consulta) = url
            .split_once('?')
            .map_or((url.as_str(), ""), |(b, q)| (b, q));
        let raiz = base.rsplit_once('/').map_or(base, |(raiz, _)| raiz);
        let propria = if consulta.is_empty() {
            format!("{raiz}/{nome}")
        } else {
            format!("{raiz}/{nome}?{consulta}")
        };
        let pool = PgPool::connect(&propria).await.expect("ligar");
        MIGRATOR.run(&pool).await.expect("migrations");
        Some(Self { admin, nome, pool })
    }

    async fn apagar(self) {
        self.pool.close().await;
        let _ = sqlx::query(&format!(
            "DROP DATABASE IF EXISTS {} WITH (FORCE)",
            self.nome
        ))
        .execute(&self.admin)
        .await;
    }
}

fn ids() -> CorrelationIds {
    CorrelationIds::generate()
}

fn authenticator() -> Authenticator {
    Authenticator::new(
        Hasher::new(HashingParams {
            memory_kib: 8 * 1024,
            iterations: 2,
            parallelism: 1,
        }),
        Throttle {
            per_ip: 1_000,
            per_email: 1_000,
            window_minutes: 15,
        },
        24,
    )
}

/// O que o `bootstrap-admin` faz numa base vazia, com a instalação registada:
/// Business de nascimento, Research activada com ela.
async fn instalar(pool: &PgPool, registar: bool) -> Uuid {
    let org = organisation::resolve_instance(
        pool,
        Some("empresa-exemplo"),
        Some("Empresa Exemplo"),
        Some(InstanceProfile::Business),
        &ids(),
    )
    .await
    .expect("instância");
    distributions::initial(
        pool,
        org.id,
        Distribution::Business,
        &[Distribution::Research],
        &ids(),
    )
    .await
    .expect("distribuições");
    if registar {
        installation::record_installation(pool, org.id, INST, &ids())
            .await
            .expect("registo");
    }
    org.id
}

async fn semear(
    pool: &PgPool,
    id: &str,
    host: &str,
    d: &str,
) -> Result<installation::Seeded, SeedRefusal> {
    installation::seed_bound_endpoint(pool, id, host, d, Some(CANONICO), &ids())
        .await
        .expect("endpoint-seed")
}

#[tokio::test]
async fn o_ponto_fixo_nasce_uma_vez_e_o_canonico_com_ele() {
    let Some(base) = Base::nova().await else {
        return;
    };
    let org = instalar(&base.pool, true).await;

    let primeiro = semear(&base.pool, INST, "Business.Instalacao.TEST", "business")
        .await
        .expect("criado");
    assert_eq!(primeiro.result, "created");
    assert_eq!(primeiro.host, "business.instalacao.test");
    assert_eq!(primeiro.state, "active");

    // A mesma intenção outra vez: nada muda, e nada se duplica.
    let segundo = semear(&base.pool, INST, "business.instalacao.test", "business")
        .await
        .expect("inalterado");
    assert_eq!(segundo.result, "unchanged");
    assert_eq!(segundo.endpoint_id, primeiro.endpoint_id);

    let pontos = installation::verify_endpoints(&base.pool).await.unwrap();
    assert_eq!(pontos.len(), 2, "{pontos:?}");
    assert!(pontos[0].canonical && pontos[0].host == "os.instalacao.test");
    assert_eq!(pontos[0].distribution, None, "o canónico é genérico");
    assert_eq!(pontos[1].distribution.as_deref(), Some("business"));

    // Auditado como a instalação, sem pessoa — e uma vez só.
    let (n, sujeito, pessoa): (i64, Option<String>, Option<Uuid>) = sqlx::query_as(
        "SELECT count(*), max(actor_subject), max(actor_person_id::text)::uuid FROM audit_events
          WHERE action = 'access_endpoint_seeded' AND organisation_id = $1",
    )
    .bind(org)
    .fetch_one(&base.pool)
    .await
    .unwrap();
    assert_eq!(n, 1, "a repetição auditou outra vez");
    assert_eq!(sujeito.as_deref(), Some("system:installer"));
    assert_eq!(pessoa, None);
    base.apagar().await;
}

#[tokio::test]
async fn as_sete_recusas_tem_cada_uma_o_seu_codigo() {
    let Some(base) = Base::nova().await else {
        return;
    };
    instalar(&base.pool, true).await;
    let pool = &base.pool;

    assert_eq!(
        semear(pool, INST, "x.instalacao.test", "enterprise").await,
        Err(SeedRefusal::DistributionUnknown)
    );
    for mau in [
        "",
        "a b.test",
        "x.test;rm",
        "../x.test",
        "https://x.test",
        "*.x.test",
    ] {
        assert_eq!(
            semear(pool, INST, mau, "business").await,
            Err(SeedRefusal::HostInvalid),
            "{mau:?}"
        );
    }
    assert_eq!(
        semear(
            pool,
            "inst-ffffffffffffffff",
            "x.instalacao.test",
            "business"
        )
        .await,
        Err(SeedRefusal::NotANewInstance),
        "outra instalação semeou nesta Instância"
    );
    assert_eq!(
        semear(pool, INST, "os.instalacao.test", "business").await,
        Err(SeedRefusal::HostIsCanonical)
    );
    assert_eq!(
        semear(pool, INST, "pessoal.instalacao.test", "personal").await,
        Err(SeedRefusal::DistributionNotEnabled)
    );
    semear(pool, INST, "r.instalacao.test", "research")
        .await
        .expect("criado");
    assert_eq!(
        semear(pool, INST, "r.instalacao.test", "business").await,
        Err(SeedRefusal::HostTaken),
        "o mesmo nome mudou de destino"
    );

    // Uma sessão — qualquer, mesmo revogada — fecha a porta para sempre.
    let pessoa: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
         SELECT organisation_id, 'Membro', 'membro@instalacao.test', 'active'
           FROM instance_identity RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (person_id, token_digest, state, expires_at, revoked_at, revoked_reason)
         VALUES ($1, repeat('a', 64), 'revoked', now() + interval '1 hour', now(), 'logout')",
    )
    .bind(pessoa)
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(
        semear(pool, INST, "outro.instalacao.test", "business").await,
        Err(SeedRefusal::SessionsExist)
    );
    base.apagar().await;
}

#[tokio::test]
async fn uma_instancia_adoptada_nao_e_nova() {
    let Some(base) = Base::nova().await else {
        return;
    };
    // Sem registo da instalação: é uma Instância que já existia.
    instalar(&base.pool, false).await;
    assert_eq!(
        semear(&base.pool, INST, "x.instalacao.test", "business").await,
        Err(SeedRefusal::NotANewInstance)
    );
    base.apagar().await;
}

#[tokio::test]
async fn sem_instancia_nao_ha_nada_a_semear() {
    let Some(base) = Base::nova().await else {
        return;
    };
    assert_eq!(
        semear(&base.pool, INST, "x.instalacao.test", "business").await,
        Err(SeedRefusal::NotANewInstance)
    );
    assert_eq!(
        installation::verify_instance(&base.pool).await.unwrap(),
        None
    );
    assert!(installation::verify_endpoints(&base.pool)
        .await
        .unwrap()
        .is_empty());
    base.apagar().await;
}

#[tokio::test]
async fn as_verificacoes_leem_a_base_e_nao_a_escrevem() {
    let Some(base) = Base::nova().await else {
        return;
    };
    let pool = &base.pool;

    let esquema = installation::verify_schema(pool).await.unwrap();
    let conhecidas = ocinye_core::db::embedded_migration_versions();
    assert_eq!(esquema.count as usize, conhecidas.len());
    assert_eq!(esquema.pending, 0);
    assert_eq!(
        esquema.latest,
        format!("{:04}", conhecidas.last().copied().unwrap())
    );

    let antes = installation::verify_admin_bootstrap(pool).await.unwrap();
    assert!(!antes.privileged_identity_exists && !antes.temporary_credential_pending);

    let org = instalar(pool, true).await;
    let instancia = installation::verify_instance(pool).await.unwrap().unwrap();
    assert_eq!(instancia.name, "Empresa Exemplo");
    assert_eq!(instancia.slug, "empresa-exemplo");
    assert_eq!(
        instancia.distributions,
        ["business", "research"],
        "a de nascimento vem primeiro"
    );
    assert_eq!(
        instancia.applications.registered as usize,
        ocinye_contracts::ApplicationId::ALL.len()
    );
    assert!(instancia.applications.active > 0);
    assert_eq!(instancia.applications.essential_inactive, 0);

    identity::bootstrap_privileged_identity(
        pool,
        &authenticator(),
        org,
        identity::HumanOwner {
            full_name: "Pessoa de Prova".to_owned(),
            email: "pessoa@instalacao.test".to_owned(),
        },
        "Pessoa de Prova (Admin)",
        "admin@instalacao.test",
        &ids(),
    )
    .await
    .expect("primeiro administrador");
    let depois = installation::verify_admin_bootstrap(pool).await.unwrap();
    assert!(depois.privileged_identity_exists);
    assert!(depois.temporary_credential_pending);

    // Ler não escreve: nenhuma linha nova de auditoria nem de esquema.
    let contar = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT (SELECT count(*) FROM audit_events) + (SELECT count(*) FROM _sqlx_migrations)",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    };
    let n = contar().await;
    installation::verify_schema(pool).await.unwrap();
    installation::verify_instance(pool).await.unwrap();
    installation::verify_endpoints(pool).await.unwrap();
    installation::verify_admin_bootstrap(pool).await.unwrap();
    assert_eq!(contar().await, n);
    base.apagar().await;
}

#[tokio::test]
async fn o_esquema_por_aplicar_e_contado_e_nao_aplicado() {
    let Some(base) = Base::nova().await else {
        return;
    };
    // Esquecer a última migração aplicada (só nesta base descartável).
    sqlx::query(
        "DELETE FROM _sqlx_migrations WHERE version = (SELECT max(version) FROM _sqlx_migrations)",
    )
    .execute(&base.pool)
    .await
    .unwrap();
    let esquema = installation::verify_schema(&base.pool).await.unwrap();
    assert_eq!(esquema.pending, 1);
    assert_eq!(
        installation::verify_schema(&base.pool)
            .await
            .unwrap()
            .pending,
        1,
        "verificar migrou"
    );
    base.apagar().await;
}
