//! Sign-in, first-login enforcement, password change and administrative reset.
//!
//! This module is the whole of the authentication decision. Everything it
//! returns is a *fact about a session*, never a password and never a hash.
//!
//! # The invariant this file exists to hold
//!
//! > Nobody enters the Ocinye Workspace with the credential an administrator
//! > created for them.
//!
//! It is held by construction, not by convention: authenticating with a
//! temporary credential yields a session in
//! [`SessionState::PasswordChangeRequired`], and the Core refuses ordinary work
//! on such a session at the extractor, before any handler runs.

use chrono::{Duration, Utc};
use ocinye_contracts::{CredentialKind, SessionState};
use sqlx::PgPool;
use uuid::Uuid;

use super::{credentials as creds, repository as repo};
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::password::{policy, Hasher, Secret};
use ocinye_observability::CorrelationIds;

/// How long a session lasts.
pub const SESSION_LIFETIME_HOURS: i64 = 12;

/// How long a restricted password-change session lasts.
///
/// Much shorter than an ordinary session: it exists to complete one task, and a
/// bootstrap session left open overnight is a bootstrap credential left open
/// overnight.
pub const PASSWORD_CHANGE_SESSION_MINUTES: i64 = 30;

/// The single message returned for every failed sign-in.
///
/// One string for wrong username, wrong password, expired credential and
/// suspended account. Anything finer is an account-enumeration oracle
/// (briefing §35).
const SIGN_IN_REFUSED: &str = "Nome de utilizador ou palavra-passe inválidos.";

/// What a caller needs after a successful sign-in.
#[derive(Debug)]
pub struct IssuedSession {
    /// The opaque session token. Exists here and in one `Set-Cookie`, nowhere
    /// else.
    pub token: Secret,
    /// What the session may be used for.
    pub state: SessionState,
    /// Who it belongs to.
    pub person_id: Uuid,
    /// Display name, so the caller need not immediately query for it.
    pub display_name: String,
}

/// Context of an authentication attempt, for throttling and evidence.
#[derive(Debug, Clone, Default)]
pub struct AttemptContext {
    /// Coarse network prefix of the client.
    pub ip_prefix: Option<String>,
    /// Client description, truncated on storage.
    pub user_agent: Option<String>,
}

/// Outcome recorded against an attempt. Never includes the password, its hash
/// or its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Succeeded,
    BadCredentials,
    AccountNotAuthenticable,
    CredentialExpired,
    RateLimited,
}

impl Outcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::BadCredentials => "bad_credentials",
            Self::AccountNotAuthenticable => "account_not_authenticable",
            Self::CredentialExpired => "credential_expired",
            Self::RateLimited => "rate_limited",
        }
    }
}

/// Throttling thresholds.
///
/// Deliberately not a lockout. Locking an account after N failures hands anyone
/// who knows a username a denial-of-service against that person (briefing §37).
/// Instead the response is delayed and then refused for a window that ends by
/// itself.
#[derive(Debug, Clone, Copy)]
pub struct Throttle {
    /// Failures from one network prefix before refusal.
    pub per_ip: i64,
    /// Failures against one username before refusal.
    pub per_username: i64,
    /// Window over which failures are counted, in minutes.
    pub window_minutes: i64,
}

impl Default for Throttle {
    fn default() -> Self {
        Self {
            per_ip: 20,
            per_username: 10,
            window_minutes: 15,
        }
    }
}

/// Used only if hashing at construction fails, which the configuration guard
/// makes unreachable. Real Argon2id, at the OWASP baseline.
const FALLBACK_DUMMY_VERIFIER: &str = "$argon2id$v=19$m=19456,t=2,p=1\
                                       $c29tZXNhbHRzb21lc2FsdA\
                                       $RdescudvJCsgt3ub+b+dWRWJTmaaJObG";

/// Everything the authentication service needs.
pub struct Authenticator {
    /// Password hasher, carrying the cost parameters in force.
    pub hasher: Hasher,
    /// Throttling thresholds.
    pub throttle: Throttle,
    /// How long a temporary credential remains valid.
    pub temporary_credential_hours: i64,
    /// A verifier nobody holds, produced with **these** parameters.
    ///
    /// See [`Authenticator::burn_equivalent_work`]. Built once at construction
    /// so the equalising verification costs what a real one costs.
    dummy_verifier: String,
}

impl Authenticator {
    /// Build an authenticator.
    ///
    /// # Panics
    ///
    /// Never in practice: the only way the dummy verifier fails to build is
    /// invalid Argon2 parameters, and those are rejected by
    /// [`HashingParams::validate`](crate::password::HashingParams::validate) at
    /// configuration time. A fallback constant is used rather than aborting,
    /// because refusing to authenticate anyone would be a worse failure than a
    /// slightly mistimed refusal.
    #[must_use]
    pub fn new(hasher: Hasher, throttle: Throttle, temporary_credential_hours: i64) -> Self {
        // Hashed with the configured parameters, at startup, once.
        //
        // The value is irrelevant — nothing verifies against it successfully —
        // but the *parameters* are not, which is the whole point.
        let dummy_verifier = hasher
            .hash(&Secret::new(
                "ocinye-timing-equalisation-value-that-nobody-holds",
            ))
            .unwrap_or_else(|_| FALLBACK_DUMMY_VERIFIER.to_owned());

        Self {
            hasher,
            throttle,
            temporary_credential_hours,
            dummy_verifier,
        }
    }

    /// Authenticate a username and password.
    ///
    /// # The shape of a refusal
    ///
    /// Every failure path returns the same message and takes broadly the same
    /// work: when no account matches, a verification is still performed against
    /// a dummy verifier so that "no such user" and "wrong password" do not
    /// differ by the cost of an Argon2 hash.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Unauthenticated`] for every failure, and
    /// [`CoreError::RateLimited`] when the attempt is throttled.
    pub async fn sign_in(
        &self,
        pool: &PgPool,
        username: &str,
        password: &Secret,
        context: &AttemptContext,
        ids: &CorrelationIds,
    ) -> CoreResult<IssuedSession> {
        let username = username.trim();

        if self.is_throttled(pool, username, context).await? {
            record_attempt(pool, username, context, Outcome::RateLimited).await;
            return Err(CoreError::RateLimited(
                "Demasiadas tentativas. Aguarde alguns minutos.".to_owned(),
            ));
        }

        // A password longer than the policy allows is refused before hashing:
        // an unauthenticated caller must not be able to choose how much work
        // the server does.
        if password.len_bytes() > policy::MAX_BYTES {
            record_attempt(pool, username, context, Outcome::BadCredentials).await;
            return Err(CoreError::Unauthenticated(SIGN_IN_REFUSED.to_owned()));
        }

        let candidate = policy::normalise(password);
        let person = repo::find_by_username(pool, username).await?;

        let Some(person) = person else {
            self.burn_equivalent_work(&candidate);
            record_attempt(pool, username, context, Outcome::BadCredentials).await;
            return Err(CoreError::Unauthenticated(SIGN_IN_REFUSED.to_owned()));
        };

        if !person.account_status().may_authenticate() {
            self.burn_equivalent_work(&candidate);
            record_attempt(pool, username, context, Outcome::AccountNotAuthenticable).await;
            return Err(CoreError::Unauthenticated(SIGN_IN_REFUSED.to_owned()));
        }

        let now = Utc::now();
        let live = creds::live_credentials(pool, person.id).await?;

        // Permanent first: someone who has set their own password should not be
        // sent through the change flow because a stale temporary row survives.
        let permanent = live
            .iter()
            .find(|c| c.kind == CredentialKind::Permanent && c.is_usable(now));
        let temporary = live
            .iter()
            .find(|c| c.kind == CredentialKind::Temporary && c.is_usable(now));

        let matched = permanent
            .filter(|c| self.hasher.verify(&candidate, &c.verifier))
            .map(|c| (c, SessionState::Active))
            .or_else(|| {
                temporary
                    .filter(|c| self.hasher.verify(&candidate, &c.verifier))
                    .map(|c| (c, SessionState::PasswordChangeRequired))
            });

        let Some((credential, session_state)) = matched else {
            // Distinguish *in the evidence trail only* between a wrong password
            // and a credential that has run out, so an operator can tell a
            // support call from an attack. The caller still sees one message.
            let expired = live.iter().any(|c| c.has_expired(now));
            let outcome = if expired {
                Outcome::CredentialExpired
            } else {
                Outcome::BadCredentials
            };
            if permanent.is_none() && temporary.is_none() {
                self.burn_equivalent_work(&candidate);
            }
            record_attempt(pool, username, context, outcome).await;
            return Err(CoreError::Unauthenticated(SIGN_IN_REFUSED.to_owned()));
        };

        // The password was right. If it is stored under weaker parameters than
        // are now configured, upgrade it while the plaintext is at hand.
        if self.hasher.needs_rehash(&credential.verifier) {
            match self.hasher.hash(&candidate) {
                Ok(verifier) => {
                    if let Err(error) =
                        creds::replace_verifier(pool, credential.id, &verifier).await
                    {
                        tracing::warn!(error = %error, "could not rehash a verifier");
                    }
                }
                Err(error) => tracing::warn!(error = %error, "could not rehash a verifier"),
            }
        }

        let lifetime = if session_state == SessionState::Active {
            Duration::hours(SESSION_LIFETIME_HOURS)
        } else {
            Duration::minutes(PASSWORD_CHANGE_SESSION_MINUTES)
        };

        let mut tx = pool.begin().await?;
        let (session_id, token) = creds::create_session(
            &mut *tx,
            person.id,
            session_state,
            lifetime,
            context.user_agent.as_deref(),
            context.ip_prefix.as_deref(),
        )
        .await?;

        audit::record(
            &mut tx,
            None,
            ids,
            AuditEntry::new(action::SIGN_IN, "person")
                .resource(person.id)
                .actor(person.id, person.organisation_id)
                .detail("session_state", session_state.as_str())
                .detail("credential_kind", credential.kind.as_str())
                .detail("session_id", session_id.to_string()),
        )
        .await?;
        tx.commit().await?;

        record_attempt(pool, username, context, Outcome::Succeeded).await;

        Ok(IssuedSession {
            token,
            state: session_state,
            person_id: person.id,
            display_name: person.preferred_name().to_owned(),
        })
    }

    /// Spend roughly the work a real verification costs.
    ///
    /// Without this, "no such username" returns in microseconds while a wrong
    /// password takes the Argon2 time, and the difference is measurable over
    /// the network. The dummy verifier is a real Argon2id hash of a value
    /// nobody holds.
    ///
    /// # Why it is built from the configured parameters
    ///
    /// Argon2 reads its cost from the PHC string it is verifying against, not
    /// from the hasher. A constant dummy therefore costs whatever *its* string
    /// says — and `docs/security/` tells operators to benchmark and raise
    /// `OCINYE_ARGON2_MEMORY_KIB`. Following that advice used to widen the gap
    /// between "no such account" and "wrong password" until it was measurable,
    /// silently reopening the enumeration oracle this function closes.
    ///
    /// Building the verifier from [`Self::hasher`] at construction means the two
    /// paths cost the same by construction, whatever the parameters are.
    fn burn_equivalent_work(&self, candidate: &Secret) {
        let _ = self.hasher.verify(candidate, &self.dummy_verifier);
    }

    async fn is_throttled(
        &self,
        pool: &PgPool,
        username: &str,
        context: &AttemptContext,
    ) -> CoreResult<bool> {
        let window = Duration::minutes(self.throttle.window_minutes);

        let by_username: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM authentication_attempts
              WHERE lower(username) = lower($1)
                AND outcome <> 'succeeded'
                AND attempted_at > now() - $2",
        )
        .bind(username)
        .bind(window)
        .fetch_one(pool)
        .await?;

        if by_username >= self.throttle.per_username {
            return Ok(true);
        }

        if let Some(prefix) = context.ip_prefix.as_deref() {
            let by_ip: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM authentication_attempts
                  WHERE ip_prefix = $1
                    AND outcome <> 'succeeded'
                    AND attempted_at > now() - $2",
            )
            .bind(prefix)
            .bind(window)
            .fetch_one(pool)
            .await?;

            if by_ip >= self.throttle.per_ip {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

/// Record an attempt.
///
/// Best-effort: a failure to write evidence must not change the authentication
/// outcome, but it must be loud in the log.
async fn record_attempt(pool: &PgPool, username: &str, context: &AttemptContext, outcome: Outcome) {
    let result = sqlx::query(
        "INSERT INTO authentication_attempts (username, ip_prefix, outcome)
         VALUES ($1, $2, $3)",
    )
    .bind(username.chars().take(64).collect::<String>())
    .bind(context.ip_prefix.as_deref())
    .bind(outcome.as_str())
    .execute(pool)
    .await;

    if let Err(error) = result {
        tracing::error!(error = %error, "could not record an authentication attempt");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_carries_the_same_message() {
        // The message must not tell an attacker which of the four things failed.
        for forbidden in ["não existe", "expirad", "suspens", "utilizador não"] {
            assert!(
                !SIGN_IN_REFUSED.to_lowercase().contains(forbidden),
                "the refusal message leaks {forbidden:?}"
            );
        }
    }

    #[test]
    fn a_restricted_session_is_much_shorter_than_an_ordinary_one() {
        assert!(
            Duration::minutes(PASSWORD_CHANGE_SESSION_MINUTES)
                < Duration::hours(SESSION_LIFETIME_HOURS)
        );
    }

    #[test]
    fn throttling_counts_both_account_and_origin() {
        let throttle = Throttle::default();
        assert!(throttle.per_username > 0);
        assert!(throttle.per_ip > throttle.per_username);
        assert!(throttle.window_minutes > 0);
    }

    #[test]
    fn the_dummy_verifier_parses_as_argon2id_so_the_work_is_real() {
        // If it did not parse, `verify` would return early and the timing
        // equalisation this exists for would silently stop working.
        let hasher = Hasher::new(crate::password::HashingParams {
            memory_kib: 8 * 1024,
            iterations: 2,
            parallelism: 1,
        });
        let authenticator = Authenticator::new(hasher, Throttle::default(), 24);
        let before = std::time::Instant::now();
        authenticator.burn_equivalent_work(&Secret::new("anything at all here"));
        assert!(
            before.elapsed() > std::time::Duration::from_micros(200),
            "the dummy verification did no work: {:?}",
            before.elapsed()
        );
    }

    /// O trabalho gasto quando não existe conta é o mesmo que existindo.
    ///
    /// # Porque este teste existe
    ///
    /// O verificador de equalização era uma constante com `m=19456,t=2,p=1`
    /// escrito lá dentro. O Argon2 lê o custo da string PHC que verifica, não do
    /// hasher — por isso, num Ocinye configurado com mais memória (que é o que
    /// `docs/security/` manda fazer), a verificação falsa passava a custar uma
    /// fracção da verdadeira, e a diferença entre «não existe» e «palavra-passe
    /// errada» voltava a ser mensurável.
    ///
    /// O teste compara os dois caminhos com parâmetros **acima** do valor que
    /// estava fixado. A margem é larga de propósito: mede tempo numa máquina
    /// partilhada, e o que interessa é apanhar uma ordem de grandeza, não medir
    /// nanossegundos.
    #[test]
    fn a_equalizacao_acompanha_os_parametros_configurados() {
        let params = crate::password::HashingParams {
            // Bem acima dos 19 MiB que estavam escritos na constante.
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        };
        let authenticator = Authenticator::new(Hasher::new(params), Throttle::default(), 24);
        let candidate = Secret::new("uma palavra-passe qualquer para medir");

        // O caminho real: verificar contra um verificador destes parâmetros.
        let real_verifier = Hasher::new(params)
            .hash(&Secret::new("outra coisa"))
            .expect("hash");

        // Aquece, para não medir o primeiro toque na memória.
        authenticator.burn_equivalent_work(&candidate);
        let _ = authenticator.hasher.verify(&candidate, &real_verifier);

        let dummy = {
            let start = std::time::Instant::now();
            authenticator.burn_equivalent_work(&candidate);
            start.elapsed()
        };
        let real = {
            let start = std::time::Instant::now();
            let _ = authenticator.hasher.verify(&candidate, &real_verifier);
            start.elapsed()
        };

        assert!(
            dummy.as_nanos() * 4 >= real.as_nanos(),
            "a verificação de equalização custa muito menos do que a real \
             ({dummy:?} contra {real:?}): «não existe» voltou a ser distinguível \
             de «palavra-passe errada»"
        );
    }

    #[test]
    fn outcome_labels_never_include_credential_material() {
        for outcome in [
            Outcome::Succeeded,
            Outcome::BadCredentials,
            Outcome::AccountNotAuthenticable,
            Outcome::CredentialExpired,
            Outcome::RateLimited,
        ] {
            let label = outcome.as_str();
            assert!(!label.contains("password"));
            assert!(!label.contains("hash"));
        }
    }
}
