//! Pesquisa institucional.
//!
//! # Porque este ecrã existe
//!
//! O Ocinye Core serve `GET /api/v1/search` desde sempre, e o Workspace não
//! tinha por onde lá chegar: a caixa «Pesquisar no Ocinye…» da topbar abria a
//! command palette, que filtra **navegação** localmente e não procura em nada.
//! Um endpoint implementado e inalcançável de um lado, uma promessa por cumprir
//! do outro (briefing §3, §32).
//!
//! # A autorização acontece na consulta
//!
//! O predicado de visibilidade faz parte do SQL, pelo que o total devolvido
//! conta apenas o que o membro pode ver. Este ecrã mostra o que recebe; nunca
//! filtra depois, e nunca revela contagens de material inacessível.

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{classification_badge, empty_state, EmptyState};
use crate::ui::icon::{icon, Icon};

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Traduz o tipo de entidade para o vocabulário do Workspace.
///
/// Um tipo que este build não conhece é mostrado tal como veio, e não escondido:
/// esconder resultados que o Core devolveu seria filtrar depois.
fn entity_label(entity_type: &str) -> &str {
    match entity_type {
        "idea" => "Ideia",
        "project" => "Projecto",
        "source" => "Referência",
        "note" => "Nota",
        "document" => "Documento",
        "dataset" => "Dataset",
        "unit" => "Unidade",
        other => other,
    }
}

/// O destino de um resultado, quando o Workspace tem ecrã para ele.
fn destination(hit: &Value) -> Option<String> {
    let id = hit.get("entity_id").and_then(Value::as_str)?;
    match text(hit, "entity_type") {
        "idea" | "project" => Some(format!("/workspaces/{id}")),
        "unit" => Some(format!("/units/{id}")),
        // Fontes, notas, documentos e datasets vivem dentro de um Research
        // Workspace; sem ecrã próprio, o resultado leva ao workspace que os
        // contém. Sem workspace conhecido, não leva a lado nenhum — melhor do
        // que uma ligação para 404 (briefing §3).
        _ => hit
            .get("workspace_id")
            .and_then(Value::as_str)
            .map(|workspace| format!("/workspaces/{workspace}")),
    }
}

/// O ecrã de pesquisa.
///
/// `semantic` traz o estado real da pesquisa semântica, apurado pelo Core. Sem
/// capacidade de embeddings o modo semântico é declarado indisponível com a
/// razão — nunca um interruptor que finge funcionar (briefing §32).
pub fn search(query: &str, results: &Value, semantic: &Value) -> impl IntoView {
    let query = query.to_owned();
    let has_query = !query.trim().is_empty();

    let hits: Vec<Value> = results
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let total = results
        .get("total")
        .and_then(Value::as_i64)
        .unwrap_or_else(|| i64::try_from(hits.len()).unwrap_or(0));

    let semantic_available = semantic
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let semantic_message = semantic
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(
            "A pesquisa semântica depende de uma capacidade de embeddings, que não está \
             actualmente disponível.",
        )
        .to_owned();

    let count_label = match total {
        0 => "Nenhum resultado".to_owned(),
        1 => "1 resultado".to_owned(),
        n => format!("{n} resultados"),
    };

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Pesquisar no Ocinye"</h1>
                    <p>
                        "A pesquisa devolve apenas aquilo a que tem acesso. Um artefacto que não
                         possa consultar não aparece, nem nas contagens."
                    </p>
                </div>
            </div>

            <form method="get" action="/search" class="oc-search-form">
                <div class="oc-table__search oc-search-form__field">
                    {icon(Icon::Search, 14)}
                    <label class="oc-sr" for="search-q">"Pesquisar"</label>
                    <input
                        id="search-q"
                        name="q"
                        type="search"
                        value=query.clone()
                        placeholder="Ideias, projectos, bibliografia, documentos, datasets…"
                        autofocus
                    />
                </div>
                <button type="submit" class="oc-btn oc-btn--primary">"Pesquisar"</button>
            </form>

            // O modo semântico é declarado, não escondido: faz parte da
            // arquitectura e o seu estado é informação útil (briefing §32).
            <div class="oc-search-modes" role="group" aria-label="Modo de pesquisa">
                <span class="oc-tab" aria-selected="true">"Textual"</span>
                {if semantic_available {
                    view! {
                        <span class="oc-tab" aria-selected="false">"Semântica"</span>
                    }
                        .into_any()
                } else {
                    view! {
                        <span
                            class="oc-tab oc-unavailable"
                            aria-disabled="true"
                            title=semantic_message.clone()
                        >
                            "Semântica — ainda não disponível"
                        </span>
                    }
                        .into_any()
                }}
            </div>

            {if has_query {
                view! {
                    <p class="oc-muted oc-mt-6">{count_label}</p>
                }
                    .into_any()
            } else {
                view! {
                    <div class="oc-vspace"></div>
                }
                    .into_any()
            }}

            {if !has_query {
                empty_state(EmptyState {
                    icon: Icon::Search,
                    title: "Pesquisar no Ocinye".to_owned(),
                    body: "Escreva um termo para procurar em ideias, projectos, bibliografia, \
                           notas, documentos e datasets."
                        .to_owned(),
                    actions: Vec::new(),
                    small: false,
                })
                .into_any()
            } else if hits.is_empty() {
                empty_state(EmptyState {
                    icon: Icon::Search,
                    title: "Nenhum resultado".to_owned(),
                    // Diz as duas razões possíveis, porque a interface não sabe
                    // qual é — e não deve sugerir que sabe.
                    body: format!(
                        "Nada corresponde a «{query}» entre os artefactos a que tem acesso."
                    ),
                    actions: Vec::new(),
                    small: false,
                })
                .into_any()
            } else {
                view! {
                    <div class="oc-results">
                        {hits
                            .iter()
                            .map(|hit| {
                                let classification = text(hit, "classification").to_owned();
                                let kind = entity_label(text(hit, "entity_type")).to_owned();
                                let title = text(hit, "title").to_owned();
                                let excerpt = hit
                                    .get("excerpt")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_owned();

                                // A vista constrói-se dentro de cada ramo: uma
                                // vista Leptos consome-se uma só vez.
                                let body = move || {
                                    view! {
                                        <div class="oc-row oc-gap-5 oc-mb-2">
                                            <span class="oc-pill">{kind}</span>
                                            {classification_badge(&classification)}
                                        </div>
                                        <div class="oc-t-item">{title}</div>
                                        {(!excerpt.is_empty())
                                            .then(|| {
                                                view! { <p class="oc-muted">{excerpt}</p> }
                                            })}
                                    }
                                };

                                match destination(hit) {
                                    Some(href) => {
                                        view! { <a class="oc-result" href=href>{body()}</a> }
                                            .into_any()
                                    }
                                    None => {
                                        view! { <div class="oc-result">{body()}</div> }.into_any()
                                    }
                                }
                            })
                            .collect_view()}
                    </div>
                }
                .into_any()
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn semantic_off() -> Value {
        json!({
            "available": false,
            "embedded_documents": 0,
            "message": "Nenhuma capacidade de embeddings está disponível."
        })
    }

    #[test]
    fn sem_termo_o_ecra_convida_em_vez_de_dizer_que_nada_existe() {
        let html = search("", &json!({"items": [], "total": 0}), &semantic_off()).to_html();
        assert!(html.contains("Escreva um termo"));
        assert!(
            !html.contains("Nenhum resultado"),
            "não pesquisar não é o mesmo que não encontrar"
        );
    }

    #[test]
    fn sem_resultados_o_ecra_atribui_o_vazio_ao_que_o_membro_pode_ver() {
        let html = search(
            "hidrogénio",
            &json!({"items": [], "total": 0}),
            &semantic_off(),
        )
        .to_html();
        assert!(html.contains("hidrogénio"));
        assert!(html.contains("a que tem acesso"));
    }

    #[test]
    fn a_pesquisa_semantica_indisponivel_e_declarada_com_a_razao() {
        let html = search("x", &json!({"items": []}), &semantic_off()).to_html();
        assert!(html.contains("ainda não disponível"));
        assert!(html.contains("Nenhuma capacidade de embeddings está disponível."));
        assert!(
            html.contains(r#"aria-disabled="true""#),
            "o modo semântico não pode parecer seleccionável"
        );
    }

    #[test]
    fn com_capacidade_o_modo_semantico_deixa_de_estar_declarado_indisponivel() {
        let html = search(
            "x",
            &json!({"items": []}),
            &json!({"available": true, "embedded_documents": 12, "message": "ok"}),
        )
        .to_html();
        assert!(!html.contains("Semântica — ainda não disponível"));
    }

    #[test]
    fn cada_resultado_mostra_a_sua_classificacao() {
        // Um artefacto RESTRICTED não pode parecer igual a um PUBLIC
        // (briefing §105).
        let html = search(
            "x",
            &json!({"items": [{
                "entity_type": "idea",
                "entity_id": "11111111-1111-1111-1111-111111111111",
                "title": "Hidrogénio verde",
                "excerpt": "Estudo preliminar",
                "classification": "RESTRICTED"
            }], "total": 1}),
            &semantic_off(),
        )
        .to_html();

        assert!(html.contains("RESTRICTED"));
        assert!(html.contains("Hidrogénio verde"));
        assert!(html.contains("1 resultado"));
        assert!(html.contains("/workspaces/11111111-1111-1111-1111-111111111111"));
    }

    #[test]
    fn um_resultado_sem_destino_conhecido_nao_finge_uma_ligacao() {
        let html = search(
            "x",
            &json!({"items": [{
                "entity_type": "note",
                "entity_id": "22222222-2222-2222-2222-222222222222",
                "title": "Nota solta",
                "classification": "INTERNAL"
            }], "total": 1}),
            &semantic_off(),
        )
        .to_html();

        assert!(html.contains("Nota solta"));
        assert!(
            !html.contains(r#"href="/notes"#),
            "não deve inventar um ecrã que não existe"
        );
    }

    #[test]
    fn a_contagem_concorda_em_singular_e_plural() {
        let one = search("x", &json!({"items": [], "total": 1}), &semantic_off()).to_html();
        assert!(one.contains("1 resultado") && !one.contains("1 resultados"));
        let many = search("x", &json!({"items": [], "total": 7}), &semantic_off()).to_html();
        assert!(many.contains("7 resultados"));
    }

    #[test]
    fn texto_interpolado_nao_injecta_markup() {
        let html = search(
            "<script>alert(1)</script>",
            &json!({"items": []}),
            &semantic_off(),
        )
        .to_html();
        assert!(!html.contains("<script>alert(1)</script>"));
    }
}
