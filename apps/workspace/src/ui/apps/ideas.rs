//! D005 · Ideias. DESIGN_LOCKED.
//!
//! Uma ideia é uma entidade com ciclo de vida (descoberta → exploração →
//! conceito → revisão → candidata a projecto → promovida), não uma nota. Os
//! campos são os do domínio: resumo, pergunta de investigação, hipótese,
//! motivação, palavras-chave. Fechar (rejeitar, arquivar) exige motivo —
//! memória institucional. Promover cria o projecto no mesmo ambiente e a
//! ideia fica, com a ligação.

use leptos::prelude::*;

use super::res::{
    detail_head, detail_or, idea_state, input, keywords, kv, links_section, list, people_section,
    prose, select, textarea, transitions, two_pane,
};
use super::{doc_form_id, empty, error, frame, nav, nye};
use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::view_models::{IdeaFormVm, IdeaStage, IdeaVm, IdeasVm};

/// Os estádios de desenvolvimento, pela ordem do domínio.
const STAGES: [IdeaStage; 6] = [
    IdeaStage::Discovery,
    IdeaStage::Exploration,
    IdeaStage::Concept,
    IdeaStage::Review,
    IdeaStage::ProjectCandidate,
    IdeaStage::Promoted,
];

fn lifecycle(s: IdeaStage) -> impl IntoView {
    let closed = matches!(s, IdeaStage::Rejected | IdeaStage::Archived);
    let at = STAGES.iter().position(|x| *x == s);
    view! {
        <ol class="oc-res-life" aria-label=t("ideas.lifecycle") data-closed=closed.then_some("")>
            {STAGES.iter().enumerate().map(|(i, st)| {
                let state = match at {
                    Some(a) if i < a => "past",
                    Some(a) if i == a => "current",
                    _ => "next",
                };
                view! {
                    <li class="oc-res-life__step" data-step=state aria-current=(state == "current").then_some("step")>
                        <span class="oc-res-life__dot" aria-hidden="true"></span>
                        <span class="oc-res-life__label">{t(idea_state(*st).key)}</span>
                    </li>
                }
            }).collect_view()}
        </ol>
    }
}

fn detail(i: &IdeaVm, back: String) -> impl IntoView {
    view! {
        <article class="oc-res-doc" data-part="idea">
            {detail_head(back, None, i.title.clone(), Some(idea_state(i.state)), Some(i.classification))}
            {lifecycle(i.state)}
            {i.outcome_note.clone().map(|n| view! {
                <p class="oc-app-note" data-tone="warn">{icon("archive")}<span><strong>{t("ideas.outcome")}</strong>{" "}{n}</span></p>
            })}
            <div class="oc-res-actions">
                {i.transitions.as_ref().map(transitions)}
                {i.nye.as_ref().map(nye)}
            </div>
            {i.promotion.as_ref().map(|p| view! {
                <details class="oc-res-promote" data-part="idea-promote">
                    <summary class="oc-app-primary">{icon("project")}<span>{t("ideas.promote")}</span></summary>
                    <form method="post" action=p.action.clone() class="oc-res-form">
                        <p class="oc-res-form__hint">{t("ideas.promote.hint")}</p>
                        {input("code", "projects.code", "", "text", true)}
                        {input("title", "projects.title", &i.title, "text", false)}
                        {textarea("objectives", "projects.objectives", "", 3)}
                        {select("responsible_person_id", "projects.responsible", &p.responsible, false)}
                        <div class="oc-res-form__foot">
                            <span class="oc-app__spacer"></span>
                            <button type="submit" class="oc-app-primary">{icon("check")}<span>{t("ideas.promote.do")}</span></button>
                        </div>
                    </form>
                </details>
            })}
            {i.promoted_project.as_ref().map(|p| links_section("ideas.promoted_to", std::slice::from_ref(p), None, None))}
            <dl class="oc-res-meta">
                {kv("res.unit", i.unit.clone())}
                {kv("res.created", i.created.clone())}
            </dl>
            {keywords(&i.keywords)}
            {prose("ideas.summary", i.summary.as_ref())}
            {prose("ideas.question", i.research_question.as_ref())}
            {prose("ideas.hypothesis", i.hypothesis.as_ref())}
            {prose("ideas.motivation", i.motivation.as_ref())}
            {links_section("res.links", &i.links, None, None)}
            {people_section(&i.members)}
        </article>
    }
}

fn form(f: &IdeaFormVm) -> impl IntoView {
    let id = doc_form_id("ideas", "new");
    view! {
        <form class="oc-res-form" id=id method="post" action=f.action.clone() data-oc="app-doc" data-state="clean" aria-labelledby="oc-res-title">
            <header class="oc-res-head">
                <a class="oc-app__icon oc-res-back" href=f.cancel_href.clone() aria-label=t("res.back")>{icon("chev-l")}</a>
                <div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("ideas.new")}</h2></div>
            </header>
            {f.error.map(error)}
            <div class="oc-app-row2">
                {select("unit_id", "res.unit", &f.units, true)}
                {select("classification", "res.classification", &f.classifications, false)}
            </div>
            {input("title", "ideas.title", &f.title, "text", true)}
            {textarea("summary", "ideas.summary", &f.summary, 3)}
            {textarea("research_question", "ideas.question", &f.research_question, 2)}
            {textarea("hypothesis", "ideas.hypothesis", &f.hypothesis, 2)}
            {textarea("motivation", "ideas.motivation", &f.motivation, 2)}
            {input("keywords", "res.keywords.input", &f.keywords, "text", false)}
            <p class="oc-res-form__hint">{t("ideas.new.hint")}</p>
            <div class="oc-res-form__foot">
                <a class="oc-app-btn" href=f.cancel_href.clone()>{t("app.cancel")}</a>
                <span class="oc-app__spacer"></span>
                <button type="submit" class="oc-app-primary" data-oc="app-save">{icon("check")}<span>{t("ideas.create")}</span></button>
            </div>
        </form>
    }
}

/// A aplicação Ideias.
pub fn app(vm: &IdeasVm) -> AnyView {
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.ideas")}</h2>
        <span class="oc-app__spacer"></span>
        {vm.new_href.clone().map(|h| view! { <a class="oc-app-primary" href=h aria-label=t("ideas.new")>{icon("plus")}<span>{t("ideas.new")}</span></a> })}
    }
    .into_any();
    let list_view = list(
        &vm.list,
        "ideas.list",
        empty("idea", "ideas.empty", Some("ideas.empty.body")).into_any(),
    );
    let detail_view = match (&vm.form, &vm.idea) {
        (Some(f), _) => form(f).into_any(),
        (None, Some(i)) => detail(i, vm.list_href.clone()).into_any(),
        (None, None) => detail_or(
            vm.idea_error,
            empty("idea", "ideas.none_open", None).into_any(),
        ),
    };
    frame(
        "ideas",
        t("nav.ideas").to_owned(),
        toolbar,
        Some(nav(&vm.nav).into_any()),
        two_pane(vm.pane, list_view, detail_view),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;

    #[test]
    fn o_ciclo_marca_o_estadio_actual_e_o_fecho() {
        let html = view! { {lifecycle(IdeaStage::Concept)} }.to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"aria-current="step""#) && html.contains(t("ideas.state.concept")));
        let closed = view! { {lifecycle(IdeaStage::Rejected)} }.to_html();
        assert!(closed.contains("data-closed") && !closed.contains(r#"aria-current="step""#));
    }
}
