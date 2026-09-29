//! D004 · Ficheiros: o espaço pessoal do membro (`GET /me/files`), na forma do
//! Design.
//!
//! O Ocinye não é o sistema de ficheiros do anfitrião: nada daqui mostra um
//! bucket, uma chave de objecto ou um caminho do servidor. Os identificadores
//! dos formulários são opacos e do Code (`f.<ficheiro>.<versão>`,
//! `d.<pasta>`), e cada acção volta ao Core, que decide pela posse.

use serde_json::Value;
use uuid::Uuid;

use crate::controllers::desktop::{bytes, instant, text, Clock};
use crate::i18n::t;
use crate::ui::view_models::{
    AppNavVm, CrumbVm, FileDetailsVm, FileItemVm, FileKind, FilePreviewVm, FilesSection,
    FilesSortKey,
};

/// A secção pedida (`?section=`).
#[must_use]
pub fn section_of(s: Option<&str>) -> FilesSection {
    match s {
        Some("recent") => FilesSection::Recent,
        Some("favourites") => FilesSection::Favourites,
        Some("trash") => FilesSection::Trash,
        _ => FilesSection::Mine,
    }
}

/// O parâmetro de uma secção (vazio = os meus ficheiros).
#[must_use]
pub const fn section_param(s: FilesSection) -> &'static str {
    match s {
        FilesSection::Recent => "recent",
        FilesSection::Favourites => "favourites",
        FilesSection::Trash => "trash",
        _ => "",
    }
}

/// A navegação: só as secções que o Core serve (sem «Partilhados comigo» nem
/// «Projectos e unidades», que ainda não existem para ficheiros pessoais).
#[must_use]
pub fn nav(current: FilesSection) -> Vec<AppNavVm> {
    [
        (FilesSection::Mine, "prod.files.mine", "files", "/files"),
        (
            FilesSection::Recent,
            "prod.files.recent",
            "clock",
            "/files?section=recent",
        ),
        (
            FilesSection::Favourites,
            "prod.files.favourites",
            "star",
            "/files?section=favourites",
        ),
        (
            FilesSection::Trash,
            "prod.files.trash",
            "trash",
            "/files?section=trash",
        ),
    ]
    .into_iter()
    .map(|(s, key, icon, href)| AppNavVm {
        label: t(key).to_owned(),
        icon,
        href: href.to_owned(),
        count: None,
        active: s == current,
    })
    .collect()
}

/// O caminho da lista do Core para esta secção e pasta.
#[must_use]
pub fn list_api(section: FilesSection, folder: Option<Uuid>) -> String {
    match section {
        FilesSection::Recent => "/api/v1/me/files?view=recents&limit=200".to_owned(),
        FilesSection::Favourites => "/api/v1/me/files?view=favourites&limit=200".to_owned(),
        FilesSection::Trash => "/api/v1/me/files/trash?limit=200".to_owned(),
        _ => match folder {
            Some(f) => format!("/api/v1/me/files?folder={f}&limit=500"),
            None => "/api/v1/me/files?limit=500".to_owned(),
        },
    }
}

/// O tipo de um ficheiro, pelo tipo MIME que o Core guardou (e pela extensão
/// quando o tipo é genérico). Só escolhe o ícone e a pré-visualização.
#[must_use]
pub fn kind_of(content_type: &str, name: &str) -> FileKind {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    let ct = content_type.to_ascii_lowercase();
    if ct.starts_with("image/") {
        FileKind::Image
    } else if ct == "application/pdf" {
        FileKind::Pdf
    } else if ct.starts_with("audio/") || ct.starts_with("video/") {
        FileKind::Media
    } else if matches!(ext.as_str(), "zip" | "gz" | "tgz" | "tar" | "7z" | "rar") {
        FileKind::Archive
    } else if matches!(
        ext.as_str(),
        "csv" | "tsv" | "xlsx" | "xls" | "ods" | "parquet"
    ) {
        FileKind::Data
    } else if matches!(
        ext.as_str(),
        "docx" | "doc" | "odt" | "pptx" | "ppt" | "odp" | "rtf"
    ) {
        FileKind::Document
    } else if matches!(
        ext.as_str(),
        "rs" | "py"
            | "js"
            | "ts"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "sh"
            | "c"
            | "h"
            | "cpp"
            | "go"
            | "java"
            | "r"
            | "sql"
    ) {
        FileKind::Code
    } else if ct.starts_with("text/") || matches!(ext.as_str(), "txt" | "md") {
        FileKind::Text
    } else {
        FileKind::Other
    }
}

/// O rótulo do tipo, na língua do membro (a extensão em maiúsculas é
/// universal; «Pasta» vem do catálogo).
fn kind_label(kind: FileKind, name: &str) -> String {
    if kind == FileKind::Folder {
        return t("prod.files.folder").to_owned();
    }
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_uppercase())
        .filter(|e| !e.is_empty() && e.len() <= 8)
        .unwrap_or_else(|| "—".to_owned())
}

/// O identificador opaco de um ficheiro nos formulários.
#[must_use]
pub fn file_ref(file: &str, version: &str) -> String {
    format!("f.{file}.{version}")
}

/// O que um identificador opaco nomeia.
#[derive(Debug, PartialEq, Eq)]
pub enum Ref {
    /// Um ficheiro (e a versão corrente, para descarregar).
    File(Uuid, Uuid),
    /// Uma pasta.
    Folder(Uuid),
}

/// Lê um identificador opaco; `None` se não for um (nunca um caminho).
#[must_use]
pub fn parse_ref(s: &str) -> Option<Ref> {
    let mut parts = s.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("f"), Some(f), Some(v), None) => Some(Ref::File(
            Uuid::parse_str(f).ok()?,
            Uuid::parse_str(v).ok()?,
        )),
        (Some("d"), Some(d), None, None) => Some(Ref::Folder(Uuid::parse_str(d).ok()?)),
        _ => None,
    }
}

/// O endereço de uma vista de Ficheiros (secção, pasta, item aberto).
#[must_use]
pub fn href(section: FilesSection, folder: Option<Uuid>, item: Option<&str>) -> String {
    let mut q = Vec::new();
    let s = section_param(section);
    if !s.is_empty() {
        q.push(format!("section={s}"));
    }
    if let Some(f) = folder {
        q.push(format!("folder={f}"));
    }
    if let Some(i) = item {
        q.push(format!("item={i}"));
    }
    if q.is_empty() {
        "/files".to_owned()
    } else {
        format!("/files?{}", q.join("&"))
    }
}

/// A ordenação pedida (`?sort=&dir=`).
#[must_use]
pub fn sort_of(sort: Option<&str>, dir: Option<&str>) -> (FilesSortKey, bool) {
    let key = match sort {
        Some("name") => FilesSortKey::Name,
        Some("size") => FilesSortKey::Size,
        Some("kind") => FilesSortKey::Kind,
        _ => FilesSortKey::Modified,
    };
    let desc = match dir {
        Some("desc") => true,
        Some("asc") => false,
        _ => key == FilesSortKey::Modified,
    };
    (key, desc)
}

struct Row {
    item: FileItemVm,
    sort_name: String,
    sort_size: i64,
    sort_time: i64,
    sort_kind: String,
}

/// Os itens desta vista: pastas primeiro (só na raiz dos meus ficheiros),
/// depois os ficheiros; ordenados como pedido; filtrados pela pesquisa desta
/// pasta. `open` é o item no inspector.
#[must_use]
pub(crate) fn items(
    listing: &Value,
    section: FilesSection,
    folder: Option<Uuid>,
    open: Option<&str>,
    sort: (FilesSortKey, bool),
    query: &str,
    clock: &Clock,
) -> Vec<FileItemVm> {
    let files: &[Value] = listing
        .get("files")
        .and_then(Value::as_array)
        .or_else(|| listing.as_array())
        .map_or(&[], Vec::as_slice);
    let q = query.trim().to_lowercase();
    let keep = |name: &str| q.is_empty() || name.to_lowercase().contains(&q);
    let mut folders = Vec::new();
    if section == FilesSection::Mine && folder.is_none() {
        for f in listing
            .get("folders")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
        {
            let (id, name) = (text(f, "id"), text(f, "name"));
            if id.is_empty() || !keep(name) {
                continue;
            }
            let fid = format!("d.{id}");
            folders.push(Row {
                item: FileItemVm {
                    href: format!("/files?folder={id}"),
                    kind: FileKind::Folder,
                    kind_label: kind_label(FileKind::Folder, name),
                    size: None,
                    modified: String::new(),
                    context: None,
                    thumb: None,
                    favourite: false,
                    shared: false,
                    selected: false,
                    open: open == Some(fid.as_str()),
                    name: name.to_owned(),
                    id: fid,
                },
                sort_name: name.to_lowercase(),
                sort_size: 0,
                sort_time: 0,
                sort_kind: String::new(),
            });
        }
    }
    let mut rows: Vec<Row> = files
        .iter()
        .filter_map(|f| {
            let (id, version, name) = (text(f, "id"), text(f, "version_id"), text(f, "name"));
            if id.is_empty() || version.is_empty() || !keep(name) {
                return None;
            }
            let kind = kind_of(text(f, "content_type"), name);
            let size = f.get("size_bytes").and_then(Value::as_i64).unwrap_or(0);
            let at = instant(f, "updated_at");
            let fid = file_ref(id, version);
            Some(Row {
                item: FileItemVm {
                    href: href(section, folder, Some(&fid)),
                    kind,
                    kind_label: kind_label(kind, name),
                    size: Some(bytes(u64::try_from(size).unwrap_or(0))),
                    modified: at.map(|a| clock.when(a)).unwrap_or_default(),
                    context: None,
                    thumb: None,
                    favourite: f.get("favourite").and_then(Value::as_bool).unwrap_or(false),
                    shared: false,
                    selected: false,
                    open: open == Some(fid.as_str()),
                    name: name.to_owned(),
                    id: fid,
                },
                sort_name: name.to_lowercase(),
                sort_size: size,
                sort_time: at.map_or(0, |a| a.timestamp()),
                sort_kind: kind_label(kind, name),
            })
        })
        .collect();
    let (key, desc) = sort;
    rows.sort_by(|a, b| {
        let o = match key {
            FilesSortKey::Name => a.sort_name.cmp(&b.sort_name),
            FilesSortKey::Size => a.sort_size.cmp(&b.sort_size),
            FilesSortKey::Modified => a.sort_time.cmp(&b.sort_time),
            FilesSortKey::Kind => a
                .sort_kind
                .cmp(&b.sort_kind)
                .then(a.sort_name.cmp(&b.sort_name)),
        };
        if desc {
            o.reverse()
        } else {
            o
        }
    });
    folders.sort_by(|a, b| a.sort_name.cmp(&b.sort_name));
    folders.into_iter().chain(rows).map(|r| r.item).collect()
}

/// O caminho: a secção e, dentro dos meus ficheiros, a pasta (as pastas
/// pessoais são planas).
#[must_use]
pub fn crumbs(section: FilesSection, folder: Option<Uuid>, listing: &Value) -> Vec<CrumbVm> {
    let root = match section {
        FilesSection::Recent => ("prod.files.recent", "/files?section=recent"),
        FilesSection::Favourites => ("prod.files.favourites", "/files?section=favourites"),
        FilesSection::Trash => ("prod.files.trash", "/files?section=trash"),
        _ => ("prod.files.mine", "/files"),
    };
    let mut out = vec![CrumbVm {
        label: t(root.0).to_owned(),
        href: root.1.to_owned(),
    }];
    if let Some(f) = folder {
        let name = listing
            .get("folders")
            .and_then(Value::as_array)
            .and_then(|fs| fs.iter().find(|x| text(x, "id") == f.to_string()))
            .map(|x| text(x, "name").to_owned())
            .unwrap_or_default();
        out.push(CrumbVm {
            label: name,
            href: format!("/files?folder={f}"),
        });
    }
    out
}

/// A pré-visualização segura para este tipo: imagem e PDF pela origem do
/// Workspace (o PDF num `iframe` com sandbox), texto pelo Core (truncado), e o
/// resto sem pré-visualização. Nada se executa.
#[must_use]
pub fn preview_for(content_type: &str, name: &str, version: &str) -> Option<FilePreviewVm> {
    let ct = content_type.to_ascii_lowercase();
    let kind = kind_of(content_type, name);
    if matches!(ct.as_str(), "image/png" | "image/jpeg" | "image/webp") {
        return Some(FilePreviewVm::Image {
            src: format!("/me/files/{version}/inline"),
        });
    }
    if ct == "application/pdf" {
        return Some(FilePreviewVm::Pdf {
            src: format!("/me/files/{version}/inline"),
        });
    }
    if matches!(kind, FileKind::Text | FileKind::Code | FileKind::Data)
        && (ct.starts_with("text/")
            || ct == "application/json"
            || ct.is_empty()
            || ct == "application/octet-stream")
    {
        // Pedido à parte (o texto vem do Core); `None` aqui quer dizer «pedir».
        return None;
    }
    Some(FilePreviewVm::Unsupported)
}

/// O inspector de um ficheiro da lista. As versões dos ficheiros pessoais não
/// têm rota própria no Core: mostra-se «sem histórico» em vez de inventar.
#[must_use]
pub(crate) fn details(
    file: &FileItemVm,
    raw: &Value,
    preview: Option<FilePreviewVm>,
    clock: &Clock,
) -> FileDetailsVm {
    let version = text(raw, "version_id");
    let mut item = file.clone();
    if let Some(at) = instant(raw, "updated_at") {
        item.modified = format!("{} · {}", clock.ddmm(at), clock.hhmm(at));
    }
    FileDetailsVm {
        item,
        created: None,
        owner: None,
        versions: None,
        preview,
        download_href: (!version.is_empty()).then(|| format!("/me/files/{version}/download")),
        shared_with: Vec::new(),
        nye: (!version.is_empty()).then(|| super::nye("file", version, "prod.nye.file")),
    }
}

/// O ficheiro cru da listagem com este identificador opaco.
#[must_use]
pub fn raw_of<'a>(listing: &'a Value, id: &str) -> Option<&'a Value> {
    let Some(Ref::File(f, _)) = parse_ref(id) else {
        return None;
    };
    listing
        .get("files")
        .and_then(Value::as_array)
        .or_else(|| listing.as_array())?
        .iter()
        .find(|x| text(x, "id") == f.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn clock() -> Clock {
        Clock {
            now: chrono::Utc.with_ymd_and_hms(2026, 9, 29, 10, 0, 0).unwrap(),
            zone: ocinye_contracts::temporal::TimeZoneName::parse("UTC").unwrap(),
            core_ok: true,
            is_admin: false,
        }
    }

    fn listing() -> Value {
        json!({
            "files": [
                { "id": "0f0e0d0c-0b0a-4908-8706-050403020100", "version_id": "1f0e0d0c-0b0a-4908-8706-050403020100",
                  "name": "vento.csv", "content_type": "text/csv", "size_bytes": 2048, "updated_at": "2026-09-29T09:00:00Z", "favourite": true },
                { "id": "2f0e0d0c-0b0a-4908-8706-050403020100", "version_id": "3f0e0d0c-0b0a-4908-8706-050403020100",
                  "name": "Anemómetro.png", "content_type": "image/png", "size_bytes": 10, "updated_at": "2026-09-20T09:00:00Z", "favourite": false }
            ],
            "folders": [{ "id": "4f0e0d0c-0b0a-4908-8706-050403020100", "name": "Dados" }]
        })
    }

    #[test]
    fn pastas_primeiro_e_nunca_um_caminho_do_anfitriao() {
        let it = items(
            &listing(),
            FilesSection::Mine,
            None,
            None,
            sort_of(Some("name"), None),
            "",
            &clock(),
        );
        assert_eq!(it[0].kind, FileKind::Folder);
        assert_eq!(it[1].name, "Anemómetro.png");
        assert!(it
            .iter()
            .all(|i| !i.href.contains("/var/") && !i.id.contains('/')));
        assert!(it[2].favourite && it[2].size.as_deref() == Some("2,0 KB"));
    }

    #[test]
    fn a_pesquisa_e_so_desta_pasta_e_pelo_nome() {
        let it = items(
            &listing(),
            FilesSection::Mine,
            None,
            None,
            sort_of(None, None),
            "vento",
            &clock(),
        );
        assert_eq!(it.len(), 1);
        assert_eq!(it[0].name, "vento.csv");
    }

    #[test]
    fn os_identificadores_sao_opacos_e_validados() {
        let f = file_ref(
            "0f0e0d0c-0b0a-4908-8706-050403020100",
            "1f0e0d0c-0b0a-4908-8706-050403020100",
        );
        assert!(matches!(parse_ref(&f), Some(Ref::File(..))));
        assert_eq!(parse_ref("../../etc/passwd"), None);
        assert_eq!(parse_ref("f.x.y"), None);
    }

    #[test]
    fn so_se_pre_visualiza_o_que_e_seguro() {
        assert!(matches!(
            preview_for("image/png", "a.png", "v"),
            Some(FilePreviewVm::Image { .. })
        ));
        assert!(matches!(
            preview_for("image/svg+xml", "a.svg", "v"),
            Some(FilePreviewVm::Unsupported)
        ));
        assert!(preview_for("text/html", "a.html", "v").is_none());
        assert!(matches!(
            preview_for("application/x-msdownload", "a.exe", "v"),
            Some(FilePreviewVm::Unsupported)
        ));
    }
}
