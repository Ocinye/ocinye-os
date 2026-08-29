//! Search rows and status.

use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// One search result.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SearchHit {
    /// Kind of artefact, for example `idea` or `dataset`.
    pub entity_type: String,
    /// Identifier of the artefact.
    pub entity_id: Uuid,
    /// Title.
    pub title: String,
    /// Bounded excerpt.
    pub excerpt: Option<String>,
    /// Classification, so the interface can show it alongside the result.
    pub classification: String,
    /// Owning workspace, when the artefact has one.
    pub workspace_id: Option<Uuid>,
    /// Lexical relevance.
    pub rank: f32,
}

/// Whether semantic search can be offered.
///
/// With no Ocinye AI node there are no embeddings. This reports that truthfully
/// rather than degrading silently to lexical results labelled as semantic
/// (`CLAUDE.md` §69).
#[derive(Debug, Clone, Serialize)]
pub struct SemanticAvailability {
    /// Whether semantic search is available.
    pub available: bool,
    /// Number of indexed documents carrying an embedding.
    pub embedded_documents: i64,
    /// Explanation for the interface.
    pub message: String,
}

/// Um resultado que veio do **corpo** de um ficheiro, e não do seu título.
///
/// # Porque é um tipo próprio
///
/// Porque transporta uma coisa que os outros resultados não têm: **onde** no
/// documento a frase está. Um `SearchHit` com a página escondida dentro do
/// excerto seria uma citação que ninguém pode verificar.
///
/// E porque a identidade é a versão. Um resultado do corpo aponta para os bytes
/// que foram lidos — se alguém carregar outra versão amanhã, esta continua a
/// dizer o que dizia.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct BodyHit {
    /// O ficheiro.
    pub file_id: Uuid,
    /// A versão exacta de onde a frase saiu.
    pub file_version_id: Uuid,
    /// O número da versão, para se poder dizer «v2» a uma pessoa.
    pub sequence: i32,
    /// O nome do ficheiro.
    pub name: String,
    /// O trecho, com os termos realçados pelo PostgreSQL.
    pub excerpt: String,
    /// Onde está: `{"page": 4}` para PDF, `{}` quando o formato não tem
    /// coordenadas.
    pub locator: serde_json::Value,
    /// A classificação **efectiva**, composta no momento da consulta.
    pub classification: String,
    /// O ambiente que o governa.
    pub workspace_id: Uuid,
    /// Relevância lexical.
    pub rank: f32,
}
