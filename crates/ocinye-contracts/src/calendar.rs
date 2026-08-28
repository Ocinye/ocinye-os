//! Compromissos institucionais: eventos, lembretes e a linha do tempo que os
//! junta.
//!
//! # O que aqui não está
//!
//! Prazos de tarefas. Uma `Task` com `due_on` **aparece** no calendário e não é
//! copiada para ele: continua a pertencer a Collaboration, e o que o calendário
//! devolve é uma projecção. Duplicá-la criaria duas datas para o mesmo prazo, e
//! uma delas ficaria errada sem ninguém saber qual (ADR-0410).

use serde::{Deserialize, Serialize};

/// Em que estado está um evento.
///
/// # Porque não existe «Concluído»
///
/// Um evento que já passou não precisa de ser marcado como concluído: a data
/// diz-o, e é exacta. Um estado `Completed` obrigaria alguém — ou algum worker —
/// a percorrer o passado a carimbar reuniões, e a única informação que isso
/// acrescentaria já estava no relógio.
///
/// Cancelar, ao contrário, é informação que **só uma pessoa tem**: a reunião não
/// vai acontecer, e quem a esperava precisa de saber. Por isso é estado, e por
/// isso o evento cancelado não desaparece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarEventState {
    /// Marcado, e a contar.
    Scheduled,
    /// Não vai acontecer. Continua visível para quem o esperava.
    Cancelled,
}

impl CalendarEventState {
    /// Representação estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Cancelled => "cancelled",
        }
    }

    /// Interpreta a representação estável.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "scheduled" => Self::Scheduled,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }

    /// Como se diz a uma pessoa.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Scheduled => "Marcado",
            Self::Cancelled => "Cancelado",
        }
    }
}

/// A quem um evento pertence.
///
/// # Não é uma árvore de autorização nova
///
/// Três destes âmbitos resolvem-se pela política que já existe: pertencer à
/// unidade, pertencer ao workspace, pertencer à instituição. O quarto —
/// `Personal` — não tem contentor nenhum, e é por isso que precisa de ser
/// nomeado: um evento pessoal alcança-se **por ser de quem é**, e por mais nada.
/// Sem esta distinção, um evento sem unidade seria indistinguível de um evento
/// institucional, que é exactamente o erro que deixaria a agenda de uma pessoa
/// à vista da instituição inteira.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventScope {
    /// Do próprio membro, e de mais ninguém.
    Personal,
    /// De uma unidade científica.
    Unit,
    /// De um Research Workspace.
    ResearchWorkspace,
    /// Da instituição.
    Institution,
}

impl EventScope {
    /// Representação estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Unit => "unit",
            Self::ResearchWorkspace => "research_workspace",
            Self::Institution => "institution",
        }
    }

    /// Interpreta a representação estável.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "personal" => Self::Personal,
            "unit" => Self::Unit,
            "research_workspace" => Self::ResearchWorkspace,
            "institution" => Self::Institution,
            _ => return None,
        })
    }

    /// Como se diz a uma pessoa.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Personal => "Pessoal",
            Self::Unit => "Unidade",
            Self::ResearchWorkspace => "Research Workspace",
            Self::Institution => "Instituição",
        }
    }

    /// Se este âmbito exige um contentor identificado.
    ///
    /// `Personal` e `Institution` não têm contentor: o primeiro é da pessoa, o
    /// segundo é de toda a gente. Os outros dois sem contentor seriam um evento
    /// «de uma unidade» que não diz qual, e a autorização não teria contra o que
    /// decidir.
    #[must_use]
    pub const fn needs_container(self) -> bool {
        matches!(self, Self::Unit | Self::ResearchWorkspace)
    }
}

/// Em que estado está um lembrete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderState {
    /// À espera da hora.
    Scheduled,
    /// Entregue: existe uma notificação por ele.
    Delivered,
    /// Adiado para mais tarde.
    Snoozed,
    /// A pessoa disse que já viu.
    Dismissed,
    /// Deixou de fazer sentido — o recurso foi cancelado, ou o dono desistiu.
    Cancelled,
}

impl ReminderState {
    /// Representação estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Delivered => "delivered",
            Self::Snoozed => "snoozed",
            Self::Dismissed => "dismissed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Interpreta a representação estável.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "scheduled" => Self::Scheduled,
            "delivered" => Self::Delivered,
            "snoozed" => Self::Snoozed,
            "dismissed" => Self::Dismissed,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }

    /// Como se diz a uma pessoa.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Scheduled => "Agendado",
            Self::Delivered => "Entregue",
            Self::Snoozed => "Adiado",
            Self::Dismissed => "Dispensado",
            Self::Cancelled => "Cancelado",
        }
    }

    /// Se este estado ainda espera pela hora.
    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Scheduled | Self::Snoozed)
    }
}

/// O que uma linha da agenda é.
///
/// # Porque isto tem de existir
///
/// A agenda mistura três coisas com origens diferentes: um evento de
/// calendário, o prazo de uma tarefa, e um lembrete. Devolvê-las todas como
/// «evento» faria a interface oferecer *cancelar* sobre um prazo de tarefa, e a
/// operação por trás não existe — a tarefa altera-se pelo seu próprio módulo.
///
/// Este tipo é o que impede a interface de assumir que tudo o que tem data é a
/// mesma coisa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalItemKind {
    /// Um evento do calendário.
    Event,
    /// O prazo de uma tarefa, projectado. Vive em Collaboration.
    TaskDue,
    /// Um lembrete pendente.
    Reminder,
}

impl TemporalItemKind {
    /// Representação estável.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::TaskDue => "task_due",
            Self::Reminder => "reminder",
        }
    }

    /// Como se diz a uma pessoa.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Event => "Evento",
            Self::TaskDue => "Prazo",
            Self::Reminder => "Lembrete",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_estados_sobrevivem_a_ida_e_volta() {
        for estado in [CalendarEventState::Scheduled, CalendarEventState::Cancelled] {
            assert_eq!(CalendarEventState::parse(estado.as_str()), Some(estado));
        }
        for estado in [
            ReminderState::Scheduled,
            ReminderState::Delivered,
            ReminderState::Snoozed,
            ReminderState::Dismissed,
            ReminderState::Cancelled,
        ] {
            assert_eq!(ReminderState::parse(estado.as_str()), Some(estado));
        }
        for ambito in [
            EventScope::Personal,
            EventScope::Unit,
            EventScope::ResearchWorkspace,
            EventScope::Institution,
        ] {
            assert_eq!(EventScope::parse(ambito.as_str()), Some(ambito));
        }
    }

    /// Só os âmbitos com contentor exigem contentor.
    ///
    /// Se `Personal` passasse a exigir um, a agenda pessoal deixava de poder
    /// existir; se `Unit` deixasse de exigir, um evento diria pertencer a uma
    /// unidade sem dizer qual, e a autorização não teria contra o que decidir.
    #[test]
    fn o_contentor_e_exigido_onde_ha_contentor() {
        assert!(!EventScope::Personal.needs_container());
        assert!(!EventScope::Institution.needs_container());
        assert!(EventScope::Unit.needs_container());
        assert!(EventScope::ResearchWorkspace.needs_container());
    }

    #[test]
    fn um_lembrete_entregue_deixa_de_esperar() {
        assert!(ReminderState::Scheduled.is_pending());
        assert!(ReminderState::Snoozed.is_pending());
        assert!(!ReminderState::Delivered.is_pending());
        assert!(!ReminderState::Dismissed.is_pending());
        assert!(!ReminderState::Cancelled.is_pending());
    }
}
