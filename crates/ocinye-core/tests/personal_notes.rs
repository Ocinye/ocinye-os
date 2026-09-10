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
    let lida = knowledge::get_personal_note(&pool, &dono, nota.id)
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
    let lista = knowledge::list_personal_notes(&pool, &ana, pagina)
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
    let final_ = knowledge::get_personal_note(&pool, &dono, nota.id)
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
