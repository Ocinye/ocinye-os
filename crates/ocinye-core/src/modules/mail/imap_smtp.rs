//! IMAP + SMTP adapter.
//!
//! The standards-based implementation of [`MailProvider`]. Chosen over any
//! vendor API because it works against every institutional mail service the
//! Ocinye is likely to meet, and because it does not make the domain depend on
//! one company's product decisions (ADR-0400).
//!
//! # What is not written here
//!
//! The SMTP conversation (`lettre`), the IMAP conversation (`async-imap`) and
//! the MIME parse (`mail-parser`). Writing any of those by hand would be the
//! same mistake as writing our own cryptography.
//!
//! # Credentials
//!
//! Read from the environment at construction and held only in memory. They are
//! never written to `mail_provider_settings`, never logged, and never returned
//! by any endpoint (briefing §58).

use std::sync::Arc;
use std::time::Duration;

use async_imap::Session;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use futures::TryStreamExt;
use lettre::message::{Mailbox as LettreMailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use ocinye_contracts::MailFolder;
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::{client::TlsStream, TlsConnector};

use super::provider::{
    AttachmentInfo, FetchedMessage, MailProvider, MessageHeader, MessagePage, OutgoingMessage,
    ProviderAddress, ProviderError, ProviderHealth, ProviderResult,
};
use crate::config::MailSecurity;
use crate::password::Secret;

/// How long any single IMAP operation may take.
///
/// A mail server that stops answering must not hold a request open until the
/// member gives up: they would have no way to tell a slow server from a broken
/// one. Thirty seconds is generous for a fetch and short enough to surface as
/// «indisponível» while somebody is still watching.
const IMAP_TIMEOUT: Duration = Duration::from_secs(30);

/// An authenticated IMAP session over TLS.
type ImapSession = Session<TlsStream<TcpStream>>;

/// How the adapter reaches the service.
pub struct ImapSmtpConfig {
    /// IMAP host.
    pub imap_host: String,
    /// IMAP port. 993 for implicit TLS, 143 for STARTTLS.
    pub imap_port: u16,
    /// How the IMAP connection is protected. Never unencrypted.
    pub imap_security: MailSecurity,
    /// SMTP host.
    pub smtp_host: String,
    /// SMTP port. 465 for implicit TLS, 587 for STARTTLS.
    pub smtp_port: u16,
    /// How the SMTP connection is protected. Never unencrypted.
    pub smtp_security: MailSecurity,
    /// The account the adapter authenticates as.
    pub username: String,
    /// Its password or application token. Redacted in `Debug`, zeroized on drop.
    pub password: Secret,
}

impl std::fmt::Debug for ImapSmtpConfig {
    /// Hosts and ports are operational detail; the credential is not.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImapSmtpConfig")
            .field("imap_host", &self.imap_host)
            .field("imap_port", &self.imap_port)
            .field("imap_security", &self.imap_security.as_str())
            .field("smtp_host", &self.smtp_host)
            .field("smtp_port", &self.smtp_port)
            .field("smtp_security", &self.smtp_security.as_str())
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// The IMAP + SMTP adapter.
pub struct ImapSmtpProvider {
    config: ImapSmtpConfig,
    smtp: AsyncSmtpTransport<Tokio1Executor>,
    /// Shared TLS configuration, built once.
    ///
    /// Reused across connections because building it parses the whole root
    /// store, which is wasteful per-request and identical every time.
    tls: Arc<ClientConfig>,
}

impl ImapSmtpProvider {
    /// Build the adapter.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::NotConfigured`] when the transport cannot be
    /// built, which in practice means an unusable host.
    pub fn new(config: ImapSmtpConfig) -> ProviderResult<Self> {
        let credentials =
            Credentials::new(config.username.clone(), config.password.expose().to_owned());

        // The configured security decides, not the port: a service on a
        // non-standard port is still a service, and guessing from the number
        // is how a connection silently ends up unencrypted. Both branches
        // verify the certificate — `lettre` is built here with no option to
        // skip that.
        let builder = match config.smtp_security {
            MailSecurity::ImplicitTls => {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
            }
            MailSecurity::StartTls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
            }
        }
        .map_err(|_| ProviderError::NotConfigured)?;

        let smtp = builder
            .port(config.smtp_port)
            .credentials(credentials)
            .timeout(Some(IMAP_TIMEOUT))
            .build();

        // Mozilla's root store, compiled in. Deliberately not the system store:
        // the set of roots the Ocinye OS trusts is then the same on a laptop,
        // in CI and on a server, instead of whatever each host happens to have.
        let roots = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        // The provider is named rather than left to `rustls` to discover.
        //
        // The dependency tree carries two — `ring` by way of `lettre`, and
        // `aws-lc-rs` by way of the AWS SDK — so the process-global default is
        // ambiguous and `rustls` panics rather than guess. Naming it here also
        // avoids `install_default()`, which is a one-shot global that would
        // race with anything else in the process trying to set it.
        let tls = ClientConfig::builder_with_provider(Arc::new(
            tokio_rustls::rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| ProviderError::NotConfigured)?
        .with_root_certificates(roots)
        .with_no_client_auth();

        Ok(Self {
            config,
            smtp,
            tls: Arc::new(tls),
        })
    }

    /// Open an authenticated IMAP session.
    ///
    /// # Why a session per operation
    ///
    /// A pooled, long-lived IMAP connection is faster and considerably harder
    /// to get right: sessions carry selected-mailbox state, servers drop them
    /// without warning, and a stale one fails in ways that look like missing
    /// mail. Correctness first; pooling is a change to make with a measurement
    /// in hand (`CLAUDE.md` §71).
    ///
    /// # Errors
    ///
    /// [`ProviderError::Unavailable`] when the host cannot be reached and
    /// [`ProviderError::AuthenticationFailed`] when the credential is refused.
    /// The two are distinct because the operator's next step differs.
    async fn session(&self) -> ProviderResult<ImapSession> {
        let server = ServerName::try_from(self.config.imap_host.clone())
            .map_err(|_| ProviderError::NotConfigured)?;

        let tcp = tokio::time::timeout(
            IMAP_TIMEOUT,
            TcpStream::connect((self.config.imap_host.as_str(), self.config.imap_port)),
        )
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;

        let stream = match self.config.imap_security {
            MailSecurity::ImplicitTls => TlsConnector::from(Arc::clone(&self.tls))
                .connect(server, tcp)
                .await
                .map_err(|_| ProviderError::Unavailable)?,

            // STARTTLS is not implemented, and returning a clear error beats
            // falling back to an unencrypted session. LWS — and every service
            // worth connecting to — offers 993.
            MailSecurity::StartTls => {
                return Err(ProviderError::Rejected(
                    "A ligação IMAP por STARTTLS não está implementada. Configure o \
                     porto 993 com TLS implícito."
                        .to_owned(),
                ))
            }
        };

        let mut client = async_imap::Client::new(stream);

        // The greeting must be consumed before anything else is sent.
        client
            .read_response()
            .await
            .map_err(|_| ProviderError::Unavailable)?
            .ok_or(ProviderError::Unavailable)?;

        client
            .login(&self.config.username, self.config.password.expose())
            .await
            // The server's refusal text can name the account, so it is dropped
            // here rather than carried outward (briefing §57).
            .map_err(|(_error, _client)| ProviderError::AuthenticationFailed)
    }

    /// Close a session politely, ignoring a failure to do so.
    ///
    /// A logout that fails changes nothing about work already done, and
    /// turning it into an error would report a successful fetch as broken.
    async fn release(mut session: ImapSession) {
        let _ = session.logout().await;
    }

    /// The conventional IMAP name for an Ocinye folder.
    ///
    /// A starting guess only. Servers disagree — `Sent`, `Sent Items`,
    /// `INBOX.Sent`, `[Gmail]/Sent Mail` are all real — so
    /// [`Self::resolve_folder`] asks the server what it actually has and falls
    /// back to this.
    ///
    /// `Starred` has no folder of its own: it is a flag, resolved by searching
    /// the inbox.
    const fn imap_name(folder: MailFolder) -> &'static str {
        match folder {
            MailFolder::Inbox | MailFolder::Starred => "INBOX",
            MailFolder::Drafts => "Drafts",
            MailFolder::Sent => "Sent",
            MailFolder::Archive => "Archive",
            MailFolder::Spam => "Junk",
            MailFolder::Trash => "Trash",
        }
    }

    /// The names a server might plausibly use for a folder, in preference
    /// order.
    ///
    /// Matched case-insensitively, and also against a trailing segment, so
    /// `INBOX.Sent` and `INBOX/Sent` resolve as `Sent`.
    const fn candidates(folder: MailFolder) -> &'static [&'static str] {
        match folder {
            MailFolder::Inbox | MailFolder::Starred => &["INBOX"],
            MailFolder::Drafts => &["Drafts", "Draft", "Rascunhos"],
            MailFolder::Sent => &["Sent", "Sent Items", "Sent Messages", "Enviados"],
            MailFolder::Archive => &["Archive", "Archives", "Arquivo"],
            MailFolder::Spam => &["Junk", "Spam", "Junk E-mail", "Lixo eletrónico"],
            MailFolder::Trash => &["Trash", "Deleted Items", "Deleted Messages", "Lixo"],
        }
    }

    /// Parse a provider identifier back into an IMAP UID.
    ///
    /// The identifier is opaque to the domain by contract, so a value that is
    /// not one of ours reads as *not found* rather than as a malformed
    /// request: it names no message either way.
    fn uid(provider_id: &str) -> ProviderResult<u32> {
        provider_id.parse().map_err(|_| ProviderError::NotFound)
    }

    /// Every mailbox the server reports, by name.
    ///
    /// # Errors
    ///
    /// [`ProviderError::Unavailable`] when the listing cannot be read.
    async fn folders(session: &mut ImapSession) -> ProviderResult<Vec<String>> {
        let listing = tokio::time::timeout(IMAP_TIMEOUT, session.list(Some(""), Some("*")))
            .await
            .map_err(|_| ProviderError::Unavailable)?
            .map_err(|_| ProviderError::Unavailable)?;

        let names: Vec<_> = listing
            .try_collect::<Vec<_>>()
            .await
            .map_err(|_| ProviderError::Unavailable)?
            .into_iter()
            .map(|entry| entry.name().to_owned())
            .collect();

        Ok(names)
    }

    /// The name this server uses for an Ocinye folder.
    ///
    /// # Why ask instead of assume
    ///
    /// Hardcoding `Sent` works until it meets a server that says
    /// `INBOX.Sent` — and then sending appears to work while nothing is ever
    /// filed, which is the kind of failure nobody notices for weeks.
    ///
    /// Falls back to the conventional name when nothing matches, so a server
    /// that refuses `LIST` still behaves as it did before.
    fn resolve_folder(available: &[String], folder: MailFolder) -> String {
        for candidate in Self::candidates(folder) {
            // Exact match first.
            if let Some(found) = available
                .iter()
                .find(|name| name.eq_ignore_ascii_case(candidate))
            {
                return found.clone();
            }

            // Then a trailing segment, for hierarchies like `INBOX.Sent`.
            if let Some(found) = available.iter().find(|name| {
                name.rsplit(['.', '/'])
                    .next()
                    .is_some_and(|last| last.eq_ignore_ascii_case(candidate))
            }) {
                return found.clone();
            }
        }

        Self::imap_name(folder).to_owned()
    }

    /// Select the IMAP mailbox behind an Ocinye folder.
    ///
    /// # Errors
    ///
    /// [`ProviderError::NotFound`] when the server has no such mailbox — some
    /// services name `Archive` differently, or do not have one at all, and
    /// that is a missing folder rather than a broken connection.
    async fn select(
        session: &mut ImapSession,
        folder: MailFolder,
    ) -> ProviderResult<async_imap::types::Mailbox> {
        // The inbox is the one name every server agrees on, so it skips the
        // listing round-trip entirely.
        let name = if matches!(folder, MailFolder::Inbox | MailFolder::Starred) {
            "INBOX".to_owned()
        } else {
            let available = Self::folders(session).await.unwrap_or_default();
            Self::resolve_folder(&available, folder)
        };

        tokio::time::timeout(IMAP_TIMEOUT, session.select(&name))
            .await
            .map_err(|_| ProviderError::Unavailable)?
            .map_err(|_| ProviderError::NotFound)
    }

    /// The UIDs for one page of a folder.
    ///
    /// Paging runs downward through the UID space: the newest messages have
    /// the highest UIDs, and `cursor` is the lowest UID already shown. This
    /// stays correct while mail arrives, which offset-based paging does not —
    /// a message delivered between two pages would shift everything and hide a
    /// message the member never saw.
    async fn page_of_uids(
        session: &mut ImapSession,
        folder: MailFolder,
        cursor: Option<&str>,
        limit: u32,
    ) -> ProviderResult<Vec<u32>> {
        let query = match (folder, cursor) {
            // `Starred` is a flag over the inbox, not a folder.
            (MailFolder::Starred, None) => "FLAGGED".to_owned(),
            (MailFolder::Starred, Some(cursor)) => {
                format!("FLAGGED UID 1:{}", Self::uid(cursor)?.saturating_sub(1))
            }
            (_, None) => "ALL".to_owned(),
            (_, Some(cursor)) => format!("UID 1:{}", Self::uid(cursor)?.saturating_sub(1)),
        };

        let found = tokio::time::timeout(IMAP_TIMEOUT, session.uid_search(&query))
            .await
            .map_err(|_| ProviderError::Unavailable)?
            .map_err(|_| ProviderError::Unavailable)?;

        let mut uids: Vec<u32> = found.into_iter().collect();
        uids.sort_unstable_by(|a, b| b.cmp(a));
        uids.truncate(limit as usize);
        Ok(uids)
    }

    /// Whether a fetched row is read, and whether it is flagged.
    fn flags_of(row: &async_imap::types::Fetch) -> (bool, bool) {
        use async_imap::types::Flag;

        let mut is_read = false;
        let mut is_starred = false;
        for flag in row.flags() {
            match flag {
                Flag::Seen => is_read = true,
                Flag::Flagged => is_starred = true,
                _ => {}
            }
        }
        (is_read, is_starred)
    }

    /// Add or remove one IMAP flag on one message.
    async fn store_flag(
        &self,
        folder: MailFolder,
        provider_id: &str,
        flag: &str,
        set: bool,
    ) -> ProviderResult<()> {
        let uid = Self::uid(provider_id)?;
        let mut session = self.session().await?;
        Self::select(&mut session, folder).await?;

        let operation = if set { "+FLAGS" } else { "-FLAGS" };

        let updates = tokio::time::timeout(
            IMAP_TIMEOUT,
            session.uid_store(uid.to_string(), format!("{operation} ({flag})")),
        )
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;

        // The response stream has to be drained before the session is reused
        // or closed, or the next command reads this one's leftovers.
        let _: Vec<_> = updates
            .try_collect()
            .await
            .map_err(|_| ProviderError::Unavailable)?;

        Self::release(session).await;
        Ok(())
    }

    /// Turn an Ocinye address into one `lettre` accepts.
    fn to_lettre(address: &ProviderAddress) -> ProviderResult<LettreMailbox> {
        let parsed = address.address.parse::<lettre::Address>().map_err(|_| {
            ProviderError::Rejected(format!("O endereço «{}» não é válido.", address.address))
        })?;

        Ok(LettreMailbox::new(address.display_name.clone(), parsed))
    }
}

#[async_trait]
impl MailProvider for ImapSmtpProvider {
    fn adapter_name(&self) -> &'static str {
        "imap_smtp"
    }

    async fn health(&self) -> ProviderHealth {
        // Only SMTP is probed: `lettre` exposes a connection test, and opening
        // an IMAP session merely to answer a health question would cost a
        // login on every call. IMAP reachability surfaces through the sync
        // worker's own error state instead (briefing §105).
        let can_send = self.smtp.test_connection().await.unwrap_or(false);

        ProviderHealth {
            // Hosts e portos, para o ecrã de administração. Nunca a credencial.
            endpoints: vec![
                format!("imap {}:{}", self.config.imap_host, self.config.imap_port),
                format!("smtp {}:{}", self.config.smtp_host, self.config.smtp_port),
            ],
            can_read: true,
            can_send,
            detail: if can_send {
                "O serviço de correio está a responder.".to_owned()
            } else {
                "O serviço de envio de correio não está a responder.".to_owned()
            },
        }
    }

    async fn list_messages(
        &self,
        _mailbox_address: &str,
        folder: MailFolder,
        cursor: Option<&str>,
        limit: u32,
    ) -> ProviderResult<MessagePage> {
        let mut session = self.session().await?;

        // `Starred` is a flag rather than a folder: it selects across the
        // inbox instead of naming a mailbox of its own.
        let selected = Self::select(&mut session, folder).await?;

        // An empty mailbox is a page with no messages — not an error, and not
        // something to distinguish from a mailbox that does not exist. The
        // select above already failed if it did not.
        if selected.exists == 0 {
            Self::release(session).await;
            return Ok(MessagePage {
                messages: Vec::new(),
                next_cursor: None,
            });
        }

        let uids = Self::page_of_uids(&mut session, folder, cursor, limit).await?;
        if uids.is_empty() {
            Self::release(session).await;
            return Ok(MessagePage {
                messages: Vec::new(),
                next_cursor: None,
            });
        }

        // Headers and flags only. Fetching whole bodies to draw a list would
        // pull megabytes to render fifty lines, and the index deliberately
        // does not keep bodies anyway (ADR-0407).
        let set = uids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");

        let fetched = tokio::time::timeout(
            IMAP_TIMEOUT,
            session.uid_fetch(&set, "(UID FLAGS RFC822.SIZE BODY.PEEK[HEADER])"),
        )
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;

        let rows: Vec<_> = fetched
            .try_collect()
            .await
            .map_err(|_| ProviderError::Unavailable)?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in &rows {
            let Some(uid) = row.uid else { continue };
            let (is_read, is_starred) = Self::flags_of(row);

            // A header block that will not parse is skipped, not fatal: one
            // malformed message must not empty somebody's inbox.
            let Some(header) = row.header() else { continue };
            let Ok(parsed) = parse_mime(header, &uid.to_string(), folder, is_read, is_starred)
            else {
                tracing::warn!(uid, "message headers could not be parsed; skipped");
                continue;
            };

            let mut header = parsed.header;
            header.size_bytes = row.size.map(i64::from);
            messages.push(header);
        }

        // Newest first, matching what the list shows.
        messages.sort_by_key(|message| std::cmp::Reverse(message.sent_at));

        // The cursor is the lowest UID on this page: the next page is
        // everything below it. Opaque to the domain, by contract.
        let next_cursor = (uids.len() as u32 >= limit)
            .then(|| uids.iter().min().map(|lowest| lowest.to_string()))
            .flatten();

        Self::release(session).await;

        Ok(MessagePage {
            messages,
            next_cursor,
        })
    }

    async fn fetch_message(
        &self,
        _mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
    ) -> ProviderResult<FetchedMessage> {
        let uid = Self::uid(provider_id)?;
        let mut session = self.session().await?;
        Self::select(&mut session, folder).await?;

        // `BODY.PEEK` and not `BODY`: reading a message in the Ocinye
        // Workspace must not silently mark it read on the server. Marking read
        // is its own operation, taken deliberately.
        let fetched = tokio::time::timeout(
            IMAP_TIMEOUT,
            session.uid_fetch(uid.to_string(), "(UID FLAGS RFC822.SIZE BODY.PEEK[])"),
        )
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;

        let rows: Vec<_> = fetched
            .try_collect()
            .await
            .map_err(|_| ProviderError::Unavailable)?;

        let row = rows.first().ok_or(ProviderError::NotFound)?;
        let (is_read, is_starred) = Self::flags_of(row);
        let body = row.body().ok_or(ProviderError::NotFound)?;

        let message = parse_mime(body, provider_id, folder, is_read, is_starred)?;

        Self::release(session).await;
        Ok(message)
    }

    async fn fetch_attachment(
        &self,
        _mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
        part_id: &str,
    ) -> ProviderResult<Vec<u8>> {
        // The whole message is fetched and the part taken from the same parse
        // that produced `part_id`. Asking IMAP for a numbered `BODY[2]`
        // instead would use a different numbering scheme, and could return a
        // part other than the one the member clicked.
        let index: usize = part_id.parse().map_err(|_| ProviderError::NotFound)?;
        let uid = Self::uid(provider_id)?;

        let mut session = self.session().await?;
        Self::select(&mut session, folder).await?;

        let fetched = tokio::time::timeout(
            IMAP_TIMEOUT,
            session.uid_fetch(uid.to_string(), "(UID BODY.PEEK[])"),
        )
        .await
        .map_err(|_| ProviderError::Unavailable)?
        .map_err(|_| ProviderError::Unavailable)?;

        let rows: Vec<_> = fetched
            .try_collect()
            .await
            .map_err(|_| ProviderError::Unavailable)?;

        let row = rows.first().ok_or(ProviderError::NotFound)?;
        let body = row.body().ok_or(ProviderError::NotFound)?;
        let bytes = attachment_bytes(body, index)?;

        Self::release(session).await;
        Ok(bytes)
    }

    async fn send_message(
        &self,
        mailbox_address: &str,
        message: &OutgoingMessage,
    ) -> ProviderResult<Option<String>> {
        // The identity was authorised by the Core before reaching here. This
        // check is the second lock: an adapter must never send as an address
        // the caller did not establish it may use (briefing §29).
        if !message.from.address.eq_ignore_ascii_case(mailbox_address) {
            return Err(ProviderError::Rejected(
                "A identidade de envio não corresponde à mailbox.".to_owned(),
            ));
        }

        let mut builder = Message::builder()
            .from(Self::to_lettre(&message.from)?)
            .subject(&message.subject);

        for recipient in &message.to {
            builder = builder.to(Self::to_lettre(recipient)?);
        }
        for recipient in &message.cc {
            builder = builder.cc(Self::to_lettre(recipient)?);
        }
        for recipient in &message.bcc {
            builder = builder.bcc(Self::to_lettre(recipient)?);
        }

        if let Some(parent) = &message.in_reply_to {
            builder = builder.in_reply_to(parent.clone());
        }
        if !message.references.is_empty() {
            builder = builder.references(message.references.join(" "));
        }

        // A plain-text part, plus one part per attachment. No HTML body: see
        // `OutgoingMessage::body`.
        let built = if message.attachments.is_empty() {
            builder.body(message.body.clone())
        } else {
            let mut multipart =
                MultiPart::mixed().singlepart(SinglePart::plain(message.body.clone()));

            for attachment in &message.attachments {
                let content_type = attachment
                    .content_type
                    .parse::<lettre::message::header::ContentType>()
                    .unwrap_or(lettre::message::header::ContentType::TEXT_PLAIN);

                multipart = multipart.singlepart(
                    lettre::message::Attachment::new(attachment.filename.clone())
                        .body(attachment.content.clone(), content_type),
                );
            }

            builder.multipart(multipart)
        }
        .map_err(|_| {
            ProviderError::Rejected("Não foi possível construir a mensagem.".to_owned())
        })?;

        match self.smtp.send(built).await {
            Ok(response) => {
                // The server's own reply lines are operational detail and stay
                // out of anything a member sees.
                tracing::info!(
                    adapter = self.adapter_name(),
                    accepted = response.is_positive(),
                    "message handed to the mail service"
                );
                Ok(None)
            }
            Err(error) if error.is_transient() => Err(ProviderError::Unavailable),
            Err(error) if error.is_permanent() => Err(ProviderError::Rejected(
                "O serviço de correio recusou a mensagem.".to_owned(),
            )),
            Err(_) => Err(ProviderError::Unavailable),
        }
    }

    async fn move_message(
        &self,
        _mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
        destination: MailFolder,
    ) -> ProviderResult<()> {
        let uid = Self::uid(provider_id)?;
        let mut session = self.session().await?;
        Self::select(&mut session, folder).await?;

        // `UID MOVE` where the server supports it. `async-imap` falls back to
        // copy-then-delete itself, which is the same operation the RFC
        // describes for servers without MOVE.
        let available = Self::folders(&mut session).await.unwrap_or_default();
        let target = Self::resolve_folder(&available, destination);

        tokio::time::timeout(IMAP_TIMEOUT, session.uid_mv(uid.to_string(), &target))
            .await
            .map_err(|_| ProviderError::Unavailable)?
            .map_err(|_| ProviderError::Unavailable)?;

        Self::release(session).await;
        Ok(())
    }

    async fn set_read(
        &self,
        _mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
        read: bool,
    ) -> ProviderResult<()> {
        self.store_flag(folder, provider_id, "\\Seen", read).await
    }

    async fn set_starred(
        &self,
        _mailbox_address: &str,
        folder: MailFolder,
        provider_id: &str,
        starred: bool,
    ) -> ProviderResult<()> {
        self.store_flag(folder, provider_id, "\\Flagged", starred)
            .await
    }
}

/// Parse a fetched MIME message into the shape the domain speaks.
///
/// Separated from the IMAP transport so it can be tested against real MIME
/// without a server — which is where the interesting failures are.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when the bytes are not a parseable
/// message.
pub fn parse_mime(
    raw: &[u8],
    provider_id: &str,
    folder: MailFolder,
    is_read: bool,
    is_starred: bool,
) -> ProviderResult<FetchedMessage> {
    // `MimeHeaders` traz `attachment_name`, `content_type` e `content_id` a
    // `MessagePart`.
    use mail_parser::{MessageParser, MimeHeaders};

    let parsed = MessageParser::default()
        .parse(raw)
        .ok_or(ProviderError::NotFound)?;

    let address_of = |group: Option<&mail_parser::Address<'_>>| -> Vec<ProviderAddress> {
        group
            .map(|addresses| {
                addresses
                    .clone()
                    .into_list()
                    .into_iter()
                    .filter_map(|address| {
                        address.address().map(|email| ProviderAddress {
                            address: email.trim().to_lowercase(),
                            display_name: address.name().map(|name| name.trim().to_owned()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let from = address_of(parsed.from()).into_iter().next().unwrap_or({
        // A message with no parseable sender is not discarded: it is shown with
        // an empty sender, which is itself information.
        ProviderAddress {
            address: String::new(),
            display_name: None,
        }
    });

    let text_body = parsed.body_text(0).map(|body| body.into_owned());
    let html_body = parsed.body_html(0).map(|body| body.into_owned());

    // The thread key comes from `References` — the whole chain, so that a reply
    // to a reply lands in the same conversation — falling back to
    // `In-Reply-To`. **Never from the subject** (briefing §31).
    let thread_key = parsed
        .references()
        .as_text_list()
        .and_then(|list| list.first().map(|id| (*id).to_string()))
        .or_else(|| parsed.in_reply_to().as_text().map(ToOwned::to_owned))
        .or_else(|| parsed.message_id().map(ToOwned::to_owned));

    let attachments: Vec<AttachmentInfo> = parsed
        .attachments()
        .enumerate()
        .map(|(index, part)| AttachmentInfo {
            part_id: index.to_string(),
            filename: part
                .attachment_name()
                .map_or_else(|| "anexo".to_owned(), ToOwned::to_owned),
            content_type: part.content_type().map_or_else(
                || "application/octet-stream".to_owned(),
                |content| {
                    content.subtype().map_or_else(
                        || content.ctype().to_owned(),
                        |sub| format!("{}/{}", content.ctype(), sub),
                    )
                },
            ),
            size_bytes: i64::try_from(part.len()).unwrap_or(0),
            is_inline: part.content_id().is_some(),
        })
        .collect();

    let sent_at: DateTime<Utc> = parsed
        .date()
        .and_then(|date| Utc.timestamp_opt(date.to_timestamp(), 0).single())
        .unwrap_or_else(Utc::now);

    // A snippet for the list. Bounded, and taken from the text alternative so
    // no markup reaches it.
    let snippet = text_body.as_ref().map(|body| {
        let flattened: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
        flattened.chars().take(200).collect()
    });

    Ok(FetchedMessage {
        header: MessageHeader {
            provider_id: provider_id.to_owned(),
            message_id: parsed.message_id().map(str::to_owned),
            thread_key,
            folder,
            from,
            to: address_of(parsed.to()),
            cc: address_of(parsed.cc()),
            subject: parsed.subject().map(str::to_owned),
            snippet,
            sent_at,
            is_read,
            is_starred,
            has_attachments: !attachments.is_empty(),
            size_bytes: i64::try_from(raw.len()).ok(),
        },
        text_body,
        html_body,
        attachments,
        bcc: address_of(parsed.bcc()),
    })
}

/// The decoded bytes of one attachment, by the index `parse_mime` gave it.
///
/// Kept beside [`parse_mime`] and using the same enumeration, so that a
/// `part_id` shown in the interface and a `part_id` resolved here can never
/// drift apart.
///
/// # Errors
///
/// Returns [`ProviderError::NotFound`] when the bytes do not parse or the
/// index names no attachment.
pub fn attachment_bytes(raw: &[u8], index: usize) -> ProviderResult<Vec<u8>> {
    use mail_parser::MessageParser;

    let parsed = MessageParser::default()
        .parse(raw)
        .ok_or(ProviderError::NotFound)?;

    let part = parsed
        .attachments()
        .nth(index)
        .ok_or(ProviderError::NotFound)?;
    Ok(part.contents().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The folder names a server reports are not the ones we would have
    /// guessed, and the mapping has to survive that.
    #[test]
    fn folder_names_come_from_the_server_not_from_a_guess() {
        let lws = [
            "INBOX".to_owned(),
            "INBOX.Sent".to_owned(),
            "INBOX.Drafts".to_owned(),
            "INBOX.Trash".to_owned(),
            "INBOX.Junk".to_owned(),
        ];

        assert_eq!(
            ImapSmtpProvider::resolve_folder(&lws, MailFolder::Sent),
            "INBOX.Sent",
            "uma hierarquia com prefixo não foi reconhecida"
        );
        assert_eq!(
            ImapSmtpProvider::resolve_folder(&lws, MailFolder::Trash),
            "INBOX.Trash"
        );
        assert_eq!(
            ImapSmtpProvider::resolve_folder(&lws, MailFolder::Inbox),
            "INBOX"
        );
    }

    #[test]
    fn a_folder_the_server_does_not_have_falls_back_to_the_convention() {
        // Nem todos os servidores têm Archive. Cair para o nome convencional
        // deixa o erro acontecer no `SELECT`, que devolve `NotFound` — «esta
        // pasta não existe» — em vez de nada acontecer em silêncio.
        let sparse = ["INBOX".to_owned()];
        assert_eq!(
            ImapSmtpProvider::resolve_folder(&sparse, MailFolder::Archive),
            "Archive"
        );
    }

    #[test]
    fn folder_matching_ignores_case_and_language() {
        let localised = ["INBOX".to_owned(), "Enviados".to_owned(), "LIXO".to_owned()];

        assert_eq!(
            ImapSmtpProvider::resolve_folder(&localised, MailFolder::Sent),
            "Enviados"
        );
        assert_eq!(
            ImapSmtpProvider::resolve_folder(&localised, MailFolder::Trash),
            "LIXO"
        );
    }

    #[test]
    fn a_provider_identifier_that_is_not_ours_names_no_message() {
        // Não é um pedido malformado: seja qual for a leitura, não nomeia
        // mensagem nenhuma, e «não encontrada» é a resposta honesta.
        assert!(matches!(
            ImapSmtpProvider::uid("não-é-um-uid"),
            Err(ProviderError::NotFound)
        ));
        assert_eq!(ImapSmtpProvider::uid("42"), Ok(42));
    }

    #[test]
    fn attachment_bytes_are_taken_by_the_same_index_the_interface_shows() {
        const WITH_ATTACHMENT: &[u8] = b"From: a@exemplo.com\r\n\
To: b@ocinye.com\r\n\
Subject: Com anexo\r\n\
Content-Type: multipart/mixed; boundary=LIMITE\r\n\
\r\n\
--LIMITE\r\n\
Content-Type: text/plain\r\n\
\r\n\
Corpo.\r\n\
--LIMITE\r\n\
Content-Type: text/csv\r\n\
Content-Disposition: attachment; filename=\"dados.csv\"\r\n\
\r\n\
a,b,c\r\n\
--LIMITE--\r\n";

        let parsed = parse_mime(WITH_ATTACHMENT, "1", MailFolder::Inbox, false, false).unwrap();
        assert_eq!(parsed.attachments.len(), 1);
        assert_eq!(parsed.attachments[0].filename, "dados.csv");
        assert_eq!(parsed.attachments[0].part_id, "0");

        // O índice que a interface mostra resolve para os bytes certos.
        let bytes = attachment_bytes(WITH_ATTACHMENT, 0).unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("a,b,c"));

        // Um índice que não existe não devolve outro anexo qualquer.
        assert!(matches!(
            attachment_bytes(WITH_ATTACHMENT, 7),
            Err(ProviderError::NotFound)
        ));
    }

    const SIMPLE: &[u8] = b"From: Carlos Silva <carlos@exemplo.com>\r\n\
To: Ana <ana@ocinye.com>\r\n\
Subject: Documentos do projecto\r\n\
Message-ID: <abc123@exemplo.com>\r\n\
Date: Mon, 18 Aug 2026 10:30:00 +0000\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Ola Ana,\r\n\r\nSeguem os documentos.\r\n";

    #[test]
    fn a_plain_message_parses_into_the_domain_shape() {
        let message = parse_mime(SIMPLE, "42", MailFolder::Inbox, false, false).unwrap();

        assert_eq!(message.header.from.address, "carlos@exemplo.com");
        assert_eq!(
            message.header.from.display_name.as_deref(),
            Some("Carlos Silva")
        );
        assert_eq!(
            message.header.subject.as_deref(),
            Some("Documentos do projecto")
        );
        assert_eq!(message.header.to.len(), 1);
        assert_eq!(message.header.to[0].address, "ana@ocinye.com");
        assert!(message.text_body.unwrap().contains("Seguem os documentos"));
        assert!(!message.header.has_attachments);
    }

    #[test]
    fn the_sender_address_is_lower_cased() {
        let raw = b"From: <CARLOS@EXEMPLO.COM>\r\nSubject: x\r\n\r\ncorpo\r\n";
        let message = parse_mime(raw, "1", MailFolder::Inbox, false, false).unwrap();
        assert_eq!(message.header.from.address, "carlos@exemplo.com");
    }

    #[test]
    fn the_thread_key_comes_from_references_and_never_from_the_subject() {
        let with_references = b"From: <a@b.c>\r\n\
Subject: Re: Documentos\r\n\
Message-ID: <resposta@b.c>\r\n\
References: <original@b.c> <segundo@b.c>\r\n\
\r\n\
corpo\r\n";

        let message = parse_mime(with_references, "1", MailFolder::Inbox, false, false).unwrap();
        let key = message.header.thread_key.expect("thread key");

        assert!(key.contains("original@b.c"), "{key}");
        assert!(
            !key.contains("Documentos"),
            "o assunto entrou na chave: {key}"
        );
    }

    #[test]
    fn two_messages_sharing_only_a_subject_are_not_one_thread() {
        // A armadilha clássica: «Re: Reunião» de dois remetentes diferentes.
        let first = b"From: <a@b.c>\r\nSubject: Reuniao\r\nMessage-ID: <um@b.c>\r\n\r\nx\r\n";
        let second = b"From: <d@e.f>\r\nSubject: Reuniao\r\nMessage-ID: <dois@e.f>\r\n\r\ny\r\n";

        let a = parse_mime(first, "1", MailFolder::Inbox, false, false).unwrap();
        let b = parse_mime(second, "2", MailFolder::Inbox, false, false).unwrap();

        assert_ne!(a.header.thread_key, b.header.thread_key);
    }

    #[test]
    fn a_message_with_no_parseable_sender_is_still_readable() {
        let raw = b"Subject: sem remetente\r\n\r\ncorpo\r\n";
        let message = parse_mime(raw, "1", MailFolder::Inbox, false, false).unwrap();
        assert!(message.header.from.address.is_empty());
        assert_eq!(message.header.subject.as_deref(), Some("sem remetente"));
    }

    #[test]
    fn malformed_input_does_not_panic() {
        for raw in [
            &b""[..],
            &b"nao e um email"[..],
            &b"\x00\x01\x02\xff"[..],
            &b"From: <"[..],
        ] {
            // Either it parses into something, or it is refused. Never a panic.
            let _ = parse_mime(raw, "1", MailFolder::Inbox, false, false);
        }
    }

    #[test]
    fn the_snippet_is_bounded_and_carries_no_markup() {
        let long = format!(
            "From: <a@b.c>\r\nSubject: x\r\nContent-Type: text/plain\r\n\r\n{}\r\n",
            "palavra ".repeat(500)
        );
        let message = parse_mime(long.as_bytes(), "1", MailFolder::Inbox, false, false).unwrap();
        let snippet = message.header.snippet.expect("snippet");

        assert!(
            snippet.chars().count() <= 200,
            "{}",
            snippet.chars().count()
        );
        assert!(!snippet.contains('<'));
    }

    #[test]
    fn html_bodies_are_carried_raw_for_the_sanitiser_to_clean() {
        let raw = b"From: <a@b.c>\r\n\
Subject: x\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p>Ola</p><script>alert(1)</script>\r\n";

        let message = parse_mime(raw, "1", MailFolder::Inbox, false, false).unwrap();
        let html = message.html_body.expect("html body");

        // Deliberadamente por limpar: sanear é responsabilidade de `sanitize`,
        // e este tipo transporta a forma crua para que esse passo não possa ser
        // saltado por engano.
        assert!(html.contains("<script>"));

        let cleaned = super::super::sanitize::sanitize_html(&html, false);
        assert!(!cleaned.html.contains("<script>"));
    }

    #[test]
    fn every_ocinye_folder_maps_to_an_imap_name() {
        for folder in MailFolder::all() {
            assert!(!ImapSmtpProvider::imap_name(folder).is_empty());
        }
        // Starred é uma flag, não uma pasta: resolve-se na INBOX.
        assert_eq!(
            ImapSmtpProvider::imap_name(MailFolder::Starred),
            ImapSmtpProvider::imap_name(MailFolder::Inbox)
        );
    }

    #[test]
    fn the_configuration_never_prints_its_credential() {
        let config = ImapSmtpConfig {
            imap_security: MailSecurity::ImplicitTls,
            smtp_security: MailSecurity::ImplicitTls,
            imap_host: "imap.exemplo.com".into(),
            imap_port: 993,
            smtp_host: "smtp.exemplo.com".into(),
            smtp_port: 587,
            username: "ana@ocinye.com".into(),
            password: Secret::new("uma-palavra-passe-verdadeira"),
        };

        let rendered = format!("{config:?}");
        assert!(rendered.contains("imap.exemplo.com"));
        assert!(!rendered.contains("uma-palavra-passe-verdadeira"));
        assert!(rendered.contains("<redacted>"));
    }
}
