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
use crate::ui::ods;
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

/// A lista das notas do membro, com as pastas, o filtro por etiqueta e o criar
/// (D8 + D12_NOTES: moldura `.ods-app`, pastas na coluna lateral).
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
        <div class="ods-app">
            <div class="ods-app__toolbar">
                <h1 class="ods-page__title">{t("notes.title")}</h1>
                <span class="ods-app__toolbar-spacer"></span>
                <a class="ods-btn ods-btn--sm" href="/notes/lixo">{t("notes.trash")}</a>
                <a class="ods-btn ods-btn--sm" href="/notes/partilhadas">{t("notes.shared_with_me")}</a>
                <form method="post" action="/notes">
                    <button type="submit" class="ods-btn ods-btn--primary ods-btn--sm">{t("notes.new")}</button>
                </form>
            </div>
            <div class="ods-app__split">
                <nav class="ods-app__side" aria-label=t("notes.title")>
                    <a
                        class="ods-app__side-item"
                        href="/notes"
                        aria-current=active_folder.is_none().then_some("page")
                    >{t("notes.folder.all")}</a>
                    {folder_rows.iter().map(|folder| {
                        let fid = field(folder, "id").to_owned();
                        let fname = field(folder, "name").to_owned();
                        let is_active = active_folder.as_deref() == Some(fid.as_str());
                        let alvo = format!("/notes?folder={}", encode_query(&fid));
                        view! {
                            <a
                                class="ods-app__side-item"
                                href=alvo
                                aria-current=is_active.then_some("page")
                                data-oc-content="1"
                            >{fname}</a>
                        }
                    }).collect::<Vec<_>>()}
                    <form method="post" action="/notes/folders">
                        <label class="ods-field">
                            <span class="ods-sr-only">{t("notes.folder.new_aria")}</span>
                            <input
                                class="ods-input"
                                data-part="notes-newfolder__input"
                                type="text"
                                name="name"
                                placeholder=t("notes.folder.new_placeholder")
                                maxlength="120"
                                required
                            />
                        </label>
                        <button class="ods-btn ods-btn--ghost ods-btn--sm ods-btn--block" type="submit">
                            {t("notes.folder.create")}
                        </button>
                    </form>
                </nav>

                <div class="ods-app__main" data-ods-scroll>
                    {active_tag.clone().map(|tag| view! {
                        <div class="ods-notice" role="status">
                            <span>{t("notes.filter.tag")} " " <span class="ods-chip" data-oc-content="1">{tag}</span></span>
                            <a class="ods-btn ods-btn--ghost ods-btn--sm" href="/notes">{t("notes.filter.view_all")}</a>
                        </div>
                    })}

                    {active_folder.clone().map(|fid| {
                        let name = active_folder_name.clone().unwrap_or_else(|| t("notes.folder.fallback").to_owned());
                        let apagar = format!("/notes/folders/{fid}/apagar");
                        view! {
                            <div class="ods-notice" role="status">
                                <span>{t("notes.filter.folder")} " " <strong data-oc-content="1">{name}</strong></span>
                                <a class="ods-btn ods-btn--ghost ods-btn--sm" href="/notes">{t("notes.filter.view_all")}</a>
                                <form method="post" action=apagar>
                                    <button type="submit" class="ods-btn ods-btn--danger-soft ods-btn--sm">{t("notes.folder.delete")}</button>
                                </form>
                            </div>
                        }
                    })}

                    {if has_notes {
                        view! {
                            <div class="ods-notes__list">
                                {rows.iter().map(note_card).collect::<Vec<_>>()}
                            </div>
                        }
                        .into_any()
                    } else {
                        let (titulo, corpo) = if active_tag.is_some() {
                            ("notes.empty.tag.title", "notes.empty.tag.body")
                        } else if active_folder.is_some() {
                            ("notes.empty.folder.title", "notes.empty.folder.body")
                        } else {
                            ("notes.empty.all.title", "notes.empty.all.body")
                        };
                        ods::vazio("notes", t(titulo).to_owned(), Some(t(corpo).to_owned())).into_any()
                    }}
                </div>
            </div>
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

    // Um `div`, e não um `a`: o título é a ligação para a nota, e cada
    // etiqueta a sua própria ligação para o filtro — um `a` dentro de um `a`
    // seria HTML inválido.
    view! {
        <div class="ods-notes__list-item">
            <a class="ods-notes__title" href=href data-oc-content="1">{title}</a>
            {(!excerpt.is_empty())
                .then(|| view! { <p class="ods-notes__excerpt" data-oc-content="1">{excerpt}</p> })}
            {(!tags.is_empty()).then(|| view! {
                <div class="ods-chips">
                    {tags.iter().map(|tag| {
                        let alvo = format!("/notes?tag={}", encode_query(tag));
                        view! { <a class="ods-chip" href=alvo data-oc-content="1">{tag.clone()}</a> }
                    }).collect::<Vec<_>>()}
                </div>
            })}
            <p class="ods-label">{updated}</p>
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

    view! {
        <div class="ods-app">
            <div class="ods-app__toolbar">
                <h1 class="ods-page__title">{t("notes.shared_with_me")}</h1>
                <span class="ods-app__toolbar-spacer"></span>
                <a class="ods-btn ods-btn--sm" href="/notes">{t("notes.shared.my_notes")}</a>
            </div>
            <div class="ods-app__main" data-ods-scroll>
                <p class="ods-page__sub">{t("notes.shared.subtitle")}</p>
                {if rows.is_empty() {
                    ods::vazio(
                        "notes",
                        t("notes.shared.empty.title").to_owned(),
                        Some(t("notes.shared.empty.body").to_owned()),
                    )
                    .into_any()
                } else {
                    view! {
                        <div class="ods-notes__list">{rows.iter().map(note_card).collect::<Vec<_>>()}</div>
                    }
                    .into_any()
                }}
            </div>
        </div>
    }
}

/// O Lixo: as notas apagadas do membro, com restaurar e eliminar de vez.
///
/// Apagar é reversível (ADR-0413 §7): daqui uma nota volta à vida ou desaparece
/// para sempre — e eliminar de vez é um segundo passo deliberado, não o primeiro.
pub fn notes_trash(_viewer: &Viewer, payload: &Value) -> impl IntoView {
    let rows = payload.as_array().cloned().unwrap_or_default();

    view! {
        <div class="ods-app">
            <div class="ods-app__toolbar">
                <h1 class="ods-page__title">{t("notes.trash")}</h1>
                <span class="ods-app__toolbar-spacer"></span>
                <a class="ods-btn ods-btn--sm" href="/notes">{crate::i18n::t("notes.my_notes")}</a>
            </div>
            <div class="ods-app__main" data-ods-scroll>
                <p class="ods-page__sub">{t("notes.trash.subtitle")}</p>
                {if rows.is_empty() {
                    ods::vazio(
                        "trash",
                        t("notes.trash.empty.title").to_owned(),
                        Some(t("notes.trash.empty.body").to_owned()),
                    )
                    .into_any()
                } else {
                    view! {
                        <ul class="ods-notes__list">
                            {rows.iter().map(|note| {
                                let id = field(note, "id").to_owned();
                                let titulo = {
                                    let bruto = field(note, "title");
                                    if bruto.is_empty() { t("notes.untitled").to_owned() } else { bruto.to_owned() }
                                };
                                let restaurar = format!("/notes/{id}/restaurar");
                                let eliminar = format!("/notes/{id}/eliminar");
                                view! {
                                    <li class="ods-notes__list-item">
                                        <span class="ods-notes__title" data-oc-content="1">{titulo}</span>
                                        <div class="ods-boot__actions">
                                            <form method="post" action=restaurar>
                                                <button type="submit" class="ods-btn ods-btn--sm">{t("notes.trash.restore")}</button>
                                            </form>
                                            <form method="post" action=eliminar>
                                                <button type="submit" class="ods-btn ods-btn--danger-soft ods-btn--sm">{t("notes.trash.purge")}</button>
                                            </form>
                                        </div>
                                    </li>
                                }
                            }).collect::<Vec<_>>()}
                        </ul>
                    }
                    .into_any()
                }}
            </div>
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
        <div class="ods-app">
            <div class="ods-app__toolbar">
                <a class="ods-btn ods-btn--ghost ods-btn--sm" href="/notes">{t("notes.editor.back")}</a>
                <span class="ods-app__toolbar-spacer"></span>
                {is_owner.then(|| {
                    let apagar = format!("/notes/{id}/apagar");
                    view! {
                        <form method="post" action=apagar>
                            <button type="submit" class="ods-btn ods-btn--danger-soft ods-btn--sm">{t("notes.editor.delete")}</button>
                        </form>
                    }
                })}
            </div>

            {(!is_owner).then(|| view! {
                <div class="ods-notice" role="status">{t("notes.editor.shared_banner")}</div>
            })}

            // O aviso de tempo real: escondido até o socket dizer que a nota
            // mudou noutro sítio. Não recarrega — informa, e a pessoa decide
            // (ADR-0413 §9).
            <div class="ods-notice ods-notice--warning" data-oc-notes-live="" hidden>
                {t("notes.editor.live")}
            </div>

            <div class="ods-app__split">
                // Actividade, Histórico e Partilha em separadores (D12). A coluna
                // lateral do D8, e não a `.ods-drawer` fixa: ver Q-15 — o comportamento de separadores que o `app.js` já
                // tem (`data-oc="tabs"`).
                <aside class="ods-app__side" aria-label=t("notes.history.title")>
                    <div class="ods-tabs" role="tablist" data-oc="tabs">
                        <button type="button" class="ods-tabs__tab" role="tab" aria-selected="true" aria-controls="nota-actividade">
                            {t("notes.activity.title")}
                        </button>
                        <button type="button" class="ods-tabs__tab" role="tab" aria-selected="false" aria-controls="nota-historico" tabindex="-1">
                            {t("notes.history.title")}
                        </button>
                        {is_owner.then(|| view! {
                            <button type="button" class="ods-tabs__tab" role="tab" aria-selected="false" aria-controls="nota-partilha" tabindex="-1">
                                {t("notes.share.title")}
                            </button>
                        })}
                    </div>
                    <div id="nota-actividade" role="tabpanel">{activity_panel(activity)}</div>
                    <div id="nota-historico" role="tabpanel" hidden>{history_panel(&id, revisions)}</div>
                    {is_owner.then(|| view! {
                        <div id="nota-partilha" role="tabpanel" hidden>{share_panel(&id, shares, people)}</div>
                    })}
                </aside>
                <div class="ods-app__main" data-ods-scroll>
                    <div
                        class="ods-editor"
                        data-oc-notes-editor=""
                        data-note-id=id.clone()
                        data-save-url=save_url
                        data-revision=revision.to_string()
                        data-oc-notes-doc=document
                    >
                        <input
                            class="ods-editor__title"
                            data-oc-notes-title=""
                            type="text"
                            value=title
                            placeholder=t("notes.untitled")
                            aria-label=t("notes.editor.title_aria")
                            autocomplete="off"
                        />
                        // O estado de gravação. Dentro do editor, porque é aí que o
                        // ProseMirror o procura (`editor/src/editor.js`).
                        <span class="ods-editor__state" data-oc-notes-status="" aria-live="polite"></span>
                        // Etiquetas e pasta são de quem arruma as suas notas — o
                        // dono. Um editor edita o conteúdo, não a organização.
                        {is_owner.then(|| view! {
                            <div class="ods-boot__actions">
                                <select
                                    class="ods-input"
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
                                <input
                                    class="ods-input"
                                    data-oc-notes-tags=""
                                    type="text"
                                    value=tags.clone()
                                    placeholder=t("notes.editor.tags_placeholder")
                                    aria-label=t("notes.editor.tags_aria")
                                    autocomplete="off"
                                />
                            </div>
                        })}
                        <div data-oc-notes-toolbar="" role="toolbar" aria-label=t("notes.editor.toolbar_aria")></div>
                        <div data-oc-notes-surface=""></div>
                    </div>
                </div>

            </div>

            // O editor vendorizado, same-origin (CSP script-src 'self').
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

/// A actividade da nota (D12: `.ods-timeline`).
fn activity_panel(activity: &Value) -> impl IntoView {
    let rows = activity.as_array().cloned().unwrap_or_default();
    if rows.is_empty() {
        return view! { <p class="ods-empty__body">{t("ods.state.empty")}</p> }.into_any();
    }
    view! {
        <ul class="ods-timeline">
            {rows.iter().map(|entry| {
                let quem = {
                    let a = field(entry, "actor_name");
                    if a.is_empty() { t("notes.someone").to_owned() } else { a.to_owned() }
                };
                let verbo = activity_label(field(entry, "kind"));
                let quando = field(entry, "created_at").split('T').next().unwrap_or("").to_owned();
                view! {
                    <li>
                        <b>{verbo}</b>
                        <p class="ods-label"><span data-oc-content="1">{quem}</span>" · "{quando}</p>
                    </li>
                }
            }).collect::<Vec<_>>()}
        </ul>
    }
    .into_any()
}

/// As revisões da nota (D12: lista de `.ods-menu__item` com data).
fn history_panel(note_id: &str, revisions: &Value) -> impl IntoView {
    let rows = revisions.as_array().cloned().unwrap_or_default();
    let note_id = note_id.to_owned();
    if rows.is_empty() {
        return view! { <p class="ods-empty__body">{t("ods.state.empty")}</p> }.into_any();
    }
    view! {
        <div class="ods-menu">
            <p class="ods-field__hint">{t("notes.history.hint")}</p>
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
                    <a class="ods-menu__item" data-part="notes-history__link" href=ver>
                        <span>{crate::i18n::tf("notes.history.version", &[("n", &numero.to_string())])}</span>
                        <span data-oc-content="1">{titulo}</span>
                        <span class="ods-menu__kbd"><span data-oc-content="1">{autor}</span>" · "{quando}</span>
                    </a>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
    .into_any()
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
        <div class="ods-app">
            <div class="ods-app__toolbar">
                <a class="ods-btn ods-btn--ghost ods-btn--sm" href=voltar.clone()>{t("notes.revision.back")}</a>
                <span class="ods-label">{crate::i18n::tf("notes.revision.subtitle", &[("n", &numero.to_string())])}</span>
                <span class="ods-app__toolbar-spacer"></span>
                {can_write.then(|| view! {
                    <form method="post" action=restaurar.clone()>
                        <input type="hidden" name="base_revision" value=base_revision.to_string() />
                        <button type="submit" class="ods-btn ods-btn--primary ods-btn--sm" title=t("notes.revision.restore_hint")>
                            {t("notes.restore_version")}
                        </button>
                    </form>
                })}
            </div>
            <div class="ods-app__main" data-ods-scroll>
                <article class="ods-editor">
                    <h1 class="ods-editor__title" data-oc-content="1">{title}</h1>
                    <div data-part="notes-reader" inner_html=html></div>
                </article>
            </div>
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

    let dono = {
        let n = field(note, "owner_name");
        if n.is_empty() {
            t("notes.someone").to_owned()
        } else {
            n.to_owned()
        }
    };
    view! {
        <div class="ods-app">
            <div class="ods-app__toolbar">
                <a class="ods-btn ods-btn--ghost ods-btn--sm" href="/notes/partilhadas">{t("notes.reader.back")}</a>
                <span class="ods-badge">{crate::i18n::tf("notes.shared_by", &[("name", &dono)])}</span>
            </div>
            <div class="ods-app__main" data-ods-scroll>
                <article class="ods-editor">
                    <div class="ods-notice" role="status">{t("notes.reader.readonly_banner")}</div>
                    <h1 class="ods-editor__title" data-oc-content="1">{title}</h1>
                    {(!tags.is_empty()).then(|| view! {
                        <div class="ods-chips">
                            {tags.iter().map(|tag| view! {
                                <span class="ods-chip" data-oc-content="1">{tag.clone()}</span>
                            }).collect::<Vec<_>>()}
                        </div>
                    })}
                    // O corpo derivado, escapado pelo Core. O `inner_html` não abre
                    // caminho a script: `to_html` só emite marcação de uma lista
                    // fechada de blocos, e a CSP continua `script-src 'self'`.
                    <div data-part="notes-reader" inner_html=html></div>
                </article>
            </div>
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
        <section>
            <p class="ods-field__hint">{t("notes.share.hint")}</p>

            {if can_share {
                view! {
                    <form method="post" action=share_url>
                        <label class="ods-field">
                            <span class="ods-field__label">{t("notes.share.person_aria")}</span>
                            <select name="person_id" class="ods-input" data-part="notes-share__person" required>
                                {candidates.into_iter().map(|(id, label)| view! {
                                    <option value=id>{label}</option>
                                }).collect::<Vec<_>>()}
                            </select>
                        </label>
                        <label class="ods-field">
                            <span class="ods-field__label">{t("notes.share.access_aria")}</span>
                            <select name="role" class="ods-input" data-part="notes-share__role">
                                <option value="viewer">{t("notes.role.viewer")}</option>
                                <option value="editor">{t("notes.role.editor")}</option>
                            </select>
                        </label>
                        <button type="submit" class="ods-btn ods-btn--primary ods-btn--sm ods-btn--block">{t("notes.share.submit")}</button>
                    </form>
                }
                .into_any()
            } else {
                view! { <p class="ods-empty__body">{t("notes.share.none_left")}</p> }.into_any()
            }}

            {has_shares.then(|| view! {
                <div class="ods-menu">
                    {share_rows.iter().map(|share| {
                        let pid = field(share, "person_id").to_owned();
                        let name = field(share, "person_name").to_owned();
                        let role = role_label(field(share, "role"));
                        let revoke_url = format!("/notes/{note_id}/revogar/{pid}");
                        view! {
                            <div class="ods-menu__item">
                                <span data-oc-content="1">{name}</span>
                                <span class="ods-badge">{role}</span>
                                <form method="post" action=revoke_url>
                                    <button type="submit" class="ods-btn ods-btn--ghost ods-btn--sm">{t("notes.share.revoke")}</button>
                                </form>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>
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
            inactive_apps: Vec::new(),
            perfil: None,
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
