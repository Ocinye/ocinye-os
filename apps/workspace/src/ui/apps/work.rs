//! D005 · O Meu Trabalho (tarefas). DESIGN_LOCKED.
//!
//! Tarefas reais do Core: estado com transições governadas (concluir e reabrir
//! são transições, nunca uma caixa que só muda o ecrã), prioridade com texto,
//! prazo como data no fuso do membro, atribuição só entre candidatos que o
//! Core devolve. Sem quadro Kanban: o Core não ordena colunas, e a lista densa
//! serve melhor o trabalho diário.

use leptos::prelude::*;

use super::res::{
    detail_head, detail_or, input, kv, links_section, list, priority_tag, select, task_state,
    textarea, transitions, two_pane,
};
use super::{doc_form_id, empty, error, frame, nav, nye};
use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::view_models::{ResOptionVm, TaskFormVm, TaskPriorityLevel, TaskVm, WorkVm};

fn detail(k: &TaskVm, back: String) -> impl IntoView {
    view! {
        <article class="oc-res-doc" data-part="task">
            {detail_head(back, None, k.title.clone(), Some(task_state(k.state)), Some(k.classification))}
            <div class="oc-res-actions">
                {k.transitions.as_ref().map(transitions)}
                {k.nye.as_ref().map(nye)}
            </div>
            <dl class="oc-res-meta">
                <div class="oc-app-kv"><dt>{t("work.col.priority")}</dt><dd>{priority_tag(k.priority)}</dd></div>
                {k.due.clone().map(|d| view! {
                    <div class="oc-app-kv"><dt>{t("work.col.due")}</dt><dd class="oc-res-due" data-overdue=k.overdue.then_some("")>{d}{k.overdue.then(|| view! { <span class="oc-res-due__flag">{icon("warning")}{t("work.overdue")}</span> })}</dd></div>
                })}
                {kv("work.col.assignee", Some(k.assignee.clone().unwrap_or_else(|| t("work.unassigned").to_owned())))}
                {kv("work.closed", k.closed.clone())}
            </dl>
            {k.assign.as_ref().map(|a| view! {
                <form class="oc-res-assign" method="post" action=a.action.clone() data-part="task-assign">
                    {select("assignee_id", "work.assign", &a.candidates, false)}
                    <button type="submit" class="oc-app-btn">{t("work.assign.do")}</button>
                </form>
            })}
            {k.workspace.as_ref().map(|w| links_section("res.workspace", std::slice::from_ref(w), None, None))}
            {super::res::prose("work.description", k.description.as_ref())}
            {links_section("res.links", &k.links, None, None)}
        </article>
    }
}

fn prio_options(p: TaskPriorityLevel) -> Vec<ResOptionVm> {
    [
        (TaskPriorityLevel::Low, "low"),
        (TaskPriorityLevel::Normal, "normal"),
        (TaskPriorityLevel::High, "high"),
        (TaskPriorityLevel::Critical, "critical"),
    ]
    .into_iter()
    .map(|(l, v)| ResOptionVm {
        value: v.into(),
        label: t(l.key()).to_owned(),
        selected: l == p,
    })
    .collect()
}

fn form(f: &TaskFormVm) -> impl IntoView {
    let id = doc_form_id("work", "new");
    view! {
        <form class="oc-res-form" id=id method="post" action=f.action.clone() data-oc="app-doc" data-state="clean" aria-labelledby="oc-res-title">
            <header class="oc-res-head">
                <a class="oc-app__icon oc-res-back" href=f.cancel_href.clone() aria-label=t("res.back")>{icon("chev-l")}</a>
                <div class="oc-res-head__text"><h2 class="oc-res-head__title" id="oc-res-title">{t("work.new")}</h2></div>
            </header>
            {f.error.map(error)}
            {select("workspace", "res.workspace", &f.workspaces, true)}
            {input("title", "work.title", &f.title, "text", true)}
            {textarea("description", "work.description", &f.description, 4)}
            <div class="oc-app-row2">
                {select("priority", "work.col.priority", &prio_options(f.priority), true)}
                {input("due_on", "work.col.due", &f.due, "date", false)}
            </div>
            <p class="oc-res-form__hint">{t("work.new.hint")}</p>
            <div class="oc-res-form__foot">
                <a class="oc-app-btn" href=f.cancel_href.clone()>{t("app.cancel")}</a>
                <span class="oc-app__spacer"></span>
                <button type="submit" class="oc-app-primary" data-oc="app-save">{icon("check")}<span>{t("work.create")}</span></button>
            </div>
        </form>
    }
}

/// A aplicação O Meu Trabalho.
pub fn app(vm: &WorkVm) -> AnyView {
    let toolbar = view! {
        <h2 class="oc-app__title">{t("nav.my_work")}</h2>
        <span class="oc-app__spacer"></span>
        {(!vm.workspace_filter.is_empty()).then(|| view! {
            <form class="oc-res-filter" method="get" role="search">
                <label><span class="oc-sr">{t("res.filter.workspace")}</span>
                    <select name="workspace">
                        {vm.workspace_filter.iter().map(|o| view! { <option value=o.value.clone() selected=o.selected>{o.label.clone()}</option> }).collect_view()}
                    </select>
                </label>
                <button type="submit" class="oc-app-btn">{t("res.filter.apply")}</button>
            </form>
        })}
        {vm.new_href.clone().map(|h| view! { <a class="oc-app-primary" href=h aria-label=t("work.new")>{icon("plus")}<span>{t("work.new")}</span></a> })}
    }
    .into_any();
    let list_view = list(
        &vm.list,
        "work.list",
        empty("work", "work.empty", Some("work.empty.body")).into_any(),
    );
    let detail_view = match (&vm.form, &vm.task) {
        (Some(f), _) => form(f).into_any(),
        (None, Some(k)) => detail(k, vm.list_href.clone()).into_any(),
        (None, None) => detail_or(
            vm.task_error,
            empty("work", "work.none_open", None).into_any(),
        ),
    };
    frame(
        "work",
        t("nav.my_work").to_owned(),
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
        AppLoad, AppPageVm, ResClassification, ResListVm, ResPane, ResTransitionVm,
        ResTransitionsVm, TaskStatus,
    };

    fn vm() -> WorkVm {
        WorkVm {
            nav: vec![],
            workspace_filter: vec![],
            list: ResListVm {
                columns: vec![],
                items: vec![],
                load: AppLoad::Ready,
                page: AppPageVm::default(),
            },
            pane: ResPane::Detail,
            task: None,
            task_error: None,
            form: None,
            new_href: None,
            list_href: "/my-work".into(),
        }
    }

    fn task() -> TaskVm {
        TaskVm {
            title: "Calibrar".into(),
            state: TaskStatus::Done,
            priority: TaskPriorityLevel::High,
            classification: ResClassification::Internal,
            description: None,
            due: Some("3 out 2026".into()),
            overdue: false,
            assignee: None,
            workspace: None,
            closed: Some("hoje".into()),
            transitions: Some(ResTransitionsVm {
                action: "/my-work/t/transitions".into(),
                options: vec![ResTransitionVm {
                    value: "in_progress",
                    label_key: "work.to.reopen",
                    requires_note: false,
                    primary: true,
                }],
            }),
            assign: None,
            links: vec![],
            nye: None,
        }
    }

    #[test]
    fn concluida_so_se_reabre_por_transicao_sem_caixa_de_verificacao() {
        let mut v = vm();
        v.task = Some(task());
        let html = app(&v).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"value="in_progress""#) && !html.contains(r#"type="checkbox""#));
        assert!(html.contains(t("work.unassigned")));
    }

    #[test]
    fn sem_ambiente_onde_criar_nao_ha_nova_tarefa() {
        let html = app(&vm()).to_html();
        assert!(!html.contains(t("work.new")));
    }
}
