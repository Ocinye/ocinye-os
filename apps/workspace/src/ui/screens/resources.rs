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
/// Como arredondar o último dígito de um tamanho legível.
///
/// A uma casa decimal, `10 GiB − 35 MiB` (9,9655 GiB) arredonda para `10,0 GiB` —
/// e então «em uso 35 MiB» convive com «disponível 10 GiB», que se lê como uma
/// contradição. A cura não é mais precisão em todo o lado, é a direcção certa: o
/// espaço livre nunca se arredonda **para cima** (não se promete espaço que não
/// há), e o usado nunca **para baixo** (não se esconde consumo). Assim os dois
/// números deixam de colidir no mesmo `10,0`.
#[derive(Clone, Copy)]
enum Arredonda {
    /// Para o valor mais próximo — para grandezas exactas (o limite).
    Perto,
    /// Para baixo — o que sobra (disponível): honesto por defeito.
    Baixo,
    /// Para cima — o que se gasta (em uso): honesto por defeito.
    Cima,
}

fn human_bytes(value: i64) -> String {
    human_bytes_com(value, Arredonda::Perto)
}

fn human_bytes_com(value: i64, arredonda: Arredonda) -> String {
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
        return format!("{value} B");
    }
    // Arredondar a uma casa decimal na direcção pedida, com o valor já na sua
    // unidade. `Perto` é o `{:.1}` de sempre; `Baixo`/`Cima` usam `floor`/`ceil`
    // sobre o valor multiplicado por dez.
    let escalado = size * 10.0;
    let uma_casa = match arredonda {
        Arredonda::Perto => escalado.round(),
        Arredonda::Baixo => escalado.floor(),
        Arredonda::Cima => escalado.ceil(),
    } / 10.0;
    format!("{uma_casa:.1} {}", UNITS[unit])
}

/// The state's localised label and badge tone.
///
/// A state the Core did not send, or one we do not know, reads as unknown rather
/// than as a reassuring "Normal": we never dress an unknown as fine. O rótulo
/// resolve-se no idioma corrente; o tom deriva do estado, não da língua.
fn state_badge(state: &str) -> (&'static str, Tone) {
    match state {
        "normal" => (crate::i18n::t("resources.state.normal"), Tone::Ok),
        "warning" => (crate::i18n::t("resources.state.warning"), Tone::Warn),
        "critical" => (crate::i18n::t("resources.state.critical"), Tone::Warn),
        "over_quota" => (crate::i18n::t("resources.state.over_quota"), Tone::Err),
        _ => (crate::i18n::t("resources.state.unknown"), Tone::Gray),
    }
}

/// A part's localised label for where the entitlement came from.
fn source_label(source: &str) -> &'static str {
    match source {
        "profile" => crate::i18n::t("resources.source.profile"),
        "override" => crate::i18n::t("resources.source.override"),
        "temporary" => crate::i18n::t("resources.source.temporary"),
        _ => crate::i18n::t("resources.source.other"),
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

    // O usado arredonda para cima e o disponível para baixo, para que nunca se
    // colapsem no mesmo valor do limite: 35 MiB em uso deixam de conviver com
    // 10,0 GiB livres num limite de 10,0 GiB.
    let used_h = human_bytes_com(used, Arredonda::Cima);
    let limit_h = if has_limit {
        human_bytes(limit)
    } else {
        crate::i18n::t("resources.no_limit").to_owned()
    };
    let available_h = if has_limit {
        human_bytes_com(available, Arredonda::Baixo)
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
                        <h1>{crate::i18n::t("resources.title")}</h1>
                        <p>
    {crate::i18n::t("resources.intro")}
                        </p>
                    </div>
                </div>
            </div>

            <div class="oc-page">
                <section class="oc-card oc-card__body oc-mb-5">
                    <div class="oc-row oc-row--between oc-mb-5">
                        <div>
                            <h2>{crate::i18n::t("resources.storage.title")}</h2>
                            <p class="oc-t-meta">
                                {crate::i18n::t("resources.storage.subtitle")}
                            </p>
                        </div>
                        {badge(state_text, state_tone)}
                    </div>

                    {if has_limit {
                        progress_bar(pct).into_any()
                    } else {
                        view! {
                            <p class="oc-t-meta">
                                {crate::i18n::t("resources.no_limit_assigned")}
                            </p>
                        }
                        .into_any()
                    }}

                    <div class="oc-grid oc-grid--3 oc-mt-5">
                        {metric(crate::i18n::t("resources.in_use"), &used_h)}
                        {metric(crate::i18n::t("resources.limit"), &limit_h)}
                        {metric(crate::i18n::t("resources.available"), &available_h)}
                    </div>
                </section>

                <section class="oc-card oc-card__body">
                    <div class="oc-mb-5">
                        <h2>{crate::i18n::t("resources.origin.title")}</h2>
                        <p class="oc-t-meta">
    {crate::i18n::t("resources.origin.help")}
                        </p>
                    </div>

                    {if parts.is_empty() {
                        view! {
                            <p class="oc-t-meta">
                                {crate::i18n::t("resources.no_allocation")}
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
                                        crate::i18n::tf("resources.expires", &[("date", &when.chars().take(10).collect::<String>())])
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
    {crate::i18n::t("resources.footnote")}
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
    fn o_disponivel_nunca_arredonda_para_o_limite_inteiro() {
        // O caso relatado: 10 GiB de limite, ~35 MiB usados. O disponível é
        // 9,9655 GiB — que a uma casa arredondava para «10,0 GiB», igual ao
        // limite, e então «em uso 35 MiB» convive com «disponível 10 GiB».
        let used = 37_000_000_i64; // ~35,3 MiB
        let limit = 10_i64 * 1024 * 1024 * 1024; // 10 GiB
        let available = limit - used;

        // Disponível para baixo: 9,9 GiB, e nunca o limite inteiro.
        assert_eq!(human_bytes_com(available, Arredonda::Baixo), "9.9 GiB");
        assert_ne!(
            human_bytes_com(available, Arredonda::Baixo),
            human_bytes(limit),
            "o disponível não pode ler-se igual ao limite quando há uso"
        );
        // Em uso para cima: 35 MiB não desaparecem no arredondamento.
        assert_eq!(human_bytes_com(used, Arredonda::Cima), "35.3 MiB");

        // E no ecrã: a métrica «Disponível» mostra 9,9 GiB, o limite 10,0 GiB.
        let me = json!({
            "storage": {
                "used_bytes": used,
                "limit_bytes": limit,
                "available_bytes": available,
                "state": "normal"
            },
            "storage_entitlement": {"quantity": limit, "parts": []}
        });
        let html = resources(&me).to_html();
        assert!(html.contains("9.9 GiB"), "disponível legível e honesto");
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

#[cfg(test)]
mod pureza {
    use super::*;
    use serde_json::json;

    /// Um ecrã, um idioma: «Meus Recursos» em francês, sem marcas portuguesas.
    #[tokio::test]
    async fn os_recursos_nao_misturam_linguas() {
        use crate::i18n::{with_locale, Locale};
        let me = json!({
            "storage": {"used_bytes": 100, "limit_bytes": 1000, "available_bytes": 900, "state": "normal"},
            "storage_entitlement": {"quantity": 1000, "parts": []}
        });
        let fr = with_locale(Locale::Fr, async { resources(&me).to_html() }).await;
        for francesa in [
            "Mes ressources",
            "Stockage personnel",
            "Utilisé",
            "Disponible",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        for portuguesa in ["Meus Recursos", "Armazenamento pessoal"] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português «{portuguesa}»"
            );
        }
    }
}
