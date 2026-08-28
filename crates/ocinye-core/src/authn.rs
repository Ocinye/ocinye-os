//! OIDC token verification.
//!
//! Ocinye Core never implements its own authentication (ADR-0102). It trusts an
//! external Identity Provider over OIDC and verifies the presented access token
//! against the provider's published JWKS. Passwords are never seen, stored,
//! proxied or logged.
//!
//! # No development bypass
//!
//! There is deliberately no "skip verification in development" path. A weakened
//! branch in the shipped binary is a weakened branch in production the day
//! someone mis-sets an environment variable. Tests substitute the *dependency*
//! that produces a verified subject, so no such branch exists here at all.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::config::OidcConfig;
use crate::error::{CoreError, CoreResult};

/// Claims the Core requires and reads.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenClaims {
    /// Subject: the stable identifier of the person at the IdP.
    pub sub: String,
    /// Email, when the IdP releases it.
    #[serde(default)]
    pub email: Option<String>,
    /// Display name, when the IdP releases it.
    #[serde(default)]
    pub name: Option<String>,
    /// Preferred username, used as a fallback for display.
    #[serde(default)]
    pub preferred_username: Option<String>,
}

impl TokenClaims {
    /// Best available display name.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.preferred_username.clone())
            .unwrap_or_else(|| self.sub.clone())
    }
}

/// A token whose signature, issuer, audience and expiry have been verified.
#[derive(Debug, Clone)]
pub struct VerifiedToken {
    /// The verified claims.
    pub claims: TokenClaims,
}

#[derive(Debug, Deserialize)]
struct Discovery {
    jwks_uri: String,
}

struct CachedKeys {
    keys: JwkSet,
    fetched_at: Instant,
}

/// Verifies access tokens against the identity provider.
pub struct TokenVerifier {
    config: OidcConfig,
    http: reqwest::Client,
    cache: RwLock<Option<CachedKeys>>,
}

impl TokenVerifier {
    /// Build a verifier.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be constructed.
    pub fn new(config: OidcConfig) -> CoreResult<Arc<Self>> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            // The IdP is reached by a configured URL; there is no reason for a
            // redirect, and following one would widen SSRF surface.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| CoreError::Internal(format!("http client: {error}")))?;

        Ok(Arc::new(Self {
            config,
            http,
            cache: RwLock::new(None),
        }))
    }

    /// Whether an identity provider is configured at all.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.config.issuer.is_empty()
    }

    async fn keys(&self) -> CoreResult<JwkSet> {
        if let Some(cached) = self.cache.read().await.as_ref() {
            if cached.fetched_at.elapsed() < self.config.jwks_cache {
                return Ok(cached.keys.clone());
            }
        }

        let keys = self.fetch_keys().await?;
        *self.cache.write().await = Some(CachedKeys {
            keys: keys.clone(),
            fetched_at: Instant::now(),
        });
        Ok(keys)
    }

    async fn fetch_keys(&self) -> CoreResult<JwkSet> {
        if !self.is_configured() {
            return Err(CoreError::Internal(
                "the identity provider is not configured".to_owned(),
            ));
        }

        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            self.config.issuer.trim_end_matches('/')
        );

        let discovery: Discovery = self
            .http
            .get(&discovery_url)
            .send()
            .await
            .map_err(|_| CoreError::Internal("the identity provider is not reachable".to_owned()))?
            .error_for_status()
            .map_err(|_| CoreError::Internal("identity provider discovery failed".to_owned()))?
            .json()
            .await
            .map_err(|_| {
                CoreError::Internal("identity provider discovery is malformed".to_owned())
            })?;

        self.http
            .get(&discovery.jwks_uri)
            .send()
            .await
            .map_err(|_| {
                CoreError::Internal("the identity provider JWKS is not reachable".to_owned())
            })?
            .error_for_status()
            .map_err(|_| CoreError::Internal("identity provider JWKS request failed".to_owned()))?
            .json()
            .await
            .map_err(|_| CoreError::Internal("identity provider JWKS is malformed".to_owned()))
    }

    /// Verify an access token.
    ///
    /// Validates signature, issuer, audience and expiry. Anything else is
    /// rejected. Failures are deliberately indistinguishable to the caller: a
    /// token that is expired, forged or simply unparseable all produce the same
    /// response, so the endpoint is not an oracle.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Unauthenticated`] when the token is not valid, and
    /// [`CoreError::Internal`] when the provider cannot be consulted.
    pub async fn verify(&self, token: &str) -> CoreResult<VerifiedToken> {
        // Sem fornecedor configurado não existe nada com que comparar, e por
        // isso a resposta é conhecida antes de se olhar para o token. Recusar
        // aqui, e não mais abaixo, é o que impede que entrada controlada por
        // terceiros seja analisada por um verificador que nunca poderia aceitar
        // seja o que for — e faz da recusa uma falha de autenticação, que é o
        // que de facto é, em vez de uma avaria interna do Ocinye.
        if !self.is_configured() {
            return Err(Self::rejected());
        }

        let header = decode_header(token).map_err(|_| Self::rejected())?;
        let kid = header.kid.ok_or_else(Self::rejected)?;

        let keys = self.keys().await?;
        let jwk = keys.find(&kid).ok_or_else(Self::rejected)?;

        let decoding_key = match &jwk.algorithm {
            AlgorithmParameters::RSA(rsa) => {
                DecodingKey::from_rsa_components(&rsa.n, &rsa.e).map_err(|_| Self::rejected())?
            }
            AlgorithmParameters::EllipticCurve(ec) => {
                DecodingKey::from_ec_components(&ec.x, &ec.y).map_err(|_| Self::rejected())?
            }
            _ => return Err(Self::rejected()),
        };

        // The algorithm comes from the *key*, never from the token header:
        // trusting the header is how `alg: none` and algorithm-confusion
        // attacks work.
        let algorithm = jwk
            .common
            .key_algorithm
            .and_then(|alg| alg.to_string().parse().ok())
            .ok_or_else(Self::rejected)?;

        let mut validation = Validation::new(algorithm);
        validation.set_issuer(&[self.config.issuer.trim_end_matches('/')]);
        validation.set_audience(&[self.config.audience.as_str()]);
        validation.required_spec_claims =
            HashSet::from(["exp".to_owned(), "iss".to_owned(), "aud".to_owned()]);
        validation.validate_exp = true;

        let data = decode::<TokenClaims>(token, &decoding_key, &validation)
            .map_err(|_| Self::rejected())?;

        if data.claims.sub.trim().is_empty() {
            return Err(Self::rejected());
        }

        Ok(VerifiedToken {
            claims: data.claims,
        })
    }

    fn rejected() -> CoreError {
        CoreError::Unauthenticated("The presented token is not valid.".to_owned())
    }
}

/// Extract a bearer token from an `Authorization` header value.
///
/// # Errors
///
/// Returns [`CoreError::Unauthenticated`] when the header is absent or is not a
/// bearer token.
pub fn bearer_token(header: Option<&str>) -> CoreResult<&str> {
    let value = header
        .ok_or_else(|| CoreError::Unauthenticated("A bearer token is required.".to_owned()))?;

    let (scheme, token) = value
        .split_once(' ')
        .ok_or_else(|| CoreError::Unauthenticated("A bearer token is required.".to_owned()))?;

    if !scheme.eq_ignore_ascii_case("bearer") || token.trim().is_empty() {
        return Err(CoreError::Unauthenticated(
            "A bearer token is required.".to_owned(),
        ));
    }
    Ok(token.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_tokens_are_extracted_case_insensitively() {
        assert_eq!(bearer_token(Some("Bearer abc")).unwrap(), "abc");
        assert_eq!(bearer_token(Some("bearer abc")).unwrap(), "abc");
        assert_eq!(bearer_token(Some("BEARER  abc ")).unwrap(), "abc");
    }

    #[test]
    fn anything_that_is_not_a_bearer_token_is_refused() {
        for header in [
            None,
            Some(""),
            Some("abc"),
            Some("Basic abc"),
            Some("Bearer "),
            Some("Bearer"),
        ] {
            assert!(bearer_token(header).is_err(), "should refuse {header:?}");
        }
    }

    #[test]
    fn rejection_messages_do_not_distinguish_failure_modes() {
        // An endpoint that says *why* a token failed is an oracle.
        let a = TokenVerifier::rejected().public_message();
        let b = bearer_token(None).unwrap_err().public_message();
        assert!(!a.contains("expired") && !a.contains("signature"));
        assert!(!b.contains("expired") && !b.contains("signature"));
    }

    #[test]
    fn display_name_falls_back_through_available_claims() {
        let claims = TokenClaims {
            sub: "sub-1".into(),
            email: None,
            name: None,
            preferred_username: Some("jdoe".into()),
        };
        assert_eq!(claims.display_name(), "jdoe");

        let bare = TokenClaims {
            sub: "sub-1".into(),
            email: None,
            name: None,
            preferred_username: None,
        };
        assert_eq!(bare.display_name(), "sub-1");
    }
}
