//! Credential and session persistence.
//!
//! Every function here takes or returns a *verifier*, never a password. The
//! only plaintext that crosses this module is the candidate being verified, and
//! it arrives as a [`Secret`] and leaves nothing behind.
//!
//! # There is no session upgrade
//!
//! Note the absence of a function that promotes a restricted session to an
//! ordinary one. A session issued for a password change is revoked and
//! replaced, never upgraded in place, so the identifier the browser held during
//! the bootstrap step cannot be reused afterwards (briefing §30).

use chrono::{DateTime, Duration, Utc};
use ocinye_contracts::{CredentialKind, CredentialState, Permission, Scope, SessionState};
use ocinye_domain::ExplicitGrant;
use sha2::{Digest, Sha256};
use sqlx::{PgExecutor, Row};
use uuid::Uuid;

use crate::error::CoreResult;
use crate::password::Secret;

/// A stored credential, without its plaintext.
#[derive(Debug, Clone)]
pub struct Credential {
    /// Identifier.
    pub id: Uuid,
    /// Whose it is.
    pub person_id: Uuid,
    /// Temporary or permanent.
    pub kind: CredentialKind,
    /// Lifecycle state.
    pub state: CredentialState,
    /// Argon2id PHC verifier.
    pub verifier: String,
    /// When a temporary credential stops being usable.
    pub expires_at: Option<DateTime<Utc>>,
    /// When it was created.
    pub created_at: DateTime<Utc>,
}

impl Credential {
    /// Whether this credential may still be used to authenticate.
    ///
    /// Expiry is evaluated here rather than trusted from `state`, because a
    /// credential expires by the passage of time, not by anyone running a
    /// sweep. A row can sit at `active` past its expiry and must still be
    /// refused (briefing §20).
    #[must_use]
    pub fn is_usable(&self, now: DateTime<Utc>) -> bool {
        self.state == CredentialState::Active && self.expires_at.is_none_or(|expiry| expiry > now)
    }

    /// Whether this credential has passed its expiry.
    #[must_use]
    pub fn has_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|expiry| expiry <= now)
    }
}

impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for Credential {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let kind: String = row.try_get("kind")?;
        let state: String = row.try_get("state")?;
        Ok(Self {
            id: row.try_get("id")?,
            person_id: row.try_get("person_id")?,
            // A row whose vocabulary this build does not know is not silently
            // downgraded to something permissive: `Revoked` is the safe reading.
            kind: CredentialKind::parse(&kind).unwrap_or(CredentialKind::Temporary),
            state: CredentialState::parse(&state).unwrap_or(CredentialState::Revoked),
            verifier: row.try_get("verifier")?,
            expires_at: row.try_get("expires_at")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

/// Every live credential for a person, in one query.
///
/// The sign-in path needs both kinds and must not do two round trips for them.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn live_credentials<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<Vec<Credential>> {
    let credentials = sqlx::query_as::<_, Credential>(
        "SELECT id, person_id, kind, state, verifier, expires_at, created_at
           FROM credentials
          WHERE person_id = $1 AND state = 'active'",
    )
    .bind(person_id)
    .fetch_all(executor)
    .await?;
    Ok(credentials)
}

/// Retire every live credential of a kind, giving a reason.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn revoke_live<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    kind: CredentialKind,
) -> CoreResult<u64> {
    let result = sqlx::query(
        "UPDATE credentials
            SET state = 'revoked', revoked_at = now(), updated_at = now()
          WHERE person_id = $1 AND kind = $2 AND state = 'active'",
    )
    .bind(person_id)
    .bind(kind.as_str())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Store a new credential.
///
/// The caller is responsible for having revoked any live credential of the same
/// kind first; the unique index enforces it either way.
///
/// # Errors
///
/// Returns an error when the insert fails, including on a live-credential clash.
pub async fn insert<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    kind: CredentialKind,
    verifier: &str,
    expires_at: Option<DateTime<Utc>>,
    issued_by: Option<Uuid>,
    reason: &str,
) -> CoreResult<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO credentials
             (person_id, kind, state, verifier, expires_at, issued_by_id, issued_reason)
         VALUES ($1, $2, 'active', $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(person_id)
    .bind(kind.as_str())
    .bind(verifier)
    .bind(expires_at)
    .bind(issued_by)
    .bind(reason)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Mark a temporary credential as consumed.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn consume<'e>(executor: impl PgExecutor<'e>, credential_id: Uuid) -> CoreResult<()> {
    sqlx::query(
        "UPDATE credentials
            SET state = 'consumed', consumed_at = now(), updated_at = now()
          WHERE id = $1 AND state = 'active'",
    )
    .bind(credential_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// Replace a verifier in place, keeping the credential's identity.
///
/// Used only for transparent rehashing after a successful verification: the
/// password did not change, only the cost parameters it is stored under.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn replace_verifier<'e>(
    executor: impl PgExecutor<'e>,
    credential_id: Uuid,
    verifier: &str,
) -> CoreResult<()> {
    sqlx::query("UPDATE credentials SET verifier = $2, updated_at = now() WHERE id = $1")
        .bind(credential_id)
        .bind(verifier)
        .execute(executor)
        .await?;
    Ok(())
}

/// Mark every credential past its expiry as expired.
///
/// Housekeeping only. Authentication never depends on this having run —
/// [`Credential::is_usable`] evaluates expiry directly.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn sweep_expired<'e>(executor: impl PgExecutor<'e>) -> CoreResult<u64> {
    let result = sqlx::query(
        "UPDATE credentials
            SET state = 'expired', updated_at = now()
          WHERE state = 'active' AND expires_at IS NOT NULL AND expires_at <= now()",
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

// ── Sessions ────────────────────────────────────────────────────────────

/// Hash a session token for storage.
///
/// SHA-256 and not Argon2: a session token is 256 bits of CSPRNG output, so it
/// has no guessable structure for a slow hash to protect. What matters is that
/// the database never holds the token itself.
#[must_use]
pub fn session_digest(token: &Secret) -> String {
    let digest = Sha256::digest(token.expose().as_bytes());
    hex::encode(digest)
}

/// A live session.
#[derive(Debug, Clone)]
pub struct StoredSession {
    /// Identifier.
    pub id: Uuid,
    /// Whose session it is.
    pub person_id: Uuid,
    /// What it may be used for.
    pub state: SessionState,
    /// When it expires.
    pub expires_at: DateTime<Utc>,
    /// When it was issued.
    pub issued_at: DateTime<Utc>,
    /// Last request seen on it.
    pub last_seen_at: DateTime<Utc>,
    /// Coarse client description.
    pub user_agent: Option<String>,
    /// Network prefix of the client.
    pub ip_prefix: Option<String>,
}

impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for StoredSession {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let state: String = row.try_get("state")?;
        Ok(Self {
            id: row.try_get("id")?,
            person_id: row.try_get("person_id")?,
            // An unrecognised state must not become `Active`. Falling back to
            // the restricted state fails closed.
            state: SessionState::parse(&state).unwrap_or(SessionState::PasswordChangeRequired),
            expires_at: row.try_get("expires_at")?,
            issued_at: row.try_get("issued_at")?,
            last_seen_at: row.try_get("last_seen_at")?,
            user_agent: row.try_get("user_agent")?,
            ip_prefix: row.try_get("ip_prefix")?,
        })
    }
}

/// Create a session and return its opaque token.
///
/// The token is 256 bits from the operating-system CSPRNG. Only its digest is
/// stored, so this is the one and only moment it exists.
///
/// # Errors
///
/// Returns an error when the insert fails.
pub async fn create_session<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    state: SessionState,
    lifetime: Duration,
    user_agent: Option<&str>,
    ip_prefix: Option<&str>,
) -> CoreResult<(Uuid, Secret)> {
    use rand::rngs::SysRng;
    use rand::TryRng;

    let mut bytes = [0_u8; 32];
    // Aqui há canal de erro, portanto a falta de entropia propaga-se em vez de
    // entrar em pânico. O que não muda é a decisão: sem entropia não se emite
    // um token de sessão.
    SysRng.try_fill_bytes(&mut bytes).map_err(|_| {
        crate::error::CoreError::Internal("system entropy is unavailable".to_owned())
    })?;
    let token = Secret::new(hex::encode(bytes));

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions
             (person_id, token_digest, state, expires_at, user_agent, ip_prefix)
         VALUES ($1, $2, $3, now() + $4, $5, $6)
         RETURNING id",
    )
    .bind(person_id)
    .bind(session_digest(&token))
    .bind(state.as_str())
    .bind(lifetime)
    .bind(user_agent.map(|value| value.chars().take(255).collect::<String>()))
    .bind(ip_prefix)
    .fetch_one(executor)
    .await?;

    Ok((id, token))
}

/// Look up a session by its token, if it is live and unexpired.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn find_session<'e>(
    executor: impl PgExecutor<'e>,
    token: &Secret,
) -> CoreResult<Option<StoredSession>> {
    let session = sqlx::query_as::<_, StoredSession>(
        "SELECT id, person_id, state, expires_at, issued_at, last_seen_at,
                user_agent, ip_prefix
           FROM sessions
          WHERE token_digest = $1 AND state <> 'revoked' AND expires_at > now()",
    )
    .bind(session_digest(token))
    .fetch_optional(executor)
    .await?;
    Ok(session)
}

/// Sessions a person currently holds, newest first.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_sessions<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<Vec<StoredSession>> {
    let sessions = sqlx::query_as::<_, StoredSession>(
        "SELECT id, person_id, state, expires_at, issued_at, last_seen_at,
                user_agent, ip_prefix
           FROM sessions
          WHERE person_id = $1 AND state <> 'revoked' AND expires_at > now()
          ORDER BY issued_at DESC",
    )
    .bind(person_id)
    .fetch_all(executor)
    .await?;
    Ok(sessions)
}

/// Record activity on a session.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn touch_session<'e>(executor: impl PgExecutor<'e>, session_id: Uuid) -> CoreResult<()> {
    sqlx::query("UPDATE sessions SET last_seen_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// Revoke one session.
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn revoke_session<'e>(
    executor: impl PgExecutor<'e>,
    session_id: Uuid,
    reason: &str,
) -> CoreResult<()> {
    sqlx::query(
        "UPDATE sessions
            SET state = 'revoked', revoked_at = now(), revoked_reason = $2
          WHERE id = $1 AND state <> 'revoked'",
    )
    .bind(session_id)
    .bind(reason)
    .execute(executor)
    .await?;
    Ok(())
}

/// Revoke every session a person holds.
///
/// Called on suspension, disabling and password reset — the three events after
/// which an existing session must not survive (briefing §90).
///
/// # Errors
///
/// Returns an error when the statement fails.
pub async fn revoke_all_sessions<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
    reason: &str,
) -> CoreResult<u64> {
    let result = sqlx::query(
        "UPDATE sessions
            SET state = 'revoked', revoked_at = now(), revoked_reason = $2
          WHERE person_id = $1 AND state <> 'revoked'",
    )
    .bind(person_id)
    .bind(reason)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

// ── Explicit access grants ──────────────────────────────────────────────

/// Live grants for a person.
///
/// "Live" is decided here, in SQL, so that the pure policy layer never has to
/// ask what time it is: everything this returns is already unrevoked and
/// unexpired.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn live_grants<'e>(
    executor: impl PgExecutor<'e>,
    person_id: Uuid,
) -> CoreResult<Vec<ExplicitGrant>> {
    let rows = sqlx::query(
        "SELECT permission, scope, scope_id
           FROM explicit_access_grants
          WHERE subject_id = $1
            AND revoked_at IS NULL
            AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(person_id)
    .fetch_all(executor)
    .await?;

    let mut grants = Vec::with_capacity(rows.len());
    for row in rows {
        let permission: String = row.try_get("permission")?;
        let scope: String = row.try_get("scope")?;

        // A grant naming a permission or scope this build does not know is
        // dropped, not guessed at. Failing closed on unknown vocabulary is the
        // whole point of parsing rather than string-matching later.
        let (Some(permission), Some(scope)) =
            (Permission::parse(&permission), Scope::parse(&scope))
        else {
            tracing::warn!(
                permission = %permission,
                scope = %scope,
                "grant names vocabulary this build does not know; ignoring it"
            );
            continue;
        };

        grants.push(ExplicitGrant {
            permission,
            scope,
            scope_id: row.try_get("scope_id")?,
        });
    }
    Ok(grants)
}

/// Last successful sign-in and recent failure count for one account.
///
/// Feeds the security tab. Returns counts and timestamps only — never the
/// credentials that were tried (briefing §38, §73).
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn attempt_summary<'e>(
    executor: impl PgExecutor<'e> + Copy,
    username: &str,
) -> CoreResult<(Option<DateTime<Utc>>, i64)> {
    let last: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT max(attempted_at) FROM authentication_attempts
          WHERE lower(username) = lower($1) AND outcome = 'succeeded'",
    )
    .bind(username)
    .fetch_one(executor)
    .await?;

    let failures: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM authentication_attempts
          WHERE lower(username) = lower($1)
            AND outcome <> 'succeeded'
            AND attempted_at > now() - interval '7 days'",
    )
    .bind(username)
    .fetch_one(executor)
    .await?;

    Ok((last, failures))
}

/// As sessões de quem pergunta, e só dela.
///
/// Não recebe identificador de pessoa: a lista sai do principal autenticado. Um
/// parâmetro por onde escolher outra conta seria, nesta superfície, a própria
/// vulnerabilidade.
///
/// # Errors
///
/// Propaga falhas da base de dados.
pub async fn list_own_sessions<'e>(
    executor: impl PgExecutor<'e>,
    principal: &ocinye_domain::Principal,
) -> CoreResult<Vec<StoredSession>> {
    list_sessions(executor, principal.person_id).await
}

/// Revoga uma sessão **do próprio**, identificada pelo cliente.
///
/// # Porque não chama directamente a primitiva
///
/// [`revoke_session`] recebe um identificador e revoga. É correcto enquanto o
/// identificador vier de dentro — a rota de terminar sessão passa o da sessão
/// autenticada, que nunca atravessou a rede.
///
/// Aqui o identificador **vem do cliente**, e um UUID não é autoridade. Sem a
/// verificação de posse abaixo, qualquer membro terminaria a sessão de qualquer
/// outro bastando conhecer-lhe o identificador — o mesmo padrão que produziu o
/// `SB1-FU-02` nos ambientes de investigação, agora na autenticação.
///
/// A resolução acontece **antes** da mutação, e a recusa é indistinguível de
/// «não existe»: dizer «existe mas não é sua» confirmaria a existência de
/// sessões alheias a quem tentasse adivinhar.
///
/// # Errors
///
/// [`CoreError::NotFound`] quando a sessão não existe ou não pertence a quem
/// pede. As duas são deliberadamente a mesma resposta.
pub async fn revoke_own_session<'e>(
    executor: impl PgExecutor<'e> + Copy,
    principal: &ocinye_domain::Principal,
    session_id: Uuid,
    reason: &str,
) -> CoreResult<()> {
    let dono: Option<Uuid> = sqlx::query_scalar("SELECT person_id FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(executor)
        .await?;

    if dono != Some(principal.person_id) {
        return Err(crate::error::CoreError::NotFound(
            "Esta sessão não existe, ou não lhe pertence.".to_owned(),
        ));
    }

    revoke_session(executor, session_id, reason).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credential(state: CredentialState, expires_in: Option<i64>) -> Credential {
        let now = Utc::now();
        Credential {
            id: Uuid::from_u128(1),
            person_id: Uuid::from_u128(2),
            kind: CredentialKind::Temporary,
            state,
            verifier: "$argon2id$v=19$m=19456,t=2,p=1$abc$def".to_owned(),
            expires_at: expires_in.map(|hours| now + Duration::hours(hours)),
            created_at: now,
        }
    }

    #[test]
    fn an_expired_credential_is_unusable_even_while_the_row_says_active() {
        // Nothing sweeps rows in real time. Expiry must be evaluated, not read.
        let stale = credential(CredentialState::Active, Some(-1));
        assert!(!stale.is_usable(Utc::now()));
        assert!(stale.has_expired(Utc::now()));
    }

    #[test]
    fn a_live_temporary_credential_is_usable() {
        let live = credential(CredentialState::Active, Some(1));
        assert!(live.is_usable(Utc::now()));
        assert!(!live.has_expired(Utc::now()));
    }

    #[test]
    fn a_consumed_or_revoked_credential_is_never_usable() {
        for state in [
            CredentialState::Consumed,
            CredentialState::Revoked,
            CredentialState::Expired,
        ] {
            assert!(
                !credential(state, Some(24)).is_usable(Utc::now()),
                "{state:?} was usable"
            );
        }
    }

    #[test]
    fn a_permanent_credential_without_expiry_stays_usable() {
        let mut permanent = credential(CredentialState::Active, None);
        permanent.kind = CredentialKind::Permanent;
        assert!(permanent.is_usable(Utc::now()));
        assert!(!permanent.has_expired(Utc::now()));
    }

    #[test]
    fn the_session_digest_is_stable_and_hides_the_token() {
        let token = Secret::new("a4f9c1e2b7d8");
        let digest = session_digest(&token);
        assert_eq!(digest, session_digest(&Secret::new("a4f9c1e2b7d8")));
        assert_eq!(digest.len(), 64);
        assert!(!digest.contains("a4f9c1e2b7d8"));
        assert_ne!(digest, session_digest(&Secret::new("a4f9c1e2b7d9")));
    }
}
