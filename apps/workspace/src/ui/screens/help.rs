//! Ajuda do Ocinye Workspace.
//!
//! # Para quem isto é escrito
//!
//! Para quem **usa** o Ocinye OS, não para quem o constrói. Nada aqui aponta
//! para `docs/`, para o repositório ou para decisões de implementação: um
//! membro que procura saber porque uma tabela está vazia não precisa de saber
//! que módulo a serve.
//!
//! # Porque é conteúdo e não um sistema
//!
//! É texto versionado com o código, e é de propósito. A ajuda descreve o
//! produto **que existe hoje**, e por isso muda quando o produto muda — no
//! mesmo commit, revista pelas mesmas pessoas. Um CMS separado envelheceria por
//! sua conta, e uma ajuda desactualizada é pior do que nenhuma: descreve com
//! confiança um sistema que já não é aquele.
//!
//! Não há pesquisa aqui. As âncoras chegam para este tamanho, e uma caixa de
//! pesquisa que filtrasse texto estático prometeria uma capacidade que não
//! existe.

use leptos::prelude::*;

use crate::ui::components::section_head;

/// Uma secção da ajuda, com âncora própria.
fn seccao(
    ancora: &'static str,
    titulo: &'static str,
    corpo: impl IntoView + 'static,
) -> impl IntoView {
    view! {
        <section class="oc-card oc-mb-5" id=ancora>
            {section_head(titulo, None, None)}
            <div class="oc-card__body">{corpo}</div>
        </section>
    }
}

/// Um parágrafo de ajuda.
fn p(texto: &'static str) -> impl IntoView {
    view! { <p class="oc-t-prose oc-mb-5">{texto}</p> }
}

/// Uma entrada de glossário: o estado, e o que significa de facto.
fn estado(nome: &'static str, significado: &'static str) -> impl IntoView {
    view! {
        <div class="oc-list__row">
            <span class="oc-badge oc-badge--gray">{nome}</span>
            <span class="oc-fill oc-t-cell-2">{significado}</span>
        </div>
    }
}

/// O ecrã de ajuda.
pub fn help() -> impl IntoView {
    view! {
        <div class="oc-page oc-page--narrow">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("help.title")}</h1>
                    <p>{crate::i18n::t("help.subtitle")}</p>
                </div>
            </div>

            <nav class="oc-card oc-card__body oc-mb-5" aria-label=crate::i18n::t("help.on_this_page")>
                <div class="oc-t-meta oc-mb-5">{crate::i18n::t("help.on_this_page")}</div>
                <div class="oc-col oc-gap-2">
                    <a href="#comecar">{crate::i18n::t("help.start")}</a>
                    <a href="#investigacao">{crate::i18n::t("nav.section.research")}</a>
                    <a href="#conhecimento">{crate::i18n::t("help.section.knowledge_data")}</a>
                    <a href="#tempo">{crate::i18n::t("help.section.time_calendar")}</a>
                    <a href="#correio">{crate::i18n::t("nav.mail")}</a>
                    <a href="#inteligencia">{crate::i18n::t("help.section.ai_agents_compute")}</a>
                    <a href="#institucional">{crate::i18n::t("nav.section.institutional")}</a>
                    <a href="#conta">{crate::i18n::t("help.section.account_security")}</a>
                    <a href="#estados">{crate::i18n::t("help.section.system_states")}</a>
                </div>
            </nav>

            {seccao(
                "comecar",
                crate::i18n::t("help.start"),
                view! {
                    {p(crate::i18n::t("help.start.p1"))}
                    {p(crate::i18n::t("help.start.p2"))}
                    {p(crate::i18n::t("help.start.p3"))}
                    <p class="oc-t-prose">
                        {crate::i18n::t("help.go_to")}<a href="/">{crate::i18n::t("help.link.home")}</a>" · "
                        <a href="/my-work">{crate::i18n::t("nav.my_work")}</a>
                    </p>
                },
            )}

            {seccao(
                "investigacao",
                crate::i18n::t("nav.section.research"),
                view! {
                    {p(crate::i18n::t("help.research.p1"))}
                    {p(crate::i18n::t("help.research.p2"))}
                    {p(crate::i18n::t("help.research.p3"))}
                    <p class="oc-t-prose">
                        {crate::i18n::t("help.go_to")}<a href="/units">{crate::i18n::t("nav.units")}</a>" · "
                        <a href="/ideas">{crate::i18n::t("nav.ideas")}</a>" · "
                        <a href="/projects">{crate::i18n::t("nav.projects")}</a>
                    </p>
                },
            )}

            {seccao(
                "conhecimento",
                crate::i18n::t("help.section.knowledge_data"),
                view! {
                    {p(crate::i18n::t("help.knowledge.p1"))}
                    {p(crate::i18n::t("help.knowledge.p2"))}
                    {p(crate::i18n::t("help.knowledge.p3"))}
                    {p(crate::i18n::t("help.knowledge.p4"))}
                    <p class="oc-t-prose">
                        {crate::i18n::t("help.go_to")}<a href="/knowledge">{crate::i18n::t("nav.knowledge")}</a>" · "
                        <a href="/bibliography">{crate::i18n::t("nav.bibliography")}</a>" · "
                        <a href="/datasets">{crate::i18n::t("nav.data")}</a>
                    </p>
                },
            )}

            {seccao(
                "arranque",
                crate::i18n::t("help.section.boot"),
                view! {
                    {p(crate::i18n::t("help.boot.p1"))}
                    {p(crate::i18n::t("help.boot.p2"))}
                    {p(crate::i18n::t("help.boot.p3"))}
                    {p(crate::i18n::t("help.boot.p4"))}
                    {p(crate::i18n::t("help.boot.p5"))}
                    {p(crate::i18n::t("help.boot.p6"))}
                },
            )}

            {seccao(
                "tempo",
                crate::i18n::t("help.section.time_calendar"),
                view! {
                    {p(crate::i18n::t("help.time.p1"))}
                    {p(crate::i18n::t("help.time.p2"))}
                    {p(crate::i18n::t("help.time.p3"))}
                    {p(crate::i18n::t("help.time.p4"))}
                    {p(crate::i18n::t("help.time.p5"))}
                    {p(crate::i18n::t("help.time.p6"))}
                    {p(crate::i18n::t("help.time.p7"))}
                },
            )}

            {seccao(
                "correio",
                crate::i18n::t("nav.mail"),
                view! {
                    {p(crate::i18n::t("help.mail.p1"))}
                    {p(crate::i18n::t("help.mail.p2"))}
                    <p class="oc-t-prose">{crate::i18n::t("help.go_to")}<a href="/mail">{crate::i18n::t("nav.mail")}</a></p>
                },
            )}

            {seccao(
                "inteligencia",
                crate::i18n::t("help.section.ai_agents_compute"),
                view! {
                    {p(crate::i18n::t("help.ai.p1"))}
                    {p(crate::i18n::t("help.ai.p2"))}
                    {p(crate::i18n::t("help.ai.p3"))}
                    <p class="oc-t-prose">
                        {crate::i18n::t("help.go_to")}<a href="/ai">{crate::i18n::t("nav.ai")}</a>" · "
                        <a href="/ai/agents">{crate::i18n::t("nav.agents")}</a>" · "
                        <a href="/compute">{crate::i18n::t("nav.compute")}</a>
                    </p>
                },
            )}

            {seccao(
                "institucional",
                crate::i18n::t("nav.section.institutional"),
                view! {
                    {p(crate::i18n::t("help.inst.p1"))}
                    {p(crate::i18n::t("help.inst.p2"))}
                    <p class="oc-t-prose">
                        {crate::i18n::t("help.go_to")}<a href="/activity">{crate::i18n::t("nav.activity")}</a>" · "
                        <a href="/admin">{crate::i18n::t("nav.admin")}</a>" · "
                        <a href="/audit">{crate::i18n::t("nav.audit")}</a>
                    </p>
                },
            )}

            {seccao(
                "conta",
                crate::i18n::t("help.section.account_security"),
                view! {
                    {p(crate::i18n::t("help.account.p1"))}
                    {p(crate::i18n::t("help.account.p2"))}
                    {p(crate::i18n::t("help.account.p3"))}
                    <p class="oc-t-prose">{crate::i18n::t("help.go_to")}<a href="/settings">{crate::i18n::t("nav.settings")}</a></p>
                },
            )}

            {seccao(
                "estados",
                crate::i18n::t("help.section.system_states"),
                view! {
                    {p(crate::i18n::t("help.states.intro"))}
                    {estado(
                        crate::i18n::t("help.state_label.no_data"),
                        crate::i18n::t("help.state.no_data_meaning"),
                    )}
                    {estado(
                        crate::i18n::t("help.state_label.no_permission"),
                        crate::i18n::t("help.state.no_access"),
                    )}
                    {estado(
                        crate::i18n::t("help.state_label.not_configured"),
                        crate::i18n::t("help.state.not_configured_meaning"),
                    )}
                    {estado(
                        crate::i18n::t("help.state_label.not_implemented"),
                        crate::i18n::t("help.state.not_in_product"),
                    )}
                    {estado(
                        crate::i18n::t("help.state_label.unavailable"),
                        crate::i18n::t("help.state.not_in_state"),
                    )}
                    <p class="oc-t-prose oc-mt-5">{crate::i18n::t("help.states.footer")}</p>
                },
            )}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Todos os destinos da Ajuda são rotas reais do Workspace.
    ///
    /// Uma página de ajuda envelhece de uma maneira particular: o texto
    /// continua a parecer correcto muito depois de o link ter deixado de
    /// resolver. Este teste compara os destinos internos contra os caminhos que
    /// o servidor conhece, para que uma rota removida quebre aqui em vez de
    /// quebrar para quem procurava ajuda.
    #[test]
    fn todos_os_destinos_da_ajuda_sao_rotas_conhecidas() {
        let html = help().to_html();
        let mut destinos: Vec<String> = Vec::new();

        for pedaco in html.split("href=\"").skip(1) {
            let alvo = pedaco.split('"').next().unwrap_or_default();
            // Âncoras internas resolvem na própria página.
            if alvo.starts_with('#') || alvo.is_empty() {
                continue;
            }
            destinos.push(alvo.to_owned());
        }

        assert!(
            !destinos.is_empty(),
            "a ajuda deixou de ligar a lado nenhum; este teste ficou sem objecto"
        );

        let conhecidas = crate::routes::ROUTES;
        for destino in &destinos {
            assert!(
                destino.starts_with('/'),
                "a ajuda aponta para fora do Workspace: {destino}"
            );
            assert!(
                conhecidas.contains(&destino.as_str()),
                "a ajuda aponta para {destino}, que não é uma rota do Workspace"
            );
        }
    }

    /// A ajuda explica os cinco estados que o Workspace distingue.
    ///
    /// É a parte que mais serve o membro: sem ela, um controlo esbatido parece
    /// avaria, e uma tabela vazia parece perda de dados.
    #[test]
    fn a_ajuda_explica_os_estados_do_sistema() {
        let html = help().to_html();
        for chave in [
            "help.state_label.no_data",
            "help.state_label.no_permission",
            "help.state_label.not_configured",
            "help.state_label.not_implemented",
            "help.state_label.unavailable",
        ] {
            let estado = crate::i18n::t(chave);
            assert!(
                html.contains(estado),
                "a ajuda deixou de explicar o estado «{estado}»"
            );
        }
    }
}

#[cfg(test)]
mod pureza_i18n {
    use super::*;

    /// Um ecrã, um idioma: a Ajuda em francês, sem chrome em português.
    ///
    /// A Ajuda é toda prosa de produto, e é onde uma língua trocada mais se nota:
    /// um parágrafo em português no meio do francês lê-se como um erro grosseiro.
    #[tokio::test]
    async fn a_ajuda_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};
        let fr = with_locale(Locale::Fr, async { help().to_html() }).await;
        for francesa in [
            "Aide",
            "Au démarrage du système",
            "États du système",
            "Aucune donnée",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        // Frases inteiras de chrome português que não podem sobreviver em francês.
        for portuguesa in ["A Home reúne", "Um lembrete não é", "Sem permissão"] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português «{portuguesa}» sobreviveu"
            );
        }
    }
}
