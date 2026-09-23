//! Actividade.
//!
//! Feed institucional de colaboração, largura máxima 900px
//! (`design/README.md` §6.13).
//!
//! **Não é o Audit Log.** Este feed mostra o que um colega já pode ver; a
//! auditoria existe para segurança e evidência, com notação técnica e acesso
//! restrito.

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::classification_badge;

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

/// O feed de actividade.
///
/// A cor do ponto vem do tipo de acontecimento, declarada em `ocinye.css` por
/// `[data-kind]`. Nunca é o único sinal: o texto diz sempre o que aconteceu.
pub fn activity(payload: &Value) -> impl IntoView {
    let rows = payload.as_array().cloned().unwrap_or_default();
    let empty = rows.is_empty();

    view! {
        <div class="oc-page oc-page--feed">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{crate::i18n::t("nav.activity")}</h1>
                    <p>{crate::i18n::t("activity.subtitle")}</p>
                </div>
            </div>

            <section class="oc-card">
                <div class="oc-card__body">
                    {if empty {
                        view! { <p class="oc-muted">{crate::i18n::t("activity.empty")}</p> }.into_any()
                    } else {
                        view! {
                            <div class="oc-col" >
                                {rows
                                    .iter()
                                    .map(|row| {
                                        let kind = text(row, "kind");
                                        let when: String = text(row, "created_at")
                                            .chars()
                                            .take(16)
                                            .collect();
                                        view! {
                                            <div class="oc-feed__row" >
                                                <i
                                                    aria-hidden="true"
                                                    class="oc-feed__dot"
                                                    data-kind=kind.clone()
                                                ></i>
                                                <div class="oc-fill" >
                                                    // A linha é composta pelo Core (`summary`) e é
                                                    // conteúdo, não chrome do Workspace. Traduzi-la é
                                                    // uma mudança no Core — emitir o acontecimento
                                                    // como `kind` + sujeito estruturado, e não uma
                                                    // frase já feita —, não deste ecrã (§11, §44).
                                                    <div class="oc-t-prose" data-oc-content="1">
                                                        {text(row, "summary")}
                                                    </div>
                                                    <div class="oc-row oc-gap-5 oc-mt-2" >
                                                        <span class="oc-mono oc-t-ghost" >
                                                            {format!("{} · {when}", text(row, "actor_name"))}
                                                        </span>
                                                        {classification_badge(&text(row, "classification"))}
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }}
                </div>
            </section>
        </div>
    }
}

#[cfg(test)]
mod pureza_i18n {
    use super::*;
    use serde_json::json;

    /// Um ecrã, um idioma: a Actividade em francês, sem chrome português. A linha
    /// de cada acontecimento é conteúdo do Core (`data-oc-content`) e fica fora
    /// desta prova — traduzi-la é uma mudança no Core (§11, §44).
    #[tokio::test]
    async fn a_actividade_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};
        let vazio = with_locale(Locale::Fr, async { activity(&json!([])).to_html() }).await;
        assert!(
            vazio.contains("Il n’y a pas encore d’activité"),
            "fr: estado vazio"
        );
        assert!(!vazio.contains("Ainda não há"), "fr: chrome português");

        let cheio = with_locale(Locale::Fr, async {
            activity(&json!([{
                "kind": "created", "created_at": "2026-09-23T13:00",
                "summary": "Idea created: X", "actor_name": "Fidel", "classification": "INTERNAL"
            }]))
            .to_html()
        })
        .await;
        assert!(cheio.contains("Ce qui a changé"), "fr: subtítulo");
        assert!(!cheio.contains("O que mudou"), "fr: subtítulo português");
        // A linha do Core está marcada como conteúdo.
        assert!(
            cheio.contains("data-oc-content=\"1\""),
            "a linha do Core é conteúdo"
        );
    }
}
