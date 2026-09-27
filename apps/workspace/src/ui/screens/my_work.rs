//! O Meu Trabalho.
//!
//! O que está atribuído ao membro (`design/README.md` §6.3).

use leptos::prelude::*;
use serde_json::Value;

use crate::i18n::t;
use crate::ui::components::{pill_tabs, Tab};
use crate::ui::components::{task_priority_badge, task_state_badge};

fn items(payload: &Value) -> Vec<Value> {
    payload
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| payload.as_array())
        .cloned()
        .unwrap_or_default()
}

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

/// O ecrã.
pub fn my_work(tasks: &Value, workspaces: &Value, activity: &Value) -> impl IntoView {
    let task_rows = items(tasks);
    let workspace_rows = items(workspaces);
    let activity_rows = items(activity);

    let tabs = vec![
        Tab::link(t("my_work.tab.tasks"), "/my-work", true),
        Tab::link(t("my_work.tab.activity"), "/activity", false),
        Tab::link(t("my_work.tab.ideas"), "/ideas", false),
        Tab::link(t("my_work.tab.projects"), "/projects", false),
        Tab::inert(t("my_work.tab.documents")),
        Tab::link(t("my_work.tab.datasets"), "/datasets", false),
        Tab::inert(t("my_work.tab.favourites")),
        Tab::inert(t("my_work.tab.notes")),
    ];

    view! {
        <div class="ods-page">
            <div class="ods-page__head">
                <div>
                    <h1 class="ods-page__title">{t("my_work.title")}</h1>
                    <p class="ods-page__sub">{t("my_work.subtitle")}</p>
                </div>
            </div>

            <div class="ods-tabs">
                {pill_tabs(tabs, t("my_work.tabs.aria"))}
            </div>

            <div>
                <section class="ods-widget ods-widget-surface" data-part="card">
                    <div class="ods-widget__head">
                        <h2>{t("my_work.tasks.title")}</h2>
                        <span class="ods-label">{task_rows.len().to_string()}</span>
                    </div>
                    <div class="ods-widget__body">
                        {if task_rows.is_empty() {
                            view! { <p class="ods-field__hint">{t("my_work.tasks.empty")}</p> }
                                .into_any()
                        } else {
                            view! {
                                <div>
                                    {task_rows
                                        .iter()
                                        .map(|row| {
                                            let state = text(row, "state");
                                            let priority = text(row, "priority");
                                            let workspace = text(row, "workspace_id");
                                            view! {
                                                <a
                                                    href=format!("/workspaces/{workspace}")
                                                >
                                                    <span data-oc-content="1">
                                                        {text(row, "title")}
                                                    </span>
                                                    {task_priority_badge(&priority)}
                                                    {task_state_badge(&state)}
                                                    <span>
                                                        {row
                                                            .get("due_on")
                                                            .and_then(Value::as_str)
                                                            .map_or_else(
                                                                || t("my_work.no_due").to_owned(),
                                                                ToOwned::to_owned,
                                                            )}
                                                    </span>
                                                </a>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }}
                    </div>
                </section>

                <div>
                    <section class="ods-widget ods-widget-surface" data-part="card">
                        <div class="ods-widget__head">
                            <h2>{t("my_work.research.title")}</h2>
                        </div>
                        <div class="ods-widget__body">
                            {if workspace_rows.is_empty() {
                                view! {
                                    <p class="ods-field__hint">
                                        {t("my_work.research.empty")}
                                    </p>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div>
                                        {workspace_rows
                                            .iter()
                                            .take(8)
                                            .map(|row| {
                                                let id = text(row, "id");
                                                view! {
                                                    <a
                                                        href=format!("/workspaces/{id}")
                                                    >
                                                        <span>
                                                            {text(row, "code")}
                                                        </span>
                                                        <span>
                                                            {text(row, "title")}
                                                        </span>
                                                    </a>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }}
                        </div>
                    </section>

                    // Os dois painéis que o dossier põe nesta coluna (§6.3).
                    //
                    // A forma é a desenhada; o conteúdo não existe, e é dito.
                    // «Documentos recentes» precisa de um endpoint que o Core
                    // não expõe, e «Unidades seguidas» precisa de um conceito
                    // de seguir que o domínio não tem. Encher qualquer um deles
                    // com o que está à mão seria mostrar uma coisa a dizer
                    // outra (`CLAUDE.md` §69).
                    <section class="ods-widget ods-widget-surface" data-part="card">
                        <div class="ods-widget__head">
                            <h2>{t("my_work.documents.title")}</h2>
                            <span class="ods-label oc-unavailable" data-part="unavailable">{t("my_work.unavailable")}</span>
                        </div>
                        <div class="ods-widget__body">
                            <p class="ods-field__hint">{t("my_work.documents.body")}</p>
                        </div>
                    </section>

                    <section class="ods-widget ods-widget-surface" data-part="card">
                        <div class="ods-widget__head">
                            <h2>{t("my_work.units.title")}</h2>
                            <span class="ods-label oc-unavailable" data-part="unavailable">{t("my_work.unavailable")}</span>
                        </div>
                        <div class="ods-widget__body">
                            <p class="ods-field__hint">{t("my_work.units.body")}</p>
                        </div>
                    </section>

                    <section class="ods-widget ods-widget-surface" data-part="card">
                        <div class="ods-widget__head">
                            <h2>{t("my_work.my_activity.title")}</h2>
                        </div>
                        <div class="ods-widget__body">
                            {if activity_rows.is_empty() {
                                view! { <p class="ods-field__hint">{t("my_work.my_activity.empty")}</p> }
                                    .into_any()
                            } else {
                                view! {
                                    <div>
                                        {activity_rows
                                            .iter()
                                            .take(8)
                                            .map(|row| {
                                                view! {
                                                    <div class="ods-field__hint" data-oc-content="1">
                                                        {text(row, "summary")}
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
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Um ecrã, um idioma: «O Meu Trabalho» em francês, sem marcas portuguesas.
    #[tokio::test]
    async fn o_meu_trabalho_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};
        let vazio = json!({"items": []});
        let fr = with_locale(Locale::Fr, async {
            my_work(&vazio, &vazio, &vazio).to_html()
        })
        .await;
        for francesa in [
            "Mon travail",
            "Tâches attribuées",
            "Recherche que je suis",
            "Mon activité",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        for portuguesa in [
            "O Meu Trabalho",
            "Tarefas atribuídas",
            "Investigação que sigo",
            "A minha actividade",
        ] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português por traduzir «{portuguesa}»"
            );
        }
    }
}
