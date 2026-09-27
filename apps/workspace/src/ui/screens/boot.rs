//! O arranque, como uma pessoa o vê.
//!
//! DESIGN_LOCKED · moldura do login (Proposta D7) aplicada ao arranque.
//!
//! # Isto não é um ecrã de carregamento
//!
//! Um ecrã de carregamento enche o tempo enquanto se espera. Isto mostra uma
//! decisão que o Core já tomou, no momento em que a página é servida — não há
//! aqui percentagens inventadas, nem etapas temporizadas para dar a impressão
//! de que alguma coisa está a acontecer.
//!
//! Se a prontidão vier num instante, o arranque dura um instante. Encenar
//! espera seria mentir sobre o sistema para o fazer parecer mais sério.
//!
//! # Continuidade com o Login
//!
//! Reutiliza a mesma moldura do ecrã de entrada — os mesmos anéis, a mesma
//! grelha, o mesmo logótipo, a mesma tipografia. Sair do arranque e chegar ao
//! Login não pode parecer que abriu outro produto.
//!
//! # Nada aqui decide
//!
//! Cada estado corresponde a uma resposta do Core, ou à ausência dela. Este
//! ficheiro escolhe palavras e disposição; não conclui prontidão, não conta
//! componentes, não decide o que é crítico.

use leptos::prelude::*;
use ocinye_contracts::readiness::ReadinessComponent;

use crate::boot::{BootOutcome, BootState};

/// A superfície de arranque.
///
/// `destino` é para onde se segue quando o Core deixa seguir. Vai numa
/// actualização de meta, e não em JavaScript: o arranque tem de funcionar antes
/// de qualquer script, que é precisamente quando é mais preciso.
///
/// Onde há JavaScript, `app.js` refaz a mesma entrega com `location.replace`,
/// porque um meta refresh com atraso acrescenta uma entrada ao histórico em vez
/// de a substituir — e o `/boot` ficava na pilha a apanhar quem carregasse em
/// «voltar». O meta continua a ser a garantia; o script é só o histórico.
pub fn boot(outcome: &BootOutcome, destino: &str) -> impl IntoView {
    let estado = outcome.state.clone();
    let tipo = estado.kind();

    let (titulo, explicacao) = match estado {
        BootState::Ready => (
            crate::i18n::t("boot.ready.title"),
            crate::i18n::t("boot.ready.body"),
        ),
        BootState::Degraded => (
            crate::i18n::t("boot.degraded.title"),
            crate::i18n::t("boot.degraded.body"),
        ),
        BootState::Blocked => (
            crate::i18n::t("boot.blocked.title"),
            crate::i18n::t("boot.blocked.body"),
        ),
        BootState::Unreachable => (
            crate::i18n::t("boot.unreachable.title"),
            crate::i18n::t("boot.unreachable.body"),
        ),
        BootState::Uninitialized | BootState::Checking => (
            crate::i18n::t("boot.checking.title"),
            crate::i18n::t("boot.asking_core"),
        ),
    };

    let limitacoes = outcome.limitations();
    let bloqueios = outcome.blockers();
    let segue = estado.may_hand_off();
    let destino = destino.to_owned();

    view! {
        <main class="ods-auth ods-boot" data-estado=tipo>
            {crate::ui::screens::login::barra(None)}
            <div class="ods-auth__stage">
            <span class="ods-auth__mark"><img src="/static/ocinye_logo.png" alt="" /></span>
            <p class="ods-auth__product">{crate::i18n::t("auth.product")}</p>

            <section class="ods-auth__card" role="status" aria-live="polite">
                <h1 class="ods-auth__title" data-part="boot__title">{titulo}</h1>
                <p class="ods-auth__lead" data-part="boot__lede">{explicacao}</p>

                // A barra só enquanto se segue caminho: um arranque parado não
                // finge que está a andar.
                {segue.then(|| view! {
                    <div class="ods-boot__bar" aria-hidden="true"><span></span></div>
                })}

                {(!bloqueios.is_empty())
                    .then(|| view! {
                        <ul class="ods-boot__list" data-part="boot__list--blocking">
                            {bloqueios.iter().map(|c| componente(c, true)).collect_view()}
                        </ul>
                    })}

                {(!limitacoes.is_empty())
                    .then(|| view! {
                        <ul class="ods-boot__list">
                            {limitacoes.iter().map(|c| componente(c, false)).collect_view()}
                        </ul>
                    })}

                {(!segue)
                    .then(|| view! {
                        <form method="get" action="/boot" class="ods-boot__actions" data-part="boot__actions">
                            <input type="hidden" name="return_to" value=destino.clone() />
                            <button type="submit" class="ods-btn ods-btn--primary ods-auth__submit" data-part="boot__retry">
                                {crate::i18n::t("boot.retry")}
                            </button>
                        </form>
                    })}
            </section>
            </div>
        </main>
    }
}

/// Um componente, como quem lê o vê.
///
/// A razão vem do Core, escrita a partir de um conjunto fixo de frases. Entra
/// como texto — o Leptos escapa-a — e nunca como HTML.
fn componente(c: &ReadinessComponent, bloqueia: bool) -> impl IntoView {
    let classe = if bloqueia {
        "ods-state ods-state--error"
    } else {
        "ods-state ods-state--unavailable"
    };
    view! {
        <li class=classe>
            <b>{c.component.label().to_owned()}</b>
            " · "
            <span>{c.reason.clone()}</span>
        </li>
    }
}

/// O que a página faz depois de mostrar o estado.
///
/// Uma actualização de meta, e não JavaScript. O arranque é o momento em que
/// menos se pode assumir que há scripts a correr, e é exactamente o momento em
/// que tem de funcionar.
///
/// O atraso é o mínimo que deixa a superfície ser vista sem parecer um salto —
/// e é declaradamente de apresentação, não de prontidão. A prontidão já foi
/// decidida antes de esta página existir.
#[must_use]
pub fn handoff_meta(outcome: &BootOutcome, destino: &str) -> Option<String> {
    outcome.state.may_hand_off().then(|| {
        format!(
            r#"<meta http-equiv="refresh" content="0.6;url={}">"#,
            html_escape(destino)
        )
    })
}

/// Escapa um destino para caber dentro de um atributo.
///
/// O destino já foi validado contra o catálogo de rotas antes de chegar aqui.
/// Isto é a segunda defesa: mesmo um valor que passasse a validação não pode
/// fechar o atributo e abrir marcação.
fn html_escape(valor: &str) -> String {
    valor
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocinye_contracts::readiness::{
        Criticality, PublicReadiness, ReadinessComponentId, ReadinessOverall, CONTRACT_VERSION,
    };
    use ocinye_contracts::system_capability::SystemCapabilityState;

    fn resultado(overall: ReadinessOverall) -> BootOutcome {
        BootOutcome {
            state: match overall {
                ReadinessOverall::Ready => BootState::Ready,
                ReadinessOverall::Degraded => BootState::Degraded,
                ReadinessOverall::Blocked => BootState::Blocked,
            },
            readiness: Some(PublicReadiness {
                overall,
                contract_version: CONTRACT_VERSION,
                components: vec![ReadinessComponent {
                    component: ReadinessComponentId::Mail,
                    state: SystemCapabilityState::NotConfigured,
                    criticality: Criticality::Optional,
                    reason: "sem fornecedor configurado".to_owned(),
                }],
            }),
        }
    }

    #[test]
    fn so_se_entrega_a_sessao_quando_o_core_deixa() {
        assert!(handoff_meta(&resultado(ReadinessOverall::Ready), "/").is_some());
        assert!(handoff_meta(&resultado(ReadinessOverall::Degraded), "/").is_some());
        assert!(handoff_meta(&resultado(ReadinessOverall::Blocked), "/").is_none());
        assert!(handoff_meta(
            &BootOutcome {
                state: BootState::Unreachable,
                readiness: None
            },
            "/"
        )
        .is_none());
    }

    /// Uma razão hostil vinda do Core não se torna marcação.
    ///
    /// # O modelo de ameaça
    ///
    /// A razão é uma `String` livre no contrato, e é a única coisa nesta
    /// superfície que vem de fora com texto que uma pessoa vai ler. O Core
    /// escreve-a sempre a partir de um conjunto fixo de frases, e há um teste
    /// do lado dele que o garante — mas essa é a garantia de um sistema a
    /// comportar-se bem, e não uma propriedade desta superfície.
    ///
    /// Aqui a pergunta é outra: se a razão chegar hostil — Core comprometido,
    /// intermediário a reescrever, um dia em que o conjunto fixo deixe de o
    /// ser — o ecrã de arranque transforma-a em marcação? Não: entra como
    /// texto, e o Leptos escapa-a.
    #[test]
    fn uma_razao_hostil_do_core_nao_se_torna_marcacao() {
        let hostil = "</span><script>alert(1)</script><span>";
        let outcome = BootOutcome {
            state: BootState::Blocked,
            readiness: Some(PublicReadiness {
                overall: ReadinessOverall::Blocked,
                contract_version: CONTRACT_VERSION,
                components: vec![ReadinessComponent {
                    component: ReadinessComponentId::Persistence,
                    state: SystemCapabilityState::Unavailable,
                    criticality: Criticality::Critical,
                    reason: hostil.to_owned(),
                }],
            }),
        };

        let html = leptos::prelude::IntoView::into_view(boot(&outcome, "/")).to_html();

        assert!(
            !html.contains("<script>"),
            "a razão do Core chegou ao documento como marcação: {html}"
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "a razão devia aparecer escapada, e não desaparecer: {html}"
        );
    }

    /// O destino não consegue fechar o atributo onde vive.
    #[test]
    fn o_destino_nao_escapa_do_atributo() {
        let malicioso = r#"/x" onload="alert(1)"#;
        let meta = handoff_meta(&resultado(ReadinessOverall::Ready), malicioso).expect("segue");
        // O que importa não é o texto `onload=` desaparecer — ele fica lá,
        // inerte, dentro do valor do atributo. O que importa é o atributo não
        // poder ser **fechado**: sem aspas cruas depois da que o abre, tudo o
        // que vem a seguir é conteúdo e não marcação.
        //
        // A primeira versão deste teste procurava a palavra, e teria falhado
        // sobre código correcto — que é a maneira de uma asserção medir outra
        // coisa que não a propriedade.
        let valor = meta
            .split("content=\"")
            .nth(1)
            .expect("o atributo content")
            .trim_end_matches('>');
        assert_eq!(
            valor.matches('"').count(),
            1,
            "o destino fechou o atributo antes do fim: {meta}"
        );
        assert!(
            meta.contains("&quot;"),
            "as aspas do destino tinham de vir escapadas: {meta}"
        );
    }

    /// Uma capacidade opcional em baixo limita; não bloqueia.
    #[test]
    fn uma_capacidade_opcional_em_baixo_e_limitacao_e_nao_bloqueio() {
        let r = resultado(ReadinessOverall::Degraded);
        assert_eq!(r.limitations().len(), 1);
        assert!(r.blockers().is_empty());
    }
}

#[cfg(test)]
mod pureza_i18n {
    use super::*;

    /// Um ecrã, um idioma: cada estado de arranque em francês, sem português.
    #[tokio::test]
    async fn o_arranque_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};
        let casos = [
            (BootState::Blocked, "IMPOSSIBLE DE DÉMARRER OCINYE OS"),
            (
                BootState::Unreachable,
                "IMPOSSIBLE DE CONTACTER OCINYE CORE",
            ),
            (BootState::Ready, "SYSTÈME OPÉRATIONNEL"),
        ];
        for (state, francesa) in casos {
            let outcome = BootOutcome {
                state,
                readiness: None,
            };
            let fr = with_locale(Locale::Fr, async { boot(&outcome, "/").to_html() }).await;
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
            assert!(!fr.contains("SISTEMA OPERACIONAL"), "fr: título português");
            assert!(!fr.contains("Estado do Sistema"), "fr: rodapé português");
        }
    }
}
