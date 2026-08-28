//! O que o calendário guarda, como sai da base.

use chrono::{DateTime, NaiveDate, Utc};
use ocinye_contracts::calendar::{CalendarEventState, EventScope, ReminderState, TemporalItemKind};
use ocinye_contracts::temporal::{Occurrence, TimeZoneName};
use ocinye_contracts::Classification;
use sqlx::FromRow;
use uuid::Uuid;

/// Um compromisso.
#[derive(Debug, Clone, FromRow)]
pub struct CalendarEvent {
    /// Identificador.
    pub id: Uuid,
    /// Organização a que pertence.
    pub organisation_id: Uuid,
    /// Âmbito, como texto estável na base.
    pub scope: String,
    /// Dono individual. Só em `personal`.
    pub owner_id: Option<Uuid>,
    /// Unidade, quando o âmbito a exige.
    pub unit_id: Option<Uuid>,
    /// Workspace, quando o âmbito o exige.
    pub workspace_id: Option<Uuid>,
    /// Título.
    pub title: String,
    /// Descrição.
    pub description: Option<String>,
    /// Onde, em texto livre.
    pub location: Option<String>,
    /// Se é de dia inteiro.
    pub all_day: bool,
    /// Instante inicial, quando tem hora.
    pub starts_at: Option<DateTime<Utc>>,
    /// Instante final, quando tem hora.
    pub ends_at: Option<DateTime<Utc>>,
    /// Zona da intenção, quando tem hora.
    pub timezone: Option<String>,
    /// Primeiro dia, quando é de dia inteiro.
    pub starts_on: Option<NaiveDate>,
    /// Dia a seguir ao último, exclusivo.
    pub ends_before: Option<NaiveDate>,
    /// Estado.
    pub state: String,
    /// Classificação.
    pub classification: String,
    /// Quem criou.
    pub created_by_id: Option<Uuid>,
    /// Quem alterou por último.
    pub updated_by_id: Option<Uuid>,
    /// Quando nasceu.
    pub created_at: DateTime<Utc>,
    /// Quando mudou.
    pub updated_at: DateTime<Utc>,
}

impl CalendarEvent {
    /// Quando acontece, como tipo e não como colunas soltas.
    ///
    /// # Panics
    ///
    /// Nunca em dados que a base aceitou: `ck_calendar_events_occurrence`
    /// garante que uma das duas formas está completa. Um pânico aqui significa
    /// que alguém escreveu na tabela por fora da restrição.
    #[must_use]
    pub fn occurrence(&self) -> Occurrence {
        if self.all_day {
            Occurrence::AllDay {
                starts_on: self.starts_on.expect("dia inteiro tem primeiro dia"),
                ends_before: self.ends_before.expect("dia inteiro tem fim exclusivo"),
            }
        } else {
            Occurrence::Timed {
                starts_at: self.starts_at.expect("com hora tem início"),
                ends_at: self.ends_at.expect("com hora tem fim"),
                timezone: self
                    .timezone
                    .as_deref()
                    .and_then(|zona| TimeZoneName::parse(zona).ok())
                    .unwrap_or_else(TimeZoneName::utc),
            }
        }
    }

    /// O âmbito, interpretado.
    #[must_use]
    pub fn scope(&self) -> EventScope {
        EventScope::parse(&self.scope).unwrap_or(EventScope::Personal)
    }

    /// O estado, interpretado.
    #[must_use]
    pub fn state(&self) -> CalendarEventState {
        CalendarEventState::parse(&self.state).unwrap_or(CalendarEventState::Scheduled)
    }

    /// A classificação, interpretada.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Internal)
    }
}

/// Um lembrete.
#[derive(Debug, Clone, FromRow)]
pub struct Reminder {
    /// Identificador.
    pub id: Uuid,
    /// Organização.
    pub organisation_id: Uuid,
    /// De quem é.
    pub owner_id: Uuid,
    /// Evento a que se refere, se algum.
    pub event_id: Option<Uuid>,
    /// Tarefa a que se refere, se alguma.
    pub task_id: Option<Uuid>,
    /// O que dizer.
    pub note: Option<String>,
    /// Quando dispara.
    pub trigger_at: DateTime<Utc>,
    /// Estado.
    pub state: String,
    /// Quantas vezes o mecanismo tentou.
    pub attempts: i32,
    /// Quando nasceu.
    pub created_at: DateTime<Utc>,
    /// Quando mudou.
    pub updated_at: DateTime<Utc>,
}

impl Reminder {
    /// O estado, interpretado.
    #[must_use]
    pub fn state(&self) -> ReminderState {
        ReminderState::parse(&self.state).unwrap_or(ReminderState::Scheduled)
    }
}

/// Uma linha da agenda, venha ela de onde vier.
///
/// # Porque não é tudo `CalendarEvent`
///
/// A agenda mistura três origens. Devolvê-las como uma só faria a interface
/// oferecer «cancelar» sobre o prazo de uma tarefa — e a operação por trás não
/// existe, porque a tarefa altera-se pelo seu próprio módulo (ADR-0410).
#[derive(Debug, Clone)]
pub struct TemporalItem {
    /// Que espécie de coisa é.
    pub kind: TemporalItemKind,
    /// Identificador do recurso de origem.
    pub id: Uuid,
    /// O que mostrar.
    pub title: String,
    /// Quando acontece.
    pub occurrence: Occurrence,
    /// Estado, como texto do domínio de origem.
    pub state: String,
    /// Classificação do recurso de origem.
    pub classification: Classification,
    /// Workspace de origem, quando existe.
    pub workspace_id: Option<Uuid>,
    /// Unidade de origem, quando existe.
    pub unit_id: Option<Uuid>,
}

/// O prazo de uma tarefa, como o calendário o vê.
///
/// Não é um evento, e o tipo diz isso. Devolver prazos como `CalendarEvent`
/// faria a interface oferecer «cancelar» sobre eles — e a operação por trás não
/// existe, porque a tarefa altera-se pelo seu próprio módulo.
#[derive(Debug, Clone, FromRow)]
pub struct TaskDeadline {
    /// A tarefa.
    pub id: Uuid,
    /// O que ela diz.
    pub title: String,
    /// Quando vence.
    pub due_on: NaiveDate,
    /// Em que estado está.
    pub state: String,
    /// Classificação da tarefa.
    pub classification: String,
    /// Workspace a que pertence.
    pub workspace_id: Uuid,
    /// Unidade a que pertence.
    pub unit_id: Uuid,
}

/// Uma notificação por ler.
#[derive(Debug, Clone, FromRow)]
pub struct Notification {
    /// Identificador.
    pub id: Uuid,
    /// A quem se destina.
    pub recipient_id: Uuid,
    /// Que espécie.
    pub kind: String,
    /// Título.
    pub title: String,
    /// Corpo curto.
    pub body: Option<String>,
    /// Tipo do recurso para onde leva.
    pub resource_type: Option<String>,
    /// Identificador do recurso para onde leva.
    pub resource_id: Option<Uuid>,
    /// Quando foi lida.
    pub read_at: Option<DateTime<Utc>>,
    /// Quando nasceu.
    pub created_at: DateTime<Utc>,
}
