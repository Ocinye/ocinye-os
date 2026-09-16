//! O cliente da fronteira de conversão.
//!
//! Implementa [`thumbnail::ConversionBoundary`] falando com o **Conversion
//! Runner** por HTTP na rede interna. O worker não tem o socket do Docker nem
//! sabe correr contentores: pede um perfil de uma lista fechada e recebe o
//! derivado, que trata como ainda não confiável (ADR-0609).

use ocinye_core::error::{CoreError, CoreResult};
use ocinye_core::modules::files::thumbnail::ConversionBoundary;

/// Fala com o Conversion Runner.
pub struct RunnerBoundary {
    client: reqwest::Client,
    base_url: String,
}

impl RunnerBoundary {
    /// Constrói a partir do ambiente. `OCINYE_CONVERSION_RUNNER_URL` nomeia o
    /// serviço na rede interna; por omissão, o nome do serviço no compose.
    #[must_use]
    pub fn from_env() -> Self {
        let base_url = std::env::var("OCINYE_CONVERSION_RUNNER_URL")
            .unwrap_or_else(|_| "http://conversion-runner:8730".to_owned());
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }
}

#[async_trait::async_trait]
impl ConversionBoundary for RunnerBoundary {
    async fn rasterize_pdf(&self, bytes: &[u8]) -> CoreResult<Vec<u8>> {
        let url = format!("{}/convert/pdf-thumbnail", self.base_url);
        let resposta = self
            .client
            .post(&url)
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|erro| {
                // A fronteira não respondeu: é retentável, e não uma afirmação
                // de que o PDF não rasteriza. O outbox volta a tentar.
                CoreError::Internal(format!("o conversion runner não respondeu: {erro}"))
            })?;

        let estado = resposta.status();
        if estado.is_success() {
            let bytes = resposta.bytes().await.map_err(|erro| {
                CoreError::Internal(format!("não foi possível ler o derivado: {erro}"))
            })?;
            return Ok(bytes.to_vec());
        }

        // 422 é o conteúdo que não converte — um estado da miniatura, com o
        // ícone como desfecho. Tudo o resto é uma avaria da fronteira, retentável.
        if estado == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
            let razao = resposta.text().await.unwrap_or_default();
            return Err(CoreError::Validation(if razao.is_empty() {
                "o PDF não rasterizou".to_owned()
            } else {
                razao
            }));
        }

        Err(CoreError::Internal(format!(
            "o conversion runner respondeu {estado}"
        )))
    }
}
