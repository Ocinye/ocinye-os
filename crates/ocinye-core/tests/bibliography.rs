//! A revisão de bibliografia, atravessando o Capability Runtime a sério.
//!
//! # O que estes testes provam
//!
//! Que existe um caminho institucional real — Core Operation → Capability
//! Runtime → componente WebAssembly → Core — e que a autoridade não se desloca
//! por causa dele. O componente lê texto; quem decide quem pode pedir, o que
//! entra, o que sai e o que isso significa continua a ser o Core.
//!
//! Nada aqui é simulado. O motor é o `wasmtime`, o componente é o `.wasm`
//! construído a partir de `wasm/capabilities/bibtex-import`, e a autorização é
//! decidida contra PostgreSQL.
//!
//! Salta quando `OCINYE_TEST_DATABASE_URL` não está definida; **falha** quando
//! está e a base não responde, e **falha** quando o componente não está
//! construído — um caminho operacional que não se pode exercitar não é um
//! caminho provado.

use std::path::PathBuf;

use ocinye_contracts::bibliography::MAX_BIBTEX_BYTES;
use ocinye_core::capabilities::{Capabilities, Component};
use ocinye_core::error::CoreError;
use ocinye_core::modules::knowledge;
use ocinye_domain::Principal;
use sqlx::PgPool;
use uuid::Uuid;

/// Uma bibliografia com tudo o que interessa: maiúsculas, desalinhamento, e
/// uma entrada que não fecha.
const BIBTEX: &str = r#"
@ARTICLE{mucai2024,
  TITLE = {Wind resource assessment for the Mucai corridor},
  author={Ana Mucai and Bruno Katchi},
     year = {2024},
  journal={Renewable Energy},
  doi = {10.1016/j.renene.2024.01.001}
}

@book{katchi2023, title = {Hidrologia do Cunene}, author = {Bruno Katchi}, year = {2023}}

@misc{isto_nao_fecha
"#;

macro_rules! base {
    () => {{
        let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
            eprintln!("skipping: OCINYE_TEST_DATABASE_URL is not set");
            return;
        };
        let pool = PgPool::connect(&url)
            .await
            .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations must apply");
        pool
    }};
}

/// O directório onde o componente foi construído.
fn componentes() -> String {
    std::env::var("OCINYE_TEST_CAPABILITY_WASM").map_or_else(
        |_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/wasm32-wasip1/release")
                .to_string_lossy()
                .into_owned()
        },
        |caminho| {
            PathBuf::from(caminho)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        },
    )
}

/// O Runtime com o componente carregado.
///
/// Falha se não estiver construído. Saltar aqui seria a suite a dizer que o
/// caminho operacional funciona sem o ter atravessado.
fn capacidades() -> Capabilities {
    let dir = componentes();
    let capacidades = Capabilities::load(&dir).expect("o motor constrói-se");
    assert!(
        capacidades.has(Component::BibtexImport),
        "o componente WebAssembly não está construído em «{dir}».\n\
         Corra: ./scripts/build-capabilities.sh"
    );
    capacidades
}

/// Uma instituição, uma unidade, um ambiente e duas pessoas.
struct Mundo {
    workspace: Uuid,
    dentro: Principal,
    fora: Principal,
}

async fn mundo(pool: &PgPool) -> Mundo {
    let tag = Uuid::new_v4().simple().to_string();
    let organisation_id: Uuid =
        sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
            .bind(format!("b{tag}"))
            .fetch_one(pool)
            .await
            .expect("organização");

    let unidade = |sufixo: &str| {
        let codigo = format!("B{}{}", sufixo, &tag[..6]).to_uppercase();
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, 'Unidade')
                 RETURNING id",
            )
            .bind(organisation_id)
            .bind(codigo)
            .fetch_one(&pool)
            .await
            .expect("unidade")
        }
    };
    let unidade_a = unidade("A").await;
    let unidade_b = unidade("B").await;

    let workspace: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces
             (organisation_id, unit_id, code, title, kind, classification)
         VALUES ($1, $2, $3, 'Ambiente', 'idea', 'INTERNAL') RETURNING id",
    )
    .bind(organisation_id)
    .bind(unidade_a)
    .bind(format!("WS-B-{}", &tag[..6]).to_uppercase())
    .fetch_one(pool)
    .await
    .expect("workspace");

    let dentro = pessoa(pool, organisation_id, unidade_a).await;
    let fora = pessoa(pool, organisation_id, unidade_b).await;

    // Pertença ao ambiente, e não só à unidade.
    //
    // Criar num ambiente de investigação depende de se pertencer a ele: a
    // unidade dá para **ler** o que é interno, e é por isso que quem está de
    // fora recebe uma recusa e não uma ausência. Rever bibliografia autoriza
    // contra a criação, e portanto exige o mesmo.
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role)
             VALUES ($1, $2, 'member')",
    )
    .bind(workspace)
    .bind(dentro.person_id)
    .execute(pool)
    .await
    .expect("pertença ao ambiente");

    Mundo {
        workspace,
        dentro: recarregar(pool, dentro.person_id).await,
        fora,
    }
}

/// Volta a estabelecer a autoridade, depois de a pertença mudar.
async fn recarregar(pool: &PgPool, person_id: Uuid) -> Principal {
    let registo = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &registo)
        .await
        .expect("principal")
}

async fn pessoa(pool: &PgPool, organisation_id: Uuid, unit_id: Uuid) -> Principal {
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
    .expect("pertença");

    let registo = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");

    ocinye_core::modules::identity::principal_for_person(pool, &registo)
        .await
        .expect("principal")
}

// ── O caminho ───────────────────────────────────────────────────────────

/// Uma bibliografia atravessa o Runtime e volta lida e arrumada.
///
/// O controlo positivo de tudo o que está abaixo: sem ele, cada recusa podia
/// estar a acontecer por uma razão que nada tem que ver com o que se afirma.
#[tokio::test]
async fn uma_bibliografia_atravessa_o_runtime_e_volta_lida() {
    let pool = base!();
    let mundo = mundo(&pool).await;

    let revisao = knowledge::review_bibliography(
        &pool,
        &capacidades(),
        &mundo.dentro,
        mundo.workspace,
        BIBTEX,
    )
    .await
    .expect("a revisão atravessa");

    assert_eq!(revisao.read_count(), 2, "duas entradas eram legíveis");
    assert!(
        !revisao.is_complete(),
        "uma entrada não fecha e tem de o dizer"
    );
    assert_eq!(revisao.unreadable.len(), 1);

    let primeira = &revisao.entries[0];
    assert_eq!(primeira.citation_key, "mucai2024");
    assert_eq!(primeira.entry_type, "article", "o tipo vem em minúsculas");
    assert_eq!(primeira.year, Some(2024));
    assert_eq!(primeira.authors, ["Ana Mucai", "Bruno Katchi"]);

    // A forma canónica: campos em minúsculas, alinhados, ordenados.
    assert!(revisao.normalized.contains("@article{mucai2024,"));
    assert!(revisao.normalized.contains("author"));
    assert!(
        !revisao.normalized.contains("TITLE"),
        "a normalização devia ter baixado o caso: {}",
        revisao.normalized
    );
    assert!(
        !revisao.normalized.contains("isto_nao_fecha"),
        "uma entrada por ler apareceu normalizada"
    );
}

/// A mesma bibliografia dá sempre a mesma revisão.
#[tokio::test]
async fn a_revisao_e_determinista() {
    let pool = base!();
    let mundo = mundo(&pool).await;
    let capacidades = capacidades();

    let primeira =
        knowledge::review_bibliography(&pool, &capacidades, &mundo.dentro, mundo.workspace, BIBTEX)
            .await
            .expect("primeira");

    for _ in 0..3 {
        let outra = knowledge::review_bibliography(
            &pool,
            &capacidades,
            &mundo.dentro,
            mundo.workspace,
            BIBTEX,
        )
        .await
        .expect("repetição");
        assert_eq!(outra, primeira, "a mesma entrada deu saídas diferentes");
    }
}

// ── A autoridade não se desloca ─────────────────────────────────────────

/// Quem não alcança o ambiente não revê bibliografia nele.
///
/// E a recusa não distingue «não existe» de «não podes»: conhecer o
/// identificador do workspace não é um direito.
#[tokio::test]
async fn quem_nao_alcanca_o_ambiente_nao_revê_bibliografia_nele() {
    let pool = base!();
    let mundo = mundo(&pool).await;

    let erro =
        knowledge::review_bibliography(&pool, &capacidades(), &mundo.fora, mundo.workspace, BIBTEX)
            .await
            .expect_err("uma pessoa de outra unidade não devia passar");

    assert!(
        matches!(
            erro,
            CoreError::NotFound(_) | CoreError::PermissionDenied(_)
        ),
        "esperava-se recusa, veio {erro:?}"
    );
}

/// O identificador do ambiente não é uma autorização.
#[tokio::test]
async fn um_identificador_inventado_nao_abre_nada() {
    let pool = base!();
    let mundo = mundo(&pool).await;

    let erro = knowledge::review_bibliography(
        &pool,
        &capacidades(),
        &mundo.dentro,
        Uuid::new_v4(),
        BIBTEX,
    )
    .await
    .expect_err("um identificador que não existe não devia abrir nada");

    assert!(matches!(erro, CoreError::NotFound(_)), "veio {erro:?}");
}

// ── Os limites ──────────────────────────────────────────────────────────

/// Uma bibliografia acima do limite é recusada antes de chegar ao Runtime.
#[tokio::test]
async fn uma_bibliografia_grande_de_mais_e_recusada() {
    let pool = base!();
    let mundo = mundo(&pool).await;

    let enorme = "@misc{k, title = {t}}\n".repeat(MAX_BIBTEX_BYTES / 10);
    assert!(enorme.len() > MAX_BIBTEX_BYTES);

    let erro = knowledge::review_bibliography(
        &pool,
        &capacidades(),
        &mundo.dentro,
        mundo.workspace,
        &enorme,
    )
    .await
    .expect_err("acima do limite não passa");

    // Recusado **pelo Core**, e não por o componente ficar sem combustível.
    //
    // As duas recusas chegam como `Validation`, e é por isso que esta asserção
    // olha para a mensagem: sem isto, remover o limite do Core deixava o teste
    // a passar — o componente esgotava-se e a tradução dava o mesmo tipo de
    // erro. Foi o que aconteceu na primeira reversão que corri, e o teste dizia
    // que estava tudo bem.
    let CoreError::Validation(mensagem) = &erro else {
        panic!("esperava-se uma recusa de validação, veio {erro:?}");
    };
    assert!(
        mensagem.contains(&MAX_BIBTEX_BYTES.to_string()),
        "a recusa devia nomear o limite do Core, e veio «{mensagem}»"
    );
}

/// Uma bibliografia vazia é uma revisão vazia, e não um erro.
#[tokio::test]
async fn uma_bibliografia_vazia_nao_e_um_erro() {
    let pool = base!();
    let mundo = mundo(&pool).await;

    let revisao =
        knowledge::review_bibliography(&pool, &capacidades(), &mundo.dentro, mundo.workspace, "")
            .await
            .expect("vazio é vazio");

    assert_eq!(revisao.read_count(), 0);
    assert!(revisao.is_complete(), "nada por ler é nada por ler");
}

// ── O que a operação não faz ────────────────────────────────────────────

/// Rever bibliografia não guarda nada.
///
/// # Porque isto é um teste e não uma afirmação
///
/// Porque «não persiste» é uma propriedade que se perde sem que ninguém repare:
/// basta alguém achar conveniente guardar o que foi colado. Conta-se o que
/// existe antes e depois, nas três tabelas onde isto poderia cair.
#[tokio::test]
async fn rever_bibliografia_nao_guarda_nada() {
    let pool = base!();
    let mundo = mundo(&pool).await;

    let contar = |tabela: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {tabela}"))
                .fetch_one(&pool)
                .await
                .expect("contagem")
        }
    };

    let antes = (
        contar("sources").await,
        contar("audit_events").await,
        contar("outbox_events").await,
    );

    knowledge::review_bibliography(
        &pool,
        &capacidades(),
        &mundo.dentro,
        mundo.workspace,
        BIBTEX,
    )
    .await
    .expect("a revisão atravessa");

    let depois = (
        contar("sources").await,
        contar("audit_events").await,
        contar("outbox_events").await,
    );

    assert_eq!(
        antes, depois,
        "rever bibliografia escreveu no estado institucional (sources, audit, outbox)"
    );
}

// ── Quando o Runtime não está lá ────────────────────────────────────────

/// Sem componente, a operação recusa — e não devolve uma revisão vazia.
///
/// A diferença importa: uma revisão vazia diz «a sua bibliografia não tem
/// nada», e o que aconteceu foi que o sistema não conseguiu ler.
#[tokio::test]
async fn sem_componente_a_operacao_recusa_em_vez_de_fingir() {
    let pool = base!();
    let mundo = mundo(&pool).await;
    let vazio = Capabilities::load("/um/directorio/sem/componentes").expect("o motor constrói-se");

    let erro =
        knowledge::review_bibliography(&pool, &vazio, &mundo.dentro, mundo.workspace, BIBTEX)
            .await
            .expect_err("sem componente não há revisão");

    assert!(
        matches!(erro, CoreError::CapabilityUnavailable(_)),
        "veio {erro:?}"
    );
}
