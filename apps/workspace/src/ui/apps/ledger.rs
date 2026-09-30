//! D007 · Meus Recursos (`/resources`), Actividade (`/activity`), Auditoria
//! (`/audit`). DESIGN_LOCKED.
//!
//! Meus Recursos é o **quadro de consumo do próprio membro** (ADR-0108): quanto
//! pode consumir, quanto consome, e porquê. Não é um índice de ficheiros nem
//! concede acesso. Só o armazenamento é medido hoje, e diz-se.
//!
//! Actividade é para **consciência**: o feed que o Core projecta. Um alvo que
//! deixou de ser visível aparece redigido, nunca com o título antigo.
//!
//! Auditoria é para **responsabilização**: prova só de leitura. Sem editar,
//! apagar ou exportar. A metadata passa por lista branca de chaves; o resto
//! conta-se, não se mostra.

use leptos::prelude::*;

use super::ops::{outcome_tag, storage_state, tag};
use super::res::{class_tag, detail_or, kv, link_row, two_pane};
use super::{empty, error, frame, load_state, more};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    ActivityItemVm, ActivityVm, AuditDetailVm, AuditRowVm, AuditVm, ResourcesVm,
};

// ── Meus Recursos ──

/// A aplicação Meus Recursos.
pub fn resources_app(vm: &ResourcesVm) -> AnyView {
    let toolbar = view! { <h2 class="oc-app__title">{t("nav.resources")}</h2> }.into_any();
    let main = load_state(vm.load, 4).unwrap_or_else(|| view! {
        <article class="oc-res-doc oc-org-wide" data-part="resources">
            <header class="oc-res-head"><div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("resources.storage")}</h2><p class="oc-res-head__tags">{tag(storage_state(vm.state))}</p></div></header>
            <div class="oc-ops-meter" data-part="ops-meter">
                <div class="oc-ops-meter__head"><span class="oc-ops-meter__label">{t("resources.used")}</span><span class="oc-ops-meter__val">{vm.limit.clone().map_or_else(|| vm.used.clone(), |l| tf("ops.used_of", &[("used", vm.used.as_str()), ("total", l.as_str())]))}</span></div>
                {vm.used_pct.map(|p| view! {
                    <span class="oc-ops-meter__bar" role="img" aria-label=tf("ops.pct", &[("n", p.to_string().as_str())])><span class="oc-ops-meter__fill" data-pct=p.to_string()></span>{vm.reserved_pct.map(|r| view! { <span class="oc-ops-meter__fill oc-ops-meter__fill--res" data-pct=r.to_string() data-after=p.to_string()></span> })}</span>
                })}
                <dl class="oc-ops-meter__facts">
                    <div><dt>{t("resources.reserved")}</dt><dd>{vm.reserved.clone()}</dd></div>
                    <div><dt>{t("resources.available")}</dt><dd>{vm.available.clone()}</dd></div>
                    <div><dt>{t("resources.limit")}</dt><dd>{vm.limit.clone().unwrap_or_else(|| t("resources.no_limit").to_owned())}</dd></div>
                </dl>
            </div>
            <section class="oc-res-sec" data-part="resources-entitlement">
                <h3 class="oc-res-sec__title">{tf("resources.why", &[("q", vm.entitlement.as_str())])}</h3>
                {if vm.parts.is_empty() { view! { <p class="oc-res-sec__empty">{t("resources.parts.none")}</p> }.into_any() } else { view! {
                    <ul class="oc-org-list">{vm.parts.iter().map(|p| view! {
                        <li class="oc-org-list__row"><span class="oc-org-grant__main"><span class="oc-ops-id">{p.quantity.clone()}</span><span class="oc-org-grant__meta"><span>{p.source.clone()}</span><span>{p.note.clone()}</span>{p.expires.clone().map(|x| view! { <span>{tf("org.grants.expires", &[("at", x.as_str())])}</span> })}</span></span></li>
                    }).collect_view()}</ul>
                }.into_any() }}
            </section>
            <p class="oc-app-note">{icon("shield")}<span>{t("resources.scope")}</span></p>
            {vm.files_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{icon("files")}<span>{t("resources.open_files")}</span></a> })}
        </article>
    }.into_any());
    frame(
        "resources",
        t("nav.resources").to_owned(),
        toolbar,
        None,
        main,
        None,
    )
}

// ── Actividade ──

fn event(e: &ActivityItemVm) -> impl IntoView {
    view! {
        {e.day.clone().map(|d| view! { <li class="oc-msg-day" role="separator"><span>{d}</span></li> })}
        <li class="oc-ops-event" data-redacted=e.redacted.then_some("")>
            <time class="oc-ops-event__at">{e.at.clone()}</time>
            <div class="oc-ops-event__main">
                <p class="oc-ops-event__line"><span class="oc-ops-event__actor">{e.actor.clone().unwrap_or_else(|| t("activity.system").to_owned())}</span>" "<span>{e.summary.clone()}</span></p>
                {match (&e.target, e.redacted) {
                    (Some(l), _) => view! { <ul class="oc-res-links">{link_row(l)}</ul> }.into_any(),
                    (None, true) => view! { <p class="oc-ops-event__redacted">{icon("lock")}{t("activity.redacted")}</p> }.into_any(),
                    _ => ().into_any(),
                }}
                <p class="oc-ops-event__meta">{e.context.clone().map(|c| view! { <span>{c}</span> })}{(!e.redacted).then(|| class_tag(e.class))}</p>
            </div>
        </li>
    }
}

/// A aplicação Actividade.
pub fn activity_app(vm: &ActivityVm) -> AnyView {
    if let Some(e) = vm.error {
        return frame(
            "activity",
            t("nav.activity").to_owned(),
            view! { <h2 class="oc-app__title">{t("nav.activity")}</h2> }.into_any(),
            None,
            error(e).into_any(),
            None,
        );
    }
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.activity")}</h2><span class="oc-app__spacer"></span>
        <form class="oc-res-filter" method="get" role="search"><label><span class="oc-sr">{t("res.filter.workspace")}</span><select name="workspace">{vm.workspaces.iter().map(|o| view! { <option value=o.value.clone() selected=o.selected>{o.label.clone()}</option> }).collect_view()}</select></label><button type="submit" class="oc-app-btn">{t("res.filter.apply")}</button></form>
    }
    .into_any();
    let main = match load_state(vm.load, 8) {
        Some(s) => s,
        None if vm.items.is_empty() => empty("activity", "activity.empty", Some("activity.empty.body")).into_any(),
        None => view! {
            <div class="oc-res-doc oc-org-wide"><h2 class="oc-sr" id="oc-res-title">{t("nav.activity")}</h2>
                <p class="oc-res-form__hint">{t("activity.not_audit")}</p>
                <ol class="oc-ops-feed" aria-label=t("nav.activity")>{vm.items.iter().map(event).collect_view()}</ol>
                {more(&vm.page)}
            </div>
        }.into_any(),
    };
    frame(
        "activity",
        t("nav.activity").to_owned(),
        toolbar,
        None,
        main,
        None,
    )
}

// ── Auditoria ──

fn audit_row(r: &AuditRowVm) -> impl IntoView {
    view! {
        <tr class="oc-res-row" data-part="res-item" data-open=r.active.then_some("")>
            <td class="oc-res-c-title">
                <a class="oc-res-name" href=r.href.clone() aria-current=r.active.then_some("true") data-part="res-open">
                    <span class="oc-res-name__title"><code class="oc-ops-code">{r.action.clone()}</code>" · "<span>{r.resource_type.clone()}</span></span>
                    <span class="oc-res-name__meta">{outcome_tag(r.outcome)}<time class="oc-res-name__code" datetime=r.at_utc.clone()>{r.at.clone()}</time></span>
                </a>
            </td>
            <td class="oc-res-c" data-prio="1">{r.actor.clone().unwrap_or_else(|| t("activity.system").to_owned())}{r.on_behalf.clone().map(|b| view! { <span class="oc-ops-sub">{tf("audit.on_behalf", &[("name", b.as_str())])}</span> })}</td>
            <td class="oc-res-c" data-prio="2">{r.class.map(class_tag)}</td>
        </tr>
    }
}

fn audit_detail(d: &AuditDetailVm, back: String) -> impl IntoView {
    let r = &d.row;
    view! {
        <article class="oc-res-doc" data-part="audit-record">
            <header class="oc-res-head"><a class="oc-app__icon oc-res-back" href=back aria-label=t("res.back")>{icon("chev-l")}</a><div class="oc-res-head__text"><p class="oc-res-head__code">{r.resource_type.clone()}</p><h2 class="oc-res-head__title" id="oc-res-title"><code class="oc-ops-code">{r.action.clone()}</code></h2><p class="oc-res-head__tags">{outcome_tag(r.outcome)}{r.class.map(class_tag)}</p></div></header>
            <dl class="oc-res-meta">
                <div class="oc-app-kv"><dt>{t("audit.when")}</dt><dd><time datetime=r.at_utc.clone()>{r.at.clone()}</time><span class="oc-ops-sub">{r.at_utc.clone()}</span></dd></div>
                {kv("audit.actor", Some(r.actor.clone().unwrap_or_else(|| t("activity.system").to_owned())))}
                {kv("audit.on_behalf.label", r.on_behalf.clone())}
                {kv("audit.correlation", d.correlation.clone())}
            </dl>
            {r.actor_filter_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{icon("user")}<span>{t("audit.same_actor")}</span></a> })}
            {d.target.as_ref().map(|l| view! { <section class="oc-res-sec"><h3 class="oc-res-sec__title">{t("audit.target")}</h3><ul class="oc-res-links">{link_row(l)}</ul></section> })}
            <section class="oc-res-sec" data-part="audit-metadata">
                <h3 class="oc-res-sec__title">{t("audit.metadata")}</h3>
                {if d.metadata.is_empty() { view! { <p class="oc-res-sec__empty">{t("audit.metadata.none")}</p> }.into_any() } else { view! {
                    <dl class="oc-res-meta">{d.metadata.iter().map(|(k, v)| view! { <div class="oc-app-kv"><dt><code>{k.clone()}</code></dt><dd>{v.clone()}</dd></div> }).collect_view()}</dl>
                }.into_any() }}
                {(d.omitted > 0).then(|| view! { <p class="oc-res-form__hint">{icon("lock")}{tf("audit.omitted", &[("n", d.omitted.to_string().as_str())])}</p> })}
            </section>
            <p class="oc-app-note">{icon("audit")}<span>{t("audit.immutable")}</span></p>
        </article>
    }
}

/// A aplicação Auditoria.
pub fn audit_app(vm: &AuditVm) -> AnyView {
    let title = view! { <h2 class="oc-app__title">{t("nav.audit")}</h2> };
    if let Some(e) = vm.error {
        return frame(
            "audit",
            t("nav.audit").to_owned(),
            title.into_any(),
            None,
            error(e).into_any(),
            None,
        );
    }
    let toolbar = view! {
        {title}<span class="oc-app__spacer"></span>
        <form class="oc-res-filter oc-ops-filters" method="get" action=vm.filter_action.clone() role="search">
            <label><span class="oc-sr">{t("audit.filter.type")}</span><select name="resource_type">{vm.types.iter().map(|o| view! { <option value=o.value.clone() selected=o.selected>{o.label.clone()}</option> }).collect_view()}</select></label>
            <label><span class="oc-sr">{t("audit.filter.since")}</span><input type="date" name="since" value=vm.since.clone() /></label>
            <button type="submit" class="oc-app-btn">{t("res.filter.apply")}</button>
        </form>
    }
    .into_any();
    let l = match load_state(vm.load, 10) {
        Some(s) => s,
        None if vm.rows.is_empty() => empty("audit", "audit.empty", Some("audit.empty.body")).into_any(),
        None => view! {
            {vm.actor.clone().map(|(n, clear)| view! { <p class="oc-app-note">{icon("user")}<span>{tf("audit.filtered_actor", &[("name", n.as_str())])}</span><a class="oc-app-btn" href=clear>{t("audit.clear")}</a></p> })}
            <table class="oc-res-table" aria-label=t("nav.audit") data-oc="res-list">
                <thead><tr><th scope="col" class="oc-res-c-title">{t("audit.col.action")}</th><th scope="col" class="oc-res-c" data-prio="1">{t("audit.actor")}</th><th scope="col" class="oc-res-c" data-prio="2">{t("res.classification")}</th></tr></thead>
                <tbody>{vm.rows.iter().map(audit_row).collect_view()}</tbody>
            </table>
            {more(&vm.page)}
        }.into_any(),
    };
    let d = match &vm.detail {
        Some(x) => audit_detail(x, vm.list_href.clone()).into_any(),
        None => detail_or(None, empty("audit", "audit.none_open", None).into_any()),
    };
    frame(
        "audit",
        t("nav.audit").to_owned(),
        toolbar,
        None,
        two_pane(vm.pane, l, d),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{AppError, AppLoad, AppPageVm, ResClassification};

    #[test]
    fn evento_redigido_nao_mostra_classificacao_nem_alvo() {
        let vm = ActivityVm {
            workspaces: vec![],
            items: vec![ActivityItemVm {
                actor: Some("<i>R</i>".into()),
                summary: "alterou".into(),
                target: None,
                redacted: true,
                context: None,
                class: ResClassification::Restricted,
                at: "10:00".into(),
                day: None,
            }],
            load: AppLoad::Ready,
            page: AppPageVm::default(),
            error: None,
        };
        let html = activity_app(&vm).to_html();
        assert_contracts(&html);
        assert!(
            html.contains(t("activity.redacted"))
                && !html.contains(t("res.class.restricted"))
                && html.contains("&lt;i&gt;")
        );
    }

    #[test]
    fn auditoria_recusada_nao_tem_filtros_nem_linhas() {
        let vm = AuditVm {
            error: Some(AppError::PermissionDenied),
            filter_action: "/audit".into(),
            types: vec![],
            since: String::new(),
            actor: None,
            rows: vec![],
            load: AppLoad::Ready,
            page: AppPageVm::default(),
            pane: crate::ui::view_models::ResPane::List,
            detail: None,
            list_href: "/audit".into(),
        };
        let html = audit_app(&vm).to_html();
        assert!(html.contains(r#"data-error="app.err.denied""#) && !html.contains("oc-res-filter"));
    }
}
