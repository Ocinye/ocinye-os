//! Draft attachment lifecycle at the database boundary.
//!
//! These exercise the attachment repository and the ownership/cleanup logic
//! against a throwaway database, using storage objects inserted directly — the
//! full upload path (bytes through `guardar_bytes_personal` + object store) is
//! covered by the resource-storage suite and the browser journey. Without
//! `OCINYE_TEST_DATABASE_URL` each returns early.

use ocinye_core::modules::mail::{self, repository as repo};
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

const DOMAIN: &str = "ocinye.com";

async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url).await.expect("database unreachable");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

async fn backend(pool: &PgPool) {
    // Idempotent and race-tolerant: the default backend is a shared row, and
    // concurrent tests inserting the same key can raise a unique conflict (a
    // second inserter cannot see the first's uncommitted row). Insert, then
    // confirm the row is visible before returning, retrying briefly.
    for _ in 0..20 {
        let _ = sqlx::query(
            "INSERT INTO storage_backends
                 (code, kind, display_name, location_label, bucket, is_default, is_active)
             VALUES ('ocinye-test-default', 's3_compatible', 'Test', 'test', 'prova', TRUE, TRUE)
             ON CONFLICT (code) DO NOTHING",
        )
        .execute(pool)
        .await;
        let ready: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM storage_backends
              WHERE code = 'ocinye-test-default' AND is_default AND is_active",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        if ready > 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("o backend de armazenamento de teste não ficou disponível");
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("m{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("org")
}

async fn person(pool: &PgPool, organisation_id: Uuid) -> ocinye_domain::Principal {
    let handle = format!("p{}", Uuid::new_v4().simple());
    let person_id: Uuid = sqlx::query_scalar(
        "INSERT INTO people (organisation_id, full_name, email, status)
              VALUES ($1, $2, $3, 'active') RETURNING id",
    )
    .bind(organisation_id)
    .bind(&handle)
    .bind(format!("{handle}@{DOMAIN}"))
    .fetch_one(pool)
    .await
    .expect("person");
    sqlx::query("INSERT INTO person_roles (person_id, role) VALUES ($1, 'research_member')")
        .bind(person_id)
        .execute(pool)
        .await
        .expect("role");
    let record = ocinye_core::modules::identity::person_by_id(pool, person_id)
        .await
        .expect("query")
        .expect("person");
    ocinye_core::modules::identity::principal_for_person(pool, &record)
        .await
        .expect("principal")
}

async fn personal_mailbox(pool: &PgPool, organisation_id: Uuid, owner_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO mailboxes (organisation_id, address, kind, owner_id)
              VALUES ($1, $2, 'personal', $3) RETURNING id",
    )
    .bind(organisation_id)
    .bind(format!("mb{}@{DOMAIN}", Uuid::new_v4().simple()))
    .bind(owner_id)
    .fetch_one(pool)
    .await
    .expect("mailbox")
}

async fn draft(pool: &PgPool, mailbox_id: Uuid, author_id: Uuid) -> Uuid {
    repo::insert_composer_draft(
        pool,
        mailbox_id,
        author_id,
        &["dest@exemplo.com".to_owned()],
        &[],
        &[],
        Some("assunto"),
        "corpo",
        None,
        None,
    )
    .await
    .expect("draft")
}

async fn stored_object(pool: &PgPool, organisation_id: Uuid, owner_id: Uuid, size: i64) -> Uuid {
    let object_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO storage_objects
             (id, backend_id, organisation_id, owner_id, object_key,
              original_filename, content_type, size_bytes, checksum_sha256,
              classification, status, created_by_id)
         SELECT $1, b.id, $2, $3, $4, 'prova.pdf', 'application/pdf', $5,
                repeat('0', 64), 'INTERNAL', 'stored', $3
           FROM storage_backends b WHERE b.is_default AND b.is_active",
    )
    .bind(object_id)
    .bind(organisation_id)
    .bind(owner_id)
    .bind(format!("k/{object_id}"))
    .bind(size)
    .execute(pool)
    .await
    .expect("object");
    object_id
}

async fn object_exists(pool: &PgPool, id: Uuid) -> bool {
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM storage_objects WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("count");
    n > 0
}

/// An attachment persists on a draft and its size counts toward the totals.
#[tokio::test]
async fn attachments_persist_and_total() {
    let Some(pool) = pool().await else { return };
    backend(&pool).await;
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let draft_id = draft(&pool, mailbox, author.person_id).await;
    let object = stored_object(&pool, org, author.person_id, 2048).await;

    let att = repo::insert_draft_attachment(
        &pool,
        draft_id,
        "prova.pdf",
        "application/pdf",
        2048,
        &"0".repeat(64),
        object,
    )
    .await
    .expect("attach");

    let list = repo::list_draft_attachments(&pool, draft_id)
        .await
        .expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, att);
    assert_eq!(list[0].filename, "prova.pdf");

    let (count, total) = repo::draft_attachment_totals(&pool, draft_id)
        .await
        .expect("totals");
    assert_eq!(count, 1);
    assert_eq!(total, 2048);

    let objects = repo::draft_attachment_objects(&pool, draft_id)
        .await
        .expect("objects");
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].object_key, format!("k/{object}"));
}

/// Removing an attachment returns its object so the bytes can be freed.
#[tokio::test]
async fn removing_an_attachment_frees_its_object() {
    let Some(pool) = pool().await else { return };
    backend(&pool).await;
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let draft_id = draft(&pool, mailbox, author.person_id).await;
    let object = stored_object(&pool, org, author.person_id, 512).await;
    let att = repo::insert_draft_attachment(
        &pool,
        draft_id,
        "x.pdf",
        "application/pdf",
        512,
        &"0".repeat(64),
        object,
    )
    .await
    .expect("attach");

    let freed = repo::delete_draft_attachment(&pool, draft_id, att)
        .await
        .expect("delete");
    assert_eq!(freed, Some(object), "the removed row names its object");

    let key = repo::delete_storage_object(&pool, object)
        .await
        .expect("delete object");
    assert_eq!(key.as_deref(), Some(format!("k/{object}").as_str()));
    assert!(
        !object_exists(&pool, object).await,
        "the object row is gone"
    );
}

/// A draft's attachments are private to its author.
#[tokio::test]
async fn attachments_are_private_to_the_author() {
    let Some(pool) = pool().await else { return };
    backend(&pool).await;
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let intruder = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let draft_id = draft(&pool, mailbox, author.person_id).await;
    let object = stored_object(&pool, org, author.person_id, 64).await;
    repo::insert_draft_attachment(
        &pool,
        draft_id,
        "s.pdf",
        "application/pdf",
        64,
        &"0".repeat(64),
        object,
    )
    .await
    .expect("attach");

    assert!(
        mail::list_attachments(&pool, &intruder, draft_id)
            .await
            .is_err(),
        "IDOR: another member listed the draft's attachments"
    );
    assert!(
        mail::list_attachments(&pool, &author, draft_id)
            .await
            .is_ok(),
        "the author can list their own"
    );
}

/// Discarding a draft removes the objects its attachments referenced — no orphan.
#[tokio::test]
async fn discarding_a_draft_removes_its_attachment_objects() {
    let Some(pool) = pool().await else { return };
    backend(&pool).await;
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let draft_id = draft(&pool, mailbox, author.person_id).await;
    let object = stored_object(&pool, org, author.person_id, 128).await;
    repo::insert_draft_attachment(
        &pool,
        draft_id,
        "d.pdf",
        "application/pdf",
        128,
        &"0".repeat(64),
        object,
    )
    .await
    .expect("attach");

    // No object store here: the rows are removed regardless; only byte deletion
    // needs a store.
    mail::discard_draft(&pool, &author, None, &CorrelationIds::generate(), draft_id)
        .await
        .expect("discard");

    assert!(
        !object_exists(&pool, object).await,
        "a discarded draft left an orphan storage object"
    );
    assert!(
        mail::get_draft(&pool, &author, draft_id).await.is_err(),
        "the draft is gone"
    );
}
