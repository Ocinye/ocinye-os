//! Agent definitions.
//!
//! # An agent is a definition, not a running thing
//!
//! It carries a name, a purpose, instructions, the capability it asks for, the
//! scope it belongs to and the knowledge it may draw on. None of that needs a
//! model, so agents are creatable with zero AI nodes registered (briefing §9).
//!
//! What a missing model prevents is **execution**, and that is *derived* from
//! capability availability at read time — never stored, so an agent's state
//! becomes correct the moment a node is enrolled, with no migration and no
//! backfill.
//!
//! # An agent never widens its actor
//!
//! `max_classification` is a ceiling, capped at creation to what the creator
//! could themselves reach and capped again at retrieval. Effective AI access is
//! the intersection of actor, agent and resource policy — never the union
//! (briefing §81).

use chrono::{DateTime, Utc};
use ocinye_contracts::{
    AiCapability, Classification, Permission, SystemCapabilities, SystemCapability,
};
use ocinye_domain::policy::evaluate;
use ocinye_domain::{can, Action, ResourceContext, ResourceKind};
use ocinye_observability::CorrelationIds;
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::audit::{self, action, AuditEntry};
use crate::error::{CoreError, CoreResult};
use ocinye_domain::Principal;

/// Where an agent lives and who may reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentScope {
    /// Its creator alone.
    Personal,
    /// One research workspace.
    Workspace,
    /// One scientific unit.
    Unit,
    /// The whole institution.
    Institutional,
}

impl AgentScope {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Workspace => "workspace",
            Self::Unit => "unit",
            Self::Institutional => "institutional",
        }
    }

    /// Parse from the stable representation.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "personal" => Self::Personal,
            "workspace" => Self::Workspace,
            "unit" => Self::Unit,
            "institutional" => Self::Institutional,
            _ => return None,
        })
    }

    /// The permission required to create an agent at this scope.
    ///
    /// Graded deliberately: a research member may create for themselves, a lead
    /// for a project, a unit manager for a unit, and only a platform
    /// administrator institution-wide (briefing §80).
    #[must_use]
    pub const fn required_permission(self) -> Permission {
        match self {
            Self::Personal => Permission::AgentsCreatePersonal,
            Self::Workspace => Permission::AgentsCreateProject,
            Self::Unit => Permission::AgentsCreateUnit,
            Self::Institutional => Permission::AgentsCreateInstitutional,
        }
    }

    /// Whether this scope names a unit or workspace.
    #[must_use]
    pub const fn needs_target(self) -> bool {
        matches!(self, Self::Workspace | Self::Unit)
    }
}

/// Whether an agent can currently run.
///
/// Derived at read time from capability availability, never stored: an agent
/// must not claim `active` when nothing can serve it (briefing §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    /// Defined, and a capability can serve it now.
    Ready,
    /// Defined and complete, but no capability can serve it.
    ///
    /// The normal state of every agent before the first AI node is registered.
    Configured,
    /// Its owner disabled it.
    Disabled,
    /// Archived.
    Archived,
}

impl AgentState {
    /// Stable representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Configured => "configured",
            Self::Disabled => "disabled",
            Self::Archived => "archived",
        }
    }

    /// Label shown to a member.
    ///
    /// `Configured` deliberately does not read as an error: nothing is broken,
    /// the infrastructure simply is not installed yet.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "Pronto",
            Self::Configured => "Configurado — sem capacidade disponível",
            Self::Disabled => "Desactivado",
            Self::Archived => "Arquivado",
        }
    }

    /// Whether the agent can be invoked right now.
    #[must_use]
    pub const fn can_execute(self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// An agent, as read.
#[derive(Debug, Clone, Serialize)]
pub struct Agent {
    /// Identifier.
    pub id: Uuid,
    /// Name.
    pub name: String,
    /// What it is for.
    pub purpose: Option<String>,
    /// How it should respond and what it must not do.
    pub instructions: Option<String>,
    /// The capability it asks for. Never a model name.
    pub capability: AiCapability,
    /// Where it lives.
    pub scope: AgentScope,
    /// Which unit or workspace, when scoped.
    pub scope_id: Option<Uuid>,
    /// Ceiling on what it may retrieve.
    pub max_classification: Classification,
    /// Whether it may draw on bibliography.
    pub uses_bibliography: bool,
    /// Whether it may draw on documents.
    pub uses_documents: bool,
    /// Whether it may draw on datasets.
    pub uses_datasets: bool,
    /// Derived execution state.
    pub state: AgentState,
    /// Human-readable state, for the interface.
    pub state_label: &'static str,
    /// Who created it.
    pub created_by_name: String,
    /// When.
    pub created_at: DateTime<Utc>,
}

/// What is needed to define an agent.
#[derive(Debug, Clone)]
pub struct NewAgent {
    /// Name.
    pub name: String,
    /// What it is for.
    pub purpose: Option<String>,
    /// How it should respond.
    pub instructions: Option<String>,
    /// The capability it asks for.
    pub capability: AiCapability,
    /// Where it lives.
    pub scope: AgentScope,
    /// Which unit or workspace.
    pub scope_id: Option<Uuid>,
    /// Ceiling on retrieval.
    pub max_classification: Classification,
    /// Knowledge sources.
    pub uses_bibliography: bool,
    /// Knowledge sources.
    pub uses_documents: bool,
    /// Knowledge sources.
    pub uses_datasets: bool,
}

/// Longest instructions accepted.
///
/// An agent whose instructions are a whole corpus is a retrieval problem
/// wearing a prompt.
const MAX_INSTRUCTIONS: usize = 8_000;

/// Create an agent.
///
/// # Errors
///
/// Returns [`CoreError::PermissionDenied`] when the scope's permission is not
/// held, [`CoreError::Validation`] for a bad definition, and
/// [`CoreError::Conflict`] when the name is taken in that scope.
pub async fn create(
    pool: &PgPool,
    actor: &Principal,
    new: &NewAgent,
    ids: &CorrelationIds,
) -> CoreResult<Uuid> {
    let name = new.name.trim();
    if name.chars().count() < 2 || name.chars().count() > 128 {
        return Err(CoreError::Validation(
            "O nome do agente deve ter entre 2 e 128 caracteres.".to_owned(),
        ));
    }
    if new
        .instructions
        .as_deref()
        .is_some_and(|text| text.chars().count() > MAX_INSTRUCTIONS)
    {
        return Err(CoreError::Validation(format!(
            "As instruções não podem exceder {MAX_INSTRUCTIONS} caracteres."
        )));
    }

    match (new.scope.needs_target(), new.scope_id) {
        (true, None) => {
            return Err(CoreError::Validation(
                "Um agente com âmbito de unidade ou de workspace tem de nomear o seu alvo."
                    .to_owned(),
            ))
        }
        (false, Some(_)) => {
            return Err(CoreError::Validation(
                "Um agente pessoal ou institucional não nomeia um alvo.".to_owned(),
            ))
        }
        _ => {}
    }

    // The scope decides which permission is required, and the context decides
    // *where* it must hold: creating a unit agent needs the permission inside
    // that unit, not merely somewhere.
    let ctx = context_for(actor, new.scope, new.scope_id);
    let permission = new.scope.required_permission();
    if !can(actor, permission, &ctx, new.scope_id).allowed {
        return Err(CoreError::PermissionDenied(
            "Não possui acesso para criar um agente com este âmbito.".to_owned(),
        ));
    }

    // An agent must never be able to reach further than the person defining it.
    //
    // The question is *could this person read material at this classification,
    // here* — which is the classification gate, not a named content permission.
    // Asking for `DocumentsView` would be wrong: a research member holds it
    // through unit or workspace membership, never at organisation scope, so a
    // perfectly ordinary INTERNAL personal agent would be refused.
    //
    // Capped again at retrieval time; this is the first of two gates
    // (briefing §81).
    let ceiling = ctx.with_classification(new.max_classification);
    if !evaluate(actor, Action::Read, &ceiling).allowed {
        return Err(CoreError::Validation(format!(
            "Não pode criar um agente com acesso a material {}: o agente nunca \
             excede quem o cria.",
            new.max_classification.as_str()
        )));
    }

    let mut tx = pool.begin().await?;

    let id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO ai_agents
             (organisation_id, name, purpose, instructions, capability, scope, scope_id,
              max_classification, uses_bibliography, uses_documents, uses_datasets,
              created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT DO NOTHING
         RETURNING id",
    )
    .bind(actor.organisation_id)
    .bind(name)
    .bind(
        new.purpose
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(
        new.instructions
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(new.capability.as_str())
    .bind(new.scope.as_str())
    .bind(new.scope_id)
    .bind(new.max_classification.as_str())
    .bind(new.uses_bibliography)
    .bind(new.uses_documents)
    .bind(new.uses_datasets)
    .bind(actor.person_id)
    .fetch_optional(&mut *tx)
    .await?;

    let id = id.ok_or_else(|| {
        CoreError::Conflict("Já existe um agente com este nome neste âmbito.".to_owned())
    })?;

    audit::record(
        &mut tx,
        Some(actor),
        ids,
        AuditEntry::new(action::CREATE, "ai_agent")
            .resource(id)
            .classified(new.max_classification)
            .detail("name", name)
            .detail("capability", new.capability.as_str())
            .detail("scope", new.scope.as_str()),
    )
    .await?;

    tx.commit().await?;
    Ok(id)
}

/// List the agents an actor may see, with their derived execution state.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list(
    pool: &PgPool,
    actor: &Principal,
    capabilities: &SystemCapabilities,
) -> CoreResult<Vec<Agent>> {
    // Visibility is decided in SQL rather than by filtering afterwards: an
    // agent an actor may not see must not travel out of the database at all.
    let rows = sqlx::query(
        "SELECT a.id, a.name, a.purpose, a.instructions, a.capability, a.scope, a.scope_id,
                a.max_classification, a.uses_bibliography, a.uses_documents, a.uses_datasets,
                a.enabled, a.created_at, p.full_name AS created_by_name
           FROM ai_agents a
           JOIN people p ON p.id = a.created_by_id
          WHERE a.organisation_id = $1
            AND a.archived_at IS NULL
            AND (
                a.scope = 'institutional'
                OR (a.scope = 'personal' AND a.created_by_id = $2)
                OR (a.scope = 'unit' AND a.scope_id = ANY($3))
                OR (a.scope = 'workspace' AND a.scope_id = ANY($4))
            )
          ORDER BY a.created_at DESC",
    )
    .bind(actor.organisation_id)
    .bind(actor.person_id)
    .bind(actor.unit_ids())
    .bind(actor.workspace_ids())
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| agent_from_row(&row, capabilities))
        .collect()
}

/// Build an agent from a row, deriving its execution state.
fn agent_from_row(
    row: &sqlx::postgres::PgRow,
    capabilities: &SystemCapabilities,
) -> CoreResult<Agent> {
    let capability_name: String = row.try_get("capability")?;
    let scope_name: String = row.try_get("scope")?;
    let classification_name: String = row.try_get("max_classification")?;
    let enabled: bool = row.try_get("enabled")?;

    // Unknown vocabulary fails closed rather than being guessed at.
    let capability = AiCapability::parse(&capability_name).ok_or_else(|| {
        CoreError::Internal(format!("unknown agent capability: {capability_name}"))
    })?;
    let scope = AgentScope::parse(&scope_name)
        .ok_or_else(|| CoreError::Internal(format!("unknown agent scope: {scope_name}")))?;
    let max_classification =
        Classification::parse(&classification_name).unwrap_or(Classification::Restricted);

    let state = if enabled {
        if capabilities.is_usable(system_capability_for(capability)) {
            AgentState::Ready
        } else {
            AgentState::Configured
        }
    } else {
        AgentState::Disabled
    };

    Ok(Agent {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        purpose: row.try_get("purpose")?,
        instructions: row.try_get("instructions")?,
        capability,
        scope,
        scope_id: row.try_get("scope_id")?,
        max_classification,
        uses_bibliography: row.try_get("uses_bibliography")?,
        uses_documents: row.try_get("uses_documents")?,
        uses_datasets: row.try_get("uses_datasets")?,
        state,
        state_label: state.label(),
        created_by_name: row.try_get("created_by_name")?,
        created_at: row.try_get("created_at")?,
    })
}

/// Map an AI capability to the system capability that serves it.
const fn system_capability_for(capability: AiCapability) -> SystemCapability {
    match capability {
        AiCapability::General => SystemCapability::AiGeneral,
        AiCapability::Coding => SystemCapability::AiCoding,
        AiCapability::Reasoning => SystemCapability::AiReasoning,
        AiCapability::Embedding => SystemCapability::AiEmbedding,
    }
}

/// The authorization context an agent of this scope lives in.
fn context_for(actor: &Principal, scope: AgentScope, scope_id: Option<Uuid>) -> ResourceContext {
    match (scope, scope_id) {
        (AgentScope::Unit, Some(unit_id)) => {
            ResourceContext::unit(ResourceKind::AiCapability, actor.organisation_id, unit_id)
        }
        (AgentScope::Workspace, Some(workspace_id)) => ResourceContext {
            kind: ResourceKind::AiCapability,
            classification: Classification::Internal,
            unit_id: None,
            workspace_id: Some(workspace_id),
            organisation_id: Some(actor.organisation_id),
        },
        _ => ResourceContext::organisation(ResourceKind::AiCapability, actor.organisation_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_scope_requires_its_own_permission() {
        // A personal agent must never be creatable with the institutional
        // permission absent, and vice versa.
        assert_eq!(
            AgentScope::Personal.required_permission(),
            Permission::AgentsCreatePersonal
        );
        assert_eq!(
            AgentScope::Institutional.required_permission(),
            Permission::AgentsCreateInstitutional
        );
        assert_ne!(
            AgentScope::Workspace.required_permission(),
            AgentScope::Unit.required_permission()
        );
    }

    #[test]
    fn only_scoped_agents_name_a_target() {
        assert!(AgentScope::Unit.needs_target());
        assert!(AgentScope::Workspace.needs_target());
        assert!(!AgentScope::Personal.needs_target());
        assert!(!AgentScope::Institutional.needs_target());
    }

    #[test]
    fn scopes_round_trip() {
        for scope in [
            AgentScope::Personal,
            AgentScope::Workspace,
            AgentScope::Unit,
            AgentScope::Institutional,
        ] {
            assert_eq!(AgentScope::parse(scope.as_str()), Some(scope));
        }
    }

    #[test]
    fn only_ready_agents_execute() {
        assert!(AgentState::Ready.can_execute());
        for state in [
            AgentState::Configured,
            AgentState::Disabled,
            AgentState::Archived,
        ] {
            assert!(!state.can_execute(), "{state:?} claimed it could execute");
        }
    }

    #[test]
    fn a_configured_agent_does_not_read_as_broken() {
        // With zero AI nodes every agent is `Configured`. That is the normal
        // state of a fresh installation, not a fault (briefing §9).
        let label = AgentState::Configured.label().to_lowercase();
        for banned in ["erro", "falha", "indisponível—", "offline"] {
            assert!(!label.contains(banned), "«{banned}» in the label");
        }
        assert!(label.contains("configurado"));
    }

    #[test]
    fn every_ai_capability_maps_to_a_system_capability() {
        for capability in AiCapability::all() {
            let mapped = system_capability_for(capability);
            assert!(
                mapped.as_str().starts_with("ai."),
                "{capability:?} mapped to {mapped:?}"
            );
        }
    }
}
