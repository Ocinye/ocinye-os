//! Meus Recursos — a member's own resource picture.
//!
//! Makes the enforced storage quota visible to the member who lives under it: how
//! much they hold, the effective limit, and *why* the limit is what it is — the
//! profile it derives from and any temporary grants that add on top.
//!
//! This is the member side of resource governance. It shows *how much* the member
//! may consume, never *what* they may access — the two are separate systems
//! (ADR-0108). The picture comes resolved and authorised from the Core; the
//! screen only renders it, and invents no number the Core did not send.

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{badge, progress_bar, Tone};

/// Human-readable bytes. Binary units, because a quota is a binary quantity.
fn human_bytes(value: i64) -> String {
    if value <= 0 {
        return "0 B".to_owned();
    }
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = value as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// The state's Portuguese label and badge tone.
///
/// A state the Core did not send, or one we do not know, reads as unknown rather
/// than as a reassuring "Normal": we never dress an unknown as fine.
fn state_badge(state: &str) -> (&'static str, Tone) {
    match state {
        "normal" => ("Normal", Tone::Ok),
        "warning" => ("Aviso", Tone::Warn),
        "critical" => ("Crítico", Tone::Warn),
        "over_quota" => ("Acima da quota", Tone::Err),
        _ => ("Desconhecido", Tone::Gray),
    }
}

/// A part's Portuguese label for where the entitlement came from.
fn source_label(source: &str) -> &'static str {
    match source {
        "profile" => "Perfil",
        "override" => "Substituição",
        "temporary" => "Concessão temporária",
        _ => "Origem",
    }
}

fn i64_at(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}

/// The Meus Recursos screen.
pub fn resources(me: &Value) -> impl IntoView {
    let storage = me.get("storage").cloned().unwrap_or(Value::Null);
    let entitlement = me
        .get("storage_entitlement")
        .cloned()
        .unwrap_or(Value::Null);

    let used = i64_at(&storage, "used_bytes");
    let limit = i64_at(&storage, "limit_bytes");
    let available = i64_at(&storage, "available_bytes");
    let state = storage
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();

    // The gauge reads the limit, never a stored percentage. A zero limit means no
    // limit resolved — shown as "sem limite", not as 0% or 100%.
    let has_limit = limit > 0;
    let pct: u8 = if has_limit {
        ((used as f64 / limit as f64) * 100.0).clamp(0.0, 100.0) as u8
    } else {
        0
    };
    let (state_text, state_tone) = state_badge(&state);

    let used_h = human_bytes(used);
    let limit_h = if has_limit {
        human_bytes(limit)
    } else {
        "sem limite".to_owned()
    };
    let available_h = if has_limit {
        human_bytes(available)
    } else {
        "—".to_owned()
    };

    let parts = entitlement
        .get("parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    view! {
        <div class="oc-band">
            <div class="oc-head oc-mb-7">
                <div class="oc-head__text">
                    <h1>"Meus Recursos"</h1>
                    <p>
                        "Quanto de capacidade institucional pode consumir, e quanto já consumiu. \
                         Ter direito a um recurso não é ter acesso a dados — são sistemas separados."
                    </p>
                </div>
            </div>
        </div>

        <div class="oc-page">
            <section class="oc-card oc-card__body oc-mb-5">
                <div class="oc-row oc-row--between oc-mb-5">
                    <div>
                        <h2>"Armazenamento pessoal"</h2>
                        <p class="oc-t-meta">
                            "Os ficheiros, imagens de notas e anexos de correio que lhe pertencem."
                        </p>
                    </div>
                    {badge(state_text, state_tone)}
                </div>

                {if has_limit {
                    progress_bar(pct).into_any()
                } else {
                    view! {
                        <p class="oc-t-meta">
                            "Sem limite de armazenamento atribuído."
                        </p>
                    }
                    .into_any()
                }}

                <div class="oc-grid oc-grid--3 oc-mt-5">
                    {metric("EM USO", &used_h)}
                    {metric("LIMITE", &limit_h)}
                    {metric("DISPONÍVEL", &available_h)}
                </div>
            </section>

            <section class="oc-card oc-card__body">
                <div class="oc-mb-5">
                    <h2>"Como se chega a este limite"</h2>
                    <p class="oc-t-meta">
                        "O limite não é um número mágico: soma-se de um perfil de alocação e \
                         das concessões que lhe foram feitas por cima."
                    </p>
                </div>

                {if parts.is_empty() {
                    view! {
                        <p class="oc-t-meta">
                            "Ainda não tem nenhuma alocação de armazenamento atribuída."
                        </p>
                    }
                    .into_any()
                } else {
                    parts
                        .iter()
                        .map(|part| {
                            let source = part
                                .get("source")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            let quantity = human_bytes(i64_at(part, "quantity"));
                            let note = part
                                .get("note")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_owned();
                            let expires = part
                                .get("expires_at")
                                .and_then(Value::as_str)
                                .map(|when| {
                                    format!("expira {}", when.chars().take(10).collect::<String>())
                                });
                            // Nota e expiração numa só linha secundária, para não
                            // depender de utilitários de espaçamento inline.
                            let meta = [Some(note).filter(|n| !n.is_empty()), expires]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" · ");
                            view! {
                                <div class="oc-row oc-row--between oc-mb-3">
                                    <div>
                                        <div class="oc-t-strong">{source_label(source)}</div>
                                        {(!meta.is_empty())
                                            .then(|| view! {
                                                <div class="oc-t-meta">{meta}</div>
                                            })}
                                    </div>
                                    <span class="oc-mono">{quantity}</span>
                                </div>
                            }
                        })
                        .collect_view()
                        .into_any()
                }}
            </section>

            <p class="oc-t-meta oc-mt-5">
                "O armazenamento é, para já, o único recurso medido e imposto. Computação, \
                 GPU e outros recursos governam-se pela mesma fundação e aparecerão aqui à \
                 medida que forem ligados."
            </p>
        </div>
    }
}

fn metric(label: &'static str, value: &str) -> impl IntoView {
    let value = value.to_owned();
    view! {
        <div class="oc-card oc-card__body">
            <div class="oc-t-meta">{label}</div>
            <div class="oc-t-kpi oc-mt-5">{value}</div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "storage": {
                "used_bytes": 5_368_709_120_i64,
                "limit_bytes": 10_737_418_240_i64,
                "available_bytes": 5_368_709_120_i64,
                "state": "normal"
            },
            "storage_entitlement": {
                "resource_type": "persistent_storage",
                "unit": "byte",
                "quantity": 10_737_418_240_i64,
                "parts": [
                    {"source": "profile", "quantity": 10_737_418_240_i64, "expires_at": null, "note": "Perfil MEMBER_STANDARD"}
                ]
            }
        })
    }

    #[test]
    fn mostra_uso_limite_e_disponivel() {
        let html = resources(&sample()).to_html();
        assert!(html.contains("5.0 GiB"), "uso legível");
        assert!(html.contains("10.0 GiB"), "limite legível");
        // 50% de uso.
        assert!(
            html.contains(r#"aria-valuenow="50""#),
            "a barra reflecte o uso"
        );
    }

    #[test]
    fn explica_a_origem_do_limite() {
        let html = resources(&sample()).to_html();
        assert!(html.contains("Perfil"), "nomeia a origem do entitlement");
        assert!(html.contains("MEMBER_STANDARD"), "mostra a nota da parte");
    }

    #[test]
    fn acima_da_quota_e_visivel_e_nao_disfarcado() {
        let over = json!({
            "storage": {
                "used_bytes": 11_000_000_000_i64,
                "limit_bytes": 10_737_418_240_i64,
                "available_bytes": 0,
                "state": "over_quota"
            },
            "storage_entitlement": {"parts": []}
        });
        let html = resources(&over).to_html();
        assert!(
            html.contains("Acima da quota"),
            "o estado é dito em voz alta"
        );
        // A barra satura em 100%, não estoura.
        assert!(html.contains(r#"aria-valuenow="100""#));
    }

    #[test]
    fn sem_limite_nao_inventa_percentagem() {
        let none = json!({
            "storage": {"used_bytes": 1024, "limit_bytes": 0, "available_bytes": 0, "state": "normal"},
            "storage_entitlement": {"parts": []}
        });
        let html = resources(&none).to_html();
        assert!(html.contains("sem limite"), "diz que não há limite");
        assert!(
            !html.contains("progressbar"),
            "não desenha uma barra sem limite"
        );
    }
}
