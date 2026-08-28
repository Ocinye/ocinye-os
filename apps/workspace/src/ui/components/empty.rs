//! Estados vazios.
//!
//! Sem ilustrações decorativas: tile técnico, título, explicação e no máximo
//! duas acções (`design/README.md` §7.5).
//!
//! Estes ecrãs importam mais do que o habitual neste sistema: a infraestrutura
//! de IA e de computação **não existe**, e o estado vazio é a forma honesta de
//! o dizer — não um espaço reservado à espera de dados inventados.

use leptos::prelude::*;

use super::button::{button, Button};
use crate::ui::icon::{icon, Icon};

/// Um estado vazio.
pub struct EmptyState {
    /// Ícone do tile.
    pub icon: Icon,
    /// Título.
    pub title: String,
    /// Explicação. Até ~430px de largura.
    pub body: String,
    /// Até duas acções.
    pub actions: Vec<Button>,
    /// Tile pequeno (58px) em vez do grande (78px).
    pub small: bool,
}

/// Renderiza um estado vazio.
pub fn empty_state(state: EmptyState) -> impl IntoView {
    let EmptyState {
        icon: kind,
        title,
        body,
        actions,
        small,
    } = state;
    let tile_class = if small {
        "oc-empty__tile oc-empty__tile--sm"
    } else {
        "oc-empty__tile"
    };
    let icon_size = if small { 26 } else { 34 };
    let has_actions = !actions.is_empty();

    view! {
        <div class="oc-empty">
            <div class=tile_class>{icon(kind, icon_size)}</div>
            <h3>{title}</h3>
            <p>{body}</p>
            {has_actions
                .then(|| {
                    view! {
                        <div class="oc-empty__actions">
                            {actions.into_iter().map(button).collect_view()}
                        </div>
                    }
                })}
        </div>
    }
}
