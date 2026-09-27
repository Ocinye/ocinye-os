//! Ecrã de início de sessão.
//!
//! Deve dar a sensação de arranque de uma workstation, não de um formulário
//! web (`design/README.md` §6.1).
//!
//! # O que este ecrã deliberadamente não tem
//!
//! MFA, códigos de seis dígitos, registo público, login social, magic links,
//! banners. A fase actual autentica com **endereço institucional e
//! palavra-passe** (ADR-0106),
//! e mais nada (ADR-0103). MFA está `PLANNED`, não implementado.
//!
//! # Quem decide
//!
//! Este ecrã recolhe credenciais e envia-as ao Ocinye Core. Não valida, não
//! compara e não decide: a autoridade de autenticação é o Core.

use leptos::prelude::*;

/// O ecrã de login.
///
/// `core_ready` reflecte uma sonda real ao Ocinye Core: sem ele, autenticar não
/// leva a lado nenhum, e é melhor dizê-lo antes do que falhar depois.
pub fn login(
    core_ready: bool,
    message: Option<String>,
    instancia: Option<String>,
) -> impl IntoView {
    // Sem resposta do Core, o nome é o do produto: nunca se adivinha uma
    // instituição à porta.
    let nome = instancia.unwrap_or_else(|| "OCINYE OS".to_owned());

    view! {
        <main class="ods-auth" data-part="login">
            <span class="ods-auth__clock" data-oc="clock"></span>
            <span class="ods-auth__mark"><img src="/static/ocinye_logo.png" alt="" /></span>
            <p class="ods-auth__inst">{nome}</p>
            <p class="ods-label">{crate::i18n::t("auth.instance_line")}</p>

            <section class="ods-auth__card">
                <h1 class="ods-auth__title">{crate::i18n::t("login.sign_in")}</h1>

                // O estado do Core antes de tentar: sem ele, autenticar não leva a
                // lado nenhum.
                {(!core_ready).then(|| view! {
                    <div class="ods-notice ods-notice--error" role="alert">
                        {crate::i18n::t("login.core_down_note")}
                    </div>
                })}

                <form method="post" action="/login">
                    <label class="ods-field">
                        <span class="ods-field__label">{crate::i18n::t("login.institutional_address")}</span>
                        <input
                            class="ods-input"
                            id="login-user"
                            name="email"
                            type="email"
                            inputmode="email"
                            autocomplete="username"
                            autocapitalize="none"
                            spellcheck="false"
                            required
                        />
                    </label>
                    <label class="ods-field">
                        <span class="ods-field__label">{crate::i18n::t("auth.password")}</span>
                        <input
                            class="ods-input"
                            id="login-pass"
                            name="password"
                            type="password"
                            autocomplete="current-password"
                            required
                        />
                    </label>

                    // O erro é o texto que o Core devolveu, nunca um genérico.
                    {message.map(|texto| view! {
                        <p class="ods-field__error" role="alert">{texto}</p>
                    })}

                    <button
                        type="submit"
                        class="ods-btn ods-btn--primary ods-btn--block"
                        data-part="login__submit"
                        disabled=!core_ready
                    >
                        {crate::i18n::t("login.sign_in")}
                    </button>
                </form>

                <div class="ods-auth__foot">
                    <span>{crate::i18n::t("login.granted_by_admin")}</span>
                    <span lang=crate::i18n::current().bcp47()>{crate::i18n::current().bcp47()}</span>
                </div>
            </section>
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_login_nao_pede_mfa_nem_oferece_registo() {
        let html = login(true, None, None).to_html();
        for forbidden in [
            "MFA",
            crate::i18n::t("login.create_account"),
            "Registar",
            "Google",
            "Microsoft",
        ] {
            assert!(
                !html.contains(forbidden),
                "o login não deve conter {forbidden}"
            );
        }
    }

    #[test]
    fn o_formulario_submete_as_credenciais_ao_core() {
        // Invertido pelo ADR-0103: o campo era desactivado porque o IdP
        // autenticava. Agora o Core é a autoridade, e o campo tem de funcionar.
        let html = login(true, None, None).to_html();
        assert!(html.contains(r#"method="post""#));
        assert!(html.contains(r#"action="/login""#));
        assert!(html.contains(r#"name="email""#));
        assert!(html.contains(r#"type="email""#));
        assert!(html.contains(r#"name="password""#));
        assert!(html.contains(r#"type="password""#));
    }

    #[test]
    fn gestores_de_palavras_passe_e_colar_funcionam() {
        // Bloquear colar empurra as pessoas para palavras-passe que consigam
        // decorar, que é o oposto do que a política quer (briefing §9).
        let html = login(true, None, None).to_html();
        assert!(html.contains(r#"autocomplete="current-password""#));
        assert!(html.contains(r#"autocomplete="username""#));
        assert!(!html.contains("onpaste"));
        assert!(!html.contains("maxlength"));
    }

    #[test]
    fn o_login_nao_revela_a_palavra_passe() {
        // Segurança: o «Mostrar» pertence ao ecrã de definição de
        // palavra-passe, não ao de entrada. Aqui a pessoa só digita uma
        // palavra-passe que já sabe, e revelá-la em claro só a expõe.
        let html = login(true, None, None).to_html();
        assert!(!html.contains("Mostrar"));
        assert!(!html.contains(r#"data-oc="reveal""#));
    }

    #[test]
    fn o_ecra_nao_promete_mfa_nem_recuperacao_automatica() {
        let html = login(true, None, None).to_html().to_lowercase();
        for ausente in [
            "mfa",
            "autenticação de dois",
            "esqueci",
            "recuperar palavra",
        ] {
            assert!(!html.contains(ausente), "o login promete «{ausente}»");
        }
    }

    #[test]
    fn com_o_core_em_baixo_o_ecra_diz_o_e_impede_submeter() {
        let html = login(false, None, None).to_html();
        assert!(html.contains("não está acessível"));
        assert!(
            html.contains("disabled"),
            "com o Core em baixo o formulário submete"
        );
    }

    #[test]
    fn todos_os_campos_tem_rotulo() {
        // O rótulo envolve o campo (D1): cada campo está dentro do seu `<label>`.
        let html = login(true, None, None).to_html();
        for id in ["login-user", "login-pass"] {
            let antes = html
                .split(&format!(r#"id="{id}""#))
                .next()
                .unwrap_or_default();
            assert!(
                antes.rfind("<label").unwrap_or(0) > antes.rfind("</label>").unwrap_or(0),
                "o campo {id} não está dentro de um rótulo"
            );
        }
    }
}

#[cfg(test)]
mod pureza_i18n {
    use super::*;

    /// Um ecrã, um idioma: o login em francês, sem português.
    #[tokio::test]
    async fn o_login_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};
        let fr = with_locale(Locale::Fr, async { login(true, None, None).to_html() }).await;
        for francesa in ["Se connecter", "Adresse institutionnelle", "Mot de passe"] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        assert!(!fr.contains("Iniciar sessão"), "fr: chrome português");
    }
}
