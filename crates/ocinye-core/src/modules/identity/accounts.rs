//! Account lifecycle: creation, password change, reset, suspension, bootstrap.
//!
//! Every function that produces a temporary credential returns it exactly once,
//! in a [`TemporaryCredential`], and stores only its verifier. There is no
//! function anywhere in the Core that reads a password back — not for an
//! administrator, not for the bootstrap administrator, not for anyone
//! (briefing §19).

use chrono::{DateTime, Duration, Utc};
use ocinye_contracts::{
    AccountStatus, CredentialKind, InstitutionalPosition, SessionState, TechnicalRole,
};
use ocinye_observability::CorrelationIds;
use sqlx::PgPool;
use uuid::Uuid;

use super::authentication::{Authenticator, IssuedSession, SESSION_LIFETIME_HOURS};
use super::model::Person;
use super::{credentials as creds, repository as repo};
use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use crate::password::{generate, policy, Secret};
use ocinye_domain::Principal;

/// Default validity of a temporary credential, in hours.
///
/// Configurable through `OCINYE_TEMPORARY_CREDENTIAL_HOURS`. Twenty-four is the
/// briefing's recommendation (§20): long enough to reach someone across a
/// working day, short enough that a credential read over the phone and written
/// on paper does not stay valid for a week.
pub const DEFAULT_TEMPORARY_CREDENTIAL_HOURS: i64 = 24;

/// A temporary credential, in the one moment it exists in the clear.
///
/// Deliberately not `Clone` and not `Serialize`: it is moved into exactly one
/// response body and dropped. Everything else about it lives as a verifier.
#[derive(Debug)]
pub struct TemporaryCredential {
    /// The credential. Shown once, never recoverable (briefing §18).
    pub secret: Secret,
    /// O endereço a que pertence.
    pub email: String,
    /// When it stops working.
    pub expires_at: DateTime<Utc>,
}

/// What an administrator supplies to create a member.
#[derive(Debug, Clone)]
pub struct NewMember {
    /// Full name.
    pub full_name: String,
    /// O endereço institucional. É a identidade e a credencial (ADR-0106).
    pub email: String,
    /// Institutional position. Grants nothing (ADR-0100).
    pub position: Option<InstitutionalPosition>,
    /// Initial technical role.
    pub role: TechnicalRole,
    /// Unit to place them in, if any.
    pub unit_id: Option<Uuid>,
}

/// Valida um endereço institucional.
///
/// # Porque validar aqui e não só na base
///
/// Para que quem escreve mal receba uma frase que se lê, e não uma violação de
/// restrição. A validação é deliberadamente frouxa: um endereço é o que o
/// servidor de correio aceitar, e um analisador rigoroso de RFC recusaria
/// endereços legítimos que existem por aí.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] quando o endereço não é aceitável.
pub fn validate_email(email: &str) -> CoreResult<String> {
    let trimmed = email.trim().to_lowercase();

    if trimmed.chars().count() < 5 || trimmed.chars().count() > 320 {
        return Err(CoreError::Validation(
            "O endereço deve ter entre 5 e 320 caracteres.".to_owned(),
        ));
    }

    // Uma arroba, e uma só: `a@b@c` não é um endereço, e um `@` sozinho
    // também não.
    let partes: Vec<&str> = trimmed.split('@').collect();
    if partes.len() != 2 || partes[0].is_empty() || partes[1].is_empty() {
        return Err(CoreError::Validation(
            "O endereço deve ter a forma «nome@dominio».".to_owned(),
        ));
    }

    // O domínio precisa de um ponto: `alguem@localhost` não é um endereço
    // institucional.
    if !partes[1].contains('.') || partes[1].starts_with('.') || partes[1].ends_with('.') {
        return Err(CoreError::Validation(
            "O domínio do endereço não é válido.".to_owned(),
        ));
    }

    if trimmed.chars().any(char::is_whitespace) {
        return Err(CoreError::Validation(
            "Um endereço não tem espaços.".to_owned(),
        ));
    }

    Ok(trimmed)
}

/// Create a member account and issue its temporary credential.
///
/// The account is created `invited` with a temporary credential and no
/// permanent one. It becomes `active` only when its holder sets their own
/// password, which is what makes the founding invariant true by construction.
///
/// # Errors
///
/// Returns [`CoreError::Validation`] for a bad address,
/// [`CoreError::Conflict`] when either is taken, and a database error otherwise.
pub async fn create_member(
    pool: &PgPool,
    authenticator: &Authenticator,
    actor: &Principal,
    new: &NewMember,
    ids: &CorrelationIds,
) -> CoreResult<(Person, TemporaryCredential)> {
    let email = validate_email(&new.email)?;

    if repo::email_taken_in(pool, actor.organisation_id, &email).await? {
        return Err(CoreError::Conflict(
            "Já existe uma conta com este endereço.".to_owned(),
        ));
    }
    if repo::email_taken(pool, &email).await? {
        return Err(CoreError::Conflict("Este email já está em uso.".to_owned()));
    }

    let secret = generate::temporary_credential();
    let verifier = authenticator.hasher.hash(&secret)?;
    let expires_at = Utc::now() + Duration::hours(authenticator.temporary_credential_hours);

    let mut tx = pool.begin().await?;

    let person = repo::insert_person(
        &mut *tx,
        actor.organisation_id,
        &email,
        new.full_name.trim(),
        new.position.map(InstitutionalPosition::as_str),
    )
    .await?;

    creds::insert(
        &mut *tx,
        person.id,
        CredentialKind::Temporary,
        &verifier,
        Some(expires_at),
        Some(actor.person_id),
        "member_created",
    )
    .await?;

    repo::grant_role(
        &mut *tx,
        person.id,
        new.role,
        "initial role at account creation",
        Some(actor.person_id),
    )
    .await?;

    if let Some(unit_id) = new.unit_id {
        repo::add_unit_membership(&mut *tx, unit_id, person.id, actor.person_id).await?;
    }

    audit::record(
        &mut tx,
        Some(actor),
        ids,
        AuditEntry::new(action::MEMBER_CREATED, "person")
            .resource(person.id)
            .detail("email", email.clone())
            .detail("role", new.role.as_str()),
    )
    .await?;

    audit::record(
        &mut tx,
        Some(actor),
        ids,
        AuditEntry::new(action::TEMPORARY_CREDENTIAL_ISSUED, "credential")
            .resource(person.id)
            .detail("reason", "member_created")
            .detail("expires_at", expires_at.to_rfc3339()),
    )
    .await?;

    tx.commit().await?;

    Ok((
        person,
        TemporaryCredential {
            secret,
            email: email.clone(),
            expires_at,
        },
    ))
}

/// Mudança voluntária de palavra-passe, por quem já está autenticado.
///
/// # Porque não reutiliza `set_permanent_password`
///
/// São dois fluxos com invariantes diferentes. Aquele serve o primeiro acesso:
/// a autoridade vem da credencial temporária que a pessoa acabou de usar, e não
/// existe palavra-passe actual para confirmar.
///
/// Este serve quem já trabalha no sistema, e a sessão aberta **não** é prova
/// suficiente. Uma sessão pode ter ficado aberta numa máquina emprestada; sem
/// pedir a palavra-passe actual, quem passasse por ali trocava a credencial e
/// ficava com a conta. É por isso que a confirmação é obrigatória aqui e não
/// existe lá.
///
/// # A conta é a da sessão
///
/// Não recebe identificador de pessoa. A conta sai do principal autenticado, e
/// não há parâmetro por onde escolher outra.
///
/// # Sessões
///
/// Segue a regra que já existe no sistema: mudar a palavra-passe **revoga todas
/// as sessões** e emite uma nova. Não foi inventada uma segunda semântica para
/// este caminho — duas regras de sessão para o mesmo efeito seria uma delas
/// estar errada em metade dos casos.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] quando a palavra-passe actual não confere —
/// a mesma mensagem de uma conta sem credencial activa, para não distinguir os
/// dois casos. [`CoreError::Validation`] quando a nova viola a política.
pub async fn change_own_password(
    pool: &PgPool,
    authenticator: &Authenticator,
    person: &Person,
    current: &Secret,
    candidate: &Secret,
    context: &super::authentication::AttemptContext,
    ids: &CorrelationIds,
) -> CoreResult<IssuedSession> {
    let now = Utc::now();
    let live = creds::live_credentials(pool, person.id).await?;

    let permanent = live
        .iter()
        .find(|c| c.kind == CredentialKind::Permanent && c.is_usable(now));

    // A confirmação é contra o verificador guardado. A ausência de credencial e
    // a palavra-passe errada dão a mesma resposta: distinguir as duas diria a
    // quem tentasse que a conta existe mas ainda não tem palavra-passe.
    let confere = permanent.is_some_and(|c| authenticator.hasher.verify(current, &c.verifier));
    if !confere {
        return Err(CoreError::PermissionDenied(
            "A palavra-passe actual não confere.".to_owned(),
        ));
    }

    set_permanent_password(pool, authenticator, person, candidate, context, ids).await
}

/// Set a permanent password, consuming the temporary credential.
///
/// This is the only path from a restricted session to an ordinary one. It ends
/// by revoking **every** session the person holds, including the one that
/// called it, and issuing a fresh one — the session identifier that existed
/// during the bootstrap step is never reused (briefing §29, §30).
///
/// # Errors
///
/// Returns [`CoreError::Validation`] when the candidate fails the policy, and
/// [`CoreError::PermissionDenied`] when there is no credential to replace.
pub async fn set_permanent_password(
    pool: &PgPool,
    authenticator: &Authenticator,
    person: &Person,
    candidate: &Secret,
    context: &super::authentication::AttemptContext,
    ids: &CorrelationIds,
) -> CoreResult<IssuedSession> {
    let now = Utc::now();
    let live = creds::live_credentials(pool, person.id).await?;

    let temporary = live
        .iter()
        .find(|c| c.kind == CredentialKind::Temporary && c.is_usable(now));
    let permanent = live
        .iter()
        .find(|c| c.kind == CredentialKind::Permanent && c.is_usable(now));

    // Reuse of the credential being replaced, and reuse of the current
    // password, are both checked against the *verifier* — this function never
    // receives one password in order to compare another (briefing §28).
    let accepted = policy::validate(
        candidate,
        |normalised| {
            temporary.is_some_and(|c| authenticator.hasher.verify(normalised, &c.verifier))
        },
        |normalised| {
            permanent.is_some_and(|c| authenticator.hasher.verify(normalised, &c.verifier))
        },
    )
    .map_err(|rejection| CoreError::Validation(rejection.message()))?;

    let verifier = authenticator.hasher.hash(&accepted)?;

    let mut tx = pool.begin().await?;

    // Order matters: revoke before inserting, or the unique live-credential
    // index refuses the new row.
    creds::revoke_live(&mut *tx, person.id, CredentialKind::Permanent).await?;
    if let Some(temporary) = temporary {
        creds::consume(&mut *tx, temporary.id).await?;
    }

    creds::insert(
        &mut *tx,
        person.id,
        CredentialKind::Permanent,
        &verifier,
        None,
        Some(person.id),
        "set_by_holder",
    )
    .await?;

    // An invited account becomes active at exactly this moment, and not before.
    if person.account_status() == AccountStatus::Invited {
        repo::set_status(&mut *tx, person.id, AccountStatus::Active).await?;
    }

    creds::revoke_all_sessions(&mut *tx, person.id, "password_changed").await?;

    let (session_id, token) = creds::create_session(
        &mut *tx,
        person.id,
        SessionState::Active,
        Duration::hours(SESSION_LIFETIME_HOURS),
        context.user_agent.as_deref(),
        context.ip_prefix.as_deref(),
    )
    .await?;

    audit::record(
        &mut tx,
        None,
        ids,
        AuditEntry::new(action::PASSWORD_SET, "credential")
            .resource(person.id)
            .actor(person.id, person.organisation_id)
            .detail("replaced_temporary", temporary.is_some())
            .detail("session_id", session_id.to_string()),
    )
    .await?;

    tx.commit().await?;

    Ok(IssuedSession {
        token,
        state: SessionState::Active,
        person_id: person.id,
        display_name: person.preferred_name().to_owned(),
    })
}

/// Reset a member's password, as an administrator.
///
/// Issues a new temporary credential, invalidates the permanent one and revokes
/// every session. The administrator never chooses the replacement, and never
/// sees the password it replaces (briefing §42, §43).
///
/// # Errors
///
/// Returns a database error, or [`CoreError::NotFound`] when the person is not
/// in the actor's organisation.
pub async fn reset_password(
    pool: &PgPool,
    authenticator: &Authenticator,
    actor: &Principal,
    person: &Person,
    ids: &CorrelationIds,
) -> CoreResult<TemporaryCredential> {
    let secret = generate::temporary_credential();
    let verifier = authenticator.hasher.hash(&secret)?;
    let expires_at = Utc::now() + Duration::hours(authenticator.temporary_credential_hours);

    let mut tx = pool.begin().await?;

    creds::revoke_live(&mut *tx, person.id, CredentialKind::Permanent).await?;
    creds::revoke_live(&mut *tx, person.id, CredentialKind::Temporary).await?;

    creds::insert(
        &mut *tx,
        person.id,
        CredentialKind::Temporary,
        &verifier,
        Some(expires_at),
        Some(actor.person_id),
        "administrative_reset",
    )
    .await?;

    let revoked = creds::revoke_all_sessions(&mut *tx, person.id, "password_reset").await?;

    audit::record(
        &mut tx,
        Some(actor),
        ids,
        AuditEntry::new(action::PASSWORD_RESET, "credential")
            .resource(person.id)
            .detail("sessions_revoked", revoked)
            .detail("expires_at", expires_at.to_rfc3339()),
    )
    .await?;

    tx.commit().await?;

    Ok(TemporaryCredential {
        secret,
        email: person.email.clone(),
        expires_at,
    })
}

/// Change an account's status.
///
/// Suspension and disabling both revoke every session immediately: an access
/// decision that only takes effect at the next sign-in is not a revocation
/// (briefing §40, §41, §90).
///
/// # Errors
///
/// Returns a database error.
pub async fn set_account_status(
    pool: &PgPool,
    actor: &Principal,
    person: &Person,
    status: AccountStatus,
    reason: &str,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    if person.id == actor.person_id && !status.may_authenticate() {
        // Locking yourself out is how an institution ends up with no
        // administrator and no way back in.
        return Err(CoreError::Validation(
            "Não pode suspender ou desactivar a sua própria conta.".to_owned(),
        ));
    }

    let mut tx = pool.begin().await?;
    repo::set_status(&mut *tx, person.id, status).await?;

    let revoked = if status.may_authenticate() {
        0
    } else {
        creds::revoke_all_sessions(&mut *tx, person.id, status.as_str()).await?
    };

    let entry = match status {
        AccountStatus::Suspended => action::ACCOUNT_SUSPENDED,
        AccountStatus::Disabled => action::ACCOUNT_DISABLED,
        AccountStatus::Active | AccountStatus::Invited => action::ACCOUNT_REINSTATED,
    };

    audit::record(
        &mut tx,
        Some(actor),
        ids,
        AuditEntry::new(entry, "person")
            .resource(person.id)
            .detail("status", status.as_str())
            .detail("reason", reason)
            .detail("sessions_revoked", revoked),
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Namespace of the advisory lock that serialises `bootstrap-admin`.
///
/// Arbitrary and stable. Paired with a hash of the organisation identifier, so
/// two organisations never contend with each other.
const BOOTSTRAP_LOCK_CLASS: i32 = 0x0C11_0001_u32 as i32;

/// Create the first platform administrator.
///
/// Refuses if a usable platform administrator already exists. That guard is the
/// whole of the one-shot protection: there is no token, no magic file and no
/// window during which the endpoint is open (briefing §12).
///
/// # One shot, including against itself
///
/// The guard is checked before the transaction and again inside it, under a
/// PostgreSQL advisory transaction lock. The lock is what makes the second
/// check mean anything: without it two concurrent runs both read "no
/// administrator" and both committed one.
///
/// Like every other account, the first administrator starts with a temporary
/// credential and must set their own password before doing anything. There is
/// no permanent bootstrap password (briefing §13).
///
/// # Errors
///
/// Returns [`CoreError::Conflict`] when a platform administrator already
/// exists, and [`CoreError::Validation`] for a bad address.
pub async fn bootstrap_platform_admin(
    pool: &PgPool,
    authenticator: &Authenticator,
    organisation_id: Uuid,
    full_name: &str,
    email: &str,
    ids: &CorrelationIds,
) -> CoreResult<(Person, TemporaryCredential)> {
    // A forma de uma identidade só: uma pessoa que também administra.
    //
    // Continua a existir porque é o que instalações sem separação precisam, e
    // porque seis provas a exercitam. **Não é** a forma da produção da Ocinye:
    // essa é `bootstrap_privileged_identity`, onde administrar deixa de ser a
    // identidade normal de alguém.
    bootstrap_uma_identidade(pool, authenticator, organisation_id, full_name, email, ids).await
}

/// A pessoa a quem a identidade privilegiada vai pertencer.
pub struct HumanOwner {
    /// O nome institucional da pessoa. Nunca o nome da identidade privilegiada.
    pub full_name: String,
    /// O endereço da pessoa, distinto do da identidade privilegiada.
    pub email: String,
}

/// A forma de uma identidade: uma pessoa que também administra.
///
/// Existe para instalações que não separam as duas coisas. A produção da Ocinye
/// usa `bootstrap_privileged_identity`, e a diferença não é de conveniência: com
/// uma identidade só, a conta de trabalho de alguém carrega `PlatformAdmin` em
/// todas as sessões — e uma sessão privilegiada indistinguível de uma sessão
/// normal é um estado que este sistema classifica como proibido.
///
/// # Errors
///
/// Devolve [`CoreError::Conflict`] quando já há administrador.
async fn bootstrap_uma_identidade(
    pool: &PgPool,
    authenticator: &Authenticator,
    organisation_id: Uuid,
    full_name: &str,
    email: &str,
    ids: &CorrelationIds,
) -> CoreResult<(Person, TemporaryCredential)> {
    if repo::has_usable_platform_admin(pool, organisation_id).await? {
        return Err(CoreError::Conflict(
            "A plataforma já tem um administrador. O bootstrap corre uma única vez.".to_owned(),
        ));
    }

    let email = validate_email(email)?;

    let secret = generate::temporary_credential();
    let verifier = authenticator.hasher.hash(&secret)?;
    let expires_at = Utc::now() + Duration::hours(authenticator.temporary_credential_hours);

    let mut tx = pool.begin().await?;

    // Serialise bootstrap attempts **in the database**.
    //
    // The re-check below is necessary and was not sufficient: a plain `SELECT`
    // under `READ COMMITTED` blocks nobody, so two concurrent runs both saw no
    // administrator, both inserted a person with a different address, and both
    // committed. Nothing in the schema forbids a second `platform_admin`, so
    // the installation ended up with two.
    //
    // An advisory transaction lock is the right instrument: it is held for
    // exactly this transaction, released on commit or rollback, needs no table
    // and no row to exist yet, and costs nothing on the path that runs once.
    // The second attempt now waits, then reads a committed administrator and
    // refuses (`CLAUDE.md` §31, briefing §64).
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2))")
        .bind(BOOTSTRAP_LOCK_CLASS)
        .bind(organisation_id.to_string())
        .execute(&mut *tx)
        .await?;

    // Re-check inside the transaction, now that the lock makes it meaningful.
    if repo::has_usable_platform_admin(&mut *tx, organisation_id).await? {
        return Err(CoreError::Conflict(
            "A plataforma já tem um administrador. O bootstrap corre uma única vez.".to_owned(),
        ));
    }

    let person = repo::insert_person(
        &mut *tx,
        organisation_id,
        &email,
        full_name.trim(),
        Some(InstitutionalPosition::Founder.as_str()),
    )
    .await?;

    creds::insert(
        &mut *tx,
        person.id,
        CredentialKind::Temporary,
        &verifier,
        Some(expires_at),
        None,
        "bootstrap",
    )
    .await?;

    repo::grant_role(
        &mut *tx,
        person.id,
        TechnicalRole::PlatformAdmin,
        "bootstrap: first platform administrator",
        None,
    )
    .await?;

    audit::record(
        &mut tx,
        None,
        ids,
        AuditEntry::new(action::BOOTSTRAP_ADMIN, "person")
            .resource(person.id)
            .actor(person.id, organisation_id)
            .detail("email", email.clone())
            .detail("expires_at", expires_at.to_rfc3339()),
    )
    .await?;

    tx.commit().await?;

    Ok((
        person,
        TemporaryCredential {
            secret,
            email: email.clone(),
            expires_at,
        },
    ))
}

/// Estabelece a pessoa e a identidade privilegiada por onde ela administra.
///
/// # Porque as duas nascem juntas
///
/// > **Uma identidade privilegiada ligada estabelece responsabilidade, e não
/// > herança de autoridade.**
///
/// Uma identidade privilegiada com credencial utilizável e sem dono válido é
/// uma conta com autoridade por quem ninguém responde. Nascerem na mesma
/// transacção é o que torna esse estado **impossível**, e não apenas
/// improvável: se qualquer metade falhar, nenhuma é escrita.
///
/// # O que atravessa a ligação
///
/// Nada. A pessoa não recebe credencial, não recebe papel, e não fica activa.
/// A identidade privilegiada não herda pertenças nem acesso científico. A seta
/// serve para a auditoria dizer quem está por trás de uma operação — e para
/// mais nada.
///
/// # Errors
///
/// Devolve [`CoreError::Conflict`] quando já há administrador, e
/// [`CoreError::Validation`] para um endereço inaceitável ou quando os dois
/// endereços são o mesmo.
pub async fn bootstrap_privileged_identity(
    pool: &PgPool,
    authenticator: &Authenticator,
    organisation_id: Uuid,
    dono: HumanOwner,
    full_name: &str,
    email: &str,
    ids: &CorrelationIds,
) -> CoreResult<(Person, TemporaryCredential)> {
    if repo::has_usable_platform_admin(pool, organisation_id).await? {
        return Err(CoreError::Conflict(
            "A plataforma já tem um administrador. O bootstrap corre uma única vez.".to_owned(),
        ));
    }

    let email = validate_email(email)?;

    let secret = generate::temporary_credential();
    let verifier = authenticator.hasher.hash(&secret)?;
    let expires_at = Utc::now() + Duration::hours(authenticator.temporary_credential_hours);

    let mut tx = pool.begin().await?;

    // Serialise bootstrap attempts **in the database**.
    //
    // The re-check below is necessary and was not sufficient: a plain `SELECT`
    // under `READ COMMITTED` blocks nobody, so two concurrent runs both saw no
    // administrator, both inserted a person with a different address, and both
    // committed. Nothing in the schema forbids a second `platform_admin`, so
    // the installation ended up with two.
    //
    // An advisory transaction lock is the right instrument: it is held for
    // exactly this transaction, released on commit or rollback, needs no table
    // and no row to exist yet, and costs nothing on the path that runs once.
    // The second attempt now waits, then reads a committed administrator and
    // refuses (`CLAUDE.md` §31, briefing §64).
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2))")
        .bind(BOOTSTRAP_LOCK_CLASS)
        .bind(organisation_id.to_string())
        .execute(&mut *tx)
        .await?;

    // Re-check inside the transaction, now that the lock makes it meaningful.
    if repo::has_usable_platform_admin(&mut *tx, organisation_id).await? {
        return Err(CoreError::Conflict(
            "A plataforma já tem um administrador. O bootstrap corre uma única vez.".to_owned(),
        ));
    }

    // ── A pessoa ────────────────────────────────────────────────────────
    //
    // Sem credencial, sem papel e sem estado activo. Existe porque a identidade
    // privilegiada precisa de um dono válido, e não para ser uma conta: dar-lhe
    // login aqui faria o servidor provisionar a instituição, que é exactamente
    // a fronteira que este bootstrap existe para não atravessar.
    let dono_email = validate_email(&dono.email)?;
    if dono_email == email {
        return Err(CoreError::Validation(
            "A pessoa e a identidade privilegiada não podem partilhar o mesmo \
             endereço: seriam a mesma credencial."
                .to_owned(),
        ));
    }
    let humano = repo::insert_person(
        &mut *tx,
        organisation_id,
        &dono_email,
        dono.full_name.trim(),
        Some(InstitutionalPosition::Founder.as_str()),
    )
    .await?;

    // ── A identidade privilegiada, ligada a ela ─────────────────────────
    let person = repo::insert_privileged_identity(
        &mut *tx,
        organisation_id,
        &email,
        full_name.trim(),
        humano.id,
    )
    .await?;

    creds::insert(
        &mut *tx,
        person.id,
        CredentialKind::Temporary,
        &verifier,
        Some(expires_at),
        // Nobody issued it: there was nobody above to issue it.
        None,
        "bootstrap",
    )
    .await?;

    repo::grant_role(
        &mut *tx,
        person.id,
        TechnicalRole::PlatformAdmin,
        "bootstrap: first platform administrator",
        None,
    )
    .await?;

    audit::record(
        &mut tx,
        None,
        ids,
        AuditEntry::new(action::BOOTSTRAP_ADMIN, "person")
            .resource(person.id)
            .actor(person.id, organisation_id)
            .detail("email", email.clone())
            // A pessoa por trás da identidade fica no registo desde o primeiro
            // acto: uma auditoria que só diga «Fidel Admin» perde quem responde.
            .detail("belongs_to_person_id", humano.id.to_string())
            .detail("belongs_to_email", dono_email.clone())
            .detail("expires_at", expires_at.to_rfc3339()),
    )
    .await?;

    tx.commit().await?;

    Ok((
        person,
        TemporaryCredential {
            secret,
            email: email.clone(),
            expires_at,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enderecos_aceitaveis_sao_aceites() {
        for email in [
            "fidel@ocinye.com",
            "fidel.monteiro@ocinye.com",
            "f+etiqueta@ocinye.com",
            "a@b.co",
            "nome-com-hifen@sub.dominio.pt",
        ] {
            assert!(validate_email(email).is_ok(), "{email:?} devia ser aceite");
        }
    }

    #[test]
    fn enderecos_inaceitaveis_sao_recusados() {
        for email in [
            "fidel",                 // sem arroba
            "@ocinye.com",           // sem nome
            "fidel@",                // sem domínio
            "a@b@c.com",             // duas arrobas
            "fidel@localhost",       // domínio sem ponto
            "fidel@.com",            // domínio a começar por ponto
            "fidel@ocinye.",         // domínio a acabar em ponto
            "fidel monteiro@oc.com", // espaço
            "a@b",                   // curto de mais e sem ponto
            "",
            "   ",
        ] {
            assert!(
                validate_email(email).is_err(),
                "{email:?} devia ser recusado"
            );
        }
    }

    #[test]
    fn um_endereco_e_normalizado_para_minusculas() {
        // Um endereço não distingue maiúsculas para efeitos de identidade, e
        // guardá-lo como foi escrito deixaria duas contas a poderem existir
        // com o mesmo nome visto por uma pessoa.
        assert_eq!(
            validate_email("  Fidel.Monteiro@Ocinye.COM  ").unwrap(),
            "fidel.monteiro@ocinye.com"
        );
    }

    #[test]
    fn a_temporary_credential_is_not_debug_printable() {
        let credential = TemporaryCredential {
            secret: Secret::new("abcd-efgh-ijkl"),
            email: "fidel@ocinye.com".into(),
            expires_at: Utc::now(),
        };
        let rendered = format!("{credential:?}");
        assert!(rendered.contains("fidel@ocinye.com"));
        assert!(
            !rendered.contains("abcd-efgh-ijkl"),
            "the credential leaked through Debug"
        );
    }

    #[test]
    fn the_default_temporary_validity_matches_the_documented_policy() {
        assert_eq!(DEFAULT_TEMPORARY_CREDENTIAL_HOURS, 24);
    }
}
