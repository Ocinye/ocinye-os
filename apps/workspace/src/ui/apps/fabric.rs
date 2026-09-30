//! D007 · IA (`/ai`), Agentes (`/ai/agents`), Computação (`/compute`). DESIGN_LOCKED.
//!
//! IA mostra o Ocinye AI Fabric — capacidades, modelos, fornecedores, política —
//! e **não é uma segunda conversa**: perguntar é na Nye. Um modelo «preferido»
//! é encaminhamento, não autoridade. Nenhum segredo: um fornecedor diz só que
//! tem credencial.
//!
//! Agentes são actores governados que pedem **uma capacidade**; o Core decide
//! em cada execução. Sem conversa por agente, sem construtor visual.
//!
//! Computação mostra nós e capacidade reportada. Não há despacho de trabalhos
//! (o Core diz `allocated = 0` por construção): a secção Trabalhos não existe.
//! Nada de shell, comando, Docker ou SSH.

use leptos::prelude::*;

use super::ops::{agent_state, cap_key, meter, node_state, plane, reason_key, tag};
use super::org::action;
use super::res::{
    class_tag, detail_or, input, kv, link_row, list, prose, select, textarea, two_pane,
};
use super::{empty, error, frame, load_state, nav, nye, primary};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    AgentFormVm, AgentScopeVm, AgentVm, AgentsVm, AiSection, AiVm, ComputeVm, NodeVm,
};

// ── IA ──

fn ai_overview(vm: &AiVm) -> impl IntoView {
    view! {
        {plane(vm.available, if vm.available { "ai.plane.on" } else { "ai.plane.off" }, vec![(t("ai.providers_healthy").to_owned(), vm.healthy_providers.to_string())], vm.message.clone())}
        <table class="oc-ops-table" aria-label=t("ai.caps")>
            <thead><tr><th scope="col">{t("ai.col.cap")}</th><th scope="col">{t("ai.col.state")}</th><th scope="col" class="oc-ops-c2">{t("ai.col.model")}</th><th scope="col" class="oc-ops-c3">{t("ai.col.route")}</th></tr></thead>
            <tbody>
                {vm.caps.iter().map(|c| view! {
                    <tr>
                        <th scope="row">{t(cap_key(c.cap))}</th>
                        <td>{if c.available { view! { <span class="oc-res-state" data-tone="done">{icon("check")}{t("ai.served")}</span> }.into_any() } else { view! { <span class="oc-ops-reason">{icon("minus")}{t(c.reason.map_or("ai.reason.unknown", reason_key))}</span> }.into_any() }}</td>
                        <td class="oc-ops-c2">{c.model.clone().unwrap_or_else(|| "—".to_owned())}</td>
                        <td class="oc-ops-c3">{c.preferred.clone().unwrap_or_else(|| t("ai.route.any").to_owned())}{c.fallback.map(|f| view! { <span class="oc-ops-sub">{t(if f { "ai.route.fallback" } else { "ai.route.no_fallback" })}</span> })}</td>
                    </tr>
                }).collect_view()}
            </tbody>
        </table>
        <p class="oc-app-note">{icon("nye")}<span>{t("ai.not_chat")}</span>{vm.nye_href.clone().map(|h| view! { <a class="oc-app-nye" href=h data-oc="app-nye">{icon("nye")}<span>{t("ai.open_nye")}</span></a> })}</p>
    }
}

fn ai_models(vm: &AiVm) -> AnyView {
    if vm.models.is_empty() {
        return empty("ai", "ai.models.empty", Some("ai.models.empty.body")).into_any();
    }
    view! {
        <table class="oc-ops-table" aria-label=t("ai.models")>
            <thead><tr><th scope="col">{t("ai.col.model")}</th><th scope="col">{t("ai.col.state")}</th><th scope="col" class="oc-ops-c2">{t("ai.col.provider")}</th><th scope="col" class="oc-ops-c3">{t("ai.col.caps")}</th><th scope="col" class="oc-ops-c4">{t("ai.col.max_class")}</th></tr></thead>
            <tbody>
                {vm.models.iter().map(|m| view! {
                    <tr data-disabled=(!m.enabled).then_some("")>
                        <th scope="row"><span class="oc-ops-id">{m.name.clone()}</span><span class="oc-ops-sub">{m.version.clone()}</span></th>
                        <td><span class="oc-res-state" data-tone=match m.tone { crate::ui::view_models::ResTone::Done => "done", crate::ui::view_models::ResTone::Attention => "attention", crate::ui::view_models::ResTone::Closed => "closed", crate::ui::view_models::ResTone::Progress => "progress", crate::ui::view_models::ResTone::Neutral => "neutral" }>{m.status.clone()}</span>{(!m.enabled).then(|| view! { <span class="oc-ops-sub">{t("ai.model.off")}</span> })}</td>
                        <td class="oc-ops-c2">{m.provider.clone()}<span class="oc-ops-sub">{t(if m.local { "ai.local" } else { "ai.external" })}</span></td>
                        <td class="oc-ops-c3">{m.caps.iter().map(|c| t(cap_key(*c))).collect::<Vec<_>>().join(" · ")}</td>
                        <td class="oc-ops-c4">{class_tag(m.max_class)}</td>
                    </tr>
                }).collect_view()}
            </tbody>
        </table>
        <p class="oc-res-form__hint">{t("ai.models.hint")}</p>
    }
    .into_any()
}

fn ai_providers(vm: &AiVm) -> AnyView {
    let Some(ps) = &vm.providers else {
        return error(crate::ui::view_models::AppError::PermissionDenied).into_any();
    };
    view! {
        <dl class="oc-res-meta">
            {kv("ai.policy.external", vm.external_max.map(|m| m.map_or_else(|| t("ai.policy.no_external").to_owned(), |c| tf("ai.policy.up_to", &[("class", t(c.key()))]))))}
            {kv("ai.policy.installation", vm.installation_external.map(|b| t(if b { "org.yes" } else { "org.no" }).to_owned()))}
        </dl>
        {if ps.is_empty() {
            empty("ai", "ai.providers.empty", Some("ai.providers.empty.body")).into_any()
        } else {
            view! {
                <ul class="oc-org-list" aria-label=t("ai.providers")>
                    {ps.iter().map(|p| view! {
                        <li class="oc-org-list__row oc-ops-prov">
                            <span class="oc-org-grant__main">
                                <span class="oc-ops-id">{p.label.clone()}</span>
                                <span class="oc-org-grant__meta">
                                    <span>{p.kind.clone()}</span>
                                    <span>{t(if p.local { "ai.local" } else { "ai.external" })}</span>
                                    <span>{t(if p.has_credential { "ai.cred.yes" } else { "ai.cred.no" })}</span>
                                    {p.checked.clone().map(|c| view! { <span>{tf("ai.checked", &[("at", c.as_str())])}</span> })}
                                </span>
                            </span>
                            <span class="oc-res-state" data-tone=if !p.enabled { "closed" } else if p.healthy == Some(true) { "done" } else { "attention" }>
                                {t(if !p.enabled { "ai.prov.disabled" } else { match p.healthy { Some(true) => "ai.prov.healthy", Some(false) => "ai.prov.unhealthy", None => "ai.prov.unchecked" } })}
                            </span>
                            {p.toggle.as_ref().map(action)}
                        </li>
                    }).collect_view()}
                </ul>
            }.into_any()
        }}
        <p class="oc-res-form__hint">{t("ai.providers.hint")}</p>
    }
    .into_any()
}

/// A aplicação IA.
pub fn ai_app(vm: &AiVm) -> AnyView {
    let toolbar = view! { <h2 class="oc-app__title">{t("nav.ai")}</h2> }.into_any();
    let (title, body) = match vm.section {
        AiSection::Overview => ("ai.overview", ai_overview(vm).into_any()),
        AiSection::Models => ("ai.models", ai_models(vm)),
        AiSection::Providers => ("ai.providers", ai_providers(vm)),
    };
    let main = load_state(vm.load, 6).unwrap_or_else(|| view! {
        <article class="oc-res-doc oc-org-wide" data-part="ai"><header class="oc-res-head"><div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t(title)}</h2></div></header>{body}</article>
    }.into_any());
    frame(
        "ai",
        t("nav.ai").to_owned(),
        toolbar,
        Some(nav(&vm.nav).into_any()),
        main,
        None,
    )
}

// ── Agentes ──

const fn scope_key(s: AgentScopeVm) -> &'static str {
    match s {
        AgentScopeVm::Personal => "agents.scope.personal",
        AgentScopeVm::Workspace => "agents.scope.workspace",
        AgentScopeVm::Unit => "agents.scope.unit",
        AgentScopeVm::Institutional => "agents.scope.institutional",
    }
}

fn agent_detail(a: &AgentVm, back: String) -> impl IntoView {
    let (b, d, ds) = a.sources;
    let sources: Vec<&str> = [
        (b, "agents.src.bibliography"),
        (d, "agents.src.documents"),
        (ds, "agents.src.datasets"),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, k)| t(k))
    .collect();
    view! {
        <article class="oc-res-doc" data-part="agent">
            <header class="oc-res-head"><a class="oc-app__icon oc-res-back" href=back aria-label=t("res.back")>{icon("chev-l")}</a><div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{a.name.clone()}</h2><p class="oc-res-head__tags">{tag(agent_state(a.status))}{class_tag(a.max_class)}</p></div></header>
            <div class="oc-res-actions">{a.nye.as_ref().map(nye)}</div>
            {prose("agents.purpose", a.purpose.as_ref())}
            <dl class="oc-res-meta">
                {kv("agents.cap", Some(t(cap_key(a.cap)).to_owned()))}
                {kv("agents.scope", Some(t(scope_key(a.scope)).to_owned()))}
                {kv("agents.sources", Some(if sources.is_empty() { t("agents.src.none").to_owned() } else { sources.join(" · ") }))}
                {kv("agents.created", Some(a.created.clone()))}
            </dl>
            {a.scope_target.as_ref().map(|l| view! { <ul class="oc-res-links">{link_row(l)}</ul> })}
            <p class="oc-app-note">{icon("shield")}<span>{t("agents.boundary")}</span></p>
            {a.instructions.clone().map(|i| view! {
                <section class="oc-res-sec oc-res-untrusted"><h3 class="oc-res-sec__title">{t("agents.instructions")}</h3><p class="oc-res-untrusted__label">{icon("user")}{t("agents.instructions.owner")}</p><div class="oc-res-prose"><p>{i}</p></div></section>
            })}
        </article>
    }
}

fn agent_form(f: &AgentFormVm, back: String) -> impl IntoView {
    view! {
        <form class="oc-res-form" id="oc-agents-doc-new" method="post" action=f.action.clone() data-oc="app-doc" data-state="clean" aria-labelledby="oc-res-title">
            <header class="oc-res-head"><a class="oc-app__icon oc-res-back" href=back.clone() aria-label=t("res.back")>{icon("chev-l")}</a><div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("agents.new")}</h2></div></header>
            {f.error.map(error)}
            {input("name", "agents.name", "", "text", true)}
            {textarea("purpose", "agents.purpose", "", 2)}
            <div class="oc-app-row2">{select("scope", "agents.scope", &f.scopes, true)}{select("scope_id", "agents.scope.target", &f.targets, false)}</div>
            <div class="oc-app-row2">{select("capability", "agents.cap", &f.caps, true)}{select("max_classification", "agents.max_class", &f.classes, true)}</div>
            <fieldset class="oc-ops-checks"><legend>{t("agents.sources")}</legend>
                <label class="oc-app-check"><input type="checkbox" name="uses_bibliography" value="true" />{t("agents.src.bibliography")}</label>
                <label class="oc-app-check"><input type="checkbox" name="uses_documents" value="true" />{t("agents.src.documents")}</label>
                <label class="oc-app-check"><input type="checkbox" name="uses_datasets" value="true" />{t("agents.src.datasets")}</label>
            </fieldset>
            {textarea("instructions", "agents.instructions", "", 4)}
            <p class="oc-res-form__hint">{t("agents.new.hint")}</p>
            <div class="oc-res-form__foot"><a class="oc-app-btn" href=back>{t("app.cancel")}</a><span class="oc-app__spacer"></span><button type="submit" class="oc-app-primary" data-oc="app-save">{icon("check")}<span>{t("agents.create")}</span></button></div>
        </form>
    }
}

/// A aplicação Agentes.
pub fn agents_app(vm: &AgentsVm) -> AnyView {
    let toolbar = view! { <h2 class="oc-app__title">{t("nav.agents")}</h2><span class="oc-app__spacer"></span>{vm.new_href.clone().map(|h| primary(h, "plus", "agents.new"))} }.into_any();
    let head = (!vm.execution_available).then(|| view! { <p class="oc-app-note oc-ops-banner" data-tone="warn">{icon("minus")}<span>{t("agents.no_execution")}</span></p> });
    let l = view! { {head}{list(&vm.list, "agents.list", empty("agent", "agents.empty", Some("agents.empty.body")).into_any())} }.into_any();
    let d = match (&vm.form, &vm.agent) {
        (Some(f), _) => agent_form(f, vm.list_href.clone()).into_any(),
        (None, Some(a)) => agent_detail(a, vm.list_href.clone()).into_any(),
        _ => detail_or(
            vm.agent_error,
            empty("agent", "agents.none_open", None).into_any(),
        ),
    };
    frame(
        "agents",
        t("nav.agents").to_owned(),
        toolbar,
        None,
        two_pane(vm.pane, l, d),
        None,
    )
}

// ── Computação ──

fn node_detail(n: &NodeVm, back: String) -> impl IntoView {
    view! {
        <article class="oc-res-doc" data-part="node">
            <header class="oc-res-head"><a class="oc-app__icon oc-res-back" href=back aria-label=t("res.back")>{icon("chev-l")}</a><div class="oc-res-head__text"><p class="oc-res-head__code">{n.identifier.clone()}</p><h2 class="oc-res-head__title" id="oc-res-title">{n.name.clone()}</h2><p class="oc-res-head__tags">{tag(node_state(n.status))}</p></div></header>
            <dl class="oc-res-meta">
                {kv("compute.kind", Some(n.kind.clone()))}
                {kv("compute.location", n.location.clone())}
                {kv("compute.control", Some(n.control.clone()))}
                {kv("compute.residency", Some(n.residency.clone()))}
                {kv("compute.gpus", n.gpus.clone())}
                {kv("compute.agent_version", n.agent_version.clone())}
                {kv("compute.last_seen", n.last_seen.clone())}
            </dl>
            <section class="oc-res-sec"><h3 class="oc-res-sec__title">{t("compute.capacity")}</h3><div class="oc-ops-meters">{n.lines.iter().map(meter).collect_view()}</div></section>
            <p class="oc-app-note">{icon("minus")}<span>{t("compute.no_dispatch")}</span></p>
        </article>
    }
}

/// A aplicação Computação.
pub fn compute_app(vm: &ComputeVm) -> AnyView {
    let toolbar = view! { <h2 class="oc-app__title">{t("nav.compute")}</h2> }.into_any();
    let l = view! {
        {plane(vm.online > 0, if vm.online > 0 { "compute.plane.on" } else { "compute.plane.off" }, vec![(t("compute.registered").to_owned(), vm.registered.to_string()), (t("compute.online").to_owned(), vm.online.to_string())], vm.message.clone())}
        {list(&vm.list, "compute.list", empty("compute", "compute.empty", Some("compute.empty.body")).into_any())}
    }
    .into_any();
    let d = match &vm.node {
        Some(n) => node_detail(n, vm.list_href.clone()).into_any(),
        None => detail_or(
            vm.node_error,
            empty("compute", "compute.none_open", None).into_any(),
        ),
    };
    frame(
        "compute",
        t("nav.compute").to_owned(),
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
    use crate::ui::view_models::{AiCap, AiCapVm, AiProviderVm, AiReason, AppLoad};

    #[test]
    fn ia_sem_inferencia_e_honesta_e_nao_e_uma_conversa() {
        let vm = AiVm {
            section: AiSection::Overview,
            nav: vec![],
            available: false,
            healthy_providers: 0,
            message: Some("Nenhum nó".into()),
            caps: vec![AiCapVm {
                cap: AiCap::General,
                available: false,
                model: None,
                reason: Some(AiReason::NoProvider),
                preferred: None,
                fallback: None,
            }],
            models: vec![],
            providers: None,
            external_max: None,
            installation_external: None,
            load: AppLoad::Ready,
            nye_href: None,
        };
        let html = ai_app(&vm).to_html();
        assert_contracts(&html);
        assert!(html.contains(t("ai.reason.no_provider")) && !html.contains("<textarea"));
    }

    #[test]
    fn fornecedor_diz_que_tem_credencial_e_nunca_o_valor() {
        let p = AiProviderVm {
            label: "<b>L</b>".into(),
            kind: "openai_compatible".into(),
            local: false,
            enabled: true,
            healthy: Some(true),
            checked: None,
            has_credential: true,
            toggle: None,
        };
        let vm = AiVm {
            section: AiSection::Providers,
            nav: vec![],
            available: true,
            healthy_providers: 1,
            message: None,
            caps: vec![],
            models: vec![],
            providers: Some(vec![p]),
            external_max: Some(None),
            installation_external: Some(false),
            load: AppLoad::Ready,
            nye_href: None,
        };
        let html = ai_app(&vm).to_html();
        assert!(html.contains(t("ai.cred.yes")) && html.contains("&lt;b&gt;L"));
    }
}
