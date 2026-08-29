//! Client for the Ocinye Core.
//!
//! Adds the member's bearer token and propagates the correlation identifier, so
//! one operation can be followed from the Workspace through the Core and into
//! the Worker.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::WorkspaceState;

/// A razão que o Core deu para um `503`, se deu alguma.
///
/// O envelope de erro do Core é plano: `message` no topo. Uma razão vazia é o
/// mesmo que nenhuma — mostrar uma linha em branco seria pior do que mostrar a
/// frase genérica.
fn razao(payload: &Value) -> Option<String> {
    let texto = payload.get("message").and_then(Value::as_str)?.trim();
    (!texto.is_empty()).then(|| texto.to_owned())
}

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
    ///
    /// **Traz a razão que o Core deu**, quando ele a deu. Sem isto, o Core
    /// dizia «o correio institucional ainda não foi configurado nesta
    /// instalação» e o membro lia «quem administra o sistema saberá o que
    /// falta» — a frase que dizia o que falta era calculada e deitada fora no
    /// caminho. `None` quando o corpo não trouxe nenhuma.
    Unavailable(Option<String>),
    /// O Core percebeu o pedido e recusou-o pelo conteúdo.
    ///
    /// `422`, e só `422`. É a única recusa cuja mensagem foi **escrita para o
    /// membro** — «uma reprodução precisa da execução que a reproduziu» — e
    /// não um detalhe interno. Juntá-la a [`ApiFailure::Failed`] trocava essa
    /// frase por uma referência de log, o que manda a pessoa perguntar a
    /// alguém aquilo que o sistema já sabia dizer-lhe.
    Rejected(String),
    /// Anything else.
    Failed(String),
}

impl std::fmt::Display for ApiFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorised => f.write_str("the session is no longer valid"),
            Self::Denied => f.write_str("this is not available to you"),
            Self::Forbidden => f.write_str("you do not have access to this operation"),
            Self::Unavailable(Some(razao)) => f.write_str(razao),
            Self::Unavailable(None) => f.write_str("a dependency of this operation is unavailable"),
            Self::Rejected(message) | Self::Failed(message) => f.write_str(message),
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
        503 => Err(ApiFailure::Unavailable(razao(
            &response.json().await.unwrap_or(Value::Null),
        ))),
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

/// Send a `DELETE` to the Core.
///
/// Existe porque retirar alguém de um grupo é uma remoção, e escrevê-la como um
/// `POST` para `/remove` faria o verbo mentir sobre o que acontece.
///
/// # Errors
///
/// Returns [`ApiFailure`] when the Core is unreachable or refuses.
pub async fn delete(
    state: &WorkspaceState,
    token: &str,
    correlation_id: &str,
    path: &str,
) -> Result<Value, ApiFailure> {
    let response = state
        .http
        .delete(format!("{}{path}", state.config.core_url))
        .bearer_auth(token)
        .header(ocinye_observability::CORRELATION_ID_HEADER, correlation_id)
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
    let status = response.status().as_u16();
    let payload: Value = response.json().await.unwrap_or(Value::Null);

    match falha_de(status, &payload) {
        Some(falha) => Err(falha),
        None => Ok(payload),
    }
}

/// O que um estado e um corpo do Core significam para o Workspace.
///
/// Separada de [`interpret`] para poder ser exercida por um teste. A versão
/// anterior tinha a decisão dentro da função assíncrona e o teste exercia uma
/// **cópia** dela escrita ao lado — e uma cópia continua verde enquanto o
/// original muda, que foi o que aconteceu: uma reversão que fazia o cliente
/// deitar a razão fora não moveu nenhum portão.
fn falha_de(status: u16, payload: &Value) -> Option<ApiFailure> {
    match status {
        200..=299 => None,
        401 => Some(ApiFailure::Unauthorised),
        403 => Some(ApiFailure::Forbidden),
        404 => Some(ApiFailure::Denied),
        503 => Some(ApiFailure::Unavailable(razao(payload))),
        422 => Some(ApiFailure::Rejected(
            payload
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("O pedido não foi aceite.")
                .to_owned(),
        )),
        _ => Some(ApiFailure::Failed(
            payload
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("the operation could not be completed")
                .to_owned(),
        )),
    }
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
        .unwrap_or("Endereço ou palavra-passe inválidos.")
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
        503 => Err(ApiFailure::Unavailable(razao(
            &response.json().await.unwrap_or(Value::Null),
        ))),
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

/// Um upload com campos ao lado do ficheiro.
///
/// O `upload` acima serve o caso de um ficheiro sozinho — a fotografia. Os
/// ficheiros institucionais trazem escolhas da pessoa que os carrega: a pasta
/// onde ficam e a classificação que declaram. Vão como campos do mesmo
/// multipart porque é o Core que os lê, e vão sem interpretação nenhuma daqui:
/// esta função não decide o que é uma classificação válida.
///
/// # Errors
///
/// Devolve erro quando o multipart não se forma, quando o Core não responde, ou
/// quando o Core recusa.
#[allow(clippy::too_many_arguments)]
pub async fn upload_with_fields(
    state: &WorkspaceState,
    token: &str,
    correlation_id: &str,
    path: &str,
    filename: String,
    content_type: String,
    data: Vec<u8>,
    fields: Vec<(&str, String)>,
) -> Result<Value, ApiFailure> {
    let part = reqwest::multipart::Part::bytes(data)
        .file_name(filename)
        .mime_str(&content_type)
        .map_err(|_| ApiFailure::Failed("the upload is malformed".to_owned()))?;

    let mut form = reqwest::multipart::Form::new().part("file", part);
    for (nome, valor) in fields {
        if !valor.is_empty() {
            form = form.text(nome.to_owned(), valor);
        }
    }

    let response = state
        .http
        .post(format!("{}{path}", state.config.core_url))
        .bearer_auth(token)
        .header(ocinye_observability::CORRELATION_ID_HEADER, correlation_id)
        .multipart(form)
        .send()
        .await
        .map_err(|error| ApiFailure::Failed(format!("the Core is unreachable: {error}")))?;

    interpret(response).await
}

/// Lê bytes de uma ligação assinada, até um limite.
///
/// Serve a pré-visualização: o Workspace lê o conteúdo com a mesma sessão com
/// que já lê tudo o resto, e devolve-o à página. Não guarda nada, e o limite
/// existe para que um ficheiro grande não passe a ser um problema de memória
/// desta aplicação.
///
/// # Errors
///
/// Devolve erro quando a ligação não responde ou responde com falha.
pub async fn fetch_bounded(
    state: &WorkspaceState,
    url: &str,
    limite: usize,
) -> Result<Vec<u8>, ApiFailure> {
    let response = state
        .http
        .get(url)
        .send()
        .await
        .map_err(|error| ApiFailure::Failed(format!("the object is unreachable: {error}")))?;

    if !response.status().is_success() {
        return Err(ApiFailure::Failed(format!(
            "the object store answered {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| ApiFailure::Failed(format!("the object could not be read: {error}")))?;

    Ok(bytes.into_iter().take(limite).collect())
}

/// Uma representação inline vinda do Core, com o tipo que o Core declarou.
///
/// Devolve os bytes e o `Content-Type` **tal como o Core os deu**. O Workspace
/// não reinterpreta nem adivinha o tipo: quem o validou foi o Core, contra uma
/// lista fechada, e adivinhar aqui reabriria exactamente a porta que essa lista
/// fecha.
///
/// # Errors
///
/// Devolve [`ApiFailure`] a descrever porque não se conseguiu.
pub async fn get_inline(
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
            let tipo = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|valor| valor.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_owned();
            let bytes = response
                .bytes()
                .await
                .map_err(|error| ApiFailure::Failed(format!("unexpected response: {error}")))?;
            Ok((tipo, bytes.to_vec()))
        }
        401 => Err(ApiFailure::Unauthorised),
        403 => Err(ApiFailure::Forbidden),
        404 => Err(ApiFailure::Denied),
        503 => Err(ApiFailure::Unavailable(None)),
        status => Err(ApiFailure::Failed(format!(
            "the Core returned status {status}"
        ))),
    }
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
mod rejection_tests {
    use super::*;

    /// A frase que o Core escreveu para o membro chega ao membro.
    ///
    /// # O defeito que isto guarda
    ///
    /// `422` caía no ramo genérico, virava [`ApiFailure::Failed`], e o
    /// `failure_response` troca uma `Failed` por uma referência de log. O Core
    /// dizia «uma reprodução precisa da execução que a reproduziu» — a frase
    /// que responde à pergunta — e a pessoa lia um identificador hexadecimal.
    ///
    /// Construído a partir do tipo do próprio Core, para que o estado não possa
    /// mudar sem isto falhar.
    #[test]
    fn uma_recusa_por_conteudo_traz_a_razao() {
        let codigo = ocinye_contracts::ErrorCode::ValidationError;
        let corpo = ocinye_contracts::ErrorBody {
            code: codigo,
            message: "Uma reprodução precisa da execução que a reproduziu.".to_owned(),
            details: serde_json::Map::new(),
            request_id: None,
            correlation_id: None,
        };
        let payload: Value = serde_json::to_value(corpo).expect("serialise");

        match falha_de(codigo.status(), &payload) {
            Some(ApiFailure::Rejected(razao)) => assert_eq!(
                razao, "Uma reprodução precisa da execução que a reproduziu.",
                "a razão chegou truncada ou trocada"
            ),
            outra => {
                panic!("uma recusa por conteúdo tem de ser `Rejected` com a razão, e foi {outra:?}")
            }
        }
    }

    /// Um erro genuíno continua a ser um erro, e não uma recusa.
    ///
    /// Sem esta metade, `Rejected` podia engolir tudo o que não fosse 401, 403,
    /// 404 ou 503 — e um `500` passaria a mostrar ao membro o texto interno de
    /// uma avaria em vez de uma referência de log.
    #[test]
    fn uma_avaria_nao_e_uma_recusa() {
        let payload = serde_json::json!({"message": "connection reset by peer"});
        assert!(
            matches!(falha_de(500, &payload), Some(ApiFailure::Failed(_))),
            "um 500 tem de continuar a ser uma avaria"
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
    /// Chama a decisão a sério, sem corpo.
    fn falha_de_teste(status: u16) -> Option<ApiFailure> {
        falha_de(status, &Value::Null)
    }

    /// A razão que o Core escreveu chega intacta ao Workspace.
    ///
    /// # O defeito que isto guarda
    ///
    /// O Core respondeu `503` com «O correio institucional ainda não foi
    /// configurado nesta instalação do Ocinye OS.» — a frase que diz o que
    /// falta. O cliente mapeava o estado para uma variante **sem campos**, a
    /// frase morria aqui, e a página mostrava a genérica: «quem administra o
    /// sistema saberá o que falta», a quem administra o sistema.
    #[test]
    fn a_razao_de_um_503_nao_se_perde_no_cliente() {
        let corpo = serde_json::json!({
            "code": "capability_unavailable",
            "message": "O correio institucional ainda não foi configurado nesta \
                        instalação do Ocinye OS.",
            "request_id": "abc",
        });

        let Some(ApiFailure::Unavailable(Some(razao))) = falha_de(503, &corpo) else {
            panic!("um 503 com razão tem de chegar como razão");
        };
        assert!(razao.contains("ainda não foi configurado"));

        // Sem corpo, ou com uma razão vazia, não se inventa nenhuma: o ecrã
        // tem uma frase genérica para esse caso, e uma linha em branco seria
        // pior do que ela.
        assert!(matches!(
            falha_de(503, &Value::Null),
            Some(ApiFailure::Unavailable(None))
        ));
        assert!(matches!(
            falha_de(503, &serde_json::json!({"message": "   "})),
            Some(ApiFailure::Unavailable(None))
        ));
    }

    #[test]
    fn um_estado_503_nao_se_confunde_com_uma_avaria() {
        // A tradução de estado para facto é feita por código, e não por
        // adivinhar a partir do texto da mensagem.
        assert!(matches!(
            falha_de_teste(503),
            Some(ApiFailure::Unavailable(_))
        ));
        assert!(matches!(falha_de_teste(502), Some(ApiFailure::Failed(_))));
        assert!(matches!(falha_de_teste(500), Some(ApiFailure::Failed(_))));
        assert!(matches!(
            falha_de_teste(401),
            Some(ApiFailure::Unauthorised)
        ));
        assert!(matches!(falha_de_teste(403), Some(ApiFailure::Forbidden)));
        assert!(matches!(falha_de_teste(404), Some(ApiFailure::Denied)));
        assert!(falha_de_teste(200).is_none());
    }
}
