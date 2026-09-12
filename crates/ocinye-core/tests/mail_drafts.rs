//! Composer draft lifecycle — autosave, resume, discard, and their boundaries.
//!
//! These exercise the real service against a throwaway database. Without
//! `OCINYE_TEST_DATABASE_URL` each returns early, and the harness's enumeration
//! contract accounts for that separately.

use ocinye_core::modules::mail::{self, DraftInput};
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

const DOMAIN: &str = "ocinye.com";

async fn pool() -> Option<PgPool> {
    let url = std::env::var("OCINYE_TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url)
        .await
        .expect("OCINYE_TEST_DATABASE_URL is set but the database is unreachable");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations must apply to the test database");
    ocinye_core::fixtures::refuse_canonical_organisation(&pool).await;
    Some(pool)
}

async fn organisation(pool: &PgPool) -> Uuid {
    let slug = format!("m{}", Uuid::new_v4().simple());
    sqlx::query_scalar("INSERT INTO organisations (slug, name) VALUES ($1, $1) RETURNING id")
        .bind(&slug)
        .fetch_one(pool)
        .await
        .expect("organisation")
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
    // Mail needs `mail.use`; drafting is gated by it. `research_member` grants it.
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

fn input(mailbox_id: Uuid, draft_id: Option<Uuid>, subject: &str, body: &str) -> DraftInput {
    DraftInput {
        draft_id,
        mailbox_id,
        to: vec!["dest@exemplo.com".to_owned()],
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: Some(subject.to_owned()),
        body: body.to_owned(),
        in_reply_to: None,
    }
}

/// Autosave creates a draft once, then updates the same row — never a new one
/// per keystroke.
#[tokio::test]
async fn save_creates_then_updates_the_same_draft() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let ids = CorrelationIds::generate();

    let created = mail::save_draft(
        &pool,
        &author,
        &ids,
        input(mailbox, None, "Olá", "primeiro"),
    )
    .await
    .expect("create");
    assert_eq!(created.subject.as_deref(), Some("Olá"));

    let updated = mail::save_draft(
        &pool,
        &author,
        &ids,
        input(mailbox, Some(created.id), "Olá", "segundo"),
    )
    .await
    .expect("update");
    assert_eq!(updated.id, created.id, "update reuses the same draft");
    assert_eq!(updated.body, "segundo");

    let drafts = mail::list_drafts(&pool, &author, None).await.expect("list");
    assert_eq!(drafts.len(), 1, "autosave did not spawn a second draft");
}

/// A draft round-trips every recipient field, Bcc included.
#[tokio::test]
async fn a_draft_round_trips_recipients_including_bcc() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let ids = CorrelationIds::generate();

    let saved = mail::save_draft(
        &pool,
        &author,
        &ids,
        DraftInput {
            draft_id: None,
            mailbox_id: mailbox,
            to: vec!["a@exemplo.com".to_owned()],
            cc: vec!["b@exemplo.com".to_owned()],
            bcc: vec!["c@exemplo.com".to_owned()],
            subject: Some("Assunto".to_owned()),
            body: "Corpo".to_owned(),
            in_reply_to: None,
        },
    )
    .await
    .expect("save");

    let got = mail::get_draft(&pool, &author, saved.id)
        .await
        .expect("get");
    assert_eq!(got.to_addresses, vec!["a@exemplo.com"]);
    assert_eq!(got.cc_addresses, vec!["b@exemplo.com"]);
    assert_eq!(got.bcc_addresses, vec!["c@exemplo.com"]);
}

/// Autosave stores an address still being typed without refusing it — an
/// in-progress address is a normal draft state, not an error.
#[tokio::test]
async fn autosave_does_not_hard_validate_addresses() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let ids = CorrelationIds::generate();

    let saved = mail::save_draft(
        &pool,
        &author,
        &ids,
        DraftInput {
            draft_id: None,
            mailbox_id: mailbox,
            to: vec!["jes".to_owned()],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: None,
            body: "meio a escrever".to_owned(),
            in_reply_to: None,
        },
    )
    .await
    .expect("save accepts a partial address");
    assert_eq!(saved.to_addresses, vec!["jes"]);
}

/// Discard removes the draft; a reload finds nothing.
#[tokio::test]
async fn discard_removes_the_draft() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let ids = CorrelationIds::generate();

    let saved = mail::save_draft(&pool, &author, &ids, input(mailbox, None, "x", "y"))
        .await
        .expect("save");
    mail::discard_draft(&pool, &author, &ids, saved.id)
        .await
        .expect("discard");

    assert!(
        mail::get_draft(&pool, &author, saved.id).await.is_err(),
        "a discarded draft is gone"
    );
    let drafts = mail::list_drafts(&pool, &author, None).await.expect("list");
    assert!(drafts.is_empty());
}

/// Discarding a draft that is already gone succeeds — the intent is satisfied.
#[tokio::test]
async fn discard_is_idempotent() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let ids = CorrelationIds::generate();

    mail::discard_draft(&pool, &author, &ids, Uuid::new_v4())
        .await
        .expect("discarding a non-existent draft is not an error");
}

/// Another member cannot read, update or discard a draft that is not theirs.
#[tokio::test]
async fn a_draft_is_private_to_its_author() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let intruder = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let ids = CorrelationIds::generate();

    let saved = mail::save_draft(
        &pool,
        &author,
        &ids,
        input(mailbox, None, "privado", "segredo"),
    )
    .await
    .expect("save");

    assert!(
        mail::get_draft(&pool, &intruder, saved.id).await.is_err(),
        "IDOR: another member must not read the draft"
    );
    assert!(
        mail::save_draft(
            &pool,
            &intruder,
            &ids,
            input(mailbox, Some(saved.id), "x", "roubado")
        )
        .await
        .is_err(),
        "IDOR: another member must not update the draft"
    );
    // A discard by the intruder deletes nothing (idempotent success), and the
    // author still finds their draft intact.
    mail::discard_draft(&pool, &intruder, &ids, saved.id)
        .await
        .expect("idempotent");
    let still = mail::get_draft(&pool, &author, saved.id)
        .await
        .expect("get");
    assert_eq!(still.body, "segredo", "the author's draft survived");
}

/// A draft cannot be created from a mailbox the caller cannot send from.
#[tokio::test]
async fn cannot_draft_from_an_unreachable_mailbox() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let owner = person(&pool, org).await;
    let outsider = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, owner.person_id).await;
    let ids = CorrelationIds::generate();

    assert!(
        mail::save_draft(&pool, &outsider, &ids, input(mailbox, None, "x", "y"))
            .await
            .is_err(),
        "a mailbox that is not the caller's yields not found"
    );
}

/// The Drafts list is newest first.
#[tokio::test]
async fn drafts_are_listed_newest_first() {
    let Some(pool) = pool().await else { return };
    let org = organisation(&pool).await;
    let author = person(&pool, org).await;
    let mailbox = personal_mailbox(&pool, org, author.person_id).await;
    let ids = CorrelationIds::generate();

    let first = mail::save_draft(&pool, &author, &ids, input(mailbox, None, "um", "1"))
        .await
        .expect("first");
    // Nudge the second draft's timestamp to be strictly later.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let second = mail::save_draft(&pool, &author, &ids, input(mailbox, None, "dois", "2"))
        .await
        .expect("second");

    let drafts = mail::list_drafts(&pool, &author, None).await.expect("list");
    assert_eq!(drafts.len(), 2);
    assert_eq!(drafts[0].id, second.id, "newest first");
    assert_eq!(drafts[1].id, first.id);
}
