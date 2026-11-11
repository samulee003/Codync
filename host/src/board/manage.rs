//! What the user does to tasks beyond adding them: edit, reopen, close, merge a finished branch, delete, and hand a task
//! to a bot.

use super::brief::Approval;
use super::{Actor, MAX_DETAIL, Task, TaskStatus, change, claim, flow, get, git, release, text, title, tools};
use crate::agent::bot::Cmd;
use crate::hub::Hub;
use anyhow::{Result, anyhow, bail, ensure};
use std::fmt::Write as _;
use std::path::Path;

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
            // Starting over: a plan the user approves is proposed again.
            if task.approval != Approval::NotNeeded {
                task.approval = Approval::Waiting;
                task.plan = None;
            }
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

/// The user (or the lead, for a subtask it planned) hands a task to a bot: it is claimed for the bot (taken from a bot
/// holding it, such as the lead planning it), which starts on it in its own chat.
pub async fn assign(hub: &Hub, id: &str, bot: &str, by: Actor<'_>) -> Result<Task> {
    let row =
        hub.store.bot(bot)?.filter(|r| !r.deleted && !r.config.is_group()).ok_or_else(|| anyhow!("unknown bot"))?;
    let held = get(&hub.store, id)?;
    if held.status == TaskStatus::Claimed && held.claimed_by.as_deref() != Some(row.config.id.as_str()) {
        release(hub, id, Actor::User, None)?;
    }
    let task = claim(hub, id, &row.config.id).await?;
    let who = match by {
        Actor::User => "the user".to_owned(),
        Actor::Bot(lead) => hub.bot_name(lead),
    };
    hub.send_cmd(&row.config.id, Cmd::Prompt { text: assigned_prompt(&task, &who), call: None })?;
    Ok(task)
}

fn assigned_prompt(task: &Task, who: &str) -> String {
    let mut prompt = format!(
        "[Codync: {who} assigned you task {} on the team board, and it is claimed for you. Read it with read_task, \
         work on it (its folder is yours to work in), share findings others could use with note_task, and end with \
         finish_task (or release_task if you can't do it). Tell the user in this chat what you did.]\n\nTask: {}",
        task.id, task.title
    );
    if !task.detail.is_empty() {
        let _ = write!(prompt, "\n\n{}", task.detail);
    }
    prompt.push_str(&super::brief::work_order(task));
    if let Some(tree) = &task.worktree {
        let _ = write!(prompt, "\n\n{}", tools::worktree_hint(tree, task.branch.as_deref()));
    }
    prompt
}
