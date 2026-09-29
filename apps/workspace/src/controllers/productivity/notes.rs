//! D004 · Notas: o que o Core diz das notas do membro, na forma do Design.
//!
//! O texto do editor sai do documento canónico por [`note_markdown`], e volta
//! a ele na gravação; a gravação é explícita, com a revisão em que o editor
//! abriu, e um conflito é uma resposta tipada que guarda o texto do membro.

use serde_json::Value;

use super::note_markdown;
use crate::controllers::desktop::{instant, text, Clock};
use crate::i18n::t;
use crate::ui::view_models::{AppNavVm, AppSaveState, NoteEditorVm, NoteItemVm};

/// As secções que o Core serve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// As notas do membro.
    Mine,
    /// As que outra pessoa partilhou com ele.
    Shared,
    /// As apagadas (restaurar, eliminar de vez).
    Trash,
}

impl Section {
    /// O caminho da secção.
    #[must_use]
    pub const fn href(self) -> &'static str {
        match self {
            Self::Mine => "/notes",
            Self::Shared => "/notes/partilhadas",
            Self::Trash => "/notes/lixo",
        }
    }

    /// A lista do Core.
    #[must_use]
    pub const fn api(self) -> &'static str {
        match self {
            Self::Mine => "/api/v1/me/notes?page_size=100",
            Self::Shared => "/api/v1/me/shared-notes",
            Self::Trash => "/api/v1/me/deleted-notes",
        }
    }
}

/// A navegação: as três secções, sem contagens (o Core não as dá).
#[must_use]
pub fn nav(current: Section) -> Vec<AppNavVm> {
    [
        (Section::Mine, "prod.notes.mine", "notes"),
        (Section::Shared, "prod.notes.shared", "share"),
        (Section::Trash, "prod.notes.trash", "trash"),
    ]
    .into_iter()
    .map(|(s, key, icon)| AppNavVm {
        label: t(key).to_owned(),
        icon,
        href: s.href().to_owned(),
        count: None,
        active: s == current,
    })
    .collect()
}

fn title_or_untitled(title: &str) -> String {
    if title.trim().is_empty() {
        t("notes.untitled").to_owned()
    } else {
        title.to_owned()
    }
}

/// Os itens da lista (`PersonalNoteSummary`).
#[must_use]
pub(crate) fn items(
    list: &[Value],
    section: Section,
    active: Option<&str>,
    clock: &Clock,
) -> Vec<NoteItemVm> {
    list.iter()
        .filter_map(|n| {
            let id = text(n, "id");
            if id.is_empty() {
                return None;
            }
            Some(NoteItemVm {
                title: title_or_untitled(text(n, "title")),
                modified: instant(n, "updated_at")
                    .map(|at| clock.when(at))
                    .unwrap_or_default(),
                context: None,
                href: format!("/notes/{id}"),
                active: active == Some(id),
                shared: section == Section::Shared,
            })
        })
        .collect()
}

/// Os itens de uma pesquisa (`GET /search?entity_types=note`): só as notas do
/// próprio membro, com a autorização do Core.
#[must_use]
pub fn search_items(hits: &[Value], active: Option<&str>) -> Vec<NoteItemVm> {
    hits.iter()
        .filter(|h| text(h, "entity_type") == "note")
        .filter_map(|h| {
            let id = text(h, "entity_id");
            (!id.is_empty()).then(|| NoteItemVm {
                title: title_or_untitled(text(h, "title")),
                modified: String::new(),
                context: None,
                href: format!("/notes/{id}"),
                active: active == Some(id),
                shared: false,
            })
        })
        .collect()
}

/// O texto de uma nota que só chega em HTML (quem a lê sem a poder editar).
/// O HTML é o que o Core deriva do documento, escapado por construção; aqui só
/// se tiram as etiquetas e se repõem as quebras.
fn text_of_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut tag = String::new();
    for c in html.chars() {
        match (in_tag, c) {
            (false, '<') => {
                in_tag = true;
                tag.clear();
            }
            (true, '>') => {
                in_tag = false;
                let name = tag
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if tag.starts_with('/')
                    && matches!(name, "p" | "h1" | "h2" | "h3" | "li" | "pre" | "ul" | "ol")
                    || name == "br"
                {
                    out.push('\n');
                }
            }
            (true, c) => tag.push(c),
            (false, c) => out.push(c),
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_owned()
}

/// O texto que o membro escreveu e ainda não ficou guardado (depois de uma
/// gravação recusada): volta ao editor tal como estava.
pub struct Pending {
    /// O título.
    pub title: String,
    /// O texto.
    pub body: String,
    /// A revisão em que o editor abriu.
    pub base_revision: i32,
}

/// O editor de uma nota (`PersonalNoteView`), com o estado de gravação.
#[must_use]
pub fn editor(view: &Value, save: AppSaveState, pending: Option<Pending>) -> Option<NoteEditorVm> {
    let id = text(view, "id").to_owned();
    if id.is_empty() {
        return None;
    }
    let access = text(view, "access");
    let from_doc = view.get("document").and_then(note_markdown::to_markdown);
    // Um documento que esta tradução não sabe escrever não se edita aqui: lê-se.
    let read_only = access == "viewer" || (view.get("document").is_some() && from_doc.is_none());
    let stored = from_doc.unwrap_or_else(|| text_of_html(text(view, "html")));
    let revision = view
        .get("revision")
        .and_then(Value::as_i64)
        .and_then(|r| i32::try_from(r).ok())
        .unwrap_or_default();
    let (title, body, revision) = match pending {
        Some(p) => (p.title, p.body, p.base_revision),
        None => (text(view, "title").to_owned(), stored, revision),
    };
    Some(NoteEditorVm {
        save_action: format!("/notes/{id}/gravar"),
        trash_action: (access == "owner").then(|| format!("/notes/{id}/apagar")),
        revisions_href: None,
        nye: Some(super::nye("note", &id, "prod.nye.note")),
        stats: None,
        id,
        title,
        body,
        revision,
        save,
        read_only,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn view(access: &str) -> Value {
        json!({
            "id": "0f0e0d0c-0b0a-4908-8706-050403020100",
            "title": "Torre 2",
            "revision": 4,
            "access": access,
            "document": { "schema_version": 1, "blocks": [
                { "type": "heading", "level": 2, "content": [{ "type": "text", "text": "Vento", "marks": [] }] }
            ]}
        })
    }

    #[test]
    fn o_dono_edita_e_a_revisao_vai_no_formulario() {
        let e = editor(&view("owner"), AppSaveState::Clean, None).unwrap();
        assert_eq!(e.body, "## Vento");
        assert_eq!(e.revision, 4);
        assert!(!e.read_only && e.trash_action.is_some());
        assert_eq!(
            e.save_action,
            "/notes/0f0e0d0c-0b0a-4908-8706-050403020100/gravar"
        );
    }

    #[test]
    fn quem_so_le_nao_edita_nem_apaga() {
        let e = editor(&view("viewer"), AppSaveState::Clean, None).unwrap();
        assert!(e.read_only && e.trash_action.is_none());
    }

    #[test]
    fn um_conflito_guarda_o_texto_do_membro() {
        let e = editor(
            &view("owner"),
            AppSaveState::Clean,
            Some(Pending {
                title: "Meu".into(),
                body: "texto meu".into(),
                base_revision: 3,
            }),
        )
        .unwrap();
        assert_eq!(
            (e.title.as_str(), e.body.as_str(), e.revision),
            ("Meu", "texto meu", 3)
        );
    }

    #[test]
    fn um_bloco_desconhecido_abre_so_para_ler() {
        let mut v = view("owner");
        v["document"]["blocks"] = json!([{ "type": "table" }]);
        v["html"] = json!("<p>a &amp; b</p><p>c</p>");
        let e = editor(&v, AppSaveState::Clean, None).unwrap();
        assert!(e.read_only);
        assert_eq!(e.body, "a & b\nc");
    }
}
