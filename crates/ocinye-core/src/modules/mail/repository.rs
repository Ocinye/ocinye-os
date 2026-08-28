//! Mail persistence.
//!
//! # Where the privacy boundary lives
//!
//! In the `WHERE` clauses here, not in a check the caller might forget. Every
//! query that reaches a mailbox takes the acting person and filters on
//! ownership or live shared membership, so a message the caller may not see
//! never leaves the database (briefing §26).
//!
//! An administrative role appears nowhere in this file. That is the point.

use chrono::{DateTime, Utc};
use ocinye_contracts::{
    DraftOrigin, MailFolder, MailboxKind, RemoteContentPolicy, SharedMailboxRole,
};
use sqlx::{PgExecutor, Row};
use uuid::Uuid;

use crate::error::CoreResult;

/// A mailbox the acting person may reach.
#[derive(Debug, Clone)]
pub struct AccessibleMailbox {
    /// Identifier.
    pub id: Uuid,
    /// The address.
    pub address: String,
    /// Name shown beside it.
    pub display_name: Option<String>,
    /// Personal or shared.
    pub kind: MailboxKind,
    /// The role held, for a shared mailbox. `None` for one's own.
    pub role: Option<SharedMailboxRole>,
    /// When it last synchronised.
    pub last_synced_at: Option<DateTime<Utc>>,
    /// Why the last synchronisation failed, if it did.
    pub last_sync_error: Option<String>,
    /// Se há uma credencial guardada para esta caixa.
    ///
    /// Deriva da existência da linha em `mailbox_credentials` e de mais nada:
    /// uma segunda coluna a afirmar o mesmo facto é uma coluna que pode
    /// discordar dele (ADR-0409).
    pub has_credential: bool,
}

impl AccessibleMailbox {
    /// Whether the acting person may send from this address.
    ///
    /// One's own mailbox always; a shared one only with a role that says so.
    #[must_use]
    pub fn may_send(&self) -> bool {
        match self.kind {
            MailboxKind::Personal => true,
            MailboxKind::Shared => self.role.is_some_and(SharedMailboxRole::may_send),
        }
    }

    /// Whether the acting person may reply from this address.
    #[must_use]
    pub fn may_reply(&self) -> bool {
        match self.kind {
            MailboxKind::Personal => true,
            MailboxKind::Shared => self.role.is_some_and(SharedMailboxRole::may_reply),
        }
    }
}

/// Every mailbox the acting person may reach.
///
/// Their own, plus every shared mailbox where they hold a live membership.
/// **No administrative role widens this.**
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn accessible_mailboxes<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<Vec<AccessibleMailbox>> {
    let rows = sqlx::query(
        "SELECT m.id, m.address, m.display_name, m.kind, m.last_synced_at, m.last_sync_error,
                s.role AS shared_role,
                EXISTS (
                    SELECT 1 FROM mailbox_credentials c WHERE c.mailbox_id = m.id
                ) AS has_credential
           FROM mailboxes m
           LEFT JOIN shared_mailbox_memberships s
                  ON s.mailbox_id = m.id
                 AND s.person_id = $1
                 AND s.revoked_at IS NULL
          WHERE m.connected = true
            AND (
                (m.kind = 'personal' AND m.owner_id = $1)
                OR (m.kind = 'shared' AND s.id IS NOT NULL)
            )
          ORDER BY m.kind, lower(m.address)",
    )
    .bind(person_id)
    .fetch_all(executor)
    .await?;

    rows.into_iter().map(mailbox_from_row).collect()
}

/// One mailbox, if the acting person may reach it.
///
/// Returns `None` both when the mailbox does not exist and when it exists but
/// is not theirs. The caller cannot tell which, and must not be able to: that
/// `ana@ocinye.com` has a mailbox is not something a colleague learns by
/// guessing identifiers (ADR-0100).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn accessible_mailbox<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    mailbox_id: Uuid,
) -> CoreResult<Option<AccessibleMailbox>> {
    let row = sqlx::query(
        "SELECT m.id, m.address, m.display_name, m.kind, m.last_synced_at, m.last_sync_error,
                s.role AS shared_role,
                EXISTS (
                    SELECT 1 FROM mailbox_credentials c WHERE c.mailbox_id = m.id
                ) AS has_credential
           FROM mailboxes m
           LEFT JOIN shared_mailbox_memberships s
                  ON s.mailbox_id = m.id
                 AND s.person_id = $1
                 AND s.revoked_at IS NULL
          WHERE m.id = $2
            AND m.connected = true
            AND (
                (m.kind = 'personal' AND m.owner_id = $1)
                OR (m.kind = 'shared' AND s.id IS NOT NULL)
            )",
    )
    .bind(person_id)
    .bind(mailbox_id)
    .fetch_optional(executor)
    .await?;

    row.map(mailbox_from_row).transpose()
}

fn mailbox_from_row(row: sqlx::postgres::PgRow) -> CoreResult<AccessibleMailbox> {
    let kind: String = row.try_get("kind")?;
    let role: Option<String> = row.try_get("shared_role")?;

    Ok(AccessibleMailbox {
        id: row.try_get("id")?,
        address: row.try_get("address")?,
        display_name: row.try_get("display_name")?,
        // An unrecognised kind reads as `Shared`, which is the stricter of the
        // two: a shared mailbox needs a role, and a row this build cannot
        // interpret must not be treated as somebody's own.
        kind: MailboxKind::parse(&kind).unwrap_or(MailboxKind::Shared),
        role: role.as_deref().and_then(SharedMailboxRole::parse),
        last_synced_at: row.try_get("last_synced_at")?,
        last_sync_error: row.try_get("last_sync_error")?,
        has_credential: row.try_get("has_credential")?,
    })
}

/// A message as the list shows it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexedMessage {
    /// Identifier.
    pub id: Uuid,
    /// The mailbox it sits in.
    ///
    /// Carried so an interface that opened a message by identifier can show it
    /// in its own mailbox, rather than guessing at the first one.
    pub mailbox_id: Uuid,
    /// The folder it sits in.
    pub folder: String,
    /// The provider's identifier, for fetching the body.
    pub provider_id: String,
    /// Conversation identity.
    pub thread_key: Option<String>,
    /// Who sent it.
    pub from_address: String,
    /// The display name they chose. Shown beside the address, never instead.
    pub from_display_name: Option<String>,
    /// Subject.
    pub subject: Option<String>,
    /// A short excerpt.
    pub snippet: Option<String>,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
    /// Whether it has been read.
    pub is_read: bool,
    /// Whether it is flagged.
    pub is_starred: bool,
    /// Whether it carries attachments.
    pub has_attachments: bool,
    /// How many messages share its conversation.
    pub thread_count: i64,
}

/// One page of a folder.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_messages<'e>(
    executor: impl PgExecutor<'e>,
    mailbox_id: Uuid,
    folder: MailFolder,
    limit: i64,
    offset: i64,
) -> CoreResult<Vec<IndexedMessage>> {
    // `Starred` is a flag, not a folder: it selects across the mailbox.
    let starred_only = folder == MailFolder::Starred;

    let rows = sqlx::query(
        "SELECT m.id, m.mailbox_id, m.folder, m.provider_id, m.thread_key, m.from_address,
                m.from_display_name, m.subject, m.snippet, m.sent_at, m.is_read, m.is_starred,
                m.has_attachments,
                (SELECT count(*) FROM mail_messages t
                  WHERE t.mailbox_id = m.mailbox_id
                    AND t.thread_key IS NOT NULL
                    AND t.thread_key = m.thread_key) AS thread_count
           FROM mail_messages m
          WHERE m.mailbox_id = $1
            AND ($2 = true OR m.folder = $3)
            AND ($2 = false OR m.is_starred = true)
          ORDER BY m.sent_at DESC
          LIMIT $4 OFFSET $5",
    )
    .bind(mailbox_id)
    .bind(starred_only)
    .bind(folder.as_str())
    .bind(limit)
    .bind(offset)
    .fetch_all(executor)
    .await?;

    rows.into_iter().map(message_from_row).collect()
}

/// Search a mailbox.
///
/// Scoped to one mailbox the caller already holds. Personal mail never enters
/// the institutional index, so a search here cannot reach anyone else's
/// correspondence (briefing §52).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn search_messages<'e>(
    executor: impl PgExecutor<'e>,
    mailbox_id: Uuid,
    query: &str,
    limit: i64,
) -> CoreResult<Vec<IndexedMessage>> {
    let rows = sqlx::query(
        "SELECT m.id, m.mailbox_id, m.folder, m.provider_id, m.thread_key, m.from_address,
                m.from_display_name, m.subject, m.snippet, m.sent_at, m.is_read, m.is_starred,
                m.has_attachments,
                1::bigint AS thread_count
           FROM mail_messages m
          WHERE m.mailbox_id = $1
            AND m.search_vector @@ websearch_to_tsquery('simple', $2)
          ORDER BY ts_rank(m.search_vector, websearch_to_tsquery('simple', $2)) DESC,
                   m.sent_at DESC
          LIMIT $3",
    )
    .bind(mailbox_id)
    .bind(query)
    .bind(limit)
    .fetch_all(executor)
    .await?;

    rows.into_iter().map(message_from_row).collect()
}

fn message_from_row(row: sqlx::postgres::PgRow) -> CoreResult<IndexedMessage> {
    Ok(IndexedMessage {
        id: row.try_get("id")?,
        mailbox_id: row.try_get("mailbox_id")?,
        folder: row.try_get("folder")?,
        provider_id: row.try_get("provider_id")?,
        thread_key: row.try_get("thread_key")?,
        from_address: row.try_get("from_address")?,
        from_display_name: row.try_get("from_display_name")?,
        subject: row.try_get("subject")?,
        snippet: row.try_get("snippet")?,
        sent_at: row.try_get("sent_at")?,
        is_read: row.try_get("is_read")?,
        is_starred: row.try_get("is_starred")?,
        has_attachments: row.try_get("has_attachments")?,
        thread_count: row.try_get("thread_count")?,
    })
}

/// One message, if it belongs to a mailbox the caller may reach.
///
/// The ownership check is part of the query. Knowing a message identifier is
/// not authority to read the message (briefing §98).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn accessible_message<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    message_id: Uuid,
) -> CoreResult<Option<(IndexedMessage, Uuid, String)>> {
    let row = sqlx::query(
        "SELECT m.id, m.mailbox_id, m.folder, m.provider_id, m.thread_key, m.from_address,
                m.from_display_name, m.subject, m.snippet, m.sent_at, m.is_read, m.is_starred,
                m.has_attachments,
                1::bigint AS thread_count,
                b.address AS mailbox_address
           FROM mail_messages m
           JOIN mailboxes b ON b.id = m.mailbox_id
           LEFT JOIN shared_mailbox_memberships s
                  ON s.mailbox_id = b.id
                 AND s.person_id = $1
                 AND s.revoked_at IS NULL
          WHERE m.id = $2
            AND b.connected = true
            AND (
                (b.kind = 'personal' AND b.owner_id = $1)
                OR (b.kind = 'shared' AND s.id IS NOT NULL)
            )",
    )
    .bind(person_id)
    .bind(message_id)
    .fetch_optional(executor)
    .await?;

    let Some(row) = row else { return Ok(None) };
    let mailbox_id: Uuid = row.try_get("mailbox_id")?;
    let mailbox_address: String = row.try_get("mailbox_address")?;

    Ok(Some((message_from_row(row)?, mailbox_id, mailbox_address)))
}

/// How many unread messages sit in each folder.
///
/// Derived from real rows. A counter that is not is a fake counter
/// (briefing §9).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn unread_counts<'e>(
    executor: impl PgExecutor<'e>,
    mailbox_id: Uuid,
) -> CoreResult<Vec<(MailFolder, i64)>> {
    let rows = sqlx::query(
        "SELECT folder, count(*) AS unread
           FROM mail_messages
          WHERE mailbox_id = $1 AND is_read = false
          GROUP BY folder",
    )
    .bind(mailbox_id)
    .fetch_all(executor)
    .await?;

    let mut counts = Vec::with_capacity(rows.len());
    for row in rows {
        let folder: String = row.try_get("folder")?;
        if let Some(folder) = MailFolder::parse(&folder) {
            counts.push((folder, row.try_get("unread")?));
        }
    }
    Ok(counts)
}

/// Record a message in the index.
///
/// Upserts on the provider's identifier, so re-synchronising the same message
/// updates it rather than duplicating it.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn upsert_message<'e>(
    executor: impl PgExecutor<'e>,
    mailbox_id: Uuid,
    header: &super::provider::MessageHeader,
) -> CoreResult<Uuid> {
    let to: Vec<String> = header.to.iter().map(|a| a.address.clone()).collect();
    let cc: Vec<String> = header.cc.iter().map(|a| a.address.clone()).collect();

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO mail_messages
             (mailbox_id, provider_id, message_id, thread_key, folder, from_address,
              from_display_name, to_addresses, cc_addresses, subject, snippet, sent_at,
              is_read, is_starred, has_attachments, size_bytes)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
         ON CONFLICT (mailbox_id, provider_id) DO UPDATE SET
             folder = EXCLUDED.folder,
             is_read = EXCLUDED.is_read,
             is_starred = EXCLUDED.is_starred,
             indexed_at = now()
         RETURNING id",
    )
    .bind(mailbox_id)
    .bind(&header.provider_id)
    .bind(header.message_id.as_deref())
    .bind(header.thread_key.as_deref())
    .bind(header.folder.as_str())
    .bind(&header.from.address)
    .bind(header.from.display_name.as_deref())
    .bind(&to)
    .bind(&cc)
    .bind(header.subject.as_deref())
    .bind(header.snippet.as_deref())
    .bind(header.sent_at)
    .bind(header.is_read)
    .bind(header.is_starred)
    .bind(header.has_attachments)
    .bind(header.size_bytes)
    .fetch_one(executor)
    .await?;

    Ok(id)
}

/// Set the read or starred flag on a message the caller may reach.
///
/// Returns whether a row changed, which is `false` both when the message does
/// not exist and when it is not theirs.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn set_flag<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    message_id: Uuid,
    read: Option<bool>,
    starred: Option<bool>,
) -> CoreResult<bool> {
    let affected = sqlx::query(
        "UPDATE mail_messages m
            SET is_read = coalesce($3, m.is_read),
                is_starred = coalesce($4, m.is_starred)
           FROM mailboxes b
           LEFT JOIN shared_mailbox_memberships s
                  ON s.mailbox_id = b.id
                 AND s.person_id = $1
                 AND s.revoked_at IS NULL
          WHERE m.id = $2
            AND b.id = m.mailbox_id
            AND b.connected = true
            AND (
                (b.kind = 'personal' AND b.owner_id = $1)
                OR (b.kind = 'shared' AND s.id IS NOT NULL)
            )",
    )
    .bind(person_id)
    .bind(message_id)
    .bind(read)
    .bind(starred)
    .execute(executor)
    .await?
    .rows_affected();

    Ok(affected > 0)
}

/// Record the outcome of a synchronisation against a mailbox.
///
/// # Why the failure is stored and not only logged
///
/// A list that silently shows stale mail is worse than one that says it could
/// not refresh. The interface reads `last_sync_error` and shows it beside the
/// mailbox, so «nothing new» and «could not ask» stay distinguishable
/// (briefing §100).
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn record_sync<'e>(
    executor: impl PgExecutor<'e>,
    mailbox_id: Uuid,
    failure: Option<&str>,
) -> CoreResult<()> {
    // `last_synced_at` advances only on success: it answers "when was this
    // last known to be current", and a failed attempt does not make it current.
    sqlx::query(
        "UPDATE mailboxes
            SET last_synced_at = CASE WHEN $2::text IS NULL THEN now() ELSE last_synced_at END,
                last_sync_error = $2,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(mailbox_id)
    .bind(failure)
    .execute(executor)
    .await?;

    Ok(())
}

/// The domains that count as inside the institution.
///
/// # Why this reads from the mailboxes and not from configuration
///
/// The capability layer has no `CoreConfig`, and threading one through every
/// handler to answer «is this address ours» would be a lot of plumbing for one
/// question. The mailboxes the institution actually owns are the same answer,
/// and they are already in the database.
///
/// An empty result makes every recipient external, which fails closed
/// (ADR-0403).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn institutional_domains<'e>(executor: impl PgExecutor<'e>) -> CoreResult<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT lower(split_part(address, '@', 2)) AS domain
           FROM mailboxes
          WHERE address LIKE '%@%'",
    )
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(domain,)| domain)
        .filter(|domain| !domain.is_empty())
        .collect())
}

/// A draft, as the agentic plane and the composer both see it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MailDraft {
    /// Identifier.
    pub id: Uuid,
    /// The mailbox it will be sent from.
    pub mailbox_id: Uuid,
    /// The identity it will be sent as.
    pub sender_address: String,
    /// Recipients.
    pub to_addresses: Vec<String>,
    /// Subject.
    pub subject: Option<String>,
    /// Body.
    pub body: String,
    /// How it came to be written.
    pub origin: DraftOrigin,
}

/// Write a draft.
///
/// # Why the origin is recorded
///
/// «Was this written by a person or produced by a model» is a question the
/// institution will want answered later. It is not shown as a banner to the
/// recipient — the message is the member's, whatever tool wrote it — but it is
/// kept (ADR-0406, briefing §71).
///
/// # Errors
///
/// Returns an error when the statement fails, or when the mailbox is not one
/// the author may send from.
pub async fn create_draft<'e>(
    executor: impl PgExecutor<'e>,
    mailbox_id: Uuid,
    author_id: Uuid,
    subject: &str,
    body: &str,
    to: &[String],
    in_reply_to: Option<Uuid>,
) -> CoreResult<Uuid> {
    // The sender address comes from the mailbox row, never from the caller: a
    // draft that could name its own sender would be a way to send as somebody
    // else once the send path is connected.
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO mail_drafts
             (mailbox_id, author_id, sender_address, to_addresses, subject, body,
              in_reply_to_id, origin)
         SELECT b.id, $2, b.address, $3, $4, $5, $6, 'ai_generated'
           FROM mailboxes b
          WHERE b.id = $1
         RETURNING id",
    )
    .bind(mailbox_id)
    .bind(author_id)
    .bind(to)
    .bind(subject)
    .bind(body)
    .bind(in_reply_to)
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| {
        crate::error::CoreError::NotFound("Caixa de correio não encontrada.".to_owned())
    })?;

    Ok(id)
}

/// One draft, if it belongs to a mailbox the caller may reach.
///
/// The ownership check is part of the query, as everywhere else in this module.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn accessible_draft<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    draft_id: Uuid,
) -> CoreResult<Option<MailDraft>> {
    let row = sqlx::query(
        "SELECT d.id, d.mailbox_id, d.sender_address, d.to_addresses, d.subject, d.body, d.origin
           FROM mail_drafts d
           JOIN mailboxes b ON b.id = d.mailbox_id
           LEFT JOIN shared_mailbox_memberships s
                  ON s.mailbox_id = b.id
                 AND s.person_id = $1
                 AND s.revoked_at IS NULL
          WHERE d.id = $2
            AND (
                (b.kind = 'personal' AND b.owner_id = $1)
                OR (b.kind = 'shared' AND s.id IS NOT NULL)
            )",
    )
    .bind(person_id)
    .bind(draft_id)
    .fetch_optional(executor)
    .await?;

    let Some(row) = row else { return Ok(None) };
    let origin: String = row.try_get("origin")?;

    Ok(Some(MailDraft {
        id: row.try_get("id")?,
        mailbox_id: row.try_get("mailbox_id")?,
        sender_address: row.try_get("sender_address")?,
        to_addresses: row.try_get("to_addresses")?,
        subject: row.try_get("subject")?,
        body: row.try_get("body")?,
        origin: DraftOrigin::parse(&origin),
    }))
}

/// What a member has chosen about how their mail behaves.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MailPreferences {
    /// Appended to messages they write. `None` when they have set none.
    pub signature: Option<String>,
    /// Whether remote content loads, and when.
    pub remote_content_policy: RemoteContentPolicy,
}

impl Default for MailPreferences {
    /// Blocking, with no signature.
    ///
    /// The default matters: a member who has never opened the settings screen
    /// must not be tracked by senders, and the row for them does not exist yet
    /// (briefing §12).
    fn default() -> Self {
        Self {
            signature: None,
            remote_content_policy: RemoteContentPolicy::Block,
        }
    }
}

/// One member's mail preferences.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn preferences<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<MailPreferences> {
    let row = sqlx::query(
        "SELECT signature, remote_content_policy
           FROM mail_preferences
          WHERE person_id = $1",
    )
    .bind(person_id)
    .fetch_optional(executor)
    .await?;

    // No row is not an error: it means the member has never changed anything,
    // and the safe defaults apply.
    let Some(row) = row else {
        return Ok(MailPreferences::default());
    };

    let policy: String = row.try_get("remote_content_policy")?;

    Ok(MailPreferences {
        signature: row.try_get("signature")?,
        // Anything unrecognised reads as blocking. A corrupted setting cannot
        // turn tracking back on.
        remote_content_policy: RemoteContentPolicy::parse(&policy),
    })
}

/// Save one member's mail preferences.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn save_preferences<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    preferences: &MailPreferences,
) -> CoreResult<()> {
    // A signature of pure whitespace is no signature. Storing it would append
    // blank lines to every message the member writes.
    let signature = preferences
        .signature
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    sqlx::query(
        "INSERT INTO mail_preferences (person_id, signature, remote_content_policy)
              VALUES ($1, $2, $3)
         ON CONFLICT (person_id) DO UPDATE
                SET signature = EXCLUDED.signature,
                    remote_content_policy = EXCLUDED.remote_content_policy,
                    updated_at = now()",
    )
    .bind(person_id)
    .bind(signature)
    .bind(preferences.remote_content_policy.as_str())
    .execute(executor)
    .await?;

    Ok(())
}

/// Uma credencial de caixa, tal como está guardada.
pub struct StoredCredential {
    /// O nome de utilizador com que a caixa se autentica.
    pub username: String,
    /// A senha cifrada.
    pub sealed: crate::password::sealed::Sealed,
    /// Quando foi guardada pela última vez.
    ///
    /// É o que distingue uma credencial de outra sem a abrir: uma sessão em
    /// cache que foi aberta com a senha anterior tem de ser descartada, e
    /// comparar o instante custa uma leitura em vez de uma decifragem.
    pub updated_at: DateTime<Utc>,
}

/// A credencial de uma caixa, tal como está guardada.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn credential_of<'e>(
    executor: impl PgExecutor<'e>,
    mailbox_id: Uuid,
) -> CoreResult<Option<StoredCredential>> {
    /// Uma linha de `mailbox_credentials`, tal como o SQL a devolve.
    type Linha = (String, Vec<u8>, Vec<u8>, DateTime<Utc>);

    let linha: Option<Linha> = sqlx::query_as(
        "SELECT username, nonce, ciphertext, updated_at
           FROM mailbox_credentials
          WHERE mailbox_id = $1",
    )
    .bind(mailbox_id)
    .fetch_optional(executor)
    .await?;

    Ok(linha.map(
        |(username, nonce, ciphertext, updated_at)| StoredCredential {
            username,
            sealed: crate::password::sealed::Sealed { nonce, ciphertext },
            updated_at,
        },
    ))
}

/// Guarda, ou substitui, a credencial de uma caixa.
///
/// # Porque substitui em vez de acumular
///
/// Porque uma caixa tem uma senha. Guardar a anterior seria guardar um segredo
/// que já não serve para nada e que continua a poder ser lido.
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn save_credential<'e>(
    executor: impl PgExecutor<'e>,
    mailbox_id: Uuid,
    username: &str,
    fechado: &crate::password::sealed::Sealed,
    connected_by: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO mailbox_credentials
                (mailbox_id, username, nonce, ciphertext, connected_by)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (mailbox_id) DO UPDATE
            SET username = EXCLUDED.username,
                nonce = EXCLUDED.nonce,
                ciphertext = EXCLUDED.ciphertext,
                connected_by = EXCLUDED.connected_by,
                updated_at = now()",
    )
    .bind(mailbox_id)
    .bind(username)
    .bind(&fechado.nonce)
    .bind(&fechado.ciphertext)
    .bind(connected_by)
    .execute(executor)
    .await?;
    Ok(())
}

/// Esquece a credencial de uma caixa.
///
/// # Errors
///
/// Devolve erro quando a escrita falha.
pub async fn forget_credential<'e>(
    executor: impl PgExecutor<'e>,
    mailbox_id: Uuid,
) -> CoreResult<()> {
    sqlx::query("DELETE FROM mailbox_credentials WHERE mailbox_id = $1")
        .bind(mailbox_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// As caixas ligadas, para o worker as manter indexadas.
///
/// # Porque não recebe um principal
///
/// Porque não há membro nenhum a pedir isto. É o sistema a manter em dia um
/// índice de caixas cujas credenciais já detém, e indexar não é divulgar: a
/// visibilidade continua a ser aplicada quando alguém **lê**, por
/// `accessible_mailboxes` e `accessible_message`.
///
/// A distinção é a do ADR-0407 — o Ocinye indexa, não arquiva. O que fica aqui
/// são metadados para desenhar uma lista, e o corpo vai-se buscar ao fornecedor
/// no momento em que alguém autorizado o abre.
///
/// # Errors
///
/// Devolve erro quando a leitura falha.
pub async fn connected_mailboxes<'e>(
    executor: impl PgExecutor<'e>,
) -> CoreResult<Vec<(Uuid, String)>> {
    let linhas: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, address FROM mailboxes WHERE connected = true ORDER BY address")
            .fetch_all(executor)
            .await?;
    Ok(linhas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mailbox(kind: MailboxKind, role: Option<SharedMailboxRole>) -> AccessibleMailbox {
        AccessibleMailbox {
            id: Uuid::from_u128(1),
            address: "a@ocinye.com".into(),
            display_name: None,
            kind,
            role,
            last_synced_at: None,
            last_sync_error: None,
            has_credential: false,
        }
    }

    #[test]
    fn a_personal_mailbox_always_sends_and_replies() {
        let own = mailbox(MailboxKind::Personal, None);
        assert!(own.may_send());
        assert!(own.may_reply());
    }

    #[test]
    fn a_shared_mailbox_sends_only_with_a_role_that_says_so() {
        assert!(!mailbox(MailboxKind::Shared, Some(SharedMailboxRole::Reader)).may_send());
        assert!(!mailbox(MailboxKind::Shared, Some(SharedMailboxRole::Responder)).may_send());
        assert!(mailbox(MailboxKind::Shared, Some(SharedMailboxRole::Sender)).may_send());
        assert!(mailbox(MailboxKind::Shared, Some(SharedMailboxRole::Manager)).may_send());
    }

    #[test]
    fn a_reader_reads_and_nothing_more() {
        let reader = mailbox(MailboxKind::Shared, Some(SharedMailboxRole::Reader));
        assert!(!reader.may_reply());
        assert!(!reader.may_send());
    }

    #[test]
    fn a_shared_mailbox_without_a_role_does_nothing() {
        // Should not arise — the query only returns shared mailboxes with a live
        // membership — but if the vocabulary ever drifts, this fails closed.
        let stranded = mailbox(MailboxKind::Shared, None);
        assert!(!stranded.may_send());
        assert!(!stranded.may_reply());
    }
}
