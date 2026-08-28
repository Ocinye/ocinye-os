//! Identity application layer.

use chrono::{Duration, Utc};
use ocinye_contracts::{InstitutionalPosition, PageRequest, TechnicalRole};
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use rand::rngs::SysRng;
use rand::TryRng;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use super::model::{Invitation, Person};
use super::repository as repo;
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::Tx;

/// How long an invitation stays valid.
pub const INVITATION_TTL_HOURS: i64 = 72;
const INVITATION_TOKEN_BYTES: usize = 32;

/// Assemble the acting principal from a verified subject.
///
/// Roles and memberships come from the database, never from token claims: they
/// are institutional facts, not assertions a client could influence (ADR-0102).
///
/// On first sign-in an invited person is bound to their verified subject, which
/// is the moment an invitation becomes access.
///
/// # Errors
///
/// Returns [`CoreError::Unauthenticated`] when the subject belongs to no member
/// of the institution.
pub async fn load_principal(
    pool: &PgPool,
    subject: &str,
    email: Option<&str>,
    display_name: &str,
    ids: &CorrelationIds,
) -> CoreResult<Principal> {
    let mut person = repo::find_by_subject(pool, subject).await?;

    if person.is_none() {
        if let Some(email) = email {
            if let Some(candidate) = repo::find_unbound_by_email(pool, email).await? {
                let mut tx = pool.begin().await?;
                repo::bind_subject(&mut *tx, candidate.id, subject).await?;
                audit::record(
                    &mut tx,
                    None,
                    ids,
                    AuditEntry::new(action::SIGN_IN, "person")
                        .resource(candidate.id)
                        .detail("event", "identity_bound"),
                )
                .await?;
                tx.commit().await?;
                person = repo::find_by_subject(pool, subject).await?;
            }
        }
    }

    let person = person.ok_or_else(|| {
        CoreError::Unauthenticated("This identity is not a member of the institution.".to_owned())
    })?;

    let roles = repo::live_roles(pool, person.id).await?;
    let unit_roles = repo::live_unit_roles(pool, person.id).await?;
    let workspace_roles = repo::live_workspace_roles(pool, person.id).await?;
    let grants = super::credentials::live_grants(pool, person.id).await?;

    // Best-effort: failing to record last activity must not deny a sign-in.
    if let Err(error) = repo::touch_last_seen(pool, person.id).await {
        tracing::warn!(error = %error, "could not record last activity");
    }

    Ok(Principal {
        subject: subject.to_owned(),
        person_id: person.id,
        organisation_id: person.organisation_id,
        // The person record is institutional truth and wins. The provider's
        // name is a fallback for a record that carries neither a display name
        // nor a full name.
        display_name: if person.preferred_name().is_empty() {
            display_name.to_owned()
        } else {
            person.preferred_name().to_owned()
        },
        is_active: person.can_act(),
        roles: roles.into_iter().collect(),
        unit_roles: unit_roles.into_iter().collect(),
        workspace_roles: workspace_roles.into_iter().collect(),
        grants,
    })
}

/// The caller's own person record.
///
/// # Why this is not `get_person(principal.person_id)`
///
/// `get_person` asks the policy whether the caller may read *people* — an
/// organisation-scope permission that an external collaborator does not hold.
/// Routed through it, such a member would be refused their own name, and the
/// account screen would tell them they lack permission to see themselves.
///
/// Identity is not a permission. Being authenticated already means the Core
/// resolved this person; returning what it resolved discloses nothing that the
/// session did not already establish. The read is pinned to
/// `principal.person_id` and to the principal's organisation, so it cannot be
/// pointed at anyone else: there is no parameter to point.
///
/// It stays a read of the member's own record and never becomes a side door
/// into the directory (`CLAUDE.md` §4).
///
/// # Errors
///
/// Returns an error when the person row is missing or the query fails.
pub async fn get_own_person(pool: &PgPool, principal: &Principal) -> CoreResult<Person> {
    repo::find_by_id(pool, principal.person_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Person not found.".to_owned()))
}

/// Load a person within the caller's organisation.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the person does not exist or belongs to
/// another organisation.
pub async fn get_person(
    pool: &PgPool,
    principal: &Principal,
    person_id: Uuid,
) -> CoreResult<Person> {
    let ctx = ResourceContext::organisation(ResourceKind::Person, principal.organisation_id);
    authorize(principal, Action::Read, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    repo::find_by_id(pool, person_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Person not found.".to_owned()))
}

/// List the people of the organisation.
///
/// # Errors
///
/// Returns an error when the caller may not read, or the query fails.
pub async fn list_people(
    pool: &PgPool,
    principal: &Principal,
    page: PageRequest,
) -> CoreResult<(Vec<Person>, i64)> {
    let ctx = ResourceContext::organisation(ResourceKind::Person, principal.organisation_id);
    authorize(principal, Action::Read, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let people = repo::list(pool, principal.organisation_id, page.limit(), page.offset()).await?;
    let total = repo::count(pool, principal.organisation_id).await?;
    Ok((people, total))
}

/// Details of a new invitation.
#[derive(Debug, Clone)]
pub struct NewInvitation {
    /// Email of the person being invited.
    pub email: String,
    /// Their full name.
    pub full_name: String,
    /// Institutional position offered. Grants nothing.
    pub institutional_position: Option<InstitutionalPosition>,
}

/// An invitation, together with the one-time token.
///
/// The plaintext token exists only in this value. Only its digest is persisted,
/// so it cannot be recovered from the database or from a backup.
#[derive(Debug, Clone)]
pub struct IssuedInvitation {
    /// The stored invitation.
    pub invitation: Invitation,
    /// The plaintext token. Shown once, never stored, never logged.
    pub token: String,
}

fn new_token() -> (String, String) {
    let mut bytes = [0_u8; INVITATION_TOKEN_BYTES];
    // Entropia do sistema: um token de convite é uma credencial, e um
    // gerador reproduzível seria uma vulnerabilidade.
    //
    // O `SysRng` é falível — o sistema pode não ter entropia para dar — e a
    // falha é deliberadamente ruidosa. Um servidor que não consegue entropia
    // não pode emitir uma credencial; qualquer alternativa mais silenciosa
    // seria exactamente a vulnerabilidade que a linha acima recusa.
    SysRng
        .try_fill_bytes(&mut bytes)
        .expect("o sistema não deu entropia para um token de convite");
    let token = hex::encode(bytes);
    let digest = hex::encode(Sha256::digest(token.as_bytes()));
    (token, digest)
}

/// Digest of an invitation token, for lookup.
#[must_use]
pub fn digest_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Invite someone to the institution.
///
/// # Errors
///
/// Returns an error when the caller may not invite, or the email is taken.
pub async fn create_invitation(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    request: NewInvitation,
) -> CoreResult<IssuedInvitation> {
    let ctx = ResourceContext::organisation(ResourceKind::Person, principal.organisation_id);
    authorize(principal, Action::ManageMembers, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let email = request.email.trim().to_ascii_lowercase();
    if !email.contains('@') || email.len() > 320 {
        return Err(CoreError::Validation(
            "A valid email address is required.".to_owned(),
        ));
    }
    if repo::email_taken(&mut **tx, &email).await? {
        return Err(CoreError::Conflict(
            "A person with this email already exists.".to_owned(),
        ));
    }

    let (token, digest) = new_token();
    let invitation = repo::insert_invitation(
        &mut **tx,
        principal.organisation_id,
        &email,
        request.full_name.trim(),
        request
            .institutional_position
            .map(InstitutionalPosition::as_str),
        &digest,
        Utc::now() + Duration::hours(INVITATION_TTL_HOURS),
        principal.person_id,
    )
    .await?;

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::INVITE, "invitation")
            .resource(invitation.id)
            // The address itself is personal data; the domain is enough to
            // review whether invitations are going where they should.
            .detail("email_domain", email.rsplit('@').next().unwrap_or_default()),
    )
    .await?;

    Ok(IssuedInvitation { invitation, token })
}

/// Accept an invitation, creating the person shell.
///
/// Unauthenticated by design: the token is the proof. The person created cannot
/// yet act — access begins at first verified sign-in.
///
/// # Errors
///
/// Returns [`CoreError::NotFound`] when the token does not match a pending,
/// unexpired invitation. Expired and unknown tokens are indistinguishable, so
/// the endpoint is not an oracle.
pub async fn accept_invitation(
    tx: &mut Tx<'_>,
    ids: &CorrelationIds,
    token: &str,
) -> CoreResult<Person> {
    let invalid = || CoreError::NotFound("This invitation is not valid.".to_owned());

    let invitation = repo::find_invitation_by_digest(&mut **tx, &digest_token(token))
        .await?
        .ok_or_else(invalid)?;

    if invitation.status != "pending" {
        return Err(invalid());
    }
    if invitation.expires_at < Utc::now() {
        repo::mark_invitation_expired(&mut **tx, invitation.id).await?;
        return Err(invalid());
    }

    let person = repo::insert_person_from_invitation(&mut **tx, &invitation).await?;
    repo::mark_invitation_accepted(&mut **tx, invitation.id, person.id).await?;

    audit::record(
        tx,
        None,
        ids,
        AuditEntry::new(action::MEMBERSHIP_CHANGE, "person")
            .resource(person.id)
            .detail("event", "invitation_accepted"),
    )
    .await?;

    Ok(person)
}

/// Grant a technical role.
///
/// # Errors
///
/// Returns an error when the caller may not administer, or the person is absent.
pub async fn grant_role(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    person_id: Uuid,
    role: TechnicalRole,
    reason: &str,
) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Platform, principal.organisation_id);
    authorize(principal, Action::Administer, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    if reason.trim().is_empty() {
        return Err(CoreError::Validation(
            "Granting a technical role requires a reason.".to_owned(),
        ));
    }

    let person = repo::find_by_id(&mut **tx, person_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Person not found.".to_owned()))?;

    let granted = repo::grant_role(
        &mut **tx,
        person.id,
        role,
        reason.trim(),
        Some(principal.person_id),
    )
    .await?;

    if granted {
        audit::record(
            tx,
            Some(principal),
            ids,
            AuditEntry::new(action::ROLE_CHANGE, "person")
                .resource(person.id)
                .detail("granted_role", role.as_str())
                .detail(
                    "reason",
                    reason.trim().chars().take(200).collect::<String>(),
                ),
        )
        .await?;
    }
    Ok(())
}

/// Revoke a technical role.
///
/// # Errors
///
/// Returns an error when the caller may not administer, or the role is not live.
pub async fn revoke_role(
    tx: &mut Tx<'_>,
    principal: &Principal,
    ids: &CorrelationIds,
    person_id: Uuid,
    role: TechnicalRole,
) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Platform, principal.organisation_id);
    authorize(principal, Action::Administer, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let person = repo::find_by_id(&mut **tx, person_id, principal.organisation_id)
        .await?
        .ok_or_else(|| CoreError::NotFound("Person not found.".to_owned()))?;

    if !repo::revoke_role(&mut **tx, person.id, role).await? {
        return Err(CoreError::NotFound("This role is not granted.".to_owned()));
    }

    audit::record(
        tx,
        Some(principal),
        ids,
        AuditEntry::new(action::ROLE_CHANGE, "person")
            .resource(person.id)
            .detail("revoked_role", role.as_str()),
    )
    .await?;
    Ok(())
}

/// Assemble a principal from a person already loaded and authenticated.
///
/// The counterpart of [`load_principal`] for the session-based path: the
/// identity is already established, so this only gathers the institutional
/// facts that decide what the person may do.
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn principal_for_person(pool: &PgPool, person: &Person) -> CoreResult<Principal> {
    let roles = repo::live_roles(pool, person.id).await?;
    let unit_roles = repo::live_unit_roles(pool, person.id).await?;
    let workspace_roles = repo::live_workspace_roles(pool, person.id).await?;
    let grants = super::credentials::live_grants(pool, person.id).await?;

    if let Err(error) = repo::touch_last_seen(pool, person.id).await {
        tracing::warn!(error = %error, "could not record last activity");
    }

    Ok(Principal {
        // Vestigial under ADR-0103: kept so the audit trail and any future
        // federation keep a stable slot for it.
        subject: person.oidc_subject.clone().unwrap_or_default(),
        person_id: person.id,
        organisation_id: person.organisation_id,
        display_name: person.preferred_name().to_owned(),
        is_active: person.can_act(),
        roles: roles.into_iter().collect(),
        unit_roles: unit_roles.into_iter().collect(),
        workspace_roles: workspace_roles.into_iter().collect(),
        grants,
    })
}

/// Load a person by identifier, without authorization.
///
/// Used by the session extractor, which has already established *who* the
/// caller is and is deciding whether they may act at all. Every caller-facing
/// lookup goes through [`get_person`], which authorises.
///
/// # Errors
///
/// Build a principal for somebody else, inside the caller's transaction.
///
/// # Why this exists separately from [`principal_for_person`]
///
/// That one answers «who is acting», reads from the pool, and records activity
/// as a side effect. This one answers a different question — «what could *this
/// other person* do here» — and has to answer it inside the transaction that is
/// about to write, so the decision cannot be taken against a membership that
/// changed in between.
///
/// It records no activity: the subject is not acting, they are being asked
/// about, and marking them as seen would be a small lie in the audit trail.
///
/// # Errors
///
/// Returns an error when a query fails.
pub async fn principal_within(tx: &mut Tx<'_>, person_id: Uuid) -> CoreResult<Option<Principal>> {
    let Some(person) = repo::find_by_id_unscoped(&mut **tx, person_id).await? else {
        return Ok(None);
    };

    let roles = repo::live_roles(&mut **tx, person.id).await?;
    let unit_roles = repo::live_unit_roles(&mut **tx, person.id).await?;
    let workspace_roles = repo::live_workspace_roles(&mut **tx, person.id).await?;
    let grants = super::credentials::live_grants(&mut **tx, person.id).await?;

    Ok(Some(Principal {
        subject: person.oidc_subject.clone().unwrap_or_default(),
        person_id: person.id,
        organisation_id: person.organisation_id,
        display_name: person.preferred_name().to_owned(),
        is_active: person.can_act(),
        roles: roles.into_iter().collect(),
        unit_roles: unit_roles.into_iter().collect(),
        workspace_roles: workspace_roles.into_iter().collect(),
        grants,
    }))
}

/// Returns an error when the query fails.
pub async fn person_by_id(pool: &PgPool, person_id: Uuid) -> CoreResult<Option<Person>> {
    repo::find_by_id_unscoped(pool, person_id).await
}

/// Live credentials for a person, for the security overview.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn live_credentials_for(
    pool: &PgPool,
    person_id: Uuid,
) -> CoreResult<Vec<super::credentials::Credential>> {
    super::credentials::live_credentials(pool, person_id).await
}

/// Sign-in history summary for a person.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn attempt_summary(
    pool: &PgPool,
    person: &Person,
) -> CoreResult<(Option<chrono::DateTime<chrono::Utc>>, i64)> {
    let Some(username) = person.username.as_deref() else {
        return Ok((None, 0));
    };
    super::credentials::attempt_summary(pool, username).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_tokens_are_unpredictable_and_only_their_digest_is_stored() {
        let (token_a, digest_a) = new_token();
        let (token_b, _) = new_token();

        assert_ne!(token_a, token_b);
        assert_eq!(token_a.len(), INVITATION_TOKEN_BYTES * 2);
        assert_eq!(digest_a.len(), 64);
        assert_ne!(digest_a, token_a);
        assert_eq!(digest_token(&token_a), digest_a);
    }
}
