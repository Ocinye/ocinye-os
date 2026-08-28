//! Ocinye Mail routes.
//!
//! # Privacy is decided below this layer
//!
//! Every handler passes the acting principal into the service, which filters on
//! mailbox ownership in SQL. No handler here consults an administrative role,
//! and none can: a personal mailbox is not reachable by privilege
//! (briefing §26).

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ocinye_contracts::{
    Classification, ComposeAction, MailAddress, MailFolder, MailboxKind, Permission,
    RemoteContentPolicy,
};
use ocinye_core::modules::mail::{self, provider::OutgoingMessage};
use ocinye_core::modules::platform;
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

/// Mail routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mail/mailboxes", get(list_mailboxes))
        .route("/mail/mailboxes/{mailbox_id}/messages", get(list_messages))
        .route("/mail/mailboxes/{mailbox_id}/sync", post(sync))
        .route("/mail/messages/{message_id}", get(read_message))
        .route("/mail/messages/{message_id}/flags", post(set_flags))
        .route("/mail/send", post(send))
        .route("/mail/assist", post(assist))
        .route("/mail/status", get(status))
        .route("/mail/preferences", get(preferences).post(save_preferences))
}

// ── Mailboxes ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MailboxView {
    id: Uuid,
    address: String,
    display_name: Option<String>,
    kind: &'static str,
    /// The role held in a shared mailbox. Absent for one's own.
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<&'static str>,
    /// Whether this identity may be sent from.
    may_send: bool,
    /// Whether replies may go out as this identity.
    may_reply: bool,
    last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Why the last synchronisation failed. Institutional language, never a
    /// protocol error.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_sync_error: Option<String>,
    unread: Vec<FolderCount>,
}

#[derive(Serialize)]
struct FolderCount {
    folder: &'static str,
    label: &'static str,
    unread: i64,
}

/// `GET /mail/mailboxes`
async fn list_mailboxes(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<Vec<MailboxView>>, ApiError> {
    let mailboxes = mail::mailboxes(&state.pool, &principal)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let mut views = Vec::with_capacity(mailboxes.len());
    for mailbox in mailboxes {
        // Counts come from real rows. A hardcoded badge is a lie the interface
        // repeats every time it renders (briefing §9).
        let counts = mail::repository::unread_counts(&state.pool, mailbox.id)
            .await
            .map_err(|error| ApiError::new(error, &ids))?;

        views.push(MailboxView {
            id: mailbox.id,
            address: mailbox.address.clone(),
            display_name: mailbox.display_name.clone(),
            kind: match mailbox.kind {
                MailboxKind::Personal => "personal",
                MailboxKind::Shared => "shared",
            },
            role: mailbox.role.map(|role| role.as_str()),
            may_send: mailbox.may_send(),
            may_reply: mailbox.may_reply(),
            last_synced_at: mailbox.last_synced_at,
            last_sync_error: mailbox.last_sync_error.clone(),
            unread: MailFolder::all()
                .into_iter()
                .map(|folder| FolderCount {
                    folder: folder.as_str(),
                    label: folder.label(),
                    unread: counts
                        .iter()
                        .find(|(f, _)| *f == folder)
                        .map_or(0, |(_, count)| *count),
                })
                .collect(),
        });
    }

    Ok(Json(views))
}

// ── Messages ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    page: Option<i64>,
}

/// How many messages one page carries.
///
/// A mailbox can hold fifty thousand messages; loading them to render a list
/// would be unusable and pointless (briefing §101).
const PAGE_SIZE: i64 = 50;

/// Página mais distante que se pode pedir.
///
/// Um `OFFSET` para além disto não descreve uma caixa de correio real, e aceitar
/// um número arbitrário do cliente é dar-lhe a escolha do trabalho que a base de
/// dados faz (`CLAUDE.md` §29).
const MAX_PAGE: i64 = 10_000;

/// `GET /mail/mailboxes/{id}/messages`
async fn list_messages(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(mailbox_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Resolves through the ownership filter: a mailbox that is not the
    // caller's reads as not found.
    let mailbox = mail::mailbox(&state.pool, &principal, mailbox_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let folder = query
        .folder
        .as_deref()
        .and_then(MailFolder::parse)
        .unwrap_or(MailFolder::Inbox);

    let messages = match query.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        Some(term) => mail::repository::search_messages(&state.pool, mailbox.id, term, PAGE_SIZE)
            .await
            .map_err(|error| ApiError::new(error, &ids))?,
        None => {
            // Saturating, e limitado: `page` chega do cliente. Sem isto,
            // `?page=9223372036854775807` transbordava a multiplicação — pânico
            // em depuração, `OFFSET` negativo em release. Todo o resto da API
            // pagina por `PageRequest`, que já é `u32` e limitado; o correio
            // era a excepção.
            let page = query.page.unwrap_or(1).clamp(1, MAX_PAGE);
            mail::repository::list_messages(
                &state.pool,
                mailbox.id,
                folder,
                PAGE_SIZE,
                (page - 1).saturating_mul(PAGE_SIZE),
            )
            .await
            .map_err(|error| ApiError::new(error, &ids))?
        }
    };

    Ok(Json(serde_json::json!({
        "folder": folder.as_str(),
        "items": messages,
        "page_size": PAGE_SIZE,
    })))
}

#[derive(Deserialize)]
struct ReadQuery {
    /// Whether the member asked for remote content on this message.
    ///
    /// Defaults to `false` everywhere. Remote images are how email tracking
    /// works, and loading them by default would tell every sender when their
    /// message was opened (briefing §12).
    #[serde(default)]
    allow_remote: bool,
}

/// `GET /mail/messages/{id}`
async fn read_message(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(message_id): Path<Uuid>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<mail::ReadableMessage>, ApiError> {
    let message = mail::read_message(
        &state.pool,
        state.mail_provider.as_ref(),
        &principal,
        message_id,
        query.allow_remote,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(message))
}

#[derive(Deserialize)]
struct FlagsRequest {
    #[serde(default)]
    read: Option<bool>,
    #[serde(default)]
    starred: Option<bool>,
}

/// `POST /mail/messages/{id}/flags`
///
/// Goes to the mail service first, and to the index only if that succeeded.
/// The reverse order would show a state the service does not have.
async fn set_flags(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(message_id): Path<Uuid>,
    Json(request): Json<FlagsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if request.read.is_none() && request.starred.is_none() {
        return Err(ApiError::new(
            CoreError::Validation("Indique que estado alterar.".to_owned()),
            &ids,
        ));
    }

    mail::set_flag(
        &state.pool,
        state.mail_provider.as_ref(),
        &principal,
        message_id,
        request.read,
        request.starred,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({ "updated": true })))
}

#[derive(Deserialize)]
struct SyncRequest {
    #[serde(default)]
    folder: Option<String>,
}

/// `POST /mail/mailboxes/{id}/sync`
///
/// Refreshes the index from the mail service. Explicit rather than automatic:
/// there is no background ingestion worker yet, and pretending otherwise would
/// leave members waiting for mail that nothing was fetching.
async fn sync(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Path(mailbox_id): Path<Uuid>,
    Json(request): Json<SyncRequest>,
) -> Result<Json<mail::SyncOutcome>, ApiError> {
    let folder = request
        .folder
        .as_deref()
        .and_then(MailFolder::parse)
        .unwrap_or(MailFolder::Inbox);

    let outcome = mail::sync(
        &state.pool,
        state.mail_provider.as_ref(),
        &principal,
        mailbox_id,
        folder,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(outcome))
}

// ── Sending ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SendRequest {
    mailbox_id: Uuid,
    #[serde(default)]
    to: Vec<String>,
    #[serde(default)]
    cc: Vec<String>,
    #[serde(default)]
    bcc: Vec<String>,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    body: String,
    /// Answer to a previous confirmation request. Never turns a refusal into a
    /// send: the policy re-decides, and a refusal stays refused.
    #[serde(default)]
    confirmed: bool,
}

/// `POST /mail/send`
///
/// The only path that hands a message to the provider, and it is reached only
/// by an explicit act. **No AI-assisted flow calls it** (briefing §15).
async fn send(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(request): Json<SendRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let domains = state.institutional_domains().await;

    let parse = |addresses: &[String]| -> Vec<MailAddress> {
        addresses
            .iter()
            .map(|address| address.trim())
            .filter(|address| !address.is_empty())
            .map(|address| MailAddress::new(address, None, &domains))
            .collect()
    };

    let to = parse(&request.to);
    let cc = parse(&request.cc);
    let bcc = parse(&request.bcc);

    let mut recipients = to.clone();
    recipients.extend(cc.clone());
    recipients.extend(bcc.clone());

    let mailbox = mail::mailbox(&state.pool, &principal, request.mailbox_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    let to_provider = |addresses: Vec<MailAddress>| {
        addresses
            .into_iter()
            .map(|address| mail::sender_identity(&address.address, None))
            .collect()
    };

    let message = OutgoingMessage {
        from: mail::sender_identity(&mailbox.address, mailbox.display_name.clone()),
        to: to_provider(to),
        cc: to_provider(cc),
        bcc: to_provider(bcc),
        subject: request.subject,
        body: request.body,
        in_reply_to: None,
        references: Vec::new(),
        attachments: Vec::new(),
    };

    // Attachments are `PLANNED`: with no object storage configured there is
    // nothing to attach, so the classification list is empty and the policy
    // sees an unclassified message. When attachments arrive, their
    // classifications come from `mail_draft_attachments` (briefing §35).
    let classifications: Vec<Classification> = Vec::new();

    mail::send(
        &state.pool,
        state.mail_provider.as_ref(),
        &principal,
        request.mailbox_id,
        message,
        &recipients,
        &classifications,
        request.confirmed,
        &ids,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(serde_json::json!({ "sent": true })))
}

// ── Assistance ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AssistRequestBody {
    action: String,
    #[serde(default)]
    instruction: String,
    #[serde(default)]
    draft_body: Option<String>,
    #[serde(default)]
    source_message_id: Option<Uuid>,
}

/// `POST /mail/assist`
///
/// Returns text. **Never sends.** The member edits what comes back, or discards
/// it, and reaches the provider only through `POST /mail/send`.
async fn assist(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(request): Json<AssistRequestBody>,
) -> Result<Json<mail::AssistResult>, ApiError> {
    let action = ComposeAction::parse(&request.action).ok_or_else(|| {
        ApiError::new(
            CoreError::Validation("Acção de assistência desconhecida.".to_owned()),
            &ids,
        )
    })?;

    let capabilities =
        platform::system_capabilities(&state.pool, &state.config, state.store.is_some())
            .await
            .map_err(|error| ApiError::new(error, &ids))?;

    let result = mail::assist(
        &state.pool,
        &principal,
        &mail::AssistRequest {
            action,
            instruction: request.instruction,
            draft_body: request.draft_body,
            source_message_id: request.source_message_id,
        },
        &capabilities,
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(result))
}

// ── Preferences ─────────────────────────────────────────────────────────

/// `GET /mail/preferences`
///
/// One's own, always. There is no path here that reads somebody else's, and
/// none is coming: an administrative role is not a key to a colleague's
/// settings any more than to their correspondence (briefing §26).
async fn preferences(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<mail::MailPreferences>, ApiError> {
    let preferences = mail::repository::preferences(&state.pool, principal.person_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(preferences))
}

#[derive(Deserialize)]
struct PreferencesRequest {
    #[serde(default)]
    signature: Option<String>,
    #[serde(default)]
    remote_content_policy: Option<String>,
}

/// `POST /mail/preferences`
async fn save_preferences(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
    Json(request): Json<PreferencesRequest>,
) -> Result<Json<mail::MailPreferences>, ApiError> {
    // A signature is arbitrary member text and is rendered later. Bounding it
    // here keeps a pathological value out of the database rather than
    // discovering it at render time.
    const SIGNATURE_LIMIT: usize = 2000;

    if let Some(signature) = request.signature.as_deref() {
        if signature.chars().count() > SIGNATURE_LIMIT {
            return Err(ApiError::new(
                CoreError::Validation(format!("A assinatura excede {SIGNATURE_LIMIT} caracteres.")),
                &ids,
            ));
        }
    }

    let preferences = mail::MailPreferences {
        signature: request.signature,
        // Anything unrecognised blocks. A malformed value cannot turn tracking
        // on by accident (see `RemoteContentPolicy::parse`).
        remote_content_policy: RemoteContentPolicy::parse(
            request.remote_content_policy.as_deref().unwrap_or("block"),
        ),
    };

    mail::repository::save_preferences(&state.pool, principal.person_id, &preferences)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(preferences))
}

// ── Status ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MailStatus {
    /// Whether mail can be read.
    can_read: bool,
    /// Whether mail can be sent. Distinct: IMAP and SMTP are different
    /// services (briefing §105).
    can_send: bool,
    /// Whether AI assistance can serve a request.
    ///
    /// Separate again: a working mail service with no AI node is a perfectly
    /// ordinary state, and so is the reverse (briefing §61).
    ai_assist_available: bool,
    /// Whether the acting member may use the assistant at all.
    may_use_ai: bool,
    /// In institutional language, safe to show.
    detail: String,
    /// Where the adapter connects. Hosts and ports, never credentials.
    endpoints: Vec<String>,
    /// The adapter in use.
    adapter: &'static str,
}

/// `GET /mail/status`
async fn status(
    State(state): State<AppState>,
    Ids(ids): Ids,
    CurrentPrincipal(principal): CurrentPrincipal,
) -> Result<Json<MailStatus>, ApiError> {
    let health = state.mail_provider.health().await;

    let capabilities =
        platform::system_capabilities(&state.pool, &state.config, state.store.is_some())
            .await
            .map_err(|error| ApiError::new(error, &ids))?;

    let institution = ocinye_domain::ResourceContext::organisation(
        ocinye_domain::ResourceKind::Person,
        principal.organisation_id,
    );

    Ok(Json(MailStatus {
        can_read: health.can_read,
        can_send: health.can_send,
        ai_assist_available: capabilities.any_ai_usable(),
        may_use_ai: ocinye_domain::can(&principal, Permission::MailAiUse, &institution, None)
            .allowed,
        detail: health.detail,
        endpoints: health.endpoints,
        adapter: state.mail_provider.adapter_name(),
    }))
}
