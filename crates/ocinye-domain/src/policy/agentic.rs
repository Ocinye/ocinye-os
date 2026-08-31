//! The policy that governs agentic action.
//!
//! # The invariant this file exists to enforce
//!
//! > **Effective Agent Access = Actor Access ∩ Agent Scope ∩ Resource Policy**
//!
//! An intersection, never a union. An agent is a *narrowing* of what the person
//! using it could already do — a lens, not a key. There is no configuration, no
//! scope and no instruction that makes an agent able to do something its actor
//! could not (briefing §13, §83).
//!
//! # Why this is in the domain crate
//!
//! It is pure. No database, no model, no I/O. That means every one of these
//! decisions is exhaustively testable, and the tests below do exhaust them —
//! which matters more here than anywhere else in the Ocinye OS, because this is
//! the layer a compromised model would be attacking.

use ocinye_contracts::agentic::{AutonomyLevel, CapabilityDescriptor, RiskLevel};
use ocinye_contracts::{Classification, Permission};
use uuid::Uuid;

use super::{can, Decision, ResourceContext};
use crate::principal::Principal;

/// What an agent is allowed to be, as configured.
///
/// # Untrusted configuration
///
/// A member writes this. It can therefore only ever *reduce*: every field is a
/// ceiling, and none is a grant. An agent claiming institutional scope and
/// every capability in the registry is still bounded by whoever is using it
/// (briefing §83).
#[derive(Debug, Clone)]
pub struct AgentBoundary {
    /// The capabilities its definition permits.
    ///
    /// A capability absent here is refused even when the actor holds the
    /// permission for it — that is the agent narrowing its own reach.
    pub allowed_capabilities: Vec<String>,
    /// The highest classification it may draw on.
    ///
    /// Capped against the actor at retrieval, so a ceiling above the actor's
    /// own reach buys nothing.
    pub classification_ceiling: Classification,
    /// How far it may go without being asked again.
    pub autonomy: AutonomyLevel,
    /// The unit it is bound to, when bound to one.
    pub unit_id: Option<Uuid>,
    /// The research workspace it is bound to, when bound to one.
    pub workspace_id: Option<Uuid>,
}

impl AgentBoundary {
    /// The boundary of the Main Agent.
    ///
    /// # Why the Main Agent is not root
    ///
    /// It has the widest *capability list* — it has to reach every domain to
    /// orchestrate — and **no privilege at all**. Every request it makes is
    /// still checked against the acting person, exactly like any other agent's
    /// (briefing §12).
    ///
    /// `classification_ceiling` is `Restricted` here because the ceiling is not
    /// what protects `RESTRICTED` material: the actor's own access is, and it is
    /// applied on top. A lower ceiling would silently hide material from
    /// somebody who is entitled to it.
    #[must_use]
    pub fn main_agent(allowed: Vec<String>) -> Self {
        Self {
            allowed_capabilities: allowed,
            classification_ceiling: Classification::Restricted,
            autonomy: AutonomyLevel::Workflow,
            unit_id: None,
            workspace_id: None,
        }
    }

    /// Whether this agent's definition admits a capability at all.
    #[must_use]
    pub(crate) fn permits_capability(&self, capability: &str) -> bool {
        self.allowed_capabilities
            .iter()
            .any(|allowed| allowed == capability)
    }
}

/// Why an agentic request was refused.
///
/// Distinct variants because the member's next step differs for each, and
/// because collapsing them tells someone who lacks permission that the hardware
/// is missing — or the reverse (briefing §68, §107).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgenticRefusal {
    /// The acting person may not do this. Not the agent's fault.
    ActorLacksPermission,
    /// The agent's own definition does not admit this capability.
    OutsideAgentBoundary,
    /// The agent's autonomy is below what the capability demands.
    AutonomyTooLow,
    /// The capability may not touch material at this classification.
    ClassificationRefused,
    /// The agent is bound to a unit or workspace that this is not in.
    OutsideAgentScope,
}

impl AgenticRefusal {
    /// A sentence a member can act on.
    ///
    /// Deliberately does not name what the agent *would* have been able to do
    /// with more authority: that is a map of the boundary, drawn for whoever is
    /// probing it.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::ActorLacksPermission => {
                "Não possui acesso para realizar esta acção. O assistente não amplia \
                 as suas permissões."
            }
            Self::OutsideAgentBoundary => {
                "Este agente não está configurado para realizar esta acção."
            }
            Self::AutonomyTooLow => "Este agente pode preparar esta acção, mas não executá-la.",
            Self::ClassificationRefused => {
                "Esta acção não pode ser aplicada a material com esta classificação."
            }
            Self::OutsideAgentScope => {
                "Este agente está limitado a outra unidade ou Research Workspace."
            }
        }
    }

    /// Stable representation, for audit.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActorLacksPermission => "actor_lacks_permission",
            Self::OutsideAgentBoundary => "outside_agent_boundary",
            Self::AutonomyTooLow => "autonomy_too_low",
            Self::ClassificationRefused => "classification_refused",
            Self::OutsideAgentScope => "outside_agent_scope",
        }
    }
}

/// Whether an agent may ask the Core to run a capability.
///
/// # The order of the gates matters
///
/// The actor is checked **first**. Every later gate can only narrow. An
/// implementation that checked the agent's boundary first and the actor second
/// would still be correct today and would be one refactor away from being a
/// privilege escalation, because the shape would no longer say which is
/// authoritative.
///
/// # What this does not do
///
/// It does not execute, and it does not decide *approval*. Approval is a
/// separate question — «may this happen at all» and «has a person said yes»
/// are different, and conflating them lets a confirmation stand in for
/// authority ([`approval_needed`]).
pub fn may_invoke(
    principal: &Principal,
    agent: &AgentBoundary,
    descriptor: &CapabilityDescriptor,
    ctx: &ResourceContext,
    resource_id: Option<Uuid>,
) -> Result<Decision, AgenticRefusal> {
    // 1. The actor. Nothing the agent says can substitute for this.
    let decision = can(principal, descriptor.permission, ctx, resource_id);
    if !decision.allowed {
        return Err(AgenticRefusal::ActorLacksPermission);
    }

    // 2. The agent's own definition. A narrowing, never a widening.
    if !agent.permits_capability(descriptor.id.as_str()) {
        return Err(AgenticRefusal::OutsideAgentBoundary);
    }

    // 3. Where the agent is bound. An agent tied to one workspace does not act
    //    in another, even for an actor who could reach both.
    if let Some(bound) = agent.workspace_id {
        if ctx.workspace_id != Some(bound) {
            return Err(AgenticRefusal::OutsideAgentScope);
        }
    }
    if let Some(bound) = agent.unit_id {
        if ctx.unit_id != Some(bound) {
            return Err(AgenticRefusal::OutsideAgentScope);
        }
    }

    // 4. Classification, twice over: the agent's ceiling and the capability's.
    //    The actor's own reach was already applied in step 1.
    if ctx.classification.level() > agent.classification_ceiling.level() {
        return Err(AgenticRefusal::ClassificationRefused);
    }
    if let Some(ceiling) = descriptor.classification_ceiling {
        if ctx.classification.level() > ceiling.level() {
            return Err(AgenticRefusal::ClassificationRefused);
        }
    }

    // 5. Autonomy. A read needs none; executing needs the level to permit it.
    if descriptor.risk.mutates() {
        let effective = agent.autonomy.min(descriptor.max_autonomy);
        if !effective.may_execute() {
            return Err(AgenticRefusal::AutonomyTooLow);
        }
    }

    Ok(decision)
}

/// Whether a person has to confirm before this capability runs.
///
/// # Separate from authority on purpose
///
/// [`may_invoke`] answers *may this happen*. This answers *has somebody said
/// yes*. A confirmation is consent to a permitted act, never authority to
/// perform a forbidden one — the same rule the mail send policy holds, for the
/// same reason.
#[must_use]
pub fn approval_needed(descriptor: &CapabilityDescriptor, agent: &AgentBoundary) -> bool {
    if descriptor.requires_approval() {
        return true;
    }

    // An agent may be configured to be more cautious than the capability
    // demands: below `Act`, it prepares and asks rather than doing.
    descriptor.risk.mutates() && !agent.autonomy.may_execute()
}

/// The highest classification this actor may feed to a model.
///
/// # Reading is not processing
///
/// `human_read = true` does not imply `ai_processing_allowed = true`. A person
/// reading `CONFIDENTIAL` material is bound by their obligations to the
/// institution; a model is a system whose retention and routing the institution
/// does not fully control (briefing §114).
///
/// `local_inference` is what changes the answer. Until an Ocinye node exists,
/// every model is somewhere the institution does not own, and the ceiling is
/// `INTERNAL`. When CAM-01 exists, the same call with `local_inference = true`
/// returns a higher ceiling with **no change to any caller** (briefing §116).
#[must_use]
pub const fn ai_processing_ceiling(local_inference: bool) -> Classification {
    if local_inference {
        Classification::Confidential
    } else {
        Classification::Internal
    }
}

/// Whether a classification may be sent for inference.
#[must_use]
pub fn may_process_with_ai(classification: Classification, local_inference: bool) -> bool {
    classification.level() <= ai_processing_ceiling(local_inference).level()
}

/// The risk a capability carries, given what it is being asked to do.
///
/// A dry run of a destructive capability describes rather than destroys, so it
/// is a read. The capability's declared risk still governs the real run.
#[must_use]
pub const fn effective_risk(descriptor: &CapabilityDescriptor, dry_run: bool) -> RiskLevel {
    if dry_run && descriptor.supports_dry_run {
        RiskLevel::ReadOnly
    } else {
        descriptor.risk
    }
}

/// Whether a permission is one an agent may ever be configured to use.
///
/// # The capabilities no agent gets
///
/// Some authority is not delegable to something that a sentence in an email
/// could influence. Changing who may access what, and managing credentials, are
/// acts a person performs deliberately — not outcomes of a conversation
/// (briefing §94, §149).
#[must_use]
pub const fn is_delegable_to_agents(permission: Permission) -> bool {
    !matches!(
        permission,
        Permission::PermissionsManage
            | Permission::RolesManage
            | Permission::MembersCreate
            | Permission::MembersManage
            | Permission::PlatformAdminister
            | Permission::AiInfrastructureManage
            | Permission::ComputeAdmin
            | Permission::MailAdminister
    )
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use ocinye_contracts::agentic::{ApprovalRequirement, CapabilityId, Reversibility};
    use ocinye_contracts::{Scope, TechnicalRole, WorkspaceRole};

    use super::*;
    use crate::policy::ResourceKind;

    const ORG: Uuid = Uuid::from_u128(1);
    const UNIT_A: Uuid = Uuid::from_u128(10);
    const UNIT_B: Uuid = Uuid::from_u128(11);
    const WS_A: Uuid = Uuid::from_u128(20);
    const WS_B: Uuid = Uuid::from_u128(21);

    fn person() -> Principal {
        Principal {
            subject: "sub-1".into(),
            person_id: Uuid::from_u128(100),
            organisation_id: ORG,
            display_name: "Test Person".into(),
            is_active: true,
            identity_kind: crate::IdentityKind::Human,
            roles: HashSet::new(),
            unit_roles: HashMap::new(),
            workspace_roles: HashMap::new(),
            grants: Vec::new(),
        }
    }

    /// A research member who leads workspace A.
    fn member() -> Principal {
        let mut principal = person();
        principal.roles.insert(TechnicalRole::ResearchMember);
        principal.workspace_roles.insert(WS_A, WorkspaceRole::Lead);
        principal
    }

    fn descriptor(
        id: &'static str,
        permission: Permission,
        risk: RiskLevel,
    ) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new(id),
            operation: ocinye_contracts::agentic::OperationId::new("test::fixture"),
            domain: id.split('.').next().unwrap_or("test").to_owned(),
            summary: "teste".to_owned(),
            permission,
            scope: Scope::Institution,
            risk,
            approval: ApprovalRequirement::Never,
            max_autonomy: AutonomyLevel::Workflow,
            reversibility: Reversibility::Reversible,
            supports_dry_run: true,
            classification_ceiling: None,
            input_schema: serde_json::json!({"type": "object"}),
        }
    }

    fn boundary(capabilities: &[&str]) -> AgentBoundary {
        AgentBoundary {
            allowed_capabilities: capabilities.iter().map(|c| (*c).to_owned()).collect(),
            classification_ceiling: Classification::Restricted,
            autonomy: AutonomyLevel::Workflow,
            unit_id: None,
            workspace_id: None,
        }
    }

    fn ctx(classification: Classification) -> ResourceContext {
        ResourceContext::workspace(ResourceKind::Idea, ORG, UNIT_A, WS_A, classification)
    }

    // ── The invariant ───────────────────────────────────────────────────

    /// The one that matters most: an agent cannot do what its actor cannot.
    ///
    /// Exhaustive over the permission catalogue. An agent is configured to
    /// allow *everything*, with the widest boundary the type permits, and is
    /// driven by a principal with no roles at all.
    #[test]
    fn an_agent_never_widens_its_actor() {
        let nobody = person();
        let permissive = AgentBoundary {
            allowed_capabilities: Permission::all()
                .iter()
                .map(|p| format!("test.{}", p.as_str().replace('.', "_")))
                .collect(),
            classification_ceiling: Classification::Restricted,
            autonomy: AutonomyLevel::Autonomous,
            unit_id: None,
            workspace_id: None,
        };

        for permission in Permission::all() {
            let id: &'static str = Box::leak(
                format!("test.{}", permission.as_str().replace('.', "_")).into_boxed_str(),
            );
            let descriptor = descriptor(id, permission, RiskLevel::LowImpact);

            assert_eq!(
                may_invoke(
                    &nobody,
                    &permissive,
                    &descriptor,
                    &ctx(Classification::Public),
                    None
                ),
                Err(AgenticRefusal::ActorLacksPermission),
                "um agente alcançou {permission:?} para quem não a tem"
            );
        }
    }

    /// The agent's own definition narrows further.
    #[test]
    fn a_capability_outside_the_agent_definition_is_refused() {
        let actor = member();
        let narrow = boundary(&["research.idea.create"]);

        // Dentro da definição: passa.
        let allowed = descriptor(
            "research.idea.create",
            Permission::IdeasView,
            RiskLevel::LowImpact,
        );
        assert!(may_invoke(
            &actor,
            &narrow,
            &allowed,
            &ctx(Classification::Internal),
            None
        )
        .is_ok());

        // Fora da definição, com a **mesma** permissão do actor: recusado.
        let outside = descriptor(
            "research.idea.archive",
            Permission::IdeasView,
            RiskLevel::LowImpact,
        );
        assert_eq!(
            may_invoke(
                &actor,
                &narrow,
                &outside,
                &ctx(Classification::Internal),
                None
            ),
            Err(AgenticRefusal::OutsideAgentBoundary)
        );
    }

    /// An agent bound to one workspace does not act in another.
    #[test]
    fn a_bound_agent_does_not_reach_another_workspace() {
        let mut actor = member();
        // O actor alcança os dois workspaces.
        actor.workspace_roles.insert(WS_B, WorkspaceRole::Lead);

        let mut bound = boundary(&["research.idea.create"]);
        bound.workspace_id = Some(WS_A);

        let descriptor = descriptor(
            "research.idea.create",
            Permission::IdeasView,
            RiskLevel::LowImpact,
        );

        assert!(may_invoke(
            &actor,
            &bound,
            &descriptor,
            &ctx(Classification::Internal),
            None
        )
        .is_ok());

        let elsewhere = ResourceContext::workspace(
            ResourceKind::Idea,
            ORG,
            UNIT_A,
            WS_B,
            Classification::Internal,
        );
        assert_eq!(
            may_invoke(&actor, &bound, &descriptor, &elsewhere, None),
            Err(AgenticRefusal::OutsideAgentScope),
            "um agente ligado a um workspace agiu noutro"
        );
    }

    #[test]
    fn a_unit_bound_agent_does_not_cross_units() {
        let actor = member();
        let mut bound = boundary(&["research.idea.create"]);
        bound.unit_id = Some(UNIT_B);

        let descriptor = descriptor(
            "research.idea.create",
            Permission::IdeasView,
            RiskLevel::LowImpact,
        );

        assert_eq!(
            may_invoke(
                &actor,
                &bound,
                &descriptor,
                &ctx(Classification::Internal),
                None
            ),
            Err(AgenticRefusal::OutsideAgentScope)
        );
    }

    // ── Classification ──────────────────────────────────────────────────

    #[test]
    fn the_agent_ceiling_and_the_capability_ceiling_both_bite() {
        let mut actor = member();
        actor.roles.insert(TechnicalRole::PlatformAdmin);

        let mut capped = boundary(&["knowledge.search"]);
        capped.classification_ceiling = Classification::Internal;

        let descriptor = descriptor(
            "knowledge.search",
            Permission::BibliographyView,
            RiskLevel::ReadOnly,
        );

        assert_eq!(
            may_invoke(
                &actor,
                &capped,
                &descriptor,
                &ctx(Classification::Confidential),
                None
            ),
            Err(AgenticRefusal::ClassificationRefused),
            "o tecto do agente não travou material acima dele"
        );

        // Agora o tecto da própria capability, com o agente sem tecto.
        let mut ceilinged = descriptor.clone();
        ceilinged.classification_ceiling = Some(Classification::Public);
        assert_eq!(
            may_invoke(
                &actor,
                &boundary(&["knowledge.search"]),
                &ceilinged,
                &ctx(Classification::Internal),
                None
            ),
            Err(AgenticRefusal::ClassificationRefused),
            "o tecto da capability não travou material acima dele"
        );
    }

    // ── Autonomy ────────────────────────────────────────────────────────

    #[test]
    fn an_agent_below_act_may_read_but_not_change() {
        let actor = member();
        let mut composer = boundary(&["research.idea.create", "knowledge.search"]);
        composer.autonomy = AutonomyLevel::Compose;

        // Ler é sempre permitido: uma leitura não altera nada.
        let read = descriptor(
            "knowledge.search",
            Permission::BibliographyView,
            RiskLevel::ReadOnly,
        );
        assert!(may_invoke(
            &actor,
            &composer,
            &read,
            &ctx(Classification::Internal),
            None
        )
        .is_ok());

        // Alterar não.
        let write = descriptor(
            "research.idea.create",
            Permission::IdeasView,
            RiskLevel::LowImpact,
        );
        assert_eq!(
            may_invoke(
                &actor,
                &composer,
                &write,
                &ctx(Classification::Internal),
                None
            ),
            Err(AgenticRefusal::AutonomyTooLow)
        );
    }

    #[test]
    fn the_lower_of_agent_and_capability_autonomy_wins() {
        let actor = member();

        // O agente pode tudo; a capability não passa de `Compose`.
        let mut cautious = descriptor("mail.send", Permission::MailSend, RiskLevel::ExternalEffect);
        cautious.max_autonomy = AutonomyLevel::Compose;

        assert_eq!(
            may_invoke(
                &actor,
                &boundary(&["mail.send"]),
                &cautious,
                &ctx(Classification::Internal),
                None
            ),
            Err(AgenticRefusal::AutonomyTooLow),
            "o tecto de autonomia da capability foi ignorado"
        );
    }

    // ── Approval ────────────────────────────────────────────────────────

    #[test]
    fn external_and_privileged_always_need_a_person_whatever_the_agent_says() {
        let permissive = boundary(&["mail.send"]);

        for risk in [RiskLevel::ExternalEffect, RiskLevel::Privileged] {
            let mut reckless = descriptor("mail.send", Permission::MailSend, risk);
            reckless.approval = ApprovalRequirement::Never;

            assert!(
                approval_needed(&reckless, &permissive),
                "{risk:?} correu sem confirmação"
            );
        }
    }

    #[test]
    fn a_cautious_agent_asks_even_where_the_capability_would_not() {
        let mut composer = boundary(&["research.idea.create"]);
        composer.autonomy = AutonomyLevel::Compose;

        let ordinary = descriptor(
            "research.idea.create",
            Permission::IdeasView,
            RiskLevel::LowImpact,
        );

        assert!(!approval_needed(
            &ordinary,
            &boundary(&["research.idea.create"])
        ));
        assert!(approval_needed(&ordinary, &composer));
    }

    #[test]
    fn reading_never_needs_confirmation() {
        let read = descriptor(
            "knowledge.search",
            Permission::BibliographyView,
            RiskLevel::ReadOnly,
        );
        assert!(!approval_needed(&read, &boundary(&["knowledge.search"])));
    }

    // ── AI processing ───────────────────────────────────────────────────

    #[test]
    fn reading_something_is_not_permission_to_feed_it_to_a_model() {
        // Sem nó local — o estado desta instalação.
        assert!(may_process_with_ai(Classification::Public, false));
        assert!(may_process_with_ai(Classification::Internal, false));
        assert!(!may_process_with_ai(Classification::Confidential, false));
        assert!(!may_process_with_ai(Classification::Restricted, false));
    }

    #[test]
    fn a_local_node_raises_the_ceiling_without_removing_it() {
        // Quando CAM-01 existir, o mesmo código responde diferente — e
        // `RESTRICTED` continua fora, porque isso exige política própria.
        assert!(may_process_with_ai(Classification::Confidential, true));
        assert!(!may_process_with_ai(Classification::Restricted, true));

        assert_eq!(ai_processing_ceiling(false), Classification::Internal);
        assert_eq!(ai_processing_ceiling(true), Classification::Confidential);
    }

    // ── Dry run ─────────────────────────────────────────────────────────

    #[test]
    fn a_dry_run_of_a_destructive_capability_is_a_read() {
        let destructive = descriptor(
            "data.dataset.archive",
            Permission::DatasetsManage,
            RiskLevel::Privileged,
        );

        assert_eq!(effective_risk(&destructive, true), RiskLevel::ReadOnly);
        assert_eq!(effective_risk(&destructive, false), RiskLevel::Privileged);
    }

    #[test]
    fn dry_run_does_not_downgrade_a_capability_that_cannot_simulate() {
        let mut no_simulation =
            descriptor("mail.send", Permission::MailSend, RiskLevel::ExternalEffect);
        no_simulation.supports_dry_run = false;

        // Pedir simulação a algo que não sabe simular não torna o envio seguro.
        assert_eq!(
            effective_risk(&no_simulation, true),
            RiskLevel::ExternalEffect
        );
    }

    // ── Non-delegable authority ─────────────────────────────────────────

    #[test]
    fn some_authority_is_never_delegated_to_an_agent() {
        for permission in [
            Permission::PermissionsManage,
            Permission::RolesManage,
            Permission::MembersCreate,
            Permission::MembersManage,
            Permission::PlatformAdminister,
            Permission::AiInfrastructureManage,
            Permission::ComputeAdmin,
            Permission::MailAdminister,
        ] {
            assert!(
                !is_delegable_to_agents(permission),
                "{permission:?} passou a ser delegável a um agente"
            );
        }

        // E o trabalho normal continua delegável.
        assert!(is_delegable_to_agents(Permission::IdeasView));
        assert!(is_delegable_to_agents(Permission::MailSend));
        assert!(is_delegable_to_agents(Permission::BibliographyView));
    }

    // ── The Main Agent ──────────────────────────────────────────────────

    #[test]
    fn the_main_agent_is_wide_but_not_privileged() {
        let main = AgentBoundary::main_agent(vec!["administration.member.suspend".to_owned()]);
        let nobody = person();

        let privileged = descriptor(
            "administration.member.suspend",
            Permission::MembersManage,
            RiskLevel::Privileged,
        );

        // A definição admite; o actor não. O actor ganha.
        assert!(main.permits_capability("administration.member.suspend"));
        assert_eq!(
            may_invoke(
                &nobody,
                &main,
                &privileged,
                &ctx(Classification::Public),
                None
            ),
            Err(AgenticRefusal::ActorLacksPermission)
        );
    }

    #[test]
    fn refusals_are_distinguishable_and_none_maps_the_boundary() {
        // Cada recusa tem representação estável própria, para auditoria.
        let refusals = [
            AgenticRefusal::ActorLacksPermission,
            AgenticRefusal::OutsideAgentBoundary,
            AgenticRefusal::AutonomyTooLow,
            AgenticRefusal::ClassificationRefused,
            AgenticRefusal::OutsideAgentScope,
        ];

        let mut labels: Vec<&str> = refusals.iter().map(|r| r.as_str()).collect();
        labels.sort_unstable();
        let count = labels.len();
        labels.dedup();
        assert_eq!(labels.len(), count);

        // E nenhuma mensagem diz o que *seria* possível com mais autoridade:
        // isso é um mapa da fronteira, desenhado para quem a está a sondar.
        for refusal in refusals {
            let message = refusal.message();
            assert!(!message.contains("permissão necessária"));
            assert!(!message.is_empty());
        }
    }
}
