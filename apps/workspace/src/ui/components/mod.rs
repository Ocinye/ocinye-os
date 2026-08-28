//! Componentes partilhados.
//!
//! Todos os ecrãs usam estes: nenhum ecrã redefine uma tabela, um badge ou um
//! botão. Se um ecrã precisar de uma variante, a variante entra aqui.

pub mod assist;
pub mod avatar;
mod badge;
pub mod button;
pub mod card;
pub mod empty;
pub mod field;
pub mod progress;
pub mod table;
pub mod tabs;

pub use assist::{assist, Assist, IDEA_SUGGESTIONS, KNOWLEDGE_SUGGESTIONS, PROJECT_SUGGESTIONS};
pub use avatar::{avatar, AvatarSize};
pub use badge::{badge, classification_badge, pill, Tone};
pub use button::{button, Button, Variant};
pub use card::{card, kpi_card, section_head, Kpi};
pub use empty::{empty_state, EmptyState};
pub use field::{
    field as text_field, field_with_value, named_checkbox, radio_group, select, select_labelled,
    textarea, textarea_with_value, RadioOption, SelectOption,
};
pub use progress::{donut, progress_bar};
pub use table::{data_table, Cell, Column, ListTab, Table};
pub use tabs::{context_tabs, pill_tabs, Tab};
