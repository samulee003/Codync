//! A claimed task that sits unchanged for a while: its idle claimer is nudged once to carry on or release it, and if it
//! still hasn't moved after another wait, the task goes back to the board for another bot. A task waiting on the user
//! (its questions or its plan) or on its unfinished subtasks is never stuck.

use super::brief::Approval;
use super::dispatch::free;
use super::{Actor, Task, TaskStatus, children, effort, release};
use crate::LockExt;
use crate::agent::bot::Cmd;
use crate::hub::Hub;
use crate::store::now_ms;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a claimed task may sit unchanged before its claimer is nudged, and how long after that before it is
/// released.
pub(super) const STALE: Duration = Duration::from_mins(20);

/// The stuck tasks a bot was nudged about, by task id, and when.
#[derive(Default)]
pub struct Poked(pub(super) Mutex<HashMap<String, Instant>>);

/// Whether a claimed task has sat unchanged for longer than `STALE` at `now` (ms). `parts_open`: a subtask of it is
/// still unfinished, so its claimer waits for the parts.
pub(super) fn stale(task: &Task, parts_open: bool, now: i64) -> bool {
    let limit = i64::try_from(STALE.as_millis()).unwrap_or(i64::MAX);
    task.status == TaskStatus::Claimed
        && !matches!(task.approval, Approval::Asking | Approval::Proposed)
        && !parts_open
        && now.saturating_sub(task.updated_at) > limit
}

/// Runs each dispatch tick.
pub fn check(hub: &Hub) {
    sweep(hub, now_ms(), Instant::now());
}

/// One pass at `now` (ms) and `at`: nudges each stale task whose claimer is idle or errored and that wasn't nudged yet,
/// and releases one already nudged `STALE` ago that still hasn't moved. A task that moved again is forgotten.
pub(super) fn sweep(hub: &Hub, now: i64, at: Instant) {
    let claimed: Vec<Task> = super::list(&hub.store).into_iter().filter(|t| t.status == TaskStatus::Claimed).collect();
    let mut nudges = Vec::new();
    let mut releases = Vec::new();
    {
        let mut poked = hub.board.stale.0.locked();
        poked.retain(|id, _| claimed.iter().any(|t| t.id == *id));
        for task in &claimed {
            let Some(bot) = task.claimed_by.as_deref() else { continue };
            let parts_open = children(&hub.store, &task.id).iter().any(|t| t.status.unfinished());
            if !stale(task, parts_open, now) {
                poked.remove(&task.id);
                continue;
            }
            if !free(hub.runtime(bot).status) {
                continue;
            }
            match poked.get(&task.id).copied() {
                None => {
                    poked.insert(task.id.clone(), at);
                    nudges.push((bot.to_owned(), task.clone()));
                }
                Some(poked_at) if at.duration_since(poked_at) >= STALE => {
                    poked.remove(&task.id);
                    releases.push(task.id.clone());
                }
                Some(_) => {}
            }
        }
    }
    for (bot, task) in nudges {
        let text = format!(
            "[Codync: task {} ({}) is still claimed for you and hasn't moved for a while. Carry on and finish_task it, \
             or release_task it with why, so it isn't left waiting.]",
            task.id, task.title
        );
        effort::count_turn(hub, &task.id);
        if let Err(error) = hub.send_cmd(&bot, Cmd::Prompt { text, call: None }) {
            tracing::warn!(bot, error = format!("{error:#}"), "stale task nudge not delivered");
        }
    }
    for id in releases {
        match release(hub, &id, Actor::User, Some("no progress for a while")) {
            Ok(_) => tracing::info!(task = %id, "stale task released to the board"),
            Err(error) => tracing::warn!(task = %id, error = format!("{error:#}"), "stale task not released"),
        }
    }
}
