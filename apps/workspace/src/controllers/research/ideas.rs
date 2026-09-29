//! D005 · Ideias. Uma ideia tem campos estruturados (resumo, pergunta,
//! hipótese, motivação, palavras-chave), um ciclo de vida do Core e, quando é
//! candidata, a promoção: cria o projecto no mesmo ambiente e a ideia fica.

use serde_json::Value;

use super::{nav, state};
use crate::controllers::desktop::{instant, text, Clock};
use crate::ui::view_models::{
    AppNavVm, IdeaStage, ResItemVm, ResStateVm, ResTone, ResTransitionVm, ResTransitionsVm,
};

/// Os grupos da lista, pelo estádio que o Core guarda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// «Em desenvolvimento»: descoberta, exploração, conceito, revisão.
    Developing,
    /// «Candidatas»: `project_candidate`.
    Candidates,
    /// «Promovidas»: `promoted` (o ambiente já é de projecto).
    Promoted,
    /// «Encerradas»: rejeitadas e arquivadas.
    Closed,
    /// «As minhas»: ideias de ambientes onde o membro tem papel.
    Mine,
}

/// O grupo pedido; por omissão, em desenvolvimento.
#[must_use]
pub fn group_of(v: Option<&str>) -> Group {
    match v {
        Some("candidates") => Group::Candidates,
        Some("promoted") => Group::Promoted,
        Some("closed") => Group::Closed,
        Some("mine") => Group::Mine,
        _ => Group::Developing,
    }
}

const fn param(g: Group) -> &'static str {
    match g {
        Group::Developing => "developing",
        Group::Candidates => "candidates",
        Group::Promoted => "promoted",
        Group::Closed => "closed",
        Group::Mine => "mine",
    }
}

/// O pedido ao Core (`idea_state` é um filtro fechado do Core).
#[must_use]
pub fn list_path(g: Group, page: u32) -> String {
    let recorte = match g {
        Group::Developing => "idea_state=discovery,exploration,concept,review",
        Group::Candidates => "idea_state=project_candidate",
        Group::Promoted => "idea_state=promoted",
        Group::Closed => "idea_state=rejected,archived",
        Group::Mine => "mine=true",
    };
    format!("/api/v1/workspaces?{recorte}&page={page}&page_size=25")
}

/// A lista com o grupo corrente.
#[must_use]
pub fn list_href(g: Group) -> String {
    format!("/ideas?nav={}", param(g))
}

/// A navegação.
#[must_use]
pub fn navigation(g: Group) -> Vec<AppNavVm> {
    [
        (Group::Developing, "ideas.nav.developing", "idea"),
        (Group::Candidates, "ideas.nav.candidates", "star"),
        (Group::Promoted, "ideas.nav.promoted", "project"),
        (Group::Closed, "ideas.nav.closed", "archive"),
        (Group::Mine, "ideas.nav.mine", "user"),
    ]
    .into_iter()
    .map(|(x, k, i)| nav(k, i, list_href(x), x == g))
    .collect()
}

/// O estádio do Core, um a um.
#[must_use]
pub fn stage(v: &str) -> Option<IdeaStage> {
    Some(match v {
        "discovery" => IdeaStage::Discovery,
        "exploration" => IdeaStage::Exploration,
        "concept" => IdeaStage::Concept,
        "review" => IdeaStage::Review,
        "project_candidate" => IdeaStage::ProjectCandidate,
        "promoted" => IdeaStage::Promoted,
        "rejected" => IdeaStage::Rejected,
        "archived" => IdeaStage::Archived,
        _ => return None,
    })
}

/// O valor estável.
#[must_use]
pub const fn stage_str(s: IdeaStage) -> &'static str {
    match s {
        IdeaStage::Discovery => "discovery",
        IdeaStage::Exploration => "exploration",
        IdeaStage::Concept => "concept",
        IdeaStage::Review => "review",
        IdeaStage::ProjectCandidate => "project_candidate",
        IdeaStage::Promoted => "promoted",
        IdeaStage::Rejected => "rejected",
        IdeaStage::Archived => "archived",
    }
}

/// O rótulo e o tom.
#[must_use]
pub const fn state_vm(s: IdeaStage) -> ResStateVm {
    match s {
        IdeaStage::Discovery => state("ideas.state.discovery", ResTone::Neutral),
        IdeaStage::Exploration => state("ideas.state.exploration", ResTone::Progress),
        IdeaStage::Concept => state("ideas.state.concept", ResTone::Progress),
        IdeaStage::Review => state("ideas.state.review", ResTone::Attention),
        IdeaStage::ProjectCandidate => state("ideas.state.project_candidate", ResTone::Progress),
        IdeaStage::Promoted => state("ideas.state.promoted", ResTone::Done),
        IdeaStage::Rejected => state("ideas.state.rejected", ResTone::Closed),
        IdeaStage::Archived => state("ideas.state.archived", ResTone::Closed),
    }
}

/// As colunas.
pub const COLUMNS: [&str; 2] = ["ideas.col.unit", "ideas.col.updated"];

/// As linhas: cada ambiente com a sua ideia, pelo resumo do Core.
#[must_use]
pub(crate) fn items(list: &[Value], open: Option<&str>, clock: &Clock) -> Vec<ResItemVm> {
    list.iter()
        .filter_map(|w| {
            let s = w.get("summary")?;
            let id = text(s, "idea_id");
            if id.is_empty() {
                return None;
            }
            Some(ResItemVm {
                title: text(w, "title").to_owned(),
                code: Some(text(w, "code").to_owned()).filter(|c| !c.is_empty()),
                state: stage(text(s, "idea_state")).map(state_vm),
                cells: vec![
                    super::opt(s, "unit_name"),
                    instant(s, "updated_at").map(|at| clock.when(at)),
                ],
                overdue: false,
                priority: None,
                href: format!("/ideas/{id}"),
                active: open == Some(id),
            })
        })
        .collect()
}

/// As transições que o Core devolveu (com `requires_note`), a quem pode
/// transitar. A principal é o passo seguinte do ciclo.
#[must_use]
pub fn transitions(idea: &Value, may: bool, id: &str) -> Option<ResTransitionsVm> {
    if !may {
        return None;
    }
    let mut options: Vec<ResTransitionVm> = idea
        .get("available_transitions")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|o| {
                    let to = stage(text(o, "state"))?;
                    Some(ResTransitionVm {
                        value: stage_str(to),
                        label_key: match to {
                            IdeaStage::Discovery => "ideas.to.discovery",
                            IdeaStage::Exploration => "ideas.to.exploration",
                            IdeaStage::Concept => "ideas.to.concept",
                            IdeaStage::Review => "ideas.to.review",
                            IdeaStage::ProjectCandidate => "ideas.to.project_candidate",
                            IdeaStage::Rejected => "ideas.to.rejected",
                            IdeaStage::Archived => "ideas.to.archived",
                            // A promoção é uma operação, nunca uma transição.
                            IdeaStage::Promoted => return None,
                        },
                        requires_note: o
                            .get("requires_note")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        primary: false,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let principal = [
        "exploration",
        "concept",
        "review",
        "project_candidate",
        "discovery",
    ]
    .iter()
    .find(|v| options.iter().any(|o| o.value == **v && !o.requires_note))
    .copied();
    for o in &mut options {
        o.primary = Some(o.value) == principal;
    }
    (!options.is_empty()).then(|| ResTransitionsVm {
        action: format!("/ideas/{id}/transitions"),
        options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn os_grupos_sao_estadios_do_core() {
        assert!(list_path(Group::Developing, 1)
            .contains("idea_state=discovery,exploration,concept,review"));
        assert!(list_path(Group::Promoted, 1).contains("idea_state=promoted"));
        assert!(
            !list_path(Group::Promoted, 1).contains("kind="),
            "promovida já é ambiente de projecto"
        );
        assert!(list_path(Group::Closed, 1).contains("idea_state=rejected,archived"));
        assert!(list_path(Group::Mine, 1).contains("mine=true"));
    }

    #[test]
    fn fechar_exige_motivo_e_promover_nao_e_transicao() {
        let i = json!({ "available_transitions": [
            { "state": "concept", "requires_note": false },
            { "state": "rejected", "requires_note": true },
            { "state": "promoted", "requires_note": false }
        ]});
        let t = transitions(&i, true, "i").unwrap();
        assert_eq!(t.options.len(), 2);
        assert!(t
            .options
            .iter()
            .any(|o| o.value == "rejected" && o.requires_note));
        assert!(t.options.iter().any(|o| o.value == "concept" && o.primary));
        assert!(transitions(&i, false, "i").is_none());
    }

    #[test]
    fn um_estadio_novo_nao_tem_leitura() {
        assert_eq!(stage("incubating"), None);
        assert_eq!(
            stage("project_candidate"),
            Some(IdeaStage::ProjectCandidate)
        );
    }
}
