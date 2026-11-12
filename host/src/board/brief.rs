//! A task as a work order the user can hold the bots to: its acceptance criteria (what must hold for the result to
//! count as done), the plan its claimer proposes before any work when the user asked to approve it first, and the
//! checker's verdict on each criterion. A task waiting for approval can't be split or finished; a check marks every
//! criterion pass or fail, and one failed criterion sends the work back.

use super::{MAX_NOTE, MAX_NOTES, Note, Task, TaskStatus, change, check_claimer, effort, text, wire};
use crate::agent::bot::Cmd;
use crate::hub::Hub;
use crate::remote::push::{self, AlertKind};
use crate::store::now_ms;
use anyhow::{Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Write as _;

/// Criteria a task holds.
const MAX_CRITERIA: usize = 12;
/// Bytes one criterion holds.
const MAX_CRITERION: usize = 300;
/// Bytes a proposed plan holds.
const MAX_PLAN: usize = 6000;
/// Questions a task asks the user before its plan.
const MAX_QUESTIONS: usize = 3;
/// Bytes one question holds.
const MAX_QUESTION: usize = 300;
/// Bytes one answer holds.
const MAX_ANSWER: usize = 2000;

/// Whether the user approves the claimer's plan before work starts (wire: `notNeeded` / `waiting` / `asking` /
/// `proposed` / `approved`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Approval {
    /// Work starts at once.
    #[default]
    NotNeeded,
    /// Its claimer proposes a plan first (or asks the user questions first).
    Waiting,
    /// The claimer's questions wait for the user's answers.
    Asking,
    /// The plan waits for the user.
    Proposed,
    Approved,
}

impl Approval {
    /// A new task's approval: the user asked to approve its plan first, or not.
    pub fn asked(first: bool) -> Self {
        if first { Self::Waiting } else { Self::NotNeeded }
    }
}

/// The checker's verdict on one criterion.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub criterion: String,
    pub pass: bool,
    #[serde(default)]
    pub note: String,
}

/// A question a claimer asks the user before its plan, and the user's answer.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub text: String,
    #[serde(default)]
    pub answer: Option<String>,
}

/// Acceptance criteria from a client or a bot: a list of strings, or one string with a criterion per line. Blank ones
/// are dropped.
pub fn criteria(value: &Value) -> Result<Vec<String>> {
    let lines: Vec<&str> = match value {
        Value::Null => vec![],
        Value::String(s) => s.lines().collect(),
        Value::Array(items) => items
            .iter()
            .map(|v| v.as_str().ok_or_else(|| anyhow!("each criterion is a string")))
            .collect::<Result<_>>()?,
        _ => bail!("acceptance is a list of criteria"),
    };
    let mut out = Vec::new();
    for line in lines {
        let line = text(line.trim_start_matches(['-', '*', '•', ' ']), "a criterion", MAX_CRITERION)?;
        ensure!(!line.contains('\n'), "a criterion is one line");
        if !line.is_empty() {
            out.push(line);
        }
    }
    ensure!(out.len() <= MAX_CRITERIA, "a task holds at most {MAX_CRITERIA} criteria");
    Ok(out)
}

/// The claimer proposes its plan (and the criteria, when it writes or refines them); the task then waits for the user.
pub fn propose(hub: &Hub, id: &str, bot: &str, plan: &str, acceptance: &Value) -> Result<Task> {
    let plan = text(plan, "plan", MAX_PLAN)?;
    ensure!(!plan.is_empty(), "plan is empty: list the steps or parts, and who does each");
    let acceptance = criteria(acceptance)?;
    let task = change(hub, id, |task| {
        ensure!(task.status == TaskStatus::Claimed, "this task is {}: claim it first", wire(task.status));
        check_claimer(hub, task, super::Actor::Bot(bot))?;
        ensure!(
            task.approval != Approval::Asking,
            "the user hasn't answered your questions about task {} yet: you'll be told",
            task.id
        );
        ensure!(
            matches!(task.approval, Approval::Waiting | Approval::Proposed),
            "this task needs no approval: go ahead with the work"
        );
        if !acceptance.is_empty() {
            task.acceptance = acceptance;
        }
        ensure!(
            !task.acceptance.is_empty(),
            "add acceptance: the checkable criteria that must hold for the result to count as done"
        );
        task.plan = Some(plan);
        task.approval = Approval::Proposed;
        Ok(())
    })?;
    if let Ok(Some(row)) = hub.store.bot(bot) {
        let body = format!("Plan for \"{}\" is waiting for your approval.", task.title);
        push::notify(hub, &row.config, None, &row.config.name, &body, AlertKind::NeedsInput);
    }
    Ok(task)
}

/// The claimer asks the user up to 3 short questions before its plan, then waits for the answers. Once asked, the
/// claimer proposes its plan with what the answers say, so it asks only once.
pub fn ask(hub: &Hub, id: &str, bot: &str, questions: &Value) -> Result<Task> {
    let questions = question_texts(questions)?;
    let task = change(hub, id, |task| {
        ensure!(task.status == TaskStatus::Claimed, "this task is {}: claim it first", wire(task.status));
        check_claimer(hub, task, super::Actor::Bot(bot))?;
        ensure!(task.approval == Approval::Waiting, "this task needs no questions: go ahead with the work");
        ensure!(task.questions.is_empty(), "you already asked: propose_plan now");
        task.questions = questions.into_iter().map(|text| Question { text, answer: None }).collect();
        task.approval = Approval::Asking;
        Ok(())
    })?;
    if let Ok(Some(row)) = hub.store.bot(bot) {
        let body = format!("{} has questions about \"{}\".", row.config.name, task.title);
        push::notify(hub, &row.config, None, &row.config.name, &body, AlertKind::NeedsInput);
    }
    Ok(task)
}

/// The questions from a client or a bot: a list of one-line strings, one to three of them.
fn question_texts(value: &Value) -> Result<Vec<String>> {
    let items = value.as_array().ok_or_else(|| anyhow!("questions is a list of 1 to {MAX_QUESTIONS} questions"))?;
    let questions = items
        .iter()
        .map(|v| {
            let q = text(v.as_str().unwrap_or_default(), "a question", MAX_QUESTION)?;
            ensure!(!q.is_empty() && !q.contains('\n'), "a question is one nonempty line");
            Ok(q)
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!((1..=MAX_QUESTIONS).contains(&questions.len()), "ask 1 to {MAX_QUESTIONS} questions");
    Ok(questions)
}

/// The user answers the claimer's questions, one answer per question, in order. The claimer is told to propose its
/// plan with them.
pub fn answer(hub: &Hub, id: &str, answers: &Value) -> Result<Task> {
    let answers = answers.as_array().ok_or_else(|| anyhow!("answers is a list with one answer per question"))?;
    let task = change(hub, id, |task| {
        ensure!(task.approval == Approval::Asking, "this task has no questions waiting for answers");
        ensure!(
            answers.len() == task.questions.len(),
            "answer each of the {} questions: {} answers given",
            task.questions.len(),
            answers.len()
        );
        for (question, answer) in task.questions.iter_mut().zip(answers) {
            question.answer = Some(text(answer.as_str().unwrap_or_default(), "an answer", MAX_ANSWER)?);
        }
        task.approval = Approval::Waiting;
        Ok(())
    })?;
    let mut prompt = format!(
        "[Codync: the user answered your questions about task {id}. Now propose_plan {id} using the answers, then wait.]\n",
        id = task.id
    );
    for question in &task.questions {
        let _ = write!(prompt, "\nQ: {}\nA: {}", question.text, question.answer.as_deref().unwrap_or_default());
    }
    tell(hub, &task, prompt);
    Ok(task)
}

/// The user approves the proposed plan: its claimer carries it out.
pub fn approve(hub: &Hub, id: &str) -> Result<Task> {
    let task = change(hub, id, |task| {
        ensure!(task.approval == Approval::Proposed, "this task has no plan waiting for approval");
        task.approval = Approval::Approved;
        Ok(())
    })?;
    tell(
        hub,
        &task,
        format!(
            "[Codync: the user approved your plan for task {id}. Carry it out now, and keep to it; if it has to change, \
             say why in a note_task.]\n\nTask: {}",
            task.title,
            id = task.id
        ),
    );
    Ok(task)
}

/// The user asks for a different plan: the note says what to change, and its claimer proposes again.
pub fn revise(hub: &Hub, id: &str, note: &str) -> Result<Task> {
    let note = text(note, "note", MAX_NOTE)?;
    ensure!(!note.is_empty(), "say what to change in the plan");
    let task = change(hub, id, |task| {
        ensure!(task.approval == Approval::Proposed, "this task has no plan waiting for approval");
        task.approval = Approval::Waiting;
        task.notes.push(Note { by: None, text: format!("Change the plan: {note}"), at: now_ms() });
        let over = task.notes.len().saturating_sub(MAX_NOTES);
        task.notes.drain(..over);
        Ok(())
    })?;
    tell(
        hub,
        &task,
        format!(
            "[Codync: the user wants your plan for task {id} changed. Revise it as they ask, then propose_plan {id} \
             again and wait.]\n\n{note}",
            id = task.id
        ),
    );
    Ok(task)
}

/// Refuses work on a task whose plan isn't approved yet.
pub fn ensure_cleared(task: &Task) -> Result<()> {
    match task.approval {
        Approval::Waiting => bail!("propose_plan {} first: the user approves the plan before any work", task.id),
        Approval::Asking => bail!("the user hasn't answered your questions about task {} yet: you'll be told", task.id),
        Approval::Proposed => bail!("the user hasn't approved the plan of task {} yet: you'll be told", task.id),
        Approval::NotNeeded | Approval::Approved => Ok(()),
    }
}

/// The checker's per-criterion verdicts (`[{criterion: number, pass, note}]`, every criterion once, numbered from 1):
/// answers them with whether all passed.
pub fn checks(task: &Task, value: &Value) -> Result<(Vec<Check>, bool)> {
    let items = value.as_array().ok_or_else(|| {
        anyhow!(
            "checks is required: one {{criterion, pass, note}} for each of the task's {} criteria",
            task.acceptance.len()
        )
    })?;
    let mut verdicts: Vec<Option<Check>> = vec![None; task.acceptance.len()];
    for item in items {
        let n = item["criterion"].as_u64().and_then(|n| usize::try_from(n).ok()).unwrap_or(0);
        let slot = n
            .checked_sub(1)
            .and_then(|i| verdicts.get_mut(i))
            .ok_or_else(|| anyhow!("criterion is a number from 1 to {}", task.acceptance.len()))?;
        ensure!(slot.is_none(), "criterion {n} is checked twice");
        let pass = item["pass"].as_bool().ok_or_else(|| anyhow!("say pass true or false for criterion {n}"))?;
        let note = text(item["note"].as_str().unwrap_or_default(), "a check's note", MAX_CRITERION)?;
        ensure!(pass || !note.is_empty(), "say what fails criterion {n} in its note");
        *slot = Some(Check { criterion: task.acceptance[n - 1].clone(), pass, note });
    }
    let missing: Vec<String> =
        verdicts.iter().enumerate().filter(|(_, v)| v.is_none()).map(|(i, _)| (i + 1).to_string()).collect();
    ensure!(missing.is_empty(), "check every criterion: {} not checked", missing.join(", "));
    let checks: Vec<Check> = verdicts.into_iter().flatten().collect();
    let passed = checks.iter().all(|c| c.pass);
    Ok((checks, passed))
}

/// The criteria as a bot reads them, numbered.
pub fn numbered(acceptance: &[String]) -> String {
    let mut out = String::new();
    for (i, c) in acceptance.iter().enumerate() {
        let _ = write!(out, "\n{}. {c}", i + 1);
    }
    out
}

/// What failed in a check, for the claimer it goes back to.
pub fn failures(checks: &[Check]) -> String {
    let mut out = String::new();
    for c in checks.iter().filter(|c| !c.pass) {
        let _ = write!(out, "\n- {}: {}", c.criterion, c.note);
    }
    out
}

/// Tells the claimer of `task` something about it: a turn the host starts for the task (`effort`).
fn tell(hub: &Hub, task: &Task, text: String) {
    let Some(bot) = &task.claimed_by else { return };
    effort::count_turn(hub, &task.id);
    if let Err(error) = hub.send_cmd(bot, Cmd::Prompt { text, call: None }) {
        tracing::warn!(bot, error = format!("{error:#}"), "plan message not delivered");
    }
}

/// What a bot starting on a task reads after its detail: the criteria the work is checked against and, while its plan
/// waits for the user, to propose it first.
pub fn work_order(task: &Task) -> String {
    let mut out = String::new();
    if !task.acceptance.is_empty() {
        let _ =
            write!(out, "\n\nAcceptance criteria (the check marks each pass or fail):{}", numbered(&task.acceptance));
    }
    match (task.approval, &task.plan) {
        (Approval::Waiting, _) => {
            if task.questions.is_empty() {
                let _ = write!(
                    out,
                    "\n\nIf what to make, for whom, with which tools or materials, or what counts as done is unclear, \
                     first ask_user {} up to 3 short questions (only ones whose answer changes the work) and stop; if \
                     it is clear, go straight to propose_plan.",
                    task.id
                );
            }
            for question in &task.questions {
                let _ = write!(out, "\n\nQ: {}\nA: {}", question.text, question.answer.as_deref().unwrap_or_default());
            }
            let _ = write!(
                out,
                "\n\nThe user approves the plan before any work: first propose_plan {} with the steps or parts and who \
                 does each, and acceptance criteria if it has none or they need sharpening. Then stop: you'll be told \
                 when it is approved or what to change.",
                task.id
            );
        }
        (Approval::Asking, _) => {
            out.push_str("\n\nYour questions wait for the user's answers: don't start until you're told.");
        }
        (Approval::Proposed, _) => {
            out.push_str("\n\nIts plan waits for the user's approval: don't start until you're told.");
        }
        (Approval::Approved, Some(plan)) => {
            let _ = write!(out, "\n\nThe plan the user approved:\n{plan}");
        }
        _ => {}
    }
    out
}
