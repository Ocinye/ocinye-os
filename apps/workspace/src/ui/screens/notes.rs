//! Notas pessoais — a lista e o editor.
//!
//! Uma nota é do membro (ADR-0413): o Core resolve o dono pela sessão, e o
//! Workspace nunca decide quem lê o quê. O editor é a única peça de JavaScript
//! vendorizada do Workspace (`/static/notes-editor.js`, apps/workspace/editor);
//! a Experience é simples, e a autoridade, a proveniência e a memória ficam no
//! Core.

use leptos::prelude::*;
use serde_json::Value;

use crate::i18n::t;
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
pub fn notes_list(
    _viewer: &Viewer,
    payload: &Value,
    folders: &Value,
    active_tag: Option<&str>,
    active_folder: Option<&str>,
) -> impl IntoView {
    let rows = payload.as_array().cloned().unwrap_or_default();
    let has_notes = !rows.is_empty();
    let active_tag = active_tag.map(ToOwned::to_owned);
    let folder_rows = folders.as_array().cloned().unwrap_or_default();
    let active_folder = active_folder.map(ToOwned::to_owned);
    // O nome da pasta activa, para a dizer no cabeçalho do filtro.
    let active_folder_name = active_folder.as_deref().and_then(|id| {
        folder_rows
            .iter()
            .find(|f| field(f, "id") == id)
            .map(|f| field(f, "name").to_owned())
    });

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{t("notes.title")}</h1>
                    <p>{t("notes.subtitle")}</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new(t("notes.trash"), Variant::Secondary).href("/notes/lixo"))}
                    {button(Button::new(t("notes.shared_with_me"), Variant::Secondary).href("/notes/partilhadas"))}
                    <form method="post" action="/notes">
                        {button(Button::new(t("notes.new"), Variant::Gold))}
                    </form>
                </div>
            </div>

            <div class="oc-notes-folders">
                <a
                    class=if active_folder.is_none() { "oc-notes-folder is-active" } else { "oc-notes-folder" }
                    href="/notes"
                >{t("notes.folder.all")}</a>
                {folder_rows.iter().map(|folder| {
                    let fid = field(folder, "id").to_owned();
                    let fname = field(folder, "name").to_owned();
                    let is_active = active_folder.as_deref() == Some(fid.as_str());
                    let alvo = format!("/notes?folder={}", encode_query(&fid));
                    view! {
                        <a
                            class=if is_active { "oc-notes-folder is-active" } else { "oc-notes-folder" }
                            href=alvo
                        >{fname}</a>
                    }
                }).collect::<Vec<_>>()}
                <form class="oc-notes-newfolder" method="post" action="/notes/folders">
                    <input
                        class="oc-notes-newfolder__input"
                        type="text"
                        name="name"
                        placeholder=t("notes.folder.new_placeholder")
                        aria-label=t("notes.folder.new_aria")
                        maxlength="120"
                        required
                    />
                    <button class="oc-notes-newfolder__button" type="submit">{t("notes.folder.create")}</button>
                </form>
            </div>

            {active_tag.clone().map(|tag| view! {
                <div class="oc-notes-filter">
                    <span>{t("notes.filter.tag")} <span class="oc-tag" data-oc-content="1">{tag}</span></span>
                    <a class="oc-notes-filter__clear" href="/notes">{t("notes.filter.view_all")}</a>
                </div>
            })}

            {active_folder.clone().map(|fid| {
                let name = active_folder_name.clone().unwrap_or_else(|| t("notes.folder.fallback").to_owned());
                let apagar = format!("/notes/folders/{fid}/apagar");
                view! {
                    <div class="oc-notes-filter">
                        <span>{t("notes.filter.folder")} <strong data-oc-content="1">{name}</strong></span>
                        <a class="oc-notes-filter__clear" href="/notes">{t("notes.filter.view_all")}</a>
                        <form method="post" action=apagar class="oc-notes-filter__delete">
                            <button type="submit" class="oc-notes-filter__delete-btn">{t("notes.folder.delete")}</button>
                        </form>
                    </div>
                }
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
                    title: t("notes.empty.tag.title").to_owned(),
                    body: t("notes.empty.tag.body").to_owned(),
                    actions: Vec::new(),
                    small: false,
                })
                    .into_any()
            } else if active_folder.is_some() {
                empty_state(EmptyState {
                    icon: Icon::Document,
                    title: t("notes.empty.folder.title").to_owned(),
                    body: t("notes.empty.folder.body").to_owned(),
                    actions: Vec::new(),
                    small: false,
                })
                    .into_any()
            } else {
                empty_state(EmptyState {
                    icon: Icon::Document,
                    title: t("notes.empty.all.title").to_owned(),
                    body: t("notes.empty.all.body").to_owned(),
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
        let bruto = field(note, "title");
        if bruto.is_empty() {
            t("notes.untitled").to_owned()
        } else {
            bruto.to_owned()
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
            <a class="oc-note-card__title" href=href data-oc-content="1">{title}</a>
            {(!excerpt.is_empty())
                .then(|| view! { <p class="oc-note-card__excerpt" data-oc-content="1">{excerpt}</p> })}
            {(!tags.is_empty()).then(|| view! {
                <div class="oc-note-card__tags">
                    {tags.iter().map(|tag| {
                        let alvo = format!("/notes?tag={}", encode_query(tag));
                        view! { <a class="oc-tag" href=alvo data-oc-content="1">{tag.clone()}</a> }
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
        Some((date, _)) if !date.is_empty() => {
            crate::i18n::tf("notes.updated_at", &[("date", date)])
        }
        _ => String::new(),
    }
}

/// A lista das notas partilhadas com o membro — as que são de outra pessoa.
///
/// Abrir uma leva a `/notes/{id}`, e é lá que o Core resolve o acesso: leitura
/// ou edição. Esta lista não decide nada — só mostra o que já foi partilhado.
pub fn shared_notes_list(_viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = payload.as_array().cloned().unwrap_or_default();
    let has_notes = !rows.is_empty();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{t("notes.shared_with_me")}</h1>
                    <p>{t("notes.shared.subtitle")}</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new(t("notes.shared.my_notes"), Variant::Secondary).href("/notes"))}
                </div>
            </div>

            {if has_notes {
                view! {
                    <div class="oc-notes-list">
                        {rows.iter().map(note_card).collect::<Vec<_>>()}
                    </div>
                }
                    .into_any()
            } else {
                empty_state(EmptyState {
                    icon: Icon::Document,
                    title: t("notes.shared.empty.title").to_owned(),
                    body: t("notes.shared.empty.body").to_owned(),
                    actions: Vec::new(),
                    small: false,
                })
                    .into_any()
            }}
        </div>
    }
}

/// O Lixo: as notas apagadas do membro, com restaurar e eliminar de vez.
///
/// Apagar é reversível (ADR-0413 §7): daqui uma nota volta à vida ou desaparece
/// para sempre — e eliminar de vez é um segundo passo deliberado, não o primeiro.
pub fn notes_trash(_viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = payload.as_array().cloned().unwrap_or_default();
    let has_notes = !rows.is_empty();

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{t("notes.trash")}</h1>
                    <p>{t("notes.trash.subtitle")}</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new(crate::i18n::t("notes.my_notes"), Variant::Secondary).href("/notes"))}
                </div>
            </div>

            {if has_notes {
                view! {
                    <ul class="oc-notes-trash">
                        {rows.iter().map(|note| {
                            let id = field(note, "id").to_owned();
                            let titulo = {
                                let bruto = field(note, "title");
                                if bruto.is_empty() { t("notes.untitled").to_owned() } else { bruto.to_owned() }
                            };
                            let restaurar = format!("/notes/{id}/restaurar");
                            let eliminar = format!("/notes/{id}/eliminar");
                            view! {
                                <li class="oc-notes-trash__item">
                                    <span class="oc-notes-trash__title" data-oc-content="1">{titulo}</span>
                                    <div class="oc-notes-trash__actions">
                                        <form method="post" action=restaurar>
                                            <button type="submit" class="oc-notes-trash__restore">{t("notes.trash.restore")}</button>
                                        </form>
                                        <form method="post" action=eliminar>
                                            <button type="submit" class="oc-notes-trash__purge">{t("notes.trash.purge")}</button>
                                        </form>
                                    </div>
                                </li>
                            }
                        }).collect::<Vec<_>>()}
                    </ul>
                }
                    .into_any()
            } else {
                empty_state(EmptyState {
                    icon: Icon::Document,
                    title: t("notes.trash.empty.title").to_owned(),
                    body: t("notes.trash.empty.body").to_owned(),
                    actions: Vec::new(),
                    small: false,
                })
                    .into_any()
            }}
        </div>
    }
}

/// A etiqueta legível de um papel de partilha.
fn role_label(role: &str) -> &'static str {
    match role {
        "editor" => t("notes.role.editor"),
        _ => t("notes.role.viewer"),
    }
}

/// O editor de uma nota.
///
/// A `note` é a `PersonalNoteView` do Core: traz o `document` estruturado
/// canónico, a revisão que a próxima gravação apresenta como `base_revision`, o
/// título e o `access` — `owner`, `editor` ou `viewer` (ADR-0413 §9). O
/// documento viaja num atributo de dados — o browser descodifica o valor, e não
/// há como escapar de um `</script>` porque não há bloco inline.
///
/// Quem só tem leitura não recebe editor nenhum: vê o corpo derivado que o Core
/// escapou por construção. Um editor recebe a superfície de edição, mas não o
/// painel de partilha nem o selector de pasta — arrumar e partilhar são do dono.
pub fn note_editor(
    _viewer: &Viewer,
    note: &Value,
    folders: &Value,
    shares: &Value,
    people: &Value,
    revisions: &Value,
    activity: &Value,
) -> impl IntoView {
    let access = field(note, "access");
    let is_viewer = access == "viewer";
    let is_owner = access == "owner";

    if is_viewer {
        return shared_note_reader(note).into_any();
    }

    let id = field(note, "id").to_owned();
    let title = field(note, "title").to_owned();
    let revision = note.get("revision").and_then(Value::as_i64).unwrap_or(0);
    let save_url = format!("/notes/{id}/gravar");
    let move_url = format!("/notes/{id}/mover");
    let tags = tags_of(note).join(", ");
    let current_folder = field(note, "folder_id").to_owned();
    let folder_rows = folders.as_array().cloned().unwrap_or_default();

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
                    <h1>{t("notes.editor.title")}</h1>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new(t("notes.editor.back"), Variant::Secondary).href("/notes"))}
                    {is_owner.then(|| {
                        let apagar = format!("/notes/{id}/apagar");
                        view! {
                            <form method="post" action=apagar class="oc-notes-delete">
                                <button type="submit" class="oc-notes-delete__btn">{t("notes.editor.delete")}</button>
                            </form>
                        }
                    })}
                </div>
            </div>

            {(!is_owner).then(|| view! {
                <div class="oc-notes-shared-banner">
                    {t("notes.editor.shared_banner")}
                </div>
            })}

            // O aviso de tempo real: escondido até o socket dizer que a nota
            // mudou noutro sítio. Não recarrega — informa, e a pessoa decide
            // (ADR-0413 §9). O `app.js` revela-o; sem JavaScript nunca aparece,
            // e a gravação com revisão base continua a proteger contra sobrepor.
            <div class="oc-notes-live" data-oc-notes-live="" hidden>
                {t("notes.editor.live")}
            </div>

            <div
                class="oc-notes-editor"
                data-oc-notes-editor=""
                data-note-id=id.clone()
                data-save-url=save_url
                data-revision=revision.to_string()
                data-oc-notes-doc=document
            >
                // O cabeçalho do documento: o título é o título da nota, e os
                // metadados vivem por baixo, numa linha calma. Não é um painel de
                // formulário — é a folha de rosto do documento (correcção de
                // experiência: uma nota é um documento, não um campo de texto).
                <header class="oc-notes-dochead">
                    <input
                        class="oc-notes-title-input"
                        data-oc-notes-title=""
                        type="text"
                        value=title
                        placeholder=t("notes.untitled")
                        aria-label=t("notes.editor.title_aria")
                        autocomplete="off"
                    />
                    // Etiquetas e pasta são de quem arruma as suas notas — o dono.
                    // Um editor edita o conteúdo, não a organização do dono.
                    {is_owner.then(|| view! {
                        <div class="oc-notes-meta">
                            <select
                                class="oc-notes-folder-select"
                                data-oc-notes-folder=""
                                data-move-url=move_url.clone()
                                aria-label=t("notes.editor.folder_aria")
                            >
                                <option value="" selected=current_folder.is_empty()>{t("notes.editor.no_folder")}</option>
                                {folder_rows.iter().map(|folder| {
                                    let fid = field(folder, "id").to_owned();
                                    let fname = field(folder, "name").to_owned();
                                    let selected = fid == current_folder;
                                    view! { <option value=fid selected=selected>{fname}</option> }
                                }).collect::<Vec<_>>()}
                            </select>
                            <span class="oc-notes-meta__sep" aria-hidden="true">"·"</span>
                            <input
                                class="oc-notes-tags-input"
                                data-oc-notes-tags=""
                                type="text"
                                value=tags.clone()
                                placeholder=t("notes.editor.tags_placeholder")
                                aria-label=t("notes.editor.tags_aria")
                                autocomplete="off"
                            />
                        </div>
                    })}
                </header>
                <div class="oc-notes-toolbar" data-oc-notes-toolbar="" role="toolbar" aria-label=t("notes.editor.toolbar_aria")></div>
                <div class="oc-notes-surface" data-oc-notes-surface=""></div>
                <div class="oc-notes-status" data-oc-notes-status="" aria-live="polite"></div>
            </div>

            {is_owner.then(|| share_panel(&id, shares, people))}

            {history_panel(&id, revisions)}

            {activity_panel(activity)}

            // O editor vendorizado, same-origin (CSP script-src 'self'). Só esta
            // página o carrega; monta-se sozinho sobre o elemento acima.
            <script src="/static/notes-editor.js" defer></script>
        </div>
    }
    .into_any()
}

/// O painel de histórico de uma nota — as revisões, da mais recente para a mais
/// antiga, cada uma com quem a escreveu e quando, e uma ligação para a ver.
///
/// Restaurar não apaga nada: repõe uma revisão antiga como revisão nova
/// (ADR-0413 §6). Só aparece quando há histórico — uma nota acabada de criar não
/// tem revisões anteriores, e um painel vazio seria ruído.
/// A etiqueta legível de um verbo de actividade.
fn activity_label(kind: &str) -> &'static str {
    match kind {
        "created" => t("notes.activity.created"),
        "shared" => t("notes.activity.shared"),
        "revoked" => t("notes.activity.revoked"),
        "deleted" => t("notes.activity.deleted"),
        "restored" => t("notes.activity.restored"),
        "updated" => t("notes.activity.updated"),
        _ => t("notes.activity.other"),
    }
}

/// O painel de actividade de uma nota — quem fez o quê, e quando.
///
/// Os acontecimentos de vida e de acesso da nota (criar, partilhar, revogar,
/// apagar, restaurar); as edições vivem no histórico de revisões, ao lado. Só
/// aparece quando há algo a mostrar.
fn activity_panel(activity: &Value) -> impl IntoView {
    let rows = activity.as_array().cloned().unwrap_or_default();
    let has_activity = !rows.is_empty();

    has_activity.then(|| view! {
        <section class="oc-notes-history">
            <h2 class="oc-notes-history__title">{t("notes.activity.title")}</h2>
            <ul class="oc-notes-history__list">
                {rows.iter().map(|entry| {
                    let quem = {
                        let a = field(entry, "actor_name");
                        if a.is_empty() { t("notes.someone").to_owned() } else { a.to_owned() }
                    };
                    let verbo = activity_label(field(entry, "kind"));
                    let quando = field(entry, "created_at").split('T').next().unwrap_or("").to_owned();
                    view! {
                        <li class="oc-notes-history__item">
                            <span class="oc-notes-history__note-title">{verbo}</span>
                            <span class="oc-notes-history__meta"><span data-oc-content="1">{quem}</span>" · "{quando}</span>
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ul>
        </section>
    })
}

fn history_panel(note_id: &str, revisions: &Value) -> impl IntoView {
    let rows = revisions.as_array().cloned().unwrap_or_default();
    let has_history = !rows.is_empty();
    let note_id = note_id.to_owned();

    has_history.then(|| view! {
        <section class="oc-notes-history">
            <h2 class="oc-notes-history__title">{t("notes.history.title")}</h2>
            <p class="oc-notes-history__hint">
                {t("notes.history.hint")}
            </p>
            <ul class="oc-notes-history__list">
                {rows.iter().map(|rev| {
                    let numero = rev.get("revision").and_then(Value::as_i64).unwrap_or(0);
                    let autor = {
                        let a = field(rev, "author_name");
                        if a.is_empty() { t("notes.history.unknown_author").to_owned() } else { a.to_owned() }
                    };
                    let quando = updated_label(field(rev, "created_at"));
                    let titulo = field(rev, "title").to_owned();
                    let ver = format!("/notes/{note_id}/revisoes/{numero}");
                    view! {
                        <li class="oc-notes-history__item">
                            <a class="oc-notes-history__link" href=ver>
                                <span class="oc-notes-history__rev">{crate::i18n::tf("notes.history.version", &[("n", &numero.to_string())])}</span>
                                <span class="oc-notes-history__note-title" data-oc-content="1">{titulo}</span>
                            </a>
                            <span class="oc-notes-history__meta"><span data-oc-content="1">{autor}</span>" · "{quando}</span>
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ul>
        </section>
    })
}

/// A pré-visualização de uma revisão antiga, em leitura, com o restauro.
///
/// O corpo é o HTML que o Core derivou dessa revisão exacta — escapado por
/// construção, como a vista de leitura de uma nota partilhada. O botão de
/// restaurar só aparece a quem pode escrever a nota; o Core recusa na mesma quem
/// não pode.
pub fn revision_preview(
    _viewer: &Viewer,
    note_id: &str,
    rev: &Value,
    base_revision: i64,
    can_write: bool,
) -> impl IntoView {
    let numero = rev.get("revision").and_then(Value::as_i64).unwrap_or(0);
    let title = {
        let bruto = field(rev, "title");
        if bruto.is_empty() {
            t("notes.untitled").to_owned()
        } else {
            bruto.to_owned()
        }
    };
    let html = field(rev, "html").to_owned();
    let voltar = format!("/notes/{note_id}");
    let restaurar = format!("/notes/{note_id}/revisoes/{numero}/restaurar");

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1 data-oc-content="1">{title}</h1>
                    <p>{crate::i18n::tf("notes.revision.subtitle", &[("n", &numero.to_string())])}</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new(t("notes.revision.back"), Variant::Secondary).href(&voltar))}
                </div>
            </div>

            {can_write.then(|| view! {
                <div class="oc-notes-history-actions">
                    <form method="post" action=restaurar.clone()>
                        <input type="hidden" name="base_revision" value=base_revision.to_string() />
                        {button(Button::new(t("notes.revision.restore"), Variant::Gold))}
                    </form>
                    <span class="oc-notes-history-actions__hint">
                        {t("notes.revision.restore_hint")}
                    </span>
                </div>
            })}

            <div class="oc-notes-reader" inner_html=html></div>
        </div>
    }
}

/// A vista de leitura de uma nota partilhada — sem editor.
///
/// O corpo é o HTML que o Core derivou e escapou por construção (`to_html`),
/// e as imagens servem-se pela mesma rota same-origin do editor, que o Core
/// autoriza a quem a nota foi partilhada. Nenhuma peça de edição é montada.
fn shared_note_reader(note: &Value) -> impl IntoView {
    let title = {
        let bruto = field(note, "title");
        if bruto.is_empty() {
            t("notes.untitled").to_owned()
        } else {
            bruto.to_owned()
        }
    };
    let html = field(note, "html").to_owned();
    let tags = tags_of(note);

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1 data-oc-content="1">{title}</h1>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new(t("notes.reader.back"), Variant::Secondary).href("/notes/partilhadas"))}
                </div>
            </div>

            <div class="oc-notes-shared-banner">
                {t("notes.reader.readonly_banner")}
            </div>

            {(!tags.is_empty()).then(|| view! {
                <div class="oc-note-card__tags">
                    {tags.iter().map(|tag| view! {
                        <span class="oc-tag" data-oc-content="1">{tag.clone()}</span>
                    }).collect::<Vec<_>>()}
                </div>
            })}

            // O corpo derivado, escapado pelo Core. O `inner_html` não abre
            // caminho a script: `to_html` só emite marcação de uma lista fechada
            // de blocos, e a CSP do Workspace continua `script-src 'self'`.
            <div class="oc-notes-reader" inner_html=html></div>
        </div>
    }
}

/// O painel de partilha de uma nota — só o dono o vê.
///
/// Concede acesso a uma pessoa (leitura ou edição) e revoga o que já concedeu.
/// A lista de pessoas exclui já quem tem partilha viva, para não oferecer uma
/// segunda concessão à mesma pessoa. O Core é a autoridade: recusa quem não é
/// dono, e recusa partilhar com quem não pertence à instituição.
fn share_panel(note_id: &str, shares: &Value, people: &Value) -> impl IntoView {
    let share_rows = shares.as_array().cloned().unwrap_or_default();
    let already: Vec<String> = share_rows
        .iter()
        .map(|s| field(s, "person_id").to_owned())
        .collect();

    let candidates: Vec<(String, String)> = people
        .get("items")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|p| {
                    let id = field(p, "id");
                    if id.is_empty() || already.iter().any(|a| a == id) {
                        return None;
                    }
                    let name = {
                        let n = field(p, "full_name");
                        if n.is_empty() {
                            "—"
                        } else {
                            n
                        }
                    };
                    let email = field(p, "email");
                    Some((id.to_owned(), format!("{name} · {email}")))
                })
                .collect()
        })
        .unwrap_or_default();

    let share_url = format!("/notes/{note_id}/partilhar");
    let has_shares = !share_rows.is_empty();
    let can_share = !candidates.is_empty();
    let note_id = note_id.to_owned();

    view! {
        <section class="oc-notes-share">
            <h2 class="oc-notes-share__title">{t("notes.share.title")}</h2>
            <p class="oc-notes-share__hint">
                {t("notes.share.hint")}
            </p>

            {if can_share {
                view! {
                    <form class="oc-notes-share__form" method="post" action=share_url>
                        <select name="person_id" class="oc-notes-share__person" aria-label=t("notes.share.person_aria") required>
                            {candidates.into_iter().map(|(id, label)| view! {
                                <option value=id>{label}</option>
                            }).collect::<Vec<_>>()}
                        </select>
                        <select name="role" class="oc-notes-share__role" aria-label=t("notes.share.access_aria")>
                            <option value="viewer">{t("notes.role.viewer")}</option>
                            <option value="editor">{t("notes.role.editor")}</option>
                        </select>
                        <button type="submit" class="oc-notes-share__submit">{t("notes.share.submit")}</button>
                    </form>
                }
                    .into_any()
            } else {
                view! {
                    <p class="oc-notes-share__empty">
                        {t("notes.share.none_left")}
                    </p>
                }
                    .into_any()
            }}

            {has_shares.then(|| view! {
                <ul class="oc-notes-share__list">
                    {share_rows.iter().map(|share| {
                        let pid = field(share, "person_id").to_owned();
                        let name = field(share, "person_name").to_owned();
                        let role = role_label(field(share, "role"));
                        let revoke_url = format!("/notes/{note_id}/revogar/{pid}");
                        view! {
                            <li class="oc-notes-share__item">
                                <span class="oc-notes-share__name" data-oc-content="1">{name}</span>
                                <span class="oc-notes-share__badge">{role}</span>
                                <form method="post" action=revoke_url class="oc-notes-share__revoke">
                                    <button type="submit" class="oc-notes-share__revoke-btn">{t("notes.share.revoke")}</button>
                                </form>
                            </li>
                        }
                    }).collect::<Vec<_>>()}
                </ul>
            })}
        </section>
    }
}

#[cfg(test)]
mod pureza {
    use super::*;
    use serde_json::json;

    fn viewer() -> Viewer {
        Viewer {
            pinned: crate::ui::apps::default_pins(),
            resolucao: crate::ui::shell::ResolucaoSessao::Resolvida,
            sessao_privilegiada: false,
            administra: false,
            zona: "UTC".to_owned().try_into().expect("fuso conhecido"),
            avatar: ocinye_contracts::AvatarChoice::Initials,
            email: Some("t@ocinye.com".to_owned()),
            session_expires_in: None,
            name: "Teste".to_owned(),
            organisation: "Ocinye".to_owned(),
            core_status: crate::ui::shell::CoreStatus::Ok,
            temporal: Vec::new(),
            temporal_failure: None,
            unread: 0,
            modules: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    /// Um ecrã, um idioma: as Notas em francês, sem marcas portuguesas.
    #[tokio::test]
    async fn as_notas_nao_misturam_linguas() {
        use crate::i18n::{with_locale, Locale};
        let v = viewer();
        let vazio = json!([]);
        let fr = with_locale(Locale::Fr, async {
            notes_list(&v, &vazio, &vazio, None, None).to_html()
        })
        .await;
        for francesa in [
            "Nouvelle note",
            "Corbeille",
            "Partagées avec moi",
            "Aucune note pour l’instant",
        ] {
            assert!(fr.contains(francesa), "fr: falta «{francesa}»");
        }
        for portuguesa in ["Nova nota", "Partilhadas comigo", "Ainda não há notas"] {
            assert!(
                !fr.contains(portuguesa),
                "fr: chrome português «{portuguesa}»"
            );
        }
    }
}
