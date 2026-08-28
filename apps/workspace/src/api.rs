//! Client for the Ocinye Core.
//!
//! Adds the member's bearer token and propagates the correlation identifier, so
//! one operation can be followed from the Workspace through the Core and into
//! the Worker.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::WorkspaceState;

/// Reasons a Core call did not return data.
#[derive(Debug)]
pub enum ApiFailure {
    /// The Core refused the token; the member should sign in again.
    Unauthorised,
    /// The resource does not exist, or must be indistinguishable from that.
    Denied,
    /// The Core refused the operation, and says so plainly.
    ///
    /// Distinct from [`ApiFailure::Denied`]: an operation the caller may not
    /// perform is not the same as a resource that may not exist, and the two
    /// deserve different words (briefing §46).
    Forbidden,
    /// A dependency the operation needs is not configured or not reachable.
    ///
    /// O Core responde `503` a isto, e é um facto diferente de uma avaria: a
    /// capacidade existe, a instalação é que não a tem de pé. A Ajuda distingue
    /// os dois para o membro — «não configurado» e «erro» pedem coisas
    /// diferentes a quem lê — e o Workspace só os pode distinguir se não os
    /// juntar aqui.
    Unavailable,
    /// Anything else.
    Failed(String),
}

impl std::fmt::Display for ApiFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorised => f.write_str("the session is no longer valid"),
            Self::Denied => f.write_str("this is not available to you"),
            Self::Forbidden => f.write_str("you do not have access to this operation"),
            Self::Unavailable => f.write_str("a dependency of this operation is unavailable"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

/// Perform an authenticated GET against the Core.
///
/// # Errors
///
/// Returns [`ApiFailure`] describing why the call did not return data.
pub async fn get<T: DeserializeOwned>(
    state: &WorkspaceState,
    token: &str,
    correlation_id: &str,
    path: &str,
) -> Result<T, ApiFailure> {
    let response = state
        .http
        .get(format!("{}{path}", state.config.core_url))
        .bearer_auth(token)
        .header(ocinye_observability::CORRELATION_ID_HEADER, correlation_id)
        .send()
        .await
        .map_err(|error| ApiFailure::Failed(format!("the Core is unreachable: {error}")))?;

    match response.status().as_u16() {
        200..=299 => response
            .json()
            .await
            .map_err(|error| ApiFailure::Failed(format!("unexpected response: {error}"))),
        401 => Err(ApiFailure::Unauthorised),
        // Recusa e inexistência chegam ambas aqui e são deliberadamente
        // indistinguíveis para leituras: revelar que um recurso existe mas está
        // fechado já é informação (ADR-0100). O Core escolhe qual devolve.
        403 => Err(ApiFailure::Forbidden),
        404 => Err(ApiFailure::Denied),
        503 => Err(ApiFailure::Unavailable),
        status => Err(ApiFailure::Failed(format!(
            "the Core returned status {status}"
        ))),
    }
}

/// Perform an authenticated POST against the Core.
///
/// # Errors
///
/// Returns [`ApiFailure`] describing why the call did not succeed.
pub async fn post(
    state: &WorkspaceState,
    token: &str,
    correlation_id: &str,
    path: &str,
    body: &Value,
) -> Result<Value, ApiFailure> {
    let response = state
        .http
        .post(format!("{}{path}", state.config.core_url))
        .bearer_auth(token)
        .header(ocinye_observability::CORRELATION_ID_HEADER, correlation_id)
        .json(body)
        .send()
        .await
        .map_err(|error| ApiFailure::Failed(format!("the Core is unreachable: {error}")))?;

    interpret(response).await
}

/// Read the Core's answer to a state-changing call.
///
/// The Core's error envelope is flat — `{"code", "message", "request_id"}`, not
/// `{"error": {…}}`. Reading the wrong shape is silent: the member would see
/// "the operation could not be completed" where the Core had written "A
/// palavra-passe deve ter pelo menos 15 caracteres." Pinned by
/// `the_core_error_envelope_is_flat` below.
async fn interpret(response: reqwest::Response) -> Result<Value, ApiFailure> {
    let status = response.status();
    let payload: Value = response.json().await.unwrap_or(Value::Null);

    if status.is_success() {
        return Ok(payload);
    }
    if status.as_u16() == 401 {
        return Err(ApiFailure::Unauthorised);
    }
    if status.as_u16() == 503 {
        return Err(ApiFailure::Unavailable);
    }

    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("the operation could not be completed")
        .to_owned();

    Err(ApiFailure::Failed(message))
}

/// Perform a POST against the Core without a session.
///
/// Exists for exactly one caller: signing in. Everything else in the Workspace
/// acts on a member's behalf and carries their session.
///
/// # Errors
///
/// Returns [`ApiFailure`] describing why the call did not return data.
pub async fn patch(
    state: &WorkspaceState,
    token: &str,
    correlation_id: &str,
    path: &str,
    body: &Value,
) -> Result<Value, ApiFailure> {
    let response = state
        .http
        .patch(format!("{}{path}", state.config.core_url))
        .bearer_auth(token)
        .header(ocinye_observability::CORRELATION_ID_HEADER, correlation_id)
        .json(body)
        .send()
        .await
        .map_err(|error| ApiFailure::Failed(format!("the Core is unreachable: {error}")))?;

    interpret(response).await
}

/// Read the Core's answer to a state-changing call.
///
/// The Core's error envelope is flat — `{"code", "message", "request_id"}`, not
/// `{"error": {…}}`. Reading the wrong shape is silent: the member would see
/// "the operation could not be completed" where the Core had written "A
/// palavra-passe deve ter pelo menos 15 caracteres." Pinned by
/// Perform a POST against the Core without a session.
///
/// Exists for exactly one caller: signing in. Everything else in the Workspace
/// acts on a member's behalf and carries their session.
///
/// # Errors
///
/// Returns [`ApiFailure`] describing why the call did not return data.
pub async fn post_unauthenticated(
    state: &WorkspaceState,
    correlation_id: &str,
    path: &str,
    body: &Value,
) -> Result<Value, ApiFailure> {
    let response = state
        .http
        .post(format!("{}{path}", state.config.core_url))
        .header(ocinye_observability::CORRELATION_ID_HEADER, correlation_id)
        .json(body)
        .send()
        .await
        .map_err(|error| ApiFailure::Failed(format!("the Core is unreachable: {error}")))?;

    let status = response.status();
    let payload: Value = response.json().await.unwrap_or(Value::Null);

    if status.is_success() {
        return Ok(payload);
    }

    // Note the absence of a 401 special case. At sign-in a 401 *is* the refusal,
    // and it must reach the caller as the Core's own message rather than as
    // "your session expired" — there was no session.
    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Nome de utilizador ou palavra-passe inválidos.")
        .to_owned();

    Err(ApiFailure::Failed(message))
}

/// Read the Core's readiness, without a member session.
///
/// Used by the sign-in page so it can say plainly when the platform is not
/// ready, rather than failing with an opaque error after sign-in.
pub async fn core_ready(state: &WorkspaceState) -> Result<Value> {
    let response = state
        .http
        .get(format!("{}/ready", state.config.core_url))
        // Curto: a página de início de sessão pergunta isto para poder dizer o
        // estado, e não deve ficar pendurada a fazê-lo.
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .context("contacting the Core")?;
    response.json().await.context("reading readiness")
}

/// Fetch raw bytes from the Core, with their content type.
///
/// The Core returns JSON for everything except the member's avatar, which is an
/// image. Routing it through [`get`] would try to parse a WebP as JSON; routing
/// it around the client would lose the bearer token, the correlation id and the
/// failure vocabulary that the rest of the Workspace speaks.
///
/// # Errors
///
/// Returns [`ApiFailure`] describing why the call did not return data.
pub async fn bytes(
    state: &WorkspaceState,
    token: &str,
    correlation_id: &str,
    path: &str,
) -> Result<(String, Vec<u8>), ApiFailure> {
    let response = state
        .http
        .get(format!("{}{path}", state.config.core_url))
        .bearer_auth(token)
        .header(ocinye_observability::CORRELATION_ID_HEADER, correlation_id)
        .send()
        .await
        .map_err(|error| ApiFailure::Failed(format!("the Core is unreachable: {error}")))?;

    match response.status().as_u16() {
        200..=299 => {
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_owned();
            let body = response
                .bytes()
                .await
                .map_err(|error| ApiFailure::Failed(format!("unexpected response: {error}")))?;
            Ok((content_type, body.to_vec()))
        }
        401 => Err(ApiFailure::Unauthorised),
        403 => Err(ApiFailure::Forbidden),
        404 => Err(ApiFailure::Denied),
        503 => Err(ApiFailure::Unavailable),
        status => Err(ApiFailure::Failed(format!(
            "the Core returned status {status}"
        ))),
    }
}

/// Upload a file to the Core as a single-part multipart form.
///
/// # Errors
///
/// Returns [`ApiFailure`] describing why the call did not succeed.
pub async fn upload(
    state: &WorkspaceState,
    token: &str,
    correlation_id: &str,
    path: &str,
    filename: String,
    content_type: String,
    data: Vec<u8>,
) -> Result<Value, ApiFailure> {
    let part = reqwest::multipart::Part::bytes(data)
        .file_name(filename)
        .mime_str(&content_type)
        .map_err(|_| ApiFailure::Failed("the upload is malformed".to_owned()))?;

    let response = state
        .http
        .post(format!("{}{path}", state.config.core_url))
        .bearer_auth(token)
        .header(ocinye_observability::CORRELATION_ID_HEADER, correlation_id)
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .map_err(|error| ApiFailure::Failed(format!("the Core is unreachable: {error}")))?;

    interpret(response).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_core_error_envelope_is_flat() {
        // Built from the Core's own type, so the two cannot drift apart
        // without this failing.
        let body = ocinye_contracts::ErrorBody {
            code: ocinye_contracts::ErrorCode::ValidationError,
            message: "A palavra-passe deve ter pelo menos 15 caracteres.".to_owned(),
            details: serde_json::Map::new(),
            request_id: Some("r".to_owned()),
            correlation_id: Some("c".to_owned()),
        };
        let payload: Value = serde_json::to_value(body).expect("serialise");

        assert!(
            payload.get("error").is_none(),
            "the envelope gained an `error` wrapper; api.rs must follow"
        );
        assert_eq!(
            payload.get("message").and_then(Value::as_str),
            Some("A palavra-passe deve ter pelo menos 15 caracteres.")
        );
    }
}

#[cfg(test)]
mod availability_tests {
    use super::*;

    /// Um serviço em falta não é uma avaria.
    ///
    /// # O que se via
    ///
    /// Com o armazenamento em baixo, carregar uma fotografia devolvia «The
    /// object could not be stored.» — em inglês, e a descrever o que falhou em
    /// vez do que se passa. Quem carregou uma fotografia conclui que a
    /// fotografia tem alguma coisa de errado, e volta a tentar com outra.
    ///
    /// O Core já distinguia: `503` para uma dependência que não responde, e
    /// outro código para uma avaria. O Workspace é que juntava os dois em
    /// `Failed(String)` e passava a mensagem crua ao membro.
    #[test]
    fn um_estado_503_nao_se_confunde_com_uma_avaria() {
        // A tradução de estado para facto é feita por código, e não por
        // adivinhar a partir do texto da mensagem.
        assert!(matches!(
            estado_para_falha(503),
            Some(ApiFailure::Unavailable)
        ));
        assert!(matches!(
            estado_para_falha(502),
            Some(ApiFailure::Failed(_))
        ));
        assert!(matches!(
            estado_para_falha(500),
            Some(ApiFailure::Failed(_))
        ));
        assert!(matches!(
            estado_para_falha(401),
            Some(ApiFailure::Unauthorised)
        ));
        assert!(matches!(
            estado_para_falha(403),
            Some(ApiFailure::Forbidden)
        ));
        assert!(matches!(estado_para_falha(404), Some(ApiFailure::Denied)));
        assert!(estado_para_falha(200).is_none());
    }

    /// O que cada estado HTTP do Core significa para o Workspace.
    fn estado_para_falha(status: u16) -> Option<ApiFailure> {
        match status {
            200..=299 => None,
            401 => Some(ApiFailure::Unauthorised),
            403 => Some(ApiFailure::Forbidden),
            404 => Some(ApiFailure::Denied),
            503 => Some(ApiFailure::Unavailable),
            other => Some(ApiFailure::Failed(format!(
                "the Core returned status {other}"
            ))),
        }
    }
}
