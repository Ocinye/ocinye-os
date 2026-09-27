//! Ecrã de primeiro acesso: definir a palavra-passe definitiva.
//!
//! Faz parte do arranque do sistema, não é um formulário de recuperação de um
//! website (briefing §109). Usa a mesma linguagem visual do ecrã de início de
//! sessão porque é o mesmo momento: alguém está a entrar no Ocinye OS pela
//! primeira vez.
//!
//! # O que este ecrã não faz
//!
//! Não valida. Mostra a regra e o que o Core respondeu; a decisão é sempre do
//! Ocinye Core (briefing §27). Um indicador de força local seria uma segunda
//! opinião sobre uma questão que já tem dono.

use leptos::prelude::*;

/// Comprimento mínimo exigido pelo Ocinye Core.
///
/// Duplicado aqui apenas para o texto do ecrã. O Core valida de novo, e é a
/// sua resposta que aparece em `message` quando difere.
const MIN_LENGTH: usize = 15;

/// O ecrã de primeiro acesso.
///
/// `message` traz a recusa do Core — comprimento, blocklist, reutilização da
/// credencial temporária — tal como o Core a redigiu.
pub fn first_access(display_name: &str, email: &str, message: Option<String>) -> impl IntoView {
    let initials = crate::ui::initials(display_name);
    let name = display_name.to_owned();
    let email = email.to_owned();

    view! {
        <main class="ods-auth" data-part="login">
            <span class="ods-auth__clock" data-oc="clock"></span>
            <span class="ods-auth__mark"><img src="/static/ocinye_logo.png" alt="" /></span>
            <p class="ods-label">{crate::i18n::t("first_access.eyebrow")}</p>

            <section class="ods-auth__card">
                <div class="ods-account__head">
                    <span class="ods-avatar" aria-hidden="true">{initials}</span>
                    <div>
                        <p class="ods-account__name">{name}</p>
                        <p class="ods-account__mail">{crate::i18n::t("first_access.set_password")}</p>
                    </div>
                </div>

                {message.map(|text| view! { <p class="ods-field__error" role="alert">{text}</p> })}

                <p class="ods-auth__lead">
                    "Por segurança, deve substituir a palavra-passe temporária antes
                     de continuar. A palavra-passe temporária deixará de funcionar."
                </p>

                <form method="post" action="/first-access">
                    // O gestor de palavras-passe precisa de saber a que conta
                    // pertence a palavra-passe nova; invisível para quem lê.
                    <input
                        type="text"
                        name="_username"
                        value=email.clone()
                        autocomplete="username"
                        aria-hidden="true"
                        tabindex="-1"
                        readonly
                        class="ods-sr-only"
                    />

                    <label class="ods-field">
                        <span class="ods-field__label">{crate::i18n::t("first_access.new_password")}</span>
                        <span class="ods-auth__pw">
                            <input
                                class="ods-input"
                                id="new-pass"
                                name="password"
                                type="password"
                                autocomplete="new-password"
                                required
                                minlength=MIN_LENGTH.to_string()
                            />
                            <button
                                type="button"
                                class="ods-btn ods-btn--ghost ods-btn--sm ods-auth__reveal"
                                data-oc="reveal"
                                data-oc-target="new-pass"
                                aria-pressed="false"
                            >
                                {crate::i18n::t("first_access.show")}
                            </button>
                        </span>
                    </label>

                    <label class="ods-field">
                        <span class="ods-field__label">{crate::i18n::t("first_access.confirm_password")}</span>
                        <span class="ods-auth__pw">
                            <input
                                class="ods-input"
                                id="confirm-pass"
                                name="confirmation"
                                type="password"
                                autocomplete="new-password"
                                required
                                minlength=MIN_LENGTH.to_string()
                            />
                            <button
                                type="button"
                                class="ods-btn ods-btn--ghost ods-btn--sm ods-auth__reveal"
                                data-oc="reveal"
                                data-oc-target="confirm-pass"
                                aria-pressed="false"
                            >
                                {crate::i18n::t("first_access.show")}
                            </button>
                        </span>
                    </label>

                    <ul class="ods-field__hint">
                        <li>{crate::i18n::tf("first_access.min_length_dot", &[("min", &MIN_LENGTH.to_string())])}</li>
                        <li>{crate::i18n::t("first_access.long_phrases")}</li>
                        <li>{crate::i18n::t("first_access.no_symbols_required")}</li>
                        <li>{crate::i18n::t("first_access.common_rejected")}</li>
                    </ul>

                    <button type="submit" class="ods-btn ods-btn--primary ods-btn--block" data-part="login__submit">
                        {crate::i18n::t("first_access.set_password_button")}
                    </button>
                </form>

                <div class="ods-auth__foot">
                    <form method="post" action="/logout">
                        <button type="submit" class="ods-btn ods-btn--ghost ods-btn--sm">
                            {crate::i18n::t("auth.sign_out")}
                        </button>
                    </form>
                    <span>{format!("{} · {}", crate::i18n::current().as_str().to_uppercase(), crate::i18n::current().bcp47())}</span>
                </div>
            </section>
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(message: Option<String>) -> String {
        first_access("João Manuel", "joao", message).to_html()
    }

    #[test]
    fn o_ecra_declara_a_regra_de_comprimento() {
        let html = render(None);
        assert!(html.contains("Mínimo de 15 caracteres"));
        assert!(html.contains(r#"minlength="15""#));
    }

    #[test]
    fn nao_impoe_composicao_artificial() {
        let html = render(None).to_lowercase();
        assert!(html.contains("não são exigidos símbolos"));
        for imposto in [
            "pelo menos uma maiúscula",
            "pelo menos um número",
            "um símbolo obrigat",
        ] {
            assert!(!html.contains(imposto), "o ecrã impõe «{imposto}»");
        }
    }

    #[test]
    fn os_dois_campos_tem_rotulo_e_permitem_gestores_de_palavras_passe() {
        let html = render(None);
        // O rótulo envolve o campo (D1): cada campo está dentro do seu `<label>`.
        for id in ["new-pass", "confirm-pass"] {
            let antes = html
                .split(&format!(r#"id="{id}""#))
                .next()
                .unwrap_or_default();
            assert!(
                antes.rfind("<label").unwrap_or(0) > antes.rfind("</label>").unwrap_or(0),
                "o campo {id} não está dentro de um rótulo"
            );
        }
        assert!(html.contains(r#"autocomplete="new-password""#));
        // Nada bloqueia colar nem limita o comprimento máximo.
        assert!(!html.contains("onpaste"));
        assert!(!html.contains("maxlength"));
    }

    #[test]
    fn os_dois_campos_podem_ser_revelados() {
        // O alternador estava só no primeiro campo. Confirmar às cegas o que se
        // acabou de ler é pedir uma gralha: a divergência só aparecia depois de
        // o formulário ser recusado.
        let html = render(None);
        for alvo in ["new-pass", "confirm-pass"] {
            assert!(
                html.contains(&format!(r#"data-oc-target="{alvo}""#)),
                "o campo {alvo} não tem alternador de visibilidade"
            );
        }
        assert_eq!(html.matches(r#"data-oc="reveal""#).count(), 2);
    }

    #[test]
    fn a_recusa_do_core_e_mostrada_tal_como_veio() {
        let html = render(Some(crate::i18n::t("first_access.too_common").to_owned()));
        assert!(html.contains(crate::i18n::t("first_access.too_common")));
        assert!(html.contains(r#"role="alert""#));
    }

    #[test]
    fn ha_sempre_uma_saida_sem_definir_a_palavra_passe() {
        // Quem não consegue completar o passo tem de poder sair (briefing §22).
        let html = render(None);
        assert!(html.contains(r#"action="/logout""#));
    }

    #[test]
    fn o_ecra_nao_oferece_nada_do_workspace() {
        // Durante a mudança obrigatória não há navegação institucional nenhuma.
        let html = render(None);
        for fuga in [
            "/ideas",
            "/projects",
            "/units",
            "/datasets",
            "oc-side",
            "oc-topbar",
        ] {
            assert!(!html.contains(fuga), "o ecrã expõe «{fuga}»");
        }
    }

    #[test]
    fn texto_interpolado_nao_injecta_markup() {
        let html = first_access("<script>alert(1)</script>", "joao", None).to_html();
        assert!(!html.contains("<script>alert(1)</script>"));
    }
}
