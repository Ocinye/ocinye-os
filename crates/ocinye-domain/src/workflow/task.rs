//! Task lifecycle.

use ocinye_contracts::TaskState;

use crate::error::{DomainError, DomainResult};

/// States reachable from `current`.
#[must_use]
pub(crate) fn task_targets_from(current: TaskState) -> &'static [TaskState] {
    use TaskState::{Blocked, Cancelled, Done, InProgress, InReview, Todo};
    match current {
        Todo => &[InProgress, Blocked, Cancelled],
        InProgress => &[Blocked, InReview, Done, Cancelled],
        Blocked => &[InProgress, Cancelled],
        InReview => &[InProgress, Done],
        // Reopening finished work is normal; both closed states allow it.
        Done => &[InProgress],
        Cancelled => &[Todo],
    }
}

/// Validate a task transition.
///
/// # Errors
///
/// Returns [`DomainError::InvalidTransition`] when the lifecycle forbids it.
pub fn assert_task_transition(current: TaskState, target: TaskState) -> DomainResult<()> {
    if task_targets_from(current).contains(&target) {
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
    fn work_cannot_jump_straight_to_done() {
        assert!(assert_task_transition(TaskState::Todo, TaskState::Done).is_err());
    }

    #[test]
    fn closed_tasks_can_be_reopened() {
        assert!(assert_task_transition(TaskState::Done, TaskState::InProgress).is_ok());
        assert!(assert_task_transition(TaskState::Cancelled, TaskState::Todo).is_ok());
    }

    #[test]
    fn no_state_is_a_dead_end() {
        for from in TaskState::all() {
            assert!(
                !task_targets_from(from).is_empty(),
                "{from:?} is a dead end"
            );
        }
    }
}
