//! The team board: tasks the user and the bots share, inspired by Agensh's claim board. Anyone adds a task; a bot
//! claims one (atomically: one claimer at a time), notes what it finds for the others, and finishes it. A task in a
//! git project gets its own worktree and branch when claimed (`git`), so bots working side by side never edit the
//! same checkout; the user merges a finished task's branch from the board. Tasks live in kv (`board.task.<id>`),
//! each change stamped with a rev and sent as a `task` event, like artifacts.
//!
//! A task the user adds goes to the lead bot first (`flow::plan`): it splits a job with parts others can do side by
//! side into subtasks (`parent`) and coordinates them, assembling the result once they're all finished. A finished
//! task is checked by another bot before it counts as done (`flow`: `checking`, `review_task`).

use crate::LockExt;
use crate::agent::bot::Cmd;
use crate::hub::Hub;
use crate::store::{Store, now_ms};
use anyhow::{Context as _, Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Mutex;

pub mod dispatch;
pub mod flow;
pub mod git;
pub mod tools;

const KEY_PREFIX: &str = "board.task.";
const MAX_TITLE: usize = 200;
const MAX_DETAIL: usize = 8000;
const MAX_NOTE: usize = 4000;
/// Notes a task keeps; the oldest go first.
const MAX_NOTES: usize = 50;
/// Live (not deleted) tasks a computer keeps.
const MAX_TASKS: usize = 500;
/// Tasks one bot may hold at once, so one bot can't take the whole board.
const MAX_CLAIMS: usize = 3;
/// Times a checker may send a task back before a finish counts without another check.
const MAX_SEND_BACKS: u8 = 2;

/// Serializes every board change, so a claim reads and writes a task with no other change in between.
#[derive(Default)]
pub struct Board {
    lock: Mutex<()>,
    /// What each bot was last told about the open tasks (`dispatch`).
    nudges: dispatch::Nudges,
}

/// Where a task stands (wire values: `open` / `claimed` / `checking` / `review` / `done` / `cancelled`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Open,
    Claimed,
    /// Finished, and another bot (`reviewer`) checks the result before it counts.
    Checking,
    /// Finished on its own branch, waiting for the user to merge it.
    Review,
    Done,
    Cancelled,
}

impl TaskStatus {
    pub fn parse(value: &str) -> Result<Self> {
        serde_json::from_value(json!(value)).map_err(|_| anyhow!("unknown task status: {value}"))
    }

    fn closed(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }

    /// Still being worked on or checked: a parent waits for its subtasks to leave these.
    fn unfinished(self) -> bool {
        matches!(self, Self::Open | Self::Claimed | Self::Checking)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// The bot that wrote it; `None`: the user.
    pub by: Option<String>,
    pub text: String,
    pub at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    /// The folder the work is in; a git repository gets a worktree per claim.
    pub project: Option<String>,
    pub status: TaskStatus,
    /// The bot that added it; `None`: the user.
    pub created_by: Option<String>,
    pub claimed_by: Option<String>,
    /// The task's branch and worktree, once claimed in a git project.
    pub branch: Option<String>,
    pub worktree: Option<String>,
    #[serde(default)]
    pub notes: Vec<Note>,
    /// What the claimer reported when it finished.
    pub summary: Option<String>,
    /// The task this one is a part of (a subtask), whose claimer assembles the parts.
    #[serde(default)]
    pub parent: Option<String>,
    /// The bot that checks (or checked) the finished work.
    #[serde(default)]
    pub reviewer: Option<String>,
    /// Times a checker sent it back.
    #[serde(default)]
    pub send_backs: u8,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub rev: i64,
    #[serde(default)]
    pub deleted: bool,
}

/// Who changes the board: the user (an app) or one of the bots.
#[derive(Clone, Copy)]
pub enum Actor<'a> {
    User,
    Bot(&'a str),
}

impl Actor<'_> {
    fn id(self) -> Option<String> {
        match self {
            Self::User => None,
            Self::Bot(id) => Some(id.to_owned()),
        }
    }
}

fn key(id: &str) -> String {
    format!("{KEY_PREFIX}{id}")
}

fn saved(store: &Store) -> impl Iterator<Item = Task> + '_ {
    store.kv_prefix(KEY_PREFIX).into_iter().filter_map(|k| serde_json::from_str(&store.kv_get(&k)?).ok())
}

pub fn get(store: &Store, id: &str) -> Result<Task> {
    let id = uuid::Uuid::parse_str(id).context("invalid task id")?.to_string();
    store
        .kv_get(&key(&id))
        .and_then(|v| serde_json::from_str::<Task>(&v).ok())
        .filter(|t| !t.deleted)
        .ok_or_else(|| anyhow!("no task {id}: list_tasks shows the board"))
}

/// The board, newest first; closed tasks after the rest.
pub fn list(store: &Store) -> Vec<Task> {
    let mut tasks: Vec<Task> = saved(store).filter(|t| !t.deleted).collect();
    tasks.sort_by_key(|t| (t.status.closed(), std::cmp::Reverse(t.updated_at)));
    tasks
}

fn event(task: &Task) -> Value {
    json!({"type": "task", "rev": task.rev, "task": task})
}

/// The `task` events a client's catch-up holds: every task changed after `since`, deletions included.
// ponytail: scans the board's kv rows; it holds at most MAX_TASKS live ones.
pub fn since(store: &Store, since: i64) -> Vec<Value> {
    saved(store).filter(|t| t.rev > since).map(|t| event(&t)).collect()
}

/// Saves a task under a new rev and tells the apps.
fn put(hub: &Hub, task: &mut Task) -> Result<()> {
    task.updated_at = now_ms();
    hub.emit_change(|| {
        hub.store.kv_set_stamped(&key(&task.id), |rev| {
            task.rev = rev;
            Ok(serde_json::to_string(task)?)
        })?;
        Ok(event(task))
    })
}

fn text(value: &str, what: &str, max: usize) -> Result<String> {
    let value = value.trim();
    ensure!(value.len() <= max, "{what} can be at most {max} bytes");
    ensure!(!value.chars().any(|c| c.is_control() && c != '\n' && c != '\t'), "{what} can't hold control characters");
    Ok(value.to_owned())
}

pub fn title(value: &str) -> Result<String> {
    let value = text(value, "title", MAX_TITLE)?;
    ensure!(!value.is_empty() && !value.contains('\n'), "title must be one nonempty line");
    Ok(value)
}

/// A project folder from a client or a bot: absolute and an existing directory.
fn project(value: Option<&str>) -> Result<Option<String>> {
    let Some(path) = value.map(str::trim).filter(|p| !p.is_empty()) else { return Ok(None) };
    let path = Path::new(path);
    ensure!(path.is_absolute(), "project must be an absolute path");
    ensure!(path.is_dir(), "project {} isn't a folder", path.display());
    Ok(Some(path.display().to_string()))
}

/// What a new task holds.
pub struct NewTask<'a> {
    pub title: &'a str,
    pub detail: &'a str,
    pub project: Option<&'a str>,
    /// The task it is a part of.
    pub parent: Option<&'a str>,
}

/// A task's subtasks, oldest first.
pub fn children(store: &Store, id: &str) -> Vec<Task> {
    let mut tasks: Vec<Task> = saved(store).filter(|t| !t.deleted && t.parent.as_deref() == Some(id)).collect();
    tasks.sort_by_key(|t| t.created_at);
    tasks
}

pub fn create(hub: &Hub, by: Actor<'_>, new: &NewTask<'_>) -> Result<Task> {
    let (title, detail, project) = (title(new.title)?, text(new.detail, "detail", MAX_DETAIL)?, project(new.project)?);
    let _g = hub.board.lock.locked();
    ensure!(saved(&hub.store).filter(|t| !t.deleted).count() < MAX_TASKS, "the board is full: delete closed tasks");
    let parent = match new.parent.map(str::trim).filter(|p| !p.is_empty()) {
        Some(id) => {
            let parent = get(&hub.store, id)?;
            ensure!(
                !parent.status.closed(),
                "the parent task is {}: a subtask needs an unfinished one",
                wire(parent.status)
            );
            Some(parent.id)
        }
        None => None,
    };
    let now = now_ms();
    let mut task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        detail,
        project,
        status: TaskStatus::Open,
        created_by: by.id(),
        claimed_by: None,
        branch: None,
        worktree: None,
        notes: vec![],
        summary: None,
        parent,
        reviewer: None,
        send_backs: 0,
        created_at: now,
        updated_at: now,
        rev: 0,
        deleted: false,
    };
    put(hub, &mut task)?;
    Ok(task)
}

/// Changes a task under the board lock: `change` checks and edits it; it is saved when `change` succeeds.
fn change(hub: &Hub, id: &str, change: impl FnOnce(&mut Task) -> Result<()>) -> Result<Task> {
    let _g = hub.board.lock.locked();
    let mut task = get(&hub.store, id)?;
    change(&mut task)?;
    put(hub, &mut task)?;
    Ok(task)
}

/// Takes an open task for `bot`. The first claimer wins; a task in a git project then gets its worktree, made outside
/// the lock (a slow disk mustn't hold the board). A worktree that can't be made gives the task back.
pub async fn claim(hub: &Hub, id: &str, bot: &str) -> Result<Task> {
    let task = take(hub, id, bot)?;
    if task.worktree.is_some() {
        return Ok(task);
    }
    let Some(project) = task.project.clone() else { return Ok(task) };
    match git::checkout(Path::new(&project), &task.id).await {
        Ok(None) => Ok(task),
        Ok(Some(tree)) => change(hub, id, |task| {
            task.branch = Some(tree.branch);
            task.worktree = Some(tree.path.display().to_string());
            Ok(())
        }),
        Err(error) => {
            let _ = release(hub, id, Actor::Bot(bot), None);
            Err(error.context("making the task's worktree"))
        }
    }
}

/// Claims an open task for `bot`, with no worktree (the first claimer wins).
fn take(hub: &Hub, id: &str, bot: &str) -> Result<Task> {
    change(hub, id, |task| {
        match (&task.claimed_by, task.status) {
            (Some(by), TaskStatus::Claimed) if by == bot => return Ok(()),
            (_, TaskStatus::Open) => {}
            (Some(by), TaskStatus::Claimed) => bail!("{} already claimed this task", hub.bot_name(by)),
            (_, status) => bail!("this task is {}: only an open task can be claimed", wire(status)),
        }
        let held = saved(&hub.store)
            .filter(|t| !t.deleted && t.status == TaskStatus::Claimed && t.claimed_by.as_deref() == Some(bot))
            .count();
        ensure!(held < MAX_CLAIMS, "you already hold {MAX_CLAIMS} tasks: finish or release one first");
        task.status = TaskStatus::Claimed;
        task.claimed_by = Some(bot.to_owned());
        Ok(())
    })
}

/// The claimer, or the user, may change a claimed task.
fn check_claimer(hub: &Hub, task: &Task, by: Actor<'_>) -> Result<()> {
    if let Actor::Bot(bot) = by
        && task.claimed_by.as_deref() != Some(bot)
    {
        match &task.claimed_by {
            Some(other) => bail!("{} holds this task, not you", hub.bot_name(other)),
            None => bail!("claim this task first"),
        }
    }
    Ok(())
}

pub fn note(hub: &Hub, id: &str, by: Actor<'_>, value: &str) -> Result<Task> {
    let value = text(value, "note", MAX_NOTE)?;
    ensure!(!value.is_empty(), "note is empty");
    change(hub, id, |task| {
        task.notes.push(Note { by: by.id(), text: value, at: now_ms() });
        let over = task.notes.len().saturating_sub(MAX_NOTES);
        task.notes.drain(..over);
        Ok(())
    })
}

/// The claimer is done: another bot checks the work first (`checking`, `flow`), unless none can or it was sent back
/// too often; then a task with a branch waits for the user's merge (`review`), any other is `done`. Work left
/// uncommitted in the worktree is refused, so the branch the user merges is the whole result, and a task with
/// unfinished subtasks can't finish.
pub async fn finish(hub: &Hub, id: &str, bot: &str, summary: &str) -> Result<Task> {
    let summary = text(summary, "summary", MAX_DETAIL)?;
    ensure!(!summary.is_empty(), "summary is required: say what you did and how you checked it");
    let task = get(&hub.store, id)?;
    check_claimer(hub, &task, Actor::Bot(bot))?;
    let open = children(&hub.store, &task.id).into_iter().filter(|t| t.status.unfinished()).count();
    ensure!(open == 0, "{open} subtasks of this task aren't finished yet: you'll be told when they are");
    if let Some(tree) = &task.worktree {
        ensure!(
            git::clean(Path::new(tree)).await?,
            "{tree} has uncommitted changes: commit them on {} first",
            task.branch.as_deref().unwrap_or("the task's branch")
        );
    }
    let checker = (task.send_backs < MAX_SEND_BACKS).then(|| flow::checker(hub, bot)).flatten();
    let task = change(hub, id, |task| {
        ensure!(task.status == TaskStatus::Claimed, "only a claimed task can be finished");
        check_claimer(hub, task, Actor::Bot(bot))?;
        task.summary = Some(summary);
        match &checker {
            Some(reviewer) => {
                task.status = TaskStatus::Checking;
                task.reviewer = Some(reviewer.clone());
            }
            None => task.status = flow::settled(task),
        }
        Ok(())
    })?;
    flow::after_finish(hub, &task);
    Ok(task)
}

/// Gives a claimed task back to the board. The branch and worktree stay, so the next claimer carries on from them.
pub fn release(hub: &Hub, id: &str, by: Actor<'_>, reason: Option<&str>) -> Result<Task> {
    let reason = reason.map(|r| text(r, "reason", MAX_NOTE)).transpose()?.filter(|r| !r.is_empty());
    change(hub, id, |task| {
        ensure!(task.status == TaskStatus::Claimed, "only a claimed task can be released");
        check_claimer(hub, task, by)?;
        task.status = TaskStatus::Open;
        task.claimed_by = None;
        task.reviewer = None;
        if let Some(reason) = reason {
            task.notes.push(Note { by: by.id(), text: format!("Released: {reason}"), at: now_ms() });
        }
        Ok(())
    })
}

/// The user edits a task's title or detail.
pub fn edit(hub: &Hub, id: &str, new_title: Option<&str>, detail: Option<&str>) -> Result<Task> {
    let new_title = new_title.map(title).transpose()?;
    let detail = detail.map(|d| text(d, "detail", MAX_DETAIL)).transpose()?;
    change(hub, id, |task| {
        if let Some(t) = new_title {
            task.title = t;
        }
        if let Some(d) = detail {
            task.detail = d;
        }
        Ok(())
    })
}

/// The user moves a task: reopen it (its claimer lets go), mark it done or cancel it. Closing removes its worktree;
/// the branch stays, so no commit is lost.
pub async fn set_status(hub: &Hub, id: &str, status: TaskStatus) -> Result<Task> {
    let was = get(&hub.store, id)?.status;
    ensure!(
        matches!(status, TaskStatus::Open | TaskStatus::Done | TaskStatus::Cancelled),
        "a task can be set open, done or cancelled"
    );
    let task = change(hub, id, |task| {
        task.status = status;
        if status == TaskStatus::Open {
            task.claimed_by = None;
            task.reviewer = None;
        }
        Ok(())
    })?;
    flow::after_close(hub, &task, was);
    if status.closed() { drop_worktree(hub, task).await } else { Ok(task) }
}

async fn drop_worktree(hub: &Hub, task: Task) -> Result<Task> {
    let (Some(project), Some(tree)) = (task.project.clone(), task.worktree.clone()) else { return Ok(task) };
    git::remove_worktree(Path::new(&project), Path::new(&tree)).await?;
    change(hub, &task.id, |task| {
        task.worktree = None;
        Ok(())
    })
}

/// The user merges a task waiting for review into its project's current branch, then the task is done and its
/// worktree and branch go. A conflict or a project with uncommitted changes merges nothing.
pub async fn merge(hub: &Hub, id: &str) -> Result<Task> {
    let task = get(&hub.store, id)?;
    ensure!(task.status == TaskStatus::Review, "only a task waiting for review can be merged");
    let (Some(project), Some(branch)) = (task.project.clone(), task.branch.clone()) else {
        bail!("this task has no branch to merge")
    };
    git::merge(Path::new(&project), &branch, &task.title).await?;
    let task = change(hub, id, |task| {
        task.status = TaskStatus::Done;
        Ok(())
    })?;
    let task = drop_worktree(hub, task).await?;
    if let Err(error) = git::delete_branch(Path::new(&project), &branch).await {
        tracing::warn!(error = format!("{error:#}"), "merged task branch not deleted");
    }
    change(hub, &task.id, |task| {
        task.branch = None;
        Ok(())
    })
}

/// The user deletes a task: a claimed one must be released first. Its branch stays.
pub async fn delete(hub: &Hub, id: &str) -> Result<()> {
    let task = get(&hub.store, id)?;
    ensure!(task.status != TaskStatus::Claimed, "a claimed task can't be deleted: reopen or cancel it first");
    let task = drop_worktree(hub, task).await?;
    change(hub, &task.id, |task| {
        task.deleted = true;
        task.notes.clear();
        task.detail.clear();
        task.summary = None;
        Ok(())
    })?;
    Ok(())
}

/// The user hands a task to a bot: it is claimed for the bot (taken from a bot holding it, such as the lead planning
/// it), which starts on it in its own chat.
pub async fn assign(hub: &Hub, id: &str, bot: &str) -> Result<Task> {
    let row =
        hub.store.bot(bot)?.filter(|r| !r.deleted && !r.config.is_group()).ok_or_else(|| anyhow!("unknown bot"))?;
    let held = get(&hub.store, id)?;
    if held.status == TaskStatus::Claimed && held.claimed_by.as_deref() != Some(row.config.id.as_str()) {
        release(hub, id, Actor::User, None)?;
    }
    let task = claim(hub, id, &row.config.id).await?;
    hub.send_cmd(&row.config.id, Cmd::Prompt { text: assigned_prompt(&task), call: None })?;
    Ok(task)
}

fn assigned_prompt(task: &Task) -> String {
    let mut prompt = format!(
        "[Codync: the user assigned you task {} on the team board, and it is claimed for you. Read it with read_task, \
         work on it, share findings others could use with note_task, and end with finish_task (or release_task if you \
         can't do it). Tell the user in this chat what you did.]\n\nTask: {}",
        task.id, task.title
    );
    if !task.detail.is_empty() {
        let _ = write!(prompt, "\n\n{}", task.detail);
    }
    if let Some(tree) = &task.worktree {
        let _ = write!(prompt, "\n\n{}", tools::worktree_hint(tree, task.branch.as_deref()));
    }
    prompt
}

fn wire(status: TaskStatus) -> String {
    json!(status).as_str().unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests;
