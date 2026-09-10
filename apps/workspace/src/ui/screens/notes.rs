//! Notas pessoais — a lista e o editor.
//!
//! Uma nota é do membro (ADR-0413): o Core resolve o dono pela sessão, e o
//! Workspace nunca decide quem lê o quê. O editor é a única peça de JavaScript
//! vendorizada do Workspace (`/static/notes-editor.js`, apps/workspace/editor);
//! a Experience é simples, e a autoridade, a proveniência e a memória ficam no
//! Core.

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::components::{button, empty_state, Button, EmptyState, Variant};
use crate::ui::icon::Icon;
use crate::ui::shell::Viewer;

fn field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn tags_of(note: &Value) -> Vec<String> {
    note.get("tags")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Codifica um valor para uma query string.
///
/// Pequeno de propósito: uma etiqueta é entrada de utilizador, e vai para um
/// `href` — percent-encoding chega, e uma dependência seria desproporcionada
/// (`CLAUDE.md` §54).
fn encode_query(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// A lista das notas do membro, com o botão de criar e o filtro por etiqueta.
pub fn notes_list(_viewer: &Viewer, payload: &Value, active_tag: Option<&str>) -> impl IntoView {
    let rows = payload.as_array().cloned().unwrap_or_default();
    let has_notes = !rows.is_empty();
    let active_tag = active_tag.map(ToOwned::to_owned);

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Notas"</h1>
                    <p>"As suas notas. Cada nota é sua, e guarda a sua própria história."</p>
                </div>
                <div class="oc-head__actions">
                    <form method="post" action="/notes">
                        {button(Button::new("Nova nota", Variant::Gold))}
                    </form>
                </div>
            </div>

            {active_tag.clone().map(|tag| view! {
                <div class="oc-notes-filter">
                    <span>"Etiqueta: " <span class="oc-tag">{tag}</span></span>
                    <a class="oc-notes-filter__clear" href="/notes">"Ver todas"</a>
                </div>
            })}

            {if has_notes {
                view! {
                    <div class="oc-notes-list">
                        {rows
                            .iter()
                            .map(note_card)
                            .collect::<Vec<_>>()}
                    </div>
                }
                    .into_any()
            } else if active_tag.is_some() {
                empty_state(EmptyState {
                    icon: Icon::Document,
                    title: "Nenhuma nota com esta etiqueta".to_owned(),
                    body: "Nenhuma das suas notas tem esta etiqueta. Veja todas as notas ou \
                           etiquete uma."
                        .to_owned(),
                    actions: Vec::new(),
                    small: false,
                })
                    .into_any()
            } else {
                empty_state(EmptyState {
                    icon: Icon::Document,
                    title: "Ainda não há notas".to_owned(),
                    body: "Uma nota é o sítio para uma ideia solta, um apontamento de reunião \
                           ou uma lista de tarefas. Comece uma."
                        .to_owned(),
                    actions: Vec::new(),
                    small: false,
                })
                    .into_any()
            }}
        </div>
    }
}

fn note_card(note: &Value) -> impl IntoView {
    let id = field(note, "id").to_owned();
    let title = {
        let t = field(note, "title");
        if t.is_empty() {
            "Sem título".to_owned()
        } else {
            t.to_owned()
        }
    };
    let excerpt = field(note, "excerpt").to_owned();
    let updated = updated_label(field(note, "updated_at"));
    let href = format!("/notes/{id}");
    let tags = tags_of(note);

    // O cartão é um `div`, não um `a`: o título é a ligação para a nota, e cada
    // etiqueta é a sua própria ligação para o filtro — um `a` dentro de um `a`
    // seria HTML inválido.
    view! {
        <div class="oc-note-card">
            <a class="oc-note-card__title" href=href>{title}</a>
            {(!excerpt.is_empty())
                .then(|| view! { <p class="oc-note-card__excerpt">{excerpt}</p> })}
            {(!tags.is_empty()).then(|| view! {
                <div class="oc-note-card__tags">
                    {tags.iter().map(|tag| {
                        let alvo = format!("/notes?tag={}", encode_query(tag));
                        view! { <a class="oc-tag" href=alvo>{tag.clone()}</a> }
                    }).collect::<Vec<_>>()}
                </div>
            })}
            <div class="oc-note-card__meta">{updated}</div>
        </div>
    }
}

/// Uma etiqueta legível para a data — a parte da data do ISO-8601, sem a hora.
fn updated_label(iso: &str) -> String {
    match iso.split_once('T') {
        Some((date, _)) if !date.is_empty() => format!("Actualizada a {date}"),
        _ => String::new(),
    }
}

/// O editor de uma nota.
///
/// A `note` é a `PersonalNoteView` do Core: traz o `document` estruturado
/// canónico, a revisão que a próxima gravação apresenta como `base_revision`, e
/// o título. O documento viaja num atributo de dados — o browser descodifica o
/// valor, e não há como escapar de um `</script>` porque não há bloco inline.
pub fn note_editor(_viewer: &Viewer, note: &Value) -> impl IntoView {
    let id = field(note, "id").to_owned();
    let title = field(note, "title").to_owned();
    let revision = note.get("revision").and_then(Value::as_i64).unwrap_or(0);
    let save_url = format!("/notes/{id}/gravar");
    let tags = tags_of(note).join(", ");

    // O documento estruturado, tal como o Core o devolve. Nulo (nota antiga) ou
    // ausente vira uma string vazia, e o editor abre um documento vazio.
    let document = match note.get("document") {
        Some(Value::Null) | None => String::new(),
        Some(doc) => doc.to_string(),
    };

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>"Nota"</h1>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Voltar às notas", Variant::Secondary).href("/notes"))}
                </div>
            </div>

            <div
                class="oc-notes-editor"
                data-oc-notes-editor=""
                data-save-url=save_url
                data-revision=revision.to_string()
                data-oc-notes-doc=document
            >
                <input
                    class="oc-notes-title-input"
                    data-oc-notes-title=""
                    type="text"
                    value=title
                    placeholder="Sem título"
                    aria-label="Título da nota"
                    autocomplete="off"
                />
                <input
                    class="oc-notes-tags-input"
                    data-oc-notes-tags=""
                    type="text"
                    value=tags
                    placeholder="Etiquetas, separadas por vírgulas"
                    aria-label="Etiquetas da nota"
                    autocomplete="off"
                />
                <div class="oc-notes-toolbar" data-oc-notes-toolbar="" role="toolbar" aria-label="Formatação"></div>
                <div class="oc-notes-surface" data-oc-notes-surface=""></div>
                <div class="oc-notes-status" data-oc-notes-status="" aria-live="polite"></div>
            </div>

            // O editor vendorizado, same-origin (CSP script-src 'self'). Só esta
            // página o carrega; monta-se sozinho sobre o elemento acima.
            <script src="/static/notes-editor.js" defer></script>
        </div>
    }
}
