//! Shared application state.

use std::sync::Arc;

use ocinye_core::authn::TokenVerifier;
use ocinye_core::capabilities::Capabilities;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::Authenticator;
use ocinye_core::modules::intelligence::InferenceProvider;
use ocinye_core::modules::mail::MailProvider;
use ocinye_core::storage::ObjectStore;
use sqlx::PgPool;
use uuid::Uuid;

/// State shared by every handler.
#[derive(Clone)]
pub struct AppState {
    /// Database pool.
    pub pool: PgPool,
    /// Configuration.
    pub config: Arc<CoreConfig>,
    /// OIDC token verifier. Vestigial under ADR-0103; retained for future
    /// federation.
    pub verifier: Arc<TokenVerifier>,
    /// Password hashing, throttling and the sign-in decision.
    pub authenticator: Arc<Authenticator>,
    /// Object store, absent when storage is not configured.
    pub store: Option<Arc<ObjectStore>>,
    /// The inference adapter.
    ///
    /// Never `None`. With no AI node this holds
    /// [`ocinye_core::modules::intelligence::NoProvider`], which refuses every
    /// call with a stated reason — the correct behaviour of an installation
    /// without inference, not a placeholder for one.
    pub inference: Arc<dyn InferenceProvider>,
    /// The mail adapter.
    ///
    /// Never `None`. When mail is not configured this holds
    /// [`ocinye_core::modules::mail::provider::UnconfiguredProvider`], which
    /// answers every call with a stated reason rather than a blank page. An
    /// `Option` here would push that decision into every handler, and one of
    /// them would eventually get it wrong.
    pub mail_provider: Arc<dyn MailProvider>,
    /// O Capability Runtime, com os componentes que esta instalação construiu.
    ///
    /// Nunca `None`. Uma instalação sem os componentes construídos tem aqui um
    /// conjunto vazio, e a operação que precisar de um recusa com uma razão que
    /// se lê — a mesma escolha que o correio e a inferência fazem, e pela mesma
    /// razão: um `Option` empurraria a decisão para dentro de cada handler, e um
    /// deles acabaria por a tomar mal.
    pub capabilities: Arc<Capabilities>,
    /// The organisation this deployment serves.
    pub organisation_id: Uuid,
}

impl AppState {
    /// The domains that count as inside the institution.
    ///
    /// Async because this will read `mail_provider_settings` once the
    /// administration screen can change domains without a restart. Today it
    /// answers from configuration.
    pub async fn institutional_domains(&self) -> Vec<String> {
        self.config.mail.institutional_domains.clone()
    }

    /// The object store, or a clear error when storage is not configured.
    ///
    /// # Errors
    ///
    /// Returns [`ocinye_core::CoreError::StorageUnavailable`] when absent.
    pub fn store(&self) -> Result<&ObjectStore, ocinye_core::CoreError> {
        self.store.as_deref().ok_or_else(|| {
            ocinye_core::CoreError::StorageUnavailable(
                "Object storage is not configured on this deployment.".to_owned(),
            )
        })
    }
}
