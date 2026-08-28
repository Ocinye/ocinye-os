//! Object storage access.
//!
//! The Core authorises, records metadata and issues short-lived signed URLs. It
//! does not proxy bulk transfer. Knowing an object key grants nothing: the
//! bucket is private and every download is authorised first (ADR-0200).

use aws_credential_types::Credentials;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use sha2::{Digest, Sha256};
use std::time::Duration;
use uuid::Uuid;

use crate::config::StorageConfig;
use crate::error::{CoreError, CoreResult};

/// Lifetime of a signed download URL.
pub const DOWNLOAD_URL_TTL: Duration = Duration::from_secs(300);

/// Content types accepted for institutional artefacts.
///
/// An allow-list, not a deny-list: anything not listed is refused rather than
/// stored just in case (`CLAUDE.md` §40).
pub const ALLOWED_CONTENT_TYPES: &[&str] = &[
    "application/pdf",
    "application/json",
    "application/zip",
    "application/gzip",
    "application/x-tar",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.oasis.opendocument.spreadsheet",
    "text/plain",
    "text/markdown",
    "text/csv",
    "text/x-bibtex",
    "image/png",
    "image/jpeg",
    "image/svg+xml",
    "image/tiff",
];

const MAX_FILENAME: usize = 180;

/// Normalise a user-supplied filename for use as metadata.
///
/// The result is never used as an object key. This strips directory traversal,
/// control characters and anything outside a conservative character set.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] when nothing usable remains.
pub fn normalise_filename(raw: &str) -> CoreResult<String> {
    let base = raw
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();

    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();

    let cleaned = cleaned.trim_matches(['-', '.']).to_owned();

    if cleaned.is_empty() {
        return Err(CoreError::Validation(
            "The file name is not usable.".to_owned(),
        ));
    }
    Ok(cleaned.chars().take(MAX_FILENAME).collect())
}

/// Validate a declared content type against the allow-list.
///
/// The client's `Content-Type` is never trusted as fact; it is checked against
/// what the institution accepts.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] when the type is not accepted.
pub fn validate_content_type(raw: &str) -> CoreResult<String> {
    let normalised = raw
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if ALLOWED_CONTENT_TYPES.contains(&normalised.as_str()) {
        Ok(normalised)
    } else {
        Err(CoreError::Validation(
            "This content type is not accepted for institutional artefacts.".to_owned(),
        ))
    }
}

/// Build the opaque object key for a stored artefact.
///
/// System-generated and unrelated to the user's filename.
#[must_use]
pub fn build_object_key(organisation_slug: &str, workspace_id: Uuid, object_id: Uuid) -> String {
    let shard = &object_id.simple().to_string()[..2];
    format!("{organisation_slug}/workspaces/{workspace_id}/{shard}/{object_id}")
}

/// SHA-256 of the given bytes, as lowercase hexadecimal.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Health of the object storage backend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageHealth {
    /// `not_configured`, `ok`, `unreachable` ou `unresponsive`.
    ///
    /// `unreachable` e `unresponsive` não são a mesma avaria, e juntá-los custa
    /// tempo a quem tem de a resolver: a primeira diz que ninguém atende, a
    /// segunda diz que **alguém está a atender e não responde** — tipicamente um
    /// proxy que ficou a segurar o porto depois de o backend desaparecer.
    pub status: &'static str,
    /// Backend code, when configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Declared physical residency of this backend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub residency: Option<&'static str>,
}

/// Quanto tempo se espera por uma resposta antes de dizer que não houve.
///
/// # Porque um probe de saúde tem de ter limite
///
/// Uma dependência que **recusa** ligações falha depressa e não faz mal a
/// ninguém. Uma que **aceita a ligação e depois emudece** não falha nunca: o
/// pedido fica pendurado enquanto houver quem espere. Foi o que aconteceu com o
/// proxy de um contentor MinIO já desligado — o porto continuava a aceitar, e
/// `/ready` deixou de responder de todo. O Workspace concluiu que o Core estava
/// inacessível, e a instituição inteira ficou sem entrar por causa de um bucket.
///
/// Três segundos é generoso para um `HEAD` a um bucket. Um armazenamento que
/// precise de mais do que isto para dizer «estou cá» está, na prática, em baixo.
const HEALTH_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// S3-compatible object storage client.
pub struct ObjectStore {
    client: Client,
    config: StorageConfig,
}

impl ObjectStore {
    /// Build a client from configuration.
    ///
    /// Returns `None` when storage is not configured, which is a legitimate
    /// state: the Core starts and reports storage as unavailable rather than
    /// refusing to run.
    #[must_use]
    pub fn new(config: StorageConfig) -> Option<Self> {
        if !config.is_configured() {
            return None;
        }

        let credentials = Credentials::new(
            config.access_key.clone(),
            config.secret_key.clone(),
            None,
            None,
            "ocinye-core",
        );

        let s3_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .endpoint_url(config.endpoint_url.clone())
            // MinIO and most self-hosted S3 implementations require path style.
            .force_path_style(true)
            .credentials_provider(credentials)
            .build();

        Some(Self {
            client: Client::from_conf(s3_config),
            config,
        })
    }

    /// The configured bucket.
    #[must_use]
    pub fn bucket(&self) -> &str {
        &self.config.bucket
    }

    /// Largest accepted upload.
    #[must_use]
    pub const fn max_upload_bytes(&self) -> u64 {
        self.config.max_upload_bytes
    }

    /// Store an object.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StorageUnavailable`] when the put fails.
    pub async fn put(
        &self,
        key: &str,
        content_type: &str,
        checksum_sha256: &str,
        data: Vec<u8>,
    ) -> CoreResult<()> {
        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(key)
            .content_type(content_type)
            .metadata("ocinye-checksum-sha256", checksum_sha256)
            .body(ByteStream::from(data))
            .send()
            .await
            .map_err(|error| {
                tracing::error!(error = ?error, "object storage put failed");
                CoreError::StorageUnavailable("The object could not be stored.".to_owned())
            })?;
        Ok(())
    }

    /// Fetch an object's bytes.
    ///
    /// The Core does not proxy bulk transfer — that is what the signed URL is
    /// for. This exists for the few objects small enough that a redirect costs
    /// more than the object: the member's avatar is a handful of kilobytes, and
    /// sending the browser to the bucket for it would expose the storage
    /// endpoint in every page and change the image URL on every render.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StorageUnavailable`] when the object cannot be read.
    pub async fn get(&self, key: &str) -> CoreResult<Vec<u8>> {
        let object = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| {
                tracing::error!(error = ?error, "object storage get failed");
                CoreError::StorageUnavailable("The object could not be read.".to_owned())
            })?;

        let bytes = object.body.collect().await.map_err(|error| {
            tracing::error!(error = ?error, "object storage read failed");
            CoreError::StorageUnavailable("The object could not be read.".to_owned())
        })?;

        Ok(bytes.into_bytes().to_vec())
    }

    /// Remove an object.
    ///
    /// Deliberately infallible from the caller's side. It is used to clean up
    /// after a superseded avatar or a failed write, and in both cases the
    /// operation the member asked for has already been decided. Turning a
    /// failed cleanup into a failed request would report a problem the member
    /// cannot act on, about a change that already happened; the orphan is
    /// logged instead.
    pub async fn delete(&self, key: &str) {
        if let Err(error) = self
            .client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
        {
            tracing::error!(error = ?error, key = %key, "object storage delete failed; object is now orphaned");
        }
    }

    /// Issue a short-lived signed download URL.
    ///
    /// Called only after the Core has authorised the download.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StorageUnavailable`] when signing fails.
    pub async fn presigned_download(&self, key: &str, filename: &str) -> CoreResult<String> {
        let presigning = PresigningConfig::expires_in(DOWNLOAD_URL_TTL)
            .map_err(|_| CoreError::Internal("invalid presigning configuration".to_owned()))?;

        // The filename is already normalised, so it cannot break out of the
        // quoted header value.
        let disposition = format!("attachment; filename=\"{filename}\"");

        let request = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(key)
            .response_content_disposition(disposition)
            .presigned(presigning)
            .await
            .map_err(|error| {
                tracing::error!(error = ?error, "presigning failed");
                CoreError::StorageUnavailable("A download URL could not be issued.".to_owned())
            })?;

        Ok(request.uri().to_owned())
    }

    /// Probe the backend.
    pub async fn health(&self) -> StorageHealth {
        let responde = tokio::time::timeout(
            HEALTH_PROBE_TIMEOUT,
            self.client.head_bucket().bucket(&self.config.bucket).send(),
        )
        .await;

        let estado = match responde {
            Ok(Ok(_)) => "ok",
            Ok(Err(error)) => {
                tracing::warn!(error = ?error, "object storage health probe failed");
                "unreachable"
            }
            Err(_) => {
                tracing::warn!(
                    endpoint = %self.config.endpoint_url,
                    timeout_ms = HEALTH_PROBE_TIMEOUT.as_millis(),
                    "object storage accepted the connection and did not answer"
                );
                "unresponsive"
            }
        };

        StorageHealth {
            status: estado,
            backend: Some(self.config.backend_code.clone()),
            residency: Some(self.config.residency.as_str()),
        }
    }
}

/// Health reported when no storage is configured.
#[must_use]
pub const fn unconfigured_health() -> StorageHealth {
    StorageHealth {
        status: "not_configured",
        backend: None,
        residency: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um armazenamento que aceita a ligação e nunca responde não pendura o
    /// probe.
    ///
    /// # O que este teste reproduz
    ///
    /// Não é «o MinIO está em baixo». É o caso pior: o contentor desapareceu e o
    /// proxy do Docker continuou a segurar o porto, a aceitar ligações e a
    /// devolver silêncio. O cliente ligava-se em meio milissegundo e ficava à
    /// espera para sempre; `/ready` bloqueava com ele, e o Workspace declarava o
    /// Core inacessível. O login da instituição inteira caiu por causa de um
    /// bucket que nem sequer é obrigatório.
    ///
    /// O ouvinte aqui aceita e não escreve nada — é exactamente esse
    /// comportamento, sem precisar de Docker nem de rede.
    #[tokio::test]
    async fn um_armazenamento_que_emudece_nao_pendura_o_probe() {
        let ouvinte = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ouvinte");
        let porto = ouvinte.local_addr().expect("endereço").port();

        // Aceita e segura. Nunca responde.
        let _mudo = tokio::spawn(async move {
            // As ligações guardam-se para não serem fechadas: fechar seria uma
            // resposta, e o que se reproduz aqui é a ausência dela.
            let mut ligacoes = Vec::new();
            while let Ok((socket, _)) = ouvinte.accept().await {
                ligacoes.push(socket);
            }
        });

        let store = ObjectStore::new(StorageConfig {
            endpoint_url: format!("http://127.0.0.1:{porto}"),
            region: "us-east-1".to_owned(),
            access_key: "chave".to_owned(),
            secret_key: "segredo".to_owned(),
            bucket: "balde".to_owned(),
            backend_code: "mudo".to_owned(),
            location_label: "teste".to_owned(),
            residency: ocinye_contracts::Residency::default(),
            max_upload_bytes: 1024,
        })
        .expect("configurado");

        let inicio = std::time::Instant::now();
        let saude = store.health().await;
        let demorou = inicio.elapsed();

        assert_eq!(
            saude.status, "unresponsive",
            "quem aceita e emudece não é «unreachable»: alguém está a atender"
        );
        assert!(
            demorou < HEALTH_PROBE_TIMEOUT * 2,
            "o probe demorou {demorou:?}; era suposto desistir ao fim de {HEALTH_PROBE_TIMEOUT:?}"
        );
    }

    #[test]
    fn traversal_and_control_characters_are_stripped_from_filenames() {
        assert_eq!(normalise_filename("../../etc/passwd").unwrap(), "passwd");
        assert_eq!(
            normalise_filename("C:\\Windows\\evil.pdf").unwrap(),
            "evil.pdf"
        );
        assert_eq!(
            normalise_filename("relatório final.pdf").unwrap(),
            "relat-rio-final.pdf"
        );
        assert_eq!(normalise_filename("  report.csv  ").unwrap(), "report.csv");
    }

    #[test]
    fn a_filename_can_never_escape_its_quoted_header_value() {
        let hostile = normalise_filename("a\"; rm -rf /; x=\".pdf").unwrap();
        assert!(!hostile.contains('"'));
        assert!(!hostile.contains(';'));
    }

    #[test]
    fn unusable_filenames_are_rejected_rather_than_defaulted() {
        for bad in ["", "   ", "///", "...", "---"] {
            assert!(normalise_filename(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn content_types_outside_the_allow_list_are_refused() {
        assert_eq!(
            validate_content_type("application/pdf").unwrap(),
            "application/pdf"
        );
        assert_eq!(
            validate_content_type("TEXT/CSV; charset=utf-8").unwrap(),
            "text/csv"
        );
        for bad in [
            "application/x-msdownload",
            "text/html",
            "",
            "application/octet-stream",
        ] {
            assert!(validate_content_type(bad).is_err(), "should refuse {bad:?}");
        }
    }

    #[test]
    fn object_keys_are_opaque_and_ignore_the_user_filename() {
        let key = build_object_key("ocinye", Uuid::from_u128(1), Uuid::from_u128(2));
        assert!(key.starts_with("ocinye/workspaces/"));
        assert!(!key.contains("passwd"));
        assert!(key.ends_with(&Uuid::from_u128(2).to_string()));
    }

    #[test]
    fn checksums_are_stable_lowercase_hex() {
        let digest = sha256_hex(b"ocinye");
        assert_eq!(digest.len(), 64);
        assert!(digest
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
