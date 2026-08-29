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

/// Quem carrega. A chave estrangeira exige que exista mesmo — e faz bem: um
/// autor inventado tornaria a autoria de uma versão uma decoração.
async fn pessoa(pool: &PgPool, ctx: &Contexto) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email)
         VALUES ($1, 'Quem carrega', $2) RETURNING id",
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
             (organisation_id, unit_id, workspace_id, file_id, title, classification)
         VALUES ($1, $2, $3, $4, 'Relatório', 'INTERNAL') RETURNING id",
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
