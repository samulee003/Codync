//! The board's tools on each bot's `team` MCP server: read the board, add tasks and subtasks, claim one, note
//! findings for the others, finish or release it, and check another bot's finished work.

use super::{Actor, NewTask, Task, TaskStatus};
use crate::hub::Hub;
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

pub const TOOLS: [&str; 9] = [
    "list_tasks",
    "read_task",
    "create_task",
    "claim_task",
    "propose_plan",
    "note_task",
    "finish_task",
    "release_task",
    "review_task",
];

pub const INSTRUCTIONS: &str = " The team board (list_tasks) holds tasks the user and the bots share. Take only work \
that fits you better than your teammates. To split a job with parts others can do side by side, add the parts as \
subtasks (create_task with parentId) and coordinate: you are told when they are all finished, then assemble the \
result. To take a task, claim_task it (one claimer per task, so two bots never do the same work), then read_task for \
its notes. A task may have acceptance criteria: the check marks each pass or fail, so meet every one. When the \
user approves a task's plan first, propose_plan before any work and wait to be told it is approved. Share what others could use (a finding, a decision, a blocker) with note_task. A task in a git project \
gives you its own worktree and branch when you claim it: work and commit only there, never in the project's own \
checkout. End with finish_task and a summary of what you did and how you checked it, or release_task when you can't \
do it. Another bot then checks your work and may send it back with what to fix; when you are asked to check a task, \
look at the result itself and answer with review_task, judging every acceptance criterion strictly.";

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
                "acceptance": {"type": "array", "items": {"type": "string"}, "description": "What must hold for it to count as done, one checkable criterion each (at most 12): the check marks each pass or fail."},
                "assignee": {"type": "string", "description": "A teammate's name: the subtask is claimed for them and they start on it at once. Leave it out to let any bot claim it."},
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
            "name": "propose_plan",
            "description": "For a task you claimed whose plan the user approves first: propose how you'll do it, then stop and wait. You're told when it is approved, or what to change.",
            "inputSchema": {"type": "object", "properties": {
                "taskId": id,
                "plan": {"type": "string", "description": "Your size call, then the steps or parts and who does each. At most 6000 bytes."},
                "acceptance": {"type": "array", "items": {"type": "string"}, "description": "The acceptance criteria: checkable, one per item (at most 12). Required when the task has none; replaces its criteria when given."},
            }, "required": ["taskId", "plan"]},
            "annotations": {"readOnlyHint": false, "idempotentHint": false},
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
            "description": "Your verdict on a task you were asked to check. A task with acceptance criteria: give checks, one per criterion; it is accepted only when every one passes. Without criteria: approve it when the result does what the task asked and works, or send it back with feedback saying exactly what to fix.",
            "inputSchema": {"type": "object", "properties": {
                "taskId": id,
                "checks": {"type": "array", "description": "One per acceptance criterion.", "items": {"type": "object", "properties": {
                    "criterion": {"type": "integer", "description": "The criterion's number, from 1."},
                    "pass": {"type": "boolean"},
                    "note": {"type": "string", "description": "What you saw; required when it fails: what to fix."},
                }, "required": ["criterion", "pass"]}},
                "approve": {"type": "boolean", "description": "true: it is done. false: send it back. With checks it must match them."},
                "feedback": {"type": "string", "description": "What you checked, and when sending it back, what to fix. At most 4000 bytes."},
            }, "required": ["taskId"]},
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
                acceptance: super::brief::criteria(&args["acceptance"])?,
                approve_plan: false,
            };
            let task = super::create(hub, Actor::Bot(bot), &new)?;
            if task.parent.is_some() {
                super::sheet::record(hub, bot, super::sheet::Deed::Planned);
            }
            match args["assignee"].as_str().map(str::trim).filter(|a| !a.is_empty()) {
                Some(name) => {
                    let to = teammate(hub, name)?;
                    super::assign(hub, &task.id, &to, Actor::Bot(bot)).await?
                }
                None => task,
            }
        }
        "claim_task" => super::claim(hub, id()?, bot).await?,
        "propose_plan" => {
            let plan = args["plan"].as_str().unwrap_or_default();
            super::brief::propose(hub, id()?, bot, plan, &args["acceptance"])?
        }
        "note_task" => {
            let task = super::note(hub, id()?, Actor::Bot(bot), args["text"].as_str().unwrap_or_default())?;
            super::sheet::record(hub, bot, super::sheet::Deed::Note);
            task
        }
        "finish_task" => super::finish(hub, id()?, bot, args["summary"].as_str().unwrap_or_default()).await?,
        "release_task" => super::release(hub, id()?, Actor::Bot(bot), args["reason"].as_str())?,
        "review_task" => {
            let verdict = super::flow::Verdict {
                approve: args["approve"].as_bool(),
                checks: &args["checks"],
                feedback: args["feedback"].as_str().unwrap_or_default(),
            };
            super::flow::review(hub, id()?, bot, verdict)?
        }
        _ => bail!("unknown board tool: {name}"),
    };
    let mut answer = json!({"task": full(hub, &task)});
    if name == "claim_task" {
        if let Some(tree) = &task.worktree {
            answer["workIn"] = worktree_hint(tree, task.branch.as_deref()).into();
        }
        let order = super::brief::work_order(&task);
        if !order.is_empty() {
            answer["workOrder"] = order.trim().into();
        }
    }
    Ok(answer)
}

/// A teammate on the board by name (or id); the error lists who there is.
fn teammate(hub: &Hub, name: &str) -> Result<String> {
    let bots = super::dispatch::eligible(hub)?;
    bots.iter().find(|c| c.name == name || c.id == name).map(|c| c.id.clone()).ok_or_else(|| {
        let names: Vec<&str> = bots.iter().map(|c| c.name.as_str()).collect();
        anyhow!("no teammate named {name}: the team is {}", names.join(", "))
    })
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
                "parentId": t.parent, "notes": t.notes.len(), "project": t.project, "approval": t.approval,
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
        "acceptance": task.acceptance, "approval": task.approval, "plan": task.plan, "checks": task.checks,
        "branch": task.branch, "worktree": task.worktree, "notes": notes, "summary": task.summary,
    })
}
