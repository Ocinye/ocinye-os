//! D005 · Conhecimento: Documentos e Bibliografia (a aplicação Bibliografia
//! abre a mesma vista, na secção de fontes).
//!
//! Um documento é uma afirmação sobre um ficheiro: mostra-se a metadata, a
//! soma e a transferência — nunca o conteúdo. Uma entrada bibliográfica é
//! texto externo: título, autores e resumo são **dados**, sem autoridade.

use serde_json::Value;

use super::nav;
use crate::controllers::desktop::{bytes, text};
use crate::i18n::t;
use crate::ui::view_models::{AppNavVm, ContentRight, KnowledgeSection, ResItemVm, ResOptionVm};

/// A secção pedida pelo caminho.
#[must_use]
pub fn section_of(v: &str) -> Option<KnowledgeSection> {
    match v {
        "documents" => Some(KnowledgeSection::Documents),
        "sources" => Some(KnowledgeSection::Sources),
        _ => None,
    }
}

/// O segmento do caminho de uma secção.
#[must_use]
pub const fn segment(s: KnowledgeSection) -> &'static str {
    match s {
        KnowledgeSection::Documents => "documents",
        KnowledgeSection::Sources => "sources",
    }
}

/// A navegação.
#[must_use]
pub fn navigation(s: KnowledgeSection) -> Vec<AppNavVm> {
    [
        (KnowledgeSection::Documents, "know.documents", "files"),
        (KnowledgeSection::Sources, "know.sources", "bibliography"),
    ]
    .into_iter()
    .map(|(x, k, i)| nav(k, i, format!("/knowledge/{}", segment(x)), x == s))
    .collect()
}

/// O pedido ao Core: a lista da secção, ou a pesquisa de âmbito no índice
/// autorizado (só fontes ou só documentos, nunca o resto do índice).
#[must_use]
pub fn list_path(s: KnowledgeSection, q: Option<&str>, page: u32) -> String {
    match q.map(str::trim).filter(|q| q.chars().count() >= 2) {
        Some(q) => format!(
            "/api/v1/search?q={}&entity_types={}&page={page}&page_size=25",
            super::encode(q),
            match s {
                KnowledgeSection::Documents => "document",
                KnowledgeSection::Sources => "source",
            }
        ),
        None => format!("/api/v1/{}?page={page}&page_size=25", segment(s)),
    }
}

/// O tipo de uma fonte (vocabulário fechado do Core), traduzido.
#[must_use]
pub fn source_type_key(v: &str) -> &'static str {
    match v {
        "article" => "prod.know.type.article",
        "book" => "prod.know.type.book",
        "book_chapter" => "prod.know.type.book_chapter",
        "conference_paper" => "prod.know.type.conference_paper",
        "thesis" => "prod.know.type.thesis",
        "report" => "prod.know.type.report",
        "standard" => "prod.know.type.standard",
        "patent" => "prod.know.type.patent",
        "dataset_reference" => "prod.know.type.dataset_reference",
        "software" => "prod.know.type.software",
        "webpage" => "prod.know.type.webpage",
        "preprint" => "prod.know.type.preprint",
        _ => "prod.know.type.other",
    }
}

/// Os tipos para o formulário.
#[must_use]
pub fn source_types(selected: &str) -> Vec<ResOptionVm> {
    [
        "article",
        "book",
        "book_chapter",
        "conference_paper",
        "thesis",
        "report",
        "standard",
        "patent",
        "dataset_reference",
        "software",
        "webpage",
        "preprint",
        "other",
    ]
    .into_iter()
    .map(|v| ResOptionVm {
        value: v.to_owned(),
        label: t(source_type_key(v)).to_owned(),
        selected: v == selected,
    })
    .collect()
}

/// O tipo de um documento, traduzido.
#[must_use]
pub fn document_kind_key(v: &str) -> &'static str {
    match v {
        "note_attachment" => "prod.know.kind.note_attachment",
        "protocol" => "prod.know.kind.protocol",
        "report" => "prod.know.kind.report",
        "presentation" => "prod.know.kind.presentation",
        "figure" => "prod.know.kind.figure",
        "contract" => "prod.know.kind.contract",
        "source_full_text" => "prod.know.kind.source_full_text",
        _ => "prod.know.kind.other",
    }
}

/// A base legal para guardar conteúdo integral, uma a uma. Um valor novo não
/// se lê como nenhuma das conhecidas.
#[must_use]
pub fn content_right(v: &str) -> Option<ContentRight> {
    Some(match v {
        "metadata_only" => ContentRight::MetadataOnly,
        "open_licence" => ContentRight::OpenLicence,
        "institutional_licence" => ContentRight::InstitutionalLicence,
        "authored_by_ocinye" => ContentRight::AuthoredByOcinye,
        "public_domain" => ContentRight::PublicDomain,
        "permission_granted" => ContentRight::PermissionGranted,
        _ => return None,
    })
}

const fn right_key(r: ContentRight) -> &'static str {
    match r {
        ContentRight::MetadataOnly => "know.right.metadata_only",
        ContentRight::OpenLicence => "know.right.open_licence",
        ContentRight::InstitutionalLicence => "know.right.institutional_licence",
        ContentRight::AuthoredByOcinye => "know.right.authored_by_ocinye",
        ContentRight::PublicDomain => "know.right.public_domain",
        ContentRight::PermissionGranted => "know.right.permission_granted",
    }
}

/// As bases legais que o formulário oferece. A que não exige mais nada
/// (`metadata_only`) é a escolhida por omissão; as outras são uma decisão de
/// quem regista, e o Core valida-as.
#[must_use]
pub fn rights(selected: &str) -> Vec<ResOptionVm> {
    [
        "metadata_only",
        "open_licence",
        "institutional_licence",
        "authored_by_ocinye",
        "public_domain",
        "permission_granted",
    ]
    .into_iter()
    .filter_map(|v| {
        Some(ResOptionVm {
            value: v.to_owned(),
            label: t(right_key(content_right(v)?)).to_owned(),
            selected: v == selected,
        })
    })
    .collect()
}

/// As colunas de cada secção.
#[must_use]
pub fn columns(s: KnowledgeSection) -> Vec<&'static str> {
    match s {
        KnowledgeSection::Sources => vec!["know.col.authors", "know.col.year", "know.col.right"],
        KnowledgeSection::Documents => vec!["know.col.type"],
    }
}

fn authors(v: &Value) -> Option<String> {
    let a: Vec<&str> = v
        .get("authors")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    match a.as_slice() {
        [] => None,
        [um] => Some((*um).to_owned()),
        [um, ..] => Some(format!("{um} et al.")),
    }
}

/// As linhas de uma lista da secção.
#[must_use]
pub fn items(s: KnowledgeSection, list: &[Value], open: Option<&str>) -> Vec<ResItemVm> {
    list.iter()
        .filter_map(|r| {
            let id = text(r, "id");
            if id.is_empty() {
                return None;
            }
            let cells = match s {
                KnowledgeSection::Sources => vec![
                    authors(r),
                    r.get("year").and_then(Value::as_i64).map(|y| y.to_string()),
                    content_right(text(r, "content_right")).map(|c| t(right_key(c)).to_owned()),
                ],
                KnowledgeSection::Documents => {
                    vec![Some(t(document_kind_key(text(r, "kind"))).to_owned())]
                }
            };
            Some(ResItemVm {
                title: text(r, "title").to_owned(),
                code: None,
                state: None,
                cells,
                overdue: false,
                priority: None,
                href: format!("/knowledge/{}/{id}", segment(s)),
                active: open == Some(id),
            })
        })
        .collect()
}

/// As linhas de uma pesquisa de âmbito: só os tipos da secção (o índice já
/// filtrou pela visibilidade de quem pergunta).
#[must_use]
pub fn hits(s: KnowledgeSection, list: &[Value], open: Option<&str>) -> Vec<ResItemVm> {
    let tipo = match s {
        KnowledgeSection::Documents => "document",
        KnowledgeSection::Sources => "source",
    };
    list.iter()
        .filter(|h| text(h, "entity_type") == tipo)
        .filter_map(|h| {
            let id = text(h, "entity_id");
            (!id.is_empty()).then(|| ResItemVm {
                title: text(h, "title").to_owned(),
                code: None,
                state: None,
                cells: vec![None; columns(s).len()],
                overdue: false,
                priority: None,
                href: format!("/knowledge/{}/{id}", segment(s)),
                active: open == Some(id),
            })
        })
        .collect()
}

/// A soma SHA-256, abreviada para ler (a completa vai no `title`).
#[must_use]
pub fn checksum(v: &str) -> Option<String> {
    let v = v.trim();
    (v.len() >= 16).then(|| format!("{}…{}", &v[..12], &v[v.len() - 4..]))
}

/// O tipo de conteúdo, legível («PDF»).
#[must_use]
pub fn content_type(v: &str) -> String {
    let sub = v.split(['/', ';']).nth(1).unwrap_or(v).trim();
    match sub {
        "pdf" => "PDF".to_owned(),
        "plain" => "TXT".to_owned(),
        "csv" => "CSV".to_owned(),
        s if s.len() <= 5 => s.to_uppercase(),
        _ => v.to_owned(),
    }
}

/// O tamanho de um documento.
#[must_use]
pub fn size(v: &Value) -> Option<String> {
    v.get("size_bytes").and_then(Value::as_u64).map(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_pesquisa_e_de_ambito_e_so_do_tipo_da_seccao() {
        let p = list_path(KnowledgeSection::Sources, Some("vento forte"), 1);
        assert!(p.starts_with("/api/v1/search?q=vento%20forte&entity_types=source"));
        assert!(list_path(KnowledgeSection::Sources, Some("v"), 1).starts_with("/api/v1/sources"));
        let h = json!([
            { "entity_type": "source", "entity_id": "s1", "title": "Artigo" },
            { "entity_type": "project", "entity_id": "p1", "title": "Projecto" },
            { "entity_type": "note", "entity_id": "n1", "title": "Nota" }
        ]);
        let v = hits(KnowledgeSection::Sources, h.as_array().unwrap(), None);
        assert_eq!(v.len(), 1, "só fontes, nunca o resto do índice");
        assert_eq!(v[0].href, "/knowledge/sources/s1");
    }

    #[test]
    fn a_base_legal_vem_do_core_e_nao_do_tipo() {
        assert_eq!(
            content_right("open_licence"),
            Some(ContentRight::OpenLicence)
        );
        assert_eq!(content_right("fair_use"), None);
        assert_eq!(
            rights("metadata_only")
                .iter()
                .filter(|o| o.selected)
                .count(),
            1
        );
    }

    #[test]
    fn a_soma_abrevia_sem_perder_o_principio_e_o_fim() {
        let s = "a".repeat(60) + "beef";
        assert_eq!(checksum(&s).unwrap(), format!("{}…beef", "a".repeat(12)));
        assert_eq!(checksum("curta"), None);
    }
}
