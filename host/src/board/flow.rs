//! How work moves between the bots on the board, beyond claiming: the lead plans each task the user adds (splitting a
//! job with parts others can do side by side into subtasks it then coordinates), another bot checks finished work
//! before it counts, and a coordinator hears when all its subtasks are finished so it assembles the result.

use super::{MAX_NOTE, MAX_NOTES, Note, Task, TaskStatus, change, children, dispatch, get, take, text, wire};
use crate::agent::bot::Cmd;
use crate::hub::{BotStatus, Hub};
use crate::store::now_ms;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::fmt::Write as _;

/// Longest description of a teammate a prompt quotes.
const ABOUT: usize = 120;

/// Where finished, accepted work lands: waiting for the user's merge when it has a branch, else done.
pub(super) fn settled(task: &Task) -> TaskStatus {
    if task.branch.is_some() { TaskStatus::Review } else { TaskStatus::Done }
}

/// The bot to check `claimer`'s work: another bot that takes part, idle ones first, then the one holding fewest tasks.
pub(super) fn checker(hub: &Hub, claimer: &str) -> Option<String> {
    let out = dispatch::opted_out(&hub.store);
    let held = dispatch::held_claims(hub);
    dispatch::eligible(hub)
        .ok()?
        .into_iter()
        .filter(|c| c.id != claimer && !out.contains(&c.id))
        .min_by_key(|c| {
            (hub.runtime(&c.id).status != BotStatus::Idle, held.get(&c.id).copied().unwrap_or(0), c.name.clone())
        })
        .map(|c| c.id)
}

/// The lead (else the first bot) takes the task to plan it, and is told how. `split`: the user asked for it to be
/// split (a goal); otherwise the lead decides. Answers who got it.
pub fn plan(hub: &Hub, id: &str, split: bool) -> Result<Value> {
    let lead = dispatch::lead(hub)?;
    let task = take(hub, id, &lead.id)?;
    let how = if split {
        "Split it into subtasks"
    } else {
        "If it has parts other bots could do side by side, split it into subtasks; if it is one bot's job, do it \
         yourself when it fits you, or release_task it (saying who fits) so a better-fitting teammate claims it"
    };
    let mut prompt = format!(
        "[Codync: the user put task {id} on the team board, and as the lead you plan it: it is claimed for you. {how}. \
         A subtask is create_task with parentId {id}: self-contained (what to do, where, how to check it is done), one \
         bot's work, naming in its detail the teammate who fits it best. Don't do the subtasks yourself: the bots \
         claim them, and you will be told when they are all finished; then check and assemble their results into the \
         task's result and finish_task {id}. Tell the user in a few lines how you planned it.]\n\nTask: {}",
        task.title
    );
    if !task.detail.is_empty() {
        let _ = write!(prompt, "\n\n{}", task.detail);
    }
    let _ = write!(prompt, "\n\n{}", teammates(hub, &lead.id));
    send(hub, &lead.id, prompt);
    Ok(json!({"botId": lead.id, "name": lead.name}))
}

/// The other bots taking part, with what each runs and is for, so a bot can tell who fits a piece of work.
pub(super) fn teammates(hub: &Hub, except: &str) -> String {
    let out = dispatch::opted_out(&hub.store);
    let mut line = String::from("Teammates:");
    for c in dispatch::eligible(hub).unwrap_or_default().iter().filter(|c| c.id != except && !out.contains(&c.id)) {
        let _ = write!(line, "\n- {} ({}", c.name, c.backend);
        if let Some(model) = c.model.as_deref().filter(|m| !m.is_empty()) {
            let _ = write!(line, ", {model}");
        }
        line.push(')');
        let about: String =
            c.description.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(ABOUT).collect();
        if !about.is_empty() {
            let _ = write!(line, ": {about}");
        }
    }
    line
}

/// The checker's verdict on a task it checks: accepted, it settles; sent back, the claimer gets it again with the
/// feedback as a note.
pub fn review(hub: &Hub, id: &str, bot: &str, approve: bool, feedback: &str) -> Result<Task> {
    let feedback = text(feedback, "feedback", MAX_NOTE)?;
    ensure!(approve || !feedback.is_empty(), "say what to fix in feedback when you send a task back");
    let task = change(hub, id, |task| {
        ensure!(
            task.status == TaskStatus::Checking,
            "this task is {}: only a task being checked can be reviewed",
            wire(task.status)
        );
        ensure!(task.reviewer.as_deref() == Some(bot), "another bot checks this task");
        if !feedback.is_empty() {
            let verdict = if approve { "Checked" } else { "Sent back" };
            task.notes.push(Note { by: Some(bot.to_owned()), text: format!("{verdict}: {feedback}"), at: now_ms() });
            let over = task.notes.len().saturating_sub(MAX_NOTES);
            task.notes.drain(..over);
        }
        if approve {
            task.status = settled(task);
        } else {
            task.status = TaskStatus::Claimed;
            task.send_backs = task.send_backs.saturating_add(1);
        }
        Ok(())
    })?;
    if approve {
        parent_ready(hub, &task);
    } else if let Some(claimer) = &task.claimed_by {
        let name = hub.bot_name(bot);
        send(
            hub,
            claimer,
            format!(
                "[Codync: {name} checked task {} ({}) and sent it back. Fix what it says, then finish_task {} again.]\
                 \n\n{feedback}",
                task.id, task.title, task.id
            ),
        );
    }
    Ok(task)
}

/// A bot finished a task: its checker is told to check it, or (no check) its coordinator may be ready.
pub(super) fn after_finish(hub: &Hub, task: &Task) {
    match (&task.status, &task.reviewer, &task.claimed_by) {
        (TaskStatus::Checking, Some(reviewer), Some(claimer)) => {
            let name = hub.bot_name(claimer);
            send(
                hub,
                reviewer,
                format!(
                    "[Codync: {name} finished task {id} on the team board, and you check it before it counts as done. \
                     read_task {id}, then look at the result itself (open the files, run it, compare it with what the \
                     task asked). If it does what was asked and works, review_task with approve true; otherwise \
                     approve false, with feedback saying exactly what to fix. Don't fix it yourself.]\n\nTask: {}",
                    task.title,
                    id = task.id
                ),
            );
        }
        _ => parent_ready(hub, task),
    }
}

/// The user closed a task (done or cancelled) that was still unfinished: its coordinator may be ready.
pub(super) fn after_close(hub: &Hub, task: &Task, was: TaskStatus) {
    if was.unfinished() && !task.status.unfinished() {
        parent_ready(hub, task);
    }
}

/// When the last unfinished subtask of a claimed task finishes, its claimer is told to assemble the result.
fn parent_ready(hub: &Hub, task: &Task) {
    let Some(parent) = task.parent.as_deref().and_then(|id| get(&hub.store, id).ok()) else { return };
    let (TaskStatus::Claimed, Some(coordinator)) = (parent.status, &parent.claimed_by) else { return };
    let parts = children(&hub.store, &parent.id);
    if parts.iter().any(|t| t.status.unfinished()) {
        return;
    }
    let mut prompt = format!(
        "[Codync: all {} subtasks of task {id} ({}) are finished. read_task {id} lists them with their summaries; check \
         their results, assemble them into the task's result, then finish_task {id}. Add a subtask (create_task with \
         parentId {id}) if a part is missing.]\n",
        parts.len(),
        parent.title,
        id = parent.id
    );
    for part in &parts {
        let _ =
            write!(prompt, "\n- {} ({}): {}", part.title, wire(part.status), part.summary.as_deref().unwrap_or("-"));
    }
    send(hub, coordinator, prompt);
}

fn send(hub: &Hub, bot: &str, text: String) {
    if let Err(error) = hub.send_cmd(bot, Cmd::Prompt { text, call: None }) {
        tracing::warn!(bot, error = format!("{error:#}"), "board message not delivered");
    }
}
