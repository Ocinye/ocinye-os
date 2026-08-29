//! Conjuntos de embeddings e recuperação semântica.
//!
//! > **O conteúdo institucional pode ser recuperado lexical e semanticamente
//! > por pessoas e agentes autorizados, através da versão exacta do ficheiro,
//! > sem que embeddings, índices, conteúdo recuperado ou modelos adquiram
//! > autoridade sobre o sistema.**

use ocinye_domain::Principal;

use sqlx::PgPool;
use uuid::Uuid;

/// Connect and migrate, or skip.
async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations must apply to the test database");

    // Antes da primeira escrita, e não depois: falhar depois de escrever
    // não é uma guarda, é um relatório de estragos.
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("a{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organisation")
}

async fn person(pool: &PgPool, organisation_id: Uuid, roles: &[&str]) -> Principal {
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
    .expect("person");

    for role in roles {
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
            .bind(person_id)
            .bind(*role)
            .execute(pool)
            .await
            .expect("role");
    }

    let record = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("query")
        .expect("person");

    ocinye_core::modules::identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

use ocinye_core::modules::files::embedding::{self, Estado};
use ocinye_core::modules::intelligence::embeddings::{
    DeterministicEmbeddings, EmbeddingIdentity, EmbeddingProvider as _, Locality,
};

/// Uma unidade, um ambiente, e a pertença de quem lá trabalha.
async fn ambiente(pool: &PgPool, organisation_id: Uuid, classificacao: &str) -> (Uuid, Uuid) {
    let sufixo = Uuid::new_v4().simple().to_string();
    let unit_id: Uuid = sqlx::query_scalar(
        "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(organisation_id)
    .bind(format!("U{}", &sufixo[..8]))
    .bind("Unidade")
    .fetch_one(pool)
    .await
    .expect("unidade");

    let workspace_id: Uuid = sqlx::query_scalar(
        "INSERT INTO research_workspaces
             (organisation_id, unit_id, code, title, kind, classification)
         VALUES ($1, $2, $3, 'Ambiente', 'idea', $4) RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(format!("W{}", &sufixo[..12]))
    .bind(classificacao)
    .fetch_one(pool)
    .await
    .expect("ambiente");

    (unit_id, workspace_id)
}

/// Relê o principal depois de a pertença mudar.
///
/// O `Principal` é um retrato tirado no momento em que foi construído. O
/// executor relê-o imediatamente antes de correr uma capacidade (ADR-0411);
/// uma chamada directa ao módulo não o faz, e por isso o teste tem de o fazer.
/// Sem isto, um teste falharia por o actor não ter a pertença que acabou de
/// receber — e a culpa parecia ser da autorização.
async fn relido(pool: &PgPool, principal: &Principal) -> Principal {
    let pessoa = ocinye_core::modules::identity::person_by_id(pool, principal.person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &pessoa)
        .await
        .expect("principal")
}

async fn pertence(pool: &PgPool, workspace_id: Uuid, principal: &Principal) {
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, person_id, role)
         VALUES ($1, $2, 'lead')",
    )
    .bind(workspace_id)
    .bind(principal.person_id)
    .execute(pool)
    .await
    .expect("pertença");
}

/// Um ficheiro com uma versão e conteúdo já extraído.
///
/// Escrito directamente, e não pelo caminho de carregamento: o que estas provas
/// medem é a exposição do conteúdo, e depender de armazenamento fá-las-ia
/// saltar em máquinas onde ele não existe — o que já custou caro neste
/// repositório.
async fn ficheiro_com_conteudo(
    pool: &PgPool,
    organisation_id: Uuid,
    unit_id: Uuid,
    workspace_id: Uuid,
    nome: &str,
    classificacao: &str,
    paginas: &[&str],
) -> (Uuid, Uuid) {
    let backend_id: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_backends (code, display_name, location_label, bucket)
         VALUES ($1, 'Prova', 'test', 'prova') RETURNING id",
    )
    .bind(format!("b{}", &Uuid::new_v4().simple().to_string()[..12]))
    .fetch_one(pool)
    .await
    .expect("backend");

    let object_id: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_objects
             (organisation_id, backend_id, object_key, original_filename,
              content_type, size_bytes, checksum_sha256, status, classification)
         VALUES ($1, $2, $3, $4, 'application/pdf', 10, $5, 'stored', $6) RETURNING id",
    )
    .bind(organisation_id)
    .bind(backend_id)
    .bind(format!("prova/{}", Uuid::new_v4()))
    .bind(nome)
    .bind(format!("{:064x}", Uuid::new_v4().as_u128()))
    .bind(classificacao)
    .fetch_one(pool)
    .await
    .expect("objecto");

    let file_id: Uuid = sqlx::query_scalar(
        "INSERT INTO files (organisation_id, unit_id, workspace_id, name, classification)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(organisation_id)
    .bind(unit_id)
    .bind(workspace_id)
    .bind(nome)
    .bind(classificacao)
    .fetch_one(pool)
    .await
    .expect("ficheiro");

    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO file_versions (file_id, sequence, storage_object_id)
         VALUES ($1, 1, $2) RETURNING id",
    )
    .bind(file_id)
    .bind(object_id)
    .fetch_one(pool)
    .await
    .expect("versão");

    let extraction_id: Uuid = sqlx::query_scalar(
        "INSERT INTO file_extractions
             (file_version_id, status, extractor_name, extractor_version,
              source_checksum_sha256, chunk_count, extracted_at)
         VALUES ($1, 'AVAILABLE', 'prova', '1', $2, $3, now()) RETURNING id",
    )
    .bind(version_id)
    .bind(format!("{:064x}", Uuid::new_v4().as_u128()))
    .bind(i32::try_from(paginas.len()).unwrap_or(0))
    .fetch_one(pool)
    .await
    .expect("extracção");

    for (indice, texto) in paginas.iter().enumerate() {
        sqlx::query(
            "INSERT INTO file_chunks (extraction_id, ordinal, text, locator)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(extraction_id)
        .bind(i32::try_from(indice).unwrap_or(0))
        .bind(*texto)
        .bind(serde_json::json!({ "page": indice + 1 }))
        .execute(pool)
        .await
        .expect("pedaço");
    }

    (file_id, version_id)
}

/// Corre o worker de embeddings, como o worker o corre.
async fn embeber(
    pool: &PgPool,
    provider: &dyn ocinye_core::modules::intelligence::embeddings::EmbeddingProvider,
    versao: Uuid,
) -> Option<Estado> {
    let mut tx = pool.begin().await.expect("tx");
    let estado = embedding::process(&mut tx, provider, versao)
        .await
        .expect("processamento");
    tx.commit().await.expect("commit");
    estado
}

// ── A identidade do modelo é a fronteira ────────────────────────────────

/// Dois modelos com a mesma dimensão continuam a ser dois modelos.
///
/// > **Compatibilidade semântica não é «o mesmo tamanho de vector».**
///
/// Este é o defeito que não dá erro: comparar espaços diferentes devolve
/// números que parecem distâncias, e a resposta errada não se distingue da
/// certa a olho.
#[tokio::test]
async fn conjuntos_de_modelos_diferentes_nunca_se_comparam() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let (unit_id, workspace_id) = ambiente(&pool, org, "INTERNAL").await;
    let actor = person(&pool, org, &["research_member"]).await;
    pertence(&pool, workspace_id, &actor).await;
    let actor = relido(&pool, &actor).await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let (_, versao) = ficheiro_com_conteudo(
        &pool,
        org,
        unit_id,
        workspace_id,
        "ensaio.pdf",
        "INTERNAL",
        &[&format!("coeficiente termoeletrico {frase}")],
    )
    .await;

    // Indexado com a revisão 1.
    let revisao_um = DeterministicEmbeddings::default();
    assert_eq!(
        embeber(&pool, &revisao_um, versao).await,
        Some(Estado::Available),
        "o conjunto não ficou disponível"
    );

    let pagina = ocinye_contracts::PageRequest::default();

    // Consultado com a revisão 1: encontra.
    let (com_a_mesma, _) = ocinye_core::modules::search::search_hybrid(
        &pool,
        &actor,
        &frase,
        None,
        pagina,
        Some(&revisao_um),
    )
    .await
    .expect("pesquisa com a mesma revisão");
    assert!(
        com_a_mesma.iter().any(|h| h.file_version_id == versao),
        "a mesma revisão não encontrou o que ela própria indexou"
    );

    // Consultado com a revisão 2 — mesma dimensão, outro modelo. O conjunto da
    // revisão 1 não é candidato, e a pesquisa semântica não devolve nada dele.
    let revisao_dois = DeterministicEmbeddings {
        revision: "2".to_owned(),
        ..DeterministicEmbeddings::default()
    };
    assert_eq!(
        revisao_um.identity().dimensions,
        revisao_dois.identity().dimensions,
        "o teste só prova alguma coisa se as dimensões coincidirem"
    );
    assert!(
        !revisao_um
            .identity()
            .compatible_with(&revisao_dois.identity()),
        "duas revisões diferentes foram consideradas compatíveis"
    );

    let candidatos = ocinye_core::modules::search::semantic_candidates(
        &pool,
        &actor,
        &frase,
        None,
        10,
        &revisao_dois,
    )
    .await
    .expect("consulta semântica");

    assert!(
        candidatos.is_empty(),
        "um conjunto de outra revisão foi comparado com esta consulta"
    );
}

/// Um provider que mente sobre a dimensão é recusado.
#[tokio::test]
async fn um_vector_da_dimensao_errada_nao_entra_no_conjunto() {
    use async_trait::async_trait;
    use ocinye_core::modules::intelligence::embeddings::{
        embed_checked, EmbeddingError, EmbeddingProvider, EmbeddingResult,
    };

    /// Declara 64 dimensões e devolve 8.
    struct Mentiroso;

    #[async_trait]
    impl EmbeddingProvider for Mentiroso {
        fn identity(&self) -> EmbeddingIdentity {
            EmbeddingIdentity {
                provider: "mentiroso".to_owned(),
                model: "not-a-model".to_owned(),
                revision: "1".to_owned(),
                dimensions: 64,
                locality: Locality::OcinyeControlled,
            }
        }
        fn max_batch(&self) -> usize {
            8
        }
        fn max_input_chars(&self) -> usize {
            1000
        }
        fn deadline(&self) -> std::time::Duration {
            std::time::Duration::from_secs(2)
        }
        async fn embed(&self, texts: &[String]) -> EmbeddingResult<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![0.1_f32; 8]).collect())
        }
    }

    let erro = embed_checked(&Mentiroso, &["texto".to_owned()])
        .await
        .expect_err("um provider que devolve a dimensão errada devia ser recusado");

    assert!(
        matches!(
            erro,
            EmbeddingError::WrongDimensions {
                expected: 64,
                actual: 8
            }
        ),
        "a recusa não diz qual foi a divergência: {erro}"
    );
}

/// Trocar de modelo cria outro conjunto e não corrompe o anterior.
#[tokio::test]
async fn trocar_de_modelo_nao_reescreve_o_conjunto_anterior() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let (unit_id, workspace_id) = ambiente(&pool, org, "INTERNAL").await;

    let (_, versao) = ficheiro_com_conteudo(
        &pool,
        org,
        unit_id,
        workspace_id,
        "ensaio.pdf",
        "INTERNAL",
        &["uma pagina de texto", "outra pagina de texto"],
    )
    .await;

    let um = DeterministicEmbeddings::default();
    let dois = DeterministicEmbeddings {
        revision: "2".to_owned(),
        ..DeterministicEmbeddings::default()
    };

    embeber(&pool, &um, versao).await;
    embeber(&pool, &dois, versao).await;

    let conjuntos: i64 =
        sqlx::query_scalar("SELECT count(*) FROM embedding_sets WHERE file_version_id = $1")
            .bind(versao)
            .fetch_one(&pool)
            .await
            .expect("contagem");
    assert_eq!(conjuntos, 2, "trocar de modelo devia criar outro conjunto");

    // Os dois continuam completos e com a sua própria proveniência.
    for provider in [&um, &dois] {
        let (estado, feitos, esperados) = embedding::status(&pool, versao, &provider.identity())
            .await
            .expect("estado")
            .expect("o conjunto existe");
        assert_eq!(
            estado,
            Estado::Available,
            "um dos conjuntos deixou de estar completo"
        );
        assert_eq!(feitos, esperados, "um conjunto ficou incompleto");
        assert!(feitos > 0);
    }
}

/// Reprocessar não duplica, e um conjunto a meio não é elegível.
#[tokio::test]
async fn reprocessar_nao_duplica_e_um_conjunto_incompleto_nao_responde() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let (unit_id, workspace_id) = ambiente(&pool, org, "INTERNAL").await;
    let actor = person(&pool, org, &["research_member"]).await;
    pertence(&pool, workspace_id, &actor).await;
    let actor = relido(&pool, &actor).await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let (_, versao) = ficheiro_com_conteudo(
        &pool,
        org,
        unit_id,
        workspace_id,
        "ensaio.pdf",
        "INTERNAL",
        &[&format!("primeira {frase}"), "segunda pagina"],
    )
    .await;

    let provider = DeterministicEmbeddings::default();
    embeber(&pool, &provider, versao).await;

    let contar = || async {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM chunk_embeddings ce
               JOIN embedding_sets es ON es.id = ce.embedding_set_id
              WHERE es.file_version_id = $1",
        )
        .bind(versao)
        .fetch_one(&pool)
        .await
        .expect("contagem");
        n
    };

    let primeiro = contar().await;
    assert_eq!(primeiro, 2, "dois pedaços deviam dar dois vectores");

    // O evento chega outra vez.
    assert_eq!(
        embeber(&pool, &provider, versao).await,
        None,
        "um conjunto completo foi reclamado outra vez"
    );
    assert_eq!(contar().await, primeiro, "reprocessar duplicou vectores");

    // Um conjunto que não está completo não responde.
    sqlx::query(
        "UPDATE embedding_sets
            SET status = 'PROCESSING', completed_at = NULL, embedded_chunks = 1
          WHERE file_version_id = $1",
    )
    .bind(versao)
    .execute(&pool)
    .await
    .expect("pôr o conjunto a meio");

    let candidatos = ocinye_core::modules::search::semantic_candidates(
        &pool, &actor, &frase, None, 10, &provider,
    )
    .await
    .expect("consulta semântica");

    assert!(
        candidatos.is_empty(),
        "um conjunto incompleto respondeu a uma pesquisa"
    );
}

// ── Soberania dos dados ─────────────────────────────────────────────────

/// Um provider externo não recebe conteúdo institucional.
///
/// > **Nenhum conteúdo institucional é enviado para um embedding provider
/// > externo sem autorização explícita de deployment.**
///
/// A pergunta é feita **antes** de o texto sair. Uma verificação a jusante
/// seria uma auditoria de uma coisa que já aconteceu.
#[tokio::test]
async fn um_provider_externo_nao_recebe_conteudo_interno() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;

    // Revisões diferentes, e não só localidades diferentes.
    //
    // `locality` **não** faz parte da identidade do conjunto, e isso é
    // deliberado: dois conjuntos do mesmo modelo são comparáveis venha ele de
    // onde vier, e fragmentar o espaço por localidade tornaria incomparável
    // aquilo que é comparável. A consequência é que o conjunto do primeiro
    // provider satisfaz o segundo — o que é a idempotência a funcionar, e não
    // um defeito. Para medir os dois caminhos é preciso que sejam dois
    // conjuntos.
    let externo = DeterministicEmbeddings {
        locality: Locality::External,
        revision: "externa".to_owned(),
        ..DeterministicEmbeddings::default()
    };
    let interno = DeterministicEmbeddings {
        revision: "interna".to_owned(),
        ..DeterministicEmbeddings::default()
    };

    for (classificacao, externo_pode) in [
        ("PUBLIC", true),
        ("INTERNAL", false),
        ("CONFIDENTIAL", false),
        ("RESTRICTED", false),
    ] {
        let (unit_id, workspace_id) = ambiente(&pool, org, classificacao).await;
        let (_, versao) = ficheiro_com_conteudo(
            &pool,
            org,
            unit_id,
            workspace_id,
            "ensaio.pdf",
            classificacao,
            &["uma pagina de texto"],
        )
        .await;

        let saiu = embeber(&pool, &externo, versao).await;
        assert_eq!(
            saiu.is_some(),
            externo_pode,
            "{classificacao}: o provider externo devia {}",
            if externo_pode {
                "aceitar"
            } else {
                "ser recusado"
            }
        );

        // E o provider da instituição segue o tecto normal: até CONFIDENTIAL.
        let dentro = embeber(&pool, &interno, versao).await;
        let interno_pode = classificacao != "RESTRICTED";
        assert_eq!(
            dentro.is_some(),
            interno_pode,
            "{classificacao}: o provider da instituição devia {}",
            if interno_pode {
                "aceitar"
            } else {
                "ser recusado"
            }
        );
    }
}

// ── Recuperação híbrida ─────────────────────────────────────────────────

/// A paráfrase encontra-se, e a pesquisa lexical sozinha não a encontrava.
///
/// # O controlo que torna isto uma prova
///
/// A pergunta **não contém** os termos do documento. A pesquisa lexical
/// devolve zero — e é isso que separa «o semântico funciona» de «os dois
/// devolveram o mesmo e eu não sei qual deles trabalhou».
#[tokio::test]
async fn a_hibrida_encontra_o_que_a_lexical_sozinha_nao_encontra() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let (unit_id, workspace_id) = ambiente(&pool, org, "INTERNAL").await;
    let actor = person(&pool, org, &["research_member"]).await;
    pertence(&pool, workspace_id, &actor).await;
    let actor = relido(&pool, &actor).await;

    // Duas palavras únicas por corrida, e a pergunta usa uma delas em conjunto
    // com palavras que o documento tem — mas nunca a frase exacta.
    let alfa = format!("alfa{}", Uuid::new_v4().simple());
    let beta = format!("beta{}", Uuid::new_v4().simple());

    let (_, versao) = ficheiro_com_conteudo(
        &pool,
        org,
        unit_id,
        workspace_id,
        "ensaio.pdf",
        "INTERNAL",
        &[&format!("{alfa} {beta} medicao registada no ensaio")],
    )
    .await;

    let provider = DeterministicEmbeddings::default();
    embeber(&pool, &provider, versao).await;

    // A pergunta usa só uma das duas marcas, mais uma palavra que não está no
    // documento. `websearch_to_tsquery` exige todos os termos, pelo que o
    // lexical não encontra nada.
    let pergunta = format!("{alfa} inexistentepalavra");
    let pagina = ocinye_contracts::PageRequest::default();

    let (lexicais, _) =
        ocinye_core::modules::search::search_bodies(&pool, &actor, &pergunta, None, pagina)
            .await
            .expect("pesquisa lexical");
    assert!(
        lexicais.is_empty(),
        "o controlo falhou: a pesquisa lexical já encontrava isto sozinha"
    );

    let (hibridos, _) = ocinye_core::modules::search::search_hybrid(
        &pool,
        &actor,
        &pergunta,
        None,
        pagina,
        Some(&provider),
    )
    .await
    .expect("pesquisa híbrida");

    assert!(
        hibridos.iter().any(|h| h.file_version_id == versao),
        "a híbrida não encontrou o que só o semântico podia encontrar"
    );
}

/// Sem provider, a híbrida é exactamente a lexical.
///
/// > **Semantic unavailable não degrada Search.**
#[tokio::test]
async fn sem_provider_a_hibrida_devolve_o_que_a_lexical_devolve() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let (unit_id, workspace_id) = ambiente(&pool, org, "INTERNAL").await;
    let actor = person(&pool, org, &["research_member"]).await;
    pertence(&pool, workspace_id, &actor).await;
    let actor = relido(&pool, &actor).await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let (_, versao) = ficheiro_com_conteudo(
        &pool,
        org,
        unit_id,
        workspace_id,
        "ensaio.pdf",
        "INTERNAL",
        &[&format!("medicao {frase}")],
    )
    .await;

    let pagina = ocinye_contracts::PageRequest::default();
    let (lexicais, total_lexical) =
        ocinye_core::modules::search::search_bodies(&pool, &actor, &frase, None, pagina)
            .await
            .expect("lexical");
    let (hibridos, total_hibrido) =
        ocinye_core::modules::search::search_hybrid(&pool, &actor, &frase, None, pagina, None)
            .await
            .expect("híbrida sem provider");

    assert_eq!(
        total_hibrido, total_lexical,
        "a contagem mudou sem provider"
    );
    assert_eq!(
        hibridos.len(),
        lexicais.len(),
        "a híbrida sem provider devolveu um número diferente de resultados"
    );
    assert!(
        hibridos.iter().any(|h| h.file_version_id == versao),
        "a pesquisa determinística deixou de encontrar o que sempre encontrou"
    );
}

/// A recuperação semântica não revela o que a autoridade recusa.
///
/// > **Authorization precedes observability.**
#[tokio::test]
async fn a_semantica_nao_revela_o_que_a_autoridade_recusa() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let (unit_id, workspace_id) = ambiente(&pool, org, "CONFIDENTIAL").await;

    let dentro = person(&pool, org, &["research_member"]).await;
    pertence(&pool, workspace_id, &dentro).await;
    let dentro = relido(&pool, &dentro).await;
    let fora = person(&pool, org, &["research_member"]).await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let (_, versao) = ficheiro_com_conteudo(
        &pool,
        org,
        unit_id,
        workspace_id,
        "dossier.pdf",
        "CONFIDENTIAL",
        &[&format!("conclusao {frase}")],
    )
    .await;

    let provider = DeterministicEmbeddings::default();
    embeber(&pool, &provider, versao).await;

    let de_dentro = ocinye_core::modules::search::semantic_candidates(
        &pool, &dentro, &frase, None, 10, &provider,
    )
    .await
    .expect("de dentro");
    assert!(
        de_dentro.iter().any(|h| h.file_version_id == versao),
        "quem pertence ao ambiente não encontrou o seu próprio ficheiro"
    );

    let de_fora = ocinye_core::modules::search::semantic_candidates(
        &pool, &fora, &frase, None, 10, &provider,
    )
    .await
    .expect("de fora");
    assert!(
        de_fora.is_empty(),
        "a recuperação semântica revelou um ficheiro que a autoridade recusa"
    );

    // Nem pela híbrida, nem na contagem.
    let pagina = ocinye_contracts::PageRequest::default();
    let (hibridos, total) = ocinye_core::modules::search::search_hybrid(
        &pool,
        &fora,
        &frase,
        None,
        pagina,
        Some(&provider),
    )
    .await
    .expect("híbrida de fora");
    assert!(
        hibridos.is_empty(),
        "a híbrida vazou o que a autoridade recusa"
    );
    assert_eq!(total, 0, "a contagem revelou o que a lista escondeu");
}

/// Os embeddings reconstroem-se, como tudo o resto abaixo da versão.
#[tokio::test]
async fn os_embeddings_reconstroem_se_a_partir_do_que_e_duravel() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let (unit_id, workspace_id) = ambiente(&pool, org, "INTERNAL").await;
    let actor = person(&pool, org, &["research_member"]).await;
    pertence(&pool, workspace_id, &actor).await;
    let actor = relido(&pool, &actor).await;

    let frase = format!("delta{}", Uuid::new_v4().simple());
    let (_, versao) = ficheiro_com_conteudo(
        &pool,
        org,
        unit_id,
        workspace_id,
        "ensaio.pdf",
        "INTERNAL",
        &[&format!("medicao {frase}")],
    )
    .await;

    let provider = DeterministicEmbeddings::default();
    embeber(&pool, &provider, versao).await;

    let antes = ocinye_core::modules::search::semantic_candidates(
        &pool, &actor, &frase, None, 10, &provider,
    )
    .await
    .expect("antes");
    assert!(
        !antes.is_empty(),
        "o conjunto não respondeu antes de ser apagado"
    );

    // O desastre.
    sqlx::query("DELETE FROM embedding_sets WHERE file_version_id = $1")
        .bind(versao)
        .execute(&pool)
        .await
        .expect("apagar os conjuntos");

    let vazio = ocinye_core::modules::search::semantic_candidates(
        &pool, &actor, &frase, None, 10, &provider,
    )
    .await
    .expect("com o conjunto apagado");
    assert!(
        vazio.is_empty(),
        "apagar o conjunto não o retirou da pesquisa; o teste não prova nada"
    );

    // A reconstrução, a partir da extracção — que continua lá.
    assert_eq!(
        embeber(&pool, &provider, versao).await,
        Some(Estado::Available),
        "a reconstrução não completou"
    );

    let depois = ocinye_core::modules::search::semantic_candidates(
        &pool, &actor, &frase, None, 10, &provider,
    )
    .await
    .expect("depois");
    assert_eq!(
        depois.first().map(|h| h.file_version_id),
        antes.first().map(|h| h.file_version_id),
        "a reconstrução não devolveu a mesma fonte"
    );
    assert_eq!(
        depois.first().map(|h| h.locator.clone()),
        antes.first().map(|h| h.locator.clone()),
        "a reconstrução pôs a frase noutro sítio"
    );
}
