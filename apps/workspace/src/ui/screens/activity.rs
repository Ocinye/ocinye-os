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
                    <h1>"Actividade"</h1>
                    <p>
                        "O que mudou no trabalho a que tem acesso. Distinto do registo de
                         auditoria, que existe para segurança e evidência."
                    </p>
                </div>
            </div>

            <section class="oc-card">
                <div class="oc-card__body">
                    {if empty {
                        view! { <p class="oc-muted">"Ainda não há actividade."</p> }.into_any()
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
                                                    <div class="oc-t-prose" >
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
