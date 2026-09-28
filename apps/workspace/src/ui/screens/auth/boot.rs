//! O arranque, como uma pessoa o vê (`GET /boot`). DESIGN_LOCKED.
//!
//! Não é um ecrã de carregamento: diz o que está a arrancar, o que parou e o
//! que a pessoa pode fazer. Nunca inventa progresso.

use leptos::prelude::*;

use super::{frame, Foot};
use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::view_models::{BootState, BootVm, Health};

/// `GET /boot`.
pub fn boot(vm: &BootVm) -> impl IntoView {
    let (state, title, body) = match vm.state {
        BootState::Starting => (
            "starting",
            "auth.boot.starting_title",
            "auth.boot.starting_body",
        ),
        BootState::Ready => ("ready", "auth.boot.ready_title", "auth.boot.ready_body"),
        BootState::Blocked => (
            "blocked",
            "auth.boot.blocked_title",
            "auth.boot.blocked_body",
        ),
    };
    let rows = vm
        .components
        .iter()
        .map(|c| {
            let (h, ic, key) = match c.health {
                Health::Operational => ("ok", "check", "auth.boot.ok"),
                Health::Degraded => ("warn", "warning", "auth.boot.degraded"),
                Health::Unavailable => ("down", "close", "auth.boot.down"),
            };
            view! {
                <li class="oc-boot__row" data-state=h>
                    {icon(ic)}
                    <span class="oc-boot__name">{c.name.clone()}</span>
                    <span class="oc-boot__state">{t(key)}</span>
                    {c.note.clone().map(|n| view! { <span class="oc-boot__note">{n}</span> })}
                </li>
            }
        })
        .collect_view();
    let action = match vm.state {
        BootState::Ready => view! {
            <a class="oc-btn oc-btn--gold" href="/login">{t("auth.boot.continue")}{icon("arrow-r")}</a>
        }
        .into_any(),
        _ => view! {
            <a class="oc-btn oc-btn--line" href="/boot">{icon("refresh")}{t("auth.retry")}</a>
        }
        .into_any(),
    };
    let reference = vm.reference.clone().map(|r| {
        let text = crate::i18n::tf("state.core_error", &[("ref", &r)]);
        view! { <p class="oc-auth__note">{text}</p> }
    });
    let card = view! {
        <div class="oc-boot" data-state=state role="status" aria-live="polite">
            <h1 class="oc-auth__title">{t(title)}</h1>
            <p class="oc-auth__lead">{t(body)}</p>
            {(vm.state == BootState::Starting).then(|| view! { <span class="oc-boot__bar" aria-hidden="true"><span></span></span> })}
            <ul class="oc-boot__list">{rows}</ul>
            {reference}
            {action}
        </div>
    }
    .into_any();
    frame(&vm.door, "boot", false, card, Foot::Language("/login"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{BootComponent, DoorVm};

    fn vm(state: BootState) -> BootVm {
        BootVm {
            door: DoorVm::default(),
            state,
            components: vec![BootComponent {
                name: "Base de dados".into(),
                health: Health::Unavailable,
                note: None,
            }],
            reference: Some("OC-1A2B".into()),
        }
    }

    #[test]
    fn parado_mostra_o_componente_a_referencia_e_tentar_de_novo() {
        let html = boot(&vm(BootState::Blocked)).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"data-state="down""#) && html.contains("OC-1A2B"));
        assert!(html.contains(r#"href="/boot""#));
    }

    #[test]
    fn pronto_leva_ao_login() {
        let html = boot(&vm(BootState::Ready)).to_html();
        assert!(html.contains(r#"href="/login""#));
    }
}
