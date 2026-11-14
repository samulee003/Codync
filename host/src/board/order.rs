//! Tasks in order: a task can wait for others (`after`), such as the part that builds on another part. It can't be
//! claimed until every task it waits for is finished, nudges leave it out until then, and a task handed to a teammate
//! in advance (`assignee`) is given to it the moment it is ready, so a plan's parts start in the right order on their
//! own. A task waits only for tasks that existed when it was added, so the order never loops.

use super::{Actor, Task, TaskStatus};
use crate::hub::Hub;
use crate::store::Store;
use anyhow::{Result, anyhow, ensure};
use serde_json::Value;

/// Tasks one task may wait for.
const MAX_AFTER: usize = 10;

/// Whether a task another waits for is out of the way: done, cancelled, or finished with no branch left to merge (a
/// branch must be merged first, so the next part builds on its work).
pub(super) fn finished(task: &Task) -> bool {
    match task.status {
        TaskStatus::Done | TaskStatus::Cancelled => true,
        TaskStatus::Review => task.branch.is_none(),
        TaskStatus::Open | TaskStatus::Claimed | TaskStatus::Checking => false,
    }
}

/// The tasks `task` still waits for (one deleted since no longer counts).
pub fn waiting_on(store: &Store, task: &Task) -> Vec<Task> {
    task.after.iter().filter_map(|id| super::get(store, id).ok()).filter(|t| !finished(t)).collect()
}

/// The tasks a new task waits for, from a client or a bot: ids of tasks on the board.
pub fn after(store: &Store, value: &Value) -> Result<Vec<String>> {
    let ids: Vec<&str> = match value {
        Value::Null => vec![],
        Value::Array(items) => {
            items.iter().map(|v| v.as_str().ok_or_else(|| anyhow!("after lists task ids"))).collect::<Result<_>>()?
        }
        _ => anyhow::bail!("after lists task ids"),
    };
    ensure!(ids.len() <= MAX_AFTER, "a task waits for at most {MAX_AFTER} others");
    let mut out: Vec<String> = Vec::new();
    for id in ids.into_iter().map(str::trim).filter(|id| !id.is_empty()) {
        let task =
            super::get(store, id).map_err(|_| anyhow!("after: no task {id} (use the ids create_task answers)"))?;
        if !out.contains(&task.id) {
            out.push(task.id);
        }
    }
    Ok(out)
}

/// Refuses work on a task that still waits for others.
pub(super) fn ensure_ready(store: &Store, task: &Task) -> Result<()> {
    let waiting = waiting_on(store, task);
    ensure!(
        waiting.is_empty(),
        "this task waits for {}: it can be claimed once that is finished",
        waiting.iter().map(|t| format!("\"{}\"", t.title)).collect::<Vec<_>>().join(", ")
    );
    Ok(())
}

/// Hands each open task that was given to a teammate in advance to it, once nothing it waits for is left. Answers the
/// tasks handed on and told.
pub async fn release_ready(hub: &Hub) -> Vec<String> {
    let ready: Vec<Task> = super::list(&hub.store)
        .into_iter()
        .filter(|t| t.status == TaskStatus::Open && t.assignee.is_some() && waiting_on(&hub.store, t).is_empty())
        .collect();
    let mut handed = Vec::new();
    for task in ready {
        let Some(bot) = task.assignee.as_deref() else { continue };
        let by = task.created_by.as_deref().map_or(Actor::User, Actor::Bot);
        match super::assign(hub, &task.id, bot, by).await {
            Ok(_) => handed.push(task.id),
            Err(error) => {
                tracing::warn!(task = task.id, error = format!("{error:#}"), "waiting task not handed on");
                // Not retried: a task left open goes to whichever bot claims it.
                let _ = super::change(hub, &task.id, |t| {
                    t.assignee = None;
                    Ok(())
                });
            }
        }
    }
    handed
}
