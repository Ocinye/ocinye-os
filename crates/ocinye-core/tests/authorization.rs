//! Integration tests of authorization against a real database.
//!
//! These exist because the read policy is enforced twice — once as a decision
//! over a loaded resource, once as a `WHERE` clause over rows — and the second
//! one can only be proved against PostgreSQL. `ocinye-domain` proves the two
//! *descriptions* agree; these prove the SQL reproduces the description.
//!
//! They test **denial**, not the happy path: a member of another unit, a
//! removed member, a direct object reference, `CONFIDENTIAL`, `RESTRICTED`,
//! search leakage and count leakage (briefing §95).
//!
//! # Running them
//!
//! ```text
//! OCINYE_TEST_DATABASE_URL=postgres://ocinye:...@localhost:5442/ocinye_test \
//!   cargo test -p ocinye-core --test authorization
//! ```
//!
//! # Not configured versus broken
//!
//! With `OCINYE_TEST_DATABASE_URL` unset, these skip: a fresh clone should not
//! need PostgreSQL to run `cargo test`.
//!
//! With it **set but unreachable, they fail.** That distinction matters: CI
//! always sets it, so CI can never quietly lose this coverage the way a plain
//! skip would allow.

use std::collections::{HashMap, HashSet};

use ocinye_contracts::{Classification, PageRequest, TechnicalRole, UnitRole, WorkspaceRole};
use ocinye_core::modules::{research, search};
use ocinye_domain::Principal;
use sqlx::PgPool;
use uuid::Uuid;

/// Connect to the test database.
///
/// Returns `None` only when no database was configured at all. A configured but
/// unreachable database is a failure, not a skip.
async fn pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("OCINYE_TEST_DATABASE_URL") else {
        eprintln!(
            "SKIPPED: OCINYE_TEST_DATABASE_URL is not set, so the authorization suite \
             did not run."
        );
        return None;
    };

    let pool = PgPool::connect(&url).await.unwrap_or_else(|error| {
        panic!(
            "OCINYE_TEST_DATABASE_URL is set but the database is unreachable: {error}\n\
             Refusing to skip: a configured database that cannot be reached is a failure."
        )
    });

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations must apply to the test database");

    Some(pool)
}

/// A fixture: one organisation, two units, one workspace per classification.
struct Fixture {
    organisation_id: Uuid,
    unit_a: Uuid,
    unit_b: Uuid,
    /// Workspace in unit A, one per classification.
    workspaces: HashMap<Classification, Uuid>,
    insider: Uuid,
    outsider: Uuid,
}

async fn seed(pool: &PgPool) -> Fixture {
    // Each run gets its own organisation, so runs do not collide.
    let suffix = Uuid::new_v4().simple().to_string();

    let organisation_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organisations (slug, name) VALUES ($1, 'Test') RETURNING id",
    )
    .bind(format!("test-{suffix}"))
    .fetch_one(pool)
    .await
    .expect("organisation");

    let person = |email: &str| {
        let email = format!("{email}-{suffix}@example.org");
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO people (organisation_id, email, full_name, status)
                 VALUES ($1, $2, 'Test Person', 'active') RETURNING id",
            )
            .bind(organisation_id)
            .bind(email)
            .fetch_one(&pool)
            .await
            .expect("person")
        }
    };

    let insider = person("insider").await;
    let outsider = person("outsider").await;

    let unit = |code: &str| {
        let code = format!("{}{}", code, &suffix[..4]).to_uppercase();
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO units (organisation_id, code, name) VALUES ($1, $2, 'Unit')
                 RETURNING id",
            )
            .bind(organisation_id)
            .bind(code)
            .fetch_one(&pool)
            .await
            .expect("unit")
        }
    };

    let unit_a = unit("A").await;
    let unit_b = unit("B").await;

    let mut workspaces = HashMap::new();
    for classification in Classification::all() {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO research_workspaces
                 (organisation_id, unit_id, code, title, kind, classification)
             VALUES ($1, $2, $3, $4, 'idea', $5) RETURNING id",
        )
        .bind(organisation_id)
        .bind(unit_a)
        .bind(format!("{}-{}", classification.as_str(), &suffix[..8]))
        .bind(format!("Workspace {classification}"))
        .bind(classification.as_str())
        .fetch_one(pool)
        .await
        .expect("workspace");

        // Every workspace is indexed, so search-leakage tests have something to
        // leak.
        sqlx::query(
            "INSERT INTO search_documents
                 (organisation_id, unit_id, workspace_id, entity_type, entity_id,
                  title, classification, search_vector)
             VALUES ($1, $2, $3, 'idea', $4, $5, $6, to_tsvector('simple', $5))",
        )
        .bind(organisation_id)
        .bind(unit_a)
        .bind(id)
        .bind(Uuid::new_v4())
        .bind(format!("ocinyetestmarker{suffix}"))
        .bind(classification.as_str())
        .execute(pool)
        .await
        .expect("index row");

        workspaces.insert(classification, id);
    }

    Fixture {
        organisation_id,
        unit_a,
        unit_b,
        workspaces,
        insider,
        outsider,
    }
}

fn principal(fixture: &Fixture, person_id: Uuid) -> Principal {
    Principal {
        subject: person_id.to_string(),
        person_id,
        organisation_id: fixture.organisation_id,
        display_name: "Test".to_owned(),
        is_active: true,
        roles: HashSet::new(),
        unit_roles: HashMap::new(),
        workspace_roles: HashMap::new(),
        grants: Vec::new(),
    }
}

#[tokio::test]
async fn a_member_of_another_unit_cannot_read_confidential_or_restricted() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;

    let mut stranger = principal(&fixture, fixture.outsider);
    stranger
        .unit_roles
        .insert(fixture.unit_b, UnitRole::Manager);

    for classification in [Classification::Confidential, Classification::Restricted] {
        let workspace_id = fixture.workspaces[&classification];
        let result = research::get_workspace(&pool, &stranger, workspace_id).await;
        assert!(
            result.is_err(),
            "a manager of another unit must not read {classification}"
        );
    }

    // INTERNAL remains readable: the shape of the institution is not a secret.
    let internal = fixture.workspaces[&Classification::Internal];
    assert!(research::get_workspace(&pool, &stranger, internal)
        .await
        .is_ok());
}

#[tokio::test]
async fn a_direct_object_reference_is_not_a_way_in() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;

    // The caller knows the exact identifier and has no membership at all.
    let stranger = principal(&fixture, fixture.outsider);
    let restricted = fixture.workspaces[&Classification::Restricted];

    let error = research::get_workspace(&pool, &stranger, restricted)
        .await
        .expect_err("knowing the id must not grant access");

    // The refusal is indistinguishable from absence, so it does not confirm
    // that the resource exists.
    assert_eq!(error.code(), ocinye_contracts::ErrorCode::NotFound);
}

#[tokio::test]
async fn no_administrative_role_alone_opens_restricted_material() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;
    let restricted = fixture.workspaces[&Classification::Restricted];

    for role in [
        TechnicalRole::PlatformAdmin,
        TechnicalRole::OrganisationAdmin,
    ] {
        let mut admin = principal(&fixture, fixture.outsider);
        admin.roles.insert(role);

        assert!(
            research::get_workspace(&pool, &admin, restricted)
                .await
                .is_err(),
            "{role:?} must not read RESTRICTED without membership"
        );

        // And it must not appear in a listing either.
        let (listed, total) = research::list_workspaces(
            &pool,
            &admin,
            research::WorkspaceQuery::default(),
            PageRequest::default(),
        )
        .await
        .expect("listing");
        assert!(!listed.iter().any(|workspace| workspace.id == restricted));
        assert_eq!(
            total,
            i64::try_from(listed.len()).unwrap(),
            "the total must count only authorised rows"
        );
    }
}

#[tokio::test]
async fn explicit_workspace_membership_is_what_opens_restricted_material() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;
    let restricted = fixture.workspaces[&Classification::Restricted];

    let mut member = principal(&fixture, fixture.insider);
    member
        .workspace_roles
        .insert(restricted, WorkspaceRole::Viewer);

    assert!(research::get_workspace(&pool, &member, restricted)
        .await
        .is_ok());

    let (listed, _) = research::list_workspaces(
        &pool,
        &member,
        research::WorkspaceQuery::default(),
        PageRequest::default(),
    )
    .await
    .expect("listing");
    assert!(listed.iter().any(|workspace| workspace.id == restricted));
}

#[tokio::test]
async fn a_revoked_member_loses_access_immediately() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;
    let confidential = fixture.workspaces[&Classification::Confidential];

    let mut member = principal(&fixture, fixture.insider);
    member.unit_roles.insert(fixture.unit_a, UnitRole::Member);
    assert!(research::get_workspace(&pool, &member, confidential)
        .await
        .is_ok());

    // Revocation is modelled as the principal no longer carrying the
    // membership, which is exactly what the next request would produce.
    member.unit_roles.remove(&fixture.unit_a);
    assert!(
        research::get_workspace(&pool, &member, confidential)
            .await
            .is_err(),
        "a revoked membership must not keep granting access"
    );
}

#[tokio::test]
async fn an_inactive_member_reads_nothing() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;

    let mut suspended = principal(&fixture, fixture.insider);
    suspended.is_active = false;
    suspended
        .unit_roles
        .insert(fixture.unit_a, UnitRole::Manager);

    for classification in Classification::all() {
        assert!(
            research::get_workspace(&pool, &suspended, fixture.workspaces[&classification])
                .await
                .is_err(),
            "a suspended member must not read {classification}"
        );
    }

    let (listed, total) = research::list_workspaces(
        &pool,
        &suspended,
        research::WorkspaceQuery::default(),
        PageRequest::default(),
    )
    .await
    .expect("listing");
    assert!(listed.is_empty());
    assert_eq!(total, 0);
}

#[tokio::test]
async fn search_does_not_leak_titles_counts_or_existence() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;

    let marker = sqlx::query_scalar::<_, String>(
        "SELECT title FROM search_documents WHERE workspace_id = $1",
    )
    .bind(fixture.workspaces[&Classification::Restricted])
    .fetch_one(&pool)
    .await
    .expect("marker");

    // A plain member with no membership searches the exact marker term.
    let stranger = principal(&fixture, fixture.outsider);
    let (hits, total) = search::search(
        &pool,
        &stranger,
        &marker,
        None,
        None,
        PageRequest::default(),
    )
    .await
    .expect("search");

    let restricted_or_confidential = hits
        .iter()
        .filter(|hit| hit.classification == "RESTRICTED" || hit.classification == "CONFIDENTIAL")
        .count();
    assert_eq!(
        restricted_or_confidential, 0,
        "search leaked classified rows"
    );

    // The count must match what was actually returned: a total that included
    // hidden rows would itself disclose their existence.
    assert_eq!(total, i64::try_from(hits.len()).unwrap());

    // A member of the workspace does see it, so the test is not passing merely
    // because nothing matched.
    let mut member = principal(&fixture, fixture.insider);
    member.workspace_roles.insert(
        fixture.workspaces[&Classification::Restricted],
        WorkspaceRole::Member,
    );
    let (member_hits, _) =
        search::search(&pool, &member, &marker, None, None, PageRequest::default())
            .await
            .expect("search");
    assert!(member_hits
        .iter()
        .any(|hit| hit.classification == "RESTRICTED"));
}

#[tokio::test]
async fn cross_organisation_reads_are_refused() {
    let Some(pool) = pool().await else { return };
    let first = seed(&pool).await;
    let second = seed(&pool).await;

    // A principal of one organisation, an identifier from another.
    let mut caller = principal(&first, first.insider);
    caller.roles.insert(TechnicalRole::PlatformAdmin);

    let foreign = second.workspaces[&Classification::Public];
    assert!(
        research::get_workspace(&pool, &caller, foreign)
            .await
            .is_err(),
        "a workspace of another organisation must not be readable"
    );
}

// ── A classificação do artefacto governa, não a do Research Workspace ──
//
// Um artefacto pode ser **mais restrito** do que o Research Workspace que o
// guarda: `effective_classification` toma a mais restritiva das duas, e
// reclassificar um workspace para baixo não toca no material que ele já contém.
//
// Quando isso acontece, listagem e acesso directo têm de dar a mesma resposta.
// Se a listagem esconde a linha e `GET /…/{id}` a devolve, a classificação
// deixou de ser uma fronteira e passou a ser uma sugestão da interface.

/// Um artefacto mais restrito do que o seu workspace, para cada tipo que tem
/// classificação própria.
struct StricterArtefacts {
    dataset: Uuid,
    note: Uuid,
    source: Uuid,
    document: Uuid,
    task: Uuid,
}

/// Semeia material `RESTRICTED` dentro do workspace `INTERNAL` da fixture.
async fn seed_stricter_than_workspace(pool: &PgPool, fixture: &Fixture) -> StricterArtefacts {
    let workspace = fixture.workspaces[&Classification::Internal];
    let suffix = Uuid::new_v4().simple().to_string();

    let dataset: Uuid = sqlx::query_scalar(
        "INSERT INTO datasets
             (organisation_id, unit_id, workspace_id, code, title, classification)
         VALUES ($1, $2, $3, $4, 'Dataset restrito', 'RESTRICTED') RETURNING id",
    )
    .bind(fixture.organisation_id)
    .bind(fixture.unit_a)
    .bind(workspace)
    .bind(format!("DS-{}", &suffix[..8]).to_uppercase())
    .fetch_one(pool)
    .await
    .expect("dataset");

    sqlx::query(
        "INSERT INTO dataset_versions (dataset_id, label, sequence, created_by_id)
         VALUES ($1, 'v1', 1, $2)",
    )
    .bind(dataset)
    .bind(fixture.insider)
    .execute(pool)
    .await
    .expect("dataset version");

    let note: Uuid = sqlx::query_scalar(
        "INSERT INTO notes
             (organisation_id, unit_id, workspace_id, title, body, classification)
         VALUES ($1, $2, $3, 'Nota restrita', 'corpo confidencial', 'RESTRICTED')
         RETURNING id",
    )
    .bind(fixture.organisation_id)
    .bind(fixture.unit_a)
    .bind(workspace)
    .fetch_one(pool)
    .await
    .expect("note");

    let source: Uuid = sqlx::query_scalar(
        "INSERT INTO sources
             (organisation_id, unit_id, workspace_id, title, abstract, classification)
         VALUES ($1, $2, $3, 'Fonte restrita', 'resumo restrito', 'RESTRICTED')
         RETURNING id",
    )
    .bind(fixture.organisation_id)
    .bind(fixture.unit_a)
    .bind(workspace)
    .fetch_one(pool)
    .await
    .expect("source");

    let backend: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_backends (code, kind, display_name, location_label, bucket)
         VALUES ($1, 's3_compatible', 'Test', 'test', 'test')
         ON CONFLICT (code) DO UPDATE SET updated_at = now()
         RETURNING id",
    )
    .bind(format!("test-{}", &suffix[..8]))
    .fetch_one(pool)
    .await
    .expect("storage backend");

    let object: Uuid = sqlx::query_scalar(
        "INSERT INTO storage_objects
             (organisation_id, unit_id, workspace_id, backend_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256, classification)
         VALUES ($1, $2, $3, $4, $5, 'f.pdf', 'application/pdf', 1, $6, 'RESTRICTED')
         RETURNING id",
    )
    .bind(fixture.organisation_id)
    .bind(fixture.unit_a)
    .bind(workspace)
    .bind(backend)
    .bind(format!("k/{suffix}"))
    .bind("0".repeat(64))
    .fetch_one(pool)
    .await
    .expect("storage object");

    let document: Uuid = sqlx::query_scalar(
        "INSERT INTO documents
             (organisation_id, unit_id, workspace_id, storage_object_id, title, classification)
         VALUES ($1, $2, $3, $4, 'Documento restrito', 'RESTRICTED') RETURNING id",
    )
    .bind(fixture.organisation_id)
    .bind(fixture.unit_a)
    .bind(workspace)
    .bind(object)
    .fetch_one(pool)
    .await
    .expect("document");

    let task: Uuid = sqlx::query_scalar(
        "INSERT INTO tasks
             (organisation_id, unit_id, workspace_id, title, classification)
         VALUES ($1, $2, $3, 'Tarefa restrita', 'RESTRICTED') RETURNING id",
    )
    .bind(fixture.organisation_id)
    .bind(fixture.unit_a)
    .bind(workspace)
    .fetch_one(pool)
    .await
    .expect("task");

    StricterArtefacts {
        dataset,
        note,
        source,
        document,
        task,
    }
}

#[tokio::test]
async fn an_artefact_stricter_than_its_workspace_is_not_reachable_by_identifier() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;
    let artefacts = seed_stricter_than_workspace(&pool, &fixture).await;

    // Um membro activo da instituição, sem pertença à unidade nem ao workspace.
    // O workspace é INTERNAL, por isso alcança-o; o material lá dentro é
    // RESTRICTED, por isso não deve alcançar nada dele.
    let stranger = principal(&fixture, fixture.outsider);
    let internal = fixture.workspaces[&Classification::Internal];
    assert!(
        research::get_workspace(&pool, &stranger, internal)
            .await
            .is_ok(),
        "o workspace INTERNAL tem de continuar legível, ou o teste não prova nada"
    );

    // O controlo: a listagem já esconde o dataset.
    let (listed, total) =
        ocinye_core::modules::data::list_datasets(&pool, &stranger, None, PageRequest::default())
            .await
            .expect("listagem de datasets");
    assert!(
        !listed.iter().any(|dataset| dataset.id == artefacts.dataset),
        "a listagem devolveu um dataset RESTRICTED a quem não pertence ao workspace"
    );
    assert_eq!(total, i64::try_from(listed.len()).unwrap());

    // E o acesso directo por identificador tem de dar a mesma resposta.
    assert!(
        ocinye_core::modules::data::get_dataset(&pool, &stranger, artefacts.dataset)
            .await
            .is_err(),
        "conhecer o identificador de um dataset RESTRICTED abriu-o"
    );
    assert!(
        ocinye_core::modules::data::list_versions(&pool, &stranger, artefacts.dataset)
            .await
            .is_err(),
        "as versões de um dataset RESTRICTED foram devolvidas"
    );
    assert!(
        ocinye_core::modules::knowledge::get_note(&pool, &stranger, artefacts.note)
            .await
            .is_err(),
        "uma nota RESTRICTED foi devolvida pelo identificador"
    );
    assert!(
        ocinye_core::modules::knowledge::get_source(&pool, &stranger, artefacts.source)
            .await
            .is_err(),
        "uma fonte RESTRICTED foi devolvida pelo identificador"
    );
    assert!(
        ocinye_core::modules::knowledge::get_document(&pool, &stranger, artefacts.document)
            .await
            .is_err(),
        "um documento RESTRICTED foi devolvido pelo identificador"
    );
    assert!(
        ocinye_core::modules::collaboration::get_task(&pool, &stranger, artefacts.task)
            .await
            .is_err(),
        "uma tarefa RESTRICTED foi devolvida pelo identificador"
    );
}

#[tokio::test]
async fn a_workspace_member_still_reaches_the_stricter_artefacts() {
    // A contraprova: a correcção fecha o acesso a quem não pertence, e **não**
    // fecha o trabalho de quem pertence.
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;
    let artefacts = seed_stricter_than_workspace(&pool, &fixture).await;

    let mut member = principal(&fixture, fixture.insider);
    member.workspace_roles.insert(
        fixture.workspaces[&Classification::Internal],
        WorkspaceRole::Member,
    );

    assert!(
        ocinye_core::modules::data::get_dataset(&pool, &member, artefacts.dataset)
            .await
            .is_ok(),
        "quem pertence ao workspace deixou de alcançar o seu próprio material"
    );
    assert!(
        ocinye_core::modules::knowledge::get_note(&pool, &member, artefacts.note)
            .await
            .is_ok()
    );
    assert!(
        ocinye_core::modules::knowledge::get_source(&pool, &member, artefacts.source)
            .await
            .is_ok()
    );
    assert!(
        ocinye_core::modules::knowledge::get_document(&pool, &member, artefacts.document)
            .await
            .is_ok()
    );
    assert!(
        ocinye_core::modules::collaboration::get_task(&pool, &member, artefacts.task)
            .await
            .is_ok()
    );
}

// ── A trilha de auditoria não é reescrita pela aplicação ────────────────

/// `UPDATE`, `DELETE` e `TRUNCATE` são todos recusados em `audit_events`.
///
/// # Porque este teste existe
///
/// A migration 0001 instalou triggers `FOR EACH ROW` para `UPDATE` e `DELETE`,
/// e a documentação passou a chamar à tabela append-only. `TRUNCATE` não
/// percorre linhas: nenhum trigger de linha corria, e a tabela esvaziava-se
/// sem uma objecção. Quem escrevesse na base podia apagar a prova de o ter
/// feito.
///
/// Cada tentativa corre dentro de uma transacção que é revertida, para que um
/// teste que falhe não leve a trilha de auditoria com ele.
#[tokio::test]
async fn the_audit_trail_cannot_be_rewritten_by_the_application() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;

    let event: Uuid = sqlx::query_scalar(
        "INSERT INTO audit_events (organisation_id, actor_person_id, action, resource_type)
         VALUES ($1, $2, 'test.event', 'person') RETURNING id",
    )
    .bind(fixture.organisation_id)
    .bind(fixture.insider)
    .fetch_one(&pool)
    .await
    .expect("audit row");

    // Alterar o desfecho de um registo: o que alguém faria para transformar
    // uma recusa registada num sucesso.
    let mut tx = pool.begin().await.expect("tx");
    let updated = sqlx::query("UPDATE audit_events SET outcome = 'success' WHERE id = $1")
        .bind(event)
        .execute(&mut *tx)
        .await;
    assert!(
        updated.is_err(),
        "um registo de auditoria foi alterado pela aplicação"
    );
    tx.rollback().await.ok();

    let mut tx = pool.begin().await.expect("tx");
    let deleted = sqlx::query("DELETE FROM audit_events WHERE id = $1")
        .bind(event)
        .execute(&mut *tx)
        .await;
    assert!(deleted.is_err(), "um registo de auditoria foi apagado");
    tx.rollback().await.ok();

    // E o comando que não percorre linhas.
    let mut tx = pool.begin().await.expect("tx");
    let truncated = sqlx::query("TRUNCATE audit_events").execute(&mut *tx).await;
    assert!(
        truncated.is_err(),
        "a trilha de auditoria foi esvaziada por TRUNCATE"
    );
    tx.rollback().await.ok();

    // E continua lá.
    let survives: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(survives, 1, "o registo não sobreviveu às tentativas");
}

// ── Ler não é processar ─────────────────────────────────────────────────

/// A pré-visualização de contexto não mostra material que nunca iria a um
/// modelo.
///
/// # Porque este teste existe
///
/// `GET /ai/context-preview` existe para tornar a fronteira de recuperação
/// inspeccionável. Aplicava a política de leitura do próprio actor e o tecto
/// declarado do modelo, e **não** o tecto institucional de processamento por
/// IA — o `may_process_with_ai` que o Context Engine agentic sempre aplicou.
///
/// Com zero nós Ocinye, nenhum modelo corre em hardware da instituição, e o
/// tecto é `INTERNAL`. Um membro que peça `max_classification=RESTRICTED` via
/// a mostrar material que nunca seria enviado — uma pré-visualização que
/// descreve outra coisa que não o sistema.
#[tokio::test]
async fn the_context_preview_never_shows_more_than_inference_would_receive() {
    let Some(pool) = pool().await else { return };
    let fixture = seed(&pool).await;

    // Um membro do workspace RESTRICTED: pode ler o material, e é isso que
    // torna o teste sobre processamento e não sobre leitura.
    let restricted = fixture.workspaces[&Classification::Restricted];
    let mut member = principal(&fixture, fixture.insider);
    member
        .workspace_roles
        .insert(restricted, WorkspaceRole::Lead);

    let marker = sqlx::query_scalar::<_, String>(
        "SELECT title FROM search_documents WHERE workspace_id = $1",
    )
    .bind(restricted)
    .fetch_one(&pool)
    .await
    .expect("marker");

    // A contraprova: a pesquisa devolve-lho, porque pode lê-lo.
    let (hits, _) = search::search(&pool, &member, &marker, None, None, PageRequest::default())
        .await
        .expect("search");
    assert!(
        hits.iter().any(|hit| hit.classification == "RESTRICTED"),
        "o membro devia poder pesquisar o seu próprio material RESTRICTED"
    );

    // E o contexto que iria a um modelo não o inclui, por mais alto que seja o
    // tecto pedido.
    let refs = ocinye_core::modules::intelligence::assemble_context(
        &pool,
        &member,
        &marker,
        ocinye_contracts::RagScope::Institutional,
        None,
        Classification::Restricted,
    )
    .await
    .expect("context");

    assert!(
        !refs
            .iter()
            .any(|reference| reference.classification == "RESTRICTED"
                || reference.classification == "CONFIDENTIAL"),
        "a pré-visualização mostrou material acima do tecto de processamento por IA: {refs:?}"
    );
}
