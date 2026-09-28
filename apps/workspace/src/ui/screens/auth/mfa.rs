//! Segundo factor (ADR-0107): configurar (D8a), códigos de recuperação (D8b) e
//! desafio (D8). Só para identidades privilegiadas. DESIGN_LOCKED.
//!
//! Segredos mostrados uma única vez: o QR e a chave manual vivem só nesta
//! resposta; os códigos só na resposta a `POST /mfa/confirm`. Nunca na URL.

use leptos::prelude::*;

use super::{core_message, frame, Foot};
use crate::i18n::t;
use crate::ui::components::icon;
use crate::ui::view_models::{MfaChallengeVm, MfaCodesVm, MfaSetupVm};

/// O QR em SVG, gerado no servidor. `None` se o URI não couber num QR.
fn qr_svg(uri: &str) -> Option<String> {
    use qrcode::render::svg;
    let code = qrcode::QrCode::new(uri.as_bytes()).ok()?;
    let out = code
        .render::<svg::Color<'_>>()
        .min_dimensions(176, 176)
        .quiet_zone(false)
        .dark_color(svg::Color("#0B1F2A"))
        .light_color(svg::Color("#FFFFFF"))
        .build();
    // Só o elemento <svg>, sem a declaração XML.
    out.find("<svg").map(|i| out[i..].to_owned())
}

/// As seis células e o campo único por cima (`static/oc-auth.js` espelha-o).
fn code_input(id: &'static str) -> impl IntoView {
    view! {
        <label class="oc-auth__label" for=id>{t("auth.mfa.code_label")}</label>
        <span class="oc-otp" data-oc="otp">
            <span class="oc-otp__cells" aria-hidden="true">
                {(0..6).map(|_| view! { <span data-part="otp-cell"></span> }).collect_view()}
            </span>
            <input
                class="oc-otp__input"
                id=id
                name="code"
                data-part="otp-input"
                inputmode="numeric"
                autocomplete="one-time-code"
                pattern="[0-9]{6}"
                required
            />
        </span>
    }
}

fn head(ic: &'static str, kicker: &'static str, title: &'static str) -> impl IntoView {
    view! {
        <p class="oc-auth__kicker oc-auth__kicker--icon">
            <span class="oc-auth__icon" data-tone="gold">{icon(ic)}</span>
            {t(kicker)}
        </p>
        <h1 class="oc-auth__title oc-auth__title--left">{t(title)}</h1>
    }
}

/// `GET /mfa` sem segundo factor configurado (D8a): duas colunas.
pub fn setup(vm: &MfaSetupVm) -> impl IntoView {
    let qr = qr_svg(&vm.otpauth_uri);
    let key = vm.manual_key.clone();
    let card = view! {
        <div class="oc-auth__cols">
            <div class="oc-auth__panel">
                {match qr {
                    Some(svg) => view! { <div class="oc-auth__qr" role="img" aria-label=t("auth.mfa.qr_label") inner_html=svg></div> }.into_any(),
                    None => view! { <p class="oc-auth__lead">{t("auth.mfa.qr_failed")}</p> }.into_any(),
                }}
                {if key.is_empty() {
                    // ADR-0107: o segredo não vai no documento enquanto não for pedido.
                    view! {
                        <a class="oc-auth__key-link" href="/mfa?show_key=1">{icon("key")}{t("auth.mfa.show_key")}</a>
                    }
                    .into_any()
                } else {
                    view! {
                        <div class="oc-auth__key">
                            <div class="oc-auth__key-row">
                                <code id="mfa-key">{key}</code>
                                <button type="button" class="oc-round" data-oc="copy" data-copy-target="mfa-key" aria-label=t("auth.copy") title=t("auth.copy")>
                                    {icon("copy")}
                                </button>
                            </div>
                            <p class="oc-auth__note">{t("auth.mfa.key_note")}</p>
                        </div>
                    }
                    .into_any()
                }}
            </div>
            <div class="oc-auth__main">
                {head("shield", "auth.mfa.setup_kicker", "auth.mfa.setup_title")}
                <p class="oc-auth__lead oc-auth__lead--left">{t("auth.mfa.setup_body")}</p>
                <form class="oc-auth__form" method="post" action="/mfa/confirm">
                    {core_message(&vm.error)}
                    {code_input("mfa-setup-code")}
                    <button type="submit" class="oc-btn oc-btn--gold">{t("auth.mfa.confirm")}</button>
                </form>
            </div>
        </div>
    }
    .into_any();
    frame(&vm.door, "mfa-setup", true, card, Foot::SignOut)
}

/// A resposta a `POST /mfa/confirm` (D8b): os códigos, uma única vez.
pub fn codes(vm: &MfaCodesVm) -> impl IntoView {
    let list = vm.codes.join("\n");
    let card = view! {
        <div class="oc-auth__cols">
            <div class="oc-auth__panel">
                <ol class="oc-auth__codes" id="mfa-codes">
                    {vm.codes.iter().map(|c| view! { <li><code>{c.clone()}</code></li> }).collect_view()}
                </ol>
                <div class="oc-auth__row">
                    <button type="button" class="oc-btn oc-btn--line oc-btn--sm" data-oc="copy" data-copy-target="mfa-codes">
                        {icon("copy")}{t("auth.mfa.copy_codes")}
                    </button>
                    <button
                        type="button"
                        class="oc-btn oc-btn--line oc-btn--sm"
                        data-oc="save-codes"
                        data-codes=list
                        data-filename=t("auth.mfa.codes_file")
                    >
                        {icon("download")}{t("auth.mfa.save_codes")}
                    </button>
                </div>
            </div>
            <div class="oc-auth__main">
                {head("key", "auth.mfa.codes_kicker", "auth.mfa.codes_title")}
                <p class="oc-auth__note">{t("auth.mfa.codes_once")}</p>
                <p class="oc-auth__lead oc-auth__lead--left">{t("auth.mfa.codes_body")}</p>
                <form class="oc-auth__form" method="post" action="/mfa/acknowledge">
                    <label class="oc-check">
                        <input type="checkbox" name="acknowledged" value="1" required />
                        <span class="oc-check__box" aria-hidden="true">{icon("check")}</span>
                        <span>{t("auth.mfa.ack")}</span>
                    </label>
                    <button type="submit" class="oc-btn oc-btn--gold">{t("auth.mfa.finish")}{icon("arrow-r")}</button>
                </form>
            </div>
        </div>
    }
    .into_any();
    frame(&vm.door, "mfa-codes", true, card, Foot::SignOut)
}

/// `GET /mfa` com segundo factor configurado (D8).
pub fn challenge(vm: &MfaChallengeVm) -> impl IntoView {
    let card = view! {
        <span class="oc-auth__icon" data-tone="gold">{icon("shield")}</span>
        <h1 class="oc-auth__title">{t("auth.mfa.challenge_title")}</h1>
        <p class="oc-auth__lead">{t("auth.mfa.challenge_body")}</p>
        <form class="oc-auth__form" method="post" action="/mfa/challenge">
            {core_message(&vm.error)}
            {code_input("mfa-challenge-code")}
            <button type="submit" class="oc-btn oc-btn--gold">{t("auth.mfa.verify")}</button>
        </form>
        <details class="oc-auth__details" open=vm.recovery_open>
            <summary>{t("auth.mfa.use_recovery")}</summary>
            <form class="oc-auth__form" method="post" action="/mfa/recovery">
                <label class="oc-pill">
                    <span class="oc-sr">{t("auth.mfa.recovery_label")}</span>
                    {icon("key")}
                    <input name="code" autocomplete="off" spellcheck="false" placeholder=t("auth.mfa.recovery_label") required />
                    <button type="submit" class="oc-go" aria-label=t("auth.mfa.verify") title=t("auth.mfa.verify")>{icon("arrow-r")}</button>
                </label>
                <p class="oc-auth__note">{t("auth.mfa.recovery_hint")}</p>
            </form>
        </details>
    }
    .into_any();
    frame(&vm.door, "mfa-challenge", false, card, Foot::SignOut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;

    #[test]
    fn configurar_gera_o_qr_e_nao_poe_o_segredo_na_url() {
        let vm = MfaSetupVm {
            otpauth_uri: "otpauth://totp/Ocinye:f?secret=JBSWY3DPEHPK3PXP".into(),
            manual_key: "JBSW Y3DP EHPK 3PXP".into(),
            ..Default::default()
        };
        let html = setup(&vm).to_html();
        assert_contracts(&html);
        assert!(html.contains("<svg") && html.contains("oc-auth__qr"));
        assert!(html.contains(r#"action="/mfa/confirm""#) && html.contains(r#"name="code""#));
        assert!(!html.contains("href=\"otpauth"));
        assert!(html.contains("oc-auth__card--wide"));
    }

    #[test]
    fn sem_pedido_a_chave_manual_nao_vai_no_documento() {
        let vm = MfaSetupVm {
            otpauth_uri: "otpauth://totp/x?secret=ABC".into(),
            manual_key: String::new(),
            ..Default::default()
        };
        let html = setup(&vm).to_html();
        assert!(html.contains(r#"href="/mfa?show_key=1""#));
        assert!(!html.contains("<code") && !html.contains("<details"));
        let pedida = setup(&MfaSetupVm {
            manual_key: "ABCD EFGH".into(),
            ..vm
        })
        .to_html();
        assert!(pedida.contains("ABCD EFGH") && pedida.contains(r#"data-oc="copy""#));
    }

    #[test]
    fn os_codigos_exigem_confirmacao() {
        let vm = MfaCodesVm {
            codes: vec!["AAAA-BBBB".into(); 10],
            ..Default::default()
        };
        let html = codes(&vm).to_html();
        assert_contracts(&html);
        assert!(html.contains(r#"action="/mfa/acknowledge""#));
        assert!(html.contains(r#"type="checkbox" name="acknowledged" value="1" required"#));
        assert_eq!(html.matches("<li><code>AAAA-BBBB</code></li>").count(), 10);
    }

    #[test]
    fn o_desafio_aceita_um_codigo_ou_um_de_recuperacao() {
        let html = challenge(&MfaChallengeVm::default()).to_html();
        assert_contracts(&html);
        assert!(
            html.contains(r#"action="/mfa/challenge""#)
                && html.contains(r#"action="/mfa/recovery""#)
        );
        assert!(html.contains(r#"autocomplete="one-time-code""#));
    }
}
