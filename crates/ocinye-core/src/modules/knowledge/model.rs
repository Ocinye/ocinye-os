//! Knowledge rows.

use chrono::{DateTime, NaiveDate, Utc};
use ocinye_contracts::Classification;
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// Kind of bibliographic source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    /// Journal article.
    Article,
    /// Book.
    Book,
    /// Chapter of a book.
    BookChapter,
    /// Conference paper.
    ConferencePaper,
    /// Thesis or dissertation.
    Thesis,
    /// Technical or institutional report.
    Report,
    /// Standard or specification.
    Standard,
    /// Patent.
    Patent,
    /// Reference to an external dataset.
    DatasetReference,
    /// Software.
    Software,
    /// Web page.
    Webpage,
    /// Preprint.
    Preprint,
    /// Anything else.
    Other,
}

impl SourceType {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Article => "article",
            Self::Book => "book",
            Self::BookChapter => "book_chapter",
            Self::ConferencePaper => "conference_paper",
            Self::Thesis => "thesis",
            Self::Report => "report",
            Self::Standard => "standard",
            Self::Patent => "patent",
            Self::DatasetReference => "dataset_reference",
            Self::Software => "software",
            Self::Webpage => "webpage",
            Self::Preprint => "preprint",
            Self::Other => "other",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "article" => Self::Article,
            "book" => Self::Book,
            "book_chapter" => Self::BookChapter,
            "conference_paper" => Self::ConferencePaper,
            "thesis" => Self::Thesis,
            "report" => Self::Report,
            "standard" => Self::Standard,
            "patent" => Self::Patent,
            "dataset_reference" => Self::DatasetReference,
            "software" => Self::Software,
            "webpage" => Self::Webpage,
            "preprint" => Self::Preprint,
            "other" => Self::Other,
            _ => return None,
        })
    }
}

/// The recorded legal basis for holding a source's full content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentRight {
    /// No basis recorded. Metadata, citation, notes and links only.
    #[default]
    MetadataOnly,
    /// Published under an open licence, which must be named.
    OpenLicence,
    /// Covered by a licence the institution holds.
    InstitutionalLicence,
    /// Authored by Ocinye.
    AuthoredByOcinye,
    /// In the public domain.
    PublicDomain,
    /// Explicit permission was granted.
    PermissionGranted,
}

impl ContentRight {
    /// Whether this basis permits storing full content.
    #[must_use]
    pub const fn allows_full_content(self) -> bool {
        !matches!(self, Self::MetadataOnly)
    }

    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MetadataOnly => "metadata_only",
            Self::OpenLicence => "open_licence",
            Self::InstitutionalLicence => "institutional_licence",
            Self::AuthoredByOcinye => "authored_by_ocinye",
            Self::PublicDomain => "public_domain",
            Self::PermissionGranted => "permission_granted",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "metadata_only" => Self::MetadataOnly,
            "open_licence" => Self::OpenLicence,
            "institutional_licence" => Self::InstitutionalLicence,
            "authored_by_ocinye" => Self::AuthoredByOcinye,
            "public_domain" => Self::PublicDomain,
            "permission_granted" => Self::PermissionGranted,
            _ => return None,
        })
    }

    /// Every basis. Used by exhaustive tests.
    #[must_use]
    pub const fn all() -> [Self; 6] {
        [
            Self::MetadataOnly,
            Self::OpenLicence,
            Self::InstitutionalLicence,
            Self::AuthoredByOcinye,
            Self::PublicDomain,
            Self::PermissionGranted,
        ]
    }
}

/// Kind of document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    /// Attached to a note.
    NoteAttachment,
    /// Experimental or operational protocol.
    Protocol,
    /// Report.
    Report,
    /// Presentation.
    Presentation,
    /// Figure or image.
    Figure,
    /// Contract or agreement.
    Contract,
    /// Full text of a bibliographic source.
    SourceFullText,
    /// Anything else.
    Other,
}

impl DocumentKind {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoteAttachment => "note_attachment",
            Self::Protocol => "protocol",
            Self::Report => "report",
            Self::Presentation => "presentation",
            Self::Figure => "figure",
            Self::Contract => "contract",
            Self::SourceFullText => "source_full_text",
            Self::Other => "other",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "note_attachment" => Self::NoteAttachment,
            "protocol" => Self::Protocol,
            "report" => Self::Report,
            "presentation" => Self::Presentation,
            "figure" => Self::Figure,
            "contract" => Self::Contract,
            "source_full_text" => Self::SourceFullText,
            "other" => Self::Other,
            _ => return None,
        })
    }
}

/// A bibliographic reference.
#[derive(Debug, Clone, FromRow)]
pub struct Source {
    /// Identifier.
    pub id: Uuid,
    /// Owning unit.
    pub unit_id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Kind of source.
    pub source_type: String,
    /// Title.
    pub title: String,
    /// Authors.
    pub authors: Vec<String>,
    /// Year of publication.
    pub year: Option<i32>,
    /// Journal, proceedings or book title.
    pub container_title: Option<String>,
    /// Publisher.
    pub publisher: Option<String>,
    /// DOI.
    pub doi: Option<String>,
    /// ISBN.
    pub isbn: Option<String>,
    /// Authorised link.
    pub url: Option<String>,
    /// Abstract.
    pub abstract_text: Option<String>,
    /// Keywords.
    pub keywords: Vec<String>,
    /// Licence, when known.
    pub licence: Option<String>,
    /// Recorded legal basis for holding full content.
    pub content_right: String,
    /// Where the reference came from.
    pub origin: Option<String>,
    /// Citation key, for example a BibTeX key.
    pub citation_key: Option<String>,
    /// Classification.
    pub classification: String,
    /// Document holding the full text, when one is permitted and attached.
    pub full_text_document_id: Option<Uuid>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl Source {
    /// Parsed legal basis.
    #[must_use]
    pub fn content_right(&self) -> ContentRight {
        ContentRight::parse(&self.content_right).unwrap_or_default()
    }

    /// Parsed classification, defaulting to the most restrictive.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Restricted)
    }
}

/// A conceptual note.
#[derive(Debug, Clone, FromRow)]
pub struct Note {
    /// Identifier.
    pub id: Uuid,
    /// Owning unit.
    pub unit_id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// Title.
    pub title: String,
    /// Body.
    pub body: String,
    /// Tags.
    pub tags: Vec<String>,
    /// Classification.
    pub classification: String,
    /// Current revision number.
    pub revision: i32,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Last change.
    pub updated_at: DateTime<Utc>,
}

impl Note {
    /// Parsed classification, defaulting to the most restrictive.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Restricted)
    }
}

/// A document backed by a stored object.
#[derive(Debug, Clone, FromRow)]
pub struct Document {
    /// Identifier.
    pub id: Uuid,
    /// Owning unit.
    pub unit_id: Uuid,
    /// Owning workspace.
    pub workspace_id: Uuid,
    /// O objecto que guarda os bytes **da versão corrente**.
    ///
    /// Chamava-se `storage_object_id`, como a coluna homónima de `documents` —
    /// que já não existe. O nome sobreviveu à coluna e passou a documentar
    /// mal: quem o lesse à procura dela não a encontraria, e quem o lesse sem
    /// procurar assumiria que um documento tem um objecto, quando tem uma
    /// história de versões e esta é a última.
    ///
    /// Tipos e nomes são documentação que compila. Um nome errado é uma
    /// afirmação errada mantida pelo compilador.
    pub current_storage_object_id: Uuid,
    /// Kind of document.
    pub kind: String,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Date the document carries.
    pub document_date: Option<NaiveDate>,
    /// Classification.
    pub classification: String,
    /// Original filename, normalised.
    pub original_filename: String,
    /// MIME type.
    pub content_type: String,
    /// Size in bytes.
    pub size_bytes: i64,
    /// SHA-256 checksum.
    pub checksum_sha256: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl Document {
    /// Parsed classification, defaulting to the most restrictive.
    #[must_use]
    pub fn classification(&self) -> Classification {
        Classification::parse(&self.classification).unwrap_or(Classification::Restricted)
    }
}

/// A typed relation between two research objects.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ResearchLink {
    /// Identifier.
    pub id: Uuid,
    /// Owning workspace.
    /// The environment the relation was declared in, when there was one.
    ///
    /// `None` diz que a aresta atravessa ambientes — nunca que é de toda a
    /// gente. A autoridade vem das duas pontas.
    pub workspace_id: Option<Uuid>,
    /// Kind of the source object.
    pub source_type_name: String,
    /// Identifier of the source object.
    pub source_id: Uuid,
    /// Relation.
    pub relation: String,
    /// Kind of the target object.
    pub target_type_name: String,
    /// Identifier of the target object.
    pub target_id: Uuid,
    /// Explanatory note.
    pub note: Option<String>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

// O vocabulário das relações mudou de casa, e não encolheu.
//
// Estavam aqui sete cadeias — `cites`, `supports`, `refutes`, `derived_from`,
// `uses`, `produces`, `relates_to` — e o ciclo científico trouxe mais oito. As
// quinze vivem agora em `ocinye_contracts::provenance::ProvenanceRelation`,
// que além do verbo declara **entre que tipos** ele faz sentido: uma lista
// fechada de verbos impede verbos inventados e não impede combinações
// absurdas.
//
// As sete originais estão lá todas, e há um teste que o exige. Retirar uma do
// vocabulário não apagaria as arestas que a usam: tornava-as ilegíveis, e a
// memória institucional passaria a conter afirmações que o sistema não sabe
// ler.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_metadata_only_forbids_full_content() {
        for right in ContentRight::all() {
            assert_eq!(
                right.allows_full_content(),
                right != ContentRight::MetadataOnly,
                "{right:?}"
            );
        }
    }

    #[test]
    fn the_default_basis_forbids_storing_full_content() {
        assert!(!ContentRight::default().allows_full_content());
    }

    #[test]
    fn content_rights_round_trip() {
        for right in ContentRight::all() {
            assert_eq!(ContentRight::parse(right.as_str()), Some(right));
        }
    }
}
