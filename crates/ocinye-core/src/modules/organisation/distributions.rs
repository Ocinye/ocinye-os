//! As Distribuições de uma Instância e o acesso de cada membro a elas (ADR-0019).
//!
//! # Três conjuntos que nunca se fundem
//!
//! - **disponíveis** — as quatro do produto (`Distribution::ALL`);
//! - **activadas** — `instance_distributions` com `state = 'enabled'` (1..4);
//! - **acessíveis** — activadas ∩ `member_distribution_access` do membro.
//!
//! # O que isto não é
//!
//! Autorização. Entrar numa Distribuição decide a experiência — predefinições,
//! fixações, fundo, primeiros passos — e nada do que o membro pode fazer lá
//! dentro: isso continua a ser do RBAC (ADR-0100). Não há `ROLE_RESEARCH`.
//!
//! # Invariantes (no Core, com guarda na base)
//!
//! - pelo menos uma Distribuição activada — desactivar a última é recusado;
//! - pelo menos um membro com `organisation.manage`, capaz de entrar, tem
//!   acesso a uma Distribuição activada — retirar o último é recusado, e
//!   desactivar a Distribuição que o deixaria de fora também.
//!
//! Desactivar guarda tudo: a linha, os acessos, as disposições, as fixações.
//! Nada aqui apaga estado de membro.

use ocinye_contracts::{
    Distribution, DistributionSet, EntryDecision, EntryResolution, ErrorCode, Permission,
    TechnicalRole,
};
use ocinye_domain::policy::permissions::{can, role_permissions};
use ocinye_domain::policy::{ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::audit::{self, action, AuditEntry};
use crate::error::{refusal, CoreError, CoreResult};

fn require(principal: &Principal, permission: Permission) -> CoreResult<()> {
    let ctx = ResourceContext::organisation(ResourceKind::Organisation, principal.organisation_id);
    if can(principal, permission, &ctx, None).allowed {
        Ok(())
    } else {
        Err(CoreError::PermissionDenied(
            "Não possui acesso à configuração da instância.".to_owned(),
        ))
    }
}

fn parse(stored: &str) -> CoreResult<Distribution> {
    stored
        .parse()
        .map_err(|error: ocinye_contracts::UnknownDistribution| {
            CoreError::Internal(error.to_string())
        })
}

fn set_of(rows: Vec<String>) -> CoreResult<DistributionSet> {
    rows.iter()
        .try_fold(DistributionSet::empty(), |set, d| Ok(set.with(parse(d)?)))
}

/// As Distribuições activadas desta Instância.
///
/// # Errors
///
/// Database errors.
pub async fn enabled<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
) -> CoreResult<DistributionSet> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT distribution FROM instance_distributions
          WHERE organisation_id = $1 AND state = 'enabled'",
    )
    .bind(organisation_id)
    .fetch_all(executor)
    .await?;
    set_of(rows)
}

/// O acesso registado de um membro — activadas ou não (desactivar guarda-o).
///
/// # Errors
///
/// Database errors.
pub async fn member_access<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    person_id: Uuid,
) -> CoreResult<DistributionSet> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT distribution FROM member_distribution_access
          WHERE organisation_id = $1 AND person_id = $2",
    )
    .bind(organisation_id)
    .bind(person_id)
    .fetch_all(executor)
    .await?;
    set_of(rows)
}

/// A decisão do Core: este membro pode entrar nesta Distribuição?
///
/// # Errors
///
/// Database errors.
pub async fn entry_decision(
    pool: &PgPool,
    organisation_id: Uuid,
    person_id: Uuid,
    distribution: Distribution,
) -> CoreResult<EntryDecision> {
    let (enabled, access): (bool, bool) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM instance_distributions
                         WHERE organisation_id = $1 AND distribution = $3 AND state = 'enabled'),
                EXISTS (SELECT 1 FROM member_distribution_access
                         WHERE organisation_id = $1 AND person_id = $2 AND distribution = $3)",
    )
    .bind(organisation_id)
    .bind(person_id)
    .bind(distribution.as_str())
    .fetch_one(pool)
    .await?;
    Ok(if !enabled {
        EntryDecision::NotEnabled
    } else if access {
        EntryDecision::Allowed
    } else {
        EntryDecision::NoAccess
    })
}

/// A recusa tipada de uma decisão que não é `Allowed` (ADR-0111).
#[must_use]
pub fn refusal_of(decision: EntryDecision) -> Option<CoreError> {
    match decision {
        EntryDecision::Allowed => None,
        EntryDecision::NotEnabled => Some(CoreError::Invariant {
            code: ErrorCode::PermissionDenied,
            reason: refusal::DISTRIBUTION_NOT_ENABLED,
            message: "Esta Distribuição não está activa nesta Instância.".to_owned(),
        }),
        EntryDecision::NoAccess => Some(CoreError::Invariant {
            code: ErrorCode::PermissionDenied,
            reason: refusal::DISTRIBUTION_NO_ACCESS,
            message: "Não tem acesso a esta Distribuição.".to_owned(),
        }),
    }
}

/// O que a entrada do membro mostra (ADR-0625 §1).
#[derive(Debug, Clone, Serialize)]
pub struct MyDistributions {
    /// As activadas na Instância.
    pub enabled: Vec<Distribution>,
    /// Activadas ∩ acesso do membro.
    pub accessible: Vec<Distribution>,
    /// A activa nesta sessão, se a sessão já entrou numa.
    pub active: Option<Distribution>,
    /// `none` · `direct` · `choose`.
    pub entry: &'static str,
    /// A Distribuição que a sessão tinha guardada, mesmo que já não valha.
    pub stored: Option<Distribution>,
    /// Porque deixou de valer: `not_enabled` (S39) · `no_access` (S18) — ou
    /// `None` quando vale, ou não havia.
    pub stored_refusal: Option<&'static str>,
    /// Os pontos de acesso activos por onde se entra em cada Distribuição
    /// acessível: o genérico canónico, e os fixos (para mudar a partir de um
    /// ponto fixo por um endereço **configurado**, nunca do pedido — S17).
    pub generic_host: Option<String>,
    /// `(Distribuição, anfitrião)` dos pontos fixos activos, só das acessíveis.
    pub bound_hosts: Vec<(Distribution, String)>,
    /// ADR-0019 §9: as aplicações activas na Instância **só** porque outra
    /// Distribuição activada as traz — a activa não as traz e a Instância não
    /// as activou explicitamente. Não se oferecem nesta sessão; a autoridade
    /// não muda (o Core continua a servir quem as abrir por outra
    /// Distribuição).
    pub inactive_here: Vec<ocinye_contracts::ApplicationId>,
}

/// As Distribuições do membro, e a decisão de entrada (0 / 1 / várias).
///
/// # Errors
///
/// Database errors.
pub async fn mine(
    pool: &PgPool,
    principal: &Principal,
    active: Option<Distribution>,
) -> CoreResult<MyDistributions> {
    let on = enabled(pool, principal.organisation_id).await?;
    let access = member_access(pool, principal.organisation_id, principal.person_id).await?;
    let accessible = on.intersect(access);
    let stored_refusal = active.and_then(|d| {
        if !on.contains(d) {
            Some(refusal::DISTRIBUTION_NOT_ENABLED)
        } else if !access.contains(d) {
            Some(refusal::DISTRIBUTION_NO_ACCESS)
        } else {
            None
        }
    });
    let hosts: Vec<(String, Option<String>, bool)> = sqlx::query_as(
        "SELECT hostname, binding_distribution, canonical FROM access_endpoints
          WHERE organisation_id = $1 AND state = 'active' ORDER BY canonical DESC, hostname",
    )
    .bind(principal.organisation_id)
    .fetch_all(pool)
    .await?;
    let generic_host = hosts
        .iter()
        .find(|(_, b, _)| b.is_none())
        .map(|(h, _, _)| h.clone());
    let bound_hosts = hosts
        .iter()
        .filter_map(|(h, b, _)| Some((b.as_deref()?.parse::<Distribution>().ok()?, h.clone())))
        .filter(|(d, _)| accessible.contains(*d))
        .collect();
    let here = active.filter(|d| accessible.contains(*d));
    let inactive_here = match here {
        Some(d) => super::applications::application_states(pool, principal.organisation_id)
            .await?
            .applications
            .into_iter()
            .filter(|a| a.active && a.id.is_optional() && !a.explicit && !d.activates(a.id))
            .map(|a| a.id)
            .collect(),
        None => Vec::new(),
    };
    Ok(MyDistributions {
        enabled: on.iter().collect(),
        accessible: accessible.iter().collect(),
        active: active.filter(|d| accessible.contains(*d)),
        entry: match EntryResolution::from_accessible(accessible) {
            EntryResolution::NoneAccessible => "none",
            EntryResolution::Direct(_) => "direct",
            EntryResolution::Choose(_) => "choose",
        },
        stored: active,
        stored_refusal,
        generic_host,
        bound_hosts,
        inactive_here,
    })
}

/// Entra numa Distribuição nesta sessão: o Core decide, e grava a escolha na
/// sessão. O contexto activo é reposto — nunca transportado (ADR-0625 §7).
///
/// # Errors
///
/// [`CoreError::Invariant`] `distribution_not_enabled` / `distribution_no_access`;
/// database errors.
pub async fn enter(
    pool: &PgPool,
    principal: &Principal,
    session_id: Uuid,
    distribution: Distribution,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    let decision = entry_decision(
        pool,
        principal.organisation_id,
        principal.person_id,
        distribution,
    )
    .await?;
    if let Some(refused) = refusal_of(decision) {
        return Err(refused);
    }
    let mut tx = pool.begin().await?;
    let previous: Option<String> = sqlx::query_scalar(
        "UPDATE sessions s SET active_distribution = $2,
                active_context_kind = NULL, active_context_id = NULL
           FROM sessions old
          WHERE s.id = $1 AND old.id = s.id AND s.person_id = $3
      RETURNING old.active_distribution",
    )
    .bind(session_id)
    .bind(distribution.as_str())
    .bind(principal.person_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    if previous.as_deref() != Some(distribution.as_str()) {
        audit::record(
            &mut tx,
            Some(principal),
            ids,
            AuditEntry::new(action::DISTRIBUTION_ENTERED, "distribution")
                .resource(principal.organisation_id)
                .detail("distribution", distribution.as_str())
                .detail("from", previous.as_deref().unwrap_or("")),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// A Distribuição activa de uma sessão ainda vale? Revalidado a cada pedido:
/// uma desactivação ou uma revogação vê-se no pedido seguinte (S39, S18).
///
/// # Errors
///
/// [`CoreError::Invariant`] when it no longer does; database errors.
pub async fn revalidate(
    pool: &PgPool,
    organisation_id: Uuid,
    person_id: Uuid,
    active: Distribution,
) -> CoreResult<()> {
    match refusal_of(entry_decision(pool, organisation_id, person_id, active).await?) {
        Some(refused) => Err(refused),
        None => Ok(()),
    }
}

/// Uma Distribuição desta Instância, para a Administração (S26).
#[derive(Debug, Clone, Serialize)]
pub struct InstanceDistribution {
    /// Qual.
    pub distribution: Distribution,
    /// `enabled`, `disabled`, ou `available` (nunca activada).
    pub state: &'static str,
    /// Quantos membros têm acesso registado.
    pub members_with_access: i64,
    /// Os anfitriões dos pontos de acesso fixos nela.
    pub endpoints: Vec<String>,
    /// As sessões vivas que estão nela agora (o facto de S38).
    pub active_sessions: i64,
}

/// As quatro Distribuições e o estado de cada uma nesta Instância.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.view`; database errors.
pub async fn instance_distributions(
    pool: &PgPool,
    principal: &Principal,
) -> CoreResult<Vec<InstanceDistribution>> {
    require(principal, Permission::OrganisationView)?;
    let rows: Vec<(String, String, i64, Vec<String>, i64)> = sqlx::query_as(
        "SELECT d.distribution, d.state,
                (SELECT count(*) FROM member_distribution_access a
                  WHERE a.organisation_id = d.organisation_id AND a.distribution = d.distribution),
                COALESCE((SELECT array_agg(e.hostname ORDER BY e.hostname) FROM access_endpoints e
                  WHERE e.organisation_id = d.organisation_id
                    AND e.binding_distribution = d.distribution), '{}'),
                (SELECT count(*) FROM sessions s JOIN people p ON p.id = s.person_id
                  WHERE p.organisation_id = d.organisation_id
                    AND s.active_distribution = d.distribution
                    AND s.revoked_at IS NULL AND s.expires_at > now())
           FROM instance_distributions d WHERE d.organisation_id = $1",
    )
    .bind(principal.organisation_id)
    .fetch_all(pool)
    .await?;
    Distribution::ALL
        .into_iter()
        .map(|d| {
            let row = rows.iter().find(|r| r.0 == d.as_str());
            Ok(InstanceDistribution {
                distribution: d,
                state: match row.map(|r| r.1.as_str()) {
                    Some("enabled") => "enabled",
                    Some("disabled") => "disabled",
                    _ => "available",
                },
                members_with_access: row.map_or(0, |r| r.2),
                endpoints: row.map(|r| r.3.clone()).unwrap_or_default(),
                active_sessions: row.map_or(0, |r| r.4),
            })
        })
        .collect()
}

/// `organisations.profile` fica um espelho só de leitura durante uma versão
/// (0060): a activada mais antiga, para que um binário anterior arranque.
async fn mirror_profile(tx: &mut crate::Tx<'_>, organisation_id: Uuid) -> CoreResult<()> {
    sqlx::query(
        "UPDATE organisations o SET profile = d.distribution
           FROM (SELECT distribution FROM instance_distributions
                  WHERE organisation_id = $1 AND state = 'enabled'
                  ORDER BY enabled_at, distribution LIMIT 1) d
          WHERE o.id = $1 AND o.profile <> d.distribution",
    )
    .bind(organisation_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Os papéis que trazem `organisation.manage` (a política, não uma lista à mão).
fn managing_roles() -> Vec<&'static str> {
    TechnicalRole::ALL
        .into_iter()
        .filter(|role| role_permissions(*role).contains(&Permission::OrganisationManage))
        .map(TechnicalRole::as_str)
        .collect()
}

/// Pelo menos um membro capaz de entrar, com `organisation.manage`, tem acesso
/// a uma Distribuição activada — senão ninguém administra a Instância.
async fn ensure_an_administrator_can_enter(
    tx: &mut crate::Tx<'_>,
    organisation_id: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended('ocinye.distribution_admin.' || $1::text, 0))",
    )
    .bind(organisation_id)
    .execute(&mut **tx)
    .await?;
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1
               FROM person_roles pr
               JOIN people p ON p.id = pr.person_id
               JOIN member_distribution_access a
                 ON a.person_id = p.id AND a.organisation_id = p.organisation_id
               JOIN instance_distributions d
                 ON d.organisation_id = a.organisation_id AND d.distribution = a.distribution
              WHERE p.organisation_id = $1
                AND pr.revoked_at IS NULL
                AND pr.role = ANY($2)
                AND p.status IN ('invited', 'active')
                AND d.state = 'enabled')",
    )
    .bind(organisation_id)
    .bind(managing_roles())
    .fetch_one(&mut **tx)
    .await?;
    if ok {
        Ok(())
    } else {
        Err(CoreError::Invariant {
            code: ErrorCode::Conflict,
            reason: refusal::LAST_ADMINISTRATOR_ACCESS,
            message: "Isto deixaria a Instância sem nenhum administrador que consiga entrar. \
                      Dê primeiro acesso a outro administrador."
                .to_owned(),
        })
    }
}

/// Activa uma Distribuição. Aditivo: não toca em disposições, fixações nem no
/// acesso dos outros membros; só quem activa recebe acesso (ADR-0019 §6).
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`; database errors.
pub async fn enable(
    pool: &PgPool,
    principal: &Principal,
    distribution: Distribution,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    require(principal, Permission::OrganisationManage)?;
    let mut tx = pool.begin().await?;
    let changed = sqlx::query(
        "INSERT INTO instance_distributions
                (organisation_id, distribution, state, enabled_at, enabled_by_id)
         VALUES ($1, $2, 'enabled', now(), $3)
         ON CONFLICT (organisation_id, distribution) DO UPDATE
            SET state = 'enabled', enabled_at = now(), enabled_by_id = $3,
                disabled_at = NULL, disabled_by_id = NULL
          WHERE instance_distributions.state <> 'enabled'",
    )
    .bind(principal.organisation_id)
    .bind(distribution.as_str())
    .bind(principal.person_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    sqlx::query(
        "INSERT INTO member_distribution_access
                (organisation_id, person_id, distribution, granted_by_id)
         VALUES ($1, $2, $3, $2) ON CONFLICT DO NOTHING",
    )
    .bind(principal.organisation_id)
    .bind(principal.person_id)
    .bind(distribution.as_str())
    .execute(&mut *tx)
    .await?;
    if changed > 0 {
        mirror_profile(&mut tx, principal.organisation_id).await?;
        audit::record(
            &mut tx,
            Some(principal),
            ids,
            AuditEntry::new(action::DISTRIBUTION_ENABLED, "instance_distribution")
                .resource(principal.organisation_id)
                .detail("distribution", distribution.as_str()),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Desactiva uma Distribuição. Guarda tudo; recusa a última, e recusa a que
/// deixaria a Instância sem administrador que entre (ADR-0019 §5–6).
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`;
/// [`CoreError::Invariant`] `last_enabled_distribution`,
/// `last_administrator_access`, `distribution_not_enabled`; database errors.
pub async fn disable(
    pool: &PgPool,
    principal: &Principal,
    distribution: Distribution,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    require(principal, Permission::OrganisationManage)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT 1 FROM instance_distributions WHERE organisation_id = $1 FOR UPDATE")
        .bind(principal.organisation_id)
        .execute(&mut *tx)
        .await?;
    let current = ocinye_contracts::EnabledDistributions::new(
        enabled(&mut *tx, principal.organisation_id).await?,
    )
    .map_err(|_| CoreError::Internal("instância sem Distribuição activada".to_owned()))?;
    current
        .disable(distribution)
        .map_err(|refused| match refused {
            ocinye_contracts::DistributionRefusal::NotEnabled(_) => CoreError::Invariant {
                code: ErrorCode::Conflict,
                reason: refusal::DISTRIBUTION_NOT_ENABLED,
                message: "Esta Distribuição não está activa.".to_owned(),
            },
            _ => CoreError::Invariant {
                code: ErrorCode::Conflict,
                reason: refusal::LAST_ENABLED_DISTRIBUTION,
                message: "A Instância tem de manter pelo menos uma Distribuição activa.".to_owned(),
            },
        })?;
    sqlx::query(
        "UPDATE instance_distributions
            SET state = 'disabled', disabled_at = now(), disabled_by_id = $3
          WHERE organisation_id = $1 AND distribution = $2",
    )
    .bind(principal.organisation_id)
    .bind(distribution.as_str())
    .bind(principal.person_id)
    .execute(&mut *tx)
    .await?;
    ensure_an_administrator_can_enter(&mut tx, principal.organisation_id).await?;
    mirror_profile(&mut tx, principal.organisation_id).await?;
    audit::record(
        &mut tx,
        Some(principal),
        ids,
        AuditEntry::new(action::DISTRIBUTION_DISABLED, "instance_distribution")
            .resource(principal.organisation_id)
            .detail("distribution", distribution.as_str()),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Uma linha da matriz de acesso (S37).
#[derive(Debug, Clone, Serialize)]
pub struct MemberAccess {
    /// Quem.
    pub person_id: Uuid,
    /// O nome.
    pub full_name: String,
    /// O estado da conta.
    pub status: String,
    /// As Distribuições activadas a que tem acesso.
    pub distributions: Vec<Distribution>,
    /// Tem um papel que traz `organisation.manage` (a guarda do último
    /// administrador, S37); um facto, não uma decisão — quem recusa é o Core.
    pub administrator: bool,
    /// As sessões vivas deste membro em cada Distribuição (o facto de S37).
    pub active_sessions: std::collections::BTreeMap<Distribution, i64>,
}

/// Quem pode abrir cada Distribuição activada (S37). Não são papéis.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`; database errors.
pub async fn access_matrix(pool: &PgPool, principal: &Principal) -> CoreResult<Vec<MemberAccess>> {
    require(principal, Permission::OrganisationManage)?;
    let rows: Vec<(Uuid, String, String, Vec<String>, bool, Vec<String>)> = sqlx::query_as(
        "SELECT p.id, p.full_name, p.status,
                COALESCE(array_agg(a.distribution ORDER BY a.distribution)
                         FILTER (WHERE a.distribution IS NOT NULL AND d.state = 'enabled'), '{}'),
                p.status IN ('invited', 'active') AND EXISTS (
                    SELECT 1 FROM person_roles pr
                     WHERE pr.person_id = p.id AND pr.revoked_at IS NULL AND pr.role = ANY($2)),
                COALESCE((SELECT array_agg(s.active_distribution) FROM sessions s
                  WHERE s.person_id = p.id AND s.active_distribution IS NOT NULL
                    AND s.revoked_at IS NULL AND s.expires_at > now()), '{}')
           FROM people p
           LEFT JOIN member_distribution_access a
             ON a.person_id = p.id AND a.organisation_id = p.organisation_id
           LEFT JOIN instance_distributions d
             ON d.organisation_id = a.organisation_id AND d.distribution = a.distribution
          WHERE p.organisation_id = $1 AND p.status <> 'deleted'
          GROUP BY p.id, p.full_name, p.status
          ORDER BY lower(p.full_name), p.id",
    )
    .bind(principal.organisation_id)
    .bind(managing_roles())
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(person_id, full_name, status, ds, administrator, live)| {
            let mut active_sessions = std::collections::BTreeMap::new();
            for d in &live {
                *active_sessions.entry(parse(d)?).or_insert(0) += 1;
            }
            Ok(MemberAccess {
                person_id,
                full_name,
                status,
                distributions: ds.iter().map(|d| parse(d)).collect::<CoreResult<_>>()?,
                administrator,
                active_sessions,
            })
        })
        .collect()
}

async fn person_in(
    tx: &mut crate::Tx<'_>,
    organisation_id: Uuid,
    person_id: Uuid,
) -> CoreResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM people WHERE id = $1 AND organisation_id = $2)",
    )
    .bind(person_id)
    .bind(organisation_id)
    .fetch_one(&mut **tx)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(CoreError::NotFound("Membro não encontrado.".to_owned()))
    }
}

/// Dá a um membro acesso a uma Distribuição activada (S37). Auditado.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`;
/// [`CoreError::NotFound`] for a person of another Instance;
/// [`CoreError::Invariant`] `distribution_not_enabled`; database errors.
pub async fn grant(
    pool: &PgPool,
    principal: &Principal,
    person_id: Uuid,
    distribution: Distribution,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    require(principal, Permission::OrganisationManage)?;
    let mut tx = pool.begin().await?;
    person_in(&mut tx, principal.organisation_id, person_id).await?;
    if !enabled(&mut *tx, principal.organisation_id)
        .await?
        .contains(distribution)
    {
        return Err(CoreError::Invariant {
            code: ErrorCode::Conflict,
            reason: refusal::DISTRIBUTION_NOT_ENABLED,
            message: "Esta Distribuição não está activa.".to_owned(),
        });
    }
    let added = sqlx::query(
        "INSERT INTO member_distribution_access
                (organisation_id, person_id, distribution, granted_by_id)
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(principal.organisation_id)
    .bind(person_id)
    .bind(distribution.as_str())
    .bind(principal.person_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if added > 0 {
        audit::record(
            &mut tx,
            Some(principal),
            ids,
            AuditEntry::new(
                action::DISTRIBUTION_ACCESS_GRANTED,
                "member_distribution_access",
            )
            .resource(person_id)
            .detail("distribution", distribution.as_str()),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Retira a um membro o acesso a uma Distribuição (S37). A sessão dele nessa
/// Distribuição deixa de valer no pedido seguinte (S18). Auditado.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without `organisation.manage`;
/// [`CoreError::Invariant`] `last_administrator_access`; database errors.
pub async fn revoke(
    pool: &PgPool,
    principal: &Principal,
    person_id: Uuid,
    distribution: Distribution,
    ids: &CorrelationIds,
) -> CoreResult<()> {
    require(principal, Permission::OrganisationManage)?;
    let mut tx = pool.begin().await?;
    person_in(&mut tx, principal.organisation_id, person_id).await?;
    let removed = sqlx::query(
        "DELETE FROM member_distribution_access
          WHERE organisation_id = $1 AND person_id = $2 AND distribution = $3",
    )
    .bind(principal.organisation_id)
    .bind(person_id)
    .bind(distribution.as_str())
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if removed > 0 {
        ensure_an_administrator_can_enter(&mut tx, principal.organisation_id).await?;
        audit::record(
            &mut tx,
            Some(principal),
            ids,
            AuditEntry::new(
                action::DISTRIBUTION_ACCESS_REVOKED,
                "member_distribution_access",
            )
            .resource(person_id)
            .detail("distribution", distribution.as_str()),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Dá ao primeiro administrador acesso a todas as activadas (fronteira D011).
///
/// # Errors
///
/// Database errors.
pub async fn grant_all_enabled<'e>(
    executor: impl PgExecutor<'e>,
    organisation_id: Uuid,
    person_id: Uuid,
) -> CoreResult<()> {
    sqlx::query(
        "INSERT INTO member_distribution_access (organisation_id, person_id, distribution)
         SELECT organisation_id, $2, distribution FROM instance_distributions
          WHERE organisation_id = $1 AND state = 'enabled'
         ON CONFLICT DO NOTHING",
    )
    .bind(organisation_id)
    .bind(person_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// O conjunto inicial de uma Instância **acabada de criar** (instalação,
/// `bootstrap-admin --distribution …`, fronteira D011): a primeira e as
/// restantes, activadas de uma vez, auditadas sem actor (é o servidor).
/// Nunca toca numa Instância que já tem membros.
///
/// # Errors
///
/// [`CoreError::Conflict`] for an Instance that already has people; database errors.
pub async fn initial(
    pool: &PgPool,
    organisation_id: Uuid,
    first: Distribution,
    more: &[Distribution],
    ids: &CorrelationIds,
) -> CoreResult<()> {
    let mut tx = pool.begin().await?;
    let people: i64 = sqlx::query_scalar("SELECT count(*) FROM people WHERE organisation_id = $1")
        .bind(organisation_id)
        .fetch_one(&mut *tx)
        .await?;
    if people > 0 {
        return Err(CoreError::Conflict(
            "a Instância já tem membros: as Distribuições activam-se na Administração.".to_owned(),
        ));
    }
    sqlx::query(
        "DELETE FROM instance_distributions WHERE organisation_id = $1 AND distribution <> $2",
    )
    .bind(organisation_id)
    .bind(first.as_str())
    .execute(&mut *tx)
    .await?;
    for d in std::iter::once(first).chain(more.iter().copied()) {
        sqlx::query(
            "INSERT INTO instance_distributions (organisation_id, distribution, state)
             VALUES ($1, $2, 'enabled') ON CONFLICT (organisation_id, distribution)
             DO UPDATE SET state = 'enabled', disabled_at = NULL, disabled_by_id = NULL",
        )
        .bind(organisation_id)
        .bind(d.as_str())
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE organisations SET profile = $2 WHERE id = $1")
        .bind(organisation_id)
        .bind(first.as_str())
        .execute(&mut *tx)
        .await?;
    let all: Vec<&str> = std::iter::once(first)
        .chain(more.iter().copied())
        .map(Distribution::as_str)
        .collect();
    audit::record(
        &mut tx,
        None,
        ids,
        AuditEntry::new(action::DISTRIBUTION_ENABLED, "instance_distribution")
            .resource(organisation_id)
            .detail("event", "installation")
            .detail("distributions", all.join(",")),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Em que Distribuição vive o estado de apresentação deste pedido (disposição,
/// fixações, fundo — ADR-0019 §7): a activa da sessão; ou, numa sessão que
/// nunca entrou numa (um cliente da API), a única acessível. Com várias e
/// nenhuma escolhida, não se adivinha.
///
/// # Errors
///
/// [`CoreError::Invariant`] `distribution_no_access` with none accessible,
/// [`CoreError::Conflict`] with several and none active; database errors.
pub async fn for_member_state(
    pool: &PgPool,
    principal: &Principal,
    active: Option<Distribution>,
) -> CoreResult<Distribution> {
    if let Some(d) = active {
        return Ok(d);
    }
    let accessible = enabled(pool, principal.organisation_id)
        .await?
        .intersect(member_access(pool, principal.organisation_id, principal.person_id).await?);
    match EntryResolution::from_accessible(accessible) {
        EntryResolution::Direct(d) => Ok(d),
        EntryResolution::NoneAccessible => Err(refusal_of(EntryDecision::NoAccess)
            .unwrap_or_else(|| CoreError::Internal("sem recusa".to_owned()))),
        EntryResolution::Choose(_) => Err(CoreError::Conflict(
            "Escolha primeiro a Distribuição (POST /me/distribution).".to_owned(),
        )),
    }
}
