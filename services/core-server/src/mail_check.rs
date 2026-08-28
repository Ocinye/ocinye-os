//! `ocinye-core-server mail-check` — a connectivity probe for the mail service.
//!
//! # Why this exists as a command
//!
//! The first question when wiring a mail service is «do the credentials
//! work?», and answering it must not require starting the whole Core, creating
//! a member, and clicking through the Workspace. It also must not require
//! anyone to paste a password into a terminal that keeps history.
//!
//! # What it never prints
//!
//! The password, the server's authentication response, message subjects,
//! senders, bodies. It prints hosts, ports, transport security, folder
//! **names**, and counts (briefing §57).
//!
//! It is safe to run against a production mailbox and safe to paste the output
//! into a ticket.
//!
//! # What it never does
//!
//! Send anything. Diagnosing an outbound path by sending mail to somebody is
//! how test messages reach real people; SMTP is probed with a connection and
//! authentication handshake, which is what actually fails when it fails.

use anyhow::{bail, Context};
use ocinye_contracts::MailFolder;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::mail::imap_smtp::{ImapSmtpConfig, ImapSmtpProvider};
use ocinye_core::modules::mail::MailProvider;
use ocinye_core::password::Secret;

/// An adapter failure, as an operator sentence with a next step.
fn explain(error: &ocinye_core::modules::mail::ProviderError) -> String {
    use ocinye_core::modules::mail::ProviderError as E;

    match error {
        E::NotConfigured => "o adaptador não foi construído; verifique o anfitrião".to_owned(),
        E::Unavailable => {
            "sem resposta — verifique anfitrião, porto, e se a rede permite a saída".to_owned()
        }
        E::AuthenticationFailed => {
            "credenciais recusadas — confirme que o utilizador é o endereço de email \
             completo, e que a conta permite acesso IMAP"
                .to_owned()
        }
        E::NotFound => "a pasta não existe neste servidor".to_owned(),
        E::TooLarge => "a resposta excedeu o limite aceite".to_owned(),
        // Carries text the adapter itself wrote, never the server's.
        E::Rejected(reason) => reason.clone(),
    }
}

/// Run the probe.
///
/// # Errors
///
/// Returns an error when mail is not configured, or when the probe fails in a
/// way the operator must act on.
pub async fn run() -> anyhow::Result<()> {
    let config = CoreConfig::from_env().context("configuration")?;

    if !config.mail.is_configured() {
        bail!(
            "Ocinye Mail is not configured.\n\
             Set OCINYE_MAIL_IMAP_HOST, OCINYE_MAIL_SMTP_HOST, OCINYE_MAIL_USERNAME and \
             OCINYE_MAIL_PASSWORD.\n\
             See docs/runbooks/configure-mail-service.md."
        );
    }

    let mail = &config.mail;

    println!("Ocinye Mail — verificação de ligação");
    println!("────────────────────────────────────");
    println!(
        "  IMAP      {}:{} ({})",
        mail.imap_host,
        mail.imap_port,
        mail.imap_security.as_str()
    );
    println!(
        "  SMTP      {}:{} ({})",
        mail.smtp_host,
        mail.smtp_port,
        mail.smtp_security.as_str()
    );
    // The account is an address, not a secret — and knowing which account was
    // probed is most of the value when a result is surprising.
    println!("  Conta     {}", mail.username);
    println!("  Domínios  {}", mail.institutional_domains.join(", "));
    println!("  Password  <definida, não impressa>");
    println!();

    let provider = ImapSmtpProvider::new(ImapSmtpConfig {
        imap_host: mail.imap_host.clone(),
        imap_port: mail.imap_port,
        imap_security: mail.imap_security,
        smtp_host: mail.smtp_host.clone(),
        smtp_port: mail.smtp_port,
        smtp_security: mail.smtp_security,
        username: mail.username.clone(),
        password: Secret::new(mail.password.clone()),
    })
    .map_err(|error| anyhow::anyhow!("adapter could not be built: {error}"))?;

    // ── SMTP ────────────────────────────────────────────────────────────
    //
    // A connection and authentication handshake. Nothing is sent: probing an
    // outbound path by emailing somebody is how test messages reach real
    // people.
    let health = provider.health().await;
    if health.can_send {
        println!("  ✓ SMTP    ligação e autenticação aceites");
    } else {
        println!("  ✗ SMTP    sem resposta, ou credenciais recusadas");
    }

    // ── IMAP ────────────────────────────────────────────────────────────
    //
    // Listing the inbox is the smallest operation that exercises the whole
    // chain: TCP, TLS, LOGIN, SELECT, SEARCH, FETCH.
    match provider
        .list_messages(&mail.username, MailFolder::Inbox, None, 1)
        .await
    {
        Ok(page) => {
            println!("  ✓ IMAP    ligação, autenticação e INBOX acessíveis");
            // A count, never a subject or a sender.
            println!(
                "            {} mensagem(ns) na primeira página",
                page.messages.len()
            );
        }
        Err(error) => {
            // The adapter's error already carries no server text. Translated
            // here into the operator's language, with the next step attached:
            // somebody diagnosing this at midnight needs to know what to
            // change, not what the enum is called.
            println!("  ✗ IMAP    {}", explain(&error));
        }
    }

    println!();

    // ── Folders ─────────────────────────────────────────────────────────
    //
    // The names this server uses, which is what decides whether Sent, Drafts
    // and Trash resolve. Folder names are not correspondence.
    println!("  Pastas encontradas neste servidor:");
    let mut any = false;
    for folder in MailFolder::all() {
        if folder == MailFolder::Starred {
            continue; // A flag, not a folder.
        }
        match provider
            .list_messages(&mail.username, folder, None, 1)
            .await
        {
            Ok(_) => {
                println!("    ✓ {:<20} {}", folder.label(), folder.as_str());
                any = true;
            }
            Err(_) => println!("    – {:<20} ausente ou inacessível", folder.label()),
        }
    }

    if !any {
        println!("    nenhuma pasta pôde ser aberta");
    }

    println!();
    if health.can_send {
        println!("Envio disponível. Leitura conforme indicado acima.");
    } else {
        println!(
            "Envio indisponível. Verifique porto e segurança de SMTP, e se a conta \
             permite autenticação."
        );
    }
    println!("Nenhuma mensagem foi enviada por esta verificação.");

    Ok(())
}
