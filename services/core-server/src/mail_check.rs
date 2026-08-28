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

    if !config.mail.transport_configured() {
        bail!(
            "Ocinye Mail is not configured.\n\
             Set OCINYE_MAIL_IMAP_HOST and OCINYE_MAIL_SMTP_HOST.\n\
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
    println!("  Domínios  {}", mail.institutional_domains.join(", "));
    // A conta é um endereço, não um segredo — e saber qual foi sondada é
    // metade do valor quando o resultado surpreende.
    if mail.has_institutional_credential() {
        println!("  Conta     {}", mail.username);
        println!("  Password  <definida, não impressa>");
    } else {
        println!("  Conta     <sem conta de serviço institucional>");
    }
    println!();

    // ── Nome e cifra, antes de credenciais ──────────────────────────────
    //
    // Uma autenticação recusada e um nome que não resolve leem-se da mesma
    // maneira no fim — «não funcionou» — e mandam quem administra mexer em
    // sítios diferentes. Separá-los aqui é o que transforma esta saída num
    // diagnóstico em vez de um veredicto.
    let mut alcance_ok = true;
    for (etiqueta, host, porto) in [
        ("IMAP", mail.imap_host.as_str(), mail.imap_port),
        ("SMTP", mail.smtp_host.as_str(), mail.smtp_port),
    ] {
        match resolver(host, porto).await {
            Ok(enderecos) if enderecos.is_empty() => {
                println!("  ✗ DNS     {etiqueta} {host} não resolve para nenhum endereço");
                alcance_ok = false;
            }
            Ok(enderecos) => {
                // Endereços IP de um serviço de correio público não são
                // segredo, e sabê-los é metade do diagnóstico quando um nome
                // aponta para o sítio errado.
                println!("  ✓ DNS     {etiqueta} {host} → {}", enderecos.join(", "));
            }
            Err(motivo) => {
                println!("  ✗ DNS     {etiqueta} {host}: {motivo}");
                alcance_ok = false;
            }
        }
    }
    println!();

    // ── Sem conta de serviço, o diagnóstico pára aqui ───────────────────
    //
    // E pára honestamente. Autenticar exige uma credencial, e a única que
    // existe nesta instalação é a de cada membro — cifrada, e a abrir apenas
    // quando é essa pessoa a agir (ADR-0409). Usá-la aqui seria pegar na senha
    // de alguém para responder a uma pergunta sobre a instituição.
    //
    // Isto não é uma falha: o transporte foi verificado, que é o que uma
    // instalação assim tem para verificar.
    if !mail.has_institutional_credential() {
        println!("  – Autenticação não verificada");
        println!("    Esta instalação não tem conta de serviço institucional.");
        println!("    Cada membro liga a sua caixa em Correio → Definições, e é aí");
        println!("    que a credencial dele é experimentada antes de ser guardada.");
        println!();
        if !alcance_ok {
            bail!("o transporte de correio não está alcançável; ver as linhas acima");
        }
        println!("Transporte alcançável. Nenhuma mensagem foi enviada por esta verificação.");
        return Ok(());
    }

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
    let mut leitura_ok = false;
    match provider
        .list_messages(&mail.username, MailFolder::Inbox, None, 1)
        .await
    {
        Ok(page) => {
            leitura_ok = true;
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

    // O estado de saída é a autoridade sobre o resultado, e não este texto.
    //
    // Sem isto, um `mail-check` que falhasse em tudo saía com zero: um script
    // que o encadeasse com `&&` continuaria, e uma CI dá-lo-ia por bom. Quem
    // decide é o código de saída.
    if !alcance_ok || !health.can_send || !leitura_ok {
        bail!("a verificação de correio não passou; ver as linhas acima");
    }

    Ok(())
}

/// Os endereços a que um nome de anfitrião resolve.
///
/// Só resolução: nada se liga aqui. Um nome que não resolve é um problema de
/// DNS, e não de credenciais — e é a distinção que decide onde se vai mexer.
async fn resolver(host: &str, porto: u16) -> Result<Vec<String>, String> {
    tokio::net::lookup_host((host, porto))
        .await
        .map(|enderecos| {
            enderecos
                .map(|endereco| endereco.ip().to_string())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        })
        .map_err(|erro| erro.to_string())
}
