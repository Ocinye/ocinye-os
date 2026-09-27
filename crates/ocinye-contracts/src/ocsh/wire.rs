//! O contrato de fio do ocsh: o que o Terminal envia e o que o Core devolve.
//!
//! # O Core não escreve frases
//!
//! A resposta leva **chaves** i18n e valores, não texto numa língua. Quem
//! traduz é o Workspace, que tem o catálogo; o Core continua indiferente à
//! língua, e o mesmo resultado serve pt, en e fr — e um futuro CLI.
//!
//! # O cliente nunca interpreta HTML
//!
//! Valores vindos de dados (nomes de ficheiros, títulos, respostas do Nye) são
//! texto. O Terminal desenha-os com nós de texto.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Um pedido de execução.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecRequest {
    /// A linha, tal como foi escrita.
    pub line: String,
    /// O contexto do separador: `None`/`"personal"` ou o id de um ambiente.
    #[serde(default)]
    pub context: Option<String>,
}

/// O contexto activo, resolvido pelo Core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextView {
    /// `None` = pessoal.
    pub workspace_id: Option<Uuid>,
    /// Código para o prompt (`WSBA7215B`), ou `None` no pessoal.
    pub code: Option<String>,
    /// Título do ambiente.
    pub title: Option<String>,
}

impl ContextView {
    /// O contexto pessoal.
    #[must_use]
    pub const fn personal() -> Self {
        Self {
            workspace_id: None,
            code: None,
            title: None,
        }
    }
}

/// Um parâmetro de uma mensagem.
pub type Param = (String, String);

/// O tom de uma nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// Sucesso.
    Ok,
    /// Informação.
    Info,
    /// Aviso.
    Warn,
    /// Erro.
    Err,
    /// Recusa.
    Deny,
}

/// Uma célula de dados. Texto de dados — nunca uma chave, nunca HTML.
pub type Cell = String;

/// Um bloco de saída.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    /// Uma nota: chave i18n + parâmetros, e sugestões de comandos.
    Note {
        /// O tom.
        tone: Tone,
        /// A chave i18n da mensagem.
        key: String,
        /// Os parâmetros (`{name}` → valor).
        #[serde(default)]
        params: Vec<Param>,
        /// Comandos sugeridos (texto de comando, estável nas três línguas).
        #[serde(default)]
        suggestions: Vec<String>,
    },
    /// Uma tabela: colunas por id (rótulo i18n `ocsh.col.<id>`), linhas de dados.
    Table {
        /// Ids das colunas.
        columns: Vec<String>,
        /// Linhas.
        rows: Vec<Vec<Cell>>,
        /// O percurso do pipeline, quando houve (`tasks.list → filter(x)`).
        #[serde(default)]
        pipeline: Option<String>,
    },
    /// Pares chave/valor (rótulo i18n `ocsh.col.<id>`).
    Facts {
        /// `(id, valor)`.
        rows: Vec<(String, Cell)>,
    },
    /// JSON (`--json`).
    Json {
        /// O valor.
        value: serde_json::Value,
    },
    /// Ajuda: comandos visíveis a esta pessoa, pela ordem do registo.
    Help {
        /// `família` ou `família sub` pedido, ou vazio para tudo.
        topic: String,
        /// `(uso, chave i18n da descrição)`.
        entries: Vec<(String, String)>,
    },
    /// Uma acção só do cliente (`clear`, `history`, `exit`).
    Client {
        /// O comando.
        action: String,
    },
}

/// A resposta de uma execução.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecResponse {
    /// Código de saída (`ExitCode::code`).
    pub exit: u8,
    /// Blocos, por ordem.
    pub blocks: Vec<Block>,
    /// A capability que correu, quando correu uma.
    #[serde(default)]
    pub capability: Option<String>,
    /// O contexto depois do comando (muda com `context use`).
    pub context: ContextView,
    /// Duração no Core, em milissegundos.
    pub ms: u64,
}
