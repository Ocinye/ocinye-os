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
/// Um segundo ambiente na mesma organização e unidade.
async fn outro_ambiente(pool: &PgPool, ctx: &Contexto) -> Contexto {
    let sufixo = Uuid::new_v4().simple().to_string();
    let workspace_id: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces (organisation_id, unit_id, code, title)
         VALUES ($1, $2, $3, 'Outro ambiente') RETURNING id",
    )
    .bind(ctx.organisation_id)
    .bind(ctx.unit_id)
    .bind(format!("W{}", &sufixo[..12]))
    .fetch_one(pool)
    .await
    .expect("outro ambiente");
    Contexto {
        organisation_id: ctx.organisation_id,
        unit_id: ctx.unit_id,
        workspace_id,
        backend_id: ctx.backend_id,
    }
}

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

/// Quem carrega. A chave estrangeira exige que exista mesmo — e faz bem: um
/// autor inventado tornaria a autoria de uma versão uma decoração.
async fn pessoa(pool: &PgPool, ctx: &Contexto) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
         VALUES ($1, 'Quem carrega', $2, 'active') RETURNING id",
    )
    .bind(ctx.organisation_id)
    .bind(format!("p{}@ocinye.com", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .expect("pessoa")
}

async fn ficheiro(pool: &PgPool, ctx: &Contexto) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO files (organisation_id, unit_id, workspace_id, name, classification)
         VALUES ($1, $2, $3, 'montagem.png', 'INTERNAL') RETURNING id",
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

// ── Os escritores ───────────────────────────────────────────────────────
//
// Os testes acima provam as invariantes da base. Estes provam o comportamento
// de produção: que a operação canónica escreve as duas representações, e que
// acrescentar uma versão nunca substitui.

/// A sequência seguinte é do Core, e sobrevive a duas escritas ao mesmo tempo.
///
/// # O defeito que isto guarda
///
/// «Ler o máximo e somar um» sem tranca é uma corrida. Duas escritas
/// simultâneas lêem o mesmo máximo, decidem ambas que são a versão seguinte, e
/// a restrição única recusa a segunda — com uma mensagem de SQL, numa
/// transacção que não tinha por onde saber que devia voltar a tentar.
///
/// Com a tranca sobre o ficheiro, a segunda espera e recebe o número certo. As
/// duas versões entram, por ordem, e nenhuma se perde.
#[tokio::test]
async fn duas_escritas_ao_mesmo_tempo_nao_disputam_o_mesmo_numero() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;
    let o1 = objecto(&pool, &ctx, '1').await;
    let quem = pessoa(&pool, &ctx).await;

    // A primeira versão, para haver máximo a disputar.
    let mut tx = pool.begin().await.expect("tx");
    ocinye_core::modules::files::add_version(&mut tx, f, o1, None, quem)
        .await
        .expect("v1");
    tx.commit().await.expect("commit");

    let o2 = objecto(&pool, &ctx, '2').await;
    let o3 = objecto(&pool, &ctx, '3').await;

    // Duas transacções abertas ao mesmo tempo, ambas a pedir a versão seguinte.
    let a = pool.clone();
    let b = pool.clone();
    let primeira = tokio::spawn(async move {
        let mut tx = a.begin().await.expect("tx");
        let v = ocinye_core::modules::files::add_version(&mut tx, f, o2, None, quem).await;
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        let feito = v.map(|r| r.sequence);
        tx.commit().await.expect("commit");
        feito
    });
    let segunda = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let mut tx = b.begin().await.expect("tx");
        let v = ocinye_core::modules::files::add_version(&mut tx, f, o3, None, quem).await;
        let feito = v.map(|r| r.sequence);
        tx.commit().await.expect("commit");
        feito
    });

    let uma = primeira.await.expect("junta").expect("primeira versão");
    let outra = segunda.await.expect("junta").expect("segunda versão");

    let mut numeros = [uma, outra];
    numeros.sort_unstable();
    assert_eq!(
        numeros,
        [2, 3],
        "duas escritas concorrentes não produziram as versões 2 e 3: {numeros:?}"
    );

    let quantas: i64 = sqlx::query_scalar("SELECT count(*) FROM file_versions WHERE file_id=$1")
        .bind(f)
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(quantas, 3, "uma das versões concorrentes desapareceu");
}

/// Acrescentar uma versão preserva todas as anteriores, com os mesmos bytes.
#[tokio::test]
async fn acrescentar_versoes_preserva_os_bytes_de_todas() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;
    let quem = pessoa(&pool, &ctx).await;

    let mut objectos = Vec::new();
    for marca in ['a', 'b', 'c'] {
        let o = objecto(&pool, &ctx, marca).await;
        objectos.push(o);
        let mut tx = pool.begin().await.expect("tx");
        ocinye_core::modules::files::add_version(&mut tx, f, o, Some("nota"), quem)
            .await
            .expect("versão");
        tx.commit().await.expect("commit");
    }

    for (indice, esperado) in objectos.iter().enumerate() {
        let sequencia = i32::try_from(indice + 1).expect("sequência");
        let guardado: Uuid = sqlx::query_scalar(
            "SELECT storage_object_id FROM file_versions WHERE file_id=$1 AND sequence=$2",
        )
        .bind(f)
        .bind(sequencia)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|_| panic!("a versão {sequencia} desapareceu"));
        assert_eq!(
            guardado, *esperado,
            "a versão {sequencia} passou a apontar para outros bytes"
        );
    }

    let mut tx = pool.begin().await.expect("tx");
    let corrente = ocinye_core::modules::files::current_version(&mut tx, f)
        .await
        .expect("corrente")
        .expect("há versões");
    tx.commit().await.expect("commit");
    assert_eq!(corrente.1, 3, "a corrente não é a de maior sequência");
    assert_eq!(
        corrente.0, objectos[2],
        "a corrente aponta para outros bytes"
    );
}

/// Acrescentar uma versão a um ficheiro que não existe é recusado pelo domínio.
///
/// A chave estrangeira apanharia isto de qualquer forma, mas com uma mensagem
/// de SQL. Quem chama merece saber que o recurso não existe.
#[tokio::test]
async fn uma_versao_para_um_ficheiro_inexistente_e_recusada() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let o = objecto(&pool, &ctx, 'z').await;

    let mut tx = pool.begin().await.expect("tx");
    let erro =
        ocinye_core::modules::files::add_version(&mut tx, Uuid::new_v4(), o, None, Uuid::new_v4())
            .await
            .expect_err("aceitou uma versão para um ficheiro que não existe");
    assert!(
        matches!(erro, ocinye_core::error::CoreError::NotFound(_)),
        "recusado por outra razão: {erro:?}"
    );
}

// ── O corte dos leitores ────────────────────────────────────────────────
//
// O objecto de um documento passou a resolver-se pela identidade estável do
// ficheiro e pela sua versão corrente. Estes testes provam que a mudança tem
// efeito — e que não destrói a história.

/// Um documento com o seu ficheiro e uma versão, prontos a crescer.
async fn documento_com_ficheiro(pool: &PgPool, ctx: &Contexto, marca: char) -> (Uuid, Uuid, Uuid) {
    let o = objecto(pool, ctx, marca).await;
    let f = ficheiro(pool, ctx).await;
    versao(pool, f, 1, o).await.expect("v1");
    let d: Uuid = sqlx::query_scalar(
        "INSERT INTO documents
             (organisation_id, unit_id, workspace_id, file_id, title)
         VALUES ($1, $2, $3, $4, 'Relatório') RETURNING id",
    )
    .bind(ctx.organisation_id)
    .bind(ctx.unit_id)
    .bind(ctx.workspace_id)
    .bind(f)
    .fetch_one(pool)
    .await
    .expect("documento");
    (d, f, o)
}

/// O que o leitor devolve para um documento, pela mesma consulta da produção.
async fn objecto_visto_pelo_leitor(pool: &PgPool, documento: Uuid) -> Uuid {
    sqlx::query_scalar(
        "SELECT v.storage_object_id
           FROM documents d
           JOIN LATERAL (
               SELECT fv.storage_object_id
                 FROM file_versions fv
                WHERE fv.file_id = d.file_id
                ORDER BY fv.sequence DESC
                LIMIT 1
           ) v ON TRUE
          WHERE d.id = $1",
    )
    .bind(documento)
    .fetch_one(pool)
    .await
    .expect("o leitor não encontrou o documento")
}

/// Carregar uma versão nova muda o que o documento devolve.
///
/// # Porque isto é a prova que faltava
///
/// O versionamento existia na base e não tinha efeito nenhum para quem usa o
/// sistema: o documento continuava a devolver o objecto da coluna antiga, e
/// carregar uma segunda versão não mudava nada. É esta a primeira vez que a
/// história das versões governa o que a pessoa recebe.
#[tokio::test]
async fn uma_versao_nova_muda_o_que_o_documento_devolve() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let (documento, f, o1) = documento_com_ficheiro(&pool, &ctx, 'p').await;

    assert_eq!(
        objecto_visto_pelo_leitor(&pool, documento).await,
        o1,
        "antes de haver segunda versão, o documento já não devolvia a primeira"
    );

    let o2 = objecto(&pool, &ctx, 'q').await;
    let quem = pessoa(&pool, &ctx).await;
    let mut tx = pool.begin().await.expect("tx");
    ocinye_core::modules::files::add_version(&mut tx, f, o2, Some("gráfico corrigido"), quem)
        .await
        .expect("v2");
    tx.commit().await.expect("commit");

    assert_eq!(
        objecto_visto_pelo_leitor(&pool, documento).await,
        o2,
        "carregar uma versão nova não mudou o que o documento devolve"
    );

    // E a anterior continua exactamente onde estava.
    let historica: Uuid = sqlx::query_scalar(
        "SELECT storage_object_id FROM file_versions WHERE file_id=$1 AND sequence=1",
    )
    .bind(f)
    .fetch_one(&pool)
    .await
    .expect("v1 desapareceu");
    assert_eq!(
        historica, o1,
        "a versão 1 deixou de apontar para os bytes que sempre apontou"
    );
}

// ── A autoridade, e quem não a tem ──────────────────────────────────────

/// A classificação do objecto guardado não decide nada.
///
/// # Porque este teste existe
///
/// A auditoria da autorização mediu que `storage_objects.classification` é
/// escrita e **nunca lida** para decidir: zero ocorrências em qualquer consulta
/// que produza `ALLOW`/`DENY`. É defesa em profundidade e metadados de
/// armazenamento, e não autoridade institucional.
///
/// Isso é verdade hoje. Este teste existe para que continue a ser: mudar só a
/// classificação do objecto não pode transformar uma recusa numa permissão nem
/// o contrário. Se alguém a voltar a ligar à decisão — por conveniência, num
/// `JOIN` que parece inofensivo — é aqui que se sabe.
///
/// > **Consistência defensiva não é autoridade semântica.**
#[tokio::test]
async fn a_classificacao_do_objecto_nao_decide_o_acesso() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;
    let o = objecto(&pool, &ctx, 'g').await;
    versao(&pool, f, 1, o).await.expect("v1");

    // O ficheiro é INTERNAL — é o que o ajudante cria. O objecto passa a
    // afirmar o extremo oposto do espectro, duas vezes.
    for extremo in ["RESTRICTED", "PUBLIC"] {
        sqlx::query("UPDATE storage_objects SET classification = $1 WHERE id = $2")
            .bind(extremo)
            .bind(o)
            .execute(&pool)
            .await
            .expect("classificar o objecto");

        let do_ficheiro: String =
            sqlx::query_scalar("SELECT classification FROM files WHERE id = $1")
                .bind(f)
                .fetch_one(&pool)
                .await
                .expect("classificação do ficheiro");
        assert_eq!(
            do_ficheiro, "INTERNAL",
            "mudar a classificação do objecto mudou a do ficheiro: passaram a \
             ser a mesma coisa, e uma delas era defesa e não autoridade"
        );
    }
}

/// As duas raízes de contexto não se podem contradizer.
///
/// # O estado que a base torna impossível
///
/// ```text
/// File.unit_id      = Unidade A
/// File.workspace_id = ambiente da Unidade B
/// ```
///
/// Seriam duas raízes de autorização a discordar, e nada a acusar: a
/// resolução de papéis usa as duas. Uma chave estrangeira composta sobre
/// `(workspace_id, unit_id)` fá-lo falhar na escrita.
#[tokio::test]
async fn a_unidade_do_ficheiro_nao_discorda_da_do_ambiente() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;

    let outra: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, 'Outra') RETURNING id",
    )
    .bind(ctx.organisation_id)
    .bind(format!("X{}", &Uuid::new_v4().simple().to_string()[..8]))
    .fetch_one(&pool)
    .await
    .expect("outra unidade");

    let erro = sqlx::query("UPDATE files SET unit_id = $1 WHERE id = $2")
        .bind(outra)
        .bind(f)
        .execute(&pool)
        .await
        .expect_err("um ficheiro ficou com unidade diferente da do seu ambiente");
    assert!(
        erro.to_string().contains("fk_files_workspace_unit"),
        "recusado por outra razão: {erro}"
    );
}

/// Uma classificação fora do vocabulário institucional é recusada.
#[tokio::test]
async fn a_classificacao_do_ficheiro_tem_vocabulario_fechado() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let f = ficheiro(&pool, &ctx).await;

    let erro = sqlx::query("UPDATE files SET classification = 'SECRETO' WHERE id = $1")
        .bind(f)
        .execute(&pool)
        .await
        .expect_err("aceitou uma classificação inventada");
    assert!(
        erro.to_string().contains("ck_files_classification"),
        "recusado por outra razão: {erro}"
    );
}

// ── O ficheiro institucional que não é documento ────────────────────────

/// O `ObjectStore` de teste, quando existe um serviço S3-compatível.
/// O armazenamento ausente é uma conveniência local e um defeito na CI.
///
/// Um teste que se salta reporta `ok`, e `cargo test` engole o `eprintln!` de
/// um teste que passa — pelo que a guarda da CI que procura «skipping» na saída
/// nunca chegou a ver estes saltos. A CI ficava verde a afirmar que o
/// armazenamento tinha sido exercido quando não havia armazenamento nenhum.
///
/// Aqui, como no `harness!` para o Chrome: sem armazenamento em máquina de
/// alguém, salta-se; sem armazenamento na CI, falha.
fn exigir_armazenamento() {
    assert!(
        std::env::var("CI").is_err(),
        "não há armazenamento, e isto é a CI: o job que afirma provar \
         integração com object storage não pode saltar essa prova. \
         Defina OCINYE_TEST_STORAGE_ENDPOINT."
    );
    eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
}

fn test_store() -> Option<ocinye_core::storage::ObjectStore> {
    ocinye_core::storage::ObjectStore::new(ocinye_core::config::StorageConfig {
        endpoint_url: std::env::var("OCINYE_TEST_STORAGE_ENDPOINT").ok()?,
        region: std::env::var("OCINYE_TEST_STORAGE_REGION")
            .unwrap_or_else(|_| "us-east-1".to_owned()),
        access_key: std::env::var("OCINYE_TEST_STORAGE_ACCESS_KEY").ok()?,
        secret_key: std::env::var("OCINYE_TEST_STORAGE_SECRET_KEY").ok()?,
        bucket: std::env::var("OCINYE_TEST_STORAGE_BUCKET")
            .unwrap_or_else(|_| "ocinye-test-artifacts".to_owned()),
        backend_code: "test".to_owned(),
        location_label: "test".to_owned(),
        residency: ocinye_contracts::storage::Residency::Undeclared,
        max_upload_bytes: 32 * 1024 * 1024,
    })
}

/// Um membro com papel no ambiente, para poder criar e ler.
async fn membro(pool: &PgPool, ctx: &Contexto, papel: &str) -> ocinye_domain::Principal {
    let id = pessoa(pool, ctx).await;
    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, 'research_member')")
        .bind(id)
        .execute(pool)
        .await
        .expect("papel técnico");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role) VALUES ($1, $2, $3)",
    )
    .bind(ctx.workspace_id)
    .bind(id)
    .bind(papel)
    .execute(pool)
    .await
    .expect("filiação no ambiente");

    principal_do_teste(pool, ctx, id).await
}

/// Alguém da organização sem qualquer relação com o ambiente.
async fn estranho(pool: &PgPool, ctx: &Contexto) -> ocinye_domain::Principal {
    let id = pessoa(pool, ctx).await;
    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, 'research_member')")
        .bind(id)
        .execute(pool)
        .await
        .expect("papel técnico");
    principal_do_teste(pool, ctx, id).await
}

async fn principal_do_teste(
    pool: &PgPool,
    _ctx: &Contexto,
    person_id: Uuid,
) -> ocinye_domain::Principal {
    let registo = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("ler a pessoa")
        .expect("a pessoa existe");
    ocinye_core::modules::identity::principal_for_person(pool, &registo)
        .await
        .expect("principal")
}

/// Uma fotografia de uma montagem experimental é um ficheiro institucional.
///
/// # A propriedade que isto fecha
///
/// `File` deixou de existir para servir `Document`. Um PNG tem identidade,
/// contexto, classificação e histórico — e o sistema **não inventou** para ele
/// significado documental, científico ou de dados.
///
/// > **Carregar um ficheiro não é o mesmo que afirmar conhecimento
/// > institucional.**
#[tokio::test]
async fn um_png_e_um_ficheiro_institucional_sem_ser_documento() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        exigir_armazenamento();
        return;
    };
    let ctx = contexto(&pool).await;
    let quem = membro(&pool, &ctx, "lead").await;

    let antes = contagens(&pool, ctx.workspace_id).await;

    let criado = criar_ficheiro(
        &pool,
        &quem,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "montagem-experimental.png".to_owned(),
            content_type: "image/png".to_owned(),
            data: b"\x89PNG\r\n\x1a\n prova".to_vec(),
            classification: None,
        },
    )
    .await;

    assert_eq!(criado.sequence, 1, "a primeira versão não é a número 1");

    let depois = contagens(&pool, ctx.workspace_id).await;
    assert_eq!(
        depois.documentos, antes.documentos,
        "carregar um ficheiro criou um documento de conhecimento"
    );
    assert_eq!(
        depois.datasets, antes.datasets,
        "carregar um ficheiro criou um dataset"
    );
    assert_eq!(
        depois.fontes, antes.fontes,
        "carregar um ficheiro criou uma fonte bibliográfica"
    );

    // E é legível por quem o criou, pelo caminho de produção.
    let mut conn = pool.acquire().await.expect("ligação");
    let (ficheiro, _) = ocinye_core::modules::files::get(&mut conn, &quem, criado.file_id)
        .await
        .expect("quem carregou não consegue ler");
    assert_eq!(ficheiro.name, "montagem-experimental.png");
}

struct Contagens {
    documentos: i64,
    datasets: i64,
    fontes: i64,
}

/// Contagens **do ambiente sob prova**, nunca da base inteira: outros testes
/// correm ao mesmo tempo e criam documentos legítimos. Uma contagem global
/// transforma a asserção «carregar não cria conhecimento» numa asserção sobre
/// o que mais está a acontecer na máquina.
async fn contagens(pool: &PgPool, workspace_id: Uuid) -> Contagens {
    let (documentos, datasets, fontes): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM documents WHERE workspace_id = $1),
                (SELECT count(*) FROM datasets   WHERE workspace_id = $1),
                (SELECT count(*) FROM sources    WHERE workspace_id = $1)",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await
    .expect("contagens");
    Contagens {
        documentos,
        datasets,
        fontes,
    }
}

/// A mesma tranca que `identity.rs` usa para o registo de armazenamento.
///
/// # Porque é preciso
///
/// `is_default` é estado **global**: o caminho de carregamento escolhe
/// `WHERE is_default AND is_active`, e há um teste noutra suite que o limpa
/// para provar que a recusa explica a causa. Sem tranca, esse teste e estes
/// disputam a mesma linha — e os dois passam ou falham conforme a ordem.
///
/// A chave é a mesma de propósito: duas trancas diferentes sobre o mesmo estado
/// não protegem nada.
const TRANCA_DO_REGISTO: i64 = 0x0000_C109_E570_9A6E;

/// Cria um ficheiro com o registo de armazenamento trancado.
///
/// A janela é curta de propósito: tranca, garante que há armazenamento
/// registado, cria, destranca.
async fn criar_ficheiro(
    pool: &PgPool,
    quem: &ocinye_domain::Principal,
    store: &ocinye_core::storage::ObjectStore,
    workspace_id: Uuid,
    pedido: ocinye_core::modules::files::NewFile,
) -> ocinye_core::modules::files::FileVersionRecord {
    let mut tranca = pool.acquire().await.expect("ligação");
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(TRANCA_DO_REGISTO)
        .execute(&mut *tranca)
        .await
        .expect("tranca");

    backend_por_omissao(pool).await;
    let mut tx = pool.begin().await.expect("tx");
    let feito = ocinye_core::modules::files::create(
        &mut tx,
        quem,
        &ocinye_observability::CorrelationIds::generate(),
        store,
        "prova",
        workspace_id,
        pedido,
    )
    .await;
    if feito.is_ok() {
        tx.commit().await.expect("commit");
    } else {
        tx.rollback().await.expect("desfazer");
    }

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(TRANCA_DO_REGISTO)
        .execute(&mut *tranca)
        .await
        .expect("destranca");

    feito.expect("criar")
}

/// Garante que existe armazenamento registado, na **mesma linha** que as
/// outras suites usam.
///
/// O código é fixo de propósito. Uma linha nova por teste acumularia
/// armazenamentos, e a suite de identidade — que limpa os `is_default` para
/// provar que a recusa explica a causa — repõe apenas o que conhece. Duas
/// suites a inventar linhas diferentes sobre o mesmo estado global é a receita
/// para se anularem uma à outra conforme a ordem em que correm.
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

/// Conhecer o identificador não é uma forma de entrar.
///
/// # As três portas que se tentam
///
/// Quem não alcança o ambiente não pode chegar ao artefacto por nenhuma das
/// identidades que o compõem:
///
/// ```text
/// FileId          → recusado
/// FileVersionId   → recusado, pelo ficheiro que a governa
/// StorageObjectId → não é sequer uma porta: não há operação que o aceite
/// ```
///
/// A do meio é a que importa aqui. Uma versão **não tem autoridade própria**:
/// resolvê-la significa resolver o ficheiro e decidir por ele. Se a versão
/// respondesse sozinha, conhecer um identificador de versão seria contornar o
/// recurso que a contém.
#[tokio::test]
async fn conhecer_o_identificador_nao_contorna_a_autorizacao() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else { return };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let forasteiro = estranho(&pool, &ctx).await;

    // Um ficheiro CONFIDENCIAL: acima de INTERNAL, logo exige filiação.
    let criado = criar_ficheiro(
        &pool,
        &dono,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "reservado.png".to_owned(),
            content_type: "image/png".to_owned(),
            data: b"\x89PNG reservado".to_vec(),
            classification: Some(ocinye_contracts::Classification::Confidential),
        },
    )
    .await;

    let mut conn = pool.acquire().await.expect("ligação");

    // Porta 1: o identificador do ficheiro.
    let pelo_ficheiro =
        ocinye_core::modules::files::get(&mut conn, &forasteiro, criado.file_id).await;
    assert!(
        pelo_ficheiro.is_err(),
        "um estranho leu um ficheiro CONFIDENCIAL de um ambiente onde não entra"
    );

    // Porta 2: o identificador da versão. É esta que prova que a versão não
    // tem autoridade própria.
    let pela_versao =
        ocinye_core::modules::files::get_version(&mut conn, &forasteiro, criado.version_id).await;
    assert!(
        pela_versao.is_err(),
        "conhecer o identificador da versão contornou o ficheiro que a governa"
    );

    // E o dono alcança as duas, para que a recusa acima signifique alguma coisa.
    ocinye_core::modules::files::get(&mut conn, &dono, criado.file_id)
        .await
        .expect("quem tem filiação não alcança o próprio ficheiro");
    let (versao, ficheiro) =
        ocinye_core::modules::files::get_version(&mut conn, &dono, criado.version_id)
            .await
            .expect("quem tem filiação não alcança a versão");
    assert_eq!(versao.file_id, ficheiro.id);
    assert_eq!(
        ficheiro.classification(),
        ocinye_contracts::Classification::Confidential
    );
}

/// A descarga e a leitura decidem pela mesma composição, explicitamente.
///
/// # A fragilidade que isto fecha
///
/// Nos documentos, a leitura dobra a classificação do ambiente com a do
/// artefacto e a descarga usa só a do artefacto — funcionam igual porque um
/// portão anterior cobre a diferença. Aqui as duas chamam `file_context`, que
/// compõe sempre da mesma maneira. Se um dia divergirem, será por decisão
/// escrita na política de acção, e não porque um chamador se esqueceu.
#[tokio::test]
async fn a_descarga_recusa_a_quem_a_leitura_recusa() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else { return };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let forasteiro = estranho(&pool, &ctx).await;
    let ids = ocinye_observability::CorrelationIds::generate();

    let criado = criar_ficheiro(
        &pool,
        &dono,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "restrito.png".to_owned(),
            content_type: "image/png".to_owned(),
            data: b"\x89PNG restrito".to_vec(),
            classification: Some(ocinye_contracts::Classification::Restricted),
        },
    )
    .await;

    let mut tx = pool.begin().await.expect("tx");
    let negada = ocinye_core::modules::files::download_url(
        &mut tx,
        &forasteiro,
        &ids,
        &store,
        criado.file_id,
    )
    .await;
    tx.rollback().await.expect("desfazer");
    assert!(
        negada.is_err(),
        "um estranho obteve ligação de descarga para material RESTRITO"
    );

    let mut tx = pool.begin().await.expect("tx");
    let permitida =
        ocinye_core::modules::files::download_url(&mut tx, &dono, &ids, &store, criado.file_id)
            .await;
    tx.commit().await.expect("commit");
    assert!(
        permitida.is_ok(),
        "quem tem filiação no ambiente não conseguiu descarregar: {:?}",
        permitida.err()
    );
}

// ── A matriz de paridade ────────────────────────────────────────────────
//
// A autoridade do artefacto mudou de representante: era `Document`, é `File`.
// A política não mudou, e é isso que estes testes medem — não que o novo
// caminho funcione, mas que **decide exactamente o mesmo**.

/// Uma classificação ilegível cai no mais restritivo.
///
/// # Porque isto tem teste próprio
///
/// Porque `unwrap_or(Restricted)` e `unwrap_or(Internal)` são uma letra de
/// diferença e a segunda é uma porta aberta. Uma refactorização que troque um
/// pelo outro compila, passa em tudo o resto, e transforma um valor corrompido
/// numa permissão.
///
/// A base recusa valores fora do vocabulário — há teste disso —, o que torna
/// este caso improvável e não impossível: uma migração futura, uma restauração
/// parcial, uma coluna alargada. Fail-closed é uma decisão, e as decisões
/// medem-se.
#[test]
fn uma_classificacao_ilegivel_cai_no_mais_restritivo() {
    use ocinye_contracts::Classification;
    assert_eq!(
        Classification::parse("LIXO").unwrap_or(Classification::Restricted),
        Classification::Restricted,
        "um valor que a instituição não reconhece passou a ser legível por mais gente"
    );
    assert_eq!(
        Classification::parse("").unwrap_or(Classification::Restricted),
        Classification::Restricted
    );
}

/// Os seis actores que a política distingue.
struct Elenco {
    membro_da_organizacao: ocinye_domain::Principal,
    membro_do_ambiente: ocinye_domain::Principal,
    membro_da_unidade: ocinye_domain::Principal,
    gestor_da_unidade: ocinye_domain::Principal,
    administrador: ocinye_domain::Principal,
}

async fn elenco(pool: &PgPool, ctx: &Contexto) -> Elenco {
    async fn com(
        pool: &PgPool,
        ctx: &Contexto,
        papeis: &[&str],
        no_ambiente: Option<&str>,
        na_unidade: Option<&str>,
    ) -> ocinye_domain::Principal {
        let id = pessoa(pool, ctx).await;
        for papel in papeis {
            sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
                .bind(id)
                .bind(*papel)
                .execute(pool)
                .await
                .expect("papel");
        }
        if let Some(papel) = no_ambiente {
            sqlx::query(
                "INSERT INTO workspace_memberships (workspace_id, person_id, role)
                 VALUES ($1, $2, $3)",
            )
            .bind(ctx.workspace_id)
            .bind(id)
            .bind(papel)
            .execute(pool)
            .await
            .expect("filiação no ambiente");
        }
        if let Some(papel) = na_unidade {
            sqlx::query(
                "INSERT INTO unit_memberships (unit_id, person_id, role) VALUES ($1, $2, $3)",
            )
            .bind(ctx.unit_id)
            .bind(id)
            .bind(papel)
            .execute(pool)
            .await
            .expect("filiação na unidade");
        }
        principal_do_teste(pool, ctx, id).await
    }

    Elenco {
        membro_da_organizacao: com(pool, ctx, &["research_member"], None, None).await,
        membro_do_ambiente: com(pool, ctx, &["research_member"], Some("member"), None).await,
        membro_da_unidade: com(pool, ctx, &["research_member"], None, Some("member")).await,
        gestor_da_unidade: com(pool, ctx, &["research_member"], None, Some("manager")).await,
        administrador: com(pool, ctx, &["platform_admin"], None, None).await,
    }
}

/// A matriz: quatro classificações × cinco actores × leitura e descarga.
///
/// # O que se espera, e de onde vem
///
/// Da política existente, medida no B1 e **não alterada**:
///
/// ```text
/// PUBLIC / INTERNAL   qualquer membro activo da organização
/// CONFIDENTIAL        ambiente, unidade, ou administrador
/// RESTRICTED          ambiente, ou gestor da unidade — administrador não basta
/// ```
///
/// A última linha é a que mais importa: um privilégio administrativo não abre
/// material RESTRITO, e essa é uma decisão institucional que sobrevive a esta
/// migração intacta.
#[tokio::test]
async fn a_matriz_de_acesso_e_a_mesma_depois_do_ficheiro_governar() {
    use ocinye_contracts::Classification::{Confidential, Internal, Public, Restricted};
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else { return };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let e = elenco(&pool, &ctx).await;
    let ids = ocinye_observability::CorrelationIds::generate();

    for (nivel, esperado) in [
        // (organização, ambiente, unidade, gestor, administrador)
        (Public, [true, true, true, true, true]),
        (Internal, [true, true, true, true, true]),
        (Confidential, [false, true, true, true, true]),
        (Restricted, [false, true, false, true, false]),
    ] {
        let criado = criar_ficheiro(
            &pool,
            &dono,
            &store,
            ctx.workspace_id,
            ocinye_core::modules::files::NewFile {
                filename: "matriz.png".to_owned(),
                content_type: "image/png".to_owned(),
                data: format!("PNG {nivel:?}").into_bytes(),
                classification: Some(nivel),
            },
        )
        .await;

        let actores = [
            ("membro da organização", &e.membro_da_organizacao),
            ("membro do ambiente", &e.membro_do_ambiente),
            ("membro da unidade", &e.membro_da_unidade),
            ("gestor da unidade", &e.gestor_da_unidade),
            ("administrador", &e.administrador),
        ];

        for ((nome, quem), permitido) in actores.iter().zip(esperado) {
            let mut conn = pool.acquire().await.expect("ligação");
            let leu = ocinye_core::modules::files::get(&mut conn, quem, criado.file_id)
                .await
                .is_ok();
            assert_eq!(
                leu, permitido,
                "leitura de {nivel:?} por «{nome}»: esperava {permitido}, deu {leu}"
            );

            let mut tx = pool.begin().await.expect("tx");
            let descarregou = ocinye_core::modules::files::download_url(
                &mut tx,
                quem,
                &ids,
                &store,
                criado.file_id,
            )
            .await
            .is_ok();
            tx.rollback().await.expect("desfazer");
            assert_eq!(
                descarregou, permitido,
                "descarga de {nivel:?} por «{nome}»: esperava {permitido}, deu {descarregou}"
            );

            // E as duas nunca discordam entre si. É a fragilidade do caminho
            // documental, fechada aqui por construção.
            assert_eq!(
                leu, descarregou,
                "leitura e descarga divergiram para {nivel:?} por «{nome}»"
            );
        }
    }
}

/// O ambiente restringe o artefacto **na leitura**, e não só na escrita.
///
/// # Porque a ordem dos acontecimentos importa
///
/// A criação já normaliza: pedir INTERNO dentro de um ambiente RESTRITO guarda
/// RESTRITO. Isso é correcto e **esconde** a composição na leitura — um teste
/// que crie o ficheiro num ambiente já restrito passa mesmo que a leitura
/// ignore o ambiente por completo, porque os dois valores coincidem.
///
/// O caso que separa os dois é este: um ficheiro nasce INTERNO num ambiente
/// INTERNO, e o ambiente **é restringido depois**. A classificação guardada
/// continua INTERNA, e só a composição na leitura impede que o artefacto
/// continue visível a toda a organização.
///
/// Foi assim que se descobriu que a versão anterior deste teste passava pela
/// razão errada: a reversão que retirava o `most_restrictive` da leitura não o
/// fazia falhar.
#[tokio::test]
async fn o_ambiente_restringe_o_artefacto_na_leitura() {
    use ocinye_contracts::Classification::Internal;
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else { return };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let forasteiro = estranho(&pool, &ctx).await;
    let _ids = ocinye_observability::CorrelationIds::generate();

    // O ambiente ainda é INTERNO: o ficheiro nasce INTERNO de facto.
    let criado = criar_ficheiro(
        &pool,
        &dono,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "interno.png".to_owned(),
            content_type: "image/png".to_owned(),
            data: b"PNG interno".to_vec(),
            classification: Some(Internal),
        },
    )
    .await;

    let guardada: String = sqlx::query_scalar("SELECT classification FROM files WHERE id = $1")
        .bind(criado.file_id)
        .fetch_one(&pool)
        .await
        .expect("classificação");
    assert_eq!(
        guardada,
        Internal.as_str(),
        "o ficheiro não ficou INTERNO, e o teste deixaria de exercer a composição"
    );

    // Um membro da organização alcança-o — é INTERNO e o ambiente também.
    let mut conn = pool.acquire().await.expect("ligação");
    ocinye_core::modules::files::get(&mut conn, &forasteiro, criado.file_id)
        .await
        .expect("um artefacto INTERNO num ambiente INTERNO devia ser legível");

    // O ambiente é restringido depois. A classificação guardada não muda.
    sqlx::query("UPDATE research_workspaces SET classification = 'RESTRICTED' WHERE id = $1")
        .bind(ctx.workspace_id)
        .execute(&pool)
        .await
        .expect("restringir o ambiente");

    let ainda: String = sqlx::query_scalar("SELECT classification FROM files WHERE id = $1")
        .bind(criado.file_id)
        .fetch_one(&pool)
        .await
        .expect("classificação");
    assert_eq!(
        ainda,
        Internal.as_str(),
        "restringir o ambiente reescreveu o ficheiro"
    );

    // E agora só a composição na leitura o pode esconder.
    let mut conn = pool.acquire().await.expect("ligação");
    assert!(
        ocinye_core::modules::files::get(&mut conn, &forasteiro, criado.file_id)
            .await
            .is_err(),
        "o ambiente foi restringido e o artefacto continuou legível a toda a \
         organização: a leitura não compõe as duas classificações"
    );

    // E quem tem filiação continua a alcançá-lo.
    let mut conn = pool.acquire().await.expect("ligação");
    ocinye_core::modules::files::get(&mut conn, &dono, criado.file_id)
        .await
        .expect("quem lidera o ambiente deixou de alcançar o próprio artefacto");
}

// ── A pesquisa responde à autoridade actual ─────────────────────────────

/// Reclassificar o ambiente esconde o recurso da pesquisa, sem reindexar.
///
/// # A fuga que isto fecha
///
/// ```text
/// t0   ambiente INTERNO · ficheiro INTERNO · índice INTERNO   → encontra
/// t1   ambiente → RESTRITO
///      leitura   → recusa
///      descarga  → recusa
///      pesquisa  → continuaria a revelar
/// ```
///
/// O índice guarda a classificação do momento em que indexou. A autoridade
/// muda por reclassificação, por filiação, por papel — e nenhuma dessas
/// mudanças toca no artefacto indexado. Sincronizar duas verdades por
/// reindexação seria frágil por natureza.
///
/// > **A pesquisa usa o índice para descobrir candidatos; a visibilidade
/// > decide-se contra o estado autoritativo actual.**
#[tokio::test]
async fn reclassificar_o_ambiente_esconde_o_recurso_da_pesquisa() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let dentro = membro(&pool, &ctx, "lead").await;
    let fora = estranho(&pool, &ctx).await;

    // Um recurso indexado como INTERNO, num ambiente INTERNO.
    let termo = format!("piranometro{}", &Uuid::new_v4().simple().to_string()[..8]);
    sqlx::query(
        "INSERT INTO search_documents
             (organisation_id, unit_id, workspace_id, entity_type, entity_id,
              title, excerpt, classification, search_vector)
         VALUES ($1, $2, $3, 'document', gen_random_uuid(), $4, $4, 'INTERNAL',
                 to_tsvector('simple', $4))",
    )
    .bind(ctx.organisation_id)
    .bind(ctx.unit_id)
    .bind(ctx.workspace_id)
    .bind(&termo)
    .execute(&pool)
    .await
    .expect("indexar");

    let procurar = |quem: ocinye_domain::Principal, termo: String| {
        let pool = pool.clone();
        async move {
            ocinye_core::modules::search::search(
                &pool,
                &quem,
                &termo,
                None,
                None,
                ocinye_contracts::PageRequest::default(),
            )
            .await
            .expect("pesquisar")
        }
    };

    // Antes: qualquer membro da organização encontra material INTERNO.
    let (achados, total) = procurar(fora.clone(), termo.clone()).await;
    assert_eq!(achados.len(), 1, "o recurso INTERNO não foi encontrado");
    assert_eq!(total, 1, "a contagem não corresponde às linhas");

    // O ambiente é restringido. **Nada é reindexado.**
    sqlx::query("UPDATE research_workspaces SET classification = 'RESTRICTED' WHERE id = $1")
        .bind(ctx.workspace_id)
        .execute(&pool)
        .await
        .expect("restringir");

    let indexada: String =
        sqlx::query_scalar("SELECT classification FROM search_documents WHERE title = $1")
            .bind(&termo)
            .fetch_one(&pool)
            .await
            .expect("classificação indexada");
    assert_eq!(
        indexada, "INTERNAL",
        "o índice foi reescrito, e o teste deixaria de provar que não precisa de o ser"
    );

    let (achados, total) = procurar(fora, termo.clone()).await;
    assert!(
        achados.is_empty(),
        "a pesquisa revelou um recurso dentro de um ambiente restrito, \
         confiando na classificação copiada no índice"
    );
    assert_eq!(
        total, 0,
        "a contagem revelou a existência do recurso que as linhas escondem"
    );

    // E quem tem filiação continua a encontrá-lo.
    let (achados, _) = procurar(dentro, termo).await;
    assert_eq!(
        achados.len(),
        1,
        "quem lidera o ambiente deixou de encontrar o próprio recurso"
    );
}

// ── Pastas ──────────────────────────────────────────────────────────────

/// Uma pasta chamada «Público» não torna nada público.
///
/// # Porque isto tem de ser um teste e não uma promessa
///
/// Porque é a tentação óbvia. Uma pasta é onde as pessoas arrumam, e a seguir
/// alguém pensa que arrumar é classificar. Se a pasta tivesse classificação —
/// ou herança, ou grants —, arrastar um artefacto seria reclassificá-lo com um
/// gesto, e sem ninguém decidir nada.
///
/// > **A pasta organiza. O ficheiro é governado.**
#[tokio::test]
async fn mover_para_uma_pasta_chamada_publico_nao_muda_o_acesso() {
    use ocinye_contracts::Classification::Restricted;
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else { return };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let fora = estranho(&pool, &ctx).await;
    let ids = ocinye_observability::CorrelationIds::generate();

    let criado = criar_ficheiro(
        &pool,
        &dono,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "restrito.png".to_owned(),
            content_type: "image/png".to_owned(),
            data: b"PNG restrito".to_vec(),
            classification: Some(Restricted),
        },
    )
    .await;

    let mut tx = pool.begin().await.expect("tx");
    let publico = ocinye_core::modules::files::create_folder(
        &mut tx,
        &dono,
        ctx.workspace_id,
        None,
        "Público",
    )
    .await
    .expect("criar pasta");
    tx.commit().await.expect("commit");

    let mut tx = pool.begin().await.expect("tx");
    ocinye_core::modules::files::move_to_folder(
        &mut tx,
        &dono,
        &ids,
        criado.file_id,
        Some(publico),
    )
    .await
    .expect("mover");
    tx.commit().await.expect("commit");

    let guardada: String = sqlx::query_scalar("SELECT classification FROM files WHERE id = $1")
        .bind(criado.file_id)
        .fetch_one(&pool)
        .await
        .expect("classificação");
    assert_eq!(
        guardada,
        Restricted.as_str(),
        "arrastar para uma pasta chamada «Público» reclassificou o artefacto"
    );

    let mut conn = pool.acquire().await.expect("ligação");
    assert!(
        ocinye_core::modules::files::get(&mut conn, &fora, criado.file_id)
            .await
            .is_err(),
        "a pasta abriu o acesso a um artefacto RESTRITO"
    );

    // E a listagem da pasta também não o revela.
    let visto = ocinye_core::modules::files::browse(&pool, &fora, ctx.workspace_id, Some(publico))
        .await
        .expect("navegar");
    assert!(
        visto.files.is_empty(),
        "a listagem da pasta revelou um artefacto que a leitura recusa"
    );
}

/// Mover um ficheiro para a pasta de outro ambiente não é uma operação de pasta.
#[tokio::test]
async fn mover_para_outro_ambiente_e_recusado() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else { return };
    let ctx = contexto(&pool).await;
    // Outro ambiente da **mesma** organização: é aí que a fronteira que
    // interessa está. Noutra organização a resposta certa é «não existe», que
    // não revela sequer que a pasta há.
    let outro = outro_ambiente(&pool, &ctx).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let dono_do_outro = membro(&pool, &outro, "lead").await;
    let ids = ocinye_observability::CorrelationIds::generate();

    let criado = criar_ficheiro(
        &pool,
        &dono,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "aqui.png".to_owned(),
            content_type: "image/png".to_owned(),
            data: b"PNG aqui".to_vec(),
            classification: None,
        },
    )
    .await;

    let mut tx = pool.begin().await.expect("tx");
    let alheia = ocinye_core::modules::files::create_folder(
        &mut tx,
        &dono_do_outro,
        outro.workspace_id,
        None,
        "Arquivo",
    )
    .await
    .expect("criar pasta no outro ambiente");
    tx.commit().await.expect("commit");

    let mut tx = pool.begin().await.expect("tx");
    let erro = ocinye_core::modules::files::move_to_folder(
        &mut tx,
        &dono,
        &ids,
        criado.file_id,
        Some(alheia),
    )
    .await
    .expect_err("um ficheiro atravessou a fronteira de autoridade por um arrasto");
    tx.rollback().await.expect("desfazer");
    assert!(
        matches!(erro, ocinye_core::error::CoreError::Validation(_)),
        "recusado por outra razão: {erro:?}"
    );
}

/// Navegar mostra pastas e ficheiros, e as migalhas sobem até à raiz.
#[tokio::test]
async fn navegar_mostra_a_arvore_e_o_caminho() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else { return };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let ids = ocinye_observability::CorrelationIds::generate();

    let mut tx = pool.begin().await.expect("tx");
    let engenharia = ocinye_core::modules::files::create_folder(
        &mut tx,
        &dono,
        ctx.workspace_id,
        None,
        "Engenharia",
    )
    .await
    .expect("pasta");
    let ensaios = ocinye_core::modules::files::create_folder(
        &mut tx,
        &dono,
        ctx.workspace_id,
        Some(engenharia),
        "Ensaios",
    )
    .await
    .expect("subpasta");
    let criado = ocinye_core::modules::files::create(
        &mut tx,
        &dono,
        &ids,
        &store,
        "prova",
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "ensaio-03.png".to_owned(),
            content_type: "image/png".to_owned(),
            data: b"PNG ensaio".to_vec(),
            classification: None,
        },
    )
    .await
    .expect("criar");
    ocinye_core::modules::files::move_to_folder(
        &mut tx,
        &dono,
        &ids,
        criado.file_id,
        Some(ensaios),
    )
    .await
    .expect("mover");
    tx.commit().await.expect("commit");

    let raiz = ocinye_core::modules::files::browse(&pool, &dono, ctx.workspace_id, None)
        .await
        .expect("raiz");
    assert!(
        raiz.folders.iter().any(|f| f.name == "Engenharia"),
        "a raiz não mostra a pasta criada"
    );

    let dentro = ocinye_core::modules::files::browse(&pool, &dono, ctx.workspace_id, Some(ensaios))
        .await
        .expect("dentro");
    assert_eq!(dentro.files.len(), 1, "o ficheiro movido não está na pasta");
    assert_eq!(dentro.files[0].name, "ensaio-03.png");
    assert_eq!(dentro.files[0].versions, 1);
    let migalhas: Vec<&str> = dentro.path.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        migalhas,
        vec!["Engenharia", "Ensaios"],
        "as migalhas não sobem até à raiz pela ordem certa"
    );
}

/// Duas pastas irmãs com o mesmo nome tornam um caminho ambíguo.
#[tokio::test]
async fn duas_pastas_irmas_nao_partilham_o_nome() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;

    let mut tx = pool.begin().await.expect("tx");
    ocinye_core::modules::files::create_folder(&mut tx, &dono, ctx.workspace_id, None, "Ensaios")
        .await
        .expect("primeira");
    let erro = ocinye_core::modules::files::create_folder(
        &mut tx,
        &dono,
        ctx.workspace_id,
        None,
        "ensaios",
    )
    .await
    .expect_err("duas irmãs ficaram com o mesmo nome");
    tx.rollback().await.expect("desfazer");
    let _ = erro;
}

/// O histórico é informação **sobre** o ficheiro, e segue a autoridade do
/// ficheiro.
///
/// Saber que um ficheiro tem sete versões, carregadas por três pessoas ao longo
/// de dois meses, é saber alguma coisa sobre o trabalho de uma unidade. Quem não
/// alcança o ficheiro não deve aprender isso por perguntar pelo histórico.
#[tokio::test]
async fn o_historico_de_versoes_segue_a_autoridade_do_ficheiro() {
    let Some(pool) = pool().await else { return };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let de_fora = estranho(&pool, &ctx).await;

    let file_id = ficheiro(&pool, &ctx).await;
    // RESTRICTED: é a classificação em que a recusa é observável. Num ficheiro
    // INTERNAL qualquer membro activo lê, e o teste passaria sem provar nada.
    sqlx::query("UPDATE files SET classification = 'RESTRICTED' WHERE id = $1")
        .bind(file_id)
        .execute(&pool)
        .await
        .expect("restringir");
    for (n, marca) in [(1, 'a'), (2, 'b'), (3, 'c')] {
        let objecto_id = objecto(&pool, &ctx, marca).await;
        versao(&pool, file_id, n, objecto_id)
            .await
            .expect("versão de prova");
    }

    let mut conn = pool.acquire().await.expect("ligação");
    let historico = ocinye_core::modules::files::versions(&mut conn, &dono, file_id)
        .await
        .expect("quem alcança o ficheiro não vê o histórico");

    assert_eq!(historico.len(), 3, "o histórico não traz todas as versões");
    assert_eq!(
        historico.iter().map(|v| v.sequence).collect::<Vec<_>>(),
        vec![3, 2, 1],
        "o histórico não vem da mais recente para a mais antiga"
    );

    // E para quem não alcança o ficheiro, o histórico não existe.
    let recusa = ocinye_core::modules::files::versions(&mut conn, &de_fora, file_id).await;
    assert!(
        recusa.is_err(),
        "quem não alcança o ficheiro leu o histórico das suas versões"
    );
}

/// Uma versão exacta descarrega-se pela sua própria identidade, e essa
/// identidade não abre nada que o ficheiro feche.
///
/// Citar «o ficheiro» é citar bytes que mudam quando alguém carrega outra
/// versão. Por isso a descarga de uma versão determinada é uma operação própria
/// — e por isso ela tem de voltar a passar pela autoridade do ficheiro, e não
/// pela sorte de quem conhece um `UUID`.
#[tokio::test]
async fn descarregar_uma_versao_exacta_passa_pela_autoridade_do_ficheiro() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        exigir_armazenamento();
        return;
    };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let de_fora = estranho(&pool, &ctx).await;
    let ids = ocinye_observability::CorrelationIds::generate();

    let primeira = criar_ficheiro(
        &pool,
        &dono,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "ensaio.pdf".to_owned(),
            content_type: "application/pdf".to_owned(),
            data: b"%PDF-1.4 primeira".to_vec(),
            classification: Some(ocinye_contracts::Classification::Restricted),
        },
    )
    .await;

    // Uma segunda versão: a corrente passa a ser outra, e a primeira continua
    // citável exactamente como estava.
    let mut tx = pool.begin().await.expect("tx");
    let segunda = ocinye_core::modules::files::upload_version(
        &mut tx,
        &dono,
        &ids,
        &store,
        "prova",
        primeira.file_id,
        ocinye_core::modules::files::NewFile {
            filename: "ensaio.pdf".to_owned(),
            content_type: "application/pdf".to_owned(),
            data: b"%PDF-1.4 segunda".to_vec(),
            classification: None,
        },
    )
    .await
    .expect("segunda versão");
    tx.commit().await.expect("commit");
    assert_eq!(segunda.sequence, 2, "a segunda versão não é a número 2");

    // Quem alcança o ficheiro descarrega a versão antiga pela identidade dela.
    let mut tx = pool.begin().await.expect("tx");
    let url = ocinye_core::modules::files::version_download_url(
        &mut tx,
        &dono,
        &ids,
        &store,
        primeira.version_id,
    )
    .await
    .expect("quem alcança o ficheiro não descarregou a versão antiga");
    tx.commit().await.expect("commit");
    assert!(!url.is_empty(), "a ligação de descarga veio vazia");

    // E quem não o alcança não descarrega, mesmo sabendo o identificador exacto
    // da versão.
    let mut tx = pool.begin().await.expect("tx");
    let recusa = ocinye_core::modules::files::version_download_url(
        &mut tx,
        &de_fora,
        &ids,
        &store,
        primeira.version_id,
    )
    .await;
    assert!(
        recusa.is_err(),
        "conhecer o identificador da versão bastou para descarregar"
    );
}

/// A pré-visualização é uma representação autorizada, e não uma porta lateral.
///
/// Existe porque a alternativa — pôr a URL do armazenamento num `<img>` — faria
/// a camada de experiência conhecer a topologia do armazenamento e obrigaria a
/// `Content-Security-Policy` a depender do deployment. Os bytes passam pelo
/// Core, e por isso o Core tem de voltar a decidir quem os vê.
#[tokio::test]
async fn a_previsualizacao_passa_pela_autoridade_do_ficheiro() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        exigir_armazenamento();
        return;
    };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let de_fora = estranho(&pool, &ctx).await;
    let ids = ocinye_observability::CorrelationIds::generate();

    // Um PNG minúsculo mas verdadeiro.
    let png: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89,
    ];
    let criado = criar_ficheiro(
        &pool,
        &dono,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "montagem.png".to_owned(),
            content_type: "image/png".to_owned(),
            data: png.clone(),
            classification: Some(ocinye_contracts::Classification::Restricted),
        },
    )
    .await;

    let mut tx = pool.begin().await.expect("tx");
    let vista = ocinye_core::modules::files::preview(&mut tx, &dono, &ids, &store, criado.file_id)
        .await
        .expect("quem alcança o ficheiro não conseguiu vê-lo");
    tx.commit().await.expect("commit");

    assert_eq!(vista.content_type, "image/png", "o tipo servido mudou");
    assert_eq!(vista.bytes, png, "os bytes servidos não são os guardados");

    // E quem não alcança o ficheiro não o vê, mesmo com o identificador exacto.
    let mut tx = pool.begin().await.expect("tx");
    let recusa =
        ocinye_core::modules::files::preview(&mut tx, &de_fora, &ids, &store, criado.file_id).await;
    assert!(
        recusa.is_err(),
        "conhecer o identificador bastou para ver o conteúdo do ficheiro"
    );
}

/// Nem tudo o que é imagem se serve inline.
///
/// Um SVG é um documento com script. Servi-lo inline na origem do Workspace
/// seria executá-lo lá — e é por isso que a lista é de formatos raster e cresce
/// por decisão, não porque alguém carregou um ficheiro novo.
#[tokio::test]
async fn um_svg_nao_se_mostra_inline_so_por_ser_imagem() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        exigir_armazenamento();
        return;
    };
    let ctx = contexto(&pool).await;
    let dono = membro(&pool, &ctx, "lead").await;
    let ids = ocinye_observability::CorrelationIds::generate();

    assert!(
        !ocinye_core::modules::files::PREVIEWABLE_TYPES.contains(&"image/svg+xml"),
        "o SVG entrou na lista dos que se mostram inline"
    );

    let criado = criar_ficheiro(
        &pool,
        &dono,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "diagrama.svg".to_owned(),
            content_type: "image/svg+xml".to_owned(),
            data: b"<svg xmlns='http://www.w3.org/2000/svg'><script>1</script></svg>".to_vec(),
            classification: None,
        },
    )
    .await;

    let mut tx = pool.begin().await.expect("tx");
    let recusa =
        ocinye_core::modules::files::preview(&mut tx, &dono, &ids, &store, criado.file_id).await;
    assert!(
        recusa.is_err(),
        "um SVG foi servido inline na origem da aplicação"
    );

    // Mas continua a ser um ficheiro institucional legítimo: guarda-se,
    // lê-se e descarrega-se. Recusar a pré-visualização não é recusar o ficheiro.
    let mut tx = pool.begin().await.expect("tx");
    ocinye_core::modules::files::download_url(&mut tx, &dono, &ids, &store, criado.file_id)
        .await
        .expect("um ficheiro que não se pré-visualiza deixou de se poder descarregar");
    tx.commit().await.expect("commit");
}
