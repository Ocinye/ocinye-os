//! Project lifecycle.

use ocinye_contracts::ProjectState;

use crate::error::{DomainError, DomainResult};

/// States reachable from `current`.
#[must_use]
pub fn project_targets_from(current: ProjectState) -> &'static [ProjectState] {
    use ProjectState::{Active, Archived, Completed, Draft, OnHold};
    match current {
        Draft => &[Active, Archived],
        Active => &[OnHold, Completed, Archived],
        OnHold => &[Active, Archived],
        // Reopening a completed project is legitimate; it happens.
        Completed => &[Active, Archived],
        // Archival is terminal: the record stands.
        Archived => &[],
    }
}

/// Validate a project transition.
///
/// # Errors
///
/// Returns [`DomainError::InvalidTransition`] when the lifecycle forbids it.
pub fn assert_project_transition(current: ProjectState, target: ProjectState) -> DomainResult<()> {
    if project_targets_from(current).contains(&target) {
        Ok(())
    } else {
        Err(DomainError::InvalidTransition {
            from: current.as_str(),
            to: target.as_str(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draft_project_cannot_be_completed_without_running() {
        assert!(assert_project_transition(ProjectState::Draft, ProjectState::Completed).is_err());
        assert!(assert_project_transition(ProjectState::Draft, ProjectState::Active).is_ok());
    }

    #[test]
    fn archived_is_terminal() {
        assert!(project_targets_from(ProjectState::Archived).is_empty());
        for target in ProjectState::all() {
            assert!(assert_project_transition(ProjectState::Archived, target).is_err());
        }
    }

    #[test]
    fn every_live_state_can_reach_archived() {
        for from in ProjectState::all() {
            if from == ProjectState::Archived {
                continue;
            }
            assert!(
                project_targets_from(from).contains(&ProjectState::Archived),
                "{from:?} must be archivable"
            );
        }
    }

    #[test]
    fn transitions_never_target_the_current_state() {
        for from in ProjectState::all() {
            assert!(!project_targets_from(from).contains(&from));
        }
    }
}
