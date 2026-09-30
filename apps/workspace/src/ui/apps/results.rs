//! D007.1 · Resultados (`/results`, `/results/{id}`). DESIGN_LOCKED.
//!
//! Um resultado é a evidência ou a conclusão que o trabalho produziu
//! (`results`, 0019) — não uma publicação, não um ficheiro, não conhecimento.
//! Nasce numa execução de estudo: por isso **não há «Novo resultado»** na
//! barra; regista-se a partir da execução (`/executions/{id}/results/new`).
//!
//! Estados exactos do Core: rascunho, em revisão, validado, substituído,
//! invalidado. A reprodutibilidade é evidência: as validações listam-se com o
//! desfecho real (confirmado, contradito, inconclusivo) e quem as registou.
//! «Registar validação» só quando o Core a oferece (`validate_href`): é uma
//! acção pessoal, não delegável a agentes (ADR-0307).
//!
//! Ambiente, projecto, execução e linhagem são ligações canónicas; cada ponta
//! reautoriza-se. Uma ligação que o membro já não alcança não chega à vista.
//! Sem DOI, factor de impacto, citações ou revista: o domínio não os tem.

use leptos::prelude::*;

use super::res::{class_tag, detail_or, kv, link_row, links_section, list, state_tag, two_pane};
use super::{empty, frame, nav, nye};
use crate::i18n::{t, tf};
use crate::ui::components::icon;
use crate::ui::view_models::{
    ResStateVm, ResTone, ResultStatus, ResultValidationVm, ResultVm, ResultsVm, ValidationKind,
    ValidationOutcome,
};

/// O estado de um resultado (`ck_results_status`).
#[must_use]
pub const fn result_state(s: ResultStatus) -> ResStateVm {
    let (key, tone) = match s {
        ResultStatus::Draft => ("results.state.draft", ResTone::Neutral),
        ResultStatus::UnderReview => ("results.state.under_review", ResTone::Attention),
        ResultStatus::Validated => ("results.state.validated", ResTone::Done),
        ResultStatus::Superseded => ("results.state.superseded", ResTone::Closed),
        ResultStatus::Invalidated => ("results.state.invalidated", ResTone::Closed),
    };
    ResStateVm { key, tone }
}

fn validation(v: &ResultValidationVm) -> impl IntoView {
    let (ok, tone, ic) = match v.outcome {
        ValidationOutcome::Confirmed => ("results.outcome.confirmed", "done", "check"),
        ValidationOutcome::Contradicted => ("results.outcome.contradicted", "closed", "warning"),
        ValidationOutcome::Inconclusive => ("results.outcome.inconclusive", "attention", "minus"),
    };
    let kind = match v.kind {
        ValidationKind::Validation => "results.kind.validation",
        ValidationKind::Reproduction => "results.kind.reproduction",
    };
    view! {
        <li class="oc-rsl-val" data-part="result-validation">
            <p class="oc-rsl-val__head"><span class="oc-res-state oc-ops-outcome" data-tone=tone>{icon(ic)}{t(ok)}</span><span class="oc-rsl-val__kind">{t(kind)}</span><time class="oc-ops-sub">{v.at.clone()}</time></p>
            <p class="oc-rsl-val__by">{v.by.clone().map_or_else(|| t("results.by.unknown").to_owned(), |b| tf("results.by", &[("name", b.as_str())]))}</p>
            {v.note.clone().map(|n| view! { <p class="oc-rsl-val__note">{n}</p> })}
            {v.execution.as_ref().map(|e| view! { <ul class="oc-res-links">{link_row(e)}</ul> })}
        </li>
    }
}

fn detail(r: &ResultVm, back: String) -> impl IntoView {
    view! {
        <article class="oc-res-doc" data-part="result">
            <header class="oc-res-head">
                <a class="oc-app__icon oc-res-back" href=back aria-label=t("res.back")>{icon("chev-l")}</a>
                <div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{r.title.clone()}</h2><p class="oc-res-head__tags">{state_tag(result_state(r.status))}{class_tag(r.class)}</p></div>
            </header>
            <div class="oc-res-actions">
                {r.validate_href.clone().map(|h| view! { <a class="oc-app-primary" href=h>{icon("check")}<span>{t("results.validate")}</span></a> })}
                {r.nye.as_ref().map(nye)}
            </div>
            {r.superseded_by.as_ref().map(|s| view! { <section class="oc-app-note" data-part="result-superseded">{icon("arrow-r")}<span>{t("results.superseded_by")}</span><ul class="oc-res-links">{link_row(s)}</ul></section> })}
            <section class="oc-res-sec"><h3 class="oc-res-sec__title">{t("results.summary")}</h3><div class="oc-res-prose">{r.summary.split("\n\n").map(|p| view! { <p>{p.to_owned()}</p> }).collect_view()}</div></section>
            <dl class="oc-res-meta">
                {kv("results.created_by", r.created_by.clone())}
                {kv("results.created", Some(r.created.clone()))}
                {kv("results.updated", Some(r.updated.clone()))}
            </dl>
            <section class="oc-res-sec"><h3 class="oc-res-sec__title">{t("results.origin")}</h3>
                {if r.workspace.is_none() && r.project.is_none() && r.execution.is_none() {
                    view! { <p class="oc-res-sec__empty">{t("results.origin.none")}</p> }.into_any()
                } else {
                    view! { <ul class="oc-res-links">{r.workspace.as_ref().map(link_row)}{r.project.as_ref().map(link_row)}{r.execution.as_ref().map(link_row)}</ul> }.into_any()
                }}
            </section>
            <section class="oc-res-sec"><h3 class="oc-res-sec__title">{t("results.validations")}</h3>
                {if r.validations.is_empty() {
                    view! { <p class="oc-res-sec__empty">{t("results.validations.none")}</p> }.into_any()
                } else {
                    view! { <ol class="oc-rsl-vals">{r.validations.iter().map(validation).collect_view()}</ol> }.into_any()
                }}
            </section>
            {links_section("results.lineage", &r.lineage, Some("results.lineage.none"), None)}
        </article>
    }
}

/// A aplicação Resultados.
pub fn app(vm: &ResultsVm) -> AnyView {
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.results")}</h2><span class="oc-app__spacer"></span>
        <form class="oc-res-filter" method="get" action=vm.filter_action.clone() role="search"><label><span class="oc-sr">{t("res.filter.workspace")}</span><select name="workspace">{vm.workspaces.iter().map(|o| view! { <option value=o.value.clone() selected=o.selected>{o.label.clone()}</option> }).collect_view()}</select></label><button type="submit" class="oc-app-btn">{t("res.filter.apply")}</button></form>
    }
    .into_any();
    let l = view! {
        {list(&vm.list, "results.list", empty("results", "results.empty", Some("results.empty.body")).into_any())}
        <p class="oc-res-form__hint oc-rsl-origin-hint">{t("results.new.where")}</p>
    }
    .into_any();
    let d = match &vm.result {
        Some(r) => detail(r, vm.list_href.clone()).into_any(),
        None => detail_or(
            vm.result_error,
            empty("results", "results.none_open", None).into_any(),
        ),
    };
    frame(
        "results",
        t("nav.results").to_owned(),
        toolbar,
        Some(nav(&vm.nav).into_any()),
        two_pane(vm.pane, l, d),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{
        AppError, AppLoad, AppPageVm, ResClassification, ResListVm, ResPane,
    };

    fn vm(result: Option<ResultVm>) -> ResultsVm {
        ResultsVm {
            nav: vec![],
            workspaces: vec![],
            filter_action: "/results".into(),
            list: ResListVm {
                columns: vec![],
                items: vec![],
                load: AppLoad::Ready,
                page: AppPageVm::default(),
            },
            pane: ResPane::Detail,
            list_href: "/results".into(),
            result,
            result_error: None,
        }
    }

    fn r() -> ResultVm {
        ResultVm {
            title: "<img src=x onerror=1>".into(),
            status: ResultStatus::UnderReview,
            class: ResClassification::Internal,
            summary: "a\n\nb".into(),
            workspace: None,
            project: None,
            execution: None,
            superseded_by: None,
            created_by: None,
            created: "1".into(),
            updated: "2".into(),
            validations: vec![],
            lineage: vec![],
            validate_href: None,
            nye: None,
        }
    }

    #[test]
    fn nao_ha_novo_resultado_na_barra() {
        let html = app(&vm(Some(r()))).to_html();
        assert_contracts(&html);
        assert!(!html.contains("/results/new") && html.contains(t("results.new.where")));
        assert!(html.contains("&lt;img") && !html.contains("<img src=x"));
    }

    #[test]
    fn validar_so_quando_o_core_oferece() {
        let html = app(&vm(Some(r()))).to_html();
        assert!(!html.contains(t("results.validate")));
        let mut x = r();
        x.validate_href = Some("/results/r1/validate".into());
        assert!(app(&vm(Some(x))).to_html().contains("/results/r1/validate"));
    }

    #[test]
    fn recusa_nao_mostra_o_resultado() {
        let mut v = vm(None);
        v.result_error = Some(AppError::NotFound);
        let html = app(&v).to_html();
        assert!(html.contains(r#"data-error="app.err.not_found""#));
    }
}
