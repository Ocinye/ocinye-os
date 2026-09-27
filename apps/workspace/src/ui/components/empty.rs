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
use crate::ui::icon::Icon;

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
    let _ = small; // o D1 tem um só tamanho de estado vazio.
    let has_actions = !actions.is_empty();

    view! {
        <div class="ods-empty">
            <span class="ods-empty__icon">
                {crate::ui::ods::icone(crate::ui::ods::icone_do_legado(kind), "ods-icon--lg")}
            </span>
            <p class="ods-empty__title">{title}</p>
            <p class="ods-empty__body">{body}</p>
            {has_actions
                .then(|| {
                    view! {
                        <div class="ods-boot__actions">
                            {actions.into_iter().map(button).collect_view()}
                        </div>
                    }
                })}
        </div>
    }
}
