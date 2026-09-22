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
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{t("my_work.title")}</h1>
                    <p>{t("my_work.subtitle")}</p>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--under oc-card__head--flush" >
                {pill_tabs(tabs, t("my_work.tabs.aria"))}
            </div>

            <div class="oc-grid oc-grid--detail">
                <section class="oc-card">
                    <div class="oc-card__head">
                        <h2>{t("my_work.tasks.title")}</h2>
                        <span class="oc-card__meta">{task_rows.len().to_string()}</span>
                    </div>
                    <div class="oc-card__body">
                        {if task_rows.is_empty() {
                            view! { <p class="oc-muted">{t("my_work.tasks.empty")}</p> }
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
                                                    class="oc-list__row"
                                                >
                                                    <span class="oc-fill oc-truncate oc-t-cell" data-oc-content="1">
                                                        {text(row, "title")}
                                                    </span>
                                                    {task_priority_badge(&priority)}
                                                    {task_state_badge(&state)}
                                                    <span class="oc-mono oc-list__meta" >
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
                    <section class="oc-card oc-mb-5" >
                        <div class="oc-card__head">
                            <h2>{t("my_work.research.title")}</h2>
                        </div>
                        <div class="oc-card__body">
                            {if workspace_rows.is_empty() {
                                view! {
                                    <p class="oc-muted">
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
                                                        class="oc-list__row"
                                                    >
                                                        <span class="oc-mono oc-list__meta" >
                                                            {text(row, "code")}
                                                        </span>
                                                        <span class="oc-fill oc-truncate oc-t-cell-2" >
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
                    <section class="oc-card oc-mb-5">
                        <div class="oc-card__head">
                            <h2>{t("my_work.documents.title")}</h2>
                            <span class="oc-card__meta oc-unavailable">{t("my_work.unavailable")}</span>
                        </div>
                        <div class="oc-card__body">
                            <p class="oc-muted">{t("my_work.documents.body")}</p>
                        </div>
                    </section>

                    <section class="oc-card oc-mb-5">
                        <div class="oc-card__head">
                            <h2>{t("my_work.units.title")}</h2>
                            <span class="oc-card__meta oc-unavailable">{t("my_work.unavailable")}</span>
                        </div>
                        <div class="oc-card__body">
                            <p class="oc-muted">{t("my_work.units.body")}</p>
                        </div>
                    </section>

                    <section class="oc-card">
                        <div class="oc-card__head">
                            <h2>{t("my_work.my_activity.title")}</h2>
                        </div>
                        <div class="oc-card__body">
                            {if activity_rows.is_empty() {
                                view! { <p class="oc-muted">{t("my_work.my_activity.empty")}</p> }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="oc-col oc-gap-7" >
                                        {activity_rows
                                            .iter()
                                            .take(8)
                                            .map(|row| {
                                                view! {
                                                    <div class="oc-t-note" data-oc-content="1">
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
