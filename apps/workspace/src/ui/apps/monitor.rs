//! D007.1 · Monitor de Actividade (`/admin/monitor`). DESIGN_LOCKED.
//!
//! O Monitor mostra **consumo operacional**; a Computação mostra **capacidade**.
//! Um nó aparece aqui como fonte de uma métrica, pelo identificador, com a
//! ligação para a sua ficha canónica na Computação — nunca como catálogo.
//!
//! Só planos que o Core/runtime diz suportar (`MonitorVm.planes`). Hoje o agente
//! do nó reporta memória e armazenamento em uso; CPU, rede e GPU não têm
//! utilização reportada e aparecem **nomeados como não reportados**, nunca como
//! zero. Sem série temporal no contrato: medidores do valor actual, com a hora
//! de cada leitura e o estado «sem sinal» quando é antiga. Nenhum gráfico
//! inventado; nenhum valor aleatório de recurso.
//!
//! Serviços e «Parar» só existem se o runtime publicar um inventário tipado
//! (`MonitorVm.services = Some`). Até lá a secção diz o que falta. Quando
//! existir, «Parar» abre a confirmação partilhada D006 (`OrgActionKind::StopService`)
//! e o Core decide, com a invariante de serviço protegido do lado do Core.
//! Nada de shell, sinal arbitrário, comando, Docker ou SSH (isso é D008).

use leptos::prelude::*;

use super::ops::{node_state, tag};
use super::org::action;
use super::{error, frame, load_state};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    Freshness, MonitorReceiptVm, MonitorSampleVm, MonitorServiceVm, MonitorStopRefusal,
    MonitorSummaryVm, MonitorVm,
};

fn summary(s: &MonitorSummaryVm) -> impl IntoView {
    let fact = |k: &'static str, v: String, warn: bool| {
        view! {
            <div class="oc-mon-fact" data-warn=warn.then_some("")><dt>{t(k)}</dt><dd>{v}</dd></div>
        }
    };
    view! {
        <dl class="oc-mon-summary" data-part="mon-summary">
            {fact("mon.sum.nodes", tf("mon.n_of", &[("n", s.nodes_online.to_string().as_str()), ("total", s.nodes_total.to_string().as_str())]), s.nodes_online < s.nodes_total)}
            {fact("mon.sum.providers", tf("mon.n_of", &[("n", s.providers_healthy.to_string().as_str()), ("total", s.providers_total.to_string().as_str())]), s.providers_healthy < s.providers_total)}
            {fact("mon.sum.apps", tf("mon.n_of", &[("n", s.apps_active.to_string().as_str()), ("total", s.apps_total.to_string().as_str())]), false)}
            {fact("mon.sum.storage", tf("mon.sum.storage.v", &[("warn", (s.storage_warning + s.storage_critical).to_string().as_str()), ("over", s.storage_over.to_string().as_str())]), s.storage_over > 0 || s.storage_critical > 0)}
            {fact("mon.sum.personal", s.personal_used.clone(), false)}
        </dl>
    }
}

fn sample(x: &MonitorSampleVm) -> impl IntoView {
    let fresh = match x.freshness {
        Freshness::Live => "live",
        Freshness::Stale => "stale",
        Freshness::Unreported => "unreported",
    };
    view! {
        <tr data-fresh=fresh>
            <th scope="row"><span class="oc-ops-code">{x.source.clone()}</span><span class="oc-ops-sub">{x.name.clone()}</span></th>
            <td>{tag(node_state(x.status))}</td>
            <td class="oc-mon-use">{match (&x.used, x.pct) {
                (Some(u), p) => view! {
                    <span class="oc-mon-use__v">{tf("ops.used_of", &[("used", u.as_str()), ("total", x.total.clone().unwrap_or_else(|| "—".into()).as_str())])}</span>
                    {p.map(|p| view! { <span class="oc-ops-meter__bar" role="img" aria-label=tf("ops.pct", &[("n", p.to_string().as_str())])><span class="oc-ops-meter__fill" data-pct=p.to_string()></span></span> })}
                }.into_any(),
                (None, _) => view! { <span class="oc-ops-reason">{icon("minus")}{t("ops.not_reported")}</span> }.into_any(),
            }}</td>
            <td class="oc-ops-c2">{x.seen.clone().map_or_else(|| view! { <span>{"—"}</span> }.into_any(), |s| view! {
                <span class="oc-mon-seen">{if x.freshness == Freshness::Stale { icon("clock").into_any() } else { ().into_any() }}{s}</span>
                {(x.freshness == Freshness::Stale).then(|| view! { <span class="oc-ops-sub">{t("mon.stale")}</span> })}
            }.into_any())}</td>
            <td class="oc-ops-c3">{x.compute_href.clone().map(|h| view! { <a class="oc-app-btn" href=h aria-label=tf("mon.open_node", &[("node", x.source.as_str())])>{icon("compute")}<span>{t("nav.compute")}</span></a> })}</td>
        </tr>
    }
}

fn planes(vm: &MonitorVm) -> AnyView {
    let Some(active) = vm.plane else {
        return view! { <p class="oc-app-note">{icon("minus")}<span>{t("mon.planes.none")}</span></p> }.into_any();
    };
    view! {
        <section class="oc-res-sec" aria-labelledby="oc-mon-planes">
            <h3 class="oc-res-sec__title" id="oc-mon-planes">{t("mon.consumption")}</h3>
            <nav class="oc-app-seg oc-mon-planes" aria-label=t("mon.planes")>
                {vm.planes.iter().map(|(p, h)| view! { <a class="oc-app-seg__opt" href=h.clone() aria-current=(*p == active).then_some("page") data-plane=p.id()>{t(p.key())}</a> }).collect_view()}
            </nav>
            {(!vm.unsupported.is_empty()).then(|| view! {
                <p class="oc-res-sec__empty" data-part="mon-unsupported">{icon("minus")}{tf("mon.planes.unreported", &[("list", vm.unsupported.iter().map(|p| t(p.key())).collect::<Vec<_>>().join(", ").as_str())])}</p>
            })}
            {if vm.samples.is_empty() {
                view! { <p class="oc-res-sec__empty">{t("mon.samples.empty")}</p> }.into_any()
            } else {
                view! {
                    <table class="oc-ops-table oc-mon-table" aria-label=tf("mon.table", &[("plane", t(active.key()))])>
                        <thead><tr><th scope="col">{t("mon.col.source")}</th><th scope="col">{t("mon.col.state")}</th><th scope="col">{t("mon.col.use")}</th><th scope="col" class="oc-ops-c2">{t("mon.col.seen")}</th><th scope="col" class="oc-ops-c3"><span class="oc-sr">{t("mon.col.canonical")}</span></th></tr></thead>
                        <tbody>{vm.samples.iter().map(sample).collect_view()}</tbody>
                    </table>
                }.into_any()
            }}
            <p class="oc-res-form__hint">{t("mon.no_series")}</p>
        </section>
    }
    .into_any()
}

fn service(s: &MonitorServiceVm) -> impl IntoView {
    view! {
        <li class="oc-org-list__row oc-mon-svc" data-protected=s.protected.then_some("")>
            <span class="oc-org-grant__main">
                <span class="oc-ops-id">{s.label.clone()}</span>
                <span class="oc-org-grant__meta">
                    {s.source.clone().map(|x| view! { <span class="oc-ops-code">{x}</span> })}
                    {s.value.clone().map(|v| view! { <span>{v}</span> })}
                    {s.protected.then(|| view! { <span class="oc-mon-protected">{icon("shield")}{t(s.protection_key.unwrap_or("mon.svc.protected"))}</span> })}
                </span>
            </span>
            {tag(s.state)}
            {s.stop.as_ref().map(action)}
        </li>
    }
}

fn services(vm: &MonitorVm) -> impl IntoView {
    view! {
        <section class="oc-res-sec" aria-labelledby="oc-mon-svcs">
            <h3 class="oc-res-sec__title" id="oc-mon-svcs">{t("mon.services")}</h3>
            {match &vm.services {
                None => view! { <p class="oc-app-note" data-part="mon-no-inventory">{icon("minus")}<span>{t("mon.services.none")}</span></p> }.into_any(),
                Some(v) if v.is_empty() => view! { <p class="oc-res-sec__empty">{t("mon.services.empty")}</p> }.into_any(),
                Some(v) => view! { <ul class="oc-org-list" aria-label=t("mon.services")>{v.iter().map(service).collect_view()}</ul><p class="oc-res-form__hint">{t("mon.services.hint")}</p> }.into_any(),
            }}
        </section>
    }
}

fn receipt(r: &MonitorReceiptVm) -> impl IntoView {
    view! {
        <section class="oc-app-note oc-mon-receipt" role="status" data-part="mon-receipt">
            {icon("check")}
            <span><strong>{tf(r.state_key, &[("name", r.target.as_str())])}</strong></span>
            <dl class="oc-ops-plane__facts">
                <div><dt>{t("mon.receipt.id")}</dt><dd><code>{r.action_id.clone()}</code></dd></div>
                <div><dt>{t("mon.receipt.at")}</dt><dd>{r.at.clone()}</dd></div>
            </dl>
            {r.audit_href.clone().map(|h| view! { <a href=h>{t("mon.receipt.audit")}</a> })}
        </section>
    }
}

fn refusal(r: MonitorStopRefusal) -> impl IntoView {
    let k = match r {
        MonitorStopRefusal::Protected => "mon.refusal.protected",
        MonitorStopRefusal::Gone => "mon.refusal.gone",
        MonitorStopRefusal::Changed => "mon.refusal.changed",
        MonitorStopRefusal::Denied => "mon.refusal.denied",
    };
    view! { <p class="oc-app-note oc-org-refusal" data-tone="warn" data-refusal=k role="alert">{icon("shield")}<span><strong>{t(&format!("{k}.title"))}</strong>{" "}{t(&format!("{k}.body"))}</span></p> }
}

/// A aplicação Monitor de Actividade.
pub fn app(vm: &MonitorVm) -> AnyView {
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.monitor")}</h2><span class="oc-app__spacer"></span>
        {vm.read_at.clone().map(|at| view! { <span class="oc-mon-read" data-part="mon-read-at">{tf("mon.read_at", &[("at", at.as_str())])}</span> })}
        <a class="oc-app-btn" href=vm.refresh_href.clone()>{icon("refresh")}<span>{t("mon.refresh")}</span></a>
    }
    .into_any();
    if let Some(e) = vm.error {
        return frame(
            "monitor",
            t("nav.monitor").to_owned(),
            view! { <h2 class="oc-app__title">{t("nav.monitor")}</h2> }.into_any(),
            None,
            error(e).into_any(),
            None,
        );
    }
    let main = load_state(vm.load, 6).unwrap_or_else(|| view! {
        <article class="oc-res-doc oc-org-wide" data-part="monitor">
            <header class="oc-res-head"><div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("mon.title")}</h2><p class="oc-res-form__hint">{t("mon.lede")}</p></div></header>
            {vm.receipt.as_ref().map(receipt)}
            {vm.refusal.map(refusal)}
            {vm.summary.as_ref().map(summary)}
            <p class="oc-mon-links">
                {vm.ai_href.clone().map(|h| view! { <a href=h>{t("mon.link.ai")}</a> })}
                {vm.apps_href.clone().map(|h| view! { <a href=h>{t("mon.link.apps")}</a> })}
            </p>
            {planes(vm)}
            {services(vm)}
            <p class="oc-app-note">{icon("shield")}<span>{t("mon.boundary")}</span></p>
        </article>
    }.into_any());
    frame(
        "monitor",
        t("nav.monitor").to_owned(),
        toolbar,
        None,
        main,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{AppError, AppLoad, MetricPlane, NodeStatus};

    fn vm() -> MonitorVm {
        MonitorVm {
            load: AppLoad::Ready,
            error: None,
            read_at: Some("10:42:05".into()),
            refresh_href: "/admin/monitor?plane=memory".into(),
            summary: None,
            planes: vec![
                (MetricPlane::Memory, "?plane=memory".into()),
                (MetricPlane::Storage, "?plane=storage".into()),
            ],
            unsupported: vec![MetricPlane::Cpu, MetricPlane::Network, MetricPlane::Gpu],
            plane: Some(MetricPlane::Memory),
            samples: vec![MonitorSampleVm {
                source: "<b>n1</b>".into(),
                name: "N".into(),
                status: NodeStatus::Offline,
                used: None,
                total: None,
                pct: None,
                seen: Some("09:10".into()),
                freshness: Freshness::Stale,
                compute_href: Some("/compute/n1".into()),
            }],
            services: None,
            receipt: None,
            refusal: None,
            ai_href: None,
            apps_href: None,
        }
    }

    #[test]
    fn plano_sem_uso_diz_nao_reportado_e_nunca_zero() {
        let html = app(&vm()).to_html();
        assert_contracts(&html);
        assert!(
            html.contains(t("ops.not_reported"))
                && !html.contains(">0 %")
                && html.contains("&lt;b&gt;n1")
        );
        assert!(html.contains(r#"data-part="mon-unsupported""#) && html.contains(t("mon.stale")));
    }

    #[test]
    fn sem_inventario_nao_ha_botao_parar() {
        let html = app(&vm()).to_html();
        assert!(
            html.contains(r#"data-part="mon-no-inventory""#)
                && !html.contains("org.act.stop_service")
        );
        assert!(!html.contains("<textarea") && !html.contains("<input"));
    }

    #[test]
    fn sem_administracao_da_plataforma_nada_se_mostra() {
        let mut v = vm();
        v.error = Some(AppError::PermissionDenied);
        let html = app(&v).to_html();
        assert!(html.contains(r#"data-error="app.err.denied""#) && !html.contains("n1"));
    }
}
