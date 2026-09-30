//! D007 · Mensagens (`/messages`, `messaging.use`). DESIGN_LOCKED.
//!
//! Comunicação nativa do Ocinye: conversas directas e de grupo, respostas,
//! menções, reacções e leitura — o que o domínio tem. Não é o Correio (sem
//! pastas, sem assunto). O corpo é sempre texto, escapado. Sem anexos (o domínio
//! não os tem), sem editar nem apagar (sem rota). O compositor guarda o texto se
//! o envio falhar. A Nye não participa.

use leptos::prelude::*;

use super::ops::presence;
use super::org::{action, avatar};
use super::res::{detail_or, two_pane};
use super::{empty, error, frame, load_state, primary};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{MessagesVm, MsgConvRowVm, MsgKind, MsgNewVm, MsgThreadVm, MsgVm};

fn conv_row(c: &MsgConvRowVm) -> impl IntoView {
    view! {
        <li>
            <a class="oc-msg-conv" href=c.href.clone() aria-current=c.active.then_some("true") data-part="res-open" data-unread=(c.unread > 0).then_some("")>
                {avatar(&c.avatar)}
                <span class="oc-msg-conv__main">
                    <span class="oc-msg-conv__top">
                        <span class="oc-msg-conv__title">{c.title.clone()}</span>
                        {(c.kind == MsgKind::Group).then(|| view! { <span class="oc-msg-conv__kind">{t("msg.group")}</span> })}
                        <span class="oc-msg-conv__at">{c.last_at.clone()}</span>
                    </span>
                    <span class="oc-msg-conv__last">{c.last.clone().unwrap_or_else(|| t("msg.no_messages").to_owned())}</span>
                </span>
                {(c.unread > 0).then(|| view! { <span class="oc-msg-conv__badge" aria-label=tf("msg.unread_n", &[("n", c.unread.to_string().as_str())])>{c.unread.to_string()}</span> })}
                {(c.mentions > 0).then(|| view! { <span class="oc-msg-conv__at-mark" aria-label=tf("msg.mentions_n", &[("n", c.mentions.to_string().as_str())])>"@"</span> })}
            </a>
        </li>
    }
}

fn message(m: &MsgVm) -> impl IntoView {
    view! {
        {m.day.clone().map(|d| view! { <li class="oc-msg-day" role="separator"><span>{d}</span></li> })}
        <li class="oc-msg" id=m.anchor.clone() data-mine=m.mine.then_some("") data-mention=m.mentions_me.then_some("")>
            {avatar(&m.avatar)}
            <div class="oc-msg__main">
                <p class="oc-msg__head"><span class="oc-msg__author">{m.author.clone()}</span><time class="oc-msg__at">{m.at.clone()}</time>{m.edited.then(|| view! { <span class="oc-msg__edited">{t("msg.edited")}</span> })}</p>
                {m.reply.clone().map(|(a, x)| view! { <p class="oc-msg__reply"><span class="oc-msg__reply-who">{a}</span>{x}</p> })}
                {match m.body.clone() {
                    Some(b) => view! { <div class="oc-msg__body">{b.split('\n').map(|l| view! { <p>{l.to_owned()}</p> }).collect_view()}</div> }.into_any(),
                    None => view! { <p class="oc-msg__withdrawn">{t("msg.withdrawn")}</p> }.into_any(),
                }}
                <div class="oc-msg__foot">
                    {(!m.reactions.is_empty()).then(|| view! {
                        <form class="oc-msg__reacts" method="post" action=m.react_action.clone().unwrap_or_default()>
                            {m.reactions.iter().map(|r| {
                                let label = tf(if r.mine { "msg.react.remove" } else { "msg.react.add" }, &[("emoji", r.emoji.as_str()), ("n", r.count.to_string().as_str())]);
                                if m.react_action.is_some() {
                                    view! { <button type="submit" class="oc-msg__react" name="emoji" value=r.emoji.clone() aria-pressed=if r.mine { "true" } else { "false" } aria-label=label><span aria-hidden="true">{r.emoji.clone()}</span>" "{r.count.to_string()}</button> }.into_any()
                                } else {
                                    view! { <span class="oc-msg__react" data-static="" aria-label=label><span aria-hidden="true">{r.emoji.clone()}</span>" "{r.count.to_string()}</span> }.into_any()
                                }
                            }).collect_view()}
                        </form>
                    })}
                    {m.reply_href.clone().map(|h| view! { <a class="oc-msg__act" href=h>{icon("reply")}<span>{t("msg.reply")}</span></a> })}
                </div>
            </div>
        </li>
    }
}

fn thread(th: &MsgThreadVm, back: String) -> impl IntoView {
    let c = &th.composer;
    view! {
        <section class="oc-msg-thread" data-part="msg-thread" aria-labelledby="oc-res-title">
            <header class="oc-res-head oc-msg-thread__head">
                <a class="oc-app__icon oc-res-back" href=back aria-label=t("res.back")>{icon("chev-l")}</a>
                <div class="oc-res-head__text">
                    <h2 class="oc-res-head__title" id="oc-res-title">{th.title.clone()}</h2>
                    <p class="oc-res-head__tags">{(th.kind == MsgKind::Group).then(|| view! { <span class="oc-msg-conv__kind">{tf("msg.members_n", &[("n", th.members.len().to_string().as_str())])}</span> })}{presence(th.presence)}</p>
                </div>
                {(th.kind == MsgKind::Group).then(|| view! {
                    <details class="oc-msg-people">
                        <summary class="oc-app-btn">{icon("user")}<span>{t("msg.people")}</span></summary>
                        <ul class="oc-org-list">{th.members.iter().map(|p| view! { <li class="oc-org-list__row"><span>{p.name.clone()}</span><span class="oc-res-people__role">{p.role.clone()}</span><span class="oc-app__spacer"></span>{p.remove.as_ref().map(action)}</li> }).collect_view()}</ul>
                        {th.leave.as_ref().map(action)}
                    </details>
                })}
            </header>
            <div class="oc-msg-scroll" data-part="msg-scroll" tabindex="0" aria-label=t("msg.history")>
                {th.older_href.clone().map(|h| view! { <a class="oc-app-btn oc-msg-older" href=h>{t("msg.older")}</a> })}
                {match load_state(th.load, 5) {
                    Some(s) => s,
                    None if th.messages.is_empty() => empty("messages", "msg.thread.empty", None).into_any(),
                    None => view! { <ol class="oc-msg-list" aria-label=t("msg.history")>{th.messages.iter().map(message).collect_view()}</ol> }.into_any(),
                }}
            </div>
            <form class="oc-msg-compose" id="oc-messages-doc-compose" method="post" action=c.action.clone() data-oc="app-doc" data-state=if c.body.is_empty() { "clean" } else { "dirty" }>
                <input type="hidden" name="idempotency_key" value=c.idempotency_key.clone() />
                {c.error.map(error)}
                {c.reply.clone().map(|(a, x, id)| view! {
                    <p class="oc-msg-compose__reply"><input type="hidden" name="reply_to" value=id /><span>{tf("msg.replying", &[("name", a.as_str())])}</span><span class="oc-msg-compose__x">{x}</span>{c.reply_cancel_href.clone().map(|h| view! { <a class="oc-app__icon" href=h aria-label=t("msg.reply.cancel")>{icon("close")}</a> })}</p>
                })}
                <label class="oc-msg-compose__field"><span class="oc-sr">{t("msg.compose")}</span><textarea name="body" rows="2" required="" placeholder=t("msg.compose") data-part="msg-body">{crate::text::rcdata(&c.body)}</textarea></label>
                <button type="submit" class="oc-app-primary" data-oc="msg-send">{icon("send")}<span>{t("msg.send")}</span></button>
            </form>
        </section>
    }
}

fn new_conv(n: &MsgNewVm, back: String) -> impl IntoView {
    view! {
        <section class="oc-res-form" aria-labelledby="oc-res-title">
            <header class="oc-res-head"><a class="oc-app__icon oc-res-back" href=back aria-label=t("res.back")>{icon("chev-l")}</a><div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("msg.new")}</h2></div></header>
            {n.error.map(error)}
            {if n.unavailable {
                view! { <p class="oc-app-note">{icon("link")}<span>{t("msg.new.unavailable")}</span></p> }.into_any()
            } else {
                view! {
                    <form class="oc-org-add__search" method="get" action=n.search_action.clone() role="search">
                        <label class="oc-app-field"><span>{t("msg.new.search")}</span><input type="search" name="q" value=n.query.clone() autocomplete="off" /></label>
                        <button type="submit" class="oc-app-btn">{icon("search")}<span>{t("units.add.find")}</span></button>
                    </form>
                    {n.candidates.as_ref().map(|c| if c.is_empty() {
                        view! { <p class="oc-res-sec__empty" role="status">{t("msg.new.none")}</p> }.into_any()
                    } else {
                        view! {
                            <form class="oc-org-add__pick" method="post" action=n.action.clone()>
                                <fieldset class="oc-org-cands"><legend class="oc-res-form__hint">{t("msg.new.direct")}</legend>
                                    {c.iter().map(|o| view! { <label class="oc-org-cand"><input type="radio" name="with" value=o.value.clone() required="" checked=o.selected /><span>{o.label.clone()}</span></label> }).collect_view()}
                                </fieldset>
                                <div class="oc-res-form__foot"><span class="oc-app__spacer"></span><button type="submit" class="oc-app-primary">{icon("messages")}<span>{t("msg.new.open")}</span></button></div>
                            </form>
                        }.into_any()
                    })}
                    <p class="oc-res-form__hint">{t("msg.new.hint")}</p>
                }.into_any()
            }}
        </section>
    }
}

/// A aplicação Mensagens.
pub fn app(vm: &MessagesVm) -> AnyView {
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.messages")}</h2>
        <span class="oc-app__spacer"></span>
        {vm.new_href.clone().map(|h| primary(h, "plus", "msg.new"))}
    }
    .into_any();
    let list = match load_state(vm.load, 8) {
        Some(s) => s,
        None if vm.conversations.is_empty() => empty("messages", "msg.empty", Some("msg.empty.body")).into_any(),
        None => view! { <ul class="oc-msg-convs" data-oc="res-list" aria-label=t("msg.list")>{vm.conversations.iter().map(conv_row).collect_view()}</ul> }.into_any(),
    };
    let detail = match (&vm.new, &vm.thread) {
        (Some(n), _) => new_conv(n, vm.list_href.clone()).into_any(),
        (None, Some(th)) => thread(th, vm.list_href.clone()).into_any(),
        (None, None) => detail_or(
            vm.thread_error,
            empty("messages", "msg.none_open", None).into_any(),
        ),
    };
    frame(
        "messages",
        t("nav.messages").to_owned(),
        toolbar,
        None,
        two_pane(vm.pane, list, detail),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{AppLoad, MsgComposerVm, OrgAvatarVm, ResPane};

    #[test]
    fn corpo_hostil_fica_texto_e_o_rascunho_sobrevive_a_falha() {
        let av = OrgAvatarVm {
            initials: "MQ".into(),
            image_href: None,
        };
        let vm = MessagesVm {
            conversations: vec![],
            load: AppLoad::Ready,
            pane: ResPane::Detail,
            thread: Some(MsgThreadVm {
                title: "Marta".into(),
                kind: MsgKind::Direct,
                presence: None,
                messages: vec![MsgVm {
                    anchor: "m-1".into(),
                    author: "Marta".into(),
                    avatar: av,
                    mine: false,
                    body: Some("<img src=x onerror=alert(1)>".into()),
                    at: "10:02".into(),
                    edited: false,
                    reply: None,
                    mentions_me: false,
                    reactions: vec![],
                    react_action: None,
                    reply_href: None,
                    day: None,
                }],
                older_href: None,
                load: AppLoad::Ready,
                composer: MsgComposerVm {
                    action: "/messages/c/send".into(),
                    body: "rascunho".into(),
                    reply: None,
                    reply_cancel_href: None,
                    idempotency_key: "k".into(),
                    error: Some(crate::ui::view_models::AppError::SaveFailed),
                },
                members: vec![],
                leave: None,
            }),
            thread_error: None,
            new: None,
            new_href: None,
            list_href: "/messages".into(),
        };
        let html = app(&vm).to_html();
        assert_contracts(&html);
        assert!(html.contains("&lt;img") && !html.contains("<img src=x"));
        assert!(html.contains(">rascunho</textarea>") && html.contains(r#"data-state="dirty""#));
        assert!(!html.contains("presence"));
    }
}
