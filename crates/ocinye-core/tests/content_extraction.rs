//! Extracção de conteúdo e pesquisa lexical do corpo.
//!
//! > **Um `FileVersion` guardado pode produzir uma representação textual
//! > derivada, reconstruível e ligada à versão exacta; essa representação torna
//! > o corpo pesquisável sem transformar o índice em autoridade e sem alterar a
//! > validade do ficheiro se o processamento falhar.**

use sqlx::PgPool;
use uuid::Uuid;

// ── Um PDF a sério ──────────────────────────────────────────────────────
//
// Escrito à mão, com as posições do `xref` calculadas, porque um PDF de
// mentira — `b"%PDF-1.4 texto"` — provaria que o extractor falha, e não que
// ele lê. As provas desta suite dependem de o leitor ler mesmo.

/// Um PDF de uma ou mais páginas, cada uma com o seu texto.
#[must_use]
pub fn pdf_com_paginas(paginas: &[&str]) -> Vec<u8> {
    let mut objectos: Vec<String> = Vec::new();

    // 1: catálogo. 2: árvore de páginas. Depois, por página, o objecto da
    // página e o seu fluxo de conteúdo. Por fim, a fonte.
    let primeira_pagina = 3;
    let ids_pagina: Vec<usize> = (0..paginas.len())
        .map(|i| primeira_pagina + i * 2)
        .collect();
    let id_fonte = primeira_pagina + paginas.len() * 2;

    objectos.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());

    let kids = ids_pagina
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    objectos.push(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {} >>",
        paginas.len()
    ));

    for (indice, texto) in paginas.iter().enumerate() {
        let id_conteudo = ids_pagina[indice] + 1;
        objectos.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Contents {id_conteudo} 0 R \
             /Resources << /Font << /F1 {id_fonte} 0 R >> >> >>"
        ));

        let escapado = texto
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");
        let fluxo = format!("BT /F1 12 Tf 72 700 Td ({escapado}) Tj ET");
        objectos.push(format!(
            "<< /Length {} >>\nstream\n{fluxo}\nendstream",
            fluxo.len()
        ));
    }

    objectos.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned());

    let mut saida = String::from("%PDF-1.4\n");
    let mut posicoes = Vec::with_capacity(objectos.len());
    for (indice, corpo) in objectos.iter().enumerate() {
        posicoes.push(saida.len());
        saida.push_str(&format!("{} 0 obj\n{corpo}\nendobj\n", indice + 1));
    }

    let inicio_xref = saida.len();
    saida.push_str(&format!("xref\n0 {}\n", objectos.len() + 1));
    saida.push_str("0000000000 65535 f \n");
    for posicao in &posicoes {
        saida.push_str(&format!("{posicao:010} 00000 n \n"));
    }
    saida.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{inicio_xref}\n%%EOF\n",
        objectos.len() + 1
    ));

    saida.into_bytes()
}

/// O leitor lê mesmo um PDF, e diz em que página estava cada frase.
///
/// Este teste não toca na base nem no armazenamento: é o extractor sozinho.
/// Existe porque tudo o resto desta suite assume que ele funciona, e uma
/// suposição por medir é o sítio onde uma prova se desfaz em silêncio.
#[test]
fn o_leitor_de_pdf_le_o_texto_e_sabe_a_pagina() {
    use ocinye_core::modules::files::extraction::{extrair, Leitura};

    let pdf = pdf_com_paginas(&[
        "coeficiente termoeletrico experimental delta-719",
        "segunda pagina com outra frase inteiramente diferente",
    ]);

    let Leitura::Texto { extractor, chunks } = extrair("application/pdf", &pdf) else {
        panic!("o leitor não conseguiu ler um PDF válido construído à mão");
    };

    assert_eq!(
        extractor.name, "pdf-extract",
        "o extractor não se identifica"
    );
    assert_eq!(chunks.len(), 2, "duas páginas deviam dar dois pedaços");

    assert!(
        chunks[0].text.contains("delta-719"),
        "a frase da primeira página não foi lida: {:?}",
        chunks[0].text
    );
    assert_eq!(
        chunks[0].locator,
        serde_json::json!({ "page": 1 }),
        "o localizador da primeira página está errado"
    );
    assert_eq!(
        chunks[1].locator,
        serde_json::json!({ "page": 2 }),
        "o localizador da segunda página está errado"
    );
}

/// Um PDF que não é um PDF não derruba o worker.
///
/// O leitor entra em pânico com documentos estranhos, e um pânico dentro do
/// worker levava consigo o lote inteiro de eventos. A falha é apanhada e
/// devolvida como estado.
#[test]
fn um_pdf_invalido_e_um_estado_e_nao_uma_queda() {
    use ocinye_core::modules::files::extraction::{extrair, Leitura};

    let lixo = b"%PDF-1.4 isto nao e um PDF, e nunca foi".to_vec();
    match extrair("application/pdf", &lixo) {
        Leitura::Falhou(razao) => assert!(!razao.is_empty(), "a falha não diz nada"),
        Leitura::Texto { chunks, .. } => assert!(
            chunks.is_empty(),
            "um PDF inválido produziu texto: {chunks:?}"
        ),
        Leitura::SemExtractor => panic!("um PDF devia ter extractor"),
    }
}

/// Um PNG guarda-se e não se extrai. Isso é estado normal, não falha.
#[test]
fn um_formato_sem_extractor_diz_que_nao_tem_extractor() {
    use ocinye_core::modules::files::extraction::{extrair, tem_extractor, Leitura};

    assert!(
        !tem_extractor("image/png"),
        "o PNG passou a ter extractor sem ninguém decidir"
    );
    assert!(
        matches!(extrair("image/png", b"\x89PNG"), Leitura::SemExtractor),
        "um PNG não devia produzir texto"
    );
}

// ── Ajudantes ───────────────────────────────────────────────────────────
//
// Iguais aos de `institutional_files.rs`, e repetidos aqui porque os binários
// de teste do cargo não partilham código sem um módulo comum. A tranca é a
// mesma chave: o backend por omissão é estado global do processo de teste.

const TRANCA_DO_REGISTO: i64 = 0x0000_C109_E570_9A6E;

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

struct Contexto {
    // Copiados inteiros do outro binário de teste para os ajudantes serem os
    // mesmos. Nem todos os campos são lidos aqui, e mantê-los evita que os dois
    // ajudantes divirjam por causa de uma limpeza local.
    #[allow(dead_code)]
    organisation_id: Uuid,
    #[allow(dead_code)]
    unit_id: Uuid,
    workspace_id: Uuid,
    #[allow(dead_code)]
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

// ── A cadeia inteira ────────────────────────────────────────────────────

use ocinye_core::modules::files::extraction::{self, Estado};

/// Corre o worker sobre a versão, como o worker corre.
async fn processar(
    pool: &PgPool,
    store: &ocinye_core::storage::ObjectStore,
    versao: Uuid,
) -> Estado {
    let mut tx = pool.begin().await.expect("tx");
    let estado = extraction::process(&mut tx, store, versao)
        .await
        .expect("processamento")
        .expect("havia trabalho por fazer");
    tx.commit().await.expect("commit");
    estado
}

async fn carregar_pdf(
    pool: &PgPool,
    quem: &ocinye_domain::Principal,
    store: &ocinye_core::storage::ObjectStore,
    ctx: &Contexto,
    nome: &str,
    paginas: &[&str],
) -> ocinye_core::modules::files::FileVersionRecord {
    criar_ficheiro(
        pool,
        quem,
        store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: nome.to_owned(),
            content_type: "application/pdf".to_owned(),
            data: pdf_com_paginas(paginas),
            classification: None,
        },
    )
    .await
}

/// Uma frase que só existe no corpo passa a encontrar-se.
///
/// # A prova
///
/// A frase não está no nome do ficheiro nem em descrição nenhuma. Antes de o
/// worker correr, a pesquisa do corpo devolve zero — e é isso que distingue
/// «pesquisar o corpo» de «pesquisar metadata com outro nome». Depois de
/// correr, devolve o ficheiro certo, o excerto certo e a página certa.
#[tokio::test]
async fn uma_frase_que_so_existe_no_corpo_torna_se_pesquisavel() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    let ctx = contexto(&pool).await;
    let quem = membro(&pool, &ctx, "lead").await;

    // Única por corrida: nunca passamos por causa de dados de outra.
    let frase = format!("delta{}", Uuid::new_v4().simple());
    let corpo = format!("coeficiente termoeletrico experimental {frase}");

    let versao = carregar_pdf(
        &pool,
        &quem,
        &store,
        &ctx,
        // O nome não contém a frase. Se contivesse, o teste passaria pela
        // pesquisa de títulos e não provava nada sobre o corpo.
        "ensaio-de-marco.pdf",
        &["primeira pagina sem nada de especial", &corpo],
    )
    .await;

    let pagina = ocinye_contracts::PageRequest::default();

    let (antes, total_antes) =
        ocinye_core::modules::search::search_bodies(&pool, &quem, &frase, None, pagina)
            .await
            .expect("pesquisa antes");
    assert!(
        antes.is_empty() && total_antes == 0,
        "a frase já era encontrável antes de o corpo ter sido lido"
    );

    let estado = processar(&pool, &store, versao.version_id).await;
    assert_eq!(
        estado,
        Estado::Available,
        "a extracção não ficou disponível"
    );

    let (depois, total_depois) =
        ocinye_core::modules::search::search_bodies(&pool, &quem, &frase, None, pagina)
            .await
            .expect("pesquisa depois");

    assert_eq!(total_depois, 1, "a frase devia estar num ficheiro e só num");
    let achado = depois.first().expect("um resultado");
    assert_eq!(
        achado.file_id, versao.file_id,
        "o resultado é de outro ficheiro"
    );
    assert_eq!(
        achado.file_version_id, versao.version_id,
        "o resultado não aponta para a versão que foi lida"
    );
    assert!(
        achado.excerpt.contains(&frase),
        "o excerto não mostra a frase encontrada: {}",
        achado.excerpt
    );
    assert_eq!(
        achado.locator,
        serde_json::json!({ "page": 2 }),
        "a citação aponta para a página errada"
    );
}

/// Restringir o ambiente esconde o corpo, sem reindexar nada.
///
/// > **A pesquisa pode usar um índice para descobrir candidatos, mas a
/// > visibilidade decide-se contra o estado autoritativo corrente.**
///
/// É a mesma reversão que fechou o portão de transição da pesquisa de títulos,
/// aplicada agora ao corpo. Os chunks não mudam; o que muda é quem os alcança.
#[tokio::test]
async fn restringir_o_ambiente_esconde_o_corpo_sem_reindexar() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    let ctx = contexto(&pool).await;
    let dentro = membro(&pool, &ctx, "lead").await;
    let fora = estranho(&pool, &ctx).await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let versao = carregar_pdf(
        &pool,
        &dentro,
        &store,
        &ctx,
        "relatorio.pdf",
        &[&format!("medicao registada {frase}")],
    )
    .await;
    processar(&pool, &store, versao.version_id).await;

    let pagina = ocinye_contracts::PageRequest::default();

    // Com o ambiente INTERNAL, quem não é do ambiente mas é da instituição
    // alcança-o — é o que INTERNAL quer dizer.
    let (vistos, total) =
        ocinye_core::modules::search::search_bodies(&pool, &fora, &frase, None, pagina)
            .await
            .expect("pesquisa com o ambiente interno");
    assert_eq!(total, 1, "um membro activo não alcançou um corpo INTERNAL");
    assert_eq!(vistos.len(), 1);

    // O ambiente passa a RESTRICTED. Nada é reindexado.
    sqlx::query("UPDATE research_workspaces SET classification = 'RESTRICTED' WHERE id = $1")
        .bind(ctx.workspace_id)
        .execute(&pool)
        .await
        .expect("restringir o ambiente");

    let chunks_intactos: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM file_chunks c
           JOIN file_extractions e ON e.id = c.extraction_id
          WHERE e.file_version_id = $1",
    )
    .bind(versao.version_id)
    .fetch_one(&pool)
    .await
    .expect("contagem de chunks");
    assert!(
        chunks_intactos > 0,
        "os chunks foram apagados; devia bastar a composição"
    );

    let (vistos, total) =
        ocinye_core::modules::search::search_bodies(&pool, &fora, &frase, None, pagina)
            .await
            .expect("pesquisa depois de restringir");
    assert!(
        vistos.is_empty(),
        "o corpo continuou a aparecer a quem o ambiente passou a excluir"
    );
    assert_eq!(total, 0, "a contagem revelou o que a lista escondeu");

    // E quem pertence ao ambiente continua a encontrá-lo.
    let (ainda, total_dentro) =
        ocinye_core::modules::search::search_bodies(&pool, &dentro, &frase, None, pagina)
            .await
            .expect("pesquisa de quem pertence");
    assert_eq!(
        total_dentro, 1,
        "quem pertence ao ambiente perdeu o resultado"
    );
    assert_eq!(ainda.len(), 1);
}

/// Correr o mesmo trabalho outra vez não duplica nada.
#[tokio::test]
async fn reprocessar_a_mesma_versao_nao_duplica_pedacos() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    let ctx = contexto(&pool).await;
    let quem = membro(&pool, &ctx, "lead").await;

    let versao = carregar_pdf(
        &pool,
        &quem,
        &store,
        &ctx,
        "a.pdf",
        &["uma pagina de texto"],
    )
    .await;
    processar(&pool, &store, versao.version_id).await;

    let contar = || async {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM file_chunks c
               JOIN file_extractions e ON e.id = c.extraction_id
              WHERE e.file_version_id = $1",
        )
        .bind(versao.version_id)
        .fetch_one(&pool)
        .await
        .expect("contagem");
        n
    };

    let primeiro = contar().await;
    assert!(primeiro > 0, "a primeira leitura não produziu pedaços");

    // O evento chega outra vez. `claim` devolve `None` porque já está lida.
    let mut tx = pool.begin().await.expect("tx");
    let repetido = extraction::process(&mut tx, &store, versao.version_id)
        .await
        .expect("reprocessamento");
    tx.commit().await.expect("commit");

    assert!(
        repetido.is_none(),
        "uma versão já lida foi reclamada outra vez"
    );
    assert_eq!(contar().await, primeiro, "reprocessar duplicou os pedaços");
}

/// Uma versão nova lê-se sozinha. A anterior fica exactamente como estava.
#[tokio::test]
async fn uma_versao_nova_nao_toca_na_extraccao_da_anterior() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    let ctx = contexto(&pool).await;
    let quem = membro(&pool, &ctx, "lead").await;
    let ids = ocinye_observability::CorrelationIds::generate();

    let so_na_v1 = format!("delta{}", Uuid::new_v4().simple());
    let so_na_v2 = format!("delta{}", Uuid::new_v4().simple());

    let v1 = carregar_pdf(
        &pool,
        &quem,
        &store,
        &ctx,
        "ensaio.pdf",
        &[&format!("primeira leitura {so_na_v1}")],
    )
    .await;
    processar(&pool, &store, v1.version_id).await;

    let mut tx = pool.begin().await.expect("tx");
    let v2 = ocinye_core::modules::files::upload_version(
        &mut tx,
        &quem,
        &ids,
        &store,
        "prova",
        v1.file_id,
        ocinye_core::modules::files::NewFile {
            filename: "ensaio.pdf".to_owned(),
            content_type: "application/pdf".to_owned(),
            data: pdf_com_paginas(&[&format!("segunda leitura {so_na_v2}")]),
            classification: None,
        },
    )
    .await
    .expect("segunda versão");
    tx.commit().await.expect("commit");
    processar(&pool, &store, v2.version_id).await;

    // A extracção da v1 continua a descrever a v1.
    let (estado_v1, chunks_v1) = extraction::status(&pool, v1.version_id)
        .await
        .expect("estado da v1")
        .expect("a v1 tem extracção");
    assert_eq!(
        estado_v1,
        Estado::Available,
        "a extracção da v1 mudou de estado"
    );
    assert!(chunks_v1 > 0, "os pedaços da v1 desapareceram");

    let pagina = ocinye_contracts::PageRequest::default();

    // A pesquisa institucional normal prefere a versão corrente.
    let (novos, _) =
        ocinye_core::modules::search::search_bodies(&pool, &quem, &so_na_v2, None, pagina)
            .await
            .expect("pesquisa da v2");
    assert_eq!(
        novos.len(),
        1,
        "a frase da versão corrente não foi encontrada"
    );
    assert_eq!(novos[0].sequence, 2, "o resultado não é da versão corrente");

    // A frase que só existia na v1 deixa de aparecer na pesquisa normal — a v1
    // não é a corrente. Mas os pedaços dela continuam guardados, e é isso que
    // permite que uma citação histórica continue a resolver.
    let (antigos, _) =
        ocinye_core::modules::search::search_bodies(&pool, &quem, &so_na_v1, None, pagina)
            .await
            .expect("pesquisa da v1");
    assert!(
        antigos.is_empty(),
        "a pesquisa normal devolveu uma versão que já não é a corrente"
    );

    let ainda_guardados: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM file_chunks c
           JOIN file_extractions e ON e.id = c.extraction_id
          WHERE e.file_version_id = $1 AND c.text LIKE $2",
    )
    .bind(v1.version_id)
    .bind(format!("%{so_na_v1}%"))
    .fetch_one(&pool)
    .await
    .expect("contagem da v1");
    assert_eq!(
        ainda_guardados, 1,
        "os pedaços da v1 foram substituídos pelos da v2"
    );
}

/// Uma extracção falhada não invalida o ficheiro.
///
/// A interface tem de poder dizer «Ficheiro guardado. Não foi possível tornar o
/// conteúdo pesquisável» — e não «o carregamento falhou», que seria mentira.
#[tokio::test]
async fn um_ficheiro_continua_valido_quando_a_extraccao_falha() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    let ctx = contexto(&pool).await;
    let quem = membro(&pool, &ctx, "lead").await;
    let ids = ocinye_observability::CorrelationIds::generate();

    let versao = criar_ficheiro(
        &pool,
        &quem,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "corrompido.pdf".to_owned(),
            content_type: "application/pdf".to_owned(),
            data: b"%PDF-1.4 isto nao e um PDF, e nunca foi".to_vec(),
            classification: None,
        },
    )
    .await;

    let estado = processar(&pool, &store, versao.version_id).await;
    assert!(
        matches!(estado, Estado::Failed | Estado::Unsupported),
        "um PDF corrompido devia ficar por ler, e ficou {estado:?}"
    );

    // O ficheiro continua a ser um ficheiro: lê-se e descarrega-se.
    let mut conn = pool.acquire().await.expect("ligação");
    let (ficheiro, _) = ocinye_core::modules::files::get(&mut conn, &quem, versao.file_id)
        .await
        .expect("o ficheiro deixou de ser legível porque a extracção falhou");
    assert_eq!(ficheiro.name, "corrompido.pdf");

    let mut tx = pool.begin().await.expect("tx");
    ocinye_core::modules::files::download_url(&mut tx, &quem, &ids, &store, versao.file_id)
        .await
        .expect("o ficheiro deixou de se poder descarregar porque a extracção falhou");
    tx.commit().await.expect("commit");
}

/// Um PNG guarda-se, não se extrai, e isso é um estado e não um erro.
#[tokio::test]
async fn um_formato_sem_extractor_fica_unsupported_e_nao_failed() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    let ctx = contexto(&pool).await;
    let quem = membro(&pool, &ctx, "lead").await;

    let versao = criar_ficheiro(
        &pool,
        &quem,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "montagem.png".to_owned(),
            content_type: "image/png".to_owned(),
            data: b"\x89PNG\r\n\x1a\n prova".to_vec(),
            classification: None,
        },
    )
    .await;

    let estado = processar(&pool, &store, versao.version_id).await;
    assert_eq!(
        estado,
        Estado::Unsupported,
        "um formato sem extractor devia ser UNSUPPORTED, e não uma falha"
    );
}

/// Extrair não é afirmar conhecimento.
///
/// Ler «a temperatura foi 82 °C» de um PDF produz texto pesquisável. Não produz
/// um resultado científico, nem um documento, nem um dataset, nem uma fonte.
#[tokio::test]
async fn extrair_conteudo_nao_cria_conhecimento_institucional() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    let ctx = contexto(&pool).await;
    let quem = membro(&pool, &ctx, "lead").await;

    let contar = || async {
        let linha: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM documents WHERE workspace_id = $1),
                    (SELECT count(*) FROM datasets   WHERE workspace_id = $1),
                    (SELECT count(*) FROM sources    WHERE workspace_id = $1),
                    (SELECT count(*) FROM results    WHERE workspace_id = $1)",
        )
        .bind(ctx.workspace_id)
        .fetch_one(&pool)
        .await
        .expect("contagens");
        linha
    };

    let antes = contar().await;

    let versao = carregar_pdf(
        &pool,
        &quem,
        &store,
        &ctx,
        "leituras.pdf",
        &["A temperatura foi 82 graus centigrados na terceira medicao."],
    )
    .await;
    let estado = processar(&pool, &store, versao.version_id).await;
    assert_eq!(estado, Estado::Available, "o corpo não foi lido");

    assert_eq!(
        contar().await,
        antes,
        "extrair conteúdo criou conhecimento que ninguém afirmou"
    );
}

/// A extracção reconstrói-se a partir do que é durável.
///
/// # Porque este teste existe
///
/// Porque o manifesto de continuidade classifica `file_extractions` e
/// `file_chunks` como **derivados reconstruíveis**, e essa classificação é a
/// razão pela qual um restore que não os traga passa por íntegro. Se não se
/// reconstruíssem, essa decisão seria uma perda de memória institucional
/// disfarçada de optimização.
///
/// A reconstrução não tem de produzir os mesmos identificadores: eles não fazem
/// parte do contrato. Tem de produzir o mesmo **significado observável** — a
/// mesma frase, encontrável, no mesmo sítio do documento.
#[tokio::test]
async fn a_extraccao_reconstroi_se_a_partir_dos_bytes_e_do_extractor() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    let ctx = contexto(&pool).await;
    let quem = membro(&pool, &ctx, "lead").await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let versao = carregar_pdf(
        &pool,
        &quem,
        &store,
        &ctx,
        "reconstruivel.pdf",
        &["pagina de abertura", &format!("medicao {frase}")],
    )
    .await;
    processar(&pool, &store, versao.version_id).await;

    let pagina = ocinye_contracts::PageRequest::default();
    let (antes, _) =
        ocinye_core::modules::search::search_bodies(&pool, &quem, &frase, None, pagina)
            .await
            .expect("pesquisa antes de apagar");
    let antes = antes
        .into_iter()
        .next()
        .expect("a frase devia ser encontrável");

    // O desastre: a projecção derivada desaparece inteira.
    sqlx::query("DELETE FROM file_extractions WHERE file_version_id = $1")
        .bind(versao.version_id)
        .execute(&pool)
        .await
        .expect("apagar a extracção");

    let (vazio, total_vazio) =
        ocinye_core::modules::search::search_bodies(&pool, &quem, &frase, None, pagina)
            .await
            .expect("pesquisa com o índice apagado");
    assert!(
        vazio.is_empty() && total_vazio == 0,
        "apagar a extracção não retirou a frase do índice; o teste não prova nada"
    );

    // A reconstrução: pede-se outra vez, e o worker corre.
    let mut tx = pool.begin().await.expect("tx");
    let ids = ocinye_observability::CorrelationIds::generate();
    extraction::queue(&mut tx, versao.version_id, &ids)
        .await
        .expect("voltar a pôr na fila");
    tx.commit().await.expect("commit");

    let estado = processar(&pool, &store, versao.version_id).await;
    assert_eq!(
        estado,
        Estado::Available,
        "a reconstrução não ficou disponível"
    );

    let (depois, total_depois) =
        ocinye_core::modules::search::search_bodies(&pool, &quem, &frase, None, pagina)
            .await
            .expect("pesquisa depois de reconstruir");
    assert_eq!(total_depois, 1, "a frase não voltou a ser encontrável");

    let depois = depois.into_iter().next().expect("um resultado");
    assert_eq!(
        depois.file_id, antes.file_id,
        "a reconstrução mudou de ficheiro"
    );
    assert_eq!(
        depois.file_version_id, antes.file_version_id,
        "a reconstrução mudou de versão"
    );
    assert_eq!(
        depois.locator, antes.locator,
        "a reconstrução pôs a frase noutra página"
    );
    assert!(
        depois.excerpt.contains(&frase),
        "o excerto reconstruído perdeu a frase"
    );
}

/// Conhecer o identificador de um chunk não contorna a autorização.
///
/// Os pedaços não são endereçáveis por si: não há operação que devolva um
/// chunk por identificador, e a única porta é a pesquisa — que compõe a
/// classificação contra o estado corrente. Este teste guarda essa ausência: se
/// alguém acrescentar uma leitura directa, terá de acrescentar autoridade com
/// ela.
#[tokio::test]
async fn os_pedacos_nao_sao_enderecaveis_por_identificador() {
    let Some(pool) = pool().await else { return };
    let Some(store) = test_store() else {
        eprintln!("saltado: OCINYE_TEST_STORAGE_ENDPOINT não está definida");
        return;
    };
    let ctx = contexto(&pool).await;
    let dentro = membro(&pool, &ctx, "lead").await;
    let fora = estranho(&pool, &ctx).await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let versao = criar_ficheiro(
        &pool,
        &dentro,
        &store,
        ctx.workspace_id,
        ocinye_core::modules::files::NewFile {
            filename: "restrito.pdf".to_owned(),
            content_type: "application/pdf".to_owned(),
            data: pdf_com_paginas(&[&format!("segredo {frase}")]),
            classification: Some(ocinye_contracts::Classification::Restricted),
        },
    )
    .await;
    processar(&pool, &store, versao.version_id).await;

    let pagina = ocinye_contracts::PageRequest::default();

    // A pesquisa não o revela a quem não alcança o ficheiro.
    let (visto, total) =
        ocinye_core::modules::search::search_bodies(&pool, &fora, &frase, None, pagina)
            .await
            .expect("pesquisa de quem está de fora");
    assert!(
        visto.is_empty() && total == 0,
        "o corpo de um ficheiro RESTRICTED vazou"
    );

    // E a superfície pública do módulo não tem por onde pedir um chunk.
    // Isto é uma afirmação sobre o que **não** existe, e é essa a garantia:
    // a única porta para o corpo é a pesquisa, e a pesquisa autoriza.
    let ficheiro_alcancavel = {
        let mut conn = pool.acquire().await.expect("ligação");
        ocinye_core::modules::files::get(&mut conn, &fora, versao.file_id)
            .await
            .is_ok()
    };
    assert!(
        !ficheiro_alcancavel,
        "quem não devia alcançar o ficheiro alcança-o, e o teste do corpo passou por acidente"
    );
}
