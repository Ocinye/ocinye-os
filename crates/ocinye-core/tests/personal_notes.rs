//! Uma nota pessoal é de quem a escreveu, e mais de ninguém.
//!
//! # O que esta suite fixa
//!
//! As notas pessoais (ADR-0413) são o primeiro objecto do Ocinye cujo dono é uma
//! **pessoa**, e não um ambiente de investigação. Isso muda a pergunta de
//! autorização: não é «este membro alcança este workspace?», é «esta nota é
//! desta pessoa?». Estas provas percorrem essa pergunta pela porta errada —
//! outro membro, um `PlatformAdmin`, uma revisão base obsoleta, um documento
//! hostil — e exigem recusa em todas.
//!
//! > **Um identificador nomeia âmbito; nunca o concede** (`CLAUDE.md` §34.2).
//!
//! # Porque «não encontrado» e não «sem acesso»
//!
//! Porque dizer «existe, mas não é sua» confirma que a nota existe, e a
//! existência já é informação ([ADR-0100]). O Core responde a um identificador
//! de outra pessoa o mesmo que responderia a um inventado.
//!
//! [ADR-0100]: ../../../docs/adrs/0100-authorization-model.md

use ocinye_contracts::{PageRequest, TechnicalRole};
use ocinye_core::error::CoreError;
use ocinye_core::modules::knowledge;
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        // Em CI a ausência da base não pode virar verde por skip: um teste que se
        // ignora a si próprio passa, e `cargo test` esconde a saída de quem passa.
        assert!(
            std::env::var("CI").is_err(),
            "OCINYE_TEST_DATABASE_URL em falta em CI: as notas pessoais ficariam \
             por verificar e a suite reportaria verde"
        );
        return None;
    };
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL está definida mas a base não responde");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("n{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organização")
}

async fn person(pool: &PgPool, organisation_id: Uuid, roles: &[TechnicalRole]) -> Principal {
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

    for role in roles {
        sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, $2)")
            .bind(person_id)
            .bind(role.as_str())
            .execute(pool)
            .await
            .expect("papel");
    }

    let pessoa = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("consulta")
        .expect("pessoa");
    ocinye_core::modules::identity::principal_for_person(pool, &pessoa)
        .await
        .expect("principal")
}

/// Um documento estruturado válido com um parágrafo de texto.
fn doc(texto: &str) -> Value {
    json!({
        "schema_version": 1,
        "blocks": [{ "type": "paragraph", "content": [{ "type": "text", "text": texto }] }]
    })
}

/// Uma edição de nota, sem etiquetas.
fn edita(base: i32, titulo: &str, documento: Value) -> knowledge::PersonalNoteEdit {
    knowledge::PersonalNoteEdit {
        base_revision: base,
        title: titulo.to_owned(),
        document: documento,
        tags: None,
    }
}

/// Cria uma nota do `dono` e devolve-a.
async fn nova_nota(
    pool: &PgPool,
    dono: &Principal,
    ids: &CorrelationIds,
    titulo: &str,
    texto: &str,
) -> knowledge::Note {
    let mut tx = pool.begin().await.expect("tx");
    let nota = knowledge::create_personal_note(&mut tx, dono, ids, titulo, doc(texto))
        .await
        .expect("cria a nota");
    tx.commit().await.expect("commit");
    nota
}

/// Recusa que não revela: `NotFound`, e não `PermissionDenied`.
#[track_caller]
fn recusa_muda<T: std::fmt::Debug>(resultado: Result<T, CoreError>, operacao: &str) {
    match resultado {
        Err(CoreError::NotFound(_)) => {}
        Err(CoreError::PermissionDenied(_)) => panic!(
            "«{operacao}» recusou dizendo «sem acesso». Isso confirma que a nota \
             existe, e a existência já é informação (ADR-0100)."
        ),
        outro => panic!("«{operacao}» devia recusar com NotFound; veio {outro:?}"),
    }
}

/// O dono lê a sua nota, com o documento e a classificação por omissão.
#[tokio::test]
async fn o_dono_le_a_sua_nota() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "A minha nota", "Rede de sensores").await;
    let (lida, _acesso) = knowledge::get_personal_note(&pool, &dono, nota.id)
        .await
        .expect("o dono lê a sua nota");

    assert_eq!(
        lida.owner_id,
        Some(dono.person_id),
        "a nota não ficou do dono"
    );
    assert_eq!(
        lida.body, "Rede de sensores",
        "a projecção de texto não bate"
    );
    assert!(
        lida.document.is_some(),
        "o documento estruturado não viajou"
    );
    // Uma nota pessoal nasce INTERNAL: nem PUBLIC por descuido, nem indecisa.
    assert_eq!(
        lida.classification, "INTERNAL",
        "a classificação por omissão mudou"
    );
}

/// A listagem de um membro só traz as notas dele.
#[tokio::test]
async fn o_dono_so_lista_as_suas_notas() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let ana = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let rui = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let da_ana = nova_nota(&pool, &ana, &ids, "Da Ana", "só da Ana").await;
    let _do_rui = nova_nota(&pool, &rui, &ids, "Do Rui", "só do Rui").await;

    let pagina = PageRequest {
        page: 1,
        page_size: 50,
    };
    let lista = knowledge::list_personal_notes(&pool, &ana, None, None, pagina)
        .await
        .expect("lista");

    assert_eq!(
        lista.len(),
        1,
        "a lista da Ana trouxe notas que não são dela"
    );
    assert_eq!(lista[0].id, da_ana.id, "a nota listada não é a da Ana");
}

/// Outro membro, com o identificador na mão, não lê a nota de alguém.
#[tokio::test]
async fn outro_membro_nao_le_a_nota_de_alguem() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let outro = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Privada", "não é para os outros").await;

    recusa_muda(
        knowledge::get_personal_note(&pool, &outro, nota.id).await,
        "ler a nota de outro",
    );
}

/// Ser `PlatformAdmin` não é ser leitor de todas as notas.
///
/// A administração da plataforma governa contas, permissões e pertenças; não
/// abre a nota pessoal de ninguém. É a distinção do `CLAUDE.md` §34 entre título
/// e capacidade, aqui sobre o objecto mais pessoal que existe.
#[tokio::test]
async fn o_platform_admin_nao_le_uma_nota_privada() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let admin = person(&pool, org, &[TechnicalRole::PlatformAdmin]).await;
    assert!(
        admin.roles.contains(&TechnicalRole::PlatformAdmin),
        "o fixture não deu PlatformAdmin, e o teste não estaria a provar nada"
    );

    let nota = nova_nota(&pool, &dono, &ids, "Privada", "nem o admin lê").await;

    recusa_muda(
        knowledge::get_personal_note(&pool, &admin, nota.id).await,
        "o PlatformAdmin a ler uma nota privada",
    );
}

/// Uma revisão base obsoleta não sobrepõe: é conflito, não última-escrita.
#[tokio::test]
async fn uma_revisao_base_obsoleta_nao_sobrepoe() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Concorrida", "início").await;
    let base = nota.revision;

    // A primeira gravação, com a revisão base correcta, avança.
    let mut tx = pool.begin().await.expect("tx");
    let avancada = knowledge::update_personal_note(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        edita(base, "Concorrida", doc("primeira sessão")),
    )
    .await
    .expect("a primeira gravação avança");
    tx.commit().await.expect("commit");
    assert_ne!(avancada.revision, base, "a revisão não avançou");

    // A segunda, ainda com a revisão base antiga, encontra a nota já mudada.
    let mut tx = pool.begin().await.expect("tx");
    let conflito = knowledge::update_personal_note(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        edita(base, "Concorrida", doc("segunda sessão, à cega")),
    )
    .await;
    match conflito {
        Err(CoreError::Conflict(_)) => {}
        outro => panic!("uma revisão base obsoleta devia dar Conflict; veio {outro:?}"),
    }

    // E a nota ficou com o trabalho da primeira, não com o da segunda.
    let (final_, _acesso) = knowledge::get_personal_note(&pool, &dono, nota.id)
        .await
        .expect("lê");
    assert_eq!(
        final_.body, "primeira sessão",
        "a segunda gravação sobrepôs em silêncio"
    );
}

/// Editar deixa para trás uma revisão imutável da versão anterior.
#[tokio::test]
async fn uma_nota_editada_deixa_uma_revisao_imutavel() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Antes", "conteúdo original").await;
    let base = nota.revision;

    let mut tx = pool.begin().await.expect("tx");
    knowledge::update_personal_note(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        edita(base, "Depois", doc("conteúdo novo")),
    )
    .await
    .expect("edita");
    tx.commit().await.expect("commit");

    let revisoes = knowledge::personal_note_revisions(&pool, &dono, nota.id)
        .await
        .expect("história");

    assert!(!revisoes.is_empty(), "editar não deixou revisão nenhuma");
    let anterior = revisoes
        .iter()
        .find(|r| r.revision == base)
        .unwrap_or_else(|| panic!("a revisão {base} anterior não ficou na história: {revisoes:?}"));
    assert_eq!(
        anterior.title, "Antes",
        "a revisão histórica não guardou o título de então"
    );
}

/// Um documento hostil é recusado na gravação, mesmo que o editor o deixasse
/// passar: o Core é a fronteira de confiança, não o cliente.
#[tokio::test]
async fn um_documento_hostil_e_recusado_na_gravacao() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Alvo", "início").await;
    let hostil = json!({
        "schema_version": 1,
        "blocks": [{
            "type": "paragraph",
            "content": [{ "type": "link", "href": "javascript:alert(1)", "text": "carrega" }]
        }]
    });

    let mut tx = pool.begin().await.expect("tx");
    let resultado = knowledge::update_personal_note(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        edita(nota.revision, "Alvo", hostil),
    )
    .await;
    match resultado {
        Err(CoreError::Validation(_)) => {}
        outro => panic!("um documento com javascript: devia ser recusado; veio {outro:?}"),
    }
}

/// Semeia um ficheiro pessoal (imagem) do `owner`, por SQL, e devolve a versão.
///
/// Direto na base, sem armazenamento: a autoridade que estas provas exercem
/// decide-se **antes** de tocar nos bytes, pelo que não é preciso um MinIO para
/// as correr. O caminho completo de carregar e mostrar é a viagem de browser.
async fn seed_personal_image(pool: &PgPool, organisation_id: Uuid, owner_id: Uuid) -> Uuid {
    let sufixo = Uuid::new_v4().simple().to_string();
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

    let object_id: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_objects
             (backend_id, organisation_id, owner_id, object_key, original_filename,
              content_type, size_bytes, checksum_sha256, classification, status)
         VALUES ($1, $2, $3, $4, 'imagem.png', 'image/png', 10, $5, 'INTERNAL', 'stored')
         RETURNING id",
    )
    .bind(backend_id)
    .bind(organisation_id)
    .bind(owner_id)
    .bind(format!("prova/{}", Uuid::new_v4()))
    .bind("0".repeat(64))
    .fetch_one(pool)
    .await
    .expect("objecto");

    let file_id: Uuid = sqlx::query_scalar(
        "INSERT INTO files (organisation_id, owner_id, name, classification, created_by_id)
         VALUES ($1, $2, 'imagem.png', 'INTERNAL', $2) RETURNING id",
    )
    .bind(organisation_id)
    .bind(owner_id)
    .fetch_one(pool)
    .await
    .expect("ficheiro");

    sqlx::query_scalar(
        "INSERT INTO file_versions (file_id, sequence, storage_object_id, created_by_id)
         VALUES ($1, 1, $2, $3) RETURNING id",
    )
    .bind(file_id)
    .bind(object_id)
    .bind(owner_id)
    .fetch_one(pool)
    .await
    .expect("versão")
}

/// Um ficheiro pessoal é do dono, e conhecer o identificador da versão não o abre.
#[tokio::test]
async fn um_ficheiro_pessoal_e_do_dono_e_so_dele() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ana = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let rui = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let versao = seed_personal_image(&pool, org, ana.person_id).await;
    let mut conn = pool.acquire().await.expect("conn");

    assert!(
        ocinye_core::modules::files::owns_personal_file_version(&mut conn, &ana, versao)
            .await
            .expect("consulta"),
        "a dona não foi reconhecida como dona da sua imagem"
    );
    assert!(
        !ocinye_core::modules::files::owns_personal_file_version(&mut conn, &rui, versao)
            .await
            .expect("consulta"),
        "outro membro foi tratado como dono da imagem alheia"
    );
    assert!(
        !ocinye_core::modules::files::owns_personal_file_version(&mut conn, &ana, Uuid::new_v4())
            .await
            .expect("consulta"),
        "uma versão inventada foi dada como do dono"
    );
}

/// Uma nota não pode referenciar a imagem de outra pessoa.
#[tokio::test]
async fn uma_nota_nao_referencia_a_imagem_de_outra_pessoa() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let ana = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let rui = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let versao_da_ana = seed_personal_image(&pool, org, ana.person_id).await;
    let doc_com_imagem = json!({
        "schema_version": 1,
        "blocks": [{ "type": "image", "file_version_id": versao_da_ana, "alt": "emprestada" }]
    });

    // O Rui tenta pôr a imagem da Ana na nota dele: recusado à entrada.
    let nota_do_rui = nova_nota(&pool, &rui, &ids, "Do Rui", "início").await;
    let mut tx = pool.begin().await.expect("tx");
    let recusa = knowledge::update_personal_note(
        &mut tx,
        &rui,
        &ids,
        nota_do_rui.id,
        edita(nota_do_rui.revision, "Do Rui", doc_com_imagem.clone()),
    )
    .await;
    match recusa {
        Err(CoreError::Validation(_)) => {}
        outro => panic!("referenciar a imagem de outra pessoa devia recusar; veio {outro:?}"),
    }
    drop(tx);

    // A Ana pode referenciar a sua própria imagem.
    let nota_da_ana = nova_nota(&pool, &ana, &ids, "Da Ana", "início").await;
    let mut tx = pool.begin().await.expect("tx");
    knowledge::update_personal_note(
        &mut tx,
        &ana,
        &ids,
        nota_da_ana.id,
        edita(nota_da_ana.revision, "Da Ana", doc_com_imagem),
    )
    .await
    .expect("a dona pode referenciar a sua imagem");
    tx.commit().await.expect("commit");
}

/// Uma nota pessoal é pesquisável — mas só pelo dono.
///
/// A fatia A não indexava as notas pessoais porque faltava a dimensão do dono no
/// índice; agora existe (migração `0033`). O termo está no **corpo**, não no
/// título, por isso a pesquisa prova que o corpo foi indexado. E a nota de uma
/// pessoa não pode aparecer na pesquisa de outra, mesmo sendo `INTERNAL`.
#[tokio::test]
async fn uma_nota_pessoal_e_pesquisavel_so_pelo_dono() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let ana = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let rui = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let termo = format!("xenolito{}", &Uuid::new_v4().simple().to_string()[..8]);
    nova_nota(
        &pool,
        &ana,
        &ids,
        "Ideia solta",
        &format!("uma nota sobre {termo}"),
    )
    .await;

    let pagina = PageRequest {
        page: 1,
        page_size: 10,
    };

    let (hits_ana, total_ana) =
        ocinye_core::modules::search::search(&pool, &ana, &termo, None, None, pagina)
            .await
            .expect("pesquisa da Ana");
    assert_eq!(
        total_ana, 1,
        "a Ana não encontrou a sua própria nota pela pesquisa"
    );
    assert_eq!(hits_ana.len(), 1, "a Ana não encontrou a sua própria nota");

    let (hits_rui, total_rui) =
        ocinye_core::modules::search::search(&pool, &rui, &termo, None, None, pagina)
            .await
            .expect("pesquisa do Rui");
    assert_eq!(
        total_rui, 0,
        "a contagem da pesquisa do Rui revelou a nota da Ana"
    );
    assert!(
        hits_rui.is_empty(),
        "a nota INTERNAL da Ana vazou para a pesquisa do Rui"
    );
}

/// A lista de notas recorta-se por etiqueta.
#[tokio::test]
async fn a_lista_de_notas_filtra_por_etiqueta() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let ana = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let com_tag = nova_nota(&pool, &ana, &ids, "Com etiqueta", "corpo a").await;
    let _sem_tag = nova_nota(&pool, &ana, &ids, "Sem etiqueta", "corpo b").await;

    // Etiquetar a primeira — as etiquetas viajam na edição.
    let mut tx = pool.begin().await.expect("tx");
    knowledge::update_personal_note(
        &mut tx,
        &ana,
        &ids,
        com_tag.id,
        knowledge::PersonalNoteEdit {
            base_revision: com_tag.revision,
            title: "Com etiqueta".to_owned(),
            document: doc("corpo a"),
            tags: Some(vec!["projeto-x".to_owned()]),
        },
    )
    .await
    .expect("etiquetar");
    tx.commit().await.expect("commit");

    let pagina = PageRequest {
        page: 1,
        page_size: 50,
    };

    let filtradas = knowledge::list_personal_notes(&pool, &ana, Some("projeto-x"), None, pagina)
        .await
        .expect("lista filtrada");
    assert_eq!(filtradas.len(), 1, "o filtro por etiqueta não recortou");
    assert_eq!(
        filtradas[0].id, com_tag.id,
        "a nota filtrada não é a etiquetada"
    );

    let nenhuma = knowledge::list_personal_notes(&pool, &ana, Some("inexistente"), None, pagina)
        .await
        .expect("lista");
    assert!(
        nenhuma.is_empty(),
        "uma etiqueta que ninguém tem devolveu notas"
    );
}

/// Cria uma pasta pessoal do dono e devolve o seu id.
async fn nova_pasta(pool: &PgPool, dono: &Principal, ids: &CorrelationIds, nome: &str) -> Uuid {
    let mut tx = pool.begin().await.expect("tx");
    let pasta = ocinye_core::modules::files::create_personal_folder(&mut tx, dono, ids, nome)
        .await
        .expect("cria a pasta");
    tx.commit().await.expect("commit");
    pasta.id
}

async fn move_para(
    pool: &PgPool,
    dono: &Principal,
    ids: &CorrelationIds,
    nota: Uuid,
    pasta: Option<Uuid>,
) {
    let mut tx = pool.begin().await.expect("tx");
    knowledge::move_personal_note(&mut tx, dono, ids, nota, pasta)
        .await
        .expect("mover");
    tx.commit().await.expect("commit");
}

/// Uma nota arruma-se numa pasta, e a lista recorta-se por ela.
#[tokio::test]
async fn uma_nota_arruma_se_numa_pasta_e_a_lista_filtra() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let ana = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let pasta = nova_pasta(&pool, &ana, &ids, "Trabalho").await;
    let outra = nova_pasta(&pool, &ana, &ids, "Pessoal").await;
    let nota = nova_nota(&pool, &ana, &ids, "Um plano", "corpo").await;
    move_para(&pool, &ana, &ids, nota.id, Some(pasta)).await;

    let pagina = PageRequest {
        page: 1,
        page_size: 50,
    };
    let na_pasta = knowledge::list_personal_notes(&pool, &ana, None, Some(pasta), pagina)
        .await
        .expect("lista");
    assert_eq!(na_pasta.len(), 1, "a pasta não trouxe a nota arrumada");
    assert_eq!(na_pasta[0].id, nota.id);
    assert_eq!(
        na_pasta[0].folder_id,
        Some(pasta),
        "a nota não ficou na pasta"
    );

    let na_outra = knowledge::list_personal_notes(&pool, &ana, None, Some(outra), pagina)
        .await
        .expect("lista");
    assert!(
        na_outra.is_empty(),
        "a nota apareceu numa pasta onde não está"
    );
}

/// Uma nota não se arruma na pasta de outra pessoa.
#[tokio::test]
async fn uma_nota_nao_se_arruma_na_pasta_de_outra_pessoa() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let ana = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let rui = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let pasta_da_ana = nova_pasta(&pool, &ana, &ids, "Da Ana").await;
    let nota_do_rui = nova_nota(&pool, &rui, &ids, "Do Rui", "corpo").await;

    let mut tx = pool.begin().await.expect("tx");
    let recusa =
        knowledge::move_personal_note(&mut tx, &rui, &ids, nota_do_rui.id, Some(pasta_da_ana))
            .await;
    match recusa {
        Err(CoreError::Validation(_)) => {}
        outro => panic!("arrumar na pasta de outra pessoa devia recusar; veio {outro:?}"),
    }
}

/// Apagar uma pasta desarruma as notas, mas não as perde.
#[tokio::test]
async fn apagar_uma_pasta_desarruma_as_notas_mas_nao_as_perde() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let ana = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let pasta = nova_pasta(&pool, &ana, &ids, "Temporária").await;
    let nota = nova_nota(&pool, &ana, &ids, "Sobrevive", "corpo").await;
    move_para(&pool, &ana, &ids, nota.id, Some(pasta)).await;

    let mut tx = pool.begin().await.expect("tx");
    ocinye_core::modules::files::delete_personal_folder(&mut tx, &ana, &ids, pasta)
        .await
        .expect("apaga a pasta");
    tx.commit().await.expect("commit");

    // A nota sobrevive, agora sem pasta.
    let (lida, _acesso) = knowledge::get_personal_note(&pool, &ana, nota.id)
        .await
        .expect("a nota sobreviveu");
    assert_eq!(
        lida.folder_id, None,
        "a nota ficou presa a uma pasta apagada"
    );
}

// ── Fatia D — partilha ─────────────────────────────────────────────────────
//
// Partilhar uma nota é dar a outra pessoa uma janela para um objecto que
// continua a ser do dono. A autoridade de escrita de quem a recebe é
// reestabelecida a cada gravação, à fonte viva (ADR-0411): revogar fecha a
// janela na gravação seguinte, e não só na próxima sessão.

use ocinye_contracts::NoteShareRole;

/// Partilha `note_id` do `dono` com `pessoa`, no papel dado.
async fn partilha(
    pool: &PgPool,
    dono: &Principal,
    ids: &CorrelationIds,
    note_id: Uuid,
    pessoa: Uuid,
    papel: NoteShareRole,
) {
    let mut tx = pool.begin().await.expect("tx");
    knowledge::share_personal_note(&mut tx, dono, ids, note_id, pessoa, papel)
        .await
        .expect("a partilha do dono avança");
    tx.commit().await.expect("commit");
}

/// Quem recebe uma nota só para leitura não a grava.
///
/// O Core reestabelece o acesso dentro da transacção da gravação; um leitor não
/// escreve, e a recusa é explícita — «só para leitura» —, não um «não
/// encontrado», porque a partilha tornou a existência legítima para esta pessoa.
#[tokio::test]
async fn um_leitor_nao_edita_uma_nota_partilhada() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let leitor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Partilhada", "para ler").await;
    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        leitor.person_id,
        NoteShareRole::Viewer,
    )
    .await;

    // Lê — e o acesso resolvido diz «viewer».
    let (_lida, acesso) = knowledge::get_personal_note(&pool, &leitor, nota.id)
        .await
        .expect("o leitor lê a nota partilhada");
    assert_eq!(
        acesso.as_str(),
        "viewer",
        "o acesso não foi resolvido como leitura"
    );

    // Mas não grava.
    let mut tx = pool.begin().await.expect("tx");
    let escrita = knowledge::update_personal_note(
        &mut tx,
        &leitor,
        &ids,
        nota.id,
        edita(nota.revision, "Partilhada", doc("o leitor tentou escrever")),
    )
    .await;
    match escrita {
        Err(CoreError::PermissionDenied(_)) => {}
        outro => panic!("um leitor a gravar devia dar PermissionDenied; veio {outro:?}"),
    }
}

/// Quem recebe uma nota para edição grava-a — e a revisão avança.
#[tokio::test]
async fn um_editor_partilhado_grava_a_nota() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let editor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Colaborada", "início").await;
    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        editor.person_id,
        NoteShareRole::Editor,
    )
    .await;

    let (_lida, acesso) = knowledge::get_personal_note(&pool, &editor, nota.id)
        .await
        .expect("o editor lê a nota");
    assert_eq!(
        acesso.as_str(),
        "editor",
        "o acesso não foi resolvido como edição"
    );

    let mut tx = pool.begin().await.expect("tx");
    let avancada = knowledge::update_personal_note(
        &mut tx,
        &editor,
        &ids,
        nota.id,
        edita(nota.revision, "Colaborada", doc("o editor contribuiu")),
    )
    .await
    .expect("o editor partilhado grava");
    tx.commit().await.expect("commit");
    assert_ne!(
        avancada.revision, nota.revision,
        "a gravação do editor não avançou a revisão"
    );
}

/// Revogar fecha a janela na gravação seguinte — autoridade fresca (ADR-0411).
///
/// O editor grava enquanto tem o acesso; o dono revoga; a gravação seguinte do
/// mesmo editor é recusada, porque a autoridade se reestabelece à fonte viva
/// dentro da transacção, e não se herda de a sessão ter começado com acesso.
#[tokio::test]
async fn um_editor_revogado_deixa_de_gravar() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let editor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Revogável", "início").await;
    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        editor.person_id,
        NoteShareRole::Editor,
    )
    .await;

    // O editor grava uma vez, com acesso vivo.
    let mut tx = pool.begin().await.expect("tx");
    let primeira = knowledge::update_personal_note(
        &mut tx,
        &editor,
        &ids,
        nota.id,
        edita(nota.revision, "Revogável", doc("enquanto podia")),
    )
    .await
    .expect("o editor grava com acesso vivo");
    tx.commit().await.expect("commit");

    // O dono revoga.
    let mut tx = pool.begin().await.expect("tx");
    knowledge::revoke_personal_note_share(&mut tx, &dono, &ids, nota.id, editor.person_id)
        .await
        .expect("o dono revoga a partilha");
    tx.commit().await.expect("commit");

    // A gravação seguinte do mesmo editor é recusada — a janela fechou.
    let mut tx = pool.begin().await.expect("tx");
    let depois = knowledge::update_personal_note(
        &mut tx,
        &editor,
        &ids,
        nota.id,
        edita(primeira.revision, "Revogável", doc("já não pode")),
    )
    .await;
    match depois {
        Err(CoreError::NotFound(_)) => {}
        outro => panic!("um editor revogado a gravar devia dar NotFound; veio {outro:?}"),
    }
}

/// Só o dono partilha: quem recebeu não re-partilha.
#[tokio::test]
async fn partilhar_exige_ser_dono() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let editor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let terceiro = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Não re-partilhável", "corpo").await;
    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        editor.person_id,
        NoteShareRole::Editor,
    )
    .await;

    // O editor, que tem escrita, não tem partilha: re-partilhar é recusado, e
    // com «não encontrado» — a autoridade de partilha é só do dono.
    let mut tx = pool.begin().await.expect("tx");
    let tentativa = knowledge::share_personal_note(
        &mut tx,
        &editor,
        &ids,
        nota.id,
        terceiro.person_id,
        NoteShareRole::Viewer,
    )
    .await;
    match tentativa {
        Err(CoreError::NotFound(_)) => {}
        outro => panic!("um não-dono a partilhar devia dar NotFound; veio {outro:?}"),
    }
}

/// Uma partilha nunca atravessa a organização.
#[tokio::test]
async fn partilhar_nao_atravessa_organizacao() {
    let Some(pool) = pool().await else { return };
    let org_a = organisation(&pool).await;
    let org_b = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org_a, &[TechnicalRole::ResearchMember]).await;
    let estranho = person(&pool, org_b, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Só cá dentro", "corpo").await;

    let mut tx = pool.begin().await.expect("tx");
    let tentativa = knowledge::share_personal_note(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        estranho.person_id,
        NoteShareRole::Viewer,
    )
    .await;
    match tentativa {
        Err(CoreError::Validation(_)) => {}
        outro => panic!("partilhar com outra organização devia dar Validation; veio {outro:?}"),
    }
}

/// Uma nota partilhada expõe a sua imagem a quem a recebe, e a mais ninguém.
///
/// A imagem de uma nota é um ficheiro do **dono**. Partilhar a nota abre
/// exactamente as imagens que ela cita — a quem a recebeu — e nenhuma outra: um
/// membro qualquer, sem partilha, não vê a imagem (ADR-0413 §8).
#[tokio::test]
async fn uma_imagem_de_nota_partilhada_ve_se_pelo_destinatario() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let leitor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let estranho = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    // Uma imagem do dono, e uma nota do dono que a cita.
    let versao = seed_personal_image(&pool, org, dono.person_id).await;
    let documento = json!({
        "schema_version": 1,
        "blocks": [{ "type": "image", "file_version_id": versao, "alt": "diagrama" }]
    });
    let mut tx = pool.begin().await.expect("tx");
    let nota = knowledge::create_personal_note(&mut tx, &dono, &ids, "Com imagem", documento)
        .await
        .expect("cria a nota com imagem");
    tx.commit().await.expect("commit");

    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        leitor.person_id,
        NoteShareRole::Viewer,
    )
    .await;

    assert!(
        knowledge::member_can_view_note_file(&pool, &leitor, versao)
            .await
            .expect("consulta"),
        "quem recebeu a nota não vê a imagem que ela cita"
    );
    assert!(
        !knowledge::member_can_view_note_file(&pool, &estranho, versao)
            .await
            .expect("consulta"),
        "um membro sem partilha vê a imagem de uma nota alheia"
    );
}

/// A lista «partilhadas comigo» mostra o que é vivo, e esquece o revogado.
#[tokio::test]
async fn a_lista_partilhadas_comigo_segue_a_partilha_viva() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let leitor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Aparece e some", "corpo").await;
    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        leitor.person_id,
        NoteShareRole::Viewer,
    )
    .await;

    let vivas = knowledge::notes_shared_with_me(&pool, &leitor, PageRequest::default())
        .await
        .expect("lista");
    assert!(
        vivas.iter().any(|n| n.id == nota.id),
        "a nota partilhada não apareceu em «partilhadas comigo»"
    );

    let mut tx = pool.begin().await.expect("tx");
    knowledge::revoke_personal_note_share(&mut tx, &dono, &ids, nota.id, leitor.person_id)
        .await
        .expect("revoga");
    tx.commit().await.expect("commit");

    let depois = knowledge::notes_shared_with_me(&pool, &leitor, PageRequest::default())
        .await
        .expect("lista");
    assert!(
        !depois.iter().any(|n| n.id == nota.id),
        "uma nota revogada continuou em «partilhadas comigo»"
    );
}

// ── Fatia E — histórico e restauro ─────────────────────────────────────────
//
// Restaurar uma versão não apaga história: repõe o conteúdo antigo como uma
// revisão nova (ADR-0413 §6), e a autoridade de escrita reavalia-se na gravação
// (ADR-0411), como em qualquer edição.

/// Restaurar repõe o conteúdo de uma versão antiga — e a estrutura, não só o
/// texto. É a razão de a revisão guardar o documento, e não apenas o corpo.
#[tokio::test]
async fn restaurar_repoe_a_versao_com_a_sua_estrutura() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    // Uma primeira versão com um título de secção — estrutura, não só texto.
    let documento_rico = json!({
        "schema_version": 1,
        "blocks": [{ "type": "heading", "level": 1, "content": [{ "type": "text", "text": "Plano" }] }]
    });
    let mut tx = pool.begin().await.expect("tx");
    let nota =
        knowledge::create_personal_note(&mut tx, &dono, &ids, "Com estrutura", documento_rico)
            .await
            .expect("cria a nota");
    tx.commit().await.expect("commit");
    let rev_inicial = nota.revision;

    // Uma edição que achata para um parágrafo simples.
    let mut tx = pool.begin().await.expect("tx");
    let editada = knowledge::update_personal_note(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        edita(rev_inicial, "Com estrutura", doc("agora é só um parágrafo")),
    )
    .await
    .expect("edita");
    tx.commit().await.expect("commit");

    // Restaurar a versão inicial (que ficou no histórico ao ser editada).
    let mut tx = pool.begin().await.expect("tx");
    let restaurada = knowledge::restore_personal_note_revision(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        rev_inicial,
        editada.revision,
    )
    .await
    .expect("restaura a versão inicial");
    tx.commit().await.expect("commit");

    // A revisão avançou — restaurar é escrever, não recuar o contador.
    assert!(
        restaurada.revision > editada.revision,
        "restaurar não avançou a revisão"
    );
    // E o documento voltou a ter a estrutura da versão inicial.
    let doc_restaurado = restaurada.document.expect("documento restaurado");
    let blocos = doc_restaurado
        .get("blocks")
        .and_then(Value::as_array)
        .expect("blocos");
    assert_eq!(
        blocos
            .first()
            .and_then(|b| b.get("type"))
            .and_then(Value::as_str),
        Some("heading"),
        "a estrutura da versão inicial não voltou: {doc_restaurado}"
    );
}

/// Quem só tem leitura não restaura.
#[tokio::test]
async fn um_leitor_nao_restaura() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let leitor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Histórica", "início").await;
    let mut tx = pool.begin().await.expect("tx");
    let editada = knowledge::update_personal_note(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        edita(nota.revision, "Histórica", doc("segunda")),
    )
    .await
    .expect("edita");
    tx.commit().await.expect("commit");

    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        leitor.person_id,
        NoteShareRole::Viewer,
    )
    .await;

    let mut tx = pool.begin().await.expect("tx");
    let tentativa = knowledge::restore_personal_note_revision(
        &mut tx,
        &leitor,
        &ids,
        nota.id,
        nota.revision,
        editada.revision,
    )
    .await;
    match tentativa {
        Err(CoreError::PermissionDenied(_)) => {}
        outro => panic!("um leitor a restaurar devia dar PermissionDenied; veio {outro:?}"),
    }
}

/// Um editor revogado deixa de restaurar — autoridade fresca (ADR-0411).
#[tokio::test]
async fn um_editor_revogado_nao_restaura() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let editor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Histórica", "início").await;
    let mut tx = pool.begin().await.expect("tx");
    let editada = knowledge::update_personal_note(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        edita(nota.revision, "Histórica", doc("segunda")),
    )
    .await
    .expect("edita");
    tx.commit().await.expect("commit");

    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        editor.person_id,
        NoteShareRole::Editor,
    )
    .await;
    let mut tx = pool.begin().await.expect("tx");
    knowledge::revoke_personal_note_share(&mut tx, &dono, &ids, nota.id, editor.person_id)
        .await
        .expect("revoga");
    tx.commit().await.expect("commit");

    let mut tx = pool.begin().await.expect("tx");
    let tentativa = knowledge::restore_personal_note_revision(
        &mut tx,
        &editor,
        &ids,
        nota.id,
        nota.revision,
        editada.revision,
    )
    .await;
    match tentativa {
        Err(CoreError::NotFound(_)) => {}
        outro => panic!("um editor revogado a restaurar devia dar NotFound; veio {outro:?}"),
    }
}

/// O conteúdo de uma revisão não se lê por quem não alcança a nota.
#[tokio::test]
async fn o_conteudo_de_uma_revisao_nao_se_le_por_estranho() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let estranho = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Privada", "início").await;
    let mut tx = pool.begin().await.expect("tx");
    knowledge::update_personal_note(
        &mut tx,
        &dono,
        &ids,
        nota.id,
        edita(nota.revision, "Privada", doc("segunda")),
    )
    .await
    .expect("edita");
    tx.commit().await.expect("commit");

    recusa_muda(
        knowledge::personal_note_revision_content(&pool, &estranho, nota.id, nota.revision).await,
        "ler o conteúdo de uma revisão de outro",
    );
}

// ── Fatia E — lixo (soft delete) ───────────────────────────────────────────
//
// Apagar uma nota é reversível (ADR-0413 §7): sai das listas, das leituras e da
// pesquisa, mas espera no Lixo. Restaurar traz de volta; eliminar
// definitivamente — e só a partir do Lixo — é que destrói.

async fn apagar(pool: &PgPool, dono: &Principal, ids: &CorrelationIds, note_id: Uuid) {
    let mut tx = pool.begin().await.expect("tx");
    knowledge::delete_personal_note(&mut tx, dono, ids, note_id)
        .await
        .expect("apaga a nota");
    tx.commit().await.expect("commit");
}

fn pagina() -> PageRequest {
    PageRequest {
        page: 1,
        page_size: 50,
    }
}

/// Apagar leva ao Lixo: sai da lista, da leitura e da pesquisa; fica no Lixo.
#[tokio::test]
async fn apagar_uma_nota_leva_a_ao_lixo() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let termo = format!("basalto{}", &Uuid::new_v4().simple().to_string()[..8]);
    let nota = nova_nota(
        &pool,
        &dono,
        &ids,
        "Para apagar",
        &format!("nota sobre {termo}"),
    )
    .await;

    apagar(&pool, &dono, &ids, nota.id).await;

    // Sai da leitura normal.
    recusa_muda(
        knowledge::get_personal_note(&pool, &dono, nota.id).await,
        "ler uma nota apagada",
    );
    // Sai da lista de notas vivas.
    let vivas = knowledge::list_personal_notes(&pool, &dono, None, None, pagina())
        .await
        .expect("lista");
    assert!(
        !vivas.iter().any(|n| n.id == nota.id),
        "uma nota apagada continuou na lista de notas vivas"
    );
    // Sai da pesquisa.
    let (_hits, total) =
        ocinye_core::modules::search::search(&pool, &dono, &termo, None, None, pagina())
            .await
            .expect("pesquisa");
    assert_eq!(
        total, 0,
        "uma nota apagada continuou a aparecer na pesquisa"
    );
    // Mas está no Lixo.
    let lixo = knowledge::deleted_personal_notes(&pool, &dono, pagina())
        .await
        .expect("lixo");
    assert!(
        lixo.iter().any(|n| n.id == nota.id),
        "a nota apagada não apareceu no Lixo"
    );
}

/// Restaurar traz do Lixo e volta a indexar para a pesquisa.
#[tokio::test]
async fn restaurar_traz_do_lixo_e_reindexa() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let termo = format!("gnaisse{}", &Uuid::new_v4().simple().to_string()[..8]);
    let nota = nova_nota(
        &pool,
        &dono,
        &ids,
        "Vai e volta",
        &format!("nota sobre {termo}"),
    )
    .await;
    apagar(&pool, &dono, &ids, nota.id).await;

    let mut tx = pool.begin().await.expect("tx");
    knowledge::restore_personal_note(&mut tx, &dono, &ids, nota.id)
        .await
        .expect("restaura");
    tx.commit().await.expect("commit");

    // Volta a ler-se, e a revisão não avançou (restaurar do lixo não é editar).
    let (lida, _acesso) = knowledge::get_personal_note(&pool, &dono, nota.id)
        .await
        .expect("lê a nota restaurada");
    assert_eq!(
        lida.revision, nota.revision,
        "restaurar do lixo mexeu na revisão"
    );
    // Volta à pesquisa.
    let (_hits, total) =
        ocinye_core::modules::search::search(&pool, &dono, &termo, None, None, pagina())
            .await
            .expect("pesquisa");
    assert_eq!(total, 1, "a nota restaurada não voltou à pesquisa");
    // Sai do Lixo.
    let lixo = knowledge::deleted_personal_notes(&pool, &dono, pagina())
        .await
        .expect("lixo");
    assert!(
        !lixo.iter().any(|n| n.id == nota.id),
        "a nota restaurada continuou no Lixo"
    );
}

/// Eliminar definitivamente, a partir do Lixo, destrói de vez.
#[tokio::test]
async fn eliminar_definitivamente_destroi() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Adeus", "corpo").await;
    apagar(&pool, &dono, &ids, nota.id).await;

    let mut tx = pool.begin().await.expect("tx");
    knowledge::purge_personal_note(&mut tx, &dono, &ids, nota.id)
        .await
        .expect("elimina definitivamente");
    tx.commit().await.expect("commit");

    // Já não está no Lixo, e as revisões foram com ela.
    let lixo = knowledge::deleted_personal_notes(&pool, &dono, pagina())
        .await
        .expect("lixo");
    assert!(
        !lixo.iter().any(|n| n.id == nota.id),
        "uma nota eliminada definitivamente continuou no Lixo"
    );
    let revisoes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM note_revisions WHERE note_id = $1")
            .bind(nota.id)
            .fetch_one(&pool)
            .await
            .expect("consulta");
    assert_eq!(
        revisoes, 0,
        "as revisões sobreviveram à eliminação definitiva"
    );
}

/// Só o dono apaga: um editor partilhado não apaga a nota alheia.
#[tokio::test]
async fn so_o_dono_apaga_a_nota() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let editor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Não é para apagares", "corpo").await;
    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        editor.person_id,
        NoteShareRole::Editor,
    )
    .await;

    let mut tx = pool.begin().await.expect("tx");
    let tentativa = knowledge::delete_personal_note(&mut tx, &editor, &ids, nota.id).await;
    match tentativa {
        Err(CoreError::NotFound(_)) => {}
        outro => panic!("um editor a apagar a nota do dono devia dar NotFound; veio {outro:?}"),
    }
    drop(tx);

    // A nota continua viva para o dono.
    let vivas = knowledge::list_personal_notes(&pool, &dono, None, None, pagina())
        .await
        .expect("lista");
    assert!(
        vivas.iter().any(|n| n.id == nota.id),
        "a nota foi apagada por quem não é o dono"
    );
}

/// Eliminar definitivamente exige estar no Lixo: uma nota viva não se destrói
/// num passo só.
#[tokio::test]
async fn eliminar_exige_estar_no_lixo() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Viva", "corpo").await;
    let mut tx = pool.begin().await.expect("tx");
    let tentativa = knowledge::purge_personal_note(&mut tx, &dono, &ids, nota.id).await;
    match tentativa {
        Err(CoreError::Validation(_)) => {}
        outro => panic!("eliminar uma nota viva devia dar Validation; veio {outro:?}"),
    }
}

// ── Fatia E — actividade ───────────────────────────────────────────────────
//
// A actividade de uma nota regista o seu ciclo de vida e os acessos — criar,
// partilhar, revogar, apagar, restaurar. As edições vivem no histórico de
// revisões, ao lado. A actividade é do dono e lê-se por quem alcança a nota.

/// A actividade acompanha o ciclo de vida: criar, partilhar, apagar, restaurar.
#[tokio::test]
async fn a_actividade_de_uma_nota_acompanha_o_ciclo_de_vida() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let leitor = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Com actividade", "corpo").await;
    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        leitor.person_id,
        NoteShareRole::Viewer,
    )
    .await;
    apagar(&pool, &dono, &ids, nota.id).await;
    // Restaurar traz a nota de volta, e é ela que se lê agora.
    let mut tx = pool.begin().await.expect("tx");
    knowledge::restore_personal_note(&mut tx, &dono, &ids, nota.id)
        .await
        .expect("restaura");
    tx.commit().await.expect("commit");

    let feed = knowledge::note_activity(&pool, &dono, nota.id)
        .await
        .expect("actividade");
    let verbos: std::collections::HashSet<&str> = feed.iter().map(|e| e.kind.as_str()).collect();
    for esperado in ["created", "shared", "deleted", "restored"] {
        assert!(
            verbos.contains(esperado),
            "a actividade não registou «{esperado}»: {verbos:?}"
        );
    }
    // O mais recente primeiro: restaurar foi a última coisa que aconteceu.
    assert_eq!(
        feed.first().map(|e| e.kind.as_str()),
        Some("restored"),
        "a actividade não veio da mais recente para trás"
    );
    // E sabe quem fez — pelo nome, não por identificador.
    assert!(
        feed.iter().all(|e| e.actor_name.is_some()),
        "uma entrada de actividade ficou sem autor"
    );
}

/// A actividade de uma nota não se lê por quem não a alcança.
#[tokio::test]
async fn a_actividade_nao_se_le_por_estranho() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let estranho = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Privada", "corpo").await;

    recusa_muda(
        knowledge::note_activity(&pool, &estranho, nota.id).await,
        "ler a actividade de uma nota de outro",
    );
}

// ── Fatia E — tempo real (destinatários) ───────────────────────────────────
//
// Avisar «esta nota mudou noutro sítio» vai a quem mais a alcança — o dono e os
// destinatários vivos —, menos quem fez a mudança (ADR-0413 §9). A publicação em
// si é fogo-e-esquece; o que se prova aqui é **a quem** se avisaria.

/// Os destinatários de um aviso são o dono e os partilhados, menos o actor.
#[tokio::test]
async fn o_aviso_de_uma_nota_vai_a_quem_a_alcanca_menos_o_actor() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let ids = CorrelationIds::generate();
    let dono = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let a = person(&pool, org, &[TechnicalRole::ResearchMember]).await;
    let b = person(&pool, org, &[TechnicalRole::ResearchMember]).await;

    let nota = nova_nota(&pool, &dono, &ids, "Colaborada", "corpo").await;
    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        a.person_id,
        NoteShareRole::Viewer,
    )
    .await;
    partilha(
        &pool,
        &dono,
        &ids,
        nota.id,
        b.person_id,
        NoteShareRole::Editor,
    )
    .await;

    // O dono edita: avisam-se os dois destinatários, não o dono.
    let quando_dono = knowledge::note_notify_recipients(&pool, nota.id, dono.person_id)
        .await
        .expect("destinatários");
    let set_dono: std::collections::HashSet<_> = quando_dono.into_iter().collect();
    assert_eq!(
        set_dono,
        [a.person_id, b.person_id].into_iter().collect(),
        "o aviso do dono não foi exactamente para os dois destinatários"
    );

    // O editor B edita: avisam-se o dono e A, não o B.
    let quando_b = knowledge::note_notify_recipients(&pool, nota.id, b.person_id)
        .await
        .expect("destinatários");
    let set_b: std::collections::HashSet<_> = quando_b.into_iter().collect();
    assert_eq!(
        set_b,
        [dono.person_id, a.person_id].into_iter().collect(),
        "o aviso do editor não foi para o dono e o outro destinatário"
    );

    // Uma nota não partilhada, editada pelo dono: não se avisa ninguém.
    let so_dono = nova_nota(&pool, &dono, &ids, "Só minha", "corpo").await;
    let ninguem = knowledge::note_notify_recipients(&pool, so_dono.id, dono.person_id)
        .await
        .expect("destinatários");
    assert!(
        ninguem.is_empty(),
        "uma nota não partilhada avisou alguém: {ninguem:?}"
    );

    // E uma partilha revogada deixa de receber avisos.
    let mut tx = pool.begin().await.expect("tx");
    knowledge::revoke_personal_note_share(&mut tx, &dono, &ids, nota.id, a.person_id)
        .await
        .expect("revoga");
    tx.commit().await.expect("commit");
    let apos = knowledge::note_notify_recipients(&pool, nota.id, dono.person_id)
        .await
        .expect("destinatários");
    assert_eq!(
        apos,
        vec![b.person_id],
        "um destinatário revogado continuou a receber avisos"
    );
}
