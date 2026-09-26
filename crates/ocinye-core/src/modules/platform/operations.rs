//! The operator's view of an Instance (Part 13 of the generalization).
//!
//! # What an operator needs, and what they must not get
//!
//! One read that answers «what is up, what is down, and where is it tight?»:
//! each node's liveness, each AI provider's last observed health, which
//! applications are active, and how close members are to their storage limits.
//!
//! It is **not** surveillance. No file name, no note, no prompt, no message, no
//! per-member figure leaves here: storage pressure is a count of members in each
//! state, never who. The readiness report (`/ready`) says whether each part of
//! the platform can serve; this says what an operator acts on.

use chrono::{DateTime, Utc};
use ocinye_contracts::{ApplicationId, ComputeNodeStatus, StorageState};
use ocinye_domain::policy::{authorize, Action, ResourceContext, ResourceKind};
use ocinye_domain::Principal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::CoreConfig;
use crate::error::{CoreError, CoreResult};
use crate::modules::{compute, intelligence, organisation, resource};

/// Everything an operator acts on, in one read.
#[derive(Debug, Clone, Serialize)]
pub struct OperationsOverview {
    /// Each node, and whether it is heartbeating.
    pub nodes: Vec<NodeHealth>,
    /// Each registered AI provider, and the outcome of its last call.
    pub ai_providers: Vec<ProviderHealth>,
    /// Each application, and whether this Instance has it active.
    pub applications: Vec<ApplicationHealth>,
    /// How close members are to their personal storage limits — counts only.
    pub storage: StoragePressure,
}

/// A node, as an operator sees it.
#[derive(Debug, Clone, Serialize)]
pub struct NodeHealth {
    /// Identifier.
    pub id: Uuid,
    /// Institutional identifier (`CAM-01`).
    pub identifier: String,
    /// Derived liveness.
    pub status: ComputeNodeStatus,
    /// When it last reported.
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// A provider, as an operator sees it — never its credential.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealth {
    /// Identifier.
    pub id: Uuid,
    /// Its name.
    pub label: String,
    /// Which protocol it speaks.
    pub kind: String,
    /// `local` or `external`.
    pub residency: String,
    /// Whether it may be routed to.
    pub enabled: bool,
    /// `unknown`, `healthy`, `unreachable` or `refused`.
    pub health: String,
    /// When that was observed.
    pub last_checked_at: Option<DateTime<Utc>>,
}

/// An application's state in this Instance.
#[derive(Debug, Clone, Serialize)]
pub struct ApplicationHealth {
    /// The application.
    pub id: ApplicationId,
    /// Whether it is active.
    pub active: bool,
}

/// Members in each storage state. Who they are is not an operator's business.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StoragePressure {
    /// Members below the warning threshold.
    pub normal: u32,
    /// Members at or above the warning threshold.
    pub warning: u32,
    /// Members at or above the strong-warning threshold.
    pub critical: u32,
    /// Members at or above their limit: new writes refused, nothing deleted.
    pub over_quota: u32,
    /// Bytes all members' own objects occupy.
    pub personal_used_bytes: i64,
}

/// The operator's view.
///
/// # Errors
///
/// [`CoreError::PermissionDenied`] without platform administration.
pub async fn overview(
    pool: &PgPool,
    principal: &Principal,
    config: &CoreConfig,
) -> CoreResult<OperationsOverview> {
    let ctx = ResourceContext::organisation(ResourceKind::Platform, principal.organisation_id);
    authorize(principal, Action::Administer, &ctx)
        .map_err(|(denial, decision)| CoreError::from_denial(denial, &decision))?;

    let nodes = compute::list_nodes(pool, principal, &config.compute)
        .await?
        .into_iter()
        .map(|(node, status)| NodeHealth {
            id: node.id,
            identifier: node.identifier,
            status,
            last_seen_at: node.last_seen_at,
        })
        .collect();

    let ai_providers = intelligence::providers::list_providers(pool, principal)
        .await?
        .into_iter()
        .map(|p| ProviderHealth {
            id: p.id,
            label: p.label,
            kind: p.kind,
            residency: p.residency,
            enabled: p.enabled,
            health: p.health,
            last_checked_at: p.last_checked_at,
        })
        .collect();

    let applications = organisation::application_states(pool, principal.organisation_id)
        .await?
        .applications
        .into_iter()
        .map(|a| ApplicationHealth {
            id: a.id,
            active: a.active,
        })
        .collect();

    let membros: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM people WHERE organisation_id = $1 AND status = 'active'",
    )
    .bind(principal.organisation_id)
    .fetch_all(pool)
    .await?;
    let mut storage = StoragePressure::default();
    for membro in membros {
        let estado = resource::personal_storage_status(pool, principal, membro).await?;
        storage.personal_used_bytes += estado.used_bytes;
        match estado.state {
            StorageState::Normal => storage.normal += 1,
            StorageState::Warning => storage.warning += 1,
            StorageState::Critical => storage.critical += 1,
            StorageState::OverQuota => storage.over_quota += 1,
        }
    }

    Ok(OperationsOverview {
        nodes,
        ai_providers,
        applications,
        storage,
    })
}
