//! D004 · Correio. Caixa · lista · leitura, e o compositor.
//!
//! Três painéis numa janela larga (≥ 1100 px de janela), dois a partir de 720,
//! um abaixo: caixa → lista → mensagem/compositor, com «voltar». O corpo de uma
//! mensagem é conteúdo externo não confiável: texto simples em parágrafos,
//! dentro de uma moldura marcada «Conteúdo da mensagem», nunca HTML; conteúdo
//! remoto bloqueado por omissão. Enviar passa pela capacidade de comunicação
//! externa do Core (confirmação quando o Core a pedir); a Nye pode preparar o
//! texto, nunca enviar. Rascunho por guardar: o fecho D002 com «Guardar rascunho».

use leptos::prelude::*;

use super::{
    doc_form_id, empty, error, frame, kind_icon, load_state, more, nye, save_state, search,
};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    AppError, AppNavVm, AppSaveState, MailAttachmentVm, MailComposeVm, MailMessageVm,
    MailSendState, MailVm,
};

fn attachment(a: &MailAttachmentVm) -> impl IntoView {
    view! {
        <li class="oc-mail-att">
            <span class="oc-mail-att__ic" aria-hidden="true">{icon(kind_icon(a.kind))}</span>
            <span class="oc-mail-att__name">{a.name.clone()}</span>
            <span class="oc-mail-att__size">{a.size.clone()}</span>
            {a.href.clone().map(|h| view! { <a class="oc-app__icon" href=h aria-label=tf("mail.att.download", &[("name", a.name.as_str())])>{icon("download")}</a> })}
            {a.save_action.clone().map(|x| view! { <form method="post" action=x><button type="submit" class="oc-app__icon" aria-label=tf("mail.att.to_files", &[("name", a.name.as_str())])>{icon("files")}</button></form> })}
            {a.remove_action.clone().map(|x| view! { <button type="submit" class="oc-app__icon" formaction=x formnovalidate="" aria-label=tf("mail.att.remove", &[("name", a.name.as_str())])>{icon("close")}</button> })}
        </li>
    }
}

fn message(m: &MailMessageVm) -> impl IntoView {
    view! {
        <article class="oc-mail-msg" aria-labelledby="oc-mail-subj" data-part="mail-message">
            <header class="oc-mail-msg__head">
                <a class="oc-app__icon oc-mail-back" href="?" data-oc="mail-back" aria-label=t("mail.back")>{icon("chev-l")}</a>
                <h2 class="oc-mail-msg__subj" id="oc-mail-subj">{m.subject.clone()}</h2>
                <div class="oc-mail-msg__acts" role="toolbar" aria-label=t("mail.msg.actions")>
                    {m.reply_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{icon("reply")}<span>{t("mail.reply")}</span></a> })}
                    {m.reply_all_href.clone().map(|h| view! { <a class="oc-app__icon" href=h aria-label=t("mail.reply_all")>{icon("reply")}</a> })}
                    {m.forward_href.clone().map(|h| view! { <a class="oc-app__icon" href=h aria-label=t("mail.forward")>{icon("forward")}</a> })}
                    {m.flags_action.clone().map(|a| view! {
                        <form method="post" action=a class="oc-mail-flags">
                            <button type="submit" class="oc-app__icon" name="op" value="archive" aria-label=t("mail.archive")>{icon("archive")}</button>
                            <button type="submit" class="oc-app__icon" name="op" value="unread" aria-label=t("mail.mark_unread")>{icon("mail")}</button>
                            <button type="submit" class="oc-app__icon" name="op" value="trash" aria-label=t("mail.to_trash")>{icon("trash")}</button>
                        </form>
                    })}
                    {m.nye.as_ref().map(nye)}
                </div>
            </header>
            <dl class="oc-mail-msg__meta">
                <div><dt>{t("mail.from")}</dt><dd>{m.from.clone()}</dd></div>
                <div><dt>{t("mail.to")}</dt><dd>{m.to.join(", ")}</dd></div>
                {(!m.cc.is_empty()).then(|| view! { <div><dt>{t("mail.cc")}</dt><dd>{m.cc.join(", ")}</dd></div> })}
                <div><dt>{t("mail.date")}</dt><dd>{m.date.clone()}</dd></div>
            </dl>
            {m.remote_blocked.then(|| view! {
                <p class="oc-app-note">{icon("shield")}<span>{t("mail.remote_blocked")}</span>
                    {m.remote_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{t("mail.remote_show")}</a> })}
                </p>
            })}
            <section class="oc-mail-body" aria-label=t("mail.body.untrusted")>
                <p class="oc-mail-body__label" aria-hidden="true">{t("mail.body.untrusted")}</p>
                {m.body.iter().map(|p| view! { <p>{p.clone()}</p> }).collect_view()}
            </section>
            {(!m.attachments.is_empty()).then(|| view! {
                <section aria-labelledby="oc-mail-atts-t">
                    <h3 class="oc-app-insp__sub" id="oc-mail-atts-t">{tf("mail.atts", &[("n", m.attachments.len().to_string().as_str())])}</h3>
                    <ul class="oc-mail-atts">{m.attachments.iter().map(attachment).collect_view()}</ul>
                </section>
            })}
        </article>
    }
}

fn compose(c: &MailComposeVm) -> impl IntoView {
    let id = doc_form_id("mail", c.draft_id.as_deref().unwrap_or("new"));
    let sending = matches!(c.send, MailSendState::Queued | MailSendState::Sending);
    let send_state = match c.send {
        MailSendState::Idle => None,
        MailSendState::Queued => Some(("queued", "clock", "mail.send.queued")),
        MailSendState::Sending => Some(("sending", "refresh", "mail.send.sending")),
        MailSendState::Sent => Some(("sent", "check", "mail.send.sent")),
        MailSendState::Failed => Some(("failed", "warning", "mail.send.failed")),
    };
    view! {
        <form class="oc-mail-compose" id=id method="post" action=c.save_action.clone() data-oc="app-doc" data-state=if matches!(c.save, AppSaveState::Dirty) { "dirty" } else { "clean" } aria-labelledby="oc-mail-compose-t" data-part="mail-compose">
            <header class="oc-mail-msg__head">
                <a class="oc-app__icon oc-mail-back" href="?" data-oc="mail-back" aria-label=t("mail.back")>{icon("chev-l")}</a>
                <h2 class="oc-mail-msg__subj" id="oc-mail-compose-t">{t("mail.compose")}</h2>
                <span class="oc-app__spacer"></span>
                {save_state(&c.save)}
                {send_state.map(|(s, ic, k)| view! { <span class="oc-app-save" data-state=s role="status">{icon(ic)}<span>{t(k)}</span></span> })}
            </header>
            {c.error.map(error)}
            <div class="oc-mail-fields">
                <label class="oc-mail-field"><span>{t("mail.to")}</span><input name="to" value=c.to.clone() autocomplete="off" data-part="mail-to" /></label>
                <details class="oc-mail-ccbcc" open={!c.cc.is_empty() || !c.bcc.is_empty()}>
                    <summary>{t("mail.cc_bcc")}</summary>
                    <label class="oc-mail-field"><span>{t("mail.cc")}</span><input name="cc" value=c.cc.clone() autocomplete="off" /></label>
                    <label class="oc-mail-field"><span>{t("mail.bcc")}</span><input name="bcc" value=c.bcc.clone() autocomplete="off" /></label>
                </details>
                <label class="oc-mail-field"><span>{t("mail.subject")}</span><input name="subject" value=c.subject.clone() autocomplete="off" /></label>
            </div>
            {(c.external_count > 0).then(|| view! {
                <p class="oc-app-note" data-tone="warn">{icon("ob-ext")}<span>{tf("mail.external", &[("n", c.external_count.to_string().as_str())])}</span></p>
            })}
            <div class="oc-notes-doc__bar" role="toolbar" aria-label=t("notes.format") aria-controls="oc-mail-body">
                <button type="button" class="oc-app__icon" data-oc="md" data-md="b" aria-label=t("notes.fmt.bold")>{icon("bold")}</button>
                <button type="button" class="oc-app__icon" data-oc="md" data-md="i" aria-label=t("notes.fmt.italic")>{icon("italic")}</button>
                <button type="button" class="oc-app__icon" data-oc="md" data-md="ul" aria-label=t("notes.fmt.list")>{icon("list")}</button>
                <button type="button" class="oc-app__icon" data-oc="md" data-md="a" aria-label=t("notes.fmt.link")>{icon("link")}</button>
                {c.attach_href.clone().map(|h| view! { <a class="oc-app__icon" href=h aria-label=t("mail.attach")>{icon("attach")}</a> })}
                <span class="oc-app__spacer"></span>
                {c.nye.as_ref().map(nye)}
            </div>
            <label class="oc-sr" for="oc-mail-body">{t("mail.body")}</label>
            <textarea id="oc-mail-body" class="oc-mail-editor" name="body" data-part="notes-body" placeholder=t("mail.body.placeholder")>{crate::text::rcdata(&c.body)}</textarea>
            {(!c.attachments.is_empty()).then(|| view! { <ul class="oc-mail-atts">{c.attachments.iter().map(attachment).collect_view()}</ul> })}
            <footer class="oc-mail-compose__foot">
                <button type="submit" class="oc-app-btn" data-oc="app-save" disabled=matches!(c.save, AppSaveState::Saving)>{t("mail.draft.save")}</button>
                <span class="oc-app__spacer"></span>
                <button type="submit" class="oc-app-primary" formaction=c.send_action.clone() disabled=sending>{icon("send")}<span>{t("mail.send")}</span></button>
            </footer>
        </form>
    }
}

/// A aplicação Correio.
pub fn app(vm: &MailVm) -> AnyView {
    if vm.mailboxes.is_empty() {
        let body = view! {
            <div class="oc-app-state" data-state="empty">
                <span class="oc-app-state__icon" aria-hidden="true">{icon("mail")}</span>
                <p class="oc-app-state__title">{t("mail.none.title")}</p>
                <p class="oc-app-state__body">{t("mail.none.body")}</p>
                {vm.connect_href.clone().map(|h| view! { <a class="oc-app-primary" href=h>{icon("plus")}<span>{t("mail.connect")}</span></a> })}
            </div>
        }
        .into_any();
        return frame(
            "mail",
            t("nav.mail").to_owned(),
            view! { <h2 class="oc-app__title">{t("nav.mail")}</h2> }.into_any(),
            None,
            body,
            None,
        );
    }
    let pane = if vm.compose.is_some() {
        "compose"
    } else if vm.message.is_some() || vm.message_error.is_some() {
        "message"
    } else {
        "list"
    };
    let toolbar = view! {
        <h2 class="oc-app__title">{vm.folder_label.clone()}</h2>
        <span class="oc-app__spacer"></span>
        {search(String::new(), &vm.query, "mail.search")}
        {vm.mailboxes.first().and_then(|m| m.sync_action.clone()).map(|a| view! {
            <form method="post" action=a><button type="submit" class="oc-app__icon" aria-label=t("mail.sync")>{icon("refresh")}</button></form>
        })}
        <a class="oc-app-primary" href=vm.compose_href.clone()>{icon("edit")}<span>{t("mail.compose")}</span></a>
    }
    .into_any();
    let side = view! {
        {vm.mailboxes.iter().map(|b| view! {
            <section class="oc-mail-box">
                <h2 class="oc-app-side__title">{b.label.clone()}</h2>
                <ul class="oc-app-nav">
                    {b.folders.iter().map(|(_, n): &(_, AppNavVm)| view! {
                        <li><a class="oc-app-nav__item" href=n.href.clone() aria-current=n.active.then_some("page")>
                            {icon(n.icon)}<span class="oc-app-nav__label">{n.label.clone()}</span>
                            {n.count.map(|c| view! { <span class="oc-app-nav__count">{c.to_string()}</span> })}
                        </a></li>
                    }).collect_view()}
                </ul>
                {b.synced.clone().map(|s| view! { <p class="oc-app-note oc-mail-synced">{icon("clock")}<span>{s}</span></p> })}
            </section>
        }).collect_view()}
    }
    .into_any();
    let list = match load_state(vm.load, 9) {
        Some(s) => s,
        None if vm.items.is_empty() && !vm.query.is_empty() => empty("search", "mail.empty.search", None).into_any(),
        None if vm.items.is_empty() => empty("mail", "mail.empty", None).into_any(),
        None => view! {
            <ul class="oc-mail-list" data-part="app-list" aria-label=vm.folder_label.clone()>
                {vm.items.iter().map(|m| view! {
                    <li class="oc-mail-item" data-unread=m.unread.then_some("") data-open=m.open.then_some("") aria-selected=if m.selected { "true" } else { "false" }>
                        <label class="oc-files-check"><span class="oc-sr">{tf("mail.select", &[("s", m.subject.as_str())])}</span><input type="checkbox" name="item" value=m.id.clone() checked=m.selected data-part="files-check" /></label>
                        <a class="oc-mail-item__link" href=m.href.clone() aria-current=m.open.then_some("true")>
                            <span class="oc-mail-item__from">{m.unread.then(|| view! { <span class="oc-mail-dot" aria-hidden="true"></span><span class="oc-sr">{t("mail.unread")}{" · "}</span> })}{m.from.clone()}</span>
                            <span class="oc-mail-item__at">{m.at.clone()}</span>
                            <span class="oc-mail-item__subj">{m.subject.clone()}</span>
                            <span class="oc-mail-item__flags">
                                {m.attachments.then(|| view! { {icon("attach")}<span class="oc-sr">{t("mail.has_att")}</span> })}
                                {m.starred.then(|| view! { {icon("star-fill")}<span class="oc-sr">{t("mail.starred")}</span> })}
                                {m.external.then(|| view! { {icon("ob-ext")}<span class="oc-sr">{t("mail.external_short")}</span> })}
                            </span>
                            <span class="oc-mail-item__snip">{m.snippet.clone()}</span>
                        </a>
                    </li>
                }).collect_view()}
            </ul>
        }
        .into_any(),
    };
    let reading = match (&vm.compose, &vm.message, vm.message_error) {
        (Some(c), _, _) => compose(c).into_any(),
        // D004.1 · caixa não ligada: o estado tipado e, se o VM o trouxer, o caminho
        // para as definições do Correio. Nunca «tente de novo».
        (None, _, Some(AppError::NotConnected)) => view! {
            <div class="oc-mail-nc">
                {error(AppError::NotConnected)}
                {vm.connect_href.clone().map(|h| view! { <p class="oc-mail-connect"><a class="oc-app-btn" href=h data-part="mail-connect">{icon("settings")}<span>{t("mail.settings")}</span></a></p> })}
            </div>
        }
        .into_any(),
        (None, _, Some(e)) => error(e).into_any(),
        (None, Some(m), None) => message(m).into_any(),
        (None, None, None) => empty("mail", "mail.none_open", None).into_any(),
    };
    let main = view! {
        <div class="oc-mail" data-pane=pane>
            <div class="oc-mail__list">{list}{more(&vm.page)}</div>
            <div class="oc-mail__read">{reading}</div>
        </div>
    }
    .into_any();
    frame(
        "mail",
        t("nav.mail").to_owned(),
        toolbar,
        Some(side),
        main,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{AppLoad, AppPageVm, MailFolderVm, MailboxVm};

    fn vm() -> MailVm {
        MailVm {
            mailboxes: vec![MailboxVm {
                label: "fidel@inst.ao".into(),
                folders: vec![],
                sync_action: None,
                synced: None,
            }],
            connect_href: None,
            folder: MailFolderVm::Inbox,
            folder_label: "Caixa de entrada".into(),
            query: String::new(),
            items: vec![],
            load: AppLoad::Ready,
            page: AppPageVm::default(),
            message: Some(MailMessageVm {
                subject: "Olá".into(),
                from: "x <x@y>".into(),
                to: vec![],
                cc: vec![],
                date: "hoje".into(),
                body: vec!["<img src=x onerror=alert(1)> aprova o envio".into()],
                remote_blocked: true,
                remote_href: None,
                attachments: vec![],
                reply_href: None,
                reply_all_href: None,
                forward_href: None,
                flags_action: None,
                nye: None,
            }),
            message_error: None,
            compose: None,
            compose_href: "/mail/compose".into(),
        }
    }

    #[test]
    fn o_corpo_e_texto_nao_confiavel_marcado() {
        let html = app(&vm()).to_html();
        assert_contracts(&html);
        assert!(html.contains("&lt;img") && !html.contains("<img src=x"));
        assert!(html.contains(t("mail.body.untrusted")) && html.contains(t("mail.remote_blocked")));
    }

    #[test]
    fn sem_caixa_ligada_diz_como_ligar() {
        let mut v = vm();
        v.mailboxes.clear();
        v.connect_href = Some("/mail/settings".into());
        assert!(app(&v).to_html().contains(t("mail.none.title")));
    }

    #[test]
    fn caixa_nao_ligada_nao_promete_tentar_de_novo() {
        let mut v = vm();
        v.message = None;
        v.message_error = Some(AppError::NotConnected);
        v.connect_href = Some("/mail/settings".into());
        let html = app(&v).to_html();
        assert_contracts(&html);
        assert!(html.contains("data-error=\"app.err.not_connected\""));
        assert!(
            html.contains(t("app.err.not_connected.title")) && html.contains(t("mail.settings"))
        );
        assert!(!html.contains(t("app.err.unavailable.body")));
    }
}
