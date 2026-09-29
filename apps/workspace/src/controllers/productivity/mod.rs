//! As aplicações de produtividade (D004): Ficheiros, Notas, Calendário e
//! Correio. Traduzem o que o Core devolve para os ViewModels do Design; não
//! decidem autorização, não guardam nada e não conhecem o armazenamento.

pub mod calendar;
pub mod files;
pub mod mail;
pub mod note_markdown;
pub mod notes;

use crate::api::ApiFailure;
use crate::ui::view_models::{AppError, AppNyeVm};

/// O erro tipado de uma aplicação para uma recusa do Core. Um recurso que não
/// existe e um que o membro não pode ver são o mesmo erro (nunca se distingue).
#[must_use]
pub fn app_error(failure: &ApiFailure) -> AppError {
    match failure {
        ApiFailure::Denied => AppError::NotFound,
        ApiFailure::Forbidden | ApiFailure::Unauthorised => AppError::PermissionDenied,
        ApiFailure::Conflict(_) => AppError::Conflict,
        ApiFailure::Rejected(_) => AppError::SaveFailed,
        ApiFailure::Unavailable(_) | ApiFailure::ApplicationInactive | ApiFailure::Failed(_) => {
            AppError::Unavailable
        }
    }
}

/// A ligação contextual à Nye: a Nye canónica com uma referência tipada. A
/// referência é só contexto; a Nye relê o recurso com a autoridade do membro, e
/// um recurso que ele não pode ver não entra (ADR-0619).
#[must_use]
pub fn nye(kind: &str, id: &str, label_key: &'static str) -> AppNyeVm {
    AppNyeVm {
        href: format!("/ai/prompt?ref={kind}:{id}"),
        label_key,
    }
}
