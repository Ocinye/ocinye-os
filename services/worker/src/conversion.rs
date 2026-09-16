//! O cliente da fronteira de conversão.
//!
//! Implementa [`thumbnail::ConversionBoundary`] falando com o **Conversion
//! Runner** por HTTP na rede interna. O worker não tem o socket do Docker nem
//! sabe correr contentores: pede um perfil de uma lista fechada e recebe o
//! derivado, que trata como ainda não confiável (ADR-0609).
//!
//! É aqui que o tipo de conteúdo se traduz num **perfil** de conversão — o
//! `ocinye-core` não conhece nomes de perfis, e o runner não conhece tipos MIME.

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

/// O perfil de conversão e a extensão de origem para um tipo de conteúdo.
///
/// O PDF vai directo ao renderizador; os formatos de escritório passam pelo
/// LibreOffice (que precisa da extensão para detectar o formato); os vídeos, pelo
/// `ffmpeg`. Um tipo que não esteja aqui não tem conversão — e o `ocinye-core` só
/// chama esta fronteira para tipos que não são raster, pelo que um `None` é um
/// tipo enfileirado sem mapa, tratado como conteúdo que não converte.
fn perfil_e_extensao(content_type: &str) -> Option<(&'static str, &'static str)> {
    Some(match content_type {
        "application/pdf" => ("pdf-thumbnail", "pdf"),
        "application/msword" => ("office-thumbnail", "doc"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            ("office-thumbnail", "docx")
        }
        "application/vnd.ms-excel" => ("office-thumbnail", "xls"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            ("office-thumbnail", "xlsx")
        }
        "application/vnd.ms-powerpoint" => ("office-thumbnail", "ppt"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            ("office-thumbnail", "pptx")
        }
        "application/vnd.oasis.opendocument.text" => ("office-thumbnail", "odt"),
        "application/vnd.oasis.opendocument.spreadsheet" => ("office-thumbnail", "ods"),
        "application/vnd.oasis.opendocument.presentation" => ("office-thumbnail", "odp"),
        "application/rtf" => ("office-thumbnail", "rtf"),
        "video/mp4" => ("video-thumbnail", "mp4"),
        "video/webm" => ("video-thumbnail", "webm"),
        "video/quicktime" => ("video-thumbnail", "mov"),
        "video/x-matroska" => ("video-thumbnail", "mkv"),
        "video/x-msvideo" => ("video-thumbnail", "avi"),
        _ => return None,
    })
}

#[async_trait::async_trait]
impl ConversionBoundary for RunnerBoundary {
    async fn to_thumbnail_image(&self, content_type: &str, bytes: &[u8]) -> CoreResult<Vec<u8>> {
        let Some((perfil, ext)) = perfil_e_extensao(content_type) else {
            return Err(CoreError::Validation(format!(
                "não há conversão para {content_type}"
            )));
        };

        let url = format!("{}/convert/{perfil}?ext={ext}", self.base_url);
        let resposta = self
            .client
            .post(&url)
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|erro| {
                // A fronteira não respondeu: é retentável, e não uma afirmação
                // de que o conteúdo não converte. O outbox volta a tentar.
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
                "o conteúdo não converteu".to_owned()
            } else {
                razao
            }));
        }

        Err(CoreError::Internal(format!(
            "o conversion runner respondeu {estado}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Todo o tipo não-raster que o domínio dá por gerável tem de ter um perfil
    /// aqui — senão o worker enfileira uma miniatura que nunca converte. Este
    /// teste liga as duas metades: os tipos do `ocinye-core` e o mapa do worker.
    #[test]
    fn cada_tipo_de_documento_e_video_tem_perfil() {
        use ocinye_core::modules::files::thumbnail::{DOCUMENT_TYPES, VIDEO_TYPES};
        for tipo in DOCUMENT_TYPES.iter().chain(VIDEO_TYPES.iter()) {
            assert!(
                perfil_e_extensao(tipo).is_some(),
                "o tipo {tipo} é gerável no core mas não tem perfil no worker"
            );
        }
    }

    #[test]
    fn um_tipo_desconhecido_nao_tem_perfil() {
        assert!(perfil_e_extensao("application/x-lixo").is_none());
    }
}
