//! D004 · Correio: as caixas do membro (`GET /mail/mailboxes`), o índice das
//! mensagens, a leitura e o compositor, na forma do Design.
//!
//! O corpo de uma mensagem é conteúdo externo não confiável: chega do Core já
//! higienizado (`ammonia`) e aqui passa a **texto simples em parágrafos** —
//! nunca HTML na página, nunca autoridade. Enviar é um efeito externo: vai ao
//! Core, e o rascunho fica guardado antes, para que um envio que falha não
//! perca o texto.

use serde_json::Value;

use crate::controllers::desktop::{instant, text, Clock};
use crate::i18n::t;
use crate::ui::view_models::{AppNavVm, MailFolderVm, MailItemVm, MailboxVm};

/// As pastas do Core (`MailFolder`), pela ordem do Design.
pub const FOLDERS: [(MailFolderVm, &str, &str, &str); 7] = [
    (MailFolderVm::Inbox, "inbox", "prod.mail.inbox", "inbox"),
    (
        MailFolderVm::Starred,
        "starred",
        "prod.mail.starred",
        "star",
    ),
    (MailFolderVm::Drafts, "drafts", "prod.mail.drafts", "edit"),
    (MailFolderVm::Sent, "sent", "prod.mail.sent", "send"),
    (
        MailFolderVm::Archive,
        "archive",
        "prod.mail.archive",
        "archive",
    ),
    (MailFolderVm::Spam, "spam", "prod.mail.spam", "warning"),
    (MailFolderVm::Trash, "trash", "prod.mail.trash", "trash"),
];

/// A pasta pedida (`?folder=`).
#[must_use]
pub fn folder_of(f: Option<&str>) -> (MailFolderVm, &'static str, &'static str) {
    FOLDERS.iter().find(|(_, v, _, _)| Some(*v) == f).map_or(
        (MailFolderVm::Inbox, "inbox", "prod.mail.inbox"),
        |(vm, v, k, _)| (*vm, *v, *k),
    )
}

/// As caixas do membro, com as pastas e as contagens de por ler que o Core dá.
#[must_use]
pub(crate) fn mailboxes(
    list: &[Value],
    current_box: &str,
    folder: &str,
    clock: &Clock,
) -> Vec<MailboxVm> {
    // Uma caixa sem credenciais ligadas continua a ser a caixa do membro: o
    // índice e os rascunhos são reais; só sincronizar não se pode.
    list.iter()
        .map(|b| {
            let connected = b.get("connected").and_then(Value::as_bool).unwrap_or(false);
            let id = text(b, "id");
            let unread = |f: &str| {
                b.get("unread")
                    .and_then(Value::as_array)
                    .and_then(|u| u.iter().find(|x| text(x, "folder") == f))
                    .and_then(|x| x.get("unread").and_then(Value::as_u64))
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| *n > 0)
            };
            MailboxVm {
                label: {
                    let name = text(b, "display_name");
                    if name.is_empty() {
                        text(b, "address").to_owned()
                    } else {
                        name.to_owned()
                    }
                },
                folders: FOLDERS
                    .iter()
                    .map(|(vm, v, key, icon)| {
                        (
                            *vm,
                            AppNavVm {
                                label: t(key).to_owned(),
                                icon,
                                href: format!("/mail?box={id}&folder={v}"),
                                count: unread(v),
                                active: id == current_box && *v == folder,
                            },
                        )
                    })
                    .collect(),
                sync_action: connected.then(|| format!("/mail/{id}/sync")),
                synced: instant(b, "last_synced_at").map(|at| clock.when(at)),
            }
        })
        .collect()
}

/// As mensagens do índice (`IndexedMessage`).
#[must_use]
pub(crate) fn items(
    list: &[Value],
    mailbox: &str,
    folder: &str,
    open: Option<&str>,
    clock: &Clock,
) -> Vec<MailItemVm> {
    list.iter()
        .filter_map(|m| {
            let id = text(m, "id");
            if id.is_empty() {
                return None;
            }
            let from = {
                let n = text(m, "from_display_name");
                if n.is_empty() {
                    text(m, "from_address").to_owned()
                } else {
                    n.to_owned()
                }
            };
            Some(MailItemVm {
                id: id.to_owned(),
                from,
                subject: text(m, "subject").to_owned(),
                snippet: text(m, "snippet").to_owned(),
                at: instant(m, "sent_at")
                    .map(|at| clock.when(at))
                    .unwrap_or_default(),
                unread: !m.get("is_read").and_then(Value::as_bool).unwrap_or(true),
                starred: m
                    .get("is_starred")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                attachments: m
                    .get("has_attachments")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                selected: false,
                open: open == Some(id),
                href: format!("/mail/message/{id}?box={mailbox}&folder={folder}"),
                external: false,
            })
        })
        .collect()
}

/// O corpo higienizado pelo Core, como parágrafos de texto simples. As
/// etiquetas saem, as entidades voltam a caracteres, e nada do que era HTML
/// chega à página como HTML (a vista escapa cada parágrafo).
#[must_use]
pub fn paragraphs(html: &str) -> Vec<String> {
    let mut text_out = String::new();
    let mut in_tag = false;
    let mut tag = String::new();
    let mut skip = 0usize;
    for c in html.chars() {
        match (in_tag, c) {
            (false, '<') => {
                in_tag = true;
                tag.clear();
            }
            (true, '>') => {
                in_tag = false;
                let lower = tag.to_ascii_lowercase();
                let name = lower
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if matches!(name, "style" | "script") {
                    if lower.starts_with('/') {
                        skip = skip.saturating_sub(1);
                    } else {
                        skip += 1;
                    }
                }
                if matches!(
                    name,
                    "p" | "div" | "br" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "blockquote"
                ) {
                    text_out.push('\n');
                }
            }
            (true, c) => tag.push(c),
            (false, c) if skip == 0 => text_out.push(c),
            _ => {}
        }
    }
    let decoded = text_out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    decoded
        .split("\n\n")
        .flat_map(|block| block.split('\n'))
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect()
}

/// Os endereços de um campo do compositor («a@x, b@y»).
#[must_use]
pub fn addresses(field: &str) -> Vec<String> {
    field
        .split([',', ';'])
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_corpo_e_texto_e_nunca_html() {
        let p = paragraphs("<p>Olá&nbsp;<b>Ana</b></p><style>x{}</style><p>&lt;script&gt;alert(1)&lt;/script&gt;</p><img src=x onerror=alert(2)>");
        assert_eq!(
            p,
            vec!["Olá Ana".to_owned(), "<script>alert(1)</script>".to_owned()]
        );
    }

    #[test]
    fn enderecos_separados_por_virgula() {
        assert_eq!(
            addresses(" a@x.pt, b@y.ao ;c@z "),
            vec!["a@x.pt", "b@y.ao", "c@z"]
        );
        assert!(addresses(" , ").is_empty());
    }
}
