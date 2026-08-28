//! Idea lifecycle.

use ocinye_contracts::IdeaState;

use crate::error::{DomainError, DomainResult};

/// The only state from which an idea may be promoted to a project.
pub const PROMOTABLE_FROM: IdeaState = IdeaState::ProjectCandidate;

/// States reachable from `current` by an ordinary transition.
///
/// The flow moves forward, allows stepping back when a review sends work
/// backwards, and always offers the two honest exits. `Promoted` is absent from
/// every list: it is reachable only through promotion, so a client cannot claim
/// a project exists by moving a state.
#[must_use]
pub fn idea_targets_from(current: IdeaState) -> &'static [IdeaState] {
    use IdeaState::{
        Archived, Concept, Discovery, Exploration, ProjectCandidate, Promoted, Rejected, Review,
    };
    match current {
        Discovery => &[Exploration, Rejected, Archived],
        Exploration => &[Concept, Discovery, Rejected, Archived],
        Concept => &[Review, Exploration, Rejected, Archived],
        Review => &[ProjectCandidate, Concept, Rejected, Archived],
        ProjectCandidate => &[Review, Rejected, Archived],
        // Terminal once a project exists: the lineage must not be rewritten.
        Promoted => &[],
        // Closing is not deletion. An idea can be picked up again.
        Rejected | Archived => &[Discovery],
    }
}

/// Whether closing into this state requires a recorded reason.
///
/// Why an idea was dropped is institutional memory, not noise (briefing §27).
#[must_use]
pub const fn requires_outcome_note(target: IdeaState) -> bool {
    matches!(target, IdeaState::Rejected | IdeaState::Archived)
}

/// Validate an ordinary idea transition.
///
/// # Errors
///
/// Returns [`DomainError::TransitionRequiresOperation`] when `Promoted` is
/// requested directly, [`DomainError::Validation`] when a closing transition
/// carries no reason, and [`DomainError::InvalidTransition`] otherwise.
pub fn assert_idea_transition(
    current: IdeaState,
    target: IdeaState,
    outcome_note: Option<&str>,
) -> DomainResult<()> {
    if target == IdeaState::Promoted {
        return Err(DomainError::TransitionRequiresOperation(
            "An idea becomes 'promoted' only by being promoted to a project.".into(),
        ));
    }

    if !idea_targets_from(current).contains(&target) {
        return Err(DomainError::InvalidTransition {
            from: current.as_str(),
            to: target.as_str(),
        });
    }

    if requires_outcome_note(target) && outcome_note.is_none_or(|note| note.trim().is_empty()) {
        return Err(DomainError::Validation(format!(
            "Moving an idea to '{}' requires a reason: why it was closed is institutional memory.",
            target.as_str()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_happy_path_runs_to_candidate() {
        let path = [
            (IdeaState::Discovery, IdeaState::Exploration),
            (IdeaState::Exploration, IdeaState::Concept),
            (IdeaState::Concept, IdeaState::Review),
            (IdeaState::Review, IdeaState::ProjectCandidate),
        ];
        for (from, to) in path {
            assert!(
                assert_idea_transition(from, to, None).is_ok(),
                "{from:?} -> {to:?}"
            );
        }
    }

    #[test]
    fn promotion_is_not_reachable_as_an_ordinary_transition() {
        for from in IdeaState::all() {
            let result = assert_idea_transition(from, IdeaState::Promoted, None);
            assert!(matches!(
                result,
                Err(DomainError::TransitionRequiresOperation(_))
            ));
            assert!(!idea_targets_from(from).contains(&IdeaState::Promoted));
        }
    }

    #[test]
    fn a_promoted_idea_is_terminal() {
        assert!(idea_targets_from(IdeaState::Promoted).is_empty());
        for to in IdeaState::all() {
            assert!(assert_idea_transition(IdeaState::Promoted, to, Some("x")).is_err());
        }
    }

    #[test]
    fn closing_an_idea_demands_a_reason() {
        for target in [IdeaState::Rejected, IdeaState::Archived] {
            assert!(matches!(
                assert_idea_transition(IdeaState::Discovery, target, None),
                Err(DomainError::Validation(_))
            ));
            assert!(matches!(
                assert_idea_transition(IdeaState::Discovery, target, Some("   ")),
                Err(DomainError::Validation(_))
            ));
            assert!(
                assert_idea_transition(IdeaState::Discovery, target, Some("no funding")).is_ok()
            );
        }
    }

    #[test]
    fn every_state_can_be_closed_or_is_deliberately_terminal() {
        for from in IdeaState::all() {
            let targets = idea_targets_from(from);
            if from == IdeaState::Promoted {
                assert!(targets.is_empty());
                continue;
            }
            if from.is_closed_without_project() {
                assert!(
                    targets.contains(&IdeaState::Discovery),
                    "{from:?} must be reopenable"
                );
                continue;
            }
            assert!(
                targets.contains(&IdeaState::Rejected) && targets.contains(&IdeaState::Archived),
                "{from:?} must offer an honest exit"
            );
        }
    }

    #[test]
    fn transitions_never_target_the_current_state() {
        for from in IdeaState::all() {
            assert!(
                !idea_targets_from(from).contains(&from),
                "{from:?} loops on itself"
            );
        }
    }
}
