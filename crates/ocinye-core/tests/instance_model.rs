//! Que Instância uma instalação serve, e como a instalação existente passa a
//! ser a primeira (ADR-0013).
//!
//! Cada prova corre numa base **só sua**, criada e destruída aqui: o registo da
//! instância é um singleton, e duas provas a registar instâncias diferentes na
//! base partilhada dos testes pisavam-se uma à outra — e pisavam as outras
//! suites.

use std::borrow::Cow;

use ocinye_contracts::InstanceProfile;
use ocinye_core::modules::organisation;
use ocinye_core::CoreError;
use ocinye_observability::CorrelationIds;
use sqlx::migrate::Migrator;
use sqlx::PgPool;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

/// A base de dados de uma prova, apagada quando a prova acaba.
struct BaseDescartavel {
    admin: PgPool,
    nome: String,
    pool: PgPool,
}

impl BaseDescartavel {
    async fn nova() -> Option<Self> {
        let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
            assert!(
                std::env::var("CI").is_err(),
                "OCINYE_TEST_DATABASE_URL em falta em CI: o modelo de instância ficaria por verificar"
            );
            return None;
        };
        let admin = PgPool::connect(&url)
            .await
            .expect("OCINYE_TEST_DATABASE_URL está definida mas a base não responde");
        let nome = format!("ocinye_instancia_{}", Uuid::new_v4().simple());
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
        let pool = PgPool::connect(&propria)
            .await
            .expect("ligar à base descartável");
        Some(Self { admin, nome, pool })
    }

    async fn migrar_tudo(&self) {
        MIGRATOR.run(&self.pool).await.expect("migrations");
    }

    async fn migrar_ate(&self, versao_exclusiva: i64) {
        let mut parcial = Migrator {
            migrations: Cow::Owned(
                MIGRATOR
                    .migrations
                    .iter()
                    .filter(|m| m.version < versao_exclusiva)
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        parcial.set_ignore_missing(false);
        parcial.run(&self.pool).await.expect("migrations parciais");
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

async fn registada(pool: &PgPool) -> Option<Uuid> {
    sqlx::query_scalar("SELECT organisation_id FROM instance_identity")
        .fetch_optional(pool)
        .await
        .expect("instance_identity")
}

async fn organizacao(pool: &PgPool, slug: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

// ── Instalação nova ─────────────────────────────────────────────────────

#[tokio::test]
async fn uma_base_vazia_sem_nome_de_instancia_recusa_adivinhar() {
    let Some(base) = BaseDescartavel::nova().await else {
        return;
    };
    base.migrar_tudo().await;

    let resultado = organisation::resolve_instance(&base.pool, None, None, None, &ids()).await;
    assert!(
        matches!(resultado, Err(CoreError::Configuration(_))),
        "sem instância e sem configuração, o Core tem de recusar; veio {resultado:?}"
    );
    assert_eq!(registada(&base.pool).await, None);
    base.apagar().await;
}

#[tokio::test]
async fn uma_instalacao_nova_cria_a_sua_instancia_com_o_nome_dado() {
    let Some(base) = BaseDescartavel::nova().await else {
        return;
    };
    base.migrar_tudo().await;

    let criada = organisation::resolve_instance(
        &base.pool,
        Some("universidade-exemplo"),
        Some("Universidade Exemplo"),
        Some(InstanceProfile::Education),
        &ids(),
    )
    .await
    .expect("criar a instância");
    assert_eq!(criada.slug, "universidade-exemplo");
    assert_eq!(criada.name, "Universidade Exemplo");
    assert_eq!(registada(&base.pool).await, Some(criada.id));

    // O arranque seguinte, já sem configuração, encontra a mesma.
    let depois = organisation::resolve_instance(&base.pool, None, None, None, &ids())
        .await
        .expect("resolver a registada");
    assert_eq!(depois.id, criada.id);
    base.apagar().await;
}

#[tokio::test]
async fn so_com_o_slug_o_nome_e_legivel_e_nao_o_slug() {
    let Some(base) = BaseDescartavel::nova().await else {
        return;
    };
    base.migrar_tudo().await;

    let criada = organisation::resolve_instance(
        &base.pool,
        Some("mondrive-lda"),
        None,
        Some(InstanceProfile::Business),
        &ids(),
    )
    .await
    .expect("criar a instância");
    assert_eq!(criada.name, "Mondrive Lda");
    base.apagar().await;
}

#[tokio::test]
async fn a_configuracao_nao_muda_a_instancia_de_uma_instalacao() {
    let Some(base) = BaseDescartavel::nova().await else {
        return;
    };
    base.migrar_tudo().await;
    let primeira = organisation::resolve_instance(
        &base.pool,
        Some("primeira"),
        None,
        Some(InstanceProfile::Personal),
        &ids(),
    )
    .await
    .expect("primeira");

    let resultado = organisation::resolve_instance(
        &base.pool,
        Some("outra"),
        None,
        Some(InstanceProfile::Personal),
        &ids(),
    )
    .await;
    assert!(
        matches!(resultado, Err(CoreError::Configuration(_))),
        "outro slug numa instalação registada tem de ser recusado; veio {resultado:?}"
    );
    assert_eq!(registada(&base.pool).await, Some(primeira.id));
    let outras: i64 = sqlx::query_scalar("SELECT count(*) FROM organisations WHERE slug = 'outra'")
        .fetch_one(&base.pool)
        .await
        .expect("contagem");
    assert_eq!(outras, 0, "a recusa não pode deixar uma organização criada");
    base.apagar().await;
}

#[tokio::test]
async fn varias_organizacoes_sem_registo_nem_configuracao_recusam() {
    let Some(base) = BaseDescartavel::nova().await else {
        return;
    };
    base.migrar_tudo().await;
    organizacao(&base.pool, "uma").await;
    organizacao(&base.pool, "duas").await;

    let resultado = organisation::resolve_instance(&base.pool, None, None, None, &ids()).await;
    assert!(matches!(resultado, Err(CoreError::Configuration(_))));
    assert_eq!(registada(&base.pool).await, None);
    base.apagar().await;
}

// ── A instalação existente ──────────────────────────────────────────────

/// A instalação de hoje, migrada até 0051, com dados e com o defeito do
/// bootstrap (nome = slug), passa a ser a primeira instância sem perder nada.
#[tokio::test]
async fn a_instalacao_existente_passa_a_primeira_instancia_sem_perdas() {
    let Some(base) = BaseDescartavel::nova().await else {
        return;
    };
    base.migrar_ate(52).await;
    let pool = &base.pool;

    // Como o bootstrap a deixava: o nome é o slug.
    let org = organizacao(pool, "ocinye").await;
    let pessoa: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
         VALUES ($1, 'Pessoa', 'pessoa@exemplo.org', 'active') RETURNING id",
    )
    .bind(org)
    .fetch_one(pool)
    .await
    .expect("pessoa");
    let unidade: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, 'UAI-001', 'Unidade') RETURNING id",
    )
    .bind(org)
    .fetch_one(pool)
    .await
    .expect("unidade");
    sqlx::query(
        "INSERT INTO unit_memberships (unit_id, person_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(unidade)
    .bind(pessoa)
    .execute(pool)
    .await
    .expect("pertença");
    sqlx::query("INSERT INTO member_app_pins (person_id, pinned_app_ids) VALUES ($1, ARRAY['notes','files'])")
        .bind(pessoa)
        .execute(pool)
        .await
        .expect("fixações");

    let tabelas = [
        "organisations",
        "people",
        "units",
        "unit_memberships",
        "member_app_pins",
    ];
    let mut antes = Vec::new();
    for tabela in tabelas {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {tabela}"))
            .fetch_one(pool)
            .await
            .expect("contagem antes");
        antes.push(n);
    }

    base.migrar_tudo().await;

    for (tabela, n_antes) in tabelas.iter().zip(antes) {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {tabela}"))
            .fetch_one(pool)
            .await
            .expect("contagem depois");
        assert_eq!(n, n_antes, "{tabela} mudou de tamanho na migração");
    }
    assert_eq!(
        registada(pool).await,
        Some(org),
        "a única organização é a instância"
    );
    let (slug, nome): (String, String) =
        sqlx::query_as("SELECT slug, name FROM organisations WHERE id = $1")
            .bind(org)
            .fetch_one(pool)
            .await
            .expect("organização");
    assert_eq!(slug, "ocinye", "o slug é identidade técnica e não muda");
    assert_eq!(nome, "Ocinye", "o nome deixa de ser o slug");
    let dono: Uuid = sqlx::query_scalar("SELECT organisation_id FROM people WHERE id = $1")
        .bind(pessoa)
        .fetch_one(pool)
        .await
        .expect("dono");
    assert_eq!(dono, org, "nenhum membro fica órfão");

    // E o Core, a arrancar sem configuração, serve essa instância.
    let resolvida = organisation::resolve_instance(pool, None, None, None, &ids())
        .await
        .expect("resolver");
    assert_eq!(resolvida.id, org);
    base.apagar().await;
}

/// Um nome que alguém escolheu não é tocado pela correcção do bootstrap.
#[tokio::test]
async fn um_nome_escolhido_nao_e_reescrito() {
    let Some(base) = BaseDescartavel::nova().await else {
        return;
    };
    base.migrar_ate(52).await;
    sqlx::query("INSERT INTO organisations (slug, name) VALUES ('lab', 'Laboratório Central')")
        .execute(&base.pool)
        .await
        .expect("organização");
    base.migrar_tudo().await;
    let nome: String = sqlx::query_scalar("SELECT name FROM organisations WHERE slug = 'lab'")
        .fetch_one(&base.pool)
        .await
        .expect("nome");
    assert_eq!(nome, "Laboratório Central");
    base.apagar().await;
}
