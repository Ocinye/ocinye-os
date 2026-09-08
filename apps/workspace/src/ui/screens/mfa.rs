//! Os ecrãs do segundo factor (ADR-0107).
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

use crate::ui::icon::{icon, Icon};

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
                    <span>{rotulo}</span>
                </span>
                <span class="oc-login__clock" data-oc="clock"></span>
            </div>

            <div class="oc-login__center">
                <div class="oc-login__brand">
                    <span class="oc-login__tile">
                        <img src="/static/ocinye_logo.png" alt="Ocinye" />
                    </span>
                    <span class="oc-login__wordmark">"OCINYE OS"</span>
                    <span class="oc-login__sub">"SEGUNDO FACTOR"</span>
                </div>

                {message
                    .map(|text| view! { <div class="oc-login__note" role="alert">{text}</div> })}

                <div class="oc-login__card">{corpo}</div>

                <div class="oc-login__row">
                    <form method="post" action="/logout">
                        <button type="submit" class="oc-login__alt oc-login__alt--button">
                            "Terminar sessão"
                        </button>
                    </form>
                    <span class="oc-login__lang">"PT · pt-PT"</span>
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
        <div class="oc-login__who">
            <span class="oc-login__name">{nome}</span>
            <span class="oc-login__mail">"Configurar o segundo factor"</span>
        </div>

        <p class="oc-mfa__lead">
            "Abra a sua aplicação autenticadora e leia o código. Depois escreva o
             código de seis dígitos que ela mostrar."
        </p>

        <figure class="oc-mfa__qr">
            <div class="oc-mfa__qr-img" inner_html=svg
                role="img"
                aria-label="Código QR de configuração do segundo factor"></div>
        </figure>

        {match manual {
            None => view! {
                <p class="oc-mfa__manual">
                    <a class="oc-link" href="/mfa?show_key=1">"Mostrar chave manual"</a>
                    " — se não puder ler o QR."
                </p>
            }
            .into_any(),
            Some(chave) => {
                let mostrar = chave.clone();
                view! {
                <div class="oc-mfa__key">
                    <span class="oc-mfa__key-label">"Chave manual"</span>
                    <code class="oc-mono" data-oc="secret" data-oc-value=chave>{mostrar}</code>
                    <button
                        type="button"
                        class="oc-btn oc-btn--sm"
                        data-oc="secret-copy"
                    >
                        "Copiar"
                    </button>
                    <p class="oc-muted oc-mt-3">
                        "Introduza esta chave na aplicação autenticadora, com o tipo
                         «baseada em tempo» (TOTP)."
                    </p>
                </div>
                }
                .into_any()
            }
        }}

        <form method="post" action="/mfa/confirm" class="oc-mt-6">
            <div class="oc-login__field">
                {icon(Icon::Lock, 13)}
                <label class="oc-sr" for="mfa-code">"Código de seis dígitos"</label>
                <input
                    id="mfa-code"
                    name="code"
                    type="text"
                    inputmode="numeric"
                    autocomplete="one-time-code"
                    pattern="[0-9 ]*"
                    required
                    placeholder="Código de seis dígitos"
                />
            </div>
            <button type="submit" class="oc-login__submit">
                "Confirmar"
                {icon(Icon::ArrowRight, 14)}
            </button>
        </form>
    }
    .into_any();

    frame("OCINYE CORE · CONFIGURAR MFA", message, corpo)
}

/// Ecrã dos códigos de recuperação: mostrados uma única vez.
///
/// Concluir exige a confirmação de que foram guardados — sem ela, o enrolamento
/// não fecha (ADR-0107). Copiar e guardar acontecem no browser; nada volta ao
/// Core.
pub fn recovery_codes(codes: &[String]) -> impl IntoView {
    let linhas = codes.join("\n");

    let corpo = view! {
        <div class="oc-login__who">
            <span class="oc-login__name">"Guardar códigos de recuperação"</span>
            <span class="oc-login__mail">"Mostrados uma única vez"</span>
        </div>

        <p class="oc-mfa__lead">
            "Guarde estes dez códigos num local seguro. Cada um serve uma única vez,
             e permite entrar quando não tiver a aplicação autenticadora à mão.
             Não voltarão a ser mostrados."
        </p>

        <pre class="oc-mfa__codes oc-mono" data-oc="recovery-codes">{linhas}</pre>

        <div class="oc-row oc-gap-3 oc-mt-3">
            <button type="button" class="oc-btn oc-btn--sm" data-oc="recovery-copy">
                "Copiar códigos"
            </button>
            <button type="button" class="oc-btn oc-btn--sm" data-oc="recovery-download">
                "Guardar ficheiro"
            </button>
        </div>

        <form method="post" action="/mfa/acknowledge" class="oc-mt-6">
            <label class="oc-check">
                <input type="checkbox" name="acknowledged" value="1" required />
                <span>"Guardei os códigos de recuperação num local seguro."</span>
            </label>
            <button type="submit" class="oc-login__submit oc-mt-3">
                "Concluir"
                {icon(Icon::ArrowRight, 14)}
            </button>
        </form>
    }
    .into_any();

    frame("OCINYE CORE · CÓDIGOS DE RECUPERAÇÃO", None, corpo)
}

/// Ecrã de desafio: o login corrente de uma identidade já enrolada.
///
/// O campo do código de autenticador e o de recuperação são secções distintas e
/// rotuladas, para que ninguém escreva um no outro sem perceber.
pub fn challenge(display_name: &str, message: Option<String>) -> impl IntoView {
    let nome = display_name.to_owned();

    let corpo = view! {
        <div class="oc-login__who">
            <span class="oc-login__name">{nome}</span>
            <span class="oc-login__mail">"Confirme o segundo factor"</span>
        </div>

        <form method="post" action="/mfa/challenge" class="oc-mt-3">
            <div class="oc-login__field">
                {icon(Icon::Lock, 13)}
                <label class="oc-sr" for="mfa-code">"Código do autenticador"</label>
                <input
                    id="mfa-code"
                    name="code"
                    type="text"
                    inputmode="numeric"
                    autocomplete="one-time-code"
                    pattern="[0-9 ]*"
                    required
                    placeholder="Código do autenticador"
                />
            </div>
            <button type="submit" class="oc-login__submit">
                "Entrar"
                {icon(Icon::ArrowRight, 14)}
            </button>
        </form>

        <details class="oc-mfa__fallback oc-mt-6">
            <summary>"Não tenho o autenticador à mão"</summary>
            <p class="oc-muted oc-mt-3">
                "Use um dos códigos de recuperação que guardou ao configurar o MFA.
                 Cada código serve uma única vez."
            </p>
            <form method="post" action="/mfa/recovery" class="oc-mt-3">
                <div class="oc-login__field">
                    {icon(Icon::Lock, 13)}
                    <label class="oc-sr" for="mfa-recovery">"Código de recuperação"</label>
                    <input
                        id="mfa-recovery"
                        name="code"
                        type="text"
                        autocomplete="off"
                        required
                        placeholder="Código de recuperação"
                    />
                </div>
                <button type="submit" class="oc-btn oc-btn--secondary">
                    "Entrar com código de recuperação"
                </button>
            </form>
        </details>
    }
    .into_any();

    frame("OCINYE CORE · SEGUNDO FACTOR", message, corpo)
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
        assert!(html.contains("Mostrar chave manual"));
        // E não há Workspace nenhum.
        for fuga in ["oc-side", "oc-topbar", "/administration", "SUPER ADMIN"] {
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
        assert!(html.contains("Não tenho o autenticador à mão"));
        // Nada do Workspace.
        for fuga in ["oc-side", "oc-topbar", "SUPER ADMIN"] {
            assert!(!html.contains(fuga), "o desafio expõe «{fuga}»");
        }
    }
}
