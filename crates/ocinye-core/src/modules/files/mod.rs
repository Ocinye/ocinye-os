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

pub mod embedding;
pub mod extraction;
pub mod repository;
pub mod service;
pub mod thumbnail;
pub mod upload;

pub use repository::{FileListing, FileRecord, FolderRecord, VersionListing};
pub use service::{
    add_version, all, browse, content, content_of_version, create, create_folder, create_personal,
    create_personal_folder, create_with_first_version, current_version, delete_personal_folder,
    download_url, download_url_personal, excerpts, file_context, get, get_version,
    guardar_bytes_personal, is_textual_type, list_personal, list_personal_favourites,
    list_personal_folders, list_personal_recent, list_personal_trash, may_write,
    move_personal_file, move_to_folder, owns_personal_file_version, owns_personal_folder, preview,
    preview_version, purge_all_personal_trash, purge_personal_file, read_download,
    read_version_download, read_version_download_personal, read_version_inline_personal,
    read_version_preview, read_version_text_personal, rename_personal_file, rename_personal_folder,
    restore_personal_file, toggle_personal_favourite, trash_personal_file, upload_version,
    version_download_url, versions, BytesGuardados, FileContext, FileDownload, FileVersionRecord,
    FolderContents, InlinePreview, InlineText, NewFile, PersonalFileListing, PersonalFiles,
    PersonalFolder, INLINE_VIEWER_TYPES, PREVIEWABLE_TYPES,
};
