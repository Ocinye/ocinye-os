//! Calendário nativo: compromissos, lembretes e a agenda que os junta.
//!
//! Ver [ADR-0410](../../../../docs/adrs/0410-temporal-center-and-native-calendar.md).

pub mod delivery;
mod model;
mod repository;
mod service;

pub use model::{CalendarEvent, Notification, Reminder, TaskDeadline, TemporalItem};
pub use repository::TimeRange;
pub use service::{
    agenda, agenda_count, cancel_event, cancel_reminder, create_event, create_reminder,
    dismiss_reminder, get_event, mark_notification_read, notifications, participants_of,
    pending_reminders, snooze_reminder, unread_notifications, update_event, EventEdit, NewEvent,
    NewReminder,
};
