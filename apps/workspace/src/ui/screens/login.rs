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

use crate::ui::icon::{icon, Icon};

/// O ecrã de login.
///
/// `core_ready` reflecte uma sonda real ao Ocinye Core: sem ele, autenticar não
/// leva a lado nenhum, e é melhor dizê-lo antes do que falhar depois.
pub fn login(core_ready: bool, message: Option<String>) -> impl IntoView {
    view! {
        <div class="oc-login">
            <div class="oc-login__layer oc-login__glow" aria-hidden="true"></div>
            <div class="oc-login__layer" aria-hidden="true">
                <span class="oc-login__ring oc-login__ring--a"></span>
                <span class="oc-login__ring oc-login__ring--b"></span>
                <span class="oc-login__ring oc-login__ring--c"></span>
            </div>
            <div class="oc-login__layer oc-login__grid" aria-hidden="true"></div>

            <div class="oc-login__bar">
                <span class="oc-login__state">
                    <i aria-hidden="true"></i>
                    <span>
                        {if core_ready { "OCINYE CORE · OPERACIONAL" } else { "OCINYE CORE · INDISPONÍVEL" }}
                    </span>
                </span>
                <span class="oc-login__clock" data-oc="clock"></span>
            </div>

            <div class="oc-login__center">
                <div class="oc-login__brand">
                    <span class="oc-login__tile">
                        <img src="/static/ocinye_logo.png" alt="Ocinye" />
                    </span>
                    <span class="oc-login__wordmark">"OCINYE OS"</span>
                    <span class="oc-login__sub">"OCINYE WORKSPACE"</span>
                </div>

                {message
                    .map(|text| view! { <div class="oc-login__note" role="alert">{text}</div> })}

                {(!core_ready)
                    .then(|| {
                        view! {
                            <div class="oc-login__note" role="alert">
                                "O Ocinye Core não está acessível neste momento. A autenticação não é
                                 possível até que esteja."
                            </div>
                        }
                    })}

                <div class="oc-login__card">
                    <div class="oc-login__who">
                        // O logótipo, e não a inicial da instituição.
                        //
                        // A letra era um substituto de quando não havia
                        // ficheiro. Este ecrã é o primeiro que alguém vê do
                        // Ocinye OS, e é onde a marca tem mais razão para
                        // aparecer inteira.
                        //
                        // `aria-hidden` porque o nome da instituição está
                        // escrito por baixo: um leitor de ecrã que anunciasse
                        // as duas coisas diria a mesma coisa duas vezes.
                        <span class="oc-login__avatar" aria-hidden="true">
                            <img src="/static/avatars/ocinye.png" alt="" />
                        </span>
                        <span class="oc-login__name">"Sessão institucional"</span>
                        <span class="oc-login__mail">"ocinye.com"</span>
                    </div>

                    <form method="post" action="/login">
                        <div class="oc-login__field">
                            {icon(Icon::Mail, 13)}
                            <label class="oc-sr" for="login-user">
                                "Endereço institucional"
                            </label>
                            // `type="email"` e `autocomplete="username"`.
                            //
                            // Não é contradição: `username` é o nome que os
                            // gestores de palavras-passe conhecem para «a
                            // conta», e é com ele que guardam o par certo. O
                            // que muda é o que lá se escreve — o endereço, que
                            // desde o ADR-0106 é a credencial única.
                            <input
                                id="login-user"
                                name="email"
                                type="email"
                                inputmode="email"
                                autocomplete="username"
                                autocapitalize="none"
                                autocorrect="off"
                                spellcheck="false"
                                required
                                placeholder="Endereço institucional"
                            />
                        </div>

                        <div class="oc-login__field">
                            {icon(Icon::Lock, 13)}
                            <label class="oc-sr" for="login-pass">"Palavra-passe"</label>
                            // Sem `maxlength`: truncar silenciosamente uma
                            // passphrase longa faria a autenticação falhar sem
                            // que o membro percebesse porquê (briefing §34).
                            <input
                                id="login-pass"
                                name="password"
                                type="password"
                                autocomplete="current-password"
                                required
                                placeholder="Palavra-passe"
                            />
                            // Gestores de palavras-passe e colar funcionam:
                            // nada aqui os bloqueia (briefing §9).
                            // Texto e não ícone: o dossier fixa 37 ícones, e um
                            // rótulo escrito diz o que faz sem precisar de
                            // convenção visual.
                            <button
                                type="button"
                                class="oc-reveal"
                                data-oc="reveal"
                                data-oc-target="login-pass"
                                aria-pressed="false"
                            >
                                "Mostrar"
                            </button>
                        </div>

                        <button type="submit" class="oc-login__submit" disabled=!core_ready>
                            "Iniciar sessão"
                            {icon(Icon::ArrowRight, 14)}
                        </button>
                    </form>

                    <div class="oc-login__row">
                        <span class="oc-login__alt">
                            "Acesso concedido pela Administração da Ocinye"
                        </span>
                        // `pt-PT` uma vez, e não «PT · pt-PT».
                        //
                        // O dossier escreve as duas metades, mas dizem a mesma
                        // coisa — a língua e a região são as mesmas — e nesta
                        // largura a linha partia-se ao meio do código: lia-se
                        // «PT · pt-» numa linha e «PT» na seguinte, como se
                        // fossem três coisas.
                        //
                        // Não é um selector: o Workspace não tem escolha de
                        // idioma, e anunciar uma seria prometê-la. É a
                        // declaração de em que língua a interface está.
                        <span class="oc-login__lang" lang="pt-PT">"pt-PT"</span>
                    </div>
                </div>
            </div>

            <div class="oc-login__foot">
                <span>{icon(Icon::Power, 13)}"Desligar"</span>
                <span>{icon(Icon::Restart, 13)}"Reiniciar"</span>
                <a href="/health">{icon(Icon::SystemStatus, 13)}"Estado do Sistema"</a>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_login_nao_pede_mfa_nem_oferece_registo() {
        let html = login(true, None).to_html();
        for forbidden in ["MFA", "Criar conta", "Registar", "Google", "Microsoft"] {
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
        let html = login(true, None).to_html();
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
        let html = login(true, None).to_html();
        assert!(html.contains(r#"autocomplete="current-password""#));
        assert!(html.contains(r#"autocomplete="username""#));
        assert!(!html.contains("onpaste"));
        assert!(!html.contains("maxlength"));
        assert!(html.contains("Mostrar"));
    }

    #[test]
    fn o_ecra_nao_promete_mfa_nem_recuperacao_automatica() {
        let html = login(true, None).to_html().to_lowercase();
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
        let html = login(false, None).to_html();
        assert!(html.contains("INDISPONÍVEL"));
        assert!(html.contains("não está acessível"));
    }

    #[test]
    fn todos_os_campos_tem_rotulo() {
        let html = login(true, None).to_html();
        assert!(html.contains("for=\"login-user\""));
        assert!(html.contains("for=\"login-pass\""));
    }
}
