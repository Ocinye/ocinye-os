//! O Meu Trabalho.
//!
//! O que está atribuído ao membro (`design/README.md` §6.3).

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{badge, Tone};
use crate::ui::components::{pill_tabs, Tab};

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
        Tab::link("Tarefas", "/my-work", true),
        Tab::link("Actividade", "/activity", false),
        Tab::link("Ideias", "/ideas", false),
        Tab::link("Projectos", "/projects", false),
        Tab::inert("Documentos"),
        Tab::link("Datasets", "/datasets", false),
        Tab::inert("Favoritos"),
        Tab::inert("Notas"),
    ];

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"O Meu Trabalho"</h1>
                    <p>"Tudo o que lhe está atribuído ou que segue de perto."</p>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--under oc-card__head--flush" >
                {pill_tabs(tabs, "Secções do meu trabalho")}
            </div>

            <div class="oc-grid oc-grid--detail">
                <section class="oc-card">
                    <div class="oc-card__head">
                        <h2>"Tarefas atribuídas"</h2>
                        <span class="oc-card__meta">{task_rows.len().to_string()}</span>
                    </div>
                    <div class="oc-card__body">
                        {if task_rows.is_empty() {
                            view! { <p class="oc-muted">"Não tem tarefas atribuídas."</p> }
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
                                                    <span class="oc-fill oc-truncate oc-t-cell" >
                                                        {text(row, "title")}
                                                    </span>
                                                    {badge(priority.clone(), Tone::of(&priority))}
                                                    {badge(state.clone(), Tone::of(&state))}
                                                    <span class="oc-mono oc-list__meta" >
                                                        {row
                                                            .get("due_on")
                                                            .and_then(Value::as_str)
                                                            .unwrap_or("sem prazo")
                                                            .to_owned()}
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
                            <h2>"Investigação que sigo"</h2>
                        </div>
                        <div class="oc-card__body">
                            {if workspace_rows.is_empty() {
                                view! {
                                    <p class="oc-muted">
                                        "Ainda não pertence a nenhum Research Workspace."
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
                            <h2>"Documentos recentes"</h2>
                            <span class="oc-card__meta oc-unavailable">"indisponível"</span>
                        </div>
                        <div class="oc-card__body">
                            <p class="oc-muted">
                                "O Ocinye Core ainda não serve os documentos abertos \
                                 recentemente por uma pessoa. Os documentos existem e \
                                 estão acessíveis a partir de cada Research Workspace."
                            </p>
                        </div>
                    </section>

                    <section class="oc-card oc-mb-5">
                        <div class="oc-card__head">
                            <h2>"Unidades seguidas"</h2>
                            <span class="oc-card__meta oc-unavailable">"indisponível"</span>
                        </div>
                        <div class="oc-card__body">
                            <p class="oc-muted">
                                "Seguir uma unidade ainda não existe no Ocinye Core. \
                                 As unidades a que pertence estão em Unidades."
                            </p>
                        </div>
                    </section>

                    <section class="oc-card">
                        <div class="oc-card__head">
                            <h2>"A minha actividade"</h2>
                        </div>
                        <div class="oc-card__body">
                            {if activity_rows.is_empty() {
                                view! { <p class="oc-muted">"Sem actividade recente."</p> }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="oc-col oc-gap-7" >
                                        {activity_rows
                                            .iter()
                                            .take(8)
                                            .map(|row| {
                                                view! {
                                                    <div class="oc-t-note" >
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
