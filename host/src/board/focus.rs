//! Each bot starts board work with a clear head: before it is handed a job it wasn't on (a task, or a part of a goal it
//! wasn't working on), its agent session starts afresh, so an earlier task doesn't steer this one (a bot that had built
//! a task once read the next one with the same title as a repeat and stopped). A job is a task with its subtasks: a
//! bot moving between parts of one goal keeps what it knows. A bot in the middle of a turn keeps its session (starting
//! afresh would stop the turn); the prompt waits behind it. The chat the user sees stays as it was.

use super::{Task, dispatch};
use crate::LockExt;
use crate::agent::bot::Cmd;
use crate::hub::Hub;
use std::collections::HashMap;
use std::sync::Mutex;

/// The job (a top-level task's id) each bot last worked on.
#[derive(Default)]
pub struct Focus(Mutex<HashMap<String, String>>);

/// The job a task belongs to: its parent's, or its own.
fn job(task: &Task) -> &str {
    task.parent.as_deref().unwrap_or(&task.id)
}

/// `bot` is about to be told about `task`: a new job starts a fresh session when the bot isn't mid-turn. Answers
/// whether it did.
pub(super) fn begin(hub: &Hub, bot: &str, task: &Task) -> bool {
    if !switch(hub, bot, task) || !dispatch::free(hub.runtime(bot).status) {
        return false;
    }
    match hub.send_cmd(bot, Cmd::NewSession) {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(bot, error = format!("{error:#}"), "board work not started afresh");
            false
        }
    }
}

/// `bot` took up `task` on its own (it claimed it mid-turn): its session carries on, and the job is now this one.
pub(super) fn note(hub: &Hub, bot: &str, task: &Task) {
    switch(hub, bot, task);
}

/// Records `task`'s job as `bot`'s: answers whether it is a different job from the last one.
pub(super) fn switch(hub: &Hub, bot: &str, task: &Task) -> bool {
    let job = job(task);
    let mut jobs = hub.board.focus.0.locked();
    if jobs.get(bot).map(String::as_str) == Some(job) {
        return false;
    }
    jobs.insert(bot.to_owned(), job.to_owned());
    true
}
