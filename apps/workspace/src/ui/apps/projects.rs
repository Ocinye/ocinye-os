//! D005 · Projectos. DESIGN_LOCKED.
//!
//! Um projecto coordena trabalho num ambiente de investigação: não é uma
//! pasta. Nasce da promoção de uma ideia (o Core não tem outra criação), e o
//! ambiente, com o que se juntou ao explorar, passa com ele. A vista mostra o
//! estado, a linhagem, as pessoas e o trabalho ligado; muda o estado só pelas
//! transições que o Core devolve.

use leptos::prelude::*;

use super::res::{
    detail_head, detail_or, kv, links_section, list, people_section, project_state, prose,
    transitions, two_pane,
};
use super::{empty, frame, nav, nye};
use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::view_models::{ProjectVm, ProjectsVm};

fn detail(p: &ProjectVm, back: String) -> impl IntoView {
    view! {
        <article class="oc-res-doc" data-part="project">
            {detail_head(back, Some(p.code.clone()), p.title.clone(), Some(project_state(p.state)), Some(p.classification))}
            <div class="oc-res-actions">
                {p.transitions.as_ref().map(transitions)}
                {p.nye.as_ref().map(nye)}
            </div>
            <dl class="oc-res-meta">
                {kv("res.unit", p.unit.clone())}
                {kv("projects.responsible", p.responsible.clone())}
                {kv("projects.started", p.started.clone())}
                {kv("projects.completed", p.completed.clone())}
            </dl>
            {p.origin_idea.as_ref().map(|i| links_section("projects.origin", std::slice::from_ref(i), None, None))}
            {prose("projects.summary", p.summary.as_ref())}
            {prose("projects.objectives", p.objectives.as_ref())}
            {links_section(
                "projects.tasks",
                &p.tasks,
                Some("projects.tasks.empty"),
                p.tasks_href.clone().map(|h| (h, "projects.tasks.all")),
            )}
            {p.new_task_href.clone().map(|h| view! { <a class="oc-app-btn oc-res-inline-new" href=h>{icon("plus")}<span>{t("work.new")}</span></a> })}
            {links_section("projects.datasets", &p.datasets, None, None)}
            {links_section("projects.knowledge", &p.knowledge, None, None)}
            {links_section("res.links", &p.links, None, None)}
            {people_section(&p.members)}
        </article>
    }
}

/// A aplicação Projectos.
pub fn app(vm: &ProjectsVm) -> AnyView {
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.projects")}</h2>
        <span class="oc-app__spacer"></span>
    }
    .into_any();
    let none = view! {
        <div class="oc-app-state" data-state="empty" role="status">
            <span class="oc-app-state__icon" aria-hidden="true">{icon("project")}</span>
            <p class="oc-app-state__title">{t("projects.empty")}</p>
            <p class="oc-app-state__body">{t("projects.empty.body")}</p>
            {vm.ideas_href.clone().map(|h| view! { <a class="oc-app-btn" href=h>{icon("idea")}<span>{t("projects.to_ideas")}</span></a> })}
        </div>
    }
    .into_any();
    let list_view = list(&vm.list, "projects.list", none);
    let detail_view = match &vm.project {
        Some(p) => detail(p, vm.list_href.clone()).into_any(),
        None => detail_or(
            vm.project_error,
            empty("project", "projects.none_open", None).into_any(),
        ),
    };
    frame(
        "projects",
        t("nav.projects").to_owned(),
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
    use crate::ui::view_models::{
        AppError, AppLoad, AppPageVm, ProjectStatus, ResClassification, ResListVm, ResPane,
    };

    fn vm() -> ProjectsVm {
        ProjectsVm {
            nav: vec![],
            list: ResListVm {
                columns: vec![],
                items: vec![],
                load: AppLoad::Ready,
                page: AppPageVm::default(),
            },
            pane: ResPane::List,
            project: None,
            project_error: None,
            ideas_href: Some("/ideas".into()),
            list_href: "/projects".into(),
        }
    }

    #[test]
    fn sem_projectos_diz_de_onde_nascem_e_nao_oferece_criar() {
        let html = app(&vm()).to_html();
        assert_contracts(&html);
        assert!(html.contains(t("projects.empty.body")) && html.contains(r#"href="/ideas""#));
        assert!(!html.contains("oc-app-primary"));
    }

    #[test]
    fn projecto_revogado_nao_mostra_conteudo() {
        let mut v = vm();
        v.pane = ResPane::Detail;
        v.project_error = Some(AppError::Revoked);
        assert!(app(&v)
            .to_html()
            .contains(r#"data-error="app.err.revoked""#));
    }

    #[test]
    fn projecto_sem_transicoes_nao_tem_botoes_de_estado() {
        let mut v = vm();
        v.project = Some(ProjectVm {
            code: "PRJ-1".into(),
            title: "<script>x</script>".into(),
            state: ProjectStatus::Archived,
            classification: ResClassification::Internal,
            summary: None,
            objectives: None,
            unit: None,
            responsible: None,
            started: None,
            completed: None,
            origin_idea: None,
            members: vec![],
            tasks: vec![],
            tasks_href: None,
            new_task_href: None,
            datasets: vec![],
            knowledge: vec![],
            links: vec![],
            transitions: None,
            nye: None,
        });
        let html = app(&v).to_html();
        assert_contracts(&html);
        assert!(!html.contains("<script>x") && !html.contains(r#"data-part="res-transitions""#));
    }
}
