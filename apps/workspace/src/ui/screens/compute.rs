//! Computação.
//!
//! **0 nós registados.** O ecrã mostra o header de colunas do estado futuro
//! seguido do estado vazio, e nunca inventa nós online
//! (`design/README.md` §6.12).

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{badge, button, empty_state, Button, EmptyState, Tone, Variant};
use crate::ui::components::{context_tabs, Tab};
use crate::ui::icon::Icon;

const COLUMNS: [&str; 8] = [
    "NÓ",
    "ESTADO",
    "LOCALIZAÇÃO",
    "CPU",
    "RAM",
    "GPU",
    "ARMAZENAMENTO",
    "SAÚDE",
];

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

/// Bytes legíveis, ou travessão quando o nó não reportou.
fn bytes(row: &Value, key: &str) -> String {
    let Some(value) = row.get(key).and_then(Value::as_i64) else {
        return "—".to_owned();
    };
    if value == 0 {
        return "0 B".to_owned();
    }

    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut size = value as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.0} {}", UNITS[unit])
}

/// O ecrã de computação.
pub fn compute(status: &Value, nodes: &Value) -> impl IntoView {
    let rows = items(nodes);
    let registered = status
        .get("registered_nodes")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let online = status
        .get("online_nodes")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let message = status
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(
            "Nenhum nó de computação Ocinye está actualmente disponível. A plataforma funciona \
             integralmente sem nenhum.",
        )
        .to_owned();

    let tabs = vec![
        Tab::link("Nós", "/compute", true),
        Tab::inert("Trabalhos"),
        Tab::inert("Recursos"),
        Tab::inert("Ambientes"),
    ];

    view! {
        <div class="oc-band" >
            <div class="oc-head oc-mb-7" >
                <div class="oc-head__text">
                    <h1>"Computação"</h1>
                    <p>"O registo de nós computacionais da Ocinye. Zero nós é um estado válido."</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Adicionar Nó", Variant::Gold).not_yet_available())}
                </div>
            </div>
            {context_tabs(tabs, "Secções de Computação")}
        </div>

        <div class="oc-page">
            <section class="oc-card oc-table oc-table--compute oc-mb-5"  data-dense="false">
                <div class="oc-table__scroll">
                    // O header de colunas do estado futuro fica visível mesmo
                    // sem nós: mostra a forma que os dados terão.
                    <div class="oc-table__head" role="row">
                        {COLUMNS
                            .iter()
                            .map(|label| view! { <span role="columnheader">{*label}</span> })
                            .collect_view()}
                    </div>

                    {if rows.is_empty() {
                        empty_state(EmptyState {
                                icon: Icon::ComputeLg,
                                title: format!("{registered} nós registados"),
                                body: message.clone(),
                                actions: vec![
                                    Button::new("Adicionar Nó", Variant::Gold).not_yet_available(),
                                ],
                                small: true,
                            })
                            .into_any()
                    } else {
                        rows.iter()
                            .map(|row| {
                                let state = text(row, "status");
                                let gpus = row
                                    .get("gpus")
                                    .and_then(Value::as_array)
                                    .map_or_else(|| "—".to_owned(), |list| list.len().to_string());
                                view! {
                                    <div class="oc-table__row" role="row">
                                        <div class="oc-cell oc-cell--mono">{text(row, "identifier")}</div>
                                        <div class="oc-cell">{badge(state.clone(), Tone::of(&state))}</div>
                                        <div class="oc-cell oc-cell--text">
                                            {text(row, "location_label")}
                                        </div>
                                        <div class="oc-cell oc-cell--mono">
                                            {row
                                                .get("cpu_cores")
                                                .and_then(Value::as_i64)
                                                .map_or_else(|| "—".to_owned(), |n| n.to_string())}
                                        </div>
                                        <div class="oc-cell oc-cell--mono">
                                            {bytes(row, "memory_bytes")}
                                        </div>
                                        <div class="oc-cell oc-cell--mono">{gpus}</div>
                                        <div class="oc-cell oc-cell--mono">
                                            {bytes(row, "storage_bytes")}
                                        </div>
                                        <div class="oc-cell oc-cell--mono">
                                            {text(row, "last_seen_at").chars().take(16).collect::<String>()}
                                        </div>
                                    </div>
                                }
                            })
                            .collect_view()
                            .into_any()
                    }}
                </div>
            </section>

            // Métricas a zero, porque zero é o valor verdadeiro.
            <div class="oc-grid oc-grid--4">
                {metric("TRABALHOS ACTIVOS", "0")}
                {metric("GPU DISPONÍVEL", "0")}
                {metric("CPU DISPONÍVEL", &online.to_string())}
                {metric("ARMAZENAMENTO", "0 B")}
            </div>
        </div>
    }
}

fn metric(label: &'static str, value: &str) -> impl IntoView {
    let value = value.to_owned();
    view! {
        <div class="oc-card oc-card__body" >
            <div class="oc-t-meta" >
                {label}
            </div>
            <div class="oc-t-kpi oc-mt-5" >
                {value}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sem_nos_o_ecra_mostra_zero_e_nao_inventa_cam_01() {
        let html = compute(
            &json!({"registered_nodes": 0, "online_nodes": 0, "message": "Nenhum nó de computação Ocinye está actualmente disponível."}),
            &json!({"items": []}),
        )
        .to_html();

        assert!(html.contains("0 nós registados"));
        assert!(!html.contains("CAM-01"), "o ecrã não pode inventar um nó");
    }

    #[test]
    fn o_header_do_estado_futuro_fica_visivel_mesmo_sem_nos() {
        let html = compute(&json!({}), &json!({"items": []})).to_html();
        for column in COLUMNS {
            assert!(html.contains(column), "falta a coluna {column}");
        }
    }
}
