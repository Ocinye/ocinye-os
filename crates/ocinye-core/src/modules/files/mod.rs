//! Ficheiros institucionais.
//!
//! # As quatro perguntas
//!
//! ```text
//! StorageObject   onde estão os bytes, e quais são
//! File            que ficheiro institucional é este, ao longo do tempo
//! FileVersion     quais eram exactamente os bytes desta versão
//! Document        que este ficheiro tem leitura documental no Conhecimento
//! ```
//!
//! Este módulo responde às duas do meio. Os bytes são de `storage`, e a leitura
//! documental é de `knowledge`.
//!
//! # O que este módulo **não** faz, ainda
//!
//! Não governa acesso. Um ficheiro resolve-se hoje através do recurso que o
//! interpreta — o documento —, e nenhuma permissão nova nasce por existirem
//! `files`. Tornar `File` um recurso governado por direito próprio é uma
//! decisão à parte, e tem de ser provada contra o modelo de autorização antes
//! de qualquer campo se mover.
//!
//! > **Versionamento acrescentado sem alterar a semântica de autorização.**

pub mod extraction;
pub mod repository;
pub mod service;

pub use repository::{FileListing, FileRecord, FolderRecord, VersionListing};
pub use service::{
    add_version, browse, content, create, create_folder, create_with_first_version,
    current_version, download_url, file_context, get, get_version, may_write, move_to_folder,
    preview, upload_version, version_download_url, versions, FileContext, FileVersionRecord,
    FolderContents, InlinePreview, NewFile, PREVIEWABLE_TYPES,
};
