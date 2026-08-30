//! Carregamento em partes: meio gigabyte através de um edge que recusa cem.
//!
//! # A propriedade
//!
//! > **Um ficheiro institucional pode ser carregado em pedaços, através de uma
//! > sessão autorizada antes do primeiro byte, sem que exista `FileVersion`
//! > enquanto o conjunto não estiver completo e verificado.**
//!
//! # O que estas provas medem
//!
//! Não medem que o código corre. Medem que **recusa** — e cada recusa foi
//! confirmada por reversão: desligada a guarda, o teste passa a falhar.

use ocinye_contracts::Classification;
use ocinye_core::modules::files::upload::{self, NewUpload};
use ocinye_core::storage::sha256_hex;
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: o carregamento em partes \
             ficaria por verificar e a suite reportaria verde"
        );
        eprintln!("SALTADO: OCINYE_TEST_DATABASE_URL não está definida.");
        return None;
    };
    let pool = PgPool::connect(&url).await.expect("base de dados");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

fn store() -> Option<ocinye_core::storage::ObjectStore> {
    let endpoint = match std::env::var("OCINYE_TEST_STORAGE_ENDPOINT") {
        Ok(valor) => valor,
        Err(_) => {
            assert!(
                std::env::var("CI").is_err(),
                "não há armazenamento, e isto é a CI: o carregamento em partes \
                 não pode contar como provado sem um object store"
            );
            eprintln!("SALTADO: OCINYE_TEST_STORAGE_ENDPOINT não está definida.");
            return None;
        }
    };
    ocinye_core::storage::ObjectStore::new(ocinye_core::config::StorageConfig {
        endpoint_url: endpoint,
        region: std::env::var("OCINYE_TEST_STORAGE_REGION")
            .unwrap_or_else(|_| "us-east-1".to_owned()),
        access_key: std::env::var("OCINYE_TEST_STORAGE_ACCESS_KEY").ok()?,
        secret_key: std::env::var("OCINYE_TEST_STORAGE_SECRET_KEY").ok()?,
        bucket: std::env::var("OCINYE_TEST_STORAGE_BUCKET")
            .unwrap_or_else(|_| "ocinye-test-artifacts".to_owned()),
        backend_code: "test".to_owned(),
        location_label: "test".to_owned(),
        residency: ocinye_contracts::storage::Residency::Undeclared,
        max_upload_bytes: 600 * 1024 * 1024,
    })
}

struct Cenario {
    pool: PgPool,
    store: ocinye_core::storage::ObjectStore,
    workspace_id: Uuid,
    quem_carrega: ocinye_domain::Principal,
    ids: CorrelationIds,
}

/// Uma organização, uma unidade, um ambiente, um backend e quem lá pertence.
async fn cenario() -> Option<Cenario> {
    let pool = pool().await?;
    let store = store()?;
    let sufixo = Uuid::new_v4().simple().to_string();

    let organisation_id: Uuid =
        sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
            .bind(format!("u{sufixo}"))
            .fetch_one(&pool)
            .await
            .expect("organização");

    let unit_id: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, $2) RETURNING id",
    )
    .bind(organisation_id)
    .bind(format!("U{}", &sufixo[..6]).to_uppercase())
    .fetch_one(&pool)
    .await
    .expect("unidade");

    let workspace_id: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces
             (organisation_id, unit_id, code, title, kind, classification)
         VALUES ($1, $2, $3, 'Ambiente', 'idea', 'INTERNAL') RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(format!("W{}", &sufixo[..6]).to_uppercase())
    .fetch_one(&pool)
    .await
    .expect("ambiente");

    backend_por_omissao(&pool).await;

    let quem_carrega = pessoa_do_ambiente(&pool, organisation_id, workspace_id, "lead").await;

    Some(Cenario {
        pool,
        store,
        workspace_id,
        quem_carrega,
        ids: CorrelationIds::generate(),
    })
}

/// A mesma tranca que as outras suites, e a mesma linha.
///
/// `storage_backends` é estado **global**: uma linha nova por teste acumularia
/// armazenamentos, e duas suites a inventar linhas diferentes sobre o mesmo
/// estado anulam-se conforme a ordem em que correm.
const TRANCA_DO_REGISTO: i64 = 0x0000_C109_E570_9A6E;

async fn backend_por_omissao(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO storage_backends
             (code, kind, display_name, location_label, bucket, is_default, is_active)
         VALUES ('ocinye-test-default', 's3_compatible', 'Test', 'test', 'prova', TRUE, TRUE)
         ON CONFLICT (code) DO UPDATE
             SET is_default = TRUE, is_active = TRUE, updated_at = now()",
    )
    .execute(pool)
    .await
    .expect("registar armazenamento de teste");
}

/// Corre o corpo com o registo de armazenamento trancado.
async fn com_registo_exclusivo<T, F>(pool: &PgPool, corpo: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let mut tranca = pool.acquire().await.expect("ligação");
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(TRANCA_DO_REGISTO)
        .execute(&mut *tranca)
        .await
        .expect("tranca");
    backend_por_omissao(pool).await;
    let resultado = corpo.await;
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(TRANCA_DO_REGISTO)
        .execute(&mut *tranca)
        .await
        .expect("destranca");
    resultado
}

async fn pessoa_do_ambiente(
    pool: &PgPool,
    organisation_id: Uuid,
    workspace_id: Uuid,
    papel: &str,
) -> ocinye_domain::Principal {
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
        "INSERT INTO workspace_memberships (workspace_id, person_id, role) VALUES ($1, $2, $3)",
    )
    .bind(workspace_id)
    .bind(person_id)
    .bind(papel)
    .execute(pool)
    .await
    .expect("pertença");

    let pessoa = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("leitura")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &pessoa)
        .await
        .expect("principal")
}

/// Bytes reprodutíveis e não constantes: um ficheiro de zeros teria a mesma
/// soma em qualquer troço, e uma parte trocada por outra passaria despercebida.
fn bytes(tamanho: usize, semente: u8) -> Vec<u8> {
    (0..tamanho)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(semente))
        .collect()
}

/// O percurso inteiro: abrir, mandar as partes, fechar, e ter o ficheiro.
///
/// Um ficheiro lógico maior do que um pedaço — que é o caso que a segmentação
/// existe para servir. Com um ficheiro de uma parte só, tudo isto passaria sem
/// nunca exercitar a montagem.
#[tokio::test]
async fn um_ficheiro_maior_do_que_um_pedaco_atravessa_em_partes() {
    let Some(c) = cenario().await else { return };

    // Três partes: duas cheias e uma curta. A última é o caso especial, e um
    // teste com um múltiplo exacto do pedaço nunca lhe tocava.
    let pedaco = upload::CHUNK_SIZE_BYTES as usize;
    let tamanho = pedaco * 2 + 1024;
    let ficheiro = bytes(tamanho, 7);
    let soma_final = sha256_hex(&ficheiro);

    let mut tx = c.pool.begin().await.expect("tx");
    let sessao = upload::begin(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        "teste",
        c.workspace_id,
        NewUpload {
            filename: "modelo-cfd.zip".to_owned(),
            content_type: "application/zip".to_owned(),
            size_bytes: tamanho as i64,
            classification: Some(Classification::Internal),
            folder_id: None,
            file_id: None,
        },
    )
    .await
    .expect("abrir");
    tx.commit().await.expect("commit");

    assert_eq!(sessao.total_parts, 3, "o ficheiro devia partir-se em três");
    assert!(
        sessao.received_parts.is_empty(),
        "uma sessão nova não tem partes"
    );

    // Enquanto a sessão está aberta, não há ficheiro nenhum.
    let ficheiros: i64 = sqlx::query_scalar("SELECT count(*) FROM files WHERE workspace_id = $1")
        .bind(c.workspace_id)
        .fetch_one(&c.pool)
        .await
        .expect("contagem");
    assert_eq!(
        ficheiros, 0,
        "existia um File antes de o carregamento estar completo"
    );

    for parte in 1..=sessao.total_parts {
        let inicio = (parte as usize - 1) * pedaco;
        let fim = (inicio + pedaco).min(tamanho);
        let troco = &ficheiro[inicio..fim];
        let mut tx = c.pool.begin().await.expect("tx");
        upload::accept_part(
            &mut tx,
            &c.quem_carrega,
            &c.store,
            sessao.id,
            parte,
            &sha256_hex(troco),
            troco.to_vec(),
        )
        .await
        .unwrap_or_else(|e| panic!("parte {parte}: {e}"));
        tx.commit().await.expect("commit");
    }

    let mut tx = c.pool.begin().await.expect("tx");
    let versao = upload::finalise(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        sessao.id,
        &soma_final,
    )
    .await
    .expect("fechar");
    tx.commit().await.expect("commit");

    // O que ficou guardado é o que saiu — em tamanho e em soma.
    let (tam, soma): (i64, String) = sqlx::query_as(
        "SELECT so.size_bytes, so.checksum_sha256
           FROM file_versions fv JOIN storage_objects so ON so.id = fv.storage_object_id
          WHERE fv.id = $1",
    )
    .bind(versao.version_id)
    .fetch_one(&c.pool)
    .await
    .expect("objecto");
    assert_eq!(
        tam, tamanho as i64,
        "o tamanho guardado não é o do ficheiro"
    );
    assert_eq!(soma, soma_final, "a soma guardada não é a do ficheiro");
}

/// Um conjunto incompleto não produz versão.
///
/// # A soma declarada é a do que foi enviado, e não a do ficheiro
///
/// De propósito. Com a soma do ficheiro inteiro, a recusa vinha da verificação
/// final — e este teste passava mesmo com as guardas de completude removidas,
/// como uma reversão mostrou. Declarando a soma do que realmente subiu, só as
/// guardas de completude podem recusar: sem elas, a instituição ficaria com uma
/// versão de um ficheiro truncado que confere com a sua própria soma.
#[tokio::test]
async fn um_carregamento_incompleto_nao_produz_versao() {
    let Some(c) = cenario().await else { return };
    let pedaco = upload::CHUNK_SIZE_BYTES as usize;
    let tamanho = pedaco + 512;
    let ficheiro = bytes(tamanho, 11);

    let mut tx = c.pool.begin().await.expect("tx");
    let sessao = upload::begin(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        "teste",
        c.workspace_id,
        NewUpload {
            filename: "incompleto.zip".to_owned(),
            content_type: "application/zip".to_owned(),
            size_bytes: tamanho as i64,
            classification: None,
            folder_id: None,
            file_id: None,
        },
    )
    .await
    .expect("abrir");
    tx.commit().await.expect("commit");

    // Só a primeira das duas.
    let mut tx = c.pool.begin().await.expect("tx");
    upload::accept_part(
        &mut tx,
        &c.quem_carrega,
        &c.store,
        sessao.id,
        1,
        &sha256_hex(&ficheiro[..pedaco]),
        ficheiro[..pedaco].to_vec(),
    )
    .await
    .expect("parte 1");
    tx.commit().await.expect("commit");

    let mut tx = c.pool.begin().await.expect("tx");
    let erro = upload::finalise(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        sessao.id,
        &sha256_hex(&ficheiro[..pedaco]),
    )
    .await;
    assert!(erro.is_err(), "um conjunto incompleto produziu uma versão");

    let ficheiros: i64 = sqlx::query_scalar("SELECT count(*) FROM files WHERE workspace_id = $1")
        .bind(c.workspace_id)
        .fetch_one(&c.pool)
        .await
        .expect("contagem");
    assert_eq!(ficheiros, 0, "ficou um File de um carregamento incompleto");
}

/// A soma final errada é recusada, e nada fica.
#[tokio::test]
async fn uma_soma_final_errada_e_recusada() {
    let Some(c) = cenario().await else { return };
    let tamanho = 4096;
    let ficheiro = bytes(tamanho, 13);

    let mut tx = c.pool.begin().await.expect("tx");
    let sessao = upload::begin(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        "teste",
        c.workspace_id,
        NewUpload {
            filename: "soma-errada.zip".to_owned(),
            content_type: "application/zip".to_owned(),
            size_bytes: tamanho as i64,
            classification: None,
            folder_id: None,
            file_id: None,
        },
    )
    .await
    .expect("abrir");
    tx.commit().await.expect("commit");

    let mut tx = c.pool.begin().await.expect("tx");
    upload::accept_part(
        &mut tx,
        &c.quem_carrega,
        &c.store,
        sessao.id,
        1,
        &sha256_hex(&ficheiro),
        ficheiro.clone(),
    )
    .await
    .expect("parte");
    tx.commit().await.expect("commit");

    // A soma de outro conteúdo qualquer.
    let mut tx = c.pool.begin().await.expect("tx");
    let erro = upload::finalise(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        sessao.id,
        &sha256_hex(b"outra coisa completamente diferente"),
    )
    .await;
    tx.commit().await.expect("commit");

    assert!(erro.is_err(), "uma soma final errada produziu uma versão");

    let ficheiros: i64 = sqlx::query_scalar("SELECT count(*) FROM files WHERE workspace_id = $1")
        .bind(c.workspace_id)
        .fetch_one(&c.pool)
        .await
        .expect("contagem");
    assert_eq!(
        ficheiros, 0,
        "uma soma errada deixou um File na instituição"
    );
}

/// Uma sessão pertence a quem a abriu.
///
/// A recusa é a mesma que a de uma sessão inexistente. Distingui-las diria a
/// quem tenta que a sessão existe — que é informação que não lhe pertence.
#[tokio::test]
async fn uma_sessao_de_outro_actor_e_recusada() {
    let Some(c) = cenario().await else { return };
    let ficheiro = bytes(2048, 17);

    let mut tx = c.pool.begin().await.expect("tx");
    let sessao = upload::begin(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        "teste",
        c.workspace_id,
        NewUpload {
            filename: "alheia.zip".to_owned(),
            content_type: "application/zip".to_owned(),
            size_bytes: ficheiro.len() as i64,
            classification: None,
            folder_id: None,
            file_id: None,
        },
    )
    .await
    .expect("abrir");
    tx.commit().await.expect("commit");

    // Outra pessoa, do mesmo ambiente e com o mesmo papel. Se fosse de fora, a
    // recusa podia ser da pertença em vez de ser da sessão.
    let organisation_id = c.quem_carrega.organisation_id;
    let outra = pessoa_do_ambiente(&c.pool, organisation_id, c.workspace_id, "lead").await;

    let mut tx = c.pool.begin().await.expect("tx");
    let erro = upload::accept_part(
        &mut tx,
        &outra,
        &c.store,
        sessao.id,
        1,
        &sha256_hex(&ficheiro),
        ficheiro.clone(),
    )
    .await;
    assert!(
        erro.is_err(),
        "outra pessoa escreveu numa sessão de carregamento que não abriu"
    );

    let alheia = match erro {
        Err(e) => format!("{e:?}"),
        Ok(_) => unreachable!("já foi afirmado que é erro"),
    };

    let inventada = Uuid::new_v4();
    let inexistente = upload::state_of(&mut tx, &c.quem_carrega, inventada)
        .await
        .map(|_| ())
        .expect_err("uma sessão inventada não existe");
    let inexistente = format!("{inexistente:?}");

    assert_eq!(
        alheia.split('(').next(),
        inexistente.split('(').next(),
        "a sessão alheia e a inexistente devem dar a mesma classe de recusa; \
         distingui-las diria a quem tenta que a sessão existe"
    );
}

/// Repetir uma parte é seguro, e não escreve duas vezes.
#[tokio::test]
async fn repetir_uma_parte_e_idempotente() {
    let Some(c) = cenario().await else { return };
    let ficheiro = bytes(4096, 19);

    let mut tx = c.pool.begin().await.expect("tx");
    let sessao = upload::begin(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        "teste",
        c.workspace_id,
        NewUpload {
            filename: "repetida.zip".to_owned(),
            content_type: "application/zip".to_owned(),
            size_bytes: ficheiro.len() as i64,
            classification: None,
            folder_id: None,
            file_id: None,
        },
    )
    .await
    .expect("abrir");
    tx.commit().await.expect("commit");

    let soma = sha256_hex(&ficheiro);
    for esperado_ja_la in [false, true] {
        let mut tx = c.pool.begin().await.expect("tx");
        let aceite = upload::accept_part(
            &mut tx,
            &c.quem_carrega,
            &c.store,
            sessao.id,
            1,
            &soma,
            ficheiro.clone(),
        )
        .await
        .expect("parte");
        tx.commit().await.expect("commit");
        assert_eq!(
            aceite.already_present, esperado_ja_la,
            "a segunda entrega da mesma parte devia ser reconhecida como já presente"
        );
        assert_eq!(
            aceite.received_parts, 1,
            "uma parte repetida contou duas vezes"
        );
    }
}

/// Uma parte com o tamanho errado é recusada.
///
/// Aceitar uma parte maior do que o pedaço acordado deixaria contornar o
/// tamanho que foi autorizado na abertura — e voltar a bater no limite do edge.
#[tokio::test]
async fn uma_parte_maior_do_que_o_pedaco_e_recusada() {
    let Some(c) = cenario().await else { return };
    let pedaco = upload::CHUNK_SIZE_BYTES as usize;
    let tamanho = pedaco + 512;

    let mut tx = c.pool.begin().await.expect("tx");
    let sessao = upload::begin(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        "teste",
        c.workspace_id,
        NewUpload {
            filename: "grande.zip".to_owned(),
            content_type: "application/zip".to_owned(),
            size_bytes: tamanho as i64,
            classification: None,
            folder_id: None,
            file_id: None,
        },
    )
    .await
    .expect("abrir");
    tx.commit().await.expect("commit");

    // O ficheiro inteiro numa parte só.
    let inteiro = bytes(tamanho, 23);
    let mut tx = c.pool.begin().await.expect("tx");
    let erro = upload::accept_part(
        &mut tx,
        &c.quem_carrega,
        &c.store,
        sessao.id,
        1,
        &sha256_hex(&inteiro),
        inteiro,
    )
    .await;
    assert!(
        erro.is_err(),
        "uma parte com o ficheiro inteiro foi aceite; a segmentação seria contornável"
    );

    // E uma parte fora do intervalo.
    let troco = bytes(128, 29);
    let erro = upload::accept_part(
        &mut tx,
        &c.quem_carrega,
        &c.store,
        sessao.id,
        99,
        &sha256_hex(&troco),
        troco,
    )
    .await;
    assert!(erro.is_err(), "uma parte fora do carregamento foi aceite");
}

/// Uma parte cuja soma não corresponde aos bytes é recusada à chegada.
#[tokio::test]
async fn uma_parte_corrompida_e_recusada_a_chegada() {
    let Some(c) = cenario().await else { return };
    let ficheiro = bytes(2048, 31);

    let mut tx = c.pool.begin().await.expect("tx");
    let sessao = upload::begin(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        "teste",
        c.workspace_id,
        NewUpload {
            filename: "corrompida.zip".to_owned(),
            content_type: "application/zip".to_owned(),
            size_bytes: ficheiro.len() as i64,
            classification: None,
            folder_id: None,
            file_id: None,
        },
    )
    .await
    .expect("abrir");
    tx.commit().await.expect("commit");

    let mut tx = c.pool.begin().await.expect("tx");
    let erro = upload::accept_part(
        &mut tx,
        &c.quem_carrega,
        &c.store,
        sessao.id,
        1,
        &sha256_hex(b"a soma de outra coisa"),
        ficheiro,
    )
    .await;
    assert!(erro.is_err(), "uma parte com soma errada foi aceite");
}

/// A pertença revogada a meio fecha a porta no fim.
///
/// Entre abrir e fechar podem passar horas. Autorizar só na abertura daria a
/// quem já não pode uma janela que fica aberta enquanto o carregamento durar.
#[tokio::test]
async fn a_pertenca_revogada_a_meio_recusa_o_fecho() {
    let Some(c) = cenario().await else { return };
    let ficheiro = bytes(2048, 37);
    let soma = sha256_hex(&ficheiro);

    let mut tx = c.pool.begin().await.expect("tx");
    let sessao = upload::begin(
        &mut tx,
        &c.quem_carrega,
        &c.ids,
        &c.store,
        "teste",
        c.workspace_id,
        NewUpload {
            filename: "revogada.zip".to_owned(),
            content_type: "application/zip".to_owned(),
            size_bytes: ficheiro.len() as i64,
            classification: None,
            folder_id: None,
            file_id: None,
        },
    )
    .await
    .expect("abrir");
    tx.commit().await.expect("commit");

    let mut tx = c.pool.begin().await.expect("tx");
    upload::accept_part(
        &mut tx,
        &c.quem_carrega,
        &c.store,
        sessao.id,
        1,
        &soma,
        ficheiro,
    )
    .await
    .expect("parte");
    tx.commit().await.expect("commit");

    // A pertença desaparece enquanto o carregamento estava a decorrer.
    sqlx::query("DELETE FROM workspace_memberships WHERE workspace_id = $1 AND person_id = $2")
        .bind(c.workspace_id)
        .bind(c.quem_carrega.person_id)
        .execute(&c.pool)
        .await
        .expect("revogar");

    // O principal é relido: um principal em cache diria que ainda pertence.
    let pessoa = ocinye_core::modules::identity::person_by_id(&c.pool, c.quem_carrega.person_id)
        .await
        .expect("leitura")
        .expect("pessoa");
    let agora = ocinye_core::modules::identity::principal_for_person(&c.pool, &pessoa)
        .await
        .expect("principal");

    let mut tx = c.pool.begin().await.expect("tx");
    let erro = upload::finalise(&mut tx, &agora, &c.ids, &c.store, sessao.id, &soma).await;
    assert!(
        erro.is_err(),
        "quem perdeu a pertença a meio conseguiu fechar o carregamento"
    );

    let ficheiros: i64 = sqlx::query_scalar("SELECT count(*) FROM files WHERE workspace_id = $1")
        .bind(c.workspace_id)
        .fetch_one(&c.pool)
        .await
        .expect("contagem");
    assert_eq!(ficheiros, 0, "ficou um File de quem já não pertencia");
}
