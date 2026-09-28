//! Início de sessão (D7), recuperar palavra-passe (D10) e fim de sessão
//! (D12/D13). DESIGN_LOCKED.
//!
//! O login é em dois passos só no browser (`static/oc-auth.js`): sem
//! JavaScript aparecem o endereço e a palavra-passe juntos, e o formulário é
//! sempre um único `POST /login` — nenhum pedido entre os dois passos, para não
//! criar um oráculo de contas. Sem chave de acesso nem SSO à porta.

use leptos::prelude::*;

use super::{core_message, frame, Foot};
use crate::i18n::t;
use crate::ui::components::{icon, pending};
use crate::ui::view_models::{Health, LoginVm, RecoverVm, SessionEndReason, SessionEndVm};

/// `GET /login`.
pub fn login(vm: &LoginVm) -> impl IntoView {
    let down = vm.door.core == Some(Health::Unavailable);
    let email = vm.email.clone();
    let card = view! {
        {down.then(|| view! {
            <div class="oc-auth__banner" role="alert">
                {icon("warning")}
                <span>
                    <strong>{t("auth.instance.unavailable")}</strong>
                    <span>{t("auth.login.down_note")}</span>
                </span>
                <a href="/login">{t("auth.retry")}</a>
            </div>
        })}
        <h1 class="oc-auth__title">{t("auth.login.title")}</h1>
        <form class="oc-auth__form" method="post" action="/login" data-oc="login-steps">
            {core_message(&vm.error)}
            <button type="button" class="oc-auth__who" data-oc="login-change" data-part="step-pw" title=t("auth.login.change") hidden>
                <span class="oc-auth__who-initial" aria-hidden="true" data-part="login-initial"></span>
                <span data-part="login-email"></span>
                {icon("chev-d")}
            </button>
            <label class="oc-pill" data-part="step-id">
                <span class="oc-sr">{t("auth.email")}</span>
                {icon("user")}
                <input
                    id="login-email"
                    name="email"
                    type="email"
                    inputmode="email"
                    autocomplete="username"
                    autocapitalize="none"
                    spellcheck="false"
                    placeholder=t("auth.email")
                    value=email
                    required
                />
                <button
                    type="button"
                    class="oc-go"
                    data-oc="login-next"
                    aria-label=t("auth.login.next")
                    title=t("auth.login.next")
                    hidden
                    disabled=down
                >
                    {icon("arrow-r")}
                </button>
            </label>
            <label class="oc-pill" data-part="step-pw">
                <span class="oc-sr">{t("auth.password")}</span>
                {icon("lock")}
                <input
                    id="login-password"
                    name="password"
                    type="password"
                    autocomplete="current-password"
                    placeholder=t("auth.password")
                    required
                />
                <button
                    type="submit"
                    class="oc-go"
                    data-part="login-submit"
                    aria-label=t("auth.login.submit")
                    title=t("auth.login.submit")
                    disabled=down
                >
                    {icon("arrow-r")}
                </button>
            </label>
            <a class="oc-auth__link" data-part="step-pw" href="/password/recover">{t("auth.login.forgot")}</a>
        </form>
    }
    .into_any();
    frame(&vm.door, "login", false, card, Foot::Language("/login"))
}

/// `GET /password/recover` (G-26). Indisponível enquanto não houver
/// `POST /password/recover`; a confirmação neutra fica desenhada.
pub fn recover(vm: &RecoverVm) -> impl IntoView {
    let card = if vm.sent {
        view! {
            <span class="oc-auth__icon" data-tone="ok">{icon("mail")}</span>
            <h1 class="oc-auth__title">{t("auth.recover.sent_title")}</h1>
            <p class="oc-auth__lead">{t("auth.recover.sent_body")}</p>
            <a class="oc-btn oc-btn--gold" href="/login">{t("auth.back_login")}</a>
        }
        .into_any()
    } else {
        let off = !vm.available;
        let described = off.then_some("recover-pending");
        view! {
            <h1 class="oc-auth__title">{t("auth.recover.title")}</h1>
            <p class="oc-auth__lead">{t("auth.recover.body")}</p>
            <form class="oc-auth__form" method="post" action="/password/recover">
                <label class="oc-pill">
                    <span class="oc-sr">{t("auth.email")}</span>
                    {icon("mail")}
                    <input
                        id="recover-email"
                        name="email"
                        type="email"
                        autocomplete="username"
                        placeholder=t("auth.email")
                        required
                        disabled=off
                        aria-describedby=described
                    />
                </label>
                {if off {
                    view! {
                        <button type="button" class="oc-btn oc-btn--gold" aria-disabled="true" aria-describedby="recover-pending">
                            {t("auth.recover.send")}
                        </button>
                        {pending("recover-pending", "auth.recover.pending")}
                    }
                    .into_any()
                } else {
                    view! {
                        <button type="submit" class="oc-btn oc-btn--gold">{t("auth.recover.send")}</button>
                    }
                    .into_any()
                }}
            </form>
            <a class="oc-auth__link" href="/login">{t("auth.back_login")}</a>
        }
        .into_any()
    };
    frame(
        &vm.door,
        "recover",
        false,
        card,
        Foot::Language("/password/recover"),
    )
}

/// `GET /login?reason=expired|revoked`. Revogado não oferece entrada.
pub fn session_end(vm: &SessionEndVm) -> impl IntoView {
    let (state, ic, title, body) = match vm.reason {
        SessionEndReason::Expired => (
            "expired",
            "clock",
            "auth.end.expired_title",
            "auth.end.expired_body",
        ),
        SessionEndReason::Revoked => (
            "revoked",
            "shield",
            "auth.end.revoked_title",
            "auth.end.revoked_body",
        ),
    };
    let action = match vm.reason {
        SessionEndReason::Expired => view! {
            <a class="oc-btn oc-btn--gold" href="/login">{t("auth.end.sign_in_again")}{icon("arrow-r")}</a>
        }
        .into_any(),
        SessionEndReason::Revoked => view! {
            <a class="oc-btn oc-btn--line" href="/help">{icon("mail")}{t("auth.end.contact")}</a>
        }
        .into_any(),
    };
    let card = view! {
        <span class="oc-auth__icon" data-state=state>{icon(ic)}</span>
        <h1 class="oc-auth__title">{t(title)}</h1>
        <p class="oc-auth__lead">{t(body)}</p>
        {action}
    }
    .into_any();
    frame(
        &vm.door,
        "session-end",
        false,
        card,
        Foot::Language("/login"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::testing::assert_contracts;
    use crate::ui::view_models::{Distribution, DoorVm};

    fn door() -> DoorVm {
        DoorVm {
            distribution: Some(Distribution::Research),
            core: Some(Health::Operational),
        }
    }

    #[test]
    fn o_login_e_um_so_post_com_os_dois_campos() {
        let html = login(&LoginVm {
            door: door(),
            ..Default::default()
        })
        .to_html();
        assert_contracts(&html);
        assert_eq!(html.matches("<form").count(), 2, "login + idioma");
        assert!(html.contains(r#"action="/login" data-oc="login-steps""#));
        assert!(html.contains(r#"name="email""#) && html.contains(r#"name="password""#));
        assert!(html.contains(r#"autocomplete="current-password""#));
    }

    #[test]
    fn o_primeiro_submit_e_o_de_entrar() {
        let html = login(&LoginVm {
            door: door(),
            ..Default::default()
        })
        .to_html();
        let first = html.find(r#"type="submit""#).expect("submit");
        let enter = html.find(r#"data-part="login-submit""#).expect("entrar");
        assert!(first < enter && enter - first < 200);
    }

    #[test]
    fn sem_sso_nem_chave_de_acesso_nem_revelar_palavra_passe() {
        let html = login(&LoginVm {
            door: door(),
            ..Default::default()
        })
        .to_html();
        for x in [
            r#"data-oc="sso""#,
            "/sso",
            ">SSO<",
            "passkey",
            "webauthn",
            r#"data-oc="reveal""#,
        ] {
            assert!(!html.contains(x), "{x}");
        }
    }

    #[test]
    fn todos_os_campos_tem_rotulo() {
        let html = login(&LoginVm {
            door: door(),
            ..Default::default()
        })
        .to_html();
        for id in ["login-email", "login-password"] {
            let before = &html[..html.find(&format!(r#"id="{id}""#)).expect(id)];
            assert!(before.rfind("<label").unwrap_or(0) > before.rfind("</label>").unwrap_or(0));
        }
    }

    #[test]
    fn o_erro_preserva_o_endereco() {
        let vm = LoginVm {
            door: door(),
            error: Some("Credenciais inválidas.".into()),
            email: "a@b.pt".into(),
        };
        let html = login(&vm).to_html();
        assert!(html.contains(r#"role="alert""#) && html.contains("Credenciais inválidas."));
        assert!(html.contains(r#"value="a@b.pt""#));
    }

    #[test]
    fn instancia_em_baixo_impede_entrar() {
        let d = DoorVm {
            core: Some(Health::Unavailable),
            ..door()
        };
        let html = login(&LoginVm {
            door: d,
            ..Default::default()
        })
        .to_html();
        assert!(html.contains("oc-auth__banner") && html.contains("disabled"));
    }

    #[test]
    fn recuperar_sem_contrato_nao_submete_e_explica() {
        let html = recover(&RecoverVm {
            door: door(),
            available: false,
            sent: false,
        })
        .to_html();
        assert_contracts(&html);
        assert_eq!(
            html.matches(r#"aria-describedby="recover-pending""#)
                .count(),
            2
        );
        assert!(!html.contains(r#"<button type="submit" class="oc-btn"#));
        assert!(!html.contains(t("auth.recover.sent_body")));
    }

    #[test]
    fn revogado_nao_oferece_entrada() {
        let html = session_end(&SessionEndVm {
            door: door(),
            reason: SessionEndReason::Revoked,
        })
        .to_html();
        assert_contracts(&html);
        assert!(!html.contains(r#"action="/login""#));
    }
}
