//! Os ecrãs do segundo factor (ADR-0107).
//!
//! DESIGN_LOCKED · Proposta D8. Moldura partilhada com o login (`login::barra`,
//! `login::identidade`); o conteúdo de cada cartão é deste ficheiro.
//!
//! Fazem parte do arranque de uma sessão privilegiada, não são um formulário de
//! um website. Usam a mesma linguagem visual do início de sessão e do primeiro
//! acesso, porque é o mesmo momento: alguém está a estabelecer autoridade.
//!
//! # O que estes ecrãs nunca fazem
//!
//! Não mostram o Workspace. Durante o MFA a pessoa está num fluxo de
//! autenticação — não há navegação, não há Administração, não há faixa de sessão
//! privilegiada. E não decidem nada: o modo (enrolar ou desafiar) vem do Core.
//!
//! # A chave manual
//!
//! O QR é o caminho primário. A chave em base32 **não** aparece ao lado dele por
//! omissão, nem viaja escondida no HTML: revela-se por uma acção explícita que
//! volta ao servidor e traz o **mesmo** seed. O QR e a chave manual são sempre o
//! mesmo segredo (ADR-0107).

use leptos::prelude::*;
use qrcode::render::svg;
use qrcode::QrCode;

/// Renderiza a URI `otpauth://` como SVG inline, para o QR caber na CSP apertada
/// do Workspace sem abrir `img-src`.
fn qr_svg(otpauth: &str) -> String {
    match QrCode::new(otpauth.as_bytes()) {
        Ok(code) => code
            .render::<svg::Color>()
            .min_dimensions(220, 220)
            .quiet_zone(true)
            .dark_color(svg::Color("#0B1F2A"))
            .light_color(svg::Color("#FFFFFF"))
            .build(),
        // Um otpauth que não cabe num QR seria um erro de programação, não do
        // membro. A chave manual continua a permitir concluir o enrolamento.
        Err(_) => String::new(),
    }
}

/// Moldura comum dos ecrãs de MFA: o mesmo fundo e barra do início de sessão.
fn frame(rotulo: &'static str, message: Option<String>, corpo: AnyView) -> impl IntoView {
    use crate::ui::screens::login::barra;
    view! {
        <main class="ods-auth" data-part="login">
            {barra(None)}
            <div class="ods-auth__stage">
                <span class="ods-auth__mark"><img src="/static/ocinye_logo.png" alt="" /></span>
                <p class="ods-auth__product">{rotulo}</p>

                <section class="ods-auth__card">
                    <span class="ods-auth__icon" data-tone="gold">{crate::ui::ods::icone("shield", "ods-icon--lg")}</span>
                    {message.map(|text| view! { <p class="ods-field__error" role="alert">{text}</p> })}
                    {corpo}
                    <div class="ods-auth__foot">
                        <form method="post" action="/logout">
                            <button type="submit" class="ods-btn ods-btn--ghost ods-btn--sm">
                                {crate::ui::ods::icone("logout", "")}
                                {crate::i18n::t("auth.sign_out")}
                            </button>
                        </form>
                        <span class="ods-auth__locale">{format!("{} · {}", crate::i18n::current().as_str().to_uppercase(), crate::i18n::current().bcp47())}</span>
                    </div>
                </section>
            </div>
        </main>
    }
}

/// Ecrã de enrolamento: ler o QR, confirmar com um código.
///
/// `manual_key` só vem preenchido quando a pessoa carregou em «Mostrar chave
/// manual» — e é então o mesmo seed que o QR codifica. Ausente, o segredo em
/// texto não está na página.
pub fn enrollment(
    display_name: &str,
    otpauth_uri: &str,
    manual_key: Option<&str>,
    message: Option<String>,
) -> impl IntoView {
    let svg = qr_svg(otpauth_uri);
    let nome = display_name.to_owned();
    let manual = manual_key.map(str::to_owned);

    let corpo = view! {
        <div class="ods-account__head"><div>
            <p class="ods-auth__title">{nome}</p>
            <p class="ods-account__mail">{crate::i18n::t("mfa.setup")}</p>
        </div></div>

        <p class="ods-auth__lead">
            {crate::i18n::t("mfa.enroll_lead")}
        </p>

        <figure class="ods-auth__qr">
            <div inner_html=svg
                role="img"
                aria-label=crate::i18n::t("mfa.qr_alt")></div>
        </figure>

        {match manual {
            None => view! {
                <p class="ods-auth__lead">
                    <a href="/mfa?show_key=1">{crate::i18n::t("mfa.show_manual_key")}</a>
                    {crate::i18n::t("mfa.if_cannot_read_qr")}
                </p>
            }
            .into_any(),
            Some(chave) => {
                let mostrar = chave.clone();
                view! {
                <div class="ods-field">
                    <span class="ods-field__label">{crate::i18n::t("mfa.manual_key")}</span>
                    <code class="ods-auth__secret" data-oc="secret" data-oc-value=chave>{mostrar}</code>
                    <button
                        type="button"
                        class="ods-btn ods-btn--sm"
                        data-oc="secret-copy"
                    >
                        {crate::i18n::t("mfa.copy_short")}
                    </button>
                    <p class="ods-field__hint">
                        {crate::i18n::t("mfa.manual_key_note")}
                    </p>
                </div>
                }
                .into_any()
            }
        }}

        <form method="post" action="/mfa/confirm">
            <label class="ods-field">
                <span class="ods-field__label">{crate::i18n::t("mfa.six_digit_code")}</span>
                <span class="ods-auth__otp">
                    <span class="ods-auth__otp-cells" aria-hidden="true"><span></span><span></span><span></span><span></span><span></span><span></span></span>
                    <input class="ods-input ods-auth__code"
                    id="mfa-code"
                    name="code"
                    type="text"
                    inputmode="numeric"
                    autocomplete="one-time-code"
                    pattern="[0-9 ]*"
                    required
                />
                </span>
            </label>
            <button type="submit" class="ods-btn ods-btn--primary ods-btn--block" data-part="login__submit">
                {crate::i18n::t("mfa.confirm_button")}
            </button>
        </form>
    }
    .into_any();

    frame(crate::i18n::t("mfa.frame.setup"), message, corpo)
}

/// Ecrã dos códigos de recuperação: mostrados uma única vez.
///
/// Concluir exige a confirmação de que foram guardados — sem ela, o enrolamento
/// não fecha (ADR-0107). Copiar e guardar acontecem no browser; nada volta ao
/// Core.
pub fn recovery_codes(codes: &[String]) -> impl IntoView {
    let linhas = codes.join("\n");

    let corpo = view! {
        <div class="ods-account__head"><div>
            <p class="ods-auth__title">{crate::i18n::t("mfa.recovery_codes")}</p>
            <p class="ods-account__mail">{crate::i18n::t("mfa.shown_once")}</p>
        </div></div>

        <p class="ods-auth__lead">{crate::i18n::t("mfa.recovery_lead")}</p>

        <pre class="ods-auth__codes" data-oc="recovery-codes">{linhas}</pre>

        <div class="ods-auth__actions">
            <button type="button" class="ods-btn ods-btn--sm" data-oc="recovery-copy">
                {crate::i18n::t("mfa.copy_codes")}
            </button>
            <button type="button" class="ods-btn ods-btn--sm" data-oc="recovery-download">
                {crate::i18n::t("mfa.save_file")}
            </button>
        </div>

        <form method="post" action="/mfa/acknowledge">
            <label class="ods-auth__ack">
                <input type="checkbox" class="ods-check" name="acknowledged" value="1" required />
                <span>{crate::i18n::t("mfa.saved_confirm")}</span>
            </label>
            <button type="submit" class="ods-btn ods-btn--primary ods-btn--block" data-part="login__submit">
                {crate::i18n::t("mfa.finish")}
            </button>
        </form>
    }
    .into_any();

    frame(crate::i18n::t("mfa.frame.recovery"), None, corpo)
}

/// Ecrã de desafio: o login corrente de uma identidade já enrolada.
///
/// O campo do código de autenticador e o de recuperação são secções distintas e
/// rotuladas, para que ninguém escreva um no outro sem perceber.
pub fn challenge(display_name: &str, message: Option<String>) -> impl IntoView {
    let nome = display_name.to_owned();

    let corpo = view! {
        <div class="ods-account__head"><div>
            <p class="ods-auth__title">{nome}</p>
            <p class="ods-account__mail">{crate::i18n::t("mfa.confirm")}</p>
        </div></div>

        <form method="post" action="/mfa/challenge">
            <label class="ods-field">
                <span class="ods-field__label">{crate::i18n::t("mfa.authenticator_code")}</span>
                <span class="ods-auth__otp">
                    <span class="ods-auth__otp-cells" aria-hidden="true"><span></span><span></span><span></span><span></span><span></span><span></span></span>
                    <input class="ods-input ods-auth__code"
                    id="mfa-code"
                    name="code"
                    type="text"
                    inputmode="numeric"
                    autocomplete="one-time-code"
                    pattern="[0-9 ]*"
                    required
                />
                </span>
            </label>
            <button type="submit" class="ods-btn ods-btn--primary ods-btn--block" data-part="login__submit">
                {crate::i18n::t("mfa.sign_in")}
            </button>
        </form>

        <details class="ods-auth__details" data-part="mfa__fallback">
            <summary>{crate::i18n::t("mfa.no_authenticator")}</summary>
            <p class="ods-field__hint">{crate::i18n::t("mfa.recovery_hint")}</p>
            <form method="post" action="/mfa/recovery">
                <label class="ods-field">
                    <span class="ods-field__label">{crate::i18n::t("mfa.recovery_code")}</span>
                    <input class="ods-input"
                        id="mfa-recovery"
                        name="code"
                        type="text"
                        autocomplete="off"
                        required
                    />
                </label>
                <button type="submit" class="ods-btn ods-btn--block">
                    {crate::i18n::t("mfa.enter_with_recovery")}
                </button>
            </form>
        </details>
    }
    .into_any();

    frame(crate::i18n::t("mfa.frame.challenge"), message, corpo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_enrolamento_mostra_o_qr_e_nao_a_chave_por_omissao() {
        let html = enrollment(
            "Fidel Admin",
            "otpauth://totp/Ocinye:fidel.admin@ocinye.com?secret=ABCDEF2345&issuer=Ocinye",
            None,
            None,
        )
        .to_html();
        // Há um QR (SVG inline).
        assert!(html.contains("<svg"), "não renderizou o QR em SVG");
        // A chave manual não está no DOM inicial.
        assert!(
            !html.contains("ABCDEF2345"),
            "o segredo em base32 apareceu na página sem acção explícita"
        );
        // Mas há como pedi-la.
        assert!(html.contains("/mfa?show_key=1"));
        assert!(html.contains(crate::i18n::t("mfa.show_manual_key")));
        // E não há Workspace nenhum.
        for fuga in [
            r#"data-oc="shell""#,
            r#"data-oc="side-pinned""#,
            "/administration",
            "SUPER ADMIN",
        ] {
            assert!(!html.contains(fuga), "o ecrã expõe «{fuga}»");
        }
    }

    #[test]
    fn a_chave_manual_revelada_e_a_mesma_do_qr() {
        let html = enrollment(
            "Fidel Admin",
            "otpauth://totp/Ocinye:x?secret=SEEDSEEDSEED&issuer=Ocinye",
            Some("SEEDSEEDSEED"),
            None,
        )
        .to_html();
        assert!(
            html.contains("SEEDSEEDSEED"),
            "a chave revelada não apareceu"
        );
        assert!(html.contains("<svg"), "o QR devia continuar presente");
        // Já não oferece revelar de novo.
        assert!(!html.contains("show_key=1"));
    }

    #[test]
    fn os_codigos_de_recuperacao_exigem_reconhecimento() {
        let codigos = vec![
            "ABCDE-FGHJK-LMNPQ".to_owned(),
            "RSTUV-WXYZ2-34567".to_owned(),
        ];
        let html = recovery_codes(&codigos).to_html();
        assert!(html.contains("ABCDE-FGHJK-LMNPQ"));
        assert!(html.contains("data-oc=\"recovery-codes\""));
        // Concluir exige a confirmação (checkbox required).
        assert!(html.contains("name=\"acknowledged\""));
        assert!(html.contains("required"));
        assert!(html.contains("action=\"/mfa/acknowledge\""));
        // Copiar e guardar existem, e são só do browser.
        assert!(html.contains("data-oc=\"recovery-copy\""));
        assert!(html.contains("data-oc=\"recovery-download\""));
    }

    #[test]
    fn o_desafio_separa_totp_de_recuperacao() {
        let html = challenge("Fidel Admin", None).to_html();
        assert!(html.contains("action=\"/mfa/challenge\""));
        assert!(html.contains("action=\"/mfa/recovery\""));
        assert!(html.contains(crate::i18n::t("mfa.no_authenticator")));
        // Nada do Workspace.
        for fuga in [
            r#"data-oc="shell""#,
            r#"data-oc="side-pinned""#,
            "SUPER ADMIN",
        ] {
            assert!(!html.contains(fuga), "o desafio expõe «{fuga}»");
        }
    }
}

#[cfg(test)]
mod pureza_i18n {
    use super::*;

    /// Um ecrã, um idioma: o enrolamento do segundo factor em francês.
    #[tokio::test]
    async fn o_mfa_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};
        let fr = with_locale(Locale::Fr, async {
            enrollment(
                "Fidel",
                "otpauth://totp/Ocinye:fidel?secret=JBSWY3DPEHPK3PXP&issuer=Ocinye",
                None,
                None,
            )
            .to_html()
        })
        .await;
        for francesa in [
            "OCINYE CORE · CONFIGURER LE MFA",
            "Confirmer",
            "Ouvrez votre application",
            "Se déconnecter",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        assert!(!fr.contains("CONFIGURAR MFA"), "fr: moldura portuguesa");
        assert!(!fr.contains("Terminar sessão"), "fr: rodapé português");
    }
}
