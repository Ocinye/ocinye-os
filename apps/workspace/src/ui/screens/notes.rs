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

/// A lista das notas do membro, com o botão de criar.
pub fn notes_list(_viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = payload.as_array().cloned().unwrap_or_default();
    let has_notes = !rows.is_empty();

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

    view! {
        <a class="oc-note-card" href=href>
            <h2 class="oc-note-card__title">{title}</h2>
            {(!excerpt.is_empty())
                .then(|| view! { <p class="oc-note-card__excerpt">{excerpt}</p> })}
            <div class="oc-note-card__meta">{updated}</div>
        </a>
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
