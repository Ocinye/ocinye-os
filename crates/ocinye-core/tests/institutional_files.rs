//! Ficheiros institucionais: identidade que dura, bytes que mudam.
//!
//! # A propriedade fundadora
//!
//! > **Um ficheiro institucional tem identidade estável; os seus bytes podem
//! > evoluir através de versões imutáveis sem destruir a história anterior.**
//!
//! É o que separa isto de uma pasta partilhada. Numa pasta, actualizar um
//! relatório é escrever por cima — e quem citou o anterior fica a citar outra
//! coisa sem saber. Aqui, a versão que alguém citou continua a existir e a
//! apontar exactamente para os mesmos bytes.
//!
//! # Porque estes testes falam SQL
//!
//! Porque as invariantes que provam são da **base**, e não do serviço. Uma
//! restrição que só existe no Rust protege o caminho que passa pelo Rust; uma
//! que está na base protege também a migração, o script de manutenção e a
//! consulta que alguém escreveu às três da manhã.
//!
//! Estes testes saltam quando `OCINYE_TEST_DATABASE_URL` não está definida, e
//! **falham** quando está definida e a base não responde.

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

/// Um contexto institucional mínimo: organização, unidade, ambiente e backend.
struct Contexto {
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    backend_id: Uuid,
}

async fn contexto(pool: &PgPool) -> Contexto {
    let sufixo = Uuid::new_v4().simple().to_string();
    let organisation_id: Uuid =
        sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $2) RETURNING id")
            .bind(format!("org{}", &sufixo[..12]))
            .bind("Organização de prova")
            .fetch_one(pool)
            .await
            .expect("organização");
    let unit_id: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(organisation_id)
    .bind(format!("U{}", &sufixo[..8]))
    .bind("Unidade de prova")
    .fetch_one(pool)
    .await
    .expect("unidade");
    let workspace_id: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces (organisation_id, unit_id, code, title)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(format!("W{}", &sufixo[..12]))
    .bind("Ambiente de prova")
    .fetch_one(pool)
    .await
    .expect("ambiente");
    let backend_id: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_backends (code, display_name, location_label, bucket)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(format!("b{}", &sufixo[..12]))
    .bind("Armazenamento de prova")
    .bind("local")
    .bind("prova")
    .fetch_one(pool)
    .await
    .expect("backend");

    Contexto {
        organisation_id,
        unit_id,
        workspace_id,
        backend_id,
    }
}

/// Bytes guardados, com uma soma que os distingue.
async fn objecto(pool: &PgPool, ctx: &Contexto, marca: char) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO storage_objects
             (backend_id, organisation_id, unit_id, workspace_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256, status)
         VALUES ($1, $2, $3, $4, $5, 'montagem.png', 'image/png', 10, $6, 'stored')
         RETURNING id",
    )
    .bind(ctx.backend_id)
    .bind(ctx.organisation_id)
    .bind(ctx.unit_id)
    .bind(ctx.workspace_id)
    .bind(format!("k/{}", Uuid::new_v4().simple()))
    .bind(marca.to_string().repeat(64))
    .fetch_one(pool)
    .await
    .expect("objecto")
}

async fn ficheiro(pool: &PgPool, ctx: &Contexto) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO files (organisation_id, unit_id, workspace_id, name)
         VALUES ($1, $2, $3, 'montagem.png') RETURNING id",
    )
    .bind(ctx.organisation_id)
    .bind(ctx.unit_id)
    .bind(ctx.workspace_id)
    .fetch_one(pool)
    .await
    .expect("ficheiro")
}

async fn versao(
    pool: &PgPool,
    file_id: Uuid,
    sequencia: i32,
    objecto_id: Uuid,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO file_versions (file_id, sequence, storage_object_id)
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(file_id)
    .bind(sequencia)
    .bind(objecto_id)
    .fetch_one(pool)
    .await
}

/// Uma versão nova não toca na anterior.
///
/// # O defeito que isto guarda
///
/// A implementação mais curta para «carregar nova versão» é um `UPDATE` que
/// troca o objecto na linha que já existe. Passa em todos os testes que olham
/// para a versão corrente, e apaga em silêncio a que alguém citou. É a
/// diferença entre versionar e sobrescrever, e não se vê pelo lado de fora.
#[tokio::test]
async fn uma_versao_nova_nao_toca_na_anterior() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;
    let o1 = objecto(&pool, &ctx, 'a').await;
    let o2 = objecto(&pool, &ctx, 'b').await;

    versao(&pool, f, 1, o1).await.expect("v1");
    versao(&pool, f, 2, o2).await.expect("v2");

    let ainda: Uuid = sqlx::query_scalar(
        "SELECT storage_object_id FROM file_versions WHERE file_id=$1 AND sequence=1",
    )
    .bind(f)
    .fetch_one(&pool)
    .await
    .expect("v1 continua a existir");
    assert_eq!(ainda, o1, "a versão 1 passou a apontar para outros bytes");

    let corrente: i32 =
        sqlx::query_scalar("SELECT max(sequence) FROM file_versions WHERE file_id=$1")
            .bind(f)
            .fetch_one(&pool)
            .await
            .expect("corrente");
    assert_eq!(corrente, 2, "a corrente não é a de maior sequência");

    let quantas: i64 = sqlx::query_scalar("SELECT count(*) FROM file_versions WHERE file_id=$1")
        .bind(f)
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(quantas, 2, "carregar a segunda versão fez desaparecer uma");
}

/// Os bytes de uma versão citada não se apagam.
///
/// # Porque isto se prova num ficheiro **sem documento**
///
/// Porque hoje `documents.storage_object_id` também protege o objecto, e um
/// teste feito sobre um documento passaria pela razão errada. Quando essa
/// coluna legada for retirada, é esta restrição que tem de continuar de pé —
/// e é esta que este teste observa.
#[tokio::test]
async fn os_bytes_de_uma_versao_citada_nao_se_apagam() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;
    let o = objecto(&pool, &ctx, 'c').await;
    versao(&pool, f, 1, o).await.expect("v1");

    let recusa = sqlx::query("DELETE FROM storage_objects WHERE id = $1")
        .bind(o)
        .execute(&pool)
        .await;
    let erro = recusa.expect_err("a base deixou apagar bytes que uma versão cita");
    assert!(
        erro.to_string().contains("file_versions"),
        "a recusa veio de outro sítio que não `file_versions`: {erro}"
    );
}

/// Duas versões com o mesmo número tornariam «a corrente» ambígua.
#[tokio::test]
async fn duas_versoes_com_o_mesmo_numero_sao_recusadas() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;
    let o1 = objecto(&pool, &ctx, 'd').await;
    let o2 = objecto(&pool, &ctx, 'e').await;
    versao(&pool, f, 1, o1).await.expect("v1");

    let erro = versao(&pool, f, 1, o2)
        .await
        .expect_err("duas versões ficaram com o número 1");
    assert!(
        erro.to_string().contains("uq_file_versions_sequence"),
        "recusado por outra razão: {erro}"
    );
}

/// Uma versão não aponta para bytes que não existem.
#[tokio::test]
async fn uma_versao_nao_aponta_para_bytes_inexistentes() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;

    let erro = versao(&pool, f, 1, Uuid::new_v4())
        .await
        .expect_err("uma versão ficou a apontar para o nada");
    assert!(
        erro.to_string().contains("foreign key"),
        "recusado por outra razão: {erro}"
    );
}

/// O mesmo objecto duas vezes no mesmo ficheiro é uma versão que não mudou nada.
#[tokio::test]
async fn o_mesmo_objecto_nao_entra_duas_vezes_no_mesmo_ficheiro() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;
    let o = objecto(&pool, &ctx, 'f').await;
    versao(&pool, f, 1, o).await.expect("v1");

    let erro = versao(&pool, f, 2, o)
        .await
        .expect_err("o mesmo objecto entrou como duas versões");
    assert!(
        erro.to_string().contains("uq_file_versions_object"),
        "recusado por outra razão: {erro}"
    );
}

/// A numeração começa em um.
#[tokio::test]
async fn a_sequencia_comeca_em_um() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;
    let o = objecto(&pool, &ctx, '0').await;

    let erro = versao(&pool, f, 0, o)
        .await
        .expect_err("uma versão zero foi aceite");
    assert!(
        erro.to_string().contains("ck_file_versions_sequence"),
        "recusado por outra razão: {erro}"
    );
}

/// Enquanto as duas fontes existirem, têm de concordar.
///
/// # Porque este teste existe, e por quanto tempo
///
/// `documents` guarda hoje o objecto de duas maneiras: directamente, em
/// `storage_object_id`, e através do ficheiro, na versão de maior sequência. É
/// uma redundância deliberada e temporária — a coluna antiga fica enquanto os
/// leitores migram.
///
/// Duas fontes da mesma verdade só são aceitáveis enquanto alguém as confronta.
/// Este teste é esse alguém, e desaparece com a coluna.
#[tokio::test]
async fn as_duas_fontes_do_objecto_de_um_documento_concordam() {
    let Some(pool) = pool().await else { return };

    let divergentes: i64 = sqlx::query_scalar(
        "SELECT count(*)
           FROM documents d
           JOIN file_versions v ON v.file_id = d.file_id
                               AND v.sequence = (SELECT max(sequence)
                                                   FROM file_versions x
                                                  WHERE x.file_id = d.file_id)
          WHERE d.file_id IS NOT NULL
            AND v.storage_object_id <> d.storage_object_id",
    )
    .fetch_one(&pool)
    .await
    .expect("comparação");

    assert_eq!(
        divergentes, 0,
        "{divergentes} documento(s) apontam para um objecto e a sua versão \
         corrente para outro. Enquanto a coluna antiga existir, as duas têm de \
         dizer o mesmo"
    );
}
