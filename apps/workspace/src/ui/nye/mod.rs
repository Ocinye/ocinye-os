//! D003 · Nye: Search · Ask · Act. DESIGN_LOCKED.
//!
//! Uma só assistente. Quatro superfícies:
//! - [`surface`]: a superfície universal — a paleta de comandos D001 alargada
//!   (D001_COMPONENT_EXTENSION). Mesmo `#oc-palette`, mesmo `data-oc="palette"`,
//!   mesmo `data-part="palette-q"`/`palette-item`, mesmo ⌘K. Envia para `/ask`.
//! - [`app`]: a aplicação Nye, corpo de uma janela gerida D002 (`WindowContent::Ready`).
//! - [`voice`]: premir para falar, dentro da aplicação. Nunca escuta contínua.
//! - [`confirm_dialog`]: a confirmação forte, desenhada depois da casca, como o
//!   `wm::dirty_close` (camada global; ver DESIGN_LOCK · D003).
//!
//! A vista não decide nada: autorização, risco, necessidade de confirmação,
//! estado de execução e disponibilidade chegam no ViewModel. Nenhum nome de
//! modelo ou fornecedor é desenhado. Não há raciocínio do modelo para mostrar:
//! a actividade é o que foi feito (capacidades, resultados), nunca o porquê interno.

use leptos::prelude::*;

use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    NyeAppVm, NyeAuth, NyeAvail, NyeAvailability, NyeBlock, NyeComposerState, NyeComposerVm,
    NyeContextKind, NyeContextState, NyeContextVm, NyeExecState, NyeGrounding, NyeHitGroupVm,
    NyeIntent, NyeKind, NyeLang, NyeLink, NyeMessageVm, NyeMsgState, NyePanel, NyeProcessing,
    NyeProposalVm, NyeRisk, NyeRole, NyeSourceVm, NyeStepState, NyeStepVm, NyeSurfaceVm, NyeTrust,
    NyeVoiceState, NyeVoiceVm, ShellVm,
};

// ── Vocabulário ──────────────────────────────────────────────────────────

fn kind_icon(k: NyeKind) -> &'static str {
    match k {
        NyeKind::App => "apps",
        NyeKind::File => "files",
        NyeKind::Folder => "folder",
        NyeKind::Note => "notes",
        NyeKind::Project => "project",
        NyeKind::Task => "tasks",
        NyeKind::Member => "user",
        NyeKind::Setting => "settings",
        NyeKind::Action => "command",
        NyeKind::Dataset => "data",
        NyeKind::Mail => "mail",
        NyeKind::Event => "calendar",
        NyeKind::Conversation => "nye",
        NyeKind::Web => "ob-globe",
        NyeKind::Other => "link",
    }
}

fn kind_key(k: NyeKind) -> &'static str {
    match k {
        NyeKind::App => "nye.kind.app",
        NyeKind::File => "nye.kind.file",
        NyeKind::Folder => "nye.kind.folder",
        NyeKind::Note => "nye.kind.note",
        NyeKind::Project => "nye.kind.project",
        NyeKind::Task => "nye.kind.task",
        NyeKind::Member => "nye.kind.member",
        NyeKind::Setting => "nye.kind.setting",
        NyeKind::Action => "nye.kind.action",
        NyeKind::Dataset => "nye.kind.dataset",
        NyeKind::Mail => "nye.kind.mail",
        NyeKind::Event => "nye.kind.event",
        NyeKind::Conversation => "nye.kind.conversation",
        NyeKind::Web => "nye.kind.web",
        NyeKind::Other => "nye.kind.other",
    }
}

fn group_key(k: NyeKind) -> &'static str {
    match k {
        NyeKind::App => "nye.group.app",
        NyeKind::File | NyeKind::Folder => "nye.group.file",
        NyeKind::Note => "nye.group.note",
        NyeKind::Project => "nye.group.project",
        NyeKind::Task => "nye.group.task",
        NyeKind::Member => "nye.group.member",
        NyeKind::Setting => "nye.group.setting",
        NyeKind::Action => "nye.group.action",
        NyeKind::Dataset => "nye.group.dataset",
        NyeKind::Mail => "nye.group.mail",
        NyeKind::Event => "nye.group.event",
        NyeKind::Conversation => "nye.group.conversation",
        NyeKind::Web | NyeKind::Other => "nye.group.other",
    }
}

fn risk_view(r: NyeRisk) -> impl IntoView {
    let (ic, key) = match r {
        NyeRisk::ReadOnly => ("eye", "nye.risk.read_only"),
        NyeRisk::Navigation => ("arrow-r", "nye.risk.navigation"),
        NyeRisk::ReversibleWrite => ("edit", "nye.risk.reversible_write"),
        NyeRisk::InstitutionalChange => ("organization", "nye.risk.institutional"),
        NyeRisk::ExternalCommunication => ("send", "nye.risk.external"),
        NyeRisk::Privileged => ("key", "nye.risk.privileged"),
        NyeRisk::Destructive => ("trash", "nye.risk.destructive"),
    };
    view! { <span class="oc-nye-risk" data-risk=r.as_str()>{icon(ic)}{t(key)}</span> }
}

fn exec_key(s: NyeExecState) -> &'static str {
    match s {
        NyeExecState::Proposed => "nye.exec.proposed",
        NyeExecState::AwaitingConfirmation => "nye.exec.awaiting",
        NyeExecState::Authorized => "nye.exec.authorized",
        NyeExecState::Running => "nye.exec.running",
        NyeExecState::Completed => "nye.exec.completed",
        NyeExecState::Partial => "nye.exec.partial",
        NyeExecState::Failed => "nye.exec.failed",
        NyeExecState::Cancelled => "nye.exec.cancelled",
        NyeExecState::Blocked => "nye.exec.blocked",
        NyeExecState::Rejected => "nye.exec.rejected",
        NyeExecState::Expired => "nye.exec.expired",
    }
}

fn exec_view(s: NyeExecState) -> impl IntoView {
    let ic = match s {
        NyeExecState::Completed => "check",
        NyeExecState::Failed | NyeExecState::Partial => "warning",
        NyeExecState::Blocked => "lock",
        NyeExecState::Cancelled | NyeExecState::Rejected | NyeExecState::Expired => "close",
        NyeExecState::Running => "refresh",
        _ => "clock",
    };
    view! { <span class="oc-nye-exec" data-state=s.as_str()>{icon(ic)}{t(exec_key(s))}</span> }
}

fn auth_key(a: NyeAuth) -> &'static str {
    match a {
        NyeAuth::Authorized => "nye.auth.authorized",
        NyeAuth::ConfirmationRequired => "nye.auth.confirmation_required",
        NyeAuth::Denied => "nye.auth.denied",
        NyeAuth::Unavailable => "nye.auth.unavailable",
        NyeAuth::PolicyBlocked => "nye.auth.policy_blocked",
    }
}

fn step_view(s: &NyeStepVm) -> impl IntoView {
    let (state, ic, key) = match s.state {
        NyeStepState::Queued => ("queued", "clock", "nye.step.queued"),
        NyeStepState::Running => ("running", "refresh", "nye.step.running"),
        NyeStepState::Waiting => ("waiting", "clock", "nye.step.waiting"),
        NyeStepState::Done => ("done", "check", "nye.step.done"),
        NyeStepState::Failed => ("failed", "warning", "nye.step.failed"),
        NyeStepState::Skipped => ("skipped", "close", "nye.step.skipped"),
        NyeStepState::AwaitingConfirmation => ("awaiting", "lock", "nye.step.awaiting"),
    };
    view! {
        <li class="oc-nye-step" data-state=state>
            <span class="oc-nye-step__mark" aria-hidden="true">{icon(ic)}</span>
            <span class="oc-nye-step__text">
                <span class="oc-nye-step__label">
                    {s.domain.clone().map(|d| view! { <span class="oc-nye-step__domain">{d}</span> })}
                    {s.label.clone()}
                </span>
                {s.detail.clone().map(|d| view! { <span class="oc-nye-step__detail">{d}</span> })}
            </span>
            <span class="oc-nye-step__state">{t(key)}</span>
        </li>
    }
}

/// A lista de actividade. `compact` = resumo expansível dentro da mensagem.
pub fn activity(steps: &[NyeStepVm], compact: bool) -> AnyView {
    if steps.is_empty() {
        return ().into_any();
    }
    let list = view! { <ol class="oc-nye-steps">{steps.iter().map(step_view).collect_view()}</ol> };
    if !compact {
        return list.into_any();
    }
    let mut domains: Vec<String> = Vec::new();
    for d in steps.iter().filter_map(|s| s.domain.clone()) {
        if !domains.contains(&d) {
            domains.push(d);
        }
    }
    let running = steps
        .iter()
        .find(|s| matches!(s.state, NyeStepState::Running | NyeStepState::Waiting));
    let summary = match running {
        Some(s) => match &s.domain {
            Some(d) => tf("nye.activity.working", &[("domain", d.as_str())]),
            None => s.label.clone(),
        },
        None => tf(
            "nye.activity.count",
            &[("n", steps.len().to_string().as_str())],
        ),
    };
    view! {
        <details class="oc-nye-acts">
            <summary class="oc-nye-acts__sum">
                {icon("activity")}
                <span>{summary}</span>
                {(!domains.is_empty()).then(|| view! { <span class="oc-nye-acts__domains">{domains.join(" · ")}</span> })}
                {icon("chev-d")}
            </summary>
            {list}
        </details>
    }
    .into_any()
}

fn source_item(s: &NyeSourceVm) -> impl IntoView {
    if !s.available {
        return view! {
            <li class="oc-nye-src" data-available="false">
                <span class="oc-nye-src__n">{s.n.to_string()}</span>
                <span class="oc-nye-src__icon" aria-hidden="true">{icon("lock")}</span>
                <span class="oc-nye-src__text"><span class="oc-nye-src__title">{t("nye.sources.unavailable")}</span></span>
            </li>
        }
        .into_any();
    }
    let external = s.trust == NyeTrust::External;
    let meta: Vec<String> = [
        Some(t(kind_key(s.kind)).to_owned()),
        (!s.context.is_empty()).then(|| s.context.clone()),
        s.locator.clone(),
        s.at.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let inner = view! {
        <span class="oc-nye-src__n">{s.n.to_string()}</span>
        <span class="oc-nye-src__icon" aria-hidden="true">{icon(kind_icon(s.kind))}</span>
        <span class="oc-nye-src__text">
            <span class="oc-nye-src__title">{s.title.clone()}</span>
            <span class="oc-nye-src__meta">{meta.join(" · ")}</span>
            {external.then(|| view! { <span class="oc-nye-src__trust">{icon("warning")}{t("nye.sources.external")}</span> })}
        </span>
    };
    view! {
        <li class="oc-nye-src" data-trust=if external { "external" } else { "ocinye" }>
            {match s.href.clone() {
                Some(h) => view! { <a class="oc-nye-src__link" href=h>{inner}<span class="oc-sr">{t("nye.sources.open")}</span></a> }.into_any(),
                None => view! { <span class="oc-nye-src__link">{inner}</span> }.into_any(),
            }}
        </li>
    }
    .into_any()
}

/// A lista de fontes. Uma fonte revogada fica, sem título nem excerto.
pub fn sources(list: &[NyeSourceVm]) -> AnyView {
    if list.is_empty() {
        return ().into_any();
    }
    let any_external = list
        .iter()
        .any(|s| s.trust == NyeTrust::External && s.available);
    view! {
        <ol class="oc-nye-srcs">{list.iter().map(source_item).collect_view()}</ol>
        {any_external.then(|| view! { <p class="oc-nye-note" data-tone="trust">{icon("shield")}<span>{t("nye.sources.external_note")}</span></p> })}
    }
    .into_any()
}

fn hits(groups: &[NyeHitGroupVm]) -> impl IntoView {
    groups
        .iter()
        .filter(|g| !g.hits.is_empty())
        .map(|g| {
            view! {
                <li class="oc-nye-group">
                    <p class="oc-nye-group__title">{t(group_key(g.kind))}</p>
                    <ul class="oc-nye-hits">
                        {g.hits.iter().map(|h| {
                            let meta: Vec<String> = [Some(h.context.clone()).filter(|c| !c.is_empty()), h.meta.clone()]
                                .into_iter().flatten().collect();
                            view! {
                                <li>
                                    <a class="oc-nye-hit" href=h.href.clone() data-part="nye-hit">
                                        <span class="oc-nye-hit__icon" aria-hidden="true">{icon(kind_icon(h.kind))}</span>
                                        <span class="oc-nye-hit__text">
                                            <span class="oc-nye-hit__title">{h.title.clone()}</span>
                                            <span class="oc-nye-hit__meta"><span class="oc-sr">{t(kind_key(h.kind))}" · "</span>{meta.join(" · ")}</span>
                                        </span>
                                        {h.app.clone().map(|a| view! { <span class="oc-nye-hit__app">{a}</span> })}
                                    </a>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                </li>
            }
        })
        .collect_view()
}

fn context_chip(c: &NyeContextVm) -> impl IntoView {
    let kind = match c.kind {
        NyeContextKind::Organization => "nye.ctx.kind.organization",
        NyeContextKind::Unit => "nye.ctx.kind.unit",
        NyeContextKind::Team => "nye.ctx.kind.team",
        NyeContextKind::Project => "nye.ctx.kind.project",
        NyeContextKind::Personal => "nye.ctx.kind.personal",
    };
    let state = match c.state {
        NyeContextState::Active => "active",
        NyeContextState::Changed => "changed",
        NyeContextState::Unavailable => "unavailable",
    };
    let label = if c.state == NyeContextState::Unavailable {
        t("nye.ctx.unavailable_short").to_owned()
    } else {
        c.label.clone()
    };
    let chip = view! {
        <span class="oc-nye-ctx__kind">{t(kind)}</span>
        <span class="oc-nye-ctx__name">{label}</span>
        {(c.state != NyeContextState::Unavailable).then(|| c.parent.clone()).flatten().map(|p| view! { <span class="oc-nye-ctx__parent">{p}</span> })}
    };
    view! {
        <span class="oc-nye-ctx" data-state=state>
            <span class="oc-sr">{t("nye.ctx.label")}": "</span>
            {icon("workspace")}
            {match c.change_href.clone() {
                Some(h) => view! { <a class="oc-nye-ctx__btn" href=h title=t("nye.ctx.change")>{chip}{icon("chev-d")}</a> }.into_any(),
                None => view! { <span class="oc-nye-ctx__btn">{chip}</span> }.into_any(),
            }}
        </span>
    }
}

fn context_note(c: &NyeContextVm) -> AnyView {
    match c.state {
        NyeContextState::Active => ().into_any(),
        NyeContextState::Changed => view! {
            <p class="oc-nye-note" role="status">{icon("workspace")}<span>{tf("nye.ctx.changed", &[("ctx", c.label.as_str())])}</span></p>
        }
        .into_any(),
        NyeContextState::Unavailable => view! {
            <p class="oc-nye-note" data-tone="warn" role="status">{icon("lock")}<span>{t("nye.ctx.unavailable")}</span></p>
        }
        .into_any(),
    }
}

/// A linha de disponibilidade. Só aparece quando algo está limitado; a falta
/// de IA nunca se diz «offline» (a pesquisa continua).
pub fn availability(a: &NyeAvailability) -> AnyView {
    match a.link {
        NyeLink::CoreUnavailable => {
            return view! {
                <div class="oc-nye-avail" data-state="down" role="alert">
                    {icon("warning")}
                    <span><strong>{t("nye.avail.core_down.title")}</strong>" "{t("nye.avail.core_down.body")}</span>
                </div>
            }
            .into_any()
        }
        NyeLink::Reconnecting => {
            return view! {
                <div class="oc-nye-avail" data-state="warn" role="status">
                    {icon("refresh")}<span>{t("nye.avail.reconnecting")}</span>
                </div>
            }
            .into_any()
        }
        NyeLink::Connected => {}
    }
    match a.ask {
        NyeAvail::Available => ().into_any(),
        NyeAvail::Unavailable(r) => view! {
            <div class="oc-nye-avail" data-state="limited" role="status">
                {icon("ai")}
                <span>
                    <strong>{t("nye.avail.no_inference.title")}</strong>" "
                    {t("nye.avail.no_inference.body")}
                    <span class="oc-nye-avail__why">{t(r.key())}</span>
                </span>
            </div>
        }
        .into_any(),
    }
}

/// A pastilha compacta de estado da Nye (não duplica o painel de estado D002).
fn status_chip(a: &NyeAvailability) -> impl IntoView {
    let part = |ok: bool, key: &'static str| {
        view! {
            <span class="oc-nye-chip__part" data-state=if ok { "ok" } else { "off" }>
                <span class="oc-nye-chip__dot" aria-hidden="true"></span>
                {t(key)}
                <span class="oc-sr">" · "{t(if ok { "nye.status.on" } else { "nye.status.off" })}</span>
            </span>
        }
    };
    view! {
        <span class="oc-nye-chip" role="group" aria-label=t("nye.status.label")>
            {part(a.search.is_available(), "nye.status.search")}
            {part(a.ask.is_available(), "nye.status.answers")}
            {part(a.voice_input.is_available(), "nye.status.voice")}
        </span>
    }
}

// ── Conteúdo seguro ──────────────────────────────────────────────────────

fn cites(list: &[u16]) -> impl IntoView {
    list.iter()
        .map(|n| view! { <sup class="oc-nye-cite"><span class="oc-sr">{t("nye.sources.ref")}" "</span>{n.to_string()}</sup> })
        .collect_view()
}

fn block(b: &NyeBlock) -> AnyView {
    match b {
        NyeBlock::Para { text, cites: c } => view! { <p>{text.clone()}{cites(c)}</p> }.into_any(),
        NyeBlock::Heading(h) => view! { <h4>{h.clone()}</h4> }.into_any(),
        NyeBlock::List(items) => view! { <ul>{items.iter().map(|i| view! { <li>{i.clone()}</li> }).collect_view()}</ul> }.into_any(),
        NyeBlock::Steps(items) => view! { <ol>{items.iter().map(|i| view! { <li>{i.clone()}</li> }).collect_view()}</ol> }.into_any(),
        NyeBlock::Code { lang, text } => view! {
            <pre class="oc-nye-code" data-lang=lang.clone()><code>{text.clone()}</code></pre>
        }
        .into_any(),
        NyeBlock::Quote { text, cite } => view! {
            <blockquote class="oc-nye-quote">{text.clone()}{cite.map(|n| cites(&[n]).into_any())}</blockquote>
        }
        .into_any(),
        NyeBlock::Table { head, rows } => view! {
            <div class="oc-nye-table"><table>
                <thead><tr>{head.iter().map(|h| view! { <th scope="col">{h.clone()}</th> }).collect_view()}</tr></thead>
                <tbody>{rows.iter().map(|r| view! { <tr>{r.iter().map(|c| view! { <td>{c.clone()}</td> }).collect_view()}</tr> }).collect_view()}</tbody>
            </table></div>
        }
        .into_any(),
    }
}

fn processing_note(p: NyeProcessing, egress_action: Option<String>) -> AnyView {
    match p {
        NyeProcessing::Local => view! { <span class="oc-nye-proc" data-state="local">{icon("shield")}{t("nye.proc.local")}</span> }.into_any(),
        NyeProcessing::External => view! { <span class="oc-nye-proc" data-state="external">{icon("ob-ext")}{t("nye.proc.external")}</span> }.into_any(),
        NyeProcessing::Blocked => view! {
            <p class="oc-nye-note" data-tone="block" role="status">{icon("lock")}<span>{t("nye.proc.blocked")}</span></p>
        }
        .into_any(),
        NyeProcessing::ApprovalRequired => view! {
            <div class="oc-nye-egress" role="group" aria-labelledby="oc-nye-egress-t">
                <p class="oc-nye-egress__text" id="oc-nye-egress-t">{icon("ob-ext")}<span>{t("nye.proc.approval")}</span></p>
                {egress_action.map(|a| view! {
                    <form class="oc-nye-egress__actions" method="post" action=a>
                        <button type="submit" class="oc-nye-btn" name="egress" value="local">{t("nye.proc.local_only")}</button>
                        <button type="submit" class="oc-nye-btn oc-nye-btn--line" name="egress" value="external">{t("nye.proc.approve")}</button>
                    </form>
                })}
            </div>
        }
        .into_any(),
    }
}

// ── Propostas e execução ─────────────────────────────────────────────────

/// Uma proposta de capacidade, com o seu estado de execução.
///
/// Confirmar e cancelar são formulários para as rotas existentes
/// (`/ask/plans/{id}/execute` · `/reject`); o `digest` vai com o pedido para que
/// o Workspace confirme ao Core exactamente o plano mostrado. Classes de alto
/// impacto não se confirmam no cartão: abrem [`confirm_dialog`].
pub fn proposal(p: &NyeProposalVm) -> impl IntoView {
    let id = format!("oc-nye-p-{}", p.plan_id);
    let title_id = format!("{id}-t");
    let decidable = p.auth == NyeAuth::ConfirmationRequired
        && p.state == NyeExecState::AwaitingConfirmation
        && !p.superseded;
    let blocked = matches!(
        p.auth,
        NyeAuth::Denied | NyeAuth::Unavailable | NyeAuth::PolicyBlocked
    );
    let exec = p.execution.clone();
    let execute = format!("/ask/plans/{}/execute", p.plan_id);
    let reject = format!("/ask/plans/{}/reject", p.plan_id);
    let review = format!("?confirm={}", p.plan_id);
    view! {
        <section class="oc-nye-prop" id=id data-risk=p.risk.as_str() data-state=p.state.as_str() data-superseded=p.superseded.then_some("") aria-labelledby=title_id.clone()>
            <header class="oc-nye-prop__head">
                <span class="oc-nye-prop__kicker">{t("nye.prop.kicker")}</span>
                {risk_view(p.risk)}
                <span class="oc-nye-prop__spacer"></span>
                <span role="status">{exec_view(p.state)}</span>
            </header>
            <h3 class="oc-nye-prop__title" id=title_id.clone()>{p.title.clone()}</h3>
            <dl class="oc-nye-prop__fields">
                <div class="oc-nye-kv"><dt>{t("nye.prop.target")}</dt><dd>{p.target.clone()}</dd></div>
                {p.scope.clone().map(|s| view! { <div class="oc-nye-kv"><dt>{t("nye.prop.scope")}</dt><dd>{s}</dd></div> })}
                {p.fields.iter().map(|f| view! {
                    <div class="oc-nye-kv" data-long=f.long.then_some("")><dt>{f.label.clone()}</dt><dd>{f.value.clone()}</dd></div>
                }).collect_view()}
            </dl>
            {(!p.lines.is_empty()).then(|| view! {
                <ul class="oc-nye-lines">
                    {p.lines.iter().map(|l| {
                        let ok = match l.ok { Some(true) => Some("ok"), Some(false) => Some("failed"), None => None };
                        let inner = view! {
                            <span class="oc-nye-line__mark" aria-hidden="true">{icon(match l.ok { Some(true) => "check", Some(false) => "warning", None => "chev-r" })}</span>
                            <span class="oc-nye-line__title">{l.title.clone()}</span>
                            {l.meta.clone().map(|m| view! { <span class="oc-nye-line__meta">{m}</span> })}
                        };
                        view! {
                            <li class="oc-nye-line" data-ok=ok>
                                {match l.href.clone() {
                                    Some(h) => view! { <a href=h>{inner}</a> }.into_any(),
                                    None => view! { <span>{inner}</span> }.into_any(),
                                }}
                            </li>
                        }
                    }).collect_view()}
                </ul>
            })}
            {(!p.consequences.is_empty()).then(|| view! {
                <div class="oc-nye-prop__cons">
                    <p class="oc-nye-prop__label">{t("nye.prop.consequences")}</p>
                    <ul>{p.consequences.iter().map(|c| view! { <li>{c.clone()}</li> }).collect_view()}</ul>
                </div>
            })}
            <p class="oc-nye-prop__auth" data-auth=if blocked { "blocked" } else { "ok" }>
                {icon(if blocked { "lock" } else { "shield" })}
                <span>{t(auth_key(p.auth))}{(p.auth == NyeAuth::Authorized && p.state == NyeExecState::Proposed).then(|| view! { " · "{t("nye.prop.no_confirm")} })}</span>
            </p>
            {p.cites_external.then(|| view! { <p class="oc-nye-note" data-tone="trust">{icon("shield")}<span>{t("nye.prop.external_origin")}</span></p> })}
            {p.superseded.then(|| view! { <p class="oc-nye-note">{icon("refresh")}<span>{t("nye.prop.superseded")}</span></p> })}
            {exec.clone().map(|e| view! {
                <div class="oc-nye-prop__exec">
                    {e.progress.map(|(d, n)| view! {
                        <p class="oc-nye-prop__progress">
                            <meter min="0" max=n.to_string() value=d.to_string()></meter>
                            <span>{tf("nye.exec.progress", &[("done", d.to_string().as_str()), ("total", n.to_string().as_str())])}</span>
                        </p>
                    })}
                    {e.summary.clone().map(|s| view! { <p class="oc-nye-prop__summary">{s}</p> })}
                    {e.error.map(|r| view! {
                        <p class="oc-state oc-state--error" role="alert">{icon("warning")}<span>{t(r.key())}{e.error_detail.clone().map(|d| view! { " "{d} })}</span></p>
                    })}
                </div>
            })}
            {decidable.then(|| {
                let strong = p.risk.high_impact();
                view! {
                    <div class="oc-nye-prop__actions">
                        <form method="post" action=reject.clone()>
                            <button type="submit" class="oc-nye-btn">{t("nye.prop.cancel")}</button>
                        </form>
                        {p.edit_href.clone().map(|h| view! { <a class="oc-nye-btn" href=h title=t("nye.prop.edit_note")>{icon("edit")}{t("nye.prop.edit")}</a> })}
                        <span class="oc-nye-prop__spacer"></span>
                        {if strong {
                            view! { <a class="oc-btn-gold" href=review.clone() data-oc="nye-review">{t("nye.prop.review")}</a> }.into_any()
                        } else {
                            view! {
                                <form method="post" action=execute.clone()>
                                    <input type="hidden" name="digest" value=p.digest.clone() />
                                    <button type="submit" class="oc-btn-gold">{icon("check")}{t("nye.prop.confirm")}</button>
                                </form>
                            }.into_any()
                        }}
                    </div>
                }
            })}
            {exec.and_then(|e| (e.retry_allowed).then_some(e.retry_action).flatten()).map(|a| view! {
                <form class="oc-nye-prop__actions" method="post" action=a>
                    <input type="hidden" name="digest" value=p.digest.clone() />
                    <button type="submit" class="oc-nye-btn">{icon("refresh")}{t("nye.exec.retry")}</button>
                </form>
            })}
            <details class="oc-nye-prop__details">
                <summary>{t("nye.prop.details")}{icon("chev-d")}</summary>
                <dl class="oc-nye-prop__fields oc-nye-prop__fields--mono">
                    <div class="oc-nye-kv"><dt>{t("nye.prop.capability")}</dt><dd>{p.capability.clone()}</dd></div>
                    {p.execution.as_ref().and_then(|e| e.audit_ref.clone()).map(|r| view! { <div class="oc-nye-kv"><dt>{t("nye.prop.audit")}</dt><dd>{r}</dd></div> })}
                    {p.execution.as_ref().and_then(|e| e.at.clone()).map(|a| view! { <div class="oc-nye-kv"><dt>{t("nye.prop.at")}</dt><dd>{a}</dd></div> })}
                    {p.expires_at.clone().map(|x| view! { <div class="oc-nye-kv"><dt>{t("nye.prop.expires")}</dt><dd>{x}</dd></div> })}
                </dl>
            </details>
        </section>
    }
}

/// A confirmação forte (EXTERNAL_COMMUNICATION, PRIVILEGED, DESTRUCTIVE).
///
/// Desenhada pela rota **depois** da casca (como `wm::dirty_close`), nunca dentro
/// de `.oc-desk`, `.oc-wm` ou da superfície da Nye: fica acima dela por ordem da
/// árvore. Só um acto explícito do membro conta como confirmação: nada no
/// conteúdo (página, documento, resposta de modelo) a substitui.
pub fn confirm_dialog(p: &NyeProposalVm) -> impl IntoView {
    let destructive = p.risk == NyeRisk::Destructive;
    let execute = format!("/ask/plans/{}/execute", p.plan_id);
    view! {
        <div class="oc-overlay oc-overlay--center" data-open="" data-oc="nye-confirm" role="alertdialog" aria-modal="true" aria-labelledby="oc-nye-confirm-t" aria-describedby="oc-nye-confirm-d">
            <form class="oc-dialog oc-nye-confirm" method="post" action=execute data-risk=p.risk.as_str()>
                <input type="hidden" name="digest" value=p.digest.clone() />
                <header class="oc-nye-confirm__head">
                    {risk_view(p.risk)}
                    <a class="oc-round-btn oc-round-btn--sm" href="?" data-oc="nye-confirm-cancel" aria-label=t("nye.prop.cancel")>{icon("close")}</a>
                </header>
                <p class="oc-nye-confirm__q">{t("nye.confirm.title")}</p>
                <h2 class="oc-dialog__title" id="oc-nye-confirm-t">{p.title.clone()}</h2>
                <dl class="oc-nye-prop__fields" id="oc-nye-confirm-d">
                    <div class="oc-nye-kv"><dt>{t("nye.prop.target")}</dt><dd>{p.target.clone()}</dd></div>
                    {p.scope.clone().map(|s| view! { <div class="oc-nye-kv"><dt>{t("nye.prop.scope")}</dt><dd>{s}</dd></div> })}
                    {p.fields.iter().map(|f| view! {
                        <div class="oc-nye-kv" data-long=f.long.then_some("")><dt>{f.label.clone()}</dt><dd>{f.value.clone()}</dd></div>
                    }).collect_view()}
                </dl>
                {(!p.consequences.is_empty()).then(|| view! {
                    <div class="oc-nye-prop__cons">
                        <p class="oc-nye-prop__label">{t("nye.prop.consequences")}</p>
                        <ul>{p.consequences.iter().map(|c| view! { <li>{c.clone()}</li> }).collect_view()}</ul>
                    </div>
                })}
                {destructive.then(|| view! { <p class="oc-nye-note" data-tone="block">{icon("warning")}<span>{t("nye.confirm.irreversible")}</span></p> })}
                {p.cites_external.then(|| view! { <p class="oc-nye-note" data-tone="trust">{icon("shield")}<span>{t("nye.prop.external_origin")}</span></p> })}
                <p class="oc-dialog__note">
                    {t("nye.confirm.immutable")}
                    {p.expires_at.clone().map(|x| view! { " "{tf("nye.confirm.expires", &[("time", x.as_str())])} })}
                </p>
                <div class="oc-dialog__actions">
                    <button type="submit" class="oc-btn-plain" formaction=format!("/ask/plans/{}/reject", p.plan_id) data-oc="nye-confirm-cancel">{t("nye.prop.cancel")}</button>
                    <span class="oc-dialog__spacer"></span>
                    <button type="submit" class=if destructive { "oc-btn-line oc-btn-line--danger" } else { "oc-btn-gold" } data-oc="nye-confirm-ok">{t("nye.prop.confirm")}</button>
                </div>
            </form>
        </div>
    }
}

// ── Mensagens ────────────────────────────────────────────────────────────

/// Uma mensagem. `compact` = dentro da superfície universal.
pub fn message(m: &NyeMessageVm, compact: bool) -> impl IntoView {
    let mine = m.role == NyeRole::Member;
    let (state, failed) = match m.state {
        NyeMsgState::Complete => ("complete", None),
        NyeMsgState::Streaming => ("streaming", None),
        NyeMsgState::Interrupted => ("interrupted", None),
        NyeMsgState::Failed(r) => ("failed", Some(r)),
    };
    let streaming = m.state == NyeMsgState::Streaming;
    let head_id = format!("oc-nye-m-{}", m.id);
    let grounding = match m.grounding {
        NyeGrounding::Ungrounded => Some(("none", "help", "nye.grounding.none")),
        NyeGrounding::Revoked => Some(("revoked", "lock", "nye.grounding.revoked")),
        _ => None,
    };
    let processing = m.processing;
    view! {
        <article class="oc-nye-msg" data-role=if mine { "member" } else { "nye" } data-state=state data-compact=compact.then_some("") aria-labelledby=head_id.clone()>
            <header class="oc-nye-msg__head" id=head_id.clone()>
                {(!mine).then(|| view! { <span class="oc-nye-mark" aria-hidden="true">{icon("nye")}</span> })}
                <strong class="oc-nye-msg__who">{t(if mine { "nye.you" } else { "nye.name" })}</strong>
                <time class="oc-nye-msg__at">{m.at.clone()}</time>
                {processing.filter(|p| matches!(p, NyeProcessing::Local | NyeProcessing::External)).map(|p| processing_note(p, None))}
            </header>
            {(!mine).then(|| activity(&m.steps, true))}
            {(!m.attachments.is_empty()).then(|| view! {
                <ul class="oc-nye-atts">{m.attachments.iter().map(|a| view! {
                    <li class="oc-nye-att">{icon(kind_icon(a.kind))}<span class="oc-nye-att__name">{a.name.clone()}</span>{a.size.clone().map(|s| view! { <span class="oc-nye-att__meta">{s}</span> })}</li>
                }).collect_view()}</ul>
            })}
            <div
                class="oc-nye-msg__body"
                data-oc=streaming.then_some("nye-stream")
                data-src=m.stream_src.clone()
                aria-busy=streaming.then_some("true")
            >
                {m.blocks.iter().map(block).collect_view()}
                {streaming.then(|| view! { <span class="oc-nye-caret" aria-hidden="true"></span> })}
            </div>
            {processing.filter(|p| matches!(p, NyeProcessing::Blocked | NyeProcessing::ApprovalRequired)).map(|p| processing_note(p, m.egress_action.clone()))}
            {grounding.map(|(g, ic, key)| view! { <p class="oc-nye-note" data-grounding=g>{icon(ic)}<span>{t(key)}</span></p> })}
            {(!m.hits.is_empty()).then(|| view! {
                <p class="oc-nye-det">{icon("search")}{t("nye.results.deterministic")}</p>
                <ul class="oc-nye-groups">{hits(&m.hits)}</ul>
            })}
            {(!m.sources.is_empty()).then(|| view! {
                <div class="oc-nye-msg__sources">
                    <p class="oc-nye-msg__label">{tf("nye.sources.count", &[("n", m.sources.len().to_string().as_str())])}</p>
                    {sources(&m.sources)}
                </div>
            })}
            {m.proposals.iter().map(proposal).collect_view()}
            {failed.map(|r| view! { <p class="oc-state oc-state--error" role="alert">{icon("warning")}<span>{t(r.key())}</span></p> })}
            {(m.state == NyeMsgState::Interrupted).then(|| view! { <p class="oc-nye-note">{icon("close")}<span>{t("nye.msg.interrupted")}</span></p> })}
            {(streaming || m.retry_action.is_some()).then(|| view! {
                <div class="oc-nye-msg__actions">
                    {streaming.then_some(m.stop_action.clone()).flatten().map(|a| view! {
                        <form method="post" action=a><button type="submit" class="oc-nye-btn">{icon("stop")}{t("nye.msg.stop")}</button></form>
                    })}
                    {(!streaming).then_some(m.retry_action.clone()).flatten().map(|a| view! {
                        <form method="post" action=a><button type="submit" class="oc-nye-btn">{icon("refresh")}{t("nye.msg.retry")}</button></form>
                    })}
                </div>
            })}
        </article>
    }
}

// ── A · Superfície universal (paleta alargada) ──────────────────────────

fn mode(
    value: &'static str,
    key: &'static str,
    hint: &'static str,
    checked: bool,
    avail: NyeAvail,
) -> impl IntoView {
    let id = format!(
        "oc-nye-mode-{}",
        if value.is_empty() { "auto" } else { value }
    );
    let why = match avail {
        NyeAvail::Available => None,
        NyeAvail::Unavailable(r) => Some(r),
    };
    let why_id = format!("{id}-why");
    view! {
        <label class="oc-nye-mode" for=id.clone() title=t(hint)>
            <input
                type="radio"
                name="intent"
                id=id.clone()
                value=value
                checked=checked
                disabled=why.is_some()
                aria-describedby=why.is_some().then(|| why_id.clone())
                data-part="nye-intent"
            />
            <span>{t(key)}</span>
            {why.map(|r| view! { <span class="oc-sr" id=why_id>{t(r.key())}</span> })}
        </label>
    }
}

/// A superfície universal da Nye: a paleta de comandos D001 alargada.
///
/// Preserva a paleta: `#oc-palette`, `data-oc="palette"`, o campo
/// `data-part="palette-q"` e a lista `data-part="palette-item"` de aplicações
/// filtrada no cliente (determinística, sem rede), ⌘K e Esc. Acrescenta: o tipo
/// de pedido (Automático / Pesquisar / Perguntar / Executar), a disponibilidade,
/// o contexto, os resultados do Core e uma resposta ou proposta curta. Envia
/// para `/ask` (Universal Command Surface existente) em vez de `/search`.
pub fn surface(vm: &ShellVm, n: &NyeSurfaceVm) -> impl IntoView {
    let a = n.availability;
    let chosen = n.intent;
    let intent_attr = chosen.map_or("auto", NyeIntent::as_str);
    let placeholder = match chosen {
        None => "nye.placeholder.auto",
        Some(NyeIntent::Search) => "nye.placeholder.search",
        Some(NyeIntent::Ask) => "nye.placeholder.ask",
        Some(NyeIntent::Act) => "nye.placeholder.act",
    };
    let detected = n.detected.filter(|_| chosen.is_none()).map(|d| {
        let key = match d {
            NyeIntent::Search => "nye.intent.search",
            NyeIntent::Ask => "nye.intent.ask",
            NyeIntent::Act => "nye.intent.act",
        };
        view! { <p class="oc-nye-read" role="status">{tf("nye.intent.detected", &[("intent", t(key))])}</p> }
    });
    let has_hits = n.hits.iter().any(|g| !g.hits.is_empty());
    let asked = !n.query.is_empty();
    view! {
        <div class="oc-overlay" id="oc-palette" data-oc="palette" data-nye="" data-open=n.open.then_some("") role="dialog" aria-modal="true" aria-labelledby="oc-nye-title">
            <a class="oc-overlay__scrim" href="#" aria-label=t("shell.close")></a>
            <form class="oc-palette oc-nyeu" method="get" action="/ask" role="search" data-oc="nye-surface" data-intent=intent_attr>
                <h2 class="oc-sr" id="oc-nye-title">{t("nye.surface.label")}</h2>
                <div class="oc-nyeu__head">
                    <label class="oc-field oc-field--lg oc-nyeu__field">
                        <span class="oc-sr">{t("nye.surface.label")}</span>
                        <span class="oc-nye-mark" aria-hidden="true">{icon("nye")}</span>
                        <input
                            name="q"
                            type="search"
                            data-part="palette-q"
                            value=n.query.clone()
                            placeholder=t(placeholder)
                            data-ph-auto=t("nye.placeholder.auto")
                            data-ph-search=t("nye.placeholder.search")
                            data-ph-ask=t("nye.placeholder.ask")
                            data-ph-act=t("nye.placeholder.act")
                            autocomplete="off"
                            aria-describedby="oc-nye-keys"
                        />
                    </label>
                    <button type="submit" class="oc-nyeu__go" aria-label=t("nye.submit")>{icon("arrow-r")}</button>
                </div>
                <div class="oc-nyeu__bar">
                    <fieldset class="oc-nye-modes">
                        <legend class="oc-sr">{t("nye.intent.legend")}</legend>
                        {mode("", "nye.intent.auto", "nye.intent.hint.auto", chosen.is_none(), NyeAvail::Available)}
                        {mode("search", "nye.intent.search", "nye.intent.hint.search", chosen == Some(NyeIntent::Search), a.search)}
                        {mode("ask", "nye.intent.ask", "nye.intent.hint.ask", chosen == Some(NyeIntent::Ask), a.ask)}
                        {mode("act", "nye.intent.act", "nye.intent.hint.act", chosen == Some(NyeIntent::Act), a.act)}
                    </fieldset>
                    {n.context.as_ref().map(context_chip)}
                </div>
                {availability(&a)}
                {detected}
                <div class="oc-nyeu__body" data-part="nye-results">
                    {n.answer.as_ref().map(|m| message(m, true))}
                    {has_hits.then(|| view! {
                        <p class="oc-nye-det">{icon("search")}{t("nye.results.deterministic")}</p>
                    })}
                    <ul class="oc-nye-groups">
                        {hits(&n.hits)}
                        <li class="oc-nye-group" data-part="nye-apps">
                            <p class="oc-nye-group__title">{t("nye.group.app")}</p>
                            <ul class="oc-palette__list oc-nye-hits">
                                {vm.apps.iter().map(|app| {
                                    let search = app.label.to_lowercase();
                                    view! {
                                        <li data-part="palette-item" data-search=search>
                                            <a class="oc-nye-hit" href=app.href data-part="nye-hit">
                                                <span class="oc-nye-hit__icon" aria-hidden="true">{icon(crate::ui::components::app_icon(app.href))}</span>
                                                <span class="oc-nye-hit__text"><span class="oc-nye-hit__title">{app.label.clone()}</span></span>
                                                <span class="oc-nye-hit__app">{t("nye.kind.app")}</span>
                                            </a>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        </li>
                    </ul>
                    {(asked && !has_hits && n.answer.is_none()).then(|| view! {
                        <p class="oc-nye-empty" role="status" data-part="nye-empty">{tf("nye.results.none", &[("q", n.query.as_str())])}</p>
                    })}
                </div>
                <p class="oc-palette__foot oc-nyeu__foot">
                    <span class="oc-nyeu__keys" id="oc-nye-keys">
                        {t("nye.keys.hint")}
                        {n.shortcut.clone().map(|s| view! { " · "{tf("nye.shortcut", &[("keys", s.as_str())])} })}
                    </span>
                    <a class="oc-nyeu__continue" href=n.continue_href.clone()>{icon("nye")}{t("nye.open_in_nye")}</a>
                </p>
            </form>
        </div>
    }
}

// ── C · A aplicação Nye ──────────────────────────────────────────────────

fn composer(c: &NyeComposerVm, a: &NyeAvailability) -> impl IntoView {
    let (state, note) = match c.state {
        NyeComposerState::Idle => ("idle", None),
        NyeComposerState::Submitting => ("submitting", Some("nye.comp.submitting")),
        NyeComposerState::Streaming => ("streaming", None),
        NyeComposerState::ProposalPending => ("pending", Some("nye.comp.pending")),
        NyeComposerState::Unavailable(_) => ("unavailable", Some("nye.comp.unavailable")),
    };
    let busy = matches!(
        c.state,
        NyeComposerState::Submitting | NyeComposerState::Streaming
    );
    let attach_ok = a.attachments.is_available() && c.attach_href.is_some();
    let voice_ok = a.voice_input.is_available();
    view! {
        <form class="oc-nye-comp" method="post" action=c.action.clone() data-oc="nye-composer" data-state=state>
            {(!c.attachments.is_empty()).then(|| view! {
                <ul class="oc-nye-atts">
                    {c.attachments.iter().map(|at| view! {
                        <li class="oc-nye-att">
                            <input type="hidden" name="attachment" value=at.value.clone() />
                            {icon(kind_icon(at.kind))}
                            <span class="oc-nye-att__name">{at.name.clone()}</span>
                            <span class="oc-nye-att__meta">{[at.size.clone(), Some(at.context.clone())].into_iter().flatten().collect::<Vec<_>>().join(" · ")}</span>
                            <button type="submit" class="oc-nye-att__x" name="remove" value=at.value.clone() formaction=c.action.clone() aria-label=tf("nye.comp.remove", &[("name", at.name.as_str())])>{icon("close")}</button>
                        </li>
                    }).collect_view()}
                </ul>
            })}
            <label class="oc-sr" for="oc-nye-input">{t("nye.comp.label")}</label>
            <textarea id="oc-nye-input" name="q" rows="1" data-part="nye-input" placeholder=t("nye.comp.placeholder") aria-describedby="oc-nye-comp-hint">{c.text.clone()}</textarea>
            <div class="oc-nye-comp__bar">
                {if attach_ok {
                    view! { <a class="oc-nye-icon-btn" href=c.attach_href.clone().unwrap_or_default() aria-label=t("nye.comp.attach") title=t("nye.comp.attach")>{icon("attach")}</a> }.into_any()
                } else {
                    view! { <button type="button" class="oc-nye-icon-btn" aria-disabled="true" aria-label=t("nye.comp.attach") title=t("nye.comp.attach_na")>{icon("attach")}</button> }.into_any()
                }}
                {if voice_ok {
                    view! { <a class="oc-nye-icon-btn" href="?voice=1" data-oc="nye-voice-open" aria-label=t("nye.comp.voice") title=t("nye.comp.voice")>{icon("mic")}</a> }.into_any()
                } else {
                    view! { <button type="button" class="oc-nye-icon-btn" aria-disabled="true" aria-label=t("nye.comp.voice") title=t("nye.voice.unavailable")>{icon("mic")}</button> }.into_any()
                }}
                <span class="oc-nye-comp__hint" id="oc-nye-comp-hint">{note.map_or_else(|| t("nye.comp.hint"), t)}</span>
                {if busy && c.stop_action.is_some() {
                    view! { <button type="submit" class="oc-nye-send" data-kind="stop" formaction=c.stop_action.clone().unwrap_or_default() aria-label=t("nye.comp.stop") title=t("nye.comp.stop")>{icon("stop")}</button> }.into_any()
                } else {
                    view! { <button type="submit" class="oc-nye-send" aria-label=t("nye.comp.send") title=t("nye.comp.send")>{icon("send")}</button> }.into_any()
                }}
            </div>
        </form>
    }
}

fn rail(vm: &NyeAppVm) -> impl IntoView {
    let list = match &vm.conversations {
        None => view! { <p class="oc-pending oc-nye-rail__gap" role="status">{icon("clock")}<span>{t("nye.app.history_gap")}</span></p> }.into_any(),
        Some(items) if items.is_empty() => view! { <p class="oc-nye-rail__empty">{t("nye.app.history_empty")}</p> }.into_any(),
        Some(items) => view! {
            <ul class="oc-nye-convs">
                {items.iter().map(|c| view! {
                    <li><a class="oc-nye-conv" href=c.href.clone() aria-current=c.active.then_some("page")>
                        <span class="oc-nye-conv__title">{c.title.clone()}</span>
                        <span class="oc-nye-conv__at">{c.at.clone()}</span>
                    </a></li>
                }).collect_view()}
            </ul>
        }
        .into_any(),
    };
    view! {
        <aside class="oc-nye__rail" id="oc-nye-rail" data-part="nye-drawer" aria-label=t("nye.app.history")>
            <div class="oc-nye__rail-head">
                <a class="oc-btn-gold oc-nye__new" href="/ai/prompt?new=1">{icon("plus")}{t("nye.app.new")}</a>
                <button type="button" class="oc-nye-icon-btn oc-nye__drawer-x" data-oc="nye-drawer-close" aria-label=t("nye.app.panel.close")>{icon("close")}</button>
            </div>
            {vm.conversations.is_some().then(|| view! {
                <form class="oc-nye-rail__search" method="get" action="/ai/prompt" role="search">
                    <label class="oc-field">
                        <span class="oc-sr">{t("nye.app.search_conv")}</span>
                        {icon("search")}
                        <input type="search" name="cq" value=vm.conv_query.clone() placeholder=t("nye.app.search_conv") autocomplete="off" />
                    </label>
                </form>
            })}
            <p class="oc-nye-rail__title">{t("nye.app.history")}</p>
            {list}
            <p class="oc-nye-rail__note">{t("nye.app.history_note")}</p>
        </aside>
    }
}

fn side(vm: &NyeAppVm) -> AnyView {
    if vm.panel == NyePanel::None {
        return ().into_any();
    }
    let (title, body) = match vm.panel {
        NyePanel::Sources => (
            "nye.sources.title",
            if vm.panel_sources.is_empty() {
                view! { <p class="oc-nye-rail__empty">{t("nye.sources.empty")}</p> }.into_any()
            } else {
                sources(&vm.panel_sources)
            },
        ),
        _ => (
            "nye.activity.title",
            if vm.panel_steps.is_empty() {
                view! { <p class="oc-nye-rail__empty">{t("nye.activity.empty")}</p> }.into_any()
            } else {
                view! { <p class="oc-nye-side__note">{t("nye.activity.note")}</p>{activity(&vm.panel_steps, false)} }.into_any()
            },
        ),
    };
    view! {
        <aside class="oc-nye__side" id="oc-nye-side" data-part="nye-drawer" data-open="" aria-labelledby="oc-nye-side-t">
            <div class="oc-nye__side-head">
                <nav class="oc-nye-tabs" aria-label=t("nye.app.panels")>
                    <a class="oc-nye-tab" href="?panel=sources" aria-current=(vm.panel == NyePanel::Sources).then_some("page")>{t("nye.sources.title")}</a>
                    <a class="oc-nye-tab" href="?panel=activity" aria-current=(vm.panel == NyePanel::Activity).then_some("page")>{t("nye.activity.title")}</a>
                </nav>
                <a class="oc-nye-icon-btn" href="?panel=none" data-oc="nye-drawer-close" aria-label=t("nye.app.panel.close")>{icon("close")}</a>
            </div>
            <h3 class="oc-sr" id="oc-nye-side-t">{t(title)}</h3>
            {body}
        </aside>
    }
    .into_any()
}

/// D · Voz: premir para falar. O microfone só está ligado em `Listening`, e o
/// indicador de gravação é explícito (ponto, texto e `aria-pressed`). O
/// `oc-nye.js` emite intenções `oc:nye` (`voice-start`/`voice-stop`/…); o runtime
/// e o STT/TTS são do Code. Sem palavra de activação.
pub fn voice(v: &NyeVoiceVm, a: &NyeAvailability) -> impl IntoView {
    let (state, key, why) = match v.state {
        NyeVoiceState::Idle => ("idle", "nye.voice.idle", None),
        NyeVoiceState::RequestingPermission => ("requesting", "nye.voice.requesting", None),
        NyeVoiceState::Listening => ("listening", "nye.voice.listening", None),
        NyeVoiceState::Transcribing => ("transcribing", "nye.voice.transcribing", None),
        NyeVoiceState::Processing => ("processing", "nye.voice.processing", None),
        NyeVoiceState::Speaking => ("speaking", "nye.voice.speaking", None),
        NyeVoiceState::Stopped => ("stopped", "nye.voice.stopped", None),
        NyeVoiceState::Unavailable(r) => ("unavailable", "nye.voice.unavailable", Some(r)),
        NyeVoiceState::Error(r) => ("error", "nye.voice.error", Some(r)),
    };
    let listening = v.state == NyeVoiceState::Listening;
    let can_talk = a.voice_input.is_available()
        && matches!(
            v.state,
            NyeVoiceState::Idle
                | NyeVoiceState::Listening
                | NyeVoiceState::Stopped
                | NyeVoiceState::Speaking
        );
    let lang_opt = |l: NyeLang, key: &'static str| {
        view! { <option value=l.as_str() selected={v.lang == l}>{t(key)}</option> }
    };
    view! {
        <section class="oc-nye-voice" data-oc="nye-voice" data-state=state aria-labelledby="oc-nye-voice-t">
            <h3 class="oc-sr" id="oc-nye-voice-t">{t("nye.voice.title")}</h3>
            <p class="oc-nye-voice__rec" data-on=listening.then_some("") role="status">
                <span class="oc-nye-voice__dot" aria-hidden="true"></span>
                <span>{t(key)}</span>
            </p>
            {why.map(|r| view! { <p class="oc-nye-voice__why">{t(r.key())}</p> })}
            {if can_talk {
                view! {
                    <button
                        type="button"
                        class="oc-nye-ptt"
                        data-oc="nye-ptt"
                        aria-pressed=if listening { "true" } else { "false" }
                        aria-describedby="oc-nye-voice-privacy"
                    >
                        <span class="oc-nye-ptt__ring" aria-hidden="true">{icon("mic")}</span>
                        <span class="oc-nye-ptt__label">{t(if listening { "nye.voice.stop_listening" } else { "nye.voice.ptt" })}</span>
                    </button>
                }.into_any()
            } else {
                view! {
                    <button type="button" class="oc-nye-ptt" aria-disabled="true" aria-describedby="oc-nye-voice-privacy">
                        <span class="oc-nye-ptt__ring" aria-hidden="true">{icon("mic")}</span>
                        <span class="oc-nye-ptt__label">{t("nye.voice.ptt")}</span>
                    </button>
                }.into_any()
            }}
            <p class="oc-nye-voice__hint">{t("nye.voice.ptt_hold")}</p>
            {v.transcript.clone().map(|tr| view! { <blockquote class="oc-nye-voice__transcript" aria-live="polite">{tr}</blockquote> })}
            <div class="oc-nye-voice__controls">
                {(v.state == NyeVoiceState::Speaking).then(|| view! {
                    <button type="button" class="oc-nye-btn" data-oc="nye-voice-stop-speaking">{icon("stop")}{t("nye.voice.stop_speaking")}</button>
                })}
                {(v.replay && a.voice_output.is_available() && v.state != NyeVoiceState::Speaking).then(|| view! {
                    <button type="button" class="oc-nye-btn" data-oc="nye-voice-replay">{icon("volume")}{t("nye.voice.replay")}</button>
                })}
                <label class="oc-nye-voice__lang">
                    <span>{t("nye.voice.lang")}</span>
                    <select name="voice_lang" data-oc="nye-voice-lang">
                        {lang_opt(NyeLang::Pt, "nye.voice.lang.pt")}
                        {lang_opt(NyeLang::En, "nye.voice.lang.en")}
                        {lang_opt(NyeLang::Fr, "nye.voice.lang.fr")}
                    </select>
                </label>
                {v.lang_detected.then(|| view! { <span class="oc-nye-voice__det">{t("nye.voice.lang_detected")}</span> })}
                <span class="oc-nye-prop__spacer"></span>
                <a class="oc-nye-btn" href="?voice=0" data-oc="nye-voice-close">{icon("type")}{t("nye.voice.to_text")}</a>
            </div>
            <p class="oc-nye-voice__privacy" id="oc-nye-voice-privacy">{icon("shield")}<span>{t("nye.voice.privacy")}</span></p>
        </section>
    }
}

/// A aplicação Nye. É o corpo de uma janela gerida (D002 · `WindowContent::Ready`),
/// com a política de lançamento do registo (`Prompt`, `SingleInstance`). As
/// várias conversas vivem dentro da aplicação, não em janelas.
///
/// Largura: o layout responde à largura da **janela** (container queries), não
/// do ecrã; não há motor responsivo próprio. Estreita, a lista de conversas e o
/// painel lateral são gavetas.
pub fn app(vm: &NyeAppVm) -> impl IntoView {
    let title = vm
        .current
        .as_ref()
        .map_or_else(|| t("nye.app.new").to_owned(), |c| c.title.clone());
    let panel = match vm.panel {
        NyePanel::None => "none",
        NyePanel::Sources => "sources",
        NyePanel::Activity => "activity",
    };
    let streaming = vm
        .current
        .as_ref()
        .is_some_and(|c| c.messages.iter().any(|m| m.state == NyeMsgState::Streaming));
    let body = match (&vm.voice, &vm.current) {
        (Some(v), _) => voice(v, &vm.availability).into_any(),
        (None, None) => view! {
            <div class="oc-nye-start">
                <span class="oc-nye-mark oc-nye-mark--lg" aria-hidden="true">{icon("nye")}</span>
                <p class="oc-nye-start__title">{t("nye.app.empty.title")}</p>
                <p class="oc-nye-start__body">{t("nye.app.empty.body")}</p>
                {(!vm.suggestions.is_empty()).then(|| view! {
                    <ul class="oc-nye-sugs">
                        {vm.suggestions.iter().map(|s| {
                            let href = format!("/ai/prompt?q={}", s.replace(' ', "+"));
                            view! { <li><a class="oc-nye-sug" href=href>{s.clone()}</a></li> }
                        }).collect_view()}
                    </ul>
                })}
            </div>
        }
        .into_any(),
        (None, Some(c)) => view! {
            <div class="oc-nye-log">{c.messages.iter().map(|m| message(m, false)).collect_view()}</div>
        }
        .into_any(),
    };
    view! {
        <div class="oc-nye" data-oc="nye-app" data-panel=panel>
            <div class="oc-nye__grid">
            {rail(vm)}
            <section class="oc-nye__main" aria-labelledby="oc-nye-conv-t">
                <header class="oc-nye__bar">
                    <button type="button" class="oc-nye-icon-btn oc-nye__rail-btn" data-oc="nye-drawer" aria-controls="oc-nye-rail" aria-expanded="false" aria-label=t("nye.app.history")>{icon("sidebar")}</button>
                    <h2 class="oc-nye__title" id="oc-nye-conv-t">{title}</h2>
                    {vm.context.as_ref().map(context_chip)}
                    <span class="oc-nye-prop__spacer"></span>
                    {status_chip(&vm.availability)}
                    <a class="oc-nye-icon-btn" href="?panel=sources" aria-current=(vm.panel == NyePanel::Sources).then_some("page") aria-label=t("nye.sources.title") title=t("nye.sources.title")>{icon("link")}</a>
                    <a class="oc-nye-icon-btn" href="?panel=activity" aria-current=(vm.panel == NyePanel::Activity).then_some("page") aria-label=t("nye.activity.title") title=t("nye.activity.title")>{icon("activity")}</a>
                </header>
                {availability(&vm.availability)}
                {vm.context.as_ref().map(context_note)}
                <div class="oc-nye__scroll" data-part="nye-scroll">{body}</div>
                <p class="oc-sr" role="status" aria-live="polite" data-part="nye-live" data-done=t("nye.msg.done")>{streaming.then(|| t("nye.msg.streaming"))}</p>
                {vm.voice.is_none().then(|| composer(&vm.composer, &vm.availability))}
            </section>
            {side(vm)}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{
        NyeAvail, NyeAvailability, NyeComposerVm, NyeConvItemVm, NyeConversationVm, NyeExecutionVm,
        NyeField, NyeHitVm, NyeReason,
    };

    fn avail(ai: bool) -> NyeAvailability {
        let off = NyeAvail::Unavailable(NyeReason::NoInference);
        NyeAvailability {
            search: NyeAvail::Available,
            ask: if ai { NyeAvail::Available } else { off },
            act: if ai { NyeAvail::Available } else { off },
            voice_input: NyeAvail::Unavailable(NyeReason::VoiceUnavailable),
            voice_output: NyeAvail::Unavailable(NyeReason::VoiceUnavailable),
            attachments: NyeAvail::Unavailable(NyeReason::CapabilityUnavailable),
            link: NyeLink::Connected,
        }
    }

    fn prop(risk: NyeRisk, auth: NyeAuth, state: NyeExecState) -> NyeProposalVm {
        NyeProposalVm {
            plan_id: "p1".into(),
            digest: "d1".into(),
            title: "Criar 3 tarefas".into(),
            capability: "collaboration.task.create".into(),
            target: "Projeto Solander".into(),
            scope: None,
            fields: vec![NyeField {
                label: "Prazo".into(),
                value: "10 out".into(),
                long: false,
            }],
            lines: vec![],
            consequences: vec![],
            risk,
            auth,
            state,
            edit_href: None,
            superseded: false,
            cites_external: false,
            expires_at: None,
            execution: None,
        }
    }

    fn surf(ai: bool) -> NyeSurfaceVm {
        NyeSurfaceVm {
            availability: avail(ai),
            intent: None,
            detected: Some(NyeIntent::Search),
            query: "vento".into(),
            hits: vec![NyeHitGroupVm {
                kind: NyeKind::File,
                hits: vec![NyeHitVm {
                    kind: NyeKind::File,
                    title: "Medições de vento — Namibe.csv".into(),
                    context: "Projetos / Solander".into(),
                    meta: Some("há 2 h".into()),
                    app: Some("Ficheiros".into()),
                    href: "/files/x".into(),
                }],
            }],
            answer: None,
            context: None,
            continue_href: "/ai/prompt?q=vento".into(),
            shortcut: None,
            open: true,
        }
    }

    #[test]
    fn a_superficie_preserva_a_paleta_d001() {
        let mut vm = crate::ui::shell::tests::vm();
        vm.nye = Some(surf(true));
        let html = surface(&vm, vm.nye.as_ref().unwrap()).to_html();
        assert_contracts(&html);
        for hook in [
            r#"id="oc-palette""#,
            r#"data-oc="palette""#,
            r#"data-part="palette-q""#,
            r#"data-part="palette-item""#,
        ] {
            assert!(html.contains(hook), "gancho D001 {hook}");
        }
        assert!(html.contains(r#"action="/ask""#) && html.contains(r#"name="intent""#));
    }

    #[test]
    fn sem_inferencia_a_pesquisa_continua_e_perguntar_fica_indisponivel() {
        let vm = crate::ui::shell::tests::vm();
        let html = surface(&vm, &surf(false)).to_html();
        assert!(html.contains(t("nye.avail.no_inference.title")));
        assert!(
            html.contains("Medições de vento") && html.contains(t("nye.results.deterministic"))
        );
        assert!(
            html.contains(r#"id="oc-nye-mode-ask-why""#)
                && html.contains(t("nye.reason.no_inference"))
        );
        assert!(!html.to_lowercase().contains("offline"));
    }

    #[test]
    fn so_o_core_decide_a_confirmacao() {
        // O risco explica; não decide. Autorizada sem confirmação → sem botões.
        let html = proposal(&prop(
            NyeRisk::ReversibleWrite,
            NyeAuth::Authorized,
            NyeExecState::Proposed,
        ))
        .to_html();
        assert!(!html.contains("/execute") && html.contains(t("nye.prop.no_confirm")));
        let html = proposal(&prop(
            NyeRisk::ReversibleWrite,
            NyeAuth::ConfirmationRequired,
            NyeExecState::AwaitingConfirmation,
        ))
        .to_html();
        assert_contracts(&html);
        assert!(
            html.contains(r#"action="/ask/plans/p1/execute""#)
                && html.contains(r#"name="digest" value="d1""#)
        );
    }

    #[test]
    fn alto_impacto_confirma_num_dialogo_global() {
        let p = prop(
            NyeRisk::ExternalCommunication,
            NyeAuth::ConfirmationRequired,
            NyeExecState::AwaitingConfirmation,
        );
        let card = proposal(&p).to_html();
        assert!(!card.contains("/execute") && card.contains(r#"data-oc="nye-review""#));
        let dlg = confirm_dialog(&p).to_html();
        assert_contracts(&dlg);
        assert!(dlg.contains(r#"role="alertdialog""#) && dlg.contains(r#"name="digest""#));
    }

    #[test]
    fn repetir_uma_accao_so_quando_o_executor_o_permite() {
        let mut p = prop(
            NyeRisk::ReversibleWrite,
            NyeAuth::Authorized,
            NyeExecState::Failed,
        );
        p.execution = Some(NyeExecutionVm {
            summary: None,
            error: Some(NyeReason::ExecutionFailed),
            error_detail: None,
            retry_allowed: false,
            retry_action: Some("/ask/plans/p1/retry".into()),
            audit_ref: Some("AUD-1".into()),
            at: None,
            progress: None,
        });
        assert!(!proposal(&p).to_html().contains("/retry"));
        p.execution.as_mut().unwrap().retry_allowed = true;
        assert!(proposal(&p).to_html().contains("/retry"));
    }

    #[test]
    fn a_fonte_revogada_nao_mostra_conteudo() {
        let s = NyeSourceVm {
            n: 1,
            kind: NyeKind::Note,
            title: "Segredo".into(),
            context: "Unidade X".into(),
            locator: None,
            at: None,
            href: Some("/notes/1".into()),
            available: false,
            trust: NyeTrust::Ocinye,
        };
        let html = sources(&[s]).to_html();
        assert!(
            !html.contains("Segredo") && !html.contains("Unidade X") && !html.contains("/notes/1")
        );
    }

    #[test]
    fn a_aplicacao_sem_historico_no_core_diz_a_verdade() {
        let vm = NyeAppVm {
            availability: avail(true),
            context: None,
            conversations: None,
            conv_query: String::new(),
            current: Some(NyeConversationVm {
                title: "x".into(),
                messages: vec![],
            }),
            composer: NyeComposerVm {
                action: "/ai/prompt".into(),
                text: String::new(),
                attachments: vec![],
                state: NyeComposerState::Idle,
                stop_action: None,
                attach_href: None,
            },
            voice: None,
            panel: NyePanel::None,
            panel_sources: vec![],
            panel_steps: vec![],
            suggestions: vec![],
        };
        let html = app(&vm).to_html();
        assert_contracts(&html);
        assert!(html.contains(t("nye.app.history_gap")));
        let _ = NyeConvItemVm {
            title: String::new(),
            at: String::new(),
            href: String::new(),
            active: false,
        };
    }

    #[test]
    fn a_voz_e_premir_para_falar() {
        let v = NyeVoiceVm {
            state: NyeVoiceState::Idle,
            lang: NyeLang::Pt,
            lang_detected: false,
            transcript: None,
            replay: false,
        };
        let mut a = avail(true);
        a.voice_input = NyeAvail::Available;
        let html = voice(&v, &a).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"data-oc="nye-ptt""#) && html.contains(r#"aria-pressed="false""#));
        assert!(!html.contains("wake"));
    }

    /// D003.1 · Estado inicial com pedido: o servidor desenha o pedido e a lista
    /// completa de aplicações (o filtro é um só, no cliente, o mesmo do `input`);
    /// os ganchos que o `oc-nye.js` usa para filtrar ao abrir, prender o foco e
    /// escalonar o Esc têm de existir; a superfície continua modal.
    #[test]
    fn d003_1_a_superficie_aberta_com_pedido_tem_os_ganchos_de_filtro_e_foco() {
        let mut vm = crate::ui::shell::tests::vm();
        let mut n = surf(false);
        n.query = "nota".into();
        n.hits = vec![];
        vm.nye = Some(n);
        let html = surface(&vm, vm.nye.as_ref().unwrap()).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"aria-modal="true""#) && html.contains(r#"role="dialog""#));
        assert!(html.contains(r#"data-open="""#));
        assert!(html.contains(r#"value="nota""#));
        for hook in [
            r#"data-oc="nye-surface""#,
            r#"data-part="palette-q""#,
            r#"data-part="nye-apps""#,
            r#"data-part="palette-item""#,
            r#"data-part="nye-empty""#,
            r#"data-part="nye-results""#,
        ] {
            assert!(html.contains(hook), "gancho D003.1 {hook}");
        }
    }

    /// D003.1 · Contrato dos alvos de toque em ecrã estreito (≤ 640): 44 px de
    /// área activa nos controlos da Nye, sem mudar o desenho compacto.
    #[test]
    fn d003_1_alvos_de_toque_de_44px_no_ecra_estreito() {
        let css = include_str!("../../../static/oc-nye.css");
        let block = css
            .split("D003.1 · alvos de toque")
            .nth(1)
            .expect("bloco D003.1 de alvos de toque");
        for sel in [
            ".oc-nye-mode",
            ".oc-nyeu__continue",
            ".oc-nye-send",
            ".oc-nye-icon-btn",
            ".oc-nye-btn",
            ".oc-nye-tab",
        ] {
            assert!(block.contains(sel), "alvo de toque {sel}");
        }
        assert!(block.contains("--oc-nye-touch: 44px"));
    }
}
