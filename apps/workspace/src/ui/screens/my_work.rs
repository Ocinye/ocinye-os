//! O Meu Trabalho.
//!
//! O que está atribuído ao membro (`design/README.md` §6.3).

use chrono::{Datelike, NaiveDate};
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

/// A tabela das tarefas — a mesma em cada grupo, com as mesmas colunas.
fn tabela(rows: Vec<Value>) -> impl IntoView {
    view! {
        <table class="ods-table">
            <thead>
                <tr>
                    <th scope="col">{t("my_work.tasks.title")}</th>
                    <th scope="col">{t("workspaces.field.priority")}</th>
                    <th scope="col">{t("workspaces.field.state")}</th>
                    <th scope="col" class="ods-num">{t("workspaces.field.due")}</th>
                </tr>
            </thead>
            <tbody>
                {rows
                    .iter()
                    .map(|row| {
                        let state = text(row, "state");
                        let priority = text(row, "priority");
                        let workspace = text(row, "workspace_id");
                        let href = format!("/workspaces/{workspace}");
                        let destino = href.clone();
                        view! {
                            <tr data-oc="table-row" data-oc-href=destino>
                                <td>
                                    <a href=href data-oc-content="1">{text(row, "title")}</a>
                                </td>
                                <td>{task_priority_badge(&priority)}</td>
                                <td>{task_state_badge(&state)}</td>
                                <td class="ods-num">
                                    {row
                                        .get("due_on")
                                        .and_then(Value::as_str)
                                        .map_or_else(
                                            || t("my_work.no_due").to_owned(),
                                            ToOwned::to_owned,
                                        )}
                                </td>
                            </tr>
                        }
                    })
                    .collect_view()}
            </tbody>
        </table>
    }
}

/// Os grupos por prazo (D13, Q-28), por esta ordem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grupo {
    Atrasadas,
    Hoje,
    EstaSemana,
    Depois,
    SemPrazo,
}

impl Grupo {
    const TODOS: [Grupo; 5] = [
        Self::Atrasadas,
        Self::Hoje,
        Self::EstaSemana,
        Self::Depois,
        Self::SemPrazo,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::Atrasadas => "overdue",
            Self::Hoje => "today",
            Self::EstaSemana => "week",
            Self::Depois => "later",
            Self::SemPrazo => "none",
        }
    }

    fn rotulo(self) -> &'static str {
        t(match self {
            Self::Atrasadas => "my_work.group.overdue",
            Self::Hoje => "my_work.group.today",
            Self::EstaSemana => "my_work.group.week",
            Self::Depois => "my_work.group.later",
            Self::SemPrazo => "my_work.group.none",
        })
    }

    /// O grupo de um prazo, visto de `hoje`. A semana acaba ao domingo.
    fn de(prazo: Option<NaiveDate>, hoje: NaiveDate) -> Self {
        let Some(dia) = prazo else {
            return Self::SemPrazo;
        };
        let fim_da_semana =
            hoje + chrono::Days::new(u64::from(6 - hoje.weekday().num_days_from_monday()));
        if dia < hoje {
            Self::Atrasadas
        } else if dia == hoje {
            Self::Hoje
        } else if dia <= fim_da_semana {
            Self::EstaSemana
        } else {
            Self::Depois
        }
    }
}

fn prazo(row: &Value) -> Option<NaiveDate> {
    row.get("due_on")
        .and_then(Value::as_str)
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

/// As tarefas agrupadas por prazo. Dentro de cada grupo, a ordem do Core; um
/// grupo vazio não aparece.
fn grupos(rows: &[Value], hoje: NaiveDate) -> impl IntoView {
    Grupo::TODOS
        .into_iter()
        .filter_map(|g| {
            let do_grupo: Vec<Value> = rows
                .iter()
                .filter(|r| Grupo::de(prazo(r), hoje) == g)
                .cloned()
                .collect();
            (!do_grupo.is_empty()).then(|| {
                let n = do_grupo.len().to_string();
                view! {
                    <section class="ods-d13-group" data-group=g.id()>
                        <p class="ods-label ods-d13-group__label">{g.rotulo()} " " <span>{n}</span></p>
                        {tabela(do_grupo)}
                    </section>
                }
            })
        })
        .collect_view()
}

fn grupos_ou_tabela(rows: &[Value], hoje: Option<NaiveDate>) -> impl IntoView {
    match hoje {
        Some(hoje) => grupos(rows, hoje).into_any(),
        None => tabela(rows.to_vec()).into_any(),
    }
}

/// O ecrã.
/// `hoje` é o dia civil no fuso que o Core diz; sem ele fica a tabela única.
pub fn my_work(
    tasks: &Value,
    workspaces: &Value,
    activity: &Value,
    hoje: Option<NaiveDate>,
) -> impl IntoView {
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

            {pill_tabs(tabs, t("my_work.tabs.aria"))}

            <div class="ods-detail">
                <section class="ods-widget ods-widget-surface" data-part="card">
                    <div class="ods-widget__head">
                        <span class="ods-widget__titles"><h2 class="ods-widget__title">{t("my_work.tasks.title")}</h2></span>
                        <span class="ods-label">{task_rows.len().to_string()}</span>
                    </div>
                    <div class="ods-widget__body">
                        {if task_rows.is_empty() {
                            view! { <p class="ods-field__hint">{t("my_work.tasks.empty")}</p> }
                                .into_any()
                        } else {
                            view! {
                                {grupos_ou_tabela(&task_rows, hoje)}
                            }
                                .into_any()
                        }}
                    </div>
                </section>

                <aside>
                    <section class="ods-widget ods-widget-surface" data-part="card">
                        <div class="ods-widget__head">
                            <span class="ods-widget__titles"><h2 class="ods-widget__title">{t("my_work.research.title")}</h2></span>
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
                            <span class="ods-widget__titles"><h2 class="ods-widget__title">{t("my_work.documents.title")}</h2></span>
                            <span class="ods-badge" data-part="unavailable">{t("my_work.unavailable")}</span>
                        </div>
                        <div class="ods-widget__body">
                            <p class="ods-field__hint">{t("my_work.documents.body")}</p>
                        </div>
                    </section>

                    <section class="ods-widget ods-widget-surface" data-part="card">
                        <div class="ods-widget__head">
                            <span class="ods-widget__titles"><h2 class="ods-widget__title">{t("my_work.units.title")}</h2></span>
                            <span class="ods-badge" data-part="unavailable">{t("my_work.unavailable")}</span>
                        </div>
                        <div class="ods-widget__body">
                            <p class="ods-field__hint">{t("my_work.units.body")}</p>
                        </div>
                    </section>

                    <section class="ods-widget ods-widget-surface" data-part="card">
                        <div class="ods-widget__head">
                            <span class="ods-widget__titles"><h2 class="ods-widget__title">{t("my_work.my_activity.title")}</h2></span>
                        </div>
                        <div class="ods-widget__body">
                            {if activity_rows.is_empty() {
                                view! { <p class="ods-field__hint">{t("my_work.my_activity.empty")}</p> }
                                    .into_any()
                            } else {
                                view! {
                                    <ol class="ods-timeline">
                                        {activity_rows
                                            .iter()
                                            .take(8)
                                            .map(|row| {
                                                view! {
                                                    <li class="ods-timeline__item" data-oc-content="1">
                                                        {text(row, "summary")}
                                                    </li>
                                                }
                                            })
                                            .collect_view()}
                                    </ol>
                                }
                                    .into_any()
                            }}
                        </div>
                    </section>
                </aside>
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
            my_work(&vazio, &vazio, &vazio, None).to_html()
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

#[cfg(test)]
mod grupos_por_prazo {
    use super::*;

    fn dia(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    /// Hoje é quarta, 2026-09-23: a semana acaba no domingo 27.
    #[test]
    fn cada_prazo_cai_no_seu_grupo() {
        let hoje = dia("2026-09-23");
        assert_eq!(Grupo::de(Some(dia("2026-09-22")), hoje), Grupo::Atrasadas);
        assert_eq!(Grupo::de(Some(hoje), hoje), Grupo::Hoje);
        assert_eq!(Grupo::de(Some(dia("2026-09-27")), hoje), Grupo::EstaSemana);
        assert_eq!(Grupo::de(Some(dia("2026-09-28")), hoje), Grupo::Depois);
        assert_eq!(Grupo::de(None, hoje), Grupo::SemPrazo);
    }

    #[test]
    fn grupos_vazios_nao_aparecem_e_sem_fuso_fica_a_tabela() {
        let tarefas = serde_json::json!({ "items": [
            { "title": "a", "due_on": "2026-09-01", "state": "open", "priority": "normal", "workspace_id": "w" },
            { "title": "b", "state": "open", "priority": "normal", "workspace_id": "w" }
        ]});
        let vazio = serde_json::json!({ "items": [] });
        let html = my_work(&tarefas, &vazio, &vazio, Some(dia("2026-09-23"))).to_html();
        assert!(html.contains(r#"data-group="overdue""#));
        assert!(html.contains(r#"data-group="none""#));
        assert!(!html.contains(r#"data-group="today""#));
        let sem = my_work(&tarefas, &vazio, &vazio, None).to_html();
        assert!(!sem.contains("data-group"));
    }
}
