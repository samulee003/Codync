//! The board's tools on each bot's `team` MCP server: read the board, add tasks and subtasks, claim one, note
//! findings for the others, finish or release it, and check another bot's finished work.

use super::{Actor, NewTask, Task, TaskStatus};
use crate::hub::Hub;
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

pub const TOOLS: [&str; 8] =
    ["list_tasks", "read_task", "create_task", "claim_task", "note_task", "finish_task", "release_task", "review_task"];

pub const INSTRUCTIONS: &str = " The team board (list_tasks) holds tasks the user and the bots share. Take only work \
that fits you better than your teammates. To split a job with parts others can do side by side, add the parts as \
subtasks (create_task with parentId) and coordinate: you are told when they are all finished, then assemble the \
result. To take a task, claim_task it (one claimer per task, so two bots never do the same work), then read_task for \
its notes. Share what others could use (a finding, a decision, a blocker) with note_task. A task in a git project \
gives you its own worktree and branch when you claim it: work and commit only there, never in the project's own \
checkout. End with finish_task and a summary of what you did and how you checked it, or release_task when you can't \
do it. Another bot then checks your work and may send it back with what to fix; when you are asked to check a task, \
look at the result itself and answer with review_task.";

/// What a bot reads about where to work on a claimed task.
pub fn worktree_hint(tree: &str, branch: Option<&str>) -> String {
    let branch = branch.unwrap_or("the task's branch");
    format!(
        "Work in {tree}: it is this task's own git worktree, on branch {branch}. Edit and commit only there (other \
         bots work in the same repository), and commit everything before finish_task. The user merges {branch} from \
         the board."
    )
}

pub fn schemas() -> Vec<Value> {
    let id = json!({"type": "string", "description": "The task's id, from list_tasks."});
    vec![
        json!({
            "name": "list_tasks",
            "description": "The team board: every task with its id, title, status (open, claimed, checking, review, done, cancelled), who claimed it, its parent task and how many notes it has. Newest first, closed tasks last.",
            "inputSchema": {"type": "object", "properties": {
                "status": {"type": "string", "enum": ["open", "claimed", "checking", "review", "done", "cancelled"], "description": "Only tasks in this status."},
                "mine": {"type": "boolean", "description": "Only the tasks you claimed."},
            }},
            "annotations": {"readOnlyHint": true},
        }),
        json!({
            "name": "read_task",
            "description": "One task in full: its detail, notes from the user and the bots, summary, who checks it, its parent and its subtasks with their summaries, and its worktree and branch when claimed in a git project.",
            "inputSchema": {"type": "object", "properties": {"taskId": id}, "required": ["taskId"]},
            "annotations": {"readOnlyHint": true},
        }),
        json!({
            "name": "create_task",
            "description": "Add an open task to the board for any bot (or the user) to claim. Make it self-contained: what to do, where, and how to check it's done. With parentId it is a subtask of a task you coordinate: you are told when all its subtasks are finished.",
            "inputSchema": {"type": "object", "properties": {
                "title": {"type": "string", "description": "One line, at most 200 bytes."},
                "detail": {"type": "string", "description": "What to do, relevant context and the expected result."},
                "project": {"type": "string", "description": "Absolute folder the work is in (default: your working directory). A git repository gives each claimer its own worktree."},
                "parentId": {"type": "string", "description": "The task this is a part of."},
            }, "required": ["title"]},
            "annotations": {"readOnlyHint": false, "idempotentHint": false},
        }),
        json!({
            "name": "claim_task",
            "description": "Take an open task: it is yours until you finish or release it, and no other bot can claim it. Answers where to work (a worktree of its own in a git project). You hold at most 3 tasks at once.",
            "inputSchema": {"type": "object", "properties": {"taskId": id}, "required": ["taskId"]},
            "annotations": {"readOnlyHint": false, "idempotentHint": true},
        }),
        json!({
            "name": "note_task",
            "description": "Add a note to any task for the user and the other bots: a finding, a decision, a blocker.",
            "inputSchema": {"type": "object", "properties": {
                "taskId": id,
                "text": {"type": "string", "description": "At most 4000 bytes."},
            }, "required": ["taskId", "text"]},
            "annotations": {"readOnlyHint": false, "idempotentHint": false},
        }),
        json!({
            "name": "finish_task",
            "description": "Finish a task you claimed. With a worktree, commit everything first. Another bot then checks it (it may send it back with what to fix); once accepted, a task with a branch waits for the user to merge it, any other is done. A task with unfinished subtasks can't finish yet.",
            "inputSchema": {"type": "object", "properties": {
                "taskId": id,
                "summary": {"type": "string", "description": "What you did and how you checked it."},
            }, "required": ["taskId", "summary"]},
            "annotations": {"readOnlyHint": false, "idempotentHint": false},
        }),
        json!({
            "name": "release_task",
            "description": "Give a task you claimed back to the board for someone else; its branch and worktree stay for the next claimer.",
            "inputSchema": {"type": "object", "properties": {
                "taskId": id,
                "reason": {"type": "string", "description": "Why, and what the next claimer should know."},
            }, "required": ["taskId"]},
            "annotations": {"readOnlyHint": false, "idempotentHint": false},
        }),
        json!({
            "name": "review_task",
            "description": "Your verdict on a task you were asked to check: approve it when the result does what the task asked and works, or send it back to its claimer with feedback saying exactly what to fix.",
            "inputSchema": {"type": "object", "properties": {
                "taskId": id,
                "approve": {"type": "boolean", "description": "true: it is done. false: send it back."},
                "feedback": {"type": "string", "description": "What you checked, and when sending it back, what to fix. At most 4000 bytes."},
            }, "required": ["taskId", "approve"]},
            "annotations": {"readOnlyHint": false, "idempotentHint": false},
        }),
    ]
}

/// A board tool call from `bot`.
pub async fn call(hub: &Hub, bot: &str, name: &str, args: &Value) -> Result<Value> {
    let id = || args["taskId"].as_str().ok_or_else(|| anyhow!("taskId is required"));
    let task = match name {
        "list_tasks" => return Ok(list(hub, bot, args)),
        "read_task" => super::get(&hub.store, id()?)?,
        "create_task" => {
            let cwd = hub.store.bot(bot)?.map(|r| r.config.cwd).unwrap_or_default();
            let project = args["project"].as_str().filter(|p| !p.trim().is_empty()).unwrap_or(&cwd);
            let new = NewTask {
                title: args["title"].as_str().unwrap_or_default(),
                detail: args["detail"].as_str().unwrap_or_default(),
                project: Some(project),
                parent: args["parentId"].as_str(),
            };
            let task = super::create(hub, Actor::Bot(bot), &new)?;
            if task.parent.is_some() {
                super::sheet::record(hub, bot, super::sheet::Deed::Planned);
            }
            task
        }
        "claim_task" => super::claim(hub, id()?, bot).await?,
        "note_task" => {
            let task = super::note(hub, id()?, Actor::Bot(bot), args["text"].as_str().unwrap_or_default())?;
            super::sheet::record(hub, bot, super::sheet::Deed::Note);
            task
        }
        "finish_task" => super::finish(hub, id()?, bot, args["summary"].as_str().unwrap_or_default()).await?,
        "release_task" => super::release(hub, id()?, Actor::Bot(bot), args["reason"].as_str())?,
        "review_task" => {
            let approve = args["approve"].as_bool().ok_or_else(|| anyhow!("approve is required"))?;
            super::flow::review(hub, id()?, bot, approve, args["feedback"].as_str().unwrap_or_default())?
        }
        _ => bail!("unknown board tool: {name}"),
    };
    let mut answer = json!({"task": full(hub, &task)});
    if name == "claim_task"
        && let Some(tree) = &task.worktree
    {
        answer["workIn"] = worktree_hint(tree, task.branch.as_deref()).into();
    }
    Ok(answer)
}

fn list(hub: &Hub, bot: &str, args: &Value) -> Value {
    let status = args["status"].as_str().and_then(|s| TaskStatus::parse(s).ok());
    let mine = args["mine"].as_bool().unwrap_or(false);
    let tasks: Vec<Value> = super::list(&hub.store)
        .iter()
        .filter(|t| status.is_none_or(|s| t.status == s))
        .filter(|t| !mine || t.claimed_by.as_deref() == Some(bot))
        .map(|t| {
            json!({
                "id": t.id, "title": t.title, "status": t.status,
                "claimedBy": t.claimed_by.as_deref().map(|b| hub.bot_name(b)),
                "parentId": t.parent, "notes": t.notes.len(), "project": t.project,
            })
        })
        .collect();
    json!({"tasks": tasks})
}

/// A task as a bot reads it: names, not ids, for the people involved.
fn full(hub: &Hub, task: &Task) -> Value {
    let who = |by: &Option<String>| by.as_deref().map_or_else(|| "the user".to_owned(), |b| hub.bot_name(b));
    let notes: Vec<Value> = task.notes.iter().map(|n| json!({"by": who(&n.by), "text": n.text, "at": n.at})).collect();
    let name = |by: &Option<String>| by.as_deref().map(|b| hub.bot_name(b));
    let subtasks: Vec<Value> = super::children(&hub.store, &task.id)
        .iter()
        .map(|t| json!({"id": t.id, "title": t.title, "status": t.status, "claimedBy": name(&t.claimed_by), "summary": t.summary}))
        .collect();
    json!({
        "id": task.id, "title": task.title, "detail": task.detail, "status": task.status, "project": task.project,
        "createdBy": who(&task.created_by), "claimedBy": name(&task.claimed_by), "checkedBy": name(&task.reviewer),
        "parentId": task.parent, "subtasks": subtasks,
        "branch": task.branch, "worktree": task.worktree, "notes": notes, "summary": task.summary,
    })
}
