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
                    <h1>"Notas"</h1>
                    <p>"As suas notas. Cada nota é sua, e guarda a sua própria história."</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Partilhadas comigo", Variant::Secondary).href("/notes/partilhadas"))}
                    <form method="post" action="/notes">
                        {button(Button::new("Nova nota", Variant::Gold))}
                    </form>
                </div>
            </div>

            <div class="oc-notes-folders">
                <a
                    class=if active_folder.is_none() { "oc-notes-folder is-active" } else { "oc-notes-folder" }
                    href="/notes"
                >"Todas"</a>
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
                        placeholder="Nova pasta"
                        aria-label="Nome da nova pasta"
                        maxlength="120"
                        required
                    />
                    <button class="oc-notes-newfolder__button" type="submit">"Criar"</button>
                </form>
            </div>

            {active_tag.clone().map(|tag| view! {
                <div class="oc-notes-filter">
                    <span>"Etiqueta: " <span class="oc-tag">{tag}</span></span>
                    <a class="oc-notes-filter__clear" href="/notes">"Ver todas"</a>
                </div>
            })}

            {active_folder.clone().map(|fid| {
                let name = active_folder_name.clone().unwrap_or_else(|| "Pasta".to_owned());
                let apagar = format!("/notes/folders/{fid}/apagar");
                view! {
                    <div class="oc-notes-filter">
                        <span>"Pasta: " <strong>{name}</strong></span>
                        <a class="oc-notes-filter__clear" href="/notes">"Ver todas"</a>
                        <form method="post" action=apagar class="oc-notes-filter__delete">
                            <button type="submit" class="oc-notes-filter__delete-btn">"Apagar pasta"</button>
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
                    title: "Nenhuma nota com esta etiqueta".to_owned(),
                    body: "Nenhuma das suas notas tem esta etiqueta. Veja todas as notas ou \
                           etiquete uma."
                        .to_owned(),
                    actions: Vec::new(),
                    small: false,
                })
                    .into_any()
            } else if active_folder.is_some() {
                empty_state(EmptyState {
                    icon: Icon::Document,
                    title: "Esta pasta está vazia".to_owned(),
                    body: "Nenhuma das suas notas está nesta pasta. Arrume uma aqui pelo editor, \
                           ou veja todas as notas."
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
                    <h1>"Partilhadas comigo"</h1>
                    <p>"Notas que outra pessoa partilhou consigo. Cada uma continua a ser dela."</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("As minhas notas", Variant::Secondary).href("/notes"))}
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
                    title: "Ainda não há notas partilhadas".to_owned(),
                    body: "Quando alguém partilhar uma nota consigo, ela aparece aqui — para \
                           ler, ou para editar, conforme o acesso que lhe deram."
                        .to_owned(),
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
        "editor" => "Edição",
        _ => "Leitura",
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
                    <h1>"Nota"</h1>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Voltar às notas", Variant::Secondary).href("/notes"))}
                </div>
            </div>

            {(!is_owner).then(|| view! {
                <div class="oc-notes-shared-banner">
                    "Esta nota foi partilhada consigo. Pode editá-la; o dono continua a ser quem a criou."
                </div>
            })}

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
                // Etiquetas e pasta são de quem arruma as suas notas — o dono.
                // Um editor edita o conteúdo, não a organização do dono.
                {is_owner.then(|| view! {
                    <input
                        class="oc-notes-tags-input"
                        data-oc-notes-tags=""
                        type="text"
                        value=tags.clone()
                        placeholder="Etiquetas, separadas por vírgulas"
                        aria-label="Etiquetas da nota"
                        autocomplete="off"
                    />
                    <select
                        class="oc-notes-folder-select"
                        data-oc-notes-folder=""
                        data-move-url=move_url.clone()
                        aria-label="Pasta da nota"
                    >
                        <option value="" selected=current_folder.is_empty()>"Sem pasta"</option>
                        {folder_rows.iter().map(|folder| {
                            let fid = field(folder, "id").to_owned();
                            let fname = field(folder, "name").to_owned();
                            let selected = fid == current_folder;
                            view! { <option value=fid selected=selected>{fname}</option> }
                        }).collect::<Vec<_>>()}
                    </select>
                })}
                <div class="oc-notes-toolbar" data-oc-notes-toolbar="" role="toolbar" aria-label="Formatação"></div>
                <div class="oc-notes-surface" data-oc-notes-surface=""></div>
                <div class="oc-notes-status" data-oc-notes-status="" aria-live="polite"></div>
            </div>

            {is_owner.then(|| share_panel(&id, shares, people))}

            {history_panel(&id, revisions)}

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
fn history_panel(note_id: &str, revisions: &Value) -> impl IntoView {
    let rows = revisions.as_array().cloned().unwrap_or_default();
    let has_history = !rows.is_empty();
    let note_id = note_id.to_owned();

    has_history.then(|| view! {
        <section class="oc-notes-history">
            <h2 class="oc-notes-history__title">"Histórico"</h2>
            <p class="oc-notes-history__hint">
                "Cada gravação deixa uma versão. Abra uma para a ver, e restaure-a se quiser — sem perder as posteriores."
            </p>
            <ul class="oc-notes-history__list">
                {rows.iter().map(|rev| {
                    let numero = rev.get("revision").and_then(Value::as_i64).unwrap_or(0);
                    let autor = {
                        let a = field(rev, "author_name");
                        if a.is_empty() { "Autor desconhecido".to_owned() } else { a.to_owned() }
                    };
                    let quando = updated_label(field(rev, "created_at"));
                    let titulo = field(rev, "title").to_owned();
                    let ver = format!("/notes/{note_id}/revisoes/{numero}");
                    view! {
                        <li class="oc-notes-history__item">
                            <a class="oc-notes-history__link" href=ver>
                                <span class="oc-notes-history__rev">{format!("Versão {numero}")}</span>
                                <span class="oc-notes-history__note-title">{titulo}</span>
                            </a>
                            <span class="oc-notes-history__meta">{autor} " · " {quando}</span>
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
        let t = field(rev, "title");
        if t.is_empty() {
            "Sem título".to_owned()
        } else {
            t.to_owned()
        }
    };
    let html = field(rev, "html").to_owned();
    let voltar = format!("/notes/{note_id}");
    let restaurar = format!("/notes/{note_id}/revisoes/{numero}/restaurar");

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{title}</h1>
                    <p>{format!("Versão {numero} desta nota — uma fotografia do que era então.")}</p>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Voltar à nota", Variant::Secondary).href(&voltar))}
                </div>
            </div>

            {can_write.then(|| view! {
                <div class="oc-notes-history-actions">
                    <form method="post" action=restaurar.clone()>
                        <input type="hidden" name="base_revision" value=base_revision.to_string() />
                        {button(Button::new("Restaurar esta versão", Variant::Gold))}
                    </form>
                    <span class="oc-notes-history-actions__hint">
                        "Restaurar repõe esta versão como a mais recente, sem apagar as que vieram depois."
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
        let t = field(note, "title");
        if t.is_empty() {
            "Sem título".to_owned()
        } else {
            t.to_owned()
        }
    };
    let html = field(note, "html").to_owned();
    let tags = tags_of(note);

    view! {
        <div class="oc-page">
            <div class="oc-head">
                <div class="oc-head__text">
                    <h1>{title}</h1>
                </div>
                <div class="oc-head__actions">
                    {button(Button::new("Voltar", Variant::Secondary).href("/notes/partilhadas"))}
                </div>
            </div>

            <div class="oc-notes-shared-banner">
                "Esta nota foi partilhada consigo só para leitura."
            </div>

            {(!tags.is_empty()).then(|| view! {
                <div class="oc-note-card__tags">
                    {tags.iter().map(|tag| view! {
                        <span class="oc-tag">{tag.clone()}</span>
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
            <h2 class="oc-notes-share__title">"Partilha"</h2>
            <p class="oc-notes-share__hint">
                "Dê a uma pessoa acesso a esta nota — só de leitura, ou também de edição."
            </p>

            {if can_share {
                view! {
                    <form class="oc-notes-share__form" method="post" action=share_url>
                        <select name="person_id" class="oc-notes-share__person" aria-label="Pessoa" required>
                            {candidates.into_iter().map(|(id, label)| view! {
                                <option value=id>{label}</option>
                            }).collect::<Vec<_>>()}
                        </select>
                        <select name="role" class="oc-notes-share__role" aria-label="Acesso">
                            <option value="viewer">"Leitura"</option>
                            <option value="editor">"Edição"</option>
                        </select>
                        <button type="submit" class="oc-notes-share__submit">"Partilhar"</button>
                    </form>
                }
                    .into_any()
            } else {
                view! {
                    <p class="oc-notes-share__empty">
                        "Não há mais ninguém com quem partilhar esta nota."
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
                                <span class="oc-notes-share__name">{name}</span>
                                <span class="oc-notes-share__badge">{role}</span>
                                <form method="post" action=revoke_url class="oc-notes-share__revoke">
                                    <button type="submit" class="oc-notes-share__revoke-btn">"Revogar"</button>
                                </form>
                            </li>
                        }
                    }).collect::<Vec<_>>()}
                </ul>
            })}
        </section>
    }
}
