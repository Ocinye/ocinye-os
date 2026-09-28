//! Ecrãs de autenticação: login, fim de sessão, recuperação, escolha de espaço.
//!
//! DESIGN_LOCKED · Proposta D7 (login), D9 (escolher espaço), D10 (recuperar
//! palavra-passe), D12 (sessão expirada), D13 (acesso revogado), D14 (Core
//! indisponível), P1–P4 (a mesma porta nos quatro perfis).
//! A estrutura e as classes são do Claude Design; o Claude Code liga dados e
//! rotas à volta destas vistas, sem as reescrever (`docs/ui/DESIGN_LOCK.md`).
//!
//! # Controlos sem contrato
//!
//! A chave de acesso e o SSO estão no desenho aprovado e aparecem, com
//! `aria-disabled` e o estado indisponível descrito, até existir ADR. Não se
//! escondem e não fingem funcionar.
//!
//! # Quem decide
//!
//! Estes ecrãs recolhem credenciais e enviam-nas ao Ocinye Core. Não validam,
//! não comparam e não decidem: a autoridade de autenticação é o Core.

use leptos::prelude::*;
use ocinye_contracts::application::InstanceProfile;

use crate::ui::ods;

/// O que se sabe da instância à porta (`GET /api/v1/instance/branding` + o
/// anfitrião do pedido). Tudo opcional: sem resposta do Core não se inventa.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Porta {
    /// O nome público da instância.
    pub nome: Option<String>,
    /// O perfil da instância (G-31).
    pub perfil: Option<InstanceProfile>,
    /// O endereço por onde se chegou, sem esquema (`os.ocinye.com`).
    pub host: Option<String>,
}

impl Porta {
    /// Só o nome, como o Workspace o conhecia antes de G-31.
    #[must_use]
    pub fn so_nome(nome: Option<String>) -> Self {
        Self {
            nome,
            ..Self::default()
        }
    }
}

/// A barra de topo comum a todos os ecrãs de autenticação: estado do Core à
/// esquerda (quando se sabe), data e hora à direita.
///
/// `core` é `None` quando o ecrã não sondou o Core (MFA, primeiro acesso): não
/// se afirma um estado que não se mediu.
pub fn barra(core: Option<bool>) -> impl IntoView {
    view! {
        <header class="ods-auth__bar">
            {core.map(|pronto| {
                let (estado, texto) = if pronto {
                    ("ok", crate::i18n::t("login.core.operational"))
                } else {
                    ("down", crate::i18n::t("login.core.unavailable"))
                };
                view! {
                    <span class="ods-auth__core" data-state=estado>
                        <span class="ods-auth__core-dot" aria-hidden="true"></span>
                        {texto}
                    </span>
                }
            })}
            <span class="ods-auth__clock" data-oc="clock"></span>
        </header>
    }
}

/// A identidade da instância por cima do cartão: logótipo, produto, nome e,
/// quando se sabem, o perfil e o endereço.
pub fn identidade(porta: &Porta) -> impl IntoView {
    let nome = porta.nome.clone();
    let perfil = porta.perfil;
    let host = porta.host.clone();
    let tem_meta = perfil.is_some() || host.is_some();
    view! {
        <span class="ods-auth__mark"><img src="/static/ocinye_logo.png" alt="" /></span>
        <p class="ods-auth__product">{crate::i18n::t("auth.product")}</p>
        {nome.map(|n| view! { <p class="ods-auth__inst">{n}</p> })}
        {tem_meta.then(|| view! {
            <p class="ods-auth__meta">
                {perfil.map(|p| view! {
                    <span class="ods-auth__meta-word">{crate::i18n::t("auth.profile_word")}</span>
                    <span class="ods-auth__profile" data-profile=p.as_str()>{nome_do_perfil(p)}</span>
                })}
                {host.map(|h| view! { <span class="ods-auth__host">{h}</span> })}
            </p>
        })}
    }
}

/// O nome do perfil como o D7 o escreve: a marca do perfil, igual nas três
/// línguas (`Research`, `Business`, `Education`, `Personal`).
fn nome_do_perfil(perfil: InstanceProfile) -> String {
    let id = perfil.as_str();
    let mut letras = id.chars();
    letras
        .next()
        .map(|c| c.to_uppercase().chain(letras).collect())
        .unwrap_or_default()
}

/// O rodapé do cartão: a frase de soberania (`auth.instance_line`, D7) e o seletor de idioma
/// (`POST /login/language`, G-30). `return_to` é o caminho a que se volta.
pub fn rodape(return_to: &str) -> impl IntoView {
    let actual = crate::i18n::current().as_str();
    let volta = return_to.to_owned();
    view! {
        <div class="ods-auth__foot">
            <span class="ods-auth__sovereign">{crate::i18n::t("auth.instance_line")}</span>
            <form class="ods-auth__langs" method="post" action="/login/language" aria-label=crate::i18n::t("auth.langs")>
                <input type="hidden" name="return_to" value=volta />
                {["pt", "en", "fr"].into_iter().map(|codigo| {
                    let activo = codigo == actual;
                    view! {
                        <button
                            type="submit"
                            class="ods-auth__lang"
                            name="lang"
                            value=codigo
                            lang=codigo
                            aria-pressed=if activo { "true" } else { "false" }
                        >
                            {codigo}
                        </button>
                    }
                }).collect_view()}
            </form>
        </div>
    }
}

/// Uma acção desenhada que ainda não tem contrato: aparece, não actua, e diz
/// porquê a quem usa leitor de ecrã.
fn indisponivel(id: &'static str, icone_id: &'static str, rotulo: &'static str) -> impl IntoView {
    let nota = format!("{id}-pending");
    view! {
        <button
            type="button"
            class="ods-btn ods-btn--block ods-auth__secondary"
            data-oc=id
            aria-disabled="true"
            aria-describedby=nota.clone()
            title=crate::i18n::t("ods.state.pending_contract")
        >
            {ods::icone(icone_id, "")}
            {rotulo}
        </button>
        <span class="ods-sr-only" id=nota>{crate::i18n::t("ods.state.pending_contract")}</span>
    }
}

/// O ecrã de login, com o que o Workspace conhecia antes de G-31.
///
/// Mantido para as chamadas existentes em `routes.rs`; o Claude Code passa
/// para [`login_na_porta`] quando ligar o perfil e o endereço.
pub fn login(
    core_ready: bool,
    message: Option<String>,
    instancia: Option<String>,
) -> impl IntoView {
    login_na_porta(core_ready, message, &Porta::so_nome(instancia))
}

/// O ecrã de login (D7).
///
/// `core_ready` reflecte uma sonda real ao Ocinye Core: sem ele, autenticar não
/// leva a lado nenhum, e é melhor dizê-lo antes do que falhar depois.
pub fn login_na_porta(core_ready: bool, message: Option<String>, porta: &Porta) -> impl IntoView {
    let empresa = porta.perfil == Some(InstanceProfile::Business);
    view! {
        <main class="ods-auth" data-part="login">
            {barra(Some(core_ready))}
            <div class="ods-auth__stage">
                {identidade(porta)}

                <section class="ods-auth__card">
                    // O estado do Core antes de tentar (D14).
                    {(!core_ready).then(|| view! {
                        <div class="ods-auth__banner" role="alert">
                            {ods::icone("warning", "ods-auth__banner-icon")}
                            <span class="ods-auth__banner-text">
                                <strong>{crate::i18n::t("login.core.unavailable")}</strong>
                                <span>{crate::i18n::t("login.core_down_note")}</span>
                            </span>
                            <a class="ods-auth__banner-link" href="/login">{crate::i18n::t("login.core.retry")}</a>
                        </div>
                    })}

                    <h1 class="ods-auth__title">{crate::i18n::t("login.sign_in")}</h1>

                    <form class="ods-auth__form" method="post" action="/login">
                        <label class="ods-field ods-auth__field">
                            <span class="ods-field__label">{crate::i18n::t("login.institutional_address")}</span>
                            <span class="ods-auth__input">
                                {ods::icone("user", "")}
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
                            </span>
                        </label>
                        <label class="ods-field ods-auth__field">
                            <span class="ods-field__label">{crate::i18n::t("auth.password")}</span>
                            <span class="ods-auth__input">
                                {ods::icone("lock", "")}
                                <input
                                    class="ods-input"
                                    id="login-pass"
                                    name="password"
                                    type="password"
                                    autocomplete="current-password"
                                    required
                                />
                            </span>
                        </label>
                        <a class="ods-auth__forgot" href="/password/recover">{crate::i18n::t("login.forgot")}</a>

                        // O erro é o texto que o Core devolveu, nunca um genérico.
                        {message.map(|texto| view! {
                            <p class="ods-field__error" role="alert">{texto}</p>
                        })}

                        <button
                            type="submit"
                            class="ods-btn ods-btn--primary ods-btn--block ods-auth__submit"
                            data-part="login__submit"
                            disabled=!core_ready
                        >
                            {crate::i18n::t("login.sign_in")}
                            {ods::icone("arrow-r", "")}
                        </button>
                    </form>

                    <p class="ods-auth__or"><span>{crate::i18n::t("login.or")}</span></p>
                    <div class="ods-auth__alt">
                        {indisponivel("passkey", "key", crate::i18n::t("auth.passkey"))}
                        {empresa.then(|| indisponivel("sso", "shield", crate::i18n::t("auth.sso")))}
                    </div>

                    {rodape("/login")}
                </section>
            </div>
        </main>
    }
}

/// Porque terminou a sessão (`/login?reason=…`, G-27).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FimDeSessao {
    /// Terminou por inactividade (D12): o cookie de sessão já não é conhecido.
    Expirada,
    /// O acesso foi retirado por um administrador (D13).
    Revogada,
}

/// Quem estava na sessão que expirou, se o Workspace ainda o sabe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuemEstava {
    /// Nome a mostrar.
    pub nome: String,
    /// O espaço de trabalho em que estava.
    pub espaco: Option<String>,
}

/// O cartão de fim de sessão (D12 / D13). Revogado não oferece formulário.
pub fn fim_de_sessao(
    motivo: FimDeSessao,
    porta: &Porta,
    quem: Option<QuemEstava>,
) -> impl IntoView {
    let (estado, icone_id, titulo, corpo) = match motivo {
        FimDeSessao::Expirada => (
            "expired",
            "clock",
            crate::i18n::t("auth.state.expired_t"),
            crate::i18n::t("auth.state.expired_b"),
        ),
        FimDeSessao::Revogada => (
            "revoked",
            "shield",
            crate::i18n::t("auth.state.revoked_t"),
            crate::i18n::t("auth.state.revoked_b"),
        ),
    };
    let pessoa = match motivo {
        FimDeSessao::Expirada => quem.map(|q| {
            let iniciais = crate::ui::initials(&q.nome);
            view! {
                <div class="ods-auth__who">
                    <span class="ods-avatar ods-avatar--sm" aria-hidden="true">{iniciais}</span>
                    <span class="ods-auth__who-text">
                        <span class="ods-auth__who-name">{q.nome}</span>
                        {q.espaco.map(|e| view! { <span class="ods-auth__who-meta">{e}</span> })}
                    </span>
                </div>
            }
        }),
        FimDeSessao::Revogada => None,
    };
    let accao = match motivo {
        FimDeSessao::Expirada => view! {
            <a class="ods-btn ods-btn--primary ods-btn--block ods-auth__submit" href="/login">
                {crate::i18n::t("auth.state.sign_in_again")}
                {ods::icone("arrow-r", "")}
            </a>
        }
        .into_any(),
        FimDeSessao::Revogada => view! {
            <a class="ods-btn ods-btn--block ods-auth__secondary" href="/help">
                {ods::icone("mail", "")}
                {crate::i18n::t("auth.state.contact")}
            </a>
            <a class="ods-auth__link" href="/login">{crate::i18n::t("auth.state.back_login")}</a>
        }
        .into_any(),
    };
    view! {
        <main class="ods-auth" data-part="login">
            {barra(None)}
            <div class="ods-auth__stage">
                {identidade(porta)}
                <section class="ods-auth__card ods-auth__card--state" data-state=estado>
                    <span class="ods-auth__icon">{ods::icone(icone_id, "ods-icon--lg")}</span>
                    <h1 class="ods-auth__title">{titulo}</h1>
                    <p class="ods-auth__lead">{corpo}</p>
                    {pessoa}
                    {accao}
                    {rodape("/login")}
                </section>
            </div>
        </main>
    }
}

/// Recuperar palavra-passe (D10, `GET /password/recover`, G-26).
///
/// `disponivel` é `false` enquanto o `POST` não existir: o formulário aparece,
/// não submete, e o estado indisponível diz porquê. A resposta ao envio é
/// sempre neutra: `enviado` mostra a mesma confirmação exista ou não a conta.
pub fn recover(enviado: bool, disponivel: bool, porta: &Porta) -> impl IntoView {
    let corpo = if enviado {
        view! {
            <span class="ods-auth__icon" data-tone="ok">{ods::icone("mail", "ods-icon--lg")}</span>
            <h1 class="ods-auth__title">{crate::i18n::t("auth.state.sent_t")}</h1>
            <p class="ods-auth__lead">{crate::i18n::t("auth.state.sent_b")}</p>
            <a class="ods-btn ods-btn--primary ods-btn--block ods-auth__submit" href="/login">
                {crate::i18n::t("auth.state.back_login")}
            </a>
        }
        .into_any()
    } else {
        view! {
            <span class="ods-auth__icon">{ods::icone("lock", "ods-icon--lg")}</span>
            <h1 class="ods-auth__title">{crate::i18n::t("auth.state.recover_t")}</h1>
            <p class="ods-auth__lead">{crate::i18n::t("auth.state.recover_b")}</p>
            {(!disponivel).then(ods::a_espera_de_contrato)}
            <form class="ods-auth__form" method="post" action="/password/recover">
                <label class="ods-field ods-auth__field">
                    <span class="ods-field__label">{crate::i18n::t("login.institutional_address")}</span>
                    <span class="ods-auth__input">
                        {ods::icone("mail", "")}
                        <input class="ods-input" id="recover-email" name="email" type="email" autocomplete="username" required disabled=!disponivel />
                    </span>
                </label>
                <button
                    type="submit"
                    class="ods-btn ods-btn--primary ods-btn--block ods-auth__submit"
                    data-part="recover__submit"
                    disabled=!disponivel
                >
                    {crate::i18n::t("auth.state.send")}
                </button>
            </form>
            <a class="ods-auth__link" href="/login">{crate::i18n::t("auth.state.back_login")}</a>
        }
        .into_any()
    };
    view! {
        <main class="ods-auth" data-part="recover">
            {barra(None)}
            <div class="ods-auth__stage">
                {identidade(porta)}
                <section class="ods-auth__card ods-auth__card--state">
                    {corpo}
                    {rodape("/password/recover")}
                </section>
            </div>
        </main>
    }
}

/// Um espaço de trabalho que o Core diz que o membro pode abrir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EspacoVista {
    /// O identificador (`/workspaces/{id}`).
    pub id: String,
    /// O nome.
    pub nome: String,
    /// O tipo, já traduzido (`UNIDADE`, `EQUIPA`, …).
    pub tipo: String,
    /// A linha de contexto (`9 membros`).
    pub meta: String,
    /// O símbolo do sprite.
    pub icone: &'static str,
}

/// Escolher espaço de trabalho (D9). Só se serve com mais de um espaço.
///
/// «Lembrar» espera por um contrato de preferência por dispositivo: aparece
/// desligado, com o estado descrito.
pub fn escolher_espaco(espacos: &[EspacoVista], porta: &Porta) -> impl IntoView {
    let lead = crate::i18n::tf("auth.ws.lead", &[("n", &espacos.len().to_string())]);
    let linhas = espacos
        .iter()
        .map(|e| {
            let href = format!("/workspaces/{}", e.id);
            let icone_id = e.icone;
            let nome = e.nome.clone();
            let tipo = e.tipo.clone();
            let meta = e.meta.clone();
            view! {
                <li>
                    <a class="ods-ws" href=href data-oc="workspace-pick">
                        <span class="ods-ws__icon">{ods::icone(icone_id, "")}</span>
                        <span class="ods-ws__text">
                            <span class="ods-ws__name">{nome}</span>
                            <span class="ods-ws__meta">{tipo}" · "{meta}</span>
                        </span>
                        {ods::icone("chev-r", "ods-ws__chev")}
                    </a>
                </li>
            }
        })
        .collect_view();
    view! {
        <main class="ods-auth" data-part="workspace-pick">
            {barra(None)}
            <div class="ods-auth__stage">
                {identidade(porta)}
                <section class="ods-auth__card">
                    <h1 class="ods-auth__title">{crate::i18n::t("auth.ws.title")}</h1>
                    <p class="ods-auth__lead">{lead}</p>
                    <ul class="ods-ws-list">{linhas}</ul>
                    <label class="ods-auth__ack" aria-disabled="true" title=crate::i18n::t("ods.state.pending_contract")>
                        <input type="checkbox" class="ods-check" name="remember" value="1" disabled />
                        <span>{crate::i18n::t("auth.ws.remember")}</span>
                    </label>
                    {rodape("/login")}
                </section>
            </div>
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn porta_empresa() -> Porta {
        Porta {
            nome: Some("Example Company".to_owned()),
            perfil: Some(InstanceProfile::Business),
            host: Some("os.example.com".to_owned()),
        }
    }

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
        let html = login(true, None, None).to_html();
        assert!(html.contains(r#"method="post""#));
        assert!(html.contains(r#"action="/login""#));
        assert!(html.contains(r#"name="email""#));
        assert!(html.contains(r#"type="email""#));
        assert!(html.contains(r#"name="password""#));
        assert!(html.contains(r#"type="password""#));
    }

    #[test]
    fn o_primeiro_botao_de_submeter_e_o_de_entrar() {
        // As viagens de browser procuram o primeiro `button[type=submit]`.
        let html = login_na_porta(true, None, &porta_empresa()).to_html();
        let submit = html.find(r#"type="submit""#).expect("submit");
        let entrar = html.find(r#"data-part="login__submit""#).expect("entrar");
        assert!(submit < entrar);
        assert!(!html[submit..entrar].contains("<button"));
        assert!(html.find("/login/language").expect("idioma") > entrar);
    }

    #[test]
    fn gestores_de_palavras_passe_e_colar_funcionam() {
        let html = login(true, None, None).to_html();
        assert!(html.contains(r#"autocomplete="current-password""#));
        assert!(html.contains(r#"autocomplete="username""#));
        assert!(!html.contains("onpaste"));
        assert!(!html.contains("maxlength"));
    }

    #[test]
    fn o_login_nao_revela_a_palavra_passe() {
        let html = login(true, None, None).to_html();
        assert!(!html.contains("Mostrar"));
        assert!(!html.contains(r#"data-oc="reveal""#));
    }

    #[test]
    fn a_recuperacao_e_um_caminho_e_nao_uma_promessa_automatica() {
        let html = login(true, None, None).to_html();
        assert!(html.contains(r#"href="/password/recover""#));
        let baixo = html.to_lowercase();
        for ausente in ["mfa", "autenticação de dois"] {
            assert!(!baixo.contains(ausente), "o login promete «{ausente}»");
        }
    }

    #[test]
    fn chave_de_acesso_e_sso_aparecem_sem_fingir() {
        let pessoal = login(true, None, None).to_html();
        assert!(pessoal.contains(r#"data-oc="passkey""#));
        assert!(
            !pessoal.contains(r#"data-oc="sso""#),
            "SSO só no perfil empresa"
        );
        let empresa = login_na_porta(true, None, &porta_empresa()).to_html();
        assert!(empresa.contains(r#"data-oc="sso""#));
        assert_eq!(empresa.matches(r#"aria-disabled="true""#).count(), 2);
    }

    #[test]
    fn o_seletor_de_idioma_submete_a_rota_de_idioma() {
        let html = login(true, None, None).to_html();
        assert!(html.contains(r#"action="/login/language""#));
        assert!(html.contains(r#"name="return_to" value="/login""#));
        assert_eq!(html.matches(r#"name="lang""#).count(), 3);
        assert_eq!(html.matches(r#"aria-pressed="true""#).count(), 1);
    }

    #[test]
    fn o_rodape_diz_a_instalacao_soberana_e_nao_nomeia_uma_organizacao() {
        let html = login(true, None, None).to_html();
        assert!(html.contains(crate::i18n::t("auth.instance_line")));
        assert!(!html.contains(crate::i18n::t("login.granted_by_admin")));
        assert_eq!(
            html.matches(crate::i18n::t("auth.instance_line")).count(),
            1
        );
    }

    #[test]
    fn o_perfil_escreve_se_como_no_d7() {
        let html = login_na_porta(true, None, &porta_empresa()).to_html();
        assert!(html.contains(">Business<"));
        assert!(html.contains("#ods-key"));
    }

    #[test]
    fn com_o_core_em_baixo_o_ecra_diz_o_e_impede_submeter() {
        let html = login(false, None, None).to_html();
        assert!(html.contains("não está acessível"));
        assert!(html.contains(r#"data-state="down""#));
        assert!(html.contains("disabled"));
    }

    #[test]
    fn com_o_core_pronto_a_barra_diz_operacional() {
        let html = login(true, None, None).to_html();
        assert!(html.contains(r#"data-state="ok""#));
        assert!(html.contains(crate::i18n::t("login.core.operational")));
        assert!(html.contains(r#"data-oc="clock""#));
    }

    #[test]
    fn a_porta_mostra_perfil_e_endereco_quando_se_sabem() {
        let sem = login(true, None, None).to_html();
        assert!(!sem.contains("ods-auth__inst"));
        assert!(!sem.contains("ods-auth__profile"));
        let com = login_na_porta(true, None, &porta_empresa()).to_html();
        assert!(com.contains("Example Company"));
        assert!(com.contains(r#"data-profile="business""#));
        assert!(com.contains("os.example.com"));
    }

    #[test]
    fn todos_os_campos_tem_rotulo() {
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

    #[test]
    fn revogado_nao_oferece_formulario_de_entrada() {
        let html = fim_de_sessao(FimDeSessao::Revogada, &Porta::default(), None).to_html();
        assert!(!html.contains(r#"action="/login""#));
        assert!(html.contains(r#"data-state="revoked""#));
    }

    #[test]
    fn recuperar_sem_contrato_nao_submete() {
        let html = recover(false, false, &Porta::default()).to_html();
        assert!(html.contains("ods-state--unavailable"));
        assert!(html.contains("disabled"));
        let ligado = recover(false, true, &Porta::default()).to_html();
        assert!(!ligado.contains("ods-state--unavailable"));
    }

    #[test]
    fn as_vistas_nao_escrevem_estilo() {
        let atributo = ["sty", "le="].concat();
        let espacos = vec![EspacoVista {
            id: "u1".to_owned(),
            nome: "UENR-001".to_owned(),
            tipo: "UNIDADE".to_owned(),
            meta: "9".to_owned(),
            icone: "units",
        }];
        for html in [
            login(false, Some("x".to_owned()), Some("y".to_owned())).to_html(),
            login_na_porta(true, None, &porta_empresa()).to_html(),
            fim_de_sessao(
                FimDeSessao::Expirada,
                &Porta::default(),
                Some(QuemEstava {
                    nome: "Fidel Monteiro".to_owned(),
                    espaco: None,
                }),
            )
            .to_html(),
            recover(false, false, &Porta::default()).to_html(),
            recover(true, true, &Porta::default()).to_html(),
            escolher_espaco(&espacos, &Porta::default()).to_html(),
        ] {
            assert!(!html.contains(&atributo));
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
        for francesa in [
            "Se connecter",
            "Adresse institutionnelle",
            "Mot de passe",
            "OPÉRATIONNEL",
            "Mot de passe oublié",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        assert!(!fr.contains("Iniciar sessão"), "fr: chrome português");
        assert!(fr.contains(r#"lang="fr" aria-pressed="true""#));
    }
}
