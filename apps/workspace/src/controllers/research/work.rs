//! D005 · O Meu Trabalho (tarefas). A aplicação é `ApplicationId::Work`, em
//! `/my-work`: não há outra «Tarefas». Concluir e reabrir são transições do
//! Core; nada muda na página sem a resposta dele.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde_json::Value;

use super::{nav, state};
use crate::controllers::desktop::{text, Clock};
use crate::ui::view_models::{
    AppNavVm, ResItemVm, ResStateVm, ResTone, ResTransitionVm, ResTransitionsVm, TaskPriorityLevel,
    TaskStatus,
};

/// Os filtros da lista, com o significado do Core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// «Atribuídas a mim»: `mine=true` (a pessoa atribuída é quem pergunta), em aberto.
    Mine,
    /// «Em aberto»: todas as que vejo que não estão concluídas nem canceladas.
    Open,
    /// «Todas as que vejo»: `open_only=false`.
    All,
}

/// O filtro pedido; por omissão, as minhas.
#[must_use]
pub fn filter_of(v: Option<&str>) -> Filter {
    match v {
        Some("open") => Filter::Open,
        Some("all") => Filter::All,
        _ => Filter::Mine,
    }
}

const fn param(f: Filter) -> &'static str {
    match f {
        Filter::Mine => "mine",
        Filter::Open => "open",
        Filter::All => "all",
    }
}

/// O pedido ao Core.
#[must_use]
pub fn list_path(f: Filter, workspace: Option<&str>, page: u32) -> String {
    let recorte = match f {
        Filter::Mine => "mine=true&open_only=true",
        Filter::Open => "open_only=true",
        Filter::All => "open_only=false",
    };
    let ambiente = workspace
        .filter(|w| uuid::Uuid::parse_str(w).is_ok())
        .map(|w| format!("&workspace_id={w}"))
        .unwrap_or_default();
    format!("/api/v1/tasks?{recorte}{ambiente}&page={page}&page_size=25")
}

/// A lista com os filtros correntes.
#[must_use]
pub fn list_href(f: Filter, workspace: Option<&str>) -> String {
    let w = workspace
        .filter(|w| uuid::Uuid::parse_str(w).is_ok())
        .map(|w| format!("&workspace={w}"))
        .unwrap_or_default();
    format!("/my-work?nav={}{w}", param(f))
}

/// A navegação.
#[must_use]
pub fn navigation(f: Filter, workspace: Option<&str>) -> Vec<AppNavVm> {
    [
        (Filter::Mine, "work.nav.mine", "user"),
        (Filter::Open, "work.nav.open", "tasks"),
        (Filter::All, "work.nav.all", "list"),
    ]
    .into_iter()
    .map(|(x, k, i)| nav(k, i, list_href(x, workspace), x == f))
    .collect()
}

/// O estado do Core, um a um.
#[must_use]
pub fn status(v: &str) -> Option<TaskStatus> {
    Some(match v {
        "todo" => TaskStatus::Todo,
        "in_progress" => TaskStatus::InProgress,
        "blocked" => TaskStatus::Blocked,
        "in_review" => TaskStatus::InReview,
        "done" => TaskStatus::Done,
        "cancelled" => TaskStatus::Cancelled,
        _ => return None,
    })
}

/// O rótulo e o tom.
#[must_use]
pub const fn state_vm(s: TaskStatus) -> ResStateVm {
    match s {
        TaskStatus::Todo => state("work.state.todo", ResTone::Neutral),
        TaskStatus::InProgress => state("work.state.in_progress", ResTone::Progress),
        TaskStatus::Blocked => state("work.state.blocked", ResTone::Attention),
        TaskStatus::InReview => state("work.state.in_review", ResTone::Attention),
        TaskStatus::Done => state("work.state.done", ResTone::Done),
        TaskStatus::Cancelled => state("work.state.cancelled", ResTone::Closed),
    }
}

/// A prioridade do Core, uma a uma.
#[must_use]
pub fn priority(v: &str) -> Option<TaskPriorityLevel> {
    Some(match v {
        "low" => TaskPriorityLevel::Low,
        "normal" => TaskPriorityLevel::Normal,
        "high" => TaskPriorityLevel::High,
        "critical" => TaskPriorityLevel::Critical,
        _ => return None,
    })
}

/// O valor enviado ao Core.
#[must_use]
pub const fn priority_str(p: TaskPriorityLevel) -> &'static str {
    match p {
        TaskPriorityLevel::Low => "low",
        TaskPriorityLevel::Normal => "normal",
        TaskPriorityLevel::High => "high",
        TaskPriorityLevel::Critical => "critical",
    }
}

/// Uma tarefa está vencida quando o prazo (uma **data**) já passou no fuso do
/// membro — o dia do prazo ainda não é atraso — e ela continua por fechar:
/// concluída ou cancelada nunca está vencida.
#[must_use]
pub(crate) fn overdue(due_on: Option<NaiveDate>, s: Option<TaskStatus>, clock: &Clock) -> bool {
    let hoje = clock.now.with_timezone(&clock.zone.zone()).date_naive();
    let aberta = !matches!(s, Some(TaskStatus::Done | TaskStatus::Cancelled));
    aberta && due_on.is_some_and(|d| d < hoje)
}

fn due_of(t: &Value) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text(t, "due_on"), "%Y-%m-%d").ok()
}

/// As colunas.
pub const COLUMNS: [&str; 3] = ["work.col.due", "work.col.workspace", "work.col.assignee"];

/// As linhas. Os nomes das pessoas e dos ambientes vêm de leituras autorizadas
/// (as pessoas dos ambientes desta página, os ambientes que o membro vê); o
/// que não se resolveu fica em branco, nunca um identificador.
#[must_use]
pub(crate) fn items(
    list: &[Value],
    open: Option<&str>,
    workspaces: &HashMap<String, String>,
    people: &HashMap<String, String>,
    clock: &Clock,
) -> Vec<ResItemVm> {
    list.iter()
        .filter_map(|t| {
            let id = text(t, "id");
            if id.is_empty() {
                return None;
            }
            let s = status(text(t, "state"));
            let due = due_of(t);
            Some(ResItemVm {
                title: text(t, "title").to_owned(),
                code: None,
                state: s.map(state_vm),
                cells: vec![
                    due.map(|d| d.format("%d/%m/%Y").to_string()),
                    workspaces.get(text(t, "workspace_id")).cloned(),
                    people.get(text(t, "assignee_id")).cloned(),
                ],
                overdue: overdue(due, s, clock),
                priority: priority(text(t, "priority")),
                href: format!("/my-work/{id}"),
                active: open == Some(id),
            })
        })
        .collect()
}

/// O rótulo de uma transição, pelo par (de, para): reabrir, retomar e voltar
/// a «por fazer» dizem-se de outra maneira.
#[must_use]
pub fn transition_label(from: &str, to: &str) -> Option<&'static str> {
    Some(match (from, to) {
        ("done", "in_progress") => "work.to.reopen",
        ("blocked", "in_progress") => "work.to.resume",
        ("cancelled", "todo") => "work.to.todo",
        (_, "in_progress") => "work.to.in_progress",
        (_, "blocked") => "work.to.blocked",
        (_, "in_review") => "work.to.in_review",
        (_, "done") => "work.to.done",
        (_, "cancelled") => "work.to.cancelled",
        (_, "todo") => "work.to.todo",
        _ => return None,
    })
}

fn state_str(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Todo => "todo",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Blocked => "blocked",
        TaskStatus::InReview => "in_review",
        TaskStatus::Done => "done",
        TaskStatus::Cancelled => "cancelled",
    }
}

/// As transições que o Core devolveu (`available_transitions`); a principal é
/// concluir, ou, fechada, reabrir.
#[must_use]
pub fn transitions(task: &Value, id: &str) -> Option<ResTransitionsVm> {
    let from = text(task, "state");
    let mut options: Vec<ResTransitionVm> = task
        .get("available_transitions")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter_map(|to| {
                    Some(ResTransitionVm {
                        value: status(to).map(state_str)?,
                        label_key: transition_label(from, to)?,
                        requires_note: false,
                        primary: false,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let principal = ["done", "in_progress", "todo"]
        .iter()
        .find(|v| options.iter().any(|o| o.value == **v))
        .copied();
    for o in &mut options {
        o.primary = Some(o.value) == principal;
    }
    (!options.is_empty()).then(|| ResTransitionsVm {
        action: format!("/my-work/{id}/transitions"),
        options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn relogio(utc: chrono::DateTime<chrono::Utc>) -> Clock {
        Clock {
            now: utc,
            zone: ocinye_contracts::temporal::TimeZoneName::parse("Africa/Luanda").unwrap(),
            core_ok: true,
            is_admin: false,
        }
    }

    #[test]
    fn vencida_e_no_fuso_do_membro_e_nunca_quando_fechada() {
        // 30/09 às 23:30 UTC já é 1/10 em Luanda (UTC+1).
        let c = relogio(
            chrono::Utc
                .with_ymd_and_hms(2026, 9, 30, 23, 30, 0)
                .unwrap(),
        );
        let d30 = NaiveDate::from_ymd_opt(2026, 9, 30);
        let d1 = NaiveDate::from_ymd_opt(2026, 10, 1);
        assert!(
            overdue(d30, Some(TaskStatus::Todo), &c),
            "30/09 já passou em Luanda"
        );
        assert!(
            !overdue(d1, Some(TaskStatus::Todo), &c),
            "o dia do prazo não é atraso"
        );
        assert!(!overdue(d30, Some(TaskStatus::Done), &c));
        assert!(!overdue(d30, Some(TaskStatus::Cancelled), &c));
        assert!(!overdue(None, Some(TaskStatus::Todo), &c));
        // Em UTC ainda seria 30/09: um teste só em UTC não veria a diferença.
        let utc = Clock {
            zone: ocinye_contracts::temporal::TimeZoneName::parse("UTC").unwrap(),
            ..relogio(
                chrono::Utc
                    .with_ymd_and_hms(2026, 9, 30, 23, 30, 0)
                    .unwrap(),
            )
        };
        assert!(!overdue(d30, Some(TaskStatus::Todo), &utc));
    }

    #[test]
    fn os_filtros_tem_o_significado_do_core() {
        assert!(list_path(Filter::Mine, None, 1).contains("mine=true&open_only=true"));
        let aberto = list_path(Filter::Open, None, 1);
        assert!(aberto.contains("open_only=true") && !aberto.contains("mine"));
        assert!(list_path(Filter::All, None, 1).contains("open_only=false"));
        // Um ambiente que não é um identificador não entra no pedido.
        assert!(!list_path(Filter::All, Some("x' OR 1=1"), 1).contains("workspace_id"));
    }

    #[test]
    fn concluir_e_reabrir_sao_transicoes_com_o_rotulo_certo() {
        let aberta = json!({ "state": "in_progress", "available_transitions": ["blocked", "in_review", "done", "cancelled"] });
        let t = transitions(&aberta, "t").unwrap();
        assert_eq!(t.action, "/my-work/t/transitions");
        let p: Vec<_> = t
            .options
            .iter()
            .filter(|o| o.primary)
            .map(|o| o.label_key)
            .collect();
        assert_eq!(p, ["work.to.done"]);
        let feita = json!({ "state": "done", "available_transitions": ["in_progress"] });
        let t = transitions(&feita, "t").unwrap();
        assert_eq!(t.options[0].label_key, "work.to.reopen");
        assert!(t.options[0].primary);
        assert_eq!(
            transition_label("blocked", "in_progress"),
            Some("work.to.resume")
        );
    }

    #[test]
    fn prioridade_e_estado_um_a_um() {
        assert_eq!(priority("critical"), Some(TaskPriorityLevel::Critical));
        assert_eq!(priority("urgent"), None);
        assert_eq!(status("archived"), None);
    }
}
