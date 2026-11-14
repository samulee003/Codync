//! What a task cost: when it was first claimed (`started_at`), when its work last counted as finished (`finished_at`)
//! and how many turns the host started for it (`turns`). The board keeps these on the task, so the apps show the effort
//! next to it.

use super::{Task, TaskStatus, get, save_task};
use crate::LockExt;
use crate::hub::Hub;
use crate::store::now_ms;

/// The first claim starts the clock; a later claim keeps it.
pub(super) fn stamp_started(task: &mut Task) {
    task.started_at.get_or_insert_with(now_ms);
}

/// Work counts as finished once it reaches review or done.
pub(super) fn stamp_finished(task: &mut Task) {
    if matches!(task.status, TaskStatus::Review | TaskStatus::Done) {
        task.finished_at = Some(now_ms());
    }
}

/// Counts a turn the host starts for a task, under the board lock. The task's `updated_at` stays as it is, so a stale
/// nudge doesn't reset the clock that releases the task. A task that is gone has no turn to count.
pub(super) fn count_turn(hub: &Hub, id: &str) {
    let _g = hub.board.lock.locked();
    let Ok(mut task) = get(&hub.store, id) else { return };
    task.turns = task.turns.saturating_add(1);
    if let Err(error) = save_task(hub, &mut task) {
        tracing::warn!(task = id, error = format!("{error:#}"), "turn not counted");
    }
}
