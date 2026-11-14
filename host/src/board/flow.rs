//! How work moves between the bots on the board, beyond claiming: the lead plans each task the user adds (splitting a
//! job with parts others can do side by side into subtasks it then coordinates), another bot checks finished work
//! before it counts, and a coordinator hears when all its subtasks are finished so it assembles the result.

use super::brief;
use super::sheet::{self, BotClass, Deed};
use super::{
    MAX_NOTE, MAX_NOTES, Note, Task, TaskStatus, change, children, dispatch, effort, get, lessons, take, text, wire,
};
use crate::LockExt;
use crate::agent::bot::Cmd;
use crate::hub::Hub;
use crate::store::now_ms;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::fmt::Write as _;

/// Longest description of a teammate a prompt quotes.
const ABOUT: usize = 120;
/// Stamina (share of the agent's tightest limit left) below which a bot is too tired for board work: it isn't nudged
/// or picked to check, so its plan isn't spent to the last drop.
pub(super) const TIRED: u8 = 10;

/// What is left of the plan of the agent `backend` runs, when it was read.
pub(super) fn stamina(hub: &Hub, backend: &str) -> Option<u8> {
    hub.usage.locked().visible().stamina.get(backend).map(|s| s.left)
}

/// Whether a bot is too tired for board work (an agent with no reading never is).
pub(super) fn tired(hub: &Hub, backend: &str) -> bool {
    stamina(hub, backend).is_some_and(|left| left < TIRED)
}

/// Where finished, accepted work lands: waiting for the user's merge when it has a branch, else done.
pub(super) fn settled(task: &Task) -> TaskStatus {
    if task.branch.is_some() { TaskStatus::Review } else { TaskStatus::Done }
}

/// The bot to check `claimer`'s work: another bot that takes part and isn't tired, martial artists first, then idle
/// ones, then the one holding fewest tasks.
pub(super) fn checker(hub: &Hub, claimer: &str) -> Option<String> {
    let out = dispatch::opted_out(&hub.store);
    let held = dispatch::held_claims(hub);
    dispatch::eligible(hub)
        .ok()?
        .into_iter()
        .filter(|c| c.id != claimer && !out.contains(&c.id) && !tired(hub, &c.backend))
        .min_by_key(|c| {
            (
                sheet::class_of(hub, &c.id) != Some(BotClass::MartialArtist),
                !dispatch::free(hub.runtime(&c.id).status),
                held.get(&c.id).copied().unwrap_or(0),
                c.name.clone(),
            )
        })
        .map(|c| c.id)
}

/// The lead (else the first bot) takes the task to plan it, and is told how. `split`: the user asked for it to be
/// split (a goal); otherwise the lead decides. Answers who got it.
pub fn plan(hub: &Hub, id: &str, split: bool) -> Result<Value> {
    let lead = dispatch::lead(hub)?;
    let task = take(hub, id, &lead.id)?;
    super::focus::begin(hub, &lead.id, &task);
    let prompt = plan_prompt(&task, split, &lessons::prompt(hub, &lead.id), &teammates(hub, &lead.id));
    send(hub, &lead.id, &task.id, prompt);
    Ok(json!({"botId": lead.id, "name": lead.name}))
}

/// What the lead is told to plan a task: size it first, so only work big enough to gain from the team is split (a team
/// costs a planning turn, a turn per part and a check per part: a task one bot does in minutes is faster and cheaper
/// alone). `split`: the user asked for a split (the wand), which still yields to a small task. `lessons`: the lead's
/// lessons block (`lessons::prompt`), empty when none.
pub(super) fn plan_prompt(task: &Task, split: bool, lessons: &str, teammates: &str) -> String {
    let id = &task.id;
    let asked = if split { " The user asked for it to be split if it is big enough." } else { "" };
    let mut prompt = format!(
        "[Codync: the user put task {id} on the team board, and as the lead you plan it: it is claimed for you.{asked} \
         First size it, and note_task {id} your call in one line (\"Size: small, one bot\" or \"Size: large, N parts\", \
         with why). Small: one bot can do it well in about 15 minutes, or its parts would each take only a few \
         minutes: then don't split, since every part costs a turn and a check. Do it yourself when it fits you, or \
         release_task {id} naming the teammate who fits it best. Large: it has independent parts that each take real \
         work (about 10 minutes or more), or that need different strengths: split it into those parts and coordinate. \
         A subtask is create_task with parentId {id}: self-contained (what to do, where), with acceptance criteria the \
         check marks pass or fail, one bot's work, and handed to the teammate who fits it best with assignee (by class and what is left of their \
         plan); leave assignee out only if anyone can do it. Give simple parts to teammates on cheaper models and keep the hard ones (and your own \
         planning and assembling) for the stronger ones. Give a part that builds on others after with their ids \
         (create_task answers each id): it waits until they are finished, then its assignee starts on it. Don't do the subtasks yourself: you will be told when they are all finished; then check \
         and assemble their results into the task's result and finish_task {id}. Tell the user in a few lines how you \
         sized and planned it.]\n\nTask: {}",
        task.title
    );
    if !task.detail.is_empty() {
        let _ = write!(prompt, "\n\n{}", task.detail);
    }
    prompt.push_str(&brief::work_order(task));
    prompt.push_str(lessons);
    let _ = write!(prompt, "\n\n{teammates}");
    prompt
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
        if let Some(class) = sheet::class_of(hub, &c.id) {
            let _ = write!(line, ", {}", class.word());
        }
        if let Some(left) = stamina(hub, &c.backend) {
            let _ = write!(line, ", {left}% of its plan left");
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
/// feedback as a note. A task with acceptance criteria is judged criterion by criterion (`brief::checks`): it is
/// accepted only when every one passes.
pub fn review(hub: &Hub, id: &str, bot: &str, verdict: Verdict<'_>) -> Result<Task> {
    let feedback = text(verdict.feedback, "feedback", MAX_NOTE)?;
    let mut approve = verdict.approve;
    let task = change(hub, id, |task| {
        ensure!(
            task.status == TaskStatus::Checking,
            "this task is {}: only a task being checked can be reviewed",
            wire(task.status)
        );
        ensure!(task.reviewer.as_deref() == Some(bot), "another bot checks this task");
        if task.acceptance.is_empty() {
            task.checks.clear();
        } else {
            let (checks, passed) = brief::checks(task, verdict.checks)?;
            ensure!(
                approve.is_none_or(|a| a == passed),
                "approve must match your checks: the task is accepted only when every criterion passes"
            );
            approve = Some(passed);
            task.checks = checks;
        }
        let approve = approve.ok_or_else(|| anyhow::anyhow!("approve is required"))?;
        let failed = brief::failures(&task.checks);
        ensure!(
            approve || !feedback.is_empty() || !failed.is_empty(),
            "say what to fix in feedback when you send a task back"
        );
        let said = format!("{feedback}{failed}");
        if !said.trim().is_empty() {
            let verdict = if approve { "Checked" } else { "Sent back" };
            task.notes.push(Note {
                by: Some(bot.to_owned()),
                text: format!("{verdict}: {}", said.trim()),
                at: now_ms(),
            });
            let over = task.notes.len().saturating_sub(MAX_NOTES);
            task.notes.drain(..over);
        }
        if approve {
            task.status = settled(task);
        } else {
            task.status = TaskStatus::Claimed;
            task.send_backs = task.send_backs.saturating_add(1);
        }
        effort::stamp_finished(task);
        Ok(())
    })?;
    let approve = matches!(task.status, TaskStatus::Review | TaskStatus::Done);
    let feedback = format!("{feedback}{}", brief::failures(&task.checks));
    sheet::record(hub, bot, Deed::Checked { caught: !approve });
    if approve {
        accepted(hub, &task);
    } else if let Some(claimer) = &task.claimed_by {
        sheet::record(hub, claimer, Deed::SentBack);
        lessons::record(hub, claimer, &task.title, &feedback);
        let name = hub.bot_name(bot);
        send(
            hub,
            claimer,
            &task.id,
            format!(
                "[Codync: {name} checked task {} ({}) and sent it back. Fix what it says, then finish_task {} again.]\
                 \n\n{feedback}",
                task.id, task.title, task.id
            ),
        );
    }
    Ok(task)
}

/// A checker's answer: per-criterion checks (for a task with acceptance criteria), whether it approves, and feedback.
#[derive(Clone, Copy)]
pub struct Verdict<'a> {
    pub approve: Option<bool>,
    pub checks: &'a Value,
    pub feedback: &'a str,
}

/// A bot finished a task: its checker is told to check it, or (no check) its coordinator may be ready.
pub(super) fn after_finish(hub: &Hub, task: &Task) {
    match (&task.status, &task.reviewer, &task.claimed_by) {
        (TaskStatus::Checking, Some(reviewer), Some(claimer)) => {
            super::focus::begin(hub, reviewer, task);
            let name = hub.bot_name(claimer);
            send(
                hub,
                reviewer,
                &task.id,
                format!(
                    "[Codync: {name} finished task {id} on the team board, and you check it before it counts as done. \
                     read_task {id}, then look at the result itself (open the files, run it, compare it with what the \
                     task asked). {how} Don't fix it yourself.]\n\nTask: {}{}",
                    task.title,
                    brief::work_order(task),
                    id = task.id,
                    how = if task.acceptance.is_empty() {
                        "If it does what was asked and works, review_task with approve true; otherwise approve false, \
                         with feedback saying exactly what to fix."
                    } else {
                        "Judge it against each acceptance criterion, strictly: review_task with checks, one {criterion: \
                         its number, pass, note} for every criterion; it is accepted only when all pass, and a criterion \
                         you have doubts about fails, with a note saying what to fix."
                    }
                ),
            );
        }
        _ => accepted(hub, task),
    }
}

/// Finished work counts: its claimer earns the deed, and its coordinator may be ready.
fn accepted(hub: &Hub, task: &Task) {
    if let Some(claimer) = &task.claimed_by {
        sheet::record(hub, claimer, Deed::Finished { first_try: task.send_backs == 0 });
    }
    parent_ready(hub, task);
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
    send(hub, coordinator, &parent.id, prompt);
}

/// Sends `bot` a prompt about `task`: a turn the host starts for it (`effort`), whether or not it is delivered.
fn send(hub: &Hub, bot: &str, task: &str, text: String) {
    effort::count_turn(hub, task);
    if let Err(error) = hub.send_cmd(bot, Cmd::Prompt { text, call: None }) {
        tracing::warn!(bot, error = format!("{error:#}"), "board message not delivered");
    }
}
