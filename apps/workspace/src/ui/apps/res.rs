//! D005 · As peças partilhadas das cinco aplicações de investigação e
//! trabalho. DESIGN_LOCKED.
//!
//! Estendem o sistema de aplicação D004 ([`super`]) sem o duplicar: a moldura,
//! a navegação, a pesquisa, os estados, a paginação e a Nye contextual são os
//! mesmos. Aqui ficam apenas o que as cinco partilham e as quatro de D004 não
//! tinham: a lista densa com prioridade de colunas, as duas partes (lista e
//! detalhe), o estado com texto e tom, as transições que o Core devolve e a
//! ligação a um recurso canónico.

use leptos::prelude::*;

use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::view_models::{
    AppError, DatasetStatus, DatasetVersionStatus, IdeaStage, ProjectStatus, ResClassification,
    ResItemVm, ResKind, ResLinkVm, ResListVm, ResOptionVm, ResPane, ResPersonVm, ResStateVm,
    ResTone, ResTransitionsVm, TaskPriorityLevel, TaskStatus,
};

use super::{error, load_state, more};

const fn tone(t: ResTone) -> &'static str {
    match t {
        ResTone::Neutral => "neutral",
        ResTone::Progress => "progress",
        ResTone::Attention => "attention",
        ResTone::Done => "done",
        ResTone::Closed => "closed",
    }
}

/// O estado de um projecto.
#[must_use]
pub const fn project_state(s: ProjectStatus) -> ResStateVm {
    let (key, tone) = match s {
        ProjectStatus::Draft => ("res.project.state.draft", ResTone::Neutral),
        ProjectStatus::Active => ("res.project.state.active", ResTone::Progress),
        ProjectStatus::OnHold => ("res.project.state.on_hold", ResTone::Attention),
        ProjectStatus::Completed => ("res.project.state.completed", ResTone::Done),
        ProjectStatus::Archived => ("res.project.state.archived", ResTone::Closed),
    };
    ResStateVm { key, tone }
}

/// O estado de uma tarefa.
#[must_use]
pub const fn task_state(s: TaskStatus) -> ResStateVm {
    let (key, tone) = match s {
        TaskStatus::Todo => ("work.state.todo", ResTone::Neutral),
        TaskStatus::InProgress => ("work.state.in_progress", ResTone::Progress),
        TaskStatus::Blocked => ("work.state.blocked", ResTone::Attention),
        TaskStatus::InReview => ("work.state.in_review", ResTone::Attention),
        TaskStatus::Done => ("work.state.done", ResTone::Done),
        TaskStatus::Cancelled => ("work.state.cancelled", ResTone::Closed),
    };
    ResStateVm { key, tone }
}

/// O estádio de uma ideia.
#[must_use]
pub const fn idea_state(s: IdeaStage) -> ResStateVm {
    let (key, tone) = match s {
        IdeaStage::Discovery => ("ideas.state.discovery", ResTone::Neutral),
        IdeaStage::Exploration => ("ideas.state.exploration", ResTone::Progress),
        IdeaStage::Concept => ("ideas.state.concept", ResTone::Progress),
        IdeaStage::Review => ("ideas.state.review", ResTone::Attention),
        IdeaStage::ProjectCandidate => ("ideas.state.project_candidate", ResTone::Attention),
        IdeaStage::Promoted => ("ideas.state.promoted", ResTone::Done),
        IdeaStage::Rejected => ("ideas.state.rejected", ResTone::Closed),
        IdeaStage::Archived => ("ideas.state.archived", ResTone::Closed),
    };
    ResStateVm { key, tone }
}

/// O estado de um dataset.
#[must_use]
pub const fn dataset_state(s: DatasetStatus) -> ResStateVm {
    let (key, tone) = match s {
        DatasetStatus::Draft => ("data.state.draft", ResTone::Neutral),
        DatasetStatus::Active => ("data.state.active", ResTone::Progress),
        DatasetStatus::Deprecated => ("data.state.deprecated", ResTone::Attention),
        DatasetStatus::Archived => ("data.state.archived", ResTone::Closed),
    };
    ResStateVm { key, tone }
}

/// O estado de uma versão de dataset.
#[must_use]
pub const fn version_state(s: DatasetVersionStatus) -> ResStateVm {
    let (key, tone) = match s {
        DatasetVersionStatus::Draft => ("data.version.draft", ResTone::Neutral),
        DatasetVersionStatus::Published => ("data.version.published", ResTone::Done),
        DatasetVersionStatus::Withdrawn => ("data.version.withdrawn", ResTone::Closed),
    };
    ResStateVm { key, tone }
}

/// O estado, com texto (a cor nunca é a única portadora).
pub fn state_tag(s: ResStateVm) -> impl IntoView {
    view! { <span class="oc-res-state" data-tone=tone(s.tone)>{t(s.key)}</span> }
}

/// A classificação. Confidencial e Restrito levam o cadeado.
pub fn class_tag(c: ResClassification) -> impl IntoView {
    let strong = matches!(
        c,
        ResClassification::Confidential | ResClassification::Restricted
    );
    view! {
        <span class="oc-res-class" data-strong=strong.then_some("")>
            {strong.then(|| icon("lock"))}{t(c.key())}
        </span>
    }
}

/// A prioridade: ícone + texto. `Normal` não se sinaliza na lista.
pub fn priority_tag(p: TaskPriorityLevel) -> impl IntoView {
    let level = match p {
        TaskPriorityLevel::Low => "low",
        TaskPriorityLevel::Normal => "normal",
        TaskPriorityLevel::High => "high",
        TaskPriorityLevel::Critical => "critical",
    };
    let ic = match p {
        TaskPriorityLevel::Low => "chev-d",
        TaskPriorityLevel::Normal => "minus",
        TaskPriorityLevel::High => "chev-u",
        TaskPriorityLevel::Critical => "warning",
    };
    view! { <span class="oc-res-prio" data-level=level>{icon(ic)}<span>{t(p.key())}</span></span> }
}

/// O ícone e o rótulo de um tipo de recurso.
#[must_use]
pub const fn kind_meta(k: ResKind) -> (&'static str, &'static str) {
    match k {
        ResKind::Project => ("project", "res.kind.project"),
        ResKind::Idea => ("idea", "res.kind.idea"),
        ResKind::Task => ("work", "res.kind.task"),
        ResKind::Dataset => ("data", "res.kind.dataset"),
        ResKind::DatasetVersion => ("data", "res.kind.dataset_version"),
        ResKind::Source => ("bibliography", "res.kind.source"),
        ResKind::Document => ("ot-doc", "res.kind.document"),
        ResKind::Note => ("notes", "res.kind.note"),
        ResKind::File => ("files", "res.kind.file"),
        ResKind::Other => ("link", "res.kind.other"),
    }
}

/// Uma ligação a um recurso canónico: tipo, título, metadata curta, relação.
/// Abre a aplicação dona do recurso; nunca o copia para aqui.
pub fn link_row(l: &ResLinkVm) -> impl IntoView {
    let (ic, kind_key) = kind_meta(l.kind);
    let kind = l
        .kind_label
        .clone()
        .unwrap_or_else(|| t(kind_key).to_owned());
    let inner = view! {
        <span class="oc-res-link__ic" aria-hidden="true">{icon(ic)}</span>
        <span class="oc-res-link__main">
            <span class="oc-res-link__title">{l.title.clone()}</span>
            <span class="oc-res-link__meta">
                {l.relation.map(|r| view! { <span class="oc-res-link__rel">{t(r.key())}</span> })}
                <span>{kind}</span>
                {l.meta.clone().map(|m| view! { <span>{m}</span> })}
                {l.by_operation.then(|| view! { <span>{t("res.rel.by_operation")}</span> })}
            </span>
        </span>
    };
    match l.href.clone() {
        Some(h) => {
            view! { <li><a class="oc-res-link" href=h data-part="res-link">{inner}</a></li> }
                .into_any()
        }
        None => {
            view! { <li><span class="oc-res-link" data-static="">{inner}</span></li> }.into_any()
        }
    }
}

/// Uma secção de ligações, com título e um vazio preciso.
pub fn links_section(
    title_key: &'static str,
    items: &[ResLinkVm],
    empty_key: Option<&'static str>,
    more: Option<(String, &'static str)>,
) -> AnyView {
    if items.is_empty() && empty_key.is_none() {
        return ().into_any();
    }
    view! {
        <section class="oc-res-sec">
            <h3 class="oc-res-sec__title">{t(title_key)}</h3>
            {if items.is_empty() {
                view! { <p class="oc-res-sec__empty">{empty_key.map(t)}</p> }.into_any()
            } else {
                view! { <ul class="oc-res-links">{items.iter().map(link_row).collect_view()}</ul> }.into_any()
            }}
            {more.map(|(h, k)| view! { <a class="oc-res-sec__more" href=h>{t(k)}</a> })}
        </section>
    }
    .into_any()
}

/// As pessoas de um ambiente (só leitura).
pub fn people_section(people: &[ResPersonVm]) -> AnyView {
    if people.is_empty() {
        return ().into_any();
    }
    view! {
        <section class="oc-res-sec">
            <h3 class="oc-res-sec__title">{t("res.members")}</h3>
            <ul class="oc-res-people">
                {people.iter().map(|p| view! {
                    <li><span class="oc-res-people__name">{p.name.clone()}</span>{p.role.clone().map(|r| view! { <span class="oc-res-people__role">{r}</span> })}</li>
                }).collect_view()}
            </ul>
        </section>
    }
    .into_any()
}

/// As transições que o Core permite agora. Um formulário; cada botão envia o
/// estado de destino. As que exigem motivo abrem um campo obrigatório.
pub fn transitions(tr: &ResTransitionsVm) -> AnyView {
    if tr.options.is_empty() {
        return ().into_any();
    }
    let direct: Vec<_> = tr.options.iter().filter(|o| !o.requires_note).collect();
    let noted: Vec<_> = tr.options.iter().filter(|o| o.requires_note).collect();
    view! {
        <form class="oc-res-trans" method="post" action=tr.action.clone() data-part="res-transitions">
            <span class="oc-res-trans__label">{t("res.transitions")}</span>
            <div class="oc-res-trans__row">
                {direct.into_iter().map(|o| {
                    let class = if o.primary { "oc-app-primary" } else { "oc-app-btn" };
                    view! { <button type="submit" class=class name="state" value=o.value>{t(o.label_key)}</button> }
                }).collect_view()}
            </div>
            {(!noted.is_empty()).then(|| view! {
                <details class="oc-res-trans__close">
                    <summary class="oc-app-btn">{t("res.transitions.close")}</summary>
                    <label class="oc-app-field">
                        <span>{t("res.transitions.note")}</span>
                        <textarea name="outcome_note" rows="3" required="" data-part="res-note"></textarea>
                    </label>
                    <div class="oc-res-trans__row">
                        {noted.into_iter().map(|o| view! {
                            <button type="submit" class="oc-app-btn oc-app-btn--danger" name="state" value=o.value>{t(o.label_key)}</button>
                        }).collect_view()}
                    </div>
                </details>
            })}
        </form>
    }
    .into_any()
}

/// Um campo de texto de um recurso: parágrafos a partir de linhas em branco.
/// Texto simples, sempre escapado: nunca HTML guardado.
pub fn prose(label_key: &'static str, text: Option<&String>) -> AnyView {
    match text {
        Some(s) if !s.trim().is_empty() => view! {
            <section class="oc-res-sec">
                <h3 class="oc-res-sec__title">{t(label_key)}</h3>
                <div class="oc-res-prose">
                    {s.split("\n\n").map(|p| view! { <p>{p.to_owned()}</p> }).collect_view()}
                </div>
            </section>
        }
        .into_any(),
        _ => ().into_any(),
    }
}

/// Palavras-chave (dados do domínio, não um sistema de etiquetas).
pub fn keywords(words: &[String]) -> AnyView {
    if words.is_empty() {
        return ().into_any();
    }
    view! {
        <ul class="oc-res-kw" aria-label=t("res.keywords")>
            {words.iter().map(|w| view! { <li>{w.clone()}</li> }).collect_view()}
        </ul>
    }
    .into_any()
}

/// Um par chave/valor dos metadados (`dl`).
pub fn kv(key: &'static str, value: Option<String>) -> AnyView {
    value
        .map(|v| view! { <div class="oc-app-kv"><dt>{t(key)}</dt><dd>{v}</dd></div> }.into_any())
        .unwrap_or_else(|| ().into_any())
}

/// O cabeçalho do detalhe: voltar (janela estreita), código, título, estado.
pub fn detail_head(
    back_href: String,
    code: Option<String>,
    title: String,
    state: Option<ResStateVm>,
    class: Option<ResClassification>,
) -> impl IntoView {
    view! {
        <header class="oc-res-head">
            <a class="oc-app__icon oc-res-back" href=back_href aria-label=t("res.back")>{icon("chev-l")}</a>
            <div class="oc-res-head__text">
                {code.map(|c| view! { <p class="oc-res-head__code">{c}</p> })}
                <h2 class="oc-res-head__title" id="oc-res-title">{title}</h2>
                <p class="oc-res-head__tags">
                    {state.map(state_tag)}
                    {class.map(class_tag)}
                </p>
            </div>
        </header>
    }
}

fn row(it: &ResItemVm, cols: usize) -> impl IntoView {
    view! {
        <tr class="oc-res-row" data-part="res-item" data-open=it.active.then_some("")>
            <td class="oc-res-c-title">
                <a class="oc-res-name" href=it.href.clone() aria-current=it.active.then_some("true") data-part="res-open">
                    <span class="oc-res-name__title">{it.title.clone()}</span>
                    <span class="oc-res-name__meta">
                        {it.code.clone().map(|c| view! { <span class="oc-res-name__code">{c}</span> })}
                        {it.state.map(state_tag)}
                        {it.priority.filter(|p| *p != TaskPriorityLevel::Normal).map(priority_tag)}
                    </span>
                </a>
            </td>
            {(0..cols).map(|i| {
                let v = it.cells.get(i).cloned().flatten();
                let od = i == 0 && it.overdue;
                view! { <td class="oc-res-c" data-prio=(i + 1).to_string() data-overdue=od.then_some("")>{v.unwrap_or_else(|| "—".into())}</td> }
            }).collect_view()}
        </tr>
    }
}

/// A lista densa. A primeira coluna (título + estado) nunca cede; as outras
/// saem por prioridade (`data-prio`) quando a lista estreita.
pub fn list(l: &ResListVm, label_key: &'static str, empty: AnyView) -> AnyView {
    if let Some(s) = load_state(l.load, 8) {
        return s;
    }
    if l.items.is_empty() {
        return empty;
    }
    let cols = l.columns.len();
    view! {
        <table class="oc-res-table" aria-label=t(label_key) data-oc="res-list">
            <thead>
                <tr>
                    <th scope="col" class="oc-res-c-title">{t("res.col.title")}</th>
                    {l.columns.iter().enumerate().map(|(i, k)| view! { <th scope="col" class="oc-res-c" data-prio=(i + 1).to_string()>{t(k)}</th> }).collect_view()}
                </tr>
            </thead>
            <tbody>{l.items.iter().map(|it| row(it, cols)).collect_view()}</tbody>
        </table>
        {more(&l.page)}
    }
    .into_any()
}

/// As duas partes: lista e detalhe. Largas lado a lado; estreitas, uma de cada vez.
pub fn two_pane(pane: ResPane, list: AnyView, detail: AnyView) -> AnyView {
    let p = match pane {
        ResPane::List => "list",
        ResPane::Detail => "detail",
    };
    view! {
        <div class="oc-res" data-pane=p>
            <div class="oc-res__list" data-part="res-list">{list}</div>
            <div class="oc-res__detail" data-part="res-detail" aria-labelledby="oc-res-title">{detail}</div>
        </div>
    }
    .into_any()
}

/// O detalhe vazio ou o erro do que foi pedido.
pub fn detail_or(err: Option<AppError>, none: AnyView) -> AnyView {
    err.map_or(none, |e| error(e).into_any())
}

/// Um selector governado (as opções vêm do Core).
pub fn select(
    name: &'static str,
    label_key: &'static str,
    options: &[ResOptionVm],
    required: bool,
) -> impl IntoView {
    view! {
        <label class="oc-app-field">
            <span>{t(label_key)}</span>
            <select name=name required=required.then_some("")>
                {options.iter().map(|o| view! { <option value=o.value.clone() selected=o.selected>{o.label.clone()}</option> }).collect_view()}
            </select>
        </label>
    }
}

/// Um campo de texto de uma linha.
pub fn input(
    name: &'static str,
    label_key: &'static str,
    value: &str,
    kind: &'static str,
    required: bool,
) -> impl IntoView {
    view! {
        <label class="oc-app-field">
            <span>{t(label_key)}</span>
            <input type=kind name=name value=value.to_owned() required=required.then_some("") autocomplete="off" />
        </label>
    }
}

/// Um campo de texto de várias linhas (conteúdo escapado: `rcdata`).
pub fn textarea(
    name: &'static str,
    label_key: &'static str,
    value: &str,
    rows: u8,
) -> impl IntoView {
    view! {
        <label class="oc-app-field">
            <span>{t(label_key)}</span>
            <textarea name=name rows=rows.to_string()>{crate::text::rcdata(value)}</textarea>
        </label>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{AppLoad, AppPageVm, ResTransitionVm};

    #[test]
    fn a_lista_tem_cabecalhos_e_o_titulo_nunca_cede() {
        let l = ResListVm {
            columns: vec!["work.col.due", "work.col.assignee"],
            items: vec![ResItemVm {
                title: "<b>x</b>".into(),
                code: None,
                state: Some(task_state(TaskStatus::Blocked)),
                cells: vec![Some("3 out".into()), None],
                overdue: true,
                priority: Some(TaskPriorityLevel::High),
                href: "/my-work/t1".into(),
                active: true,
            }],
            load: AppLoad::Ready,
            page: AppPageVm::default(),
        };
        let html = list(&l, "work.list", ().into_any()).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"scope="col""#) && html.contains(r#"data-prio="2""#));
        assert!(html.contains("&lt;b&gt;") && html.contains(r#"aria-current="true""#));
        assert!(html.contains(t("work.state.blocked")) && html.contains(t("work.prio.high")));
    }

    #[test]
    fn transicoes_so_as_do_core_e_fechar_exige_motivo() {
        let tr = ResTransitionsVm {
            action: "/ideas/i/transitions".into(),
            options: vec![
                ResTransitionVm {
                    value: "concept",
                    label_key: "ideas.to.concept",
                    requires_note: false,
                    primary: true,
                },
                ResTransitionVm {
                    value: "rejected",
                    label_key: "ideas.to.rejected",
                    requires_note: true,
                    primary: false,
                },
            ],
        };
        let html = transitions(&tr).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"value="concept""#) && html.contains(r#"name="outcome_note""#));
        assert!(html.contains(r#"required"#));
    }

    #[test]
    fn uma_ligacao_sem_ecra_nao_e_um_elo() {
        let l = ResLinkVm {
            kind: ResKind::File,
            kind_label: None,
            title: "a.csv".into(),
            meta: None,
            relation: None,
            by_operation: false,
            href: None,
        };
        let html = view! { <ul>{link_row(&l)}</ul> }.to_html();
        assert!(!html.contains("<a ") && html.contains(r#"data-static"#));
    }
}
