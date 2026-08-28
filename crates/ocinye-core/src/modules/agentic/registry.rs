//! The Ocinye Capability Registry.
//!
//! # A closed set, defined in code
//!
//! The registry is built once, from a fixed list. It is not a table, and that
//! is deliberate: a capability set editable at runtime is a capability set no
//! test can pin, and this is the layer where an exhaustive test is worth most.
//! Adding a capability is a code change that goes through review — which is
//! exactly the amount of ceremony that granting an agent a new power deserves.
//!
//! # What a model is allowed to see
//!
//! Never the whole registry. [`CapabilityRegistry::available_to`] filters by
//! what the acting person could hold and by what the current context is about,
//! because sending sixty descriptors to plan «create a task» wastes context and
//! hands out a map of the system (briefing §21, §138).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use ocinye_contracts::agentic::{CapabilityDescriptor, CapabilityId, CapabilityResult};
use ocinye_domain::{is_delegable_to_agents, Principal};

use super::executor::ExecutionContext;
use crate::error::CoreResult;

/// Something the Core can be asked to do.
///
/// # Why a trait and not a function pointer
///
/// A handler owns its input type. It deserialises the proposed input into a
/// concrete struct and fails cleanly when that does not fit, which is where
/// «the model made up an argument» becomes a validation error instead of a
/// surprise three layers down (briefing §23, §173).
#[async_trait]
pub trait CapabilityHandler: Send + Sync {
    /// What the Core publishes about this capability.
    fn descriptor(&self) -> CapabilityDescriptor;

    /// Do it.
    ///
    /// Called **only** after the executor has validated the input against the
    /// schema, resolved the context and confirmed the acting person may do
    /// this. A handler never repeats those checks and never skips the domain
    /// service that owns the invariant.
    ///
    /// # Errors
    ///
    /// Returns whatever the domain service returns. The executor translates.
    async fn execute(&self, ctx: &ExecutionContext<'_>) -> CoreResult<CapabilityResult>;
}

/// Every capability the Ocinye OS publishes.
pub struct CapabilityRegistry {
    entries: BTreeMap<String, Arc<dyn CapabilityHandler>>,
}

impl CapabilityRegistry {
    /// Build the registry.
    fn build() -> Self {
        let handlers: Vec<Arc<dyn CapabilityHandler>> = super::capabilities::all();

        let mut entries = BTreeMap::new();
        for handler in handlers {
            let descriptor = handler.descriptor();

            // A capability whose permission is not delegable would be an agent
            // holding authority nobody meant to give it. Refusing at startup
            // rather than at call time means it cannot ship unnoticed.
            assert!(
                is_delegable_to_agents(descriptor.permission),
                "capability `{}` requires `{}`, which is not delegable to agents",
                descriptor.id,
                descriptor.permission.as_str()
            );

            let previous = entries.insert(descriptor.id.as_str().to_owned(), handler);
            assert!(
                previous.is_none(),
                "two capabilities registered as `{}`",
                descriptor.id
            );
        }

        Self { entries }
    }

    /// Look one up.
    ///
    /// A well-formed identifier for something that does not exist returns
    /// `None`. That is the whole defence against a model inventing a capability
    /// name: the registry is the only thing that knows what exists
    /// (briefing §161).
    #[must_use]
    pub fn get(&self, id: &CapabilityId) -> Option<&Arc<dyn CapabilityHandler>> {
        self.entries.get(id.as_str())
    }

    /// Every descriptor, for the administration screen.
    #[must_use]
    pub fn all(&self) -> Vec<CapabilityDescriptor> {
        self.entries
            .values()
            .map(|handler| handler.descriptor())
            .collect()
    }

    /// How many capabilities exist.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty. It never is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The descriptors this person could plausibly use.
    ///
    /// # A filter, not an authorisation
    ///
    /// This narrows what a model is shown to what the acting person holds the
    /// permission for at institutional scope. It is **not** the access
    /// decision: a capability that survives this filter is still checked in
    /// full, against the real resource, before it runs. Hiding is courtesy;
    /// the refusal is what protects.
    #[must_use]
    pub fn available_to(
        &self,
        principal: &Principal,
        domains: Option<&[&str]>,
    ) -> Vec<CapabilityDescriptor> {
        let institution = ocinye_domain::ResourceContext::organisation(
            ocinye_domain::ResourceKind::Person,
            principal.organisation_id,
        );

        self.entries
            .values()
            .map(|handler| handler.descriptor())
            .filter(|descriptor| {
                domains.is_none_or(|wanted| wanted.contains(&descriptor.domain.as_str()))
            })
            .filter(|descriptor| {
                ocinye_domain::can(principal, descriptor.permission, &institution, None).allowed
            })
            .collect()
    }
}

/// The registry, built once.
pub fn registry() -> &'static CapabilityRegistry {
    static REGISTRY: OnceLock<CapabilityRegistry> = OnceLock::new();
    REGISTRY.get_or_init(CapabilityRegistry::build)
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use ocinye_contracts::agentic::RiskLevel;
    use ocinye_contracts::{Permission, TechnicalRole};
    use uuid::Uuid;

    use super::*;

    fn person(roles: &[TechnicalRole]) -> Principal {
        Principal {
            subject: "sub".into(),
            person_id: Uuid::from_u128(1),
            organisation_id: Uuid::from_u128(2),
            display_name: "Test".into(),
            is_active: true,
            roles: roles.iter().copied().collect::<HashSet<_>>(),
            unit_roles: HashMap::new(),
            workspace_roles: HashMap::new(),
            grants: Vec::new(),
        }
    }

    /// The registry builds, which means every assertion in `build` held.
    #[test]
    fn the_registry_builds_and_is_not_empty() {
        let registry = registry();
        assert!(!registry.is_empty());
    }

    /// No capability may carry authority that is not delegable.
    ///
    /// `build` asserts this, so a violation is a startup panic rather than a
    /// silent grant. This test states it as a property so the reason is
    /// findable.
    #[test]
    fn no_capability_carries_non_delegable_authority() {
        for descriptor in registry().all() {
            assert!(
                is_delegable_to_agents(descriptor.permission),
                "`{}` exige `{}`, que não é delegável",
                descriptor.id,
                descriptor.permission.as_str()
            );
        }
    }

    /// Every identifier is well-formed and matches its declared domain.
    #[test]
    fn identifiers_are_well_formed_and_agree_with_their_domain() {
        for descriptor in registry().all() {
            assert!(
                CapabilityId::parse(descriptor.id.as_str()).is_some(),
                "`{}` não é um identificador válido",
                descriptor.id
            );
            assert_eq!(
                descriptor.id.domain(),
                descriptor.domain,
                "`{}` declara o domínio `{}`",
                descriptor.id,
                descriptor.domain
            );
        }
    }

    /// Risk and approval agree with each other.
    #[test]
    fn nothing_external_or_privileged_runs_without_a_person() {
        for descriptor in registry().all() {
            if descriptor.risk.always_requires_approval() {
                assert!(
                    descriptor.requires_approval(),
                    "`{}` é {:?} e não exige confirmação",
                    descriptor.id,
                    descriptor.risk
                );
            }
        }
    }

    /// A capability that says it can simulate must not be a pure read.
    ///
    /// A dry run of something that changes nothing is the operation itself, and
    /// offering it suggests a distinction that is not there.
    #[test]
    fn dry_run_is_only_offered_where_there_is_something_to_simulate() {
        for descriptor in registry().all() {
            if descriptor.supports_dry_run {
                assert_ne!(
                    descriptor.risk,
                    RiskLevel::ReadOnly,
                    "`{}` é uma leitura e oferece simulação",
                    descriptor.id
                );
            }
        }
    }

    /// Undo is never promised where undo does not exist.
    #[test]
    fn irreversible_capabilities_do_not_claim_reversibility() {
        for descriptor in registry().all() {
            if descriptor.risk == RiskLevel::ReadOnly {
                assert_eq!(
                    descriptor.reversibility,
                    ocinye_contracts::agentic::Reversibility::NothingToUndo,
                    "`{}` não altera nada e declara reversibilidade",
                    descriptor.id
                );
            }
        }
    }

    /// A well-formed name for something that does not exist resolves to nothing.
    #[test]
    fn an_invented_capability_resolves_to_nothing() {
        let invented = CapabilityId::parse("mail.delete_everything").expect("bem formado");
        assert!(
            registry().get(&invented).is_none(),
            "o registry devolveu um handler para uma capability inventada"
        );
    }

    /// The filter narrows by what the person holds.
    #[test]
    fn the_filter_shows_less_to_someone_who_holds_less() {
        let member = person(&[TechnicalRole::ResearchMember]);
        let nobody = person(&[]);

        let for_member = registry().available_to(&member, None);
        let for_nobody = registry().available_to(&nobody, None);

        assert!(
            for_nobody.len() < for_member.len(),
            "alguém sem papéis viu tanto quanto um membro de investigação"
        );

        // E quem não tem papéis nenhuns não vê nada que exija permissão.
        for descriptor in &for_nobody {
            assert!(
                !matches!(
                    descriptor.permission,
                    Permission::MailSend | Permission::IdeasView
                ),
                "`{}` apareceu a quem não tem papéis",
                descriptor.id
            );
        }
    }

    /// Filtering by domain narrows further.
    #[test]
    fn the_filter_narrows_by_domain() {
        let member = person(&[TechnicalRole::ResearchMember]);

        let everything = registry().available_to(&member, None);
        let mail_only = registry().available_to(&member, Some(&["mail"]));

        assert!(mail_only.len() <= everything.len());
        for descriptor in &mail_only {
            assert_eq!(descriptor.domain, "mail");
        }
    }

    /// Hiding is courtesy; the refusal is what protects.
    ///
    /// A capability the filter omits is still resolvable by identifier — the
    /// executor is what refuses it. This test exists so nobody later mistakes
    /// the filter for the access decision.
    #[test]
    fn the_filter_is_not_the_access_decision() {
        let nobody = person(&[]);
        let hidden = registry().available_to(&nobody, None);

        for descriptor in registry().all() {
            let shown = hidden.iter().any(|d| d.id == descriptor.id);
            if !shown {
                assert!(
                    registry().get(&descriptor.id).is_some(),
                    "o registry deixou de resolver `{}` por estar filtrada",
                    descriptor.id
                );
            }
        }
    }
}
