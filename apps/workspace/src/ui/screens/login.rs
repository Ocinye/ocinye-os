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

/// O que a porta mostra da Instância antes de haver sessão (`GET
/// /instance/branding`, público).
#[derive(Debug, Clone, Default)]
pub struct Porta {
    /// O nome da Instância.
    pub nome: Option<String>,
    /// O perfil (`research`, `business`, …).
    pub perfil: Option<String>,
    /// O anfitrião por onde a pessoa chegou (`os.ocinye.com`).
    pub anfitriao: Option<String>,
}

fn icone(id: &'static str) -> impl IntoView {
    crate::ui::ods::icone(id, "")
}

/// O perfil como o protótipo o escreve: o nome do produto, igual nas três
/// línguas.
fn nome_do_perfil(perfil: &str) -> String {
    let mut c = perfil.chars();
    c.next()
        .map(|p| p.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// A casca dos ecrãs de entrada (D3, transcrita do protótipo): o estado do
/// Core e o relógio em cima, a marca, «OCINYE OS», a Instância, o perfil e o
/// anfitrião, e o cartão.
pub fn casca(core_ready: bool, porta: &Porta, cartao: impl IntoView + 'static) -> impl IntoView {
    casca_com(core_ready, porta, "", cartao)
}

/// A casca, com a variante do cartão (D16: `--state`, `data-state`).
fn casca_com(
    core_ready: bool,
    porta: &Porta,
    estado: &'static str,
    cartao: impl IntoView + 'static,
) -> impl IntoView {
    // Sem resposta do Core, o nome é o do produto: nunca se adivinha uma
    // instituição à porta.
    let nome = porta.nome.clone().unwrap_or_else(|| "Ocinye OS".to_owned());
    let perfil = porta.perfil.as_deref().map(nome_do_perfil);
    let anfitriao = porta.anfitriao.clone();
    view! {
        <main class="ods-auth" data-part="login">
            <div class="ods-auth__top">
                <span class="ods-auth__core" data-state=if core_ready { "ok" } else { "degraded" }>
                    <span class="ods-auth__core-dot" aria-hidden="true"></span>
                    {crate::i18n::t(if core_ready { "auth.core_operational" } else { "auth.state.core_deg" })}
                </span>
                <span class="ods-auth__clock" data-oc="clock"></span>
            </div>
            <span class="ods-auth__mark"><img src="/static/ocinye_logo.png" alt="Ocinye" /></span>
            <p class="ods-auth__os">{crate::i18n::t("auth.os_mark")}</p>
            <p class="ods-auth__inst">{nome}</p>
            {(perfil.is_some() || anfitriao.is_some()).then(|| view! {
                <p class="ods-auth__meta">
                    {perfil.map(|p| view! {
                        <span class="ods-auth__meta-k">{crate::i18n::t("auth.profile_word")}</span>
                        <span class="ods-auth__profile">{p}</span>
                    })}
                    {anfitriao.map(|h| view! { <span class="ods-auth__host">{format!("· {h}")}</span> })}
                </p>
            })}
            // D16 · D14: o Core que não está pronto diz-se numa faixa acima do
            // cartão, com o estado real — nunca «operacional» sem resposta.
            {(!core_ready).then(|| view! {
                <div class="ods-auth__banner" role="status" data-state="degraded">
                    <span class="ods-dot ods-dot--warning" aria-hidden="true"></span>
                    <strong>{crate::i18n::t("auth.state.core_deg")}</strong>
                    <span>{crate::i18n::t("auth.state.degraded_b")}</span>
                    <a class="ods-btn ods-btn--ghost ods-btn--sm" href="/login">{crate::i18n::t("auth.state.retry")}</a>
                </div>
            })}
            <section
                class=if estado.is_empty() { "ods-auth__card" } else { "ods-auth__card ods-auth__card--state" }
                data-state=(!estado.is_empty()).then_some(estado)
            >
                {cartao}
            </section>
        </main>
    }
}

/// O rodapé do cartão: a instalação soberana e o idioma (pt/en/fr).
///
/// O idioma muda-se antes de haver sessão por um formulário público que só grava
/// o cookie de idioma e volta à página (`POST /login/language`).
fn rodape(voltar_a: &'static str) -> impl IntoView {
    let actual = crate::i18n::current();
    view! {
        <div class="ods-auth__foot">
            <span class="ods-auth__sovereign">{crate::i18n::t("auth.sovereign")}</span>
            <form method="post" action="/login/language" class="ods-auth__langs" aria-label=crate::i18n::t("auth.language")>
                <input type="hidden" name="return_to" value=voltar_a />
                {ocinye_contracts::locale::Locale::ALL
                    .into_iter()
                    .map(|loc| {
                        let codigo = loc.as_str();
                        view! {
                            <button
                                type="submit"
                                class="ods-auth__lang"
                                name="lang"
                                value=codigo
                                lang=codigo
                                aria-pressed=if loc == actual { "true" } else { "false" }
                            >
                                {codigo}
                            </button>
                        }
                    })
                    .collect_view()}
            </form>
        </div>
    }
}

/// O ecrã de login.
///
/// `core_ready` reflecte uma sonda real ao Ocinye Core: sem ele, autenticar não
/// leva a lado nenhum, e é melhor dizê-lo antes do que falhar depois.
pub fn login(core_ready: bool, message: Option<String>, porta: Porta) -> impl IntoView {
    let cartao = view! {
        <h1 class="ods-auth__title">{crate::i18n::t("login.sign_in")}</h1>

        <form method="post" action="/login">
            <label class="ods-auth__field" for="login-user">
                <span class="ods-auth__label">{crate::i18n::t("login.id_label")}</span>
                <span class="ods-auth__input">
                    {icone("user")}
                    <input
                        id="login-user"
                        name="email"
                        type="email"
                        inputmode="email"
                        autocomplete="username"
                        autocapitalize="none"
                        spellcheck="false"
                        required
                    />
                </span>
            </label>
            <label class="ods-auth__field" for="login-pass">
                <span class="ods-auth__label">{crate::i18n::t("auth.password")}</span>
                <span class="ods-auth__input">
                    {icone("lock")}
                    <input
                        id="login-pass"
                        name="password"
                        type="password"
                        autocomplete="current-password"
                        required
                    />
                </span>
            </label>

            <div class="ods-auth__forgot">
                <a href="/password/recover">{crate::i18n::t("login.forgot")}</a>
            </div>

            // O erro é o texto que o Core devolveu, nunca um genérico.
            {message.map(|texto| view! {
                <p class="ods-field__error" role="alert">{texto}</p>
            })}

            <button
                type="submit"
                class="ods-auth__submit"
                data-part="login__submit"
            >
                {crate::i18n::t("login.sign_in")}
                {icone("arrow-r")}
            </button>
        </form>

        <div class="ods-auth__or">{crate::i18n::t("login.or")}</div>
        <div class="ods-auth__alt">
            // As chaves de acesso (WebAuthn) estão decididas e não existem
            // (CLAUDE.md §33): o botão é o do desenho, no estado indisponível do
            // pacote (contrato em falta).
            <button
                type="button"
                class="ods-auth__alt-btn ods-state--unavailable"
                aria-disabled="true"
                title=crate::i18n::t("ods.state.pending_contract")
                data-tip=crate::i18n::t("ods.state.pending_contract")
            >
                {icone("key")}
                {crate::i18n::t("auth.passkey")}
            </button>
        </div>

        {rodape("/login")}
    };
    casca(core_ready, &porta, cartao)
}

/// D16 · D10 — «Esqueceu a palavra-passe?» (`GET /password/recover`).
///
/// O envio (`POST /password/recover`, G-26) ainda não existe no Core: o cartão
/// é o do desenho, com o formulário no estado indisponível do pacote
/// (`ods-state--unavailable` + contrato em falta). Nada se recolhe.
pub fn recover(core_ready: bool, porta: Porta) -> impl IntoView {
    let cartao = view! {
        <span class="ods-auth__icon">{icone("key")}</span>
        <h1 class="ods-auth__title">{crate::i18n::t("auth.state.recover_t")}</h1>
        <p class="ods-auth__lead">{crate::i18n::t("auth.state.recover_b")}</p>
        <form method="post" action="/password/recover" class="ods-state--unavailable" aria-disabled="true">
            <label class="ods-auth__field" for="recover-email">
                <span class="ods-auth__label">{crate::i18n::t("login.id_label")}</span>
                <span class="ods-auth__input">
                    {icone("mail")}
                    <input id="recover-email" type="email" autocomplete="username" disabled />
                </span>
            </label>
            <p class="ods-auth__lead" role="note">{crate::i18n::t("ods.state.pending_contract")}</p>
            <button type="submit" class="ods-auth__submit" disabled>
                {crate::i18n::t("auth.state.send")}
            </button>
        </form>
        <div class="ods-auth__forgot">
            <a class="ods-auth__back" href="/login">{crate::i18n::t("auth.state.back_login")}</a>
        </div>
        {rodape("/password/recover")}
    };
    casca_com(core_ready, &porta, "recover", cartao)
}

/// D16 · D12 — a sessão expirou (`GET /login?reason=expired`).
///
/// O desenho diz «após 30 minutos de inactividade»; a sessão do Ocinye dura o
/// que `OCINYE_WORKSPACE_SESSION_TTL_SECONDS` diz, e não expira por
/// inactividade. A frase diz o que é verdade (Q-43).
pub fn expired(core_ready: bool, porta: Porta) -> impl IntoView {
    let cartao = view! {
        <span class="ods-auth__icon">{icone("clock")}</span>
        <h1 class="ods-auth__title">{crate::i18n::t("auth.state.expired_t")}</h1>
        <p class="ods-auth__lead">{crate::i18n::t("login.expired_body")}</p>
        <a class="ods-auth__submit" href="/login">
            {crate::i18n::t("auth.state.sign_in_again")}
            {icone("arrow-r")}
        </a>
        {rodape("/login")}
    };
    casca_com(core_ready, &porta, "expired", cartao)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_login_nao_pede_mfa_nem_oferece_registo() {
        let html = login(true, None, Porta::default()).to_html();
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
        let html = login(true, None, Porta::default()).to_html();
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
        let html = login(true, None, Porta::default()).to_html();
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
        let html = login(true, None, Porta::default()).to_html();
        assert!(!html.contains("Mostrar"));
        assert!(!html.contains(r#"data-oc="reveal""#));
    }

    #[test]
    fn o_ecra_nao_promete_mfa_nem_recuperacao_automatica() {
        let html = login(true, None, Porta::default()).to_html().to_lowercase();
        for ausente in ["mfa", "autenticação de dois"] {
            assert!(!html.contains(ausente), "o login promete «{ausente}»");
        }
        // «Esqueceu a palavra-passe?» existe (D3/D16) e leva ao cartão do
        // desenho. O envio (G-26) não existe: o formulário está indisponível,
        // não submete nada, e di-lo (contrato em falta).
        assert!(html.contains(r#"href="/password/recover""#));
        let recuperar = recover(true, Porta::default()).to_html();
        assert!(
            recuperar.contains("ods-state--unavailable"),
            "o envio finge existir"
        );
        assert!(
            !recuperar.contains(r#"name="email""#),
            "recolhe um endereço que não usa"
        );
        let enviar = &recuperar[recuperar.rfind(r#"type="submit""#).expect("botão")..];
        assert!(
            enviar[..enviar.find('>').unwrap()].contains("disabled"),
            "o envio está activo"
        );
        assert!(
            recuperar.contains(crate::i18n::t("ods.state.pending_contract")),
            "não diz porque está indisponível"
        );
    }

    /// As chaves de acesso estão no desenho e não existem: o botão declara-se
    /// indisponível, e não finge.
    #[test]
    fn a_chave_de_acesso_declara_se_indisponivel() {
        let html = login(true, None, Porta::default()).to_html();
        let fim = html
            .find("ods-auth__alt-btn")
            .expect("o botão da chave de acesso");
        let inicio = html[..fim].rfind("<button").expect("<button");
        let botao = &html[inicio..fim + html[fim..].find('>').unwrap_or(0)];
        assert!(botao.contains(r#"aria-disabled="true""#), "{botao}");
    }

    /// A porta diz a Instância, o perfil e o anfitrião, como o protótipo.
    #[test]
    fn a_porta_diz_instancia_perfil_e_anfitriao() {
        let porta = Porta {
            nome: Some("Ocinye".into()),
            perfil: Some("research".into()),
            anfitriao: Some("os.ocinye.com".into()),
        };
        let html = login(true, None, porta).to_html();
        for esperado in [
            "OCINYE OS",
            "Ocinye",
            "Research",
            "os.ocinye.com",
            "OCINYE CORE · OPERACIONAL",
        ] {
            assert!(html.contains(esperado), "falta «{esperado}»");
        }
    }

    #[test]
    fn com_o_core_em_baixo_o_ecra_diz_o_numa_faixa() {
        // D16 · D14: o estado real numa faixa acima do cartão, e o login
        // continua disponível — o Core diz o resto se não puder autenticar.
        let html = login(false, None, Porta::default()).to_html();
        assert!(
            html.contains(r#"data-state="degraded""#),
            "sem faixa de estado"
        );
        assert!(
            html.contains("DEGRADADO"),
            "diz «operacional» sem resposta real"
        );
        assert!(!html.contains("OPERACIONAL"));
    }

    #[test]
    fn todos_os_campos_tem_rotulo() {
        // O rótulo envolve o campo (D1): cada campo está dentro do seu `<label>`.
        let html = login(true, None, Porta::default()).to_html();
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
        let fr = with_locale(Locale::Fr, async {
            login(true, None, Porta::default()).to_html()
        })
        .await;
        for francesa in [
            "Se connecter",
            "Adresse électronique ou identifiant",
            "Mot de passe",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        assert!(!fr.contains("Iniciar sessão"), "fr: chrome português");
    }
}
