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

use crate::ui::icon::{icon, Icon};

/// Comprimento mínimo exigido pelo Ocinye Core.
///
/// Duplicado aqui apenas para o texto do ecrã. O Core valida de novo, e é a
/// sua resposta que aparece em `message` quando difere.
const MIN_LENGTH: usize = 15;

/// O ecrã de primeiro acesso.
///
/// `message` traz a recusa do Core — comprimento, blocklist, reutilização da
/// credencial temporária — tal como o Core a redigiu.
pub fn first_access(display_name: &str, username: &str, message: Option<String>) -> impl IntoView {
    let initials = crate::ui::initials(display_name);
    let name = display_name.to_owned();
    let username = username.to_owned();

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
                    <span>"OCINYE CORE · PRIMEIRO ACESSO"</span>
                </span>
                <span class="oc-login__clock" data-oc="clock"></span>
            </div>

            <div class="oc-login__center">
                <div class="oc-login__brand">
                    <span class="oc-login__tile">
                        <img src="/static/ocinye_logo.png" alt="Ocinye" />
                    </span>
                    <span class="oc-login__wordmark">"OCINYE OS"</span>
                    <span class="oc-login__sub">"PRIMEIRO ACESSO"</span>
                </div>

                {message
                    .map(|text| view! { <div class="oc-login__note" role="alert">{text}</div> })}

                <div class="oc-login__card">
                    <div class="oc-login__who">
                        <span class="oc-login__avatar" aria-hidden="true">{initials}</span>
                        <span class="oc-login__name">{name}</span>
                        <span class="oc-login__mail">"Defina a sua palavra-passe"</span>
                    </div>

                    <p class="oc-first__lead">
                        "Por segurança, deve substituir a palavra-passe temporária antes
                         de continuar. A palavra-passe temporária deixará de funcionar."
                    </p>

                    <form method="post" action="/first-access">
                        // O gestor de palavras-passe precisa de saber a quem
                        // pertence a palavra-passe nova.
                        //
                        // Sem este campo o formulário só tem `password` e
                        // `confirmation`: o browser guarda a credencial sem
                        // nome associado e, no ecrã de entrada seguinte,
                        // preenche a palavra-passe deixando o utilizador
                        // vazio. É a convenção documentada para formulários de
                        // mudança de palavra-passe.
                        //
                        // O campo é enviado com o formulário — `readonly` é
                        // submetido, ao contrário de `disabled` — e o servidor
                        // ignora-o: `PasswordForm` só tem `password` e
                        // `confirmation`. É deliberado que o ignore. Quem está
                        // autenticado sai da sessão, e aceitar um nome vindo do
                        // formulário seria deixar o cliente escolher a conta.
                        <input
                            type="text"
                            name="_username"
                            value=username.clone()
                            autocomplete="username"
                            aria-hidden="true"
                            tabindex="-1"
                            readonly
                            class="oc-sr"
                        />

                        <div class="oc-login__field">
                            {icon(Icon::Lock, 13)}
                            <label class="oc-sr" for="new-pass">"Nova palavra-passe"</label>
                            <input
                                id="new-pass"
                                name="password"
                                type="password"
                                autocomplete="new-password"
                                required
                                minlength=MIN_LENGTH.to_string()
                                placeholder="Nova palavra-passe"
                            />
                            <button
                                type="button"
                                class="oc-reveal"
                                data-oc="reveal"
                                data-oc-target="new-pass"
                                aria-pressed="false"
                            >
                                "Mostrar"
                            </button>
                        </div>

                        <div class="oc-login__field">
                            {icon(Icon::Lock, 13)}
                            <label class="oc-sr" for="confirm-pass">
                                "Confirmar nova palavra-passe"
                            </label>
                            <input
                                id="confirm-pass"
                                name="confirmation"
                                type="password"
                                autocomplete="new-password"
                                required
                                minlength=MIN_LENGTH.to_string()
                                placeholder="Confirmar nova palavra-passe"
                            />
                            // Também aqui, e pela mesma razão que no campo de
                            // cima: quem não consegue ler o que escreveu só
                            // descobre a divergência quando o formulário é
                            // recusado. Confirmar às cegas transforma uma gralha
                            // numa tentativa perdida.
                            <button
                                type="button"
                                class="oc-reveal"
                                data-oc="reveal"
                                data-oc-target="confirm-pass"
                                aria-pressed="false"
                            >
                                "Mostrar"
                            </button>
                        </div>

                        <ul class="oc-first__rules">
                            <li>{format!("Mínimo de {MIN_LENGTH} caracteres.")}</li>
                            <li>"Frases longas são aceites, com espaços e acentos."</li>
                            <li>"Não são exigidos símbolos nem maiúsculas."</li>
                            <li>"Palavras-passe comuns ou previsíveis são recusadas."</li>
                        </ul>

                        <button type="submit" class="oc-login__submit">
                            "Definir palavra-passe"
                            {icon(Icon::ArrowRight, 14)}
                        </button>
                    </form>

                    <div class="oc-login__row">
                        <form method="post" action="/logout">
                            <button type="submit" class="oc-login__alt oc-login__alt--button">
                                "Terminar sessão"
                            </button>
                        </form>
                        <span class="oc-login__lang">"PT · pt-PT"</span>
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
        assert!(html.contains(r#"for="new-pass""#));
        assert!(html.contains(r#"for="confirm-pass""#));
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
        let html = render(Some("Esta palavra-passe é demasiado comum.".to_owned()));
        assert!(html.contains("Esta palavra-passe é demasiado comum."));
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
