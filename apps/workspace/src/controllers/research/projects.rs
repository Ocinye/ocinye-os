//! D005 · Projectos. Um projecto nasce da promoção de uma ideia (não há
//! criação directa no Core), vive num ambiente de investigação e move-se pelas
//! transições que o Core devolve.

use serde_json::Value;

use super::{nav, state};
use crate::controllers::desktop::{instant, text, Clock};
use crate::ui::view_models::{
    AppNavVm, ProjectStatus, ResItemVm, ResStateVm, ResTone, ResTransitionVm, ResTransitionsVm,
};

/// Os filtros da lista, com o significado que o Core lhes dá.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// «Em curso»: projectos `active` (`in_progress=true`).
    InProgress,
    /// «Os meus»: projectos em cujo ambiente o membro **tem papel**
    /// (`mine=true`) — participar, não apenas ver.
    Mine,
    /// «Todos»: todos os que o membro pode ver.
    All,
}

/// O filtro pedido (`?nav=`); por omissão, «Em curso».
#[must_use]
pub fn filter_of(v: Option<&str>) -> Filter {
    match v {
        Some("mine") => Filter::Mine,
        Some("all") => Filter::All,
        _ => Filter::InProgress,
    }
}

const fn param(f: Filter) -> &'static str {
    match f {
        Filter::InProgress => "in_progress",
        Filter::Mine => "mine",
        Filter::All => "all",
    }
}

/// O pedido ao Core para a lista.
#[must_use]
pub fn list_path(f: Filter, page: u32) -> String {
    let recorte = match f {
        Filter::InProgress => "&in_progress=true",
        Filter::Mine => "&mine=true",
        Filter::All => "",
    };
    format!("/api/v1/workspaces?kind=project{recorte}&page={page}&page_size=25")
}

/// A lista com o filtro corrente (o «voltar» e a paginação).
#[must_use]
pub fn list_href(f: Filter) -> String {
    format!("/projects?nav={}", param(f))
}

/// A navegação.
#[must_use]
pub fn navigation(f: Filter) -> Vec<AppNavVm> {
    [
        (Filter::InProgress, "projects.nav.in_progress", "clock"),
        (Filter::Mine, "projects.nav.mine", "user"),
        (Filter::All, "projects.nav.all", "project"),
    ]
    .into_iter()
    .map(|(x, k, i)| nav(k, i, list_href(x), x == f))
    .collect()
}

/// O estado do Core, um a um. Um estado novo não tem leitura aqui.
#[must_use]
pub fn status(v: &str) -> Option<ProjectStatus> {
    Some(match v {
        "draft" => ProjectStatus::Draft,
        "active" => ProjectStatus::Active,
        "on_hold" => ProjectStatus::OnHold,
        "completed" => ProjectStatus::Completed,
        "archived" => ProjectStatus::Archived,
        _ => return None,
    })
}

/// O rótulo e o tom de um estado.
#[must_use]
pub const fn state_vm(s: ProjectStatus) -> ResStateVm {
    match s {
        ProjectStatus::Draft => state("res.project.state.draft", ResTone::Neutral),
        ProjectStatus::Active => state("res.project.state.active", ResTone::Progress),
        ProjectStatus::OnHold => state("res.project.state.on_hold", ResTone::Attention),
        ProjectStatus::Completed => state("res.project.state.completed", ResTone::Done),
        ProjectStatus::Archived => state("res.project.state.archived", ResTone::Closed),
    }
}

/// As colunas da lista, por prioridade.
pub const COLUMNS: [&str; 3] = [
    "projects.col.unit",
    "projects.col.responsible",
    "projects.col.updated",
];

/// As linhas: cada ambiente com projecto, pelo resumo que o Core juntou.
/// Um ambiente sem projecto (ainda ideia) não é linha desta lista.
#[must_use]
pub(crate) fn items(list: &[Value], open: Option<&str>, clock: &Clock) -> Vec<ResItemVm> {
    list.iter()
        .filter_map(|w| {
            let s = w.get("summary")?;
            let id = text(s, "project_id");
            if id.is_empty() {
                return None;
            }
            Some(ResItemVm {
                title: text(w, "title").to_owned(),
                // O código do projecto, não o do ambiente.
                code: super::opt(s, "project_code"),
                state: status(text(s, "project_state")).map(state_vm),
                cells: vec![
                    super::opt(s, "unit_name"),
                    super::opt(s, "responsible_name"),
                    instant(s, "updated_at").map(|at| clock.when(at)),
                ],
                overdue: false,
                priority: None,
                href: format!("/projects/{id}"),
                active: open == Some(id),
            })
        })
        .collect()
}

/// O rótulo de uma transição de projecto.
#[must_use]
pub fn transition_label(to: &str) -> Option<&'static str> {
    Some(match to {
        "active" => "projects.to.active",
        "on_hold" => "projects.to.on_hold",
        "completed" => "projects.to.completed",
        "archived" => "projects.to.archived",
        _ => return None,
    })
}

/// As transições que o Core devolveu, quando o membro pode transitar neste
/// ambiente (`may_transition`, a mesma pergunta que a rota faz).
#[must_use]
pub fn transitions(project: &Value, may: bool, id: &str) -> Option<ResTransitionsVm> {
    if !may {
        return None;
    }
    let options: Vec<ResTransitionVm> = project
        .get("available_transitions")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter_map(|to| {
                    let label_key = transition_label(to)?;
                    let value = status(to).map(status_str)?;
                    Some(ResTransitionVm {
                        value,
                        label_key,
                        requires_note: false,
                        primary: matches!(to, "active" | "completed"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let mut options = options;
    // Uma acção principal só: a primeira que o for.
    let mut seen = false;
    for o in &mut options {
        if o.primary {
            o.primary = !seen;
            seen = true;
        }
    }
    (!options.is_empty()).then(|| ResTransitionsVm {
        action: format!("/projects/{id}/transitions"),
        options,
    })
}

/// O valor estável de um estado (o que se envia ao Core).
#[must_use]
pub const fn status_str(s: ProjectStatus) -> &'static str {
    match s {
        ProjectStatus::Draft => "draft",
        ProjectStatus::Active => "active",
        ProjectStatus::OnHold => "on_hold",
        ProjectStatus::Completed => "completed",
        ProjectStatus::Archived => "archived",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn os_filtros_tem_o_significado_do_core() {
        assert!(list_path(Filter::InProgress, 1).contains("&in_progress=true"));
        assert!(list_path(Filter::Mine, 1).contains("&mine=true"));
        let todos = list_path(Filter::All, 2);
        assert!(
            !todos.contains("in_progress") && !todos.contains("mine") && todos.contains("page=2")
        );
        assert_eq!(filter_of(None), Filter::InProgress);
    }

    #[test]
    fn um_estado_desconhecido_nao_ganha_rotulo() {
        assert_eq!(status("active"), Some(ProjectStatus::Active));
        assert_eq!(status("paused"), None);
    }

    #[test]
    fn transicoes_so_as_do_core_e_so_a_quem_pode() {
        let p = json!({ "available_transitions": ["on_hold", "completed", "archived"] });
        let t = transitions(&p, true, "x").unwrap();
        let v: Vec<_> = t.options.iter().map(|o| o.value).collect();
        assert_eq!(v, ["on_hold", "completed", "archived"]);
        assert_eq!(t.options.iter().filter(|o| o.primary).count(), 1);
        assert!(transitions(&p, false, "x").is_none());
        assert!(transitions(&json!({ "available_transitions": [] }), true, "x").is_none());
    }
}
