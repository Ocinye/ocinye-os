//! Conhecimento — o Knowledge Hub.
//!
//! A memória institucional: bibliografia, fontes, notas, documentos, resultados
//! e publicações (`design/README.md` §6.8).

use leptos::prelude::*;
use serde_json::Value;

use crate::i18n::t;
use crate::ui::components::{assist, pill_tabs, Assist, Tab, KNOWLEDGE_SUGGESTIONS};

fn count(payload: &Value) -> i64 {
    payload
        .get("total")
        .and_then(Value::as_i64)
        .or_else(|| {
            payload
                .as_array()
                .map(|a| i64::try_from(a.len()).unwrap_or(0))
        })
        .unwrap_or(0)
}

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("—")
        .to_owned()
}

/// Os contadores do hub, cada um vindo do Core.
pub struct KnowledgeCounts {
    /// Referências bibliográficas.
    pub bibliography: Value,
    /// Documentos.
    pub documents: Value,
    /// Datasets.
    pub datasets: Value,
    /// Resultados de pesquisa recente, usados como "adicionado recentemente".
    pub recent: Value,
    /// Se alguma capacidade de inferência pode ser servida, segundo o Core.
    pub inference_available: bool,
    /// Se este membro pode usar assistência.
    pub may_use_assistance: bool,
}

/// O Knowledge Hub.
pub fn knowledge(counts: KnowledgeCounts) -> impl IntoView {
    let KnowledgeCounts {
        bibliography,
        documents,
        datasets,
        recent,
        inference_available,
        may_use_assistance,
    } = counts;

    let tabs = vec![
        Tab::link(t("knowledge.tab.all"), "/knowledge", true),
        Tab::link(t("knowledge.tab.bibliography"), "/bibliography", false),
        Tab::inert(t("knowledge.tab.sources")),
        Tab::inert(t("knowledge.tab.notes")),
        Tab::inert(t("knowledge.tab.documents")),
        Tab::inert(t("knowledge.tab.results")),
        Tab::inert(t("knowledge.tab.publications")),
    ];

    let recent_rows = recent
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{t("knowledge.title")}</h1>
                    <p>{t("knowledge.subtitle")}</p>
                </div>
            </div>

            <div class="oc-tabs oc-tabs--under oc-card__head--flush" >
                {pill_tabs(tabs, t("knowledge.tabs.aria"))}
            </div>

            <div class="oc-grid oc-grid--4 oc-mb-5" >
                {counter(t("knowledge.counter.bibliography"), count(&bibliography), Some("/bibliography"))}
                {counter(t("knowledge.counter.documents"), count(&documents), None)}
                {counter(t("knowledge.counter.datasets"), count(&datasets), Some("/datasets"))}
                {counter_not_implemented(t("knowledge.counter.results"))}
            </div>

            {assist(Assist {
                here: t("knowledge.here"),
                workspace_id: None,
                resource: None,
                suggestions: KNOWLEDGE_SUGGESTIONS,
                inference_available,
                may_use: may_use_assistance,
            })}

            <section class="oc-card">
                <div class="oc-card__head">
                    <h2>{t("knowledge.recent.title")}</h2>
                </div>
                <div class="oc-card__body">
                    {if recent_rows.is_empty() {
                        view! {
                            <p class="oc-muted">
                                {t("knowledge.recent.empty")}
                            </p>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div>
                                {recent_rows
                                    .iter()
                                    .take(10)
                                    .map(|row| {
                                        let kind = text(row, "entity_type").to_uppercase();
                                        view! {
                                            <div class="oc-list__row" >
                                                <span class="oc-pill">{kind}</span>
                                                <span class="oc-fill oc-truncate oc-t-cell" data-oc-content="1">
                                                    {text(row, "title")}
                                                </span>
                                                {crate::ui::components::classification_badge(
                                                    &text(row, "classification"),
                                                )}
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

/// Um indicador do acervo.
///
/// `href` é opcional porque um número verdadeiro pode não ter ecrã por trás.
/// «Documentos» é esse caso: a contagem vem do Core, mas não existe uma lista
/// institucional de documentos para onde ir. Ligá-lo à própria página de
/// Conhecimento seria um controlo que não leva a lado nenhum — e um cartão que
/// se comporta como ligação promete navegação.
fn counter(label: &'static str, value: i64, href: Option<&'static str>) -> impl IntoView {
    let conteudo = move || {
        view! {
            <div class="oc-t-meta" >
                {label.to_uppercase()}
            </div>
            <div class="oc-t-kpi oc-mt-5" >
                {value.to_string()}
            </div>
        }
    };

    href.map_or_else(
        || {
            let interior = conteudo();
            view! {
                <div
                    class="oc-card oc-card__body oc-card__body--block"
                    title=t("knowledge.no_screen")
                >
                    {interior}
                </div>
            }
            .into_any()
        },
        |href| {
            let interior = conteudo();
            view! {
                <a
                    class="oc-card oc-card--clickable oc-card__body oc-card__body--block"
                    href=href
                >
                    {interior}
                </a>
            }
            .into_any()
        },
    )
}

/// Um indicador de uma entidade que o domínio ainda não tem.
///
/// # Porque não mostra zero
///
/// Os três estados de um contador são distintos, e a interface tem de os
/// distinguir:
///
/// | | |
/// |---|---|
/// | `N` | a entidade existe, a consulta correu, tem N registos |
/// | `0` | a entidade existe, a consulta correu, não tem nenhum |
/// | `—` | a entidade **não existe** no domínio |
///
/// «Resultados» é hoje o terceiro caso: não há tabela, repositório nem
/// consulta. Escrever `0` afirmaria que a consulta correu e não devolveu nada,
/// o que seria falso — e indistinguível de um acervo vazio.
///
/// Fica como cartão sem destino: o conceito pertence ao modelo do Research
/// Workspace e continua declarado, mas não promete um ecrã que não existe.
fn counter_not_implemented(label: &'static str) -> impl IntoView {
    view! {
        <div
            class="oc-card oc-card__body oc-card__body--block oc-unavailable"
            aria-disabled="true"
            title=t("knowledge.not_in_core")
        >
            <div class="oc-t-meta" >
                {label.to_uppercase()}
            </div>
            <div class="oc-t-kpi oc-mt-5" >
                "—"
            </div>
            <div class="oc-t-caption--muted" >
                {t("knowledge.not_implemented")}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn contagens() -> KnowledgeCounts {
        KnowledgeCounts {
            bibliography: json!({"total": 0}),
            documents: json!({"total": 0}),
            datasets: json!({"total": 0}),
            recent: json!({"items": []}),
            inference_available: false,
            may_use_assistance: false,
        }
    }

    /// Um ecrã, um idioma: «Conhecimento» em francês, sem marcas portuguesas.
    #[tokio::test]
    async fn o_conhecimento_nao_mistura_linguas() {
        use crate::i18n::{with_locale, Locale};
        let fr = with_locale(Locale::Fr, async { knowledge(contagens()).to_html() }).await;
        for francesa in [
            "Connaissance",
            "mémoire institutionnelle",
            "Ajouté récemment",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        for portuguesa in ["Adicionado recentemente", "memória institucional"] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português «{portuguesa}»"
            );
        }
    }
}
