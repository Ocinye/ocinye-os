//! Shared application state.

use std::sync::Arc;

use ocinye_core::authn::TokenVerifier;
use ocinye_core::capabilities::Capabilities;
use ocinye_core::config::CoreConfig;
use ocinye_core::modules::identity::Authenticator;
use ocinye_core::modules::intelligence::InferenceProvider;
use ocinye_core::modules::mail::provider::CredentialProbe;
use ocinye_core::modules::mail::ProviderRegistry;
use ocinye_core::realtime::Realtime;
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
    /// O provider de embeddings, quando esta instalação tem um.
    ///
    /// `None` é um estado normal: a pesquisa lexical não depende disto, e a
    /// interface declara a semântica indisponível em vez de fingir que a tem.
    pub embeddings:
        Option<Arc<dyn ocinye_core::modules::intelligence::embeddings::EmbeddingProvider>>,
    /// O correio, e qual credencial abre a sessão de cada acção.
    ///
    /// A da instituição para indexar e para o trabalho agentic, a de cada
    /// membro para o que ele faz. A escolha vive num sítio só porque escolher
    /// mal não dá erro: dá uma acção correcta atribuída à pessoa errada
    /// (ADR-0409).
    ///
    /// Nunca ausente. Sem correio configurado, o adaptador da instituição é o
    /// [`ocinye_core::modules::mail::provider::UnconfiguredProvider`], que
    /// responde a cada chamada com uma razão em vez de uma página vazia — um
    /// `Option` empurraria essa decisão para dentro de cada handler, e um deles
    /// acabaria por a tomar mal.
    pub mail_registry: Arc<ProviderRegistry>,
    /// Quem diz se uma credencial de caixa abre sessão, antes de ser guardada.
    ///
    /// Em produção é o próprio registo, que tenta um `LOGIN`. É um campo — e
    /// não uma chamada dentro do serviço — porque um harness sem servidor de
    /// correio tem de poder declarar o que assume, em vez de a verificação
    /// desaparecer do caminho que os testes percorrem (ADR-0409 §8).
    pub mail_probe: Arc<dyn CredentialProbe>,
    /// O Capability Runtime, com os componentes que esta instalação construiu.
    ///
    /// Nunca `None`. Uma instalação sem os componentes construídos tem aqui um
    /// conjunto vazio, e a operação que precisar de um recusa com uma razão que
    /// se lê — a mesma escolha que o correio e a inferência fazem, e pela mesma
    /// razão: um `Option` empurraria a decisão para dentro de cada handler, e um
    /// deles acabaria por a tomar mal.
    pub capabilities: Arc<Capabilities>,
    /// O plano realtime: propagação, presença e `typing`.
    ///
    /// Nunca ausente. Sem Redis configurado é um plano que aceita tudo e não
    /// propaga nada — o comportamento correcto de uma instalação sem tempo
    /// real, e não um sítio por preencher (ADR-0012 §9).
    pub realtime: Arc<Realtime>,
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
