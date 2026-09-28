//! Autenticação e arranque (D7–D14). DESIGN_LOCKED.
//!
//! Todos os ecrãs partilham a mesma moldura: barra com o estado da Instância e
//! o relógio, identidade à porta (logótipo, «OCINYE OS», distribuição), cartão
//! de vidro e rodapé com «Acesso seguro…» e o idioma.

use leptos::prelude::*;

use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::view_models::{Distribution, DoorVm, Health};

pub mod boot;
pub mod first_access;
pub mod identity;
pub mod login;
pub mod mfa;

/// O que o rodapé do cartão mostra à direita.
#[derive(Clone, Copy)]
pub(crate) enum Foot {
    /// O seletor de idioma (`POST /login/language`). `return_to` só aceita
    /// `/login` e `/password/recover` (lista fechada no servidor).
    Language(&'static str),
    /// «Terminar sessão» (`POST /logout`), nos fluxos com sessão restrita.
    SignOut,
}

/// A barra de cima: estado da Instância (se sondado) e relógio.
pub(crate) fn bar(core: Option<Health>) -> impl IntoView {
    view! {
        <header class="oc-auth__bar">
            {core.map(|h| {
                let (state, key) = match h {
                    Health::Operational => ("ok", "auth.instance.operational"),
                    Health::Degraded => ("warn", "auth.instance.degraded"),
                    Health::Unavailable => ("down", "auth.instance.unavailable"),
                };
                view! {
                    <span class="oc-auth__core" data-state=state>
                        <span class="oc-auth__dot" aria-hidden="true"></span>
                        {t(key)}
                    </span>
                }
            })}
            <span class="oc-auth__clock" data-oc="clock"></span>
        </header>
    }
}

fn distribution_key(d: Distribution) -> &'static str {
    match d {
        Distribution::Research => "dist.research",
        Distribution::Business => "dist.business",
        Distribution::Personal => "dist.personal",
        Distribution::Education => "dist.education",
    }
}

/// A identidade à porta: logótipo, «OCINYE OS» e a distribuição (código + nome).
/// Sem nome da Instância nem endereço (decisão do Fidel, D7 v3).
pub(crate) fn identity(door: &DoorVm) -> impl IntoView {
    let dist = door.distribution.map(|d| {
        let name = t(distribution_key(d));
        let code: String = name.chars().take(2).collect();
        view! {
            <p class="oc-auth__dist" data-distribution=d.as_str() title=t("auth.distribution")>
                <span class="oc-auth__dist-code" aria-hidden="true">{code}</span>
                <span class="oc-sr">{t("auth.distribution")}" "</span>
                {name}
            </p>
        }
    });
    view! {
        <span class="oc-auth__mark"><img src="/static/ocinye-logo.png" alt="" width="68" height="68" /></span>
        <p class="oc-auth__product">{t("auth.product")}</p>
        {dist}
    }
}

/// O rodapé do cartão.
pub(crate) fn foot(kind: Foot) -> impl IntoView {
    let right = match kind {
        Foot::Language(return_to) => {
            let actual = crate::i18n::current().as_str();
            view! {
                <form class="oc-auth__langs" method="post" action="/login/language" aria-label=t("auth.language")>
                    <input type="hidden" name="return_to" value=return_to />
                    {["pt", "en", "fr"]
                        .into_iter()
                        .map(|code| {
                            let on = code == actual;
                            view! {
                                <button
                                    type="submit"
                                    class="oc-auth__lang"
                                    name="lang"
                                    value=code
                                    lang=code
                                    aria-pressed=if on { "true" } else { "false" }
                                >
                                    {code}
                                </button>
                            }
                        })
                        .collect_view()}
                </form>
            }
            .into_any()
        }
        Foot::SignOut => view! {
            <form method="post" action="/logout">
                <button type="submit" class="oc-auth__ghost">
                    {icon("logout")}
                    {t("auth.sign_out")}
                </button>
            </form>
        }
        .into_any(),
    };
    view! {
        <footer class="oc-auth__foot">
            <span>{t("auth.secure_access")}</span>
            {right}
        </footer>
    }
}

/// A moldura completa. `wide` só para D8a/D8b (duas colunas).
pub(crate) fn frame(
    door: &DoorVm,
    part: &'static str,
    wide: bool,
    card: AnyView,
    kind: Foot,
) -> impl IntoView {
    let card_class = if wide {
        "oc-auth__card oc-auth__card--wide"
    } else {
        "oc-auth__card"
    };
    view! {
        <div class="oc-auth" data-part=part>
            {bar(door.core)}
            <main class="oc-auth__stage" id="oc-main">
                {identity(door)}
                <section class=card_class>
                    {card}
                    {foot(kind)}
                </section>
            </main>
        </div>
    }
}

/// A mensagem do Core, tal como veio (nunca uma genérica inventada).
pub(crate) fn core_message(message: &Option<String>) -> impl IntoView {
    message.clone().map(|text| {
        view! {
            <p class="oc-auth__error" role="alert">
                {icon("warning")}
                <span>{text}</span>
            </p>
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_porta_mostra_codigo_e_nome_da_distribuicao() {
        let door = DoorVm {
            distribution: Some(Distribution::Business),
            core: None,
        };
        let html = identity(&door).to_html();
        assert!(html.contains(r#"data-distribution="business""#));
        assert!(html.contains(">Bu<"));
        assert!(html.contains("Business"));
        assert!(!html.contains("perfil") && !html.contains("Perfil"));
    }

    #[test]
    fn sem_sonda_nao_se_afirma_estado() {
        let html = bar(None).to_html();
        assert!(!html.contains("oc-auth__core"));
        assert!(html.contains(r#"data-oc="clock""#));
    }

    #[test]
    fn o_idioma_so_volta_para_rotas_da_lista_fechada() {
        let html = foot(Foot::Language("/password/recover")).to_html();
        assert!(html.contains(r#"action="/login/language""#));
        assert!(html.contains(r#"name="return_to" value="/password/recover""#));
        assert_eq!(html.matches(r#"name="lang""#).count(), 3);
        assert_eq!(html.matches(r#"aria-pressed="true""#).count(), 1);
        crate::ui::testing::assert_contracts(&html);
    }
}
