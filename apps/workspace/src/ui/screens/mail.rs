//! Ocinye Mail — a superfície humana do correio institucional.
//!
//! # Três decisões estruturais deste ecrã
//!
//! **O único sítio do Workspace onde entra HTML alheio.** O corpo de uma
//! mensagem é escrito por quem a enviou. Chega aqui já limpo pelo Ocinye Core
//! (`ocinye_core::modules::mail::sanitize`) e é o único `inner_html` da
//! interface. Nenhum outro ecrã injecta markup que não tenha construído.
//!
//! **Gerar não é enviar.** O composer é um formulário com dois botões de
//! submissão e destinos diferentes: `/mail/assist` devolve texto para o campo,
//! `/mail/send` entrega ao serviço de correio. Não é uma convenção — são duas
//! rotas, e a de assistência não tem forma de chamar a outra (briefing §15).
//!
//! **Conteúdo remoto não carrega sozinho.** Uma imagem remota diz a quem
//! enviou a mensagem que ela foi aberta, e a que horas. O Core substitui-as e
//! conta-as; este ecrã mostra a contagem e um botão para as carregar
//! explicitamente (briefing §12).

use leptos::prelude::*;
use serde_json::Value;

use ocinye_contracts::{ComposeAction, RemoteContentPolicy};

use crate::ui::components::{
    badge, button, card, empty_state, field_with_value, section_head, select, select_labelled,
    textarea_with_value, Button, EmptyState, SelectOption, Tone, Variant,
};
use crate::ui::icon::{icon, Icon};
use crate::ui::shell::Viewer;

/// O contexto de correio partilhado por todos os ecrãs desta família.
pub struct MailView {
    /// `GET /api/v1/mail/status`.
    pub status: Value,
    /// O resultado da última actualização pedida nesta visita, se houve.
    ///
    /// Mostrado como confirmação: sem ele, carregar em «Actualizar» e não ver
    /// nada de novo é indistinguível de o botão não funcionar (briefing §100).
    pub sync_notice: Option<String>,
    /// `GET /api/v1/mail/mailboxes`.
    pub mailboxes: Value,
    /// A caixa aberta, quando alguma.
    pub active_mailbox: Option<String>,
    /// A pasta aberta.
    pub folder: String,
    /// O termo pesquisado, se algum.
    pub query: String,
}

impl MailView {
    /// Se o serviço consegue ler correio.
    fn can_read(&self) -> bool {
        self.status
            .get("can_read")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Se o serviço consegue enviar.
    ///
    /// Distinto de ler: IMAP e SMTP são serviços diferentes e falham em
    /// separado (briefing §105).
    fn can_send(&self) -> bool {
        self.status
            .get("can_send")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// O que o Core disse sobre o estado do serviço.
    fn detail(&self) -> String {
        self.status
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or(
                "O estado do correio institucional não pôde ser determinado. Nada foi \
                 enviado.",
            )
            .to_owned()
    }

    /// Se a assistência de escrita pode servir um pedido agora.
    fn ai_available(&self) -> bool {
        self.status
            .get("ai_assist_available")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Se este membro pode sequer usar a assistência.
    ///
    /// Separado do anterior de propósito: não poder e não haver são coisas
    /// diferentes, e a interface deve dizer qual é (briefing §61).
    fn may_use_ai(&self) -> bool {
        self.status
            .get("may_use_ai")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn boxes(&self) -> &[Value] {
        self.mailboxes.as_array().map_or(&[], Vec::as_slice)
    }

    /// A caixa aberta, ou a primeira a que o membro tem acesso.
    fn current(&self) -> Option<&Value> {
        self.active_mailbox.as_ref().map_or_else(
            || self.boxes().first(),
            |wanted| {
                self.boxes()
                    .iter()
                    .find(|mailbox| mailbox.get("id").and_then(Value::as_str) == Some(wanted))
            },
        )
    }
}

/// Texto de um campo JSON, com alternativa.
fn text<'a>(value: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or(fallback)
}

// ── O ecrã principal ────────────────────────────────────────────────────

/// A caixa de correio: identidades, pastas, lista e painel de leitura.
///
/// `open` é a mensagem aberta, quando alguma. Um único ecrã em vez de dois
/// porque o design mantém a lista visível ao ler — sair da lista para ler e
/// voltar a entrar para ler a seguinte é o que torna um cliente de correio
/// cansativo (`CLAUDE.md` §47).
pub fn mail(
    viewer: &Viewer,
    view: &MailView,
    messages: &Value,
    open: Option<&Value>,
) -> impl IntoView {
    let unavailable = !view.can_read();
    let detail = view.detail();
    let boxes = view.boxes().to_vec();

    // Sem serviço configurado não há caixas, e a lista de pastas com contagens
    // a zero pareceria uma caixa vazia em vez de um serviço ausente. São
    // estados diferentes e o membro tem de os distinguir (briefing §60).
    if unavailable && boxes.is_empty() {
        return unavailable_screen(&detail).into_any();
    }

    let current = view.current().cloned().unwrap_or(Value::Null);
    let current_id = text(&current, "id", "").to_owned();
    let folder = view.folder.clone();
    let query = view.query.clone();
    let can_compose = view.can_send()
        && current
            .get("may_send")
            .and_then(Value::as_bool)
            .unwrap_or(false);

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Correio"</h1>
                    <p>
                        "O correio institucional da Ocinye, dentro do Ocinye Workspace."
                    </p>
                </div>
                <div class="oc-head__actions">
                    {sync_action(&current_id, view.can_read(), &folder)}
                    {compose_action(can_compose, &current_id, view.can_send())}
                    {button(Button::new("Definições", Variant::Secondary).href("/mail/settings"))}
                </div>
            </div>

            {view.sync_notice.as_ref().map(|notice| view! {
                <div class="oc-callout oc-mail__banner" role="status">
                    {icon(Icon::Restart, 15)}
                    <p>{notice.clone()}</p>
                </div>
            })}

            {(!view.can_send()).then(|| service_notice(&detail))}

            <div class="oc-mail">
                {rail(&boxes, &current_id, &folder)}
                {list(&current_id, &folder, &query, messages, open)}
                {open.map_or_else(
                    || reading_placeholder().into_any(),
                    |message| reading(viewer, view, message).into_any(),
                )}
            </div>
        </div>
    }
    .into_any()
}

/// Actualizar esta pasta a partir do serviço de correio.
///
/// Explícito, e não automático, porque é isso que o Ocinye OS faz hoje: não
/// existe processo que actualize o correio recebido sozinho. Um botão que
/// existe diz a verdade sobre isso; uma lista que nunca muda não
/// (`CLAUDE.md` §69).
fn sync_action(mailbox_id: &str, service_up: bool, folder: &str) -> impl IntoView {
    if !service_up || mailbox_id.is_empty() {
        return view! {
            <span class="oc-btn oc-btn--secondary oc-unavailable" aria-disabled="true"
                  title="O serviço de correio não está disponível.">
                "Actualizar"
            </span>
        }
        .into_any();
    }

    let action = format!("/mail/{mailbox_id}/sync");
    view! {
        // Formulário e não ligação: actualizar altera estado, e um `GET` que
        // altera estado é actualizado por qualquer pré-carregamento do browser.
        <form method="post" action=action class="oc-mail__sync">
            <input type="hidden" name="folder" value=folder.to_owned() />
            <button type="submit" class="oc-btn oc-btn--secondary">
                {icon(Icon::Restart, 13)}
                "Actualizar"
            </button>
        </form>
    }
    .into_any()
}

/// O botão de escrever, ou a razão pela qual não está disponível.
fn compose_action(can_compose: bool, mailbox_id: &str, service_up: bool) -> impl IntoView {
    if can_compose {
        button(
            Button::new("Escrever", Variant::Primary)
                .href(format!("/mail/compose?mailbox={mailbox_id}")),
        )
        .into_any()
    } else {
        // Visível e declarado, não escondido: quem não vê o botão conclui que a
        // funcionalidade não existe (briefing §53).
        let reason = if service_up {
            "Não possui autorização para enviar a partir desta caixa."
        } else {
            "O serviço de envio não está disponível."
        };
        view! {
            <span class="oc-btn oc-btn--primary oc-unavailable" aria-disabled="true" title=reason>
                "Escrever"
            </span>
        }
        .into_any()
    }
}

/// O aviso de serviço, quando o correio não pode ser enviado.
fn service_notice(detail: &str) -> impl IntoView {
    let detail = detail.to_owned();
    view! {
        <div class="oc-callout oc-callout--warning oc-mail__banner" role="status">
            {icon(Icon::Shield, 15)}
            <p>{detail}</p>
        </div>
    }
}

/// O ecrã inteiro, quando o correio não está configurado nesta instalação.
///
/// Uma caixa de entrada vazia seria uma mentira: sugeriria que não há
/// mensagens, quando o que não há é serviço (`CLAUDE.md` §69).
fn unavailable_screen(detail: &str) -> impl IntoView {
    let detail = detail.to_owned();
    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Correio"</h1>
                    <p>"Correio institucional da Ocinye."</p>
                </div>
            </div>

            {empty_state(EmptyState {
                icon: Icon::Mail,
                title: "O correio institucional não está configurado".to_owned(),
                body: format!(
                    "{detail} Esta é uma situação de configuração da instalação, não uma \
                     falha do seu acesso. Quem administra o Ocinye OS pode activá-lo."
                ),
                actions: vec![Button::new("Administração", Variant::Secondary).href("/admin")],
                small: false,
            })}
        </div>
    }
}

// ── Coluna 1: identidades e pastas ──────────────────────────────────────

fn rail(boxes: &[Value], current_id: &str, folder: &str) -> impl IntoView {
    let boxes = boxes.to_vec();
    let current_id = current_id.to_owned();
    let folder = folder.to_owned();

    view! {
        <nav class="oc-mail__rail" aria-label="Caixas de correio">
            {boxes
                .into_iter()
                .map(|mailbox| {
                    let id = text(&mailbox, "id", "").to_owned();
                    let active = id == current_id;
                    let shared = text(&mailbox, "kind", "personal") == "shared";
                    let address = text(&mailbox, "address", "").to_owned();
                    let name = mailbox
                        .get("display_name")
                        .and_then(Value::as_str)
                        .unwrap_or(&address)
                        .to_owned();
                    let sync_error = mailbox
                        .get("last_sync_error")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let folders = mailbox
                        .get("unread")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let folder = folder.clone();

                    view! {
                        <div class="oc-mail__box" data-active=active.to_string()>
                            <div class="oc-mail__box-head">
                                <span class="oc-mail__box-name">{name}</span>
                                {shared.then(|| badge("Partilhada", Tone::Gray))}
                            </div>
                            <span class="oc-mail__box-address oc-mono">{address}</span>

                            // Uma falha de sincronização aparece onde a caixa
                            // aparece. Escondê-la faria a lista parecer
                            // actualizada quando não está (briefing §100).
                            {sync_error.map(|reason| view! {
                                <p class="oc-mail__box-error" role="status">{reason}</p>
                            })}

                            <ul class="oc-mail__folders">
                                {folders
                                    .into_iter()
                                    .map(|entry| {
                                        let key = text(&entry, "folder", "inbox").to_owned();
                                        let label = text(&entry, "label", "").to_owned();
                                        let unread = entry
                                            .get("unread")
                                            .and_then(Value::as_i64)
                                            .unwrap_or(0);
                                        let selected = active && key == folder;
                                        let href = format!("/mail/{id}?folder={key}");

                                        view! {
                                            <li>
                                                <a
                                                    class="oc-mail__folder"
                                                    href=href
                                                    aria-current=selected.then_some("page")
                                                >
                                                    <span>{label}</span>
                                                    {(unread > 0).then(|| view! {
                                                        <span class="oc-mail__count">{unread}</span>
                                                    })}
                                                </a>
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        </div>
                    }
                })
                .collect_view()}
        </nav>
    }
}

// ── Coluna 2: a lista de mensagens ──────────────────────────────────────

fn list(
    mailbox_id: &str,
    folder: &str,
    query: &str,
    messages: &Value,
    open: Option<&Value>,
) -> impl IntoView {
    let items = messages
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mailbox_id = mailbox_id.to_owned();
    let folder = folder.to_owned();
    let action = format!("/mail/{mailbox_id}");
    let open_id = open
        .and_then(|message| message.get("message"))
        .and_then(|message| message.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let searching = !query.is_empty();
    let count = items.len();

    view! {
        <div class="oc-mail__list">
            // Pesquisa por GET, dentro da caixa aberta. O Core filtra por
            // pertença antes de devolver — pesquisar não é forma de alcançar
            // correio alheio (`CLAUDE.md` §28).
            <form class="oc-mail__search" method="get" action=action role="search">
                <input type="hidden" name="folder" value=folder.clone() />
                <label class="oc-sr" for="mail-q">"Pesquisar nesta caixa"</label>
                <span class="oc-mail__search-icon">{icon(Icon::Search, 14)}</span>
                <input
                    class="oc-input"
                    id="mail-q"
                    name="q"
                    type="search"
                    value=query.to_owned()
                    placeholder="Pesquisar nesta caixa…"
                />
                <button type="submit" class="oc-btn oc-btn--secondary">"Pesquisar"</button>
            </form>

            {searching.then(|| view! {
                <p class="oc-mail__result-count" role="status">
                    {format!("{count} resultado(s) para a pesquisa.")}
                </p>
            })}

            {if items.is_empty() {
                empty_state(EmptyState {
                    icon: Icon::Mail,
                    title: if searching {
                        "Nenhuma mensagem corresponde".to_owned()
                    } else {
                        "Nenhuma mensagem nesta pasta".to_owned()
                    },
                    body: if searching {
                        "Nenhuma mensagem desta caixa corresponde ao termo pesquisado."
                            .to_owned()
                    } else {
                        "Esta pasta não tem mensagens indexadas no Ocinye OS.".to_owned()
                    },
                    actions: Vec::new(),
                    small: true,
                })
                .into_any()
            } else {
                view! {
                    <ul class="oc-mail__items">
                        {items
                            .into_iter()
                            .map(|message| row(&message, &open_id))
                            .collect_view()}
                    </ul>
                }
                .into_any()
            }}
        </div>
    }
}

fn row(message: &Value, open_id: &str) -> impl IntoView {
    let id = text(message, "id", "").to_owned();
    let selected = id == open_id;
    let unread = !message
        .get("is_read")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let starred = message
        .get("is_starred")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // O nome escolhido por quem envia aparece ao lado do endereço, nunca em
    // vez dele: um remetente que se apresenta como «Ocinye Suporte» a partir
    // de um domínio qualquer não deve poder esconder o domínio (briefing §14).
    let from = text(message, "from_display_name", "").to_owned();
    let address = text(message, "from_address", "(remetente desconhecido)").to_owned();
    let from = if from.is_empty() {
        address.clone()
    } else {
        from
    };
    let subject = text(message, "subject", "(sem assunto)").to_owned();
    let preview = text(message, "snippet", "").to_owned();
    let when = short_date(text(message, "sent_at", ""));
    let has_attachments = message
        .get("has_attachments")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let href = format!("/mail/message/{id}");

    view! {
        <li>
            <a
                class="oc-mail__item"
                href=href
                data-unread=unread.to_string()
                aria-current=selected.then_some("page")
            >
                <div class="oc-mail__item-top">
                    <span class="oc-mail__from">{from}</span>
                    <span class="oc-mail__address oc-mono">{address}</span>
                    <span class="oc-mail__when oc-mono">{when}</span>
                </div>
                <div class="oc-mail__item-mid">
                    // Não-lida marcada por peso e por marca, nunca só por cor
                    // (`CLAUDE.md` §51).
                    {unread.then(|| view! { <span class="oc-mail__dot" aria-label="Não lida"></span> })}
                    <span class="oc-mail__subject">{subject}</span>
                    {starred.then(|| view! {
                        <span class="oc-mail__star" aria-label="Assinalada">
                            {icon(Icon::Star, 12)}
                        </span>
                    })}
                    {has_attachments.then(|| view! {
                        <span class="oc-mail__clip" aria-label="Tem anexos">
                            {icon(Icon::Attach, 12)}
                        </span>
                    })}
                </div>
                <p class="oc-mail__preview">{preview}</p>
            </a>
        </li>
    }
}

/// A coluna de leitura antes de se abrir alguma coisa.
fn reading_placeholder() -> impl IntoView {
    view! {
        <div class="oc-mail__pane oc-mail__pane--empty">
            {empty_state(EmptyState {
                icon: Icon::Mail,
                title: "Seleccione uma mensagem".to_owned(),
                body: "O conteúdo aparece aqui. Imagens e conteúdo remoto não são \
                       carregados automaticamente."
                    .to_owned(),
                actions: Vec::new(),
                small: true,
            })}
        </div>
    }
}

// ── Coluna 3: a leitura ─────────────────────────────────────────────────

fn reading(viewer: &Viewer, view: &MailView, payload: &Value) -> impl IntoView {
    let message = payload.get("message").cloned().unwrap_or(Value::Null);
    let id = text(&message, "id", "").to_owned();
    let subject = text(&message, "subject", "(sem assunto)").to_owned();
    let from_display = text(&message, "from_display_name", "").to_owned();
    let from_address = text(&message, "from_address", "").to_owned();
    let when = short_date(text(&message, "sent_at", ""));
    let starred = message
        .get("is_starred")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let body_html = payload
        .get("body_html")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let blocked = payload
        .get("blocked_remote_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let domains: Vec<String> = payload
        .get("linked_domains")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let attachments = payload
        .get("attachments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let recipients = addresses(payload, "to");
    let copies = addresses(payload, "cc");

    let can_reply = view.can_send();
    let reply_href = format!("/mail/compose?reply={id}");
    let _ = viewer;

    view! {
        <article class="oc-mail__pane">
            <header class="oc-mail__pane-head">
                <h2>{subject}</h2>
                <div class="oc-mail__meta">
                    <span class="oc-mail__from">
                        {if from_display.is_empty() { from_address.clone() } else { from_display }}
                    </span>
                    <span class="oc-mono oc-mail__address">{from_address}</span>
                    <span class="oc-mono oc-mail__when">{when}</span>
                </div>
                {(!recipients.is_empty()).then(|| view! {
                    <p class="oc-mail__recipients">"Para: "<span class="oc-mono">{recipients}</span></p>
                })}
                {(!copies.is_empty()).then(|| view! {
                    <p class="oc-mail__recipients">"Cc: "<span class="oc-mono">{copies}</span></p>
                })}

                <div class="oc-mail__actions">
                    {if can_reply {
                        button(Button::new("Responder", Variant::Primary).href(reply_href)).into_any()
                    } else {
                        view! {
                            <span
                                class="oc-btn oc-btn--primary oc-unavailable"
                                aria-disabled="true"
                                title="O serviço de envio não está disponível."
                            >"Responder"</span>
                        }
                        .into_any()
                    }}
                    {flag_form(&id, "starred", !starred, if starred {
                        "Retirar destaque"
                    } else {
                        "Assinalar"
                    }, Icon::Star)}
                    {flag_form(&id, "read", false, "Marcar como não lida", Icon::Mail)}
                </div>
            </header>

            {(blocked > 0).then(|| remote_banner(&id, blocked))}

            // O único `inner_html` do Ocinye Workspace. O conteúdo vem
            // higienizado do Ocinye Core, por lista de permissões: sem
            // `<script>`, sem `on*`, sem `javascript:`, sem `<iframe>`,
            // sem `<form>`. Ver `ocinye_core::modules::mail::sanitize`.
            <div class="oc-mail__body" inner_html=body_html></div>

            {(!attachments.is_empty()).then(|| attachment_list(&attachments))}

            {(!domains.is_empty()).then(|| view! {
                <p class="oc-mail__domains">
                    "Esta mensagem liga para: "
                    <span class="oc-mono">{domains.join(", ")}</span>
                </p>
            })}
        </article>
    }
}

/// Uma lista de endereços num campo do payload.
fn addresses(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// Um botão que altera um estado da mensagem.
///
/// Formulário e não script: funciona sem JavaScript, e o `POST` deixa rasto no
/// Core em vez de mudar apenas o que se vê.
fn flag_form(
    id: &str,
    field: &'static str,
    value: bool,
    label: &'static str,
    kind: Icon,
) -> impl IntoView {
    let action = format!("/mail/message/{id}/flags");
    view! {
        <form method="post" action=action class="oc-mail__flag">
            <input type="hidden" name="field" value=field />
            <input type="hidden" name="value" value=value.to_string() />
            <button type="submit" class="oc-btn oc-btn--secondary" title=label>
                {icon(kind, 13)}
                {label}
            </button>
        </form>
    }
}

/// O aviso de conteúdo remoto bloqueado.
///
/// # Porque não há botão para o carregar
///
/// Havia, e não fazia nada. O botão ia a `?remote=1`, o Ocinye Core devolvia o
/// corpo com os `src` originais — e a Content Security Policy do Workspace, que
/// declara `img-src 'self' data:`, recusava cada um deles. A página recarregava,
/// o aviso desaparecia porque já nada estava por carregar, e as imagens
/// continuavam a não aparecer. O membro ficava com a impressão de que o pedido
/// tinha sido atendido.
///
/// Das duas saídas possíveis, alargar a CSP a origens de terceiros seria
/// desmontar a última barreira contra o rastreio por email para repor um botão.
/// A outra — servir o conteúdo remoto através do Ocinye, sem contactar o
/// remetente a partir do browser do membro — é uma funcionalidade por
/// construir, não uma correcção.
///
/// Fica o estado, dito por inteiro. Um botão que finge é pior do que uma
/// ausência declarada (`CLAUDE.md` §69, briefing §66).
fn remote_banner(_id: &str, blocked: u64) -> impl IntoView {
    view! {
        <div class="oc-callout oc-mail__banner" role="status">
            {icon(Icon::Shield, 15)}
            <p>
                {format!(
                    "{blocked} elemento(s) remoto(s) não foram carregados. O Ocinye \
                     Workspace não vai buscar conteúdo a servidores de terceiros: \
                     fazê-lo informaria quem enviou esta mensagem de que ela foi aberta."
                )}
            </p>
        </div>
    }
}

/// Os anexos, descritos.
///
/// Sem descarregamento nesta fase: o Ocinye OS não tem armazenamento de
/// objectos configurado, e um botão que falhasse ao ser carregado seria pior do
/// que um estado declarado (briefing §60, `CLAUDE.md` §69).
fn attachment_list(attachments: &[Value]) -> impl IntoView {
    let attachments = attachments.to_vec();
    view! {
        <section class="oc-mail__attachments">
            <h3>"Anexos"</h3>
            <ul>
                {attachments
                    .into_iter()
                    .map(|attachment| {
                        let name = text(&attachment, "filename", "anexo").to_owned();
                        let kind = text(&attachment, "content_type", "").to_owned();
                        let size = attachment
                            .get("size_bytes")
                            .and_then(Value::as_i64)
                            .unwrap_or(0);
                        view! {
                            <li class="oc-mail__attachment">
                                {icon(Icon::Attach, 13)}
                                <span class="oc-mail__attachment-name">{name}</span>
                                <span class="oc-mono oc-mail__attachment-meta">
                                    {format!("{kind} · {}", human_size(size))}
                                </span>
                                <span class="oc-unavailable" aria-disabled="true"
                                      title="A descarga de anexos ainda não está disponível.">
                                    "Descarregar"
                                </span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
}

/// Uma data ISO-8601 reduzida ao que cabe numa linha de lista.
///
/// Sem biblioteca de formatação: o Core devolve UTC em formato fixo, e cortar
/// no `T` é suficiente para o que a lista mostra. Uma dependência inteira para
/// isto seria desproporcionada (`CLAUDE.md` §54).
fn short_date(raw: &str) -> String {
    let (date, rest) = raw.split_once('T').unwrap_or((raw, ""));
    let time: String = rest.chars().take(5).collect();

    if time.is_empty() {
        date.to_owned()
    } else {
        format!("{date} {time}")
    }
}

/// Um tamanho em bytes, legível.
fn human_size(bytes: i64) -> String {
    let bytes = bytes.max(0);
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.0} kB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ── O composer ──────────────────────────────────────────────────────────

/// O que o composer tem neste momento.
///
/// Existe porque o composer é re-renderizado com o que já lá estava: com uma
/// sugestão gerada, com um pedido de confirmação, ou com uma recusa. Devolver
/// os campos vazios perderia o trabalho de quem escreveu.
#[derive(Default)]
pub struct ComposeDraft {
    /// A caixa a partir da qual se envia.
    pub mailbox_id: String,
    /// Destinatários, separados por vírgula.
    pub to: String,
    /// Em cópia.
    pub cc: String,
    /// Assunto.
    pub subject: String,
    /// Corpo.
    pub body: String,
    /// A mensagem a que isto responde, quando é uma resposta.
    pub reply_to: Option<String>,
    /// Instrução dada à assistência, para não se perder ao regenerar.
    pub instruction: String,
    /// Confirmação de envio para fora da instituição, quando pedida.
    pub confirmation: Option<String>,
    /// Recusa ou erro a mostrar.
    pub error: Option<String>,
    /// Aviso de que o texto abaixo foi gerado e ainda não foi enviado.
    pub generated: bool,
}

/// O ecrã de composição.
pub fn compose(view: &MailView, draft: &ComposeDraft) -> impl IntoView {
    let boxes = view.boxes().to_vec();
    let can_send = view.can_send();

    let identities: Vec<(String, bool)> = boxes
        .iter()
        .filter(|mailbox| {
            mailbox
                .get("may_send")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(|mailbox| (text(mailbox, "address", "").to_owned(), true))
        .collect();

    let no_identity = identities.is_empty();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{if draft.reply_to.is_some() { "Responder" } else { "Nova mensagem" }}</h1>
                    <p>
                        "Nada é enviado antes de carregar em «Enviar». A assistência de escrita
                         devolve texto para este formulário e nunca envia."
                    </p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Voltar ao correio", Variant::Secondary).href("/mail"))}
                </div>
            </div>

            {draft.error.as_ref().map(|reason| view! {
                <div class="oc-callout oc-callout--error oc-mail__banner" role="alert">
                    {icon(Icon::Shield, 15)}
                    <p>{reason.clone()}</p>
                </div>
            })}

            {draft.confirmation.as_ref().map(|reason| view! {
                <div class="oc-callout oc-callout--warning oc-mail__banner" role="alert">
                    {icon(Icon::Shield, 15)}
                    <p>{reason.clone()}</p>
                </div>
            })}

            {draft.generated.then(|| view! {
                // A distinção mais importante deste ecrã, dita por escrito
                // (briefing §15).
                <div class="oc-callout oc-mail__banner oc-mail__banner--ai" role="status">
                    <span class="oc-btn__dot"></span>
                    <p>
                        "Texto sugerido pela assistência do Ocinye OS. "
                        <strong>"Ainda não foi enviado."</strong>
                        " Reveja-o e edite-o antes de enviar."
                    </p>
                </div>
            })}

            // Um só formulário, dois destinos. O botão de assistência submete para
            // `/mail/assist`, que devolve texto; o de envio submete para
            // `/mail/send`, que é a única rota que fala com o serviço de correio.
            <form class="oc-mail__compose" method="post" action="/mail/send">
                <input type="hidden" name="mailbox_id" value=draft.mailbox_id.clone() />
                {draft.reply_to.as_ref().map(|id| view! {
                    <input type="hidden" name="reply_to" value=id.clone() />
                })}

                {if no_identity {
                    view! {
                        <p class="oc-mail__no-identity" role="status">
                            "Não possui nenhuma identidade de correio a partir da qual possa enviar."
                        </p>
                    }
                    .into_any()
                } else {
                    select("mail-from", "De", "from", identities).into_any()
                }}

                {field_with_value("mail-to", "Para", "to",
                    "endereços separados por vírgula", "text", draft.to.clone())}
                {field_with_value("mail-cc", "Cc", "cc",
                    "opcional", "text", draft.cc.clone())}
                {field_with_value("mail-subject", "Assunto", "subject",
                    "assunto da mensagem", "text", draft.subject.clone())}
                {textarea_with_value("mail-body", "Mensagem", "body",
                    "Escreva a mensagem…", 260, draft.body.clone())}

                {assistance_panel(view, draft)}

                {draft.confirmation.is_some().then(|| view! {
                    <label class="oc-check" for="mail-confirm">
                        <input type="checkbox" id="mail-confirm" name="confirmed" value="true" />
                        <span>
                            "Confirmo que pretendo enviar esta mensagem para fora da instituição."
                        </span>
                    </label>
                })}

                <div class="oc-mail__compose-actions">
                    {if can_send && !no_identity {
                        view! {
                            <button type="submit" class="oc-btn oc-btn--primary">
                                {icon(Icon::Send, 13)}
                                "Enviar"
                            </button>
                        }
                        .into_any()
                    } else {
                        view! {
                            <span class="oc-btn oc-btn--primary oc-unavailable" aria-disabled="true"
                                  title="O serviço de envio não está disponível.">
                                "Enviar"
                            </span>
                        }
                        .into_any()
                    }}
                    <a class="oc-btn oc-btn--secondary" href="/mail">"Descartar"</a>
                </div>
            </form>
        </div>
    }
}

/// O painel de assistência de escrita.
///
/// Vive dentro do formulário para poder ler o que já foi escrito, mas o seu
/// botão submete para outra rota. As três situações — pode e há, pode e não há,
/// não pode — dizem-se por extenso: um painel apagado sem explicação é tão
/// opaco como um controlo que não faz nada (briefing §61).
fn assistance_panel(view: &MailView, draft: &ComposeDraft) -> impl IntoView {
    let may = view.may_use_ai();
    let available = view.ai_available();
    let instruction = draft.instruction.clone();

    let head = section_head("Assistência de escrita", None, None);

    if !may {
        return card(
            head,
            view! {
                <p class="oc-mail__assist-note">
                    "Não possui autorização para usar a assistência de escrita no correio.
                     Escrever, responder e enviar continuam disponíveis."
                </p>
            },
        )
        .into_any();
    }

    if !available {
        return card(
            head,
            view! {
                <p class="oc-mail__assist-note">
                    "A assistência de escrita depende de uma capacidade de IA do Ocinye OS,
                     que não está actualmente disponível. Escrever, responder e enviar não
                     dependem dela e continuam a funcionar normalmente."
                </p>
                <a class="oc-mail__assist-link" href="/ai">"Ver o estado da inteligência"</a>
            },
        )
        .into_any();
    }

    card(
        head,
        view! {
            <p class="oc-mail__assist-note">
                "A assistência devolve texto para o campo «Mensagem». Nunca envia."
            </p>

            // As acções vêm do contrato, não de uma lista escrita aqui: uma
            // acção acrescentada ao Core aparece sozinha, e uma removida
            // desaparece em vez de ficar a produzir recusas.
            {select_labelled(
                "mail-assist-action",
                "O que pretende",
                "action",
                ComposeAction::all()
                    .into_iter()
                    .map(|action| {
                        SelectOption::new(action.as_str(), action.label())
                            .selected(action == ComposeAction::Generate)
                    })
                    .collect(),
            )}

            {textarea_with_value(
                "mail-instruction",
                "Instrução",
                "instruction",
                "Descreva o que pretende. O conteúdo do email é tratado como dados, nunca como instruções.",
                92,
                instruction,
            )}

            // Submete para `/mail/assist`. O `formaction` é o mecanismo do
            // próprio HTML: nenhum script, nenhuma ambiguidade sobre qual das
            // duas rotas é chamada.
            <button
                type="submit"
                formaction="/mail/assist"
                class="oc-btn oc-btn--secondary"
            >
                <span class="oc-btn__dot"></span>
                "Gerar sugestão"
            </button>
        },
    )
    .into_any()
}

// ── Definições ──────────────────────────────────────────────────────────

/// As preferências de correio do membro, e o estado do serviço.
pub fn settings(view: &MailView, preferences: &Value) -> impl IntoView {
    let signature = preferences
        .get("signature")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    // Qualquer valor irreconhecível bloqueia: uma preferência corrompida não
    // pode voltar a ligar o rastreio (ver `RemoteContentPolicy::parse`).
    let remote_policy =
        RemoteContentPolicy::parse(text(preferences, "remote_content_policy", "block"));

    let endpoints: Vec<String> = view
        .status
        .get("endpoints")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    let adapter = text(&view.status, "adapter", "desconhecido").to_owned();
    let can_read = view.can_read();
    let can_send = view.can_send();
    let ai = view.ai_available();
    let detail = view.detail();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Definições de correio"</h1>
                    <p>"As suas preferências e o estado do serviço."</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Voltar ao correio", Variant::Secondary).href("/mail"))}
                </div>
            </div>

            <div class="oc-grid oc-grid--2">
                {card(
                    section_head("Preferências", None, None),
                    view! {
                        <form method="post" action="/mail/settings">
                            {textarea_with_value(
                                "mail-signature",
                                "Assinatura",
                                "signature",
                                "Acrescentada ao fim das mensagens que escrever.",
                                120,
                                signature,
                            )}

                            {select_labelled(
                                "mail-remote",
                                "Conteúdo remoto",
                                "remote_content_policy",
                                vec![
                                    SelectOption::new(
                                        RemoteContentPolicy::Block.as_str(),
                                        "Nunca carregar",
                                    )
                                    .selected(remote_policy == RemoteContentPolicy::Block),
                                    SelectOption::new(
                                        RemoteContentPolicy::AllowOnce.as_str(),
                                        "Perguntar em cada mensagem",
                                    )
                                    .selected(remote_policy == RemoteContentPolicy::AllowOnce),
                                    SelectOption::new(
                                        RemoteContentPolicy::AllowKnownSenders.as_str(),
                                        "Carregar de remetentes que eu permitir",
                                    )
                                    .selected(
                                        remote_policy == RemoteContentPolicy::AllowKnownSenders,
                                    ),
                                ],
                            )}

                            <p class="oc-mail__assist-note">
                                "Carregar conteúdo remoto informa quem enviou a mensagem de que
                                 ela foi aberta. O Ocinye OS não o carrega por omissão."
                            </p>

                            <button type="submit" class="oc-btn oc-btn--primary">"Guardar"</button>
                        </form>
                    },
                )}

                {card(
                    section_head("Estado do serviço", None, None),
                    view! {
                        <ul class="oc-mail__status">
                            <li>
                                <span>"Leitura"</span>
                                {state_badge(can_read)}
                            </li>
                            <li>
                                <span>"Envio"</span>
                                {state_badge(can_send)}
                            </li>
                            <li>
                                <span>"Assistência de escrita"</span>
                                {state_badge(ai)}
                            </li>
                            <li>
                                <span>"Adaptador"</span>
                                <span class="oc-mono">{adapter}</span>
                            </li>
                        </ul>

                        <p class="oc-mail__assist-note">{detail}</p>

                        // Anfitriões e portos. Nenhuma credencial aparece aqui, e
                        // nenhuma pode: o Core não as devolve (briefing §59).
                        {(!endpoints.is_empty()).then(|| view! {
                            <p class="oc-mono oc-mail__endpoints">{endpoints.join(" · ")}</p>
                        })}
                    },
                )}
            </div>
        </div>
    }
}

fn state_badge(ok: bool) -> impl IntoView {
    if ok {
        badge("Disponível", Tone::Ok).into_any()
    } else {
        badge("Indisponível", Tone::Gray).into_any()
    }
}

#[cfg(test)]
mod integridade {
    use super::*;
    use serde_json::json;

    fn sem_servico() -> MailView {
        MailView {
            status: json!({
                "can_read": false,
                "can_send": false,
                "detail": "O correio institucional ainda não foi configurado.",
            }),
            sync_notice: None,
            mailboxes: json!({"items": []}),
            active_mailbox: None,
            folder: "inbox".to_owned(),
            query: String::new(),
        }
    }

    fn viewer() -> Viewer {
        Viewer {
            avatar: ocinye_contracts::AvatarChoice::Initials,
            username: Some("jmanuel".to_owned()),
            email: Some("jmanuel@ocinye.com".to_owned()),
            session_expires_in: Some(std::time::Duration::from_secs(8 * 3600)),
            name: "Teste".to_owned(),
            organisation: "Ocinye".to_owned(),
            core_status: crate::ui::shell::CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            capabilities: ocinye_contracts::Permission::all()
                .into_iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
        }
    }

    /// Serviço ausente não é caixa vazia.
    ///
    /// São estados diferentes e parecem-se: uma lista de pastas com contagens a
    /// zero diria «não tem mensagens» quando o que não há é serviço. O membro
    /// concluiria que ninguém lhe escreveu — e agiria em cima disso.
    #[test]
    fn sem_configuracao_o_ecra_diz_o_e_nao_finge_uma_caixa() {
        let html = mail(&viewer(), &sem_servico(), &json!({"items": []}), None).to_html();

        assert!(
            html.contains("não está configurado"),
            "o ecrã não declarou a ausência de serviço"
        );
        assert!(
            !html.contains("oc-mail__rail"),
            "apareceu uma lista de caixas sem haver serviço"
        );
        assert!(
            !html.contains("Não tem mensagens"),
            "a ausência de serviço foi apresentada como caixa vazia"
        );
    }
}
