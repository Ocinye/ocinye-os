//! Calendário: agenda, eventos, lembretes e notificações.
//!
//! # O que estes handlers não fazem
//!
//! Autorizar. Cada rota conduz à operação do Core e mais nada: a política vive
//! lá, e escrevê-la outra vez aqui daria duas respostas para a mesma pergunta
//! (ADR-0307). O que estas funções fazem é traduzir HTTP em intenção — e
//! traduzir de volta os erros temporais, para que uma hora que não existe chegue
//! como uma frase e não como um 500.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use ocinye_contracts::calendar::EventScope;
use ocinye_contracts::temporal::{resolve_local, Occurrence, TimeZoneName};
use ocinye_contracts::Classification;
use ocinye_core::modules::calendar::{self, TimeRange};
use ocinye_core::CoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::extract::{CurrentPrincipal, Ids};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/calendar/agenda", get(agenda))
        .route("/calendar/events", post(create_event))
        .route(
            "/calendar/events/{event_id}",
            get(get_event).patch(update_event),
        )
        .route("/calendar/events/{event_id}/cancel", post(cancel_event))
        .route(
            "/calendar/reminders",
            get(list_reminders).post(create_reminder),
        )
        .route("/calendar/reminders/{reminder_id}/snooze", post(snooze))
        .route("/calendar/reminders/{reminder_id}/dismiss", post(dismiss))
        .route(
            "/calendar/reminders/{reminder_id}/cancel",
            post(cancel_reminder),
        )
        .route("/notifications", get(list_notifications))
        .route("/notifications/{notification_id}/read", post(mark_read))
}

// ── Intervalo ───────────────────────────────────────────────────────────

/// Quanto tempo uma consulta pode abranger.
///
/// # Porque existe um tecto
///
/// Porque `1900 → 2500` é uma consulta que ninguém precisa e que qualquer pessoa
/// pode pedir. Um ano chega para o mês, a semana e a agenda; quem precisar de
/// mais pede em pedaços, e a base continua a responder a toda a gente.
const MAX_RANGE_DAYS: i64 = 366;

#[derive(Deserialize)]
struct RangeQuery {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

impl RangeQuery {
    fn validate(self) -> Result<TimeRange, CoreError> {
        if self.to <= self.from {
            return Err(CoreError::Validation(
                "O intervalo termina antes de começar.".to_owned(),
            ));
        }
        if self.to - self.from > Duration::days(MAX_RANGE_DAYS) {
            return Err(CoreError::Validation(format!(
                "Um intervalo não pode abranger mais do que {MAX_RANGE_DAYS} dias."
            )));
        }
        Ok(TimeRange {
            start: self.from,
            end: self.to,
        })
    }
}

// ── Vistas ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct TemporalItemView {
    kind: &'static str,
    id: Uuid,
    title: String,
    all_day: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    starts_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ends_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    starts_on: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ends_before: Option<NaiveDate>,
    state: String,
    classification: String,
}

impl From<calendar::TemporalItem> for TemporalItemView {
    fn from(item: calendar::TemporalItem) -> Self {
        let (all_day, starts_at, ends_at, timezone, starts_on, ends_before) = match item.occurrence
        {
            Occurrence::Timed {
                starts_at,
                ends_at,
                timezone,
            } => (
                false,
                Some(starts_at),
                Some(ends_at),
                Some(timezone.as_str().to_owned()),
                None,
                None,
            ),
            Occurrence::AllDay {
                starts_on,
                ends_before,
            } => (true, None, None, None, Some(starts_on), Some(ends_before)),
        };

        Self {
            kind: item.kind.as_str(),
            id: item.id,
            title: item.title,
            all_day,
            starts_at,
            ends_at,
            timezone,
            starts_on,
            ends_before,
            state: item.state,
            classification: item.classification.as_str().to_owned(),
        }
    }
}

#[derive(Serialize)]
struct AgendaView {
    items: Vec<TemporalItemView>,
    total: i64,
}

#[derive(Serialize)]
struct EventView {
    id: Uuid,
    scope: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    all_day: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    starts_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ends_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    starts_on: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ends_before: Option<NaiveDate>,
    state: String,
    classification: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_id: Option<Uuid>,
}

impl From<calendar::CalendarEvent> for EventView {
    fn from(event: calendar::CalendarEvent) -> Self {
        Self {
            id: event.id,
            scope: event.scope.clone(),
            title: event.title.clone(),
            description: event.description.clone(),
            location: event.location.clone(),
            all_day: event.all_day,
            starts_at: event.starts_at,
            ends_at: event.ends_at,
            timezone: event.timezone.clone(),
            starts_on: event.starts_on,
            ends_before: event.ends_before,
            state: event.state.clone(),
            classification: event.classification.clone(),
            unit_id: event.unit_id,
            workspace_id: event.workspace_id,
        }
    }
}

#[derive(Serialize)]
struct ReminderView {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    trigger_at: DateTime<Utc>,
    state: String,
}

impl From<calendar::Reminder> for ReminderView {
    fn from(reminder: calendar::Reminder) -> Self {
        Self {
            id: reminder.id,
            event_id: reminder.event_id,
            task_id: reminder.task_id,
            note: reminder.note.clone(),
            trigger_at: reminder.trigger_at,
            state: reminder.state.clone(),
        }
    }
}

#[derive(Serialize)]
struct NotificationView {
    id: Uuid,
    kind: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_id: Option<Uuid>,
    read: bool,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct NotificationsView {
    notifications: Vec<NotificationView>,
    unread: i64,
}

// ── Entrada ─────────────────────────────────────────────────────────────

/// Quando um evento acontece, como o cliente o descreve.
///
/// # Porque a hora local e a zona chegam separadas
///
/// Porque o instante canónico é o Core que o calcula. Deixar o cliente enviar um
/// `DateTime<Utc>` já convertido daria a um browser mal configurado — ou a um
/// modelo — o direito de decidir o que significa «14:00 em Paris», e a resposta
/// mudaria com quem pergunta.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OccurrenceInput {
    Timed {
        starts_at: NaiveDateTime,
        ends_at: NaiveDateTime,
        timezone: String,
    },
    AllDay {
        starts_on: NaiveDate,
        ends_before: NaiveDate,
    },
}

impl OccurrenceInput {
    fn resolve(self) -> Result<Occurrence, CoreError> {
        match self {
            Self::Timed {
                starts_at,
                ends_at,
                timezone,
            } => {
                let zone = TimeZoneName::parse(&timezone).map_err(CoreError::Validation)?;
                // Um erro temporal é uma frase, e não um 500. A hora que o
                // relógio salta na mudança para o horário de Verão é um engano
                // humano honesto, e a resposta certa é dizê-lo.
                let inicio = resolve_local(starts_at, zone)
                    .map_err(|erro| CoreError::Validation(erro.to_string()))?;
                let fim = resolve_local(ends_at, zone)
                    .map_err(|erro| CoreError::Validation(erro.to_string()))?;
                Ok(Occurrence::Timed {
                    starts_at: inicio,
                    ends_at: fim,
                    timezone: zone,
                })
            }
            Self::AllDay {
                starts_on,
                ends_before,
            } => Ok(Occurrence::AllDay {
                starts_on,
                ends_before,
            }),
        }
    }
}

#[derive(Deserialize)]
struct CreateEventRequest {
    scope: String,
    #[serde(default)]
    unit_id: Option<Uuid>,
    #[serde(default)]
    workspace_id: Option<Uuid>,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    location: Option<String>,
    occurrence: OccurrenceInput,
    #[serde(default)]
    classification: Option<String>,
    /// Quem participa, por identificador institucional.
    ///
    /// Opcional: uma actividade sem participantes é uma actividade válida, e
    /// exigir a lista tornaria obrigatório escrever `[]` a quem marca sozinho.
    #[serde(default)]
    participants: Option<Vec<Uuid>>,
}

#[derive(Deserialize)]
struct UpdateEventRequest {
    #[serde(default)]
    title: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    location: Option<Option<String>>,
    #[serde(default)]
    occurrence: Option<OccurrenceInput>,
}

/// Distingue «não mexer» de «apagar».
///
/// Sem isto, `null` e ausência liam-se da mesma maneira, e não haveria forma de
/// tirar uma descrição a um evento sem tirar também tudo o que não foi enviado.
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
struct CreateReminderRequest {
    #[serde(default)]
    event_id: Option<Uuid>,
    #[serde(default)]
    task_id: Option<Uuid>,
    #[serde(default)]
    note: Option<String>,
    trigger_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct SnoozeRequest {
    #[serde(default)]
    minutes: Option<i64>,
}

// ── Handlers ────────────────────────────────────────────────────────────

async fn agenda(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Query(range): Query<RangeQuery>,
) -> Result<Json<AgendaView>, ApiError> {
    let range = range.validate().map_err(|e| ApiError::new(e, &ids))?;

    let (items, total) = tokio::try_join!(
        calendar::agenda(&state.pool, &principal, range, 500),
        calendar::agenda_count(&state.pool, &principal, range),
    )
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(AgendaView {
        items: items.into_iter().map(TemporalItemView::from).collect(),
        total,
    }))
}

async fn create_event(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<CreateEventRequest>,
) -> Result<Json<EventView>, ApiError> {
    let scope = EventScope::parse(&request.scope).ok_or_else(|| {
        ApiError::new(
            CoreError::Validation("Âmbito desconhecido.".to_owned()),
            &ids,
        )
    })?;
    let occurrence = request
        .occurrence
        .resolve()
        .map_err(|e| ApiError::new(e, &ids))?;
    let classification = request
        .classification
        .as_deref()
        .and_then(Classification::parse);

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let event = calendar::create_event(
        &mut tx,
        &principal,
        &ids,
        calendar::NewEvent {
            scope,
            unit_id: request.unit_id,
            workspace_id: request.workspace_id,
            title: request.title,
            description: request.description,
            location: request.location,
            occurrence,
            classification,
            participants: request.participants.clone().unwrap_or_default(),
        },
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(EventView::from(event)))
}

async fn get_event(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(event_id): Path<Uuid>,
) -> Result<Json<EventView>, ApiError> {
    let event = calendar::get_event(&state.pool, &principal, event_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(Json(EventView::from(event)))
}

async fn update_event(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(event_id): Path<Uuid>,
    Json(request): Json<UpdateEventRequest>,
) -> Result<Json<EventView>, ApiError> {
    let occurrence = request
        .occurrence
        .map(OccurrenceInput::resolve)
        .transpose()
        .map_err(|e| ApiError::new(e, &ids))?;

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let event = calendar::update_event(
        &mut tx,
        &principal,
        &ids,
        event_id,
        calendar::EventEdit {
            title: request.title,
            description: request.description,
            location: request.location,
            occurrence,
        },
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;
    tx.commit().await.map_err(CoreError::from)?;

    Ok(Json(EventView::from(event)))
}

async fn cancel_event(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(event_id): Path<Uuid>,
) -> Result<Json<EventView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let event = calendar::cancel_event(&mut tx, &principal, &ids, event_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(EventView::from(event)))
}

async fn list_reminders(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
) -> Result<Json<Vec<ReminderView>>, ApiError> {
    let reminders = calendar::pending_reminders(&state.pool, &principal, 100)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(Json(
        reminders.into_iter().map(ReminderView::from).collect(),
    ))
}

async fn create_reminder(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Json(request): Json<CreateReminderRequest>,
) -> Result<Json<ReminderView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let reminder = calendar::create_reminder(
        &mut tx,
        &principal,
        &ids,
        calendar::NewReminder {
            event_id: request.event_id,
            task_id: request.task_id,
            note: request.note,
            trigger_at: request.trigger_at,
        },
    )
    .await
    .map_err(|error| ApiError::new(error, &ids))?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(ReminderView::from(reminder)))
}

async fn snooze(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(reminder_id): Path<Uuid>,
    Json(request): Json<SnoozeRequest>,
) -> Result<Json<ReminderView>, ApiError> {
    let minutes = request.minutes.unwrap_or(10).clamp(1, 60 * 24 * 7);
    let until = Utc::now() + Duration::minutes(minutes);

    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let reminder = calendar::snooze_reminder(&mut tx, &principal, &ids, reminder_id, until)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(ReminderView::from(reminder)))
}

async fn dismiss(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(reminder_id): Path<Uuid>,
) -> Result<Json<ReminderView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let reminder = calendar::dismiss_reminder(&mut tx, &principal, &ids, reminder_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(ReminderView::from(reminder)))
}

async fn cancel_reminder(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(reminder_id): Path<Uuid>,
) -> Result<Json<ReminderView>, ApiError> {
    let mut tx = state.pool.begin().await.map_err(CoreError::from)?;
    let reminder = calendar::cancel_reminder(&mut tx, &principal, &ids, reminder_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    tx.commit().await.map_err(CoreError::from)?;
    Ok(Json(ReminderView::from(reminder)))
}

async fn list_notifications(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
) -> Result<Json<NotificationsView>, ApiError> {
    let (notifications, unread) = tokio::try_join!(
        calendar::notifications(&state.pool, &principal, 50),
        calendar::unread_notifications(&state.pool, &principal),
    )
    .map_err(|error| ApiError::new(error, &ids))?;

    Ok(Json(NotificationsView {
        notifications: notifications
            .into_iter()
            .map(|n| NotificationView {
                id: n.id,
                kind: n.kind,
                title: n.title,
                body: n.body,
                resource_type: n.resource_type,
                resource_id: n.resource_id,
                read: n.read_at.is_some(),
                created_at: n.created_at,
            })
            .collect(),
        unread,
    }))
}

async fn mark_read(
    State(state): State<AppState>,
    CurrentPrincipal(principal): CurrentPrincipal,
    Ids(ids): Ids,
    Path(notification_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    calendar::mark_notification_read(&state.pool, &principal, notification_id)
        .await
        .map_err(|error| ApiError::new(error, &ids))?;
    Ok(Json(serde_json::json!({ "read": true })))
}
