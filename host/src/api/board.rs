//! The team board's methods for the apps: `tasks` (with the bots turned off from claiming), `createTask`, `editTask`,
//! `noteTask`, `assignTask`, `setTaskStatus`, `mergeTask`, `deleteTask`, `approvePlan`, `revisePlan`, `setBoardClaims`, `splitGoal`, `botSheets` and `setBotClass`. Task
//! changes reach every client as `task` events.

use super::str_arg;
use crate::board::{self, Actor, NewTask, TaskStatus};
use crate::hub::Hub;
use anyhow::Result;
use serde_json::{Value, json};
use std::ops::ControlFlow;
use std::sync::Arc;

pub(super) async fn call(hub: &Arc<Hub>, method: &str, b: Value) -> Result<ControlFlow<Value, Value>> {
    let task = match method {
        "tasks" => {
            let out = board::dispatch::opted_out(&hub.store);
            return Ok(ControlFlow::Break(json!({"tasks": board::list(&hub.store), "optedOut": out})));
        }
        // Whether a bot comes for open tasks on its own (on unless the user turns it off).
        "setBoardClaims" => {
            let out = board::dispatch::set_claims(hub, str_arg(&b, "botId")?, b["on"].as_bool().unwrap_or(true))?;
            return Ok(ControlFlow::Break(json!({"optedOut": out})));
        }
        // The bots' character sheets: class, level and abilities (`board::sheet`).
        "botSheets" => return Ok(ControlFlow::Break(json!({"sheets": board::sheet::sheets(hub)?}))),
        "setBotClass" => {
            let class = b["class"].as_str().map(board::sheet::BotClass::parse).transpose()?;
            let sheet = board::sheet::set_class(hub, str_arg(&b, "botId")?, class)?;
            return Ok(ControlFlow::Break(json!({"sheet": sheet})));
        }
        "splitGoal" => return Ok(ControlFlow::Break(board::dispatch::split_goal(hub, str_arg(&b, "goal")?)?)),
        "createTask" => {
            let new = NewTask {
                title: str_arg(&b, "title")?,
                detail: b["detail"].as_str().unwrap_or_default(),
                project: b["project"].as_str(),
                parent: None,
                acceptance: board::brief::criteria(&b["acceptance"])?,
                // The user approves the plan before any work unless they turn it off.
                approve_plan: b["approvePlan"].as_bool().unwrap_or(true),
            };
            let task = board::create(hub, Actor::User, &new)?;
            // The lead plans it: splits it into subtasks, takes it, or leaves it for a better-fitting bot. With no bot
            // yet, it just waits on the board.
            if let Err(error) = board::flow::plan(hub, &task.id, false) {
                tracing::info!(error = format!("{error:#}"), "new task left open: no bot to plan it");
            }
            board::get(&hub.store, &task.id)?
        }
        "editTask" => board::edit(hub, str_arg(&b, "taskId")?, b["title"].as_str(), b["detail"].as_str())?,
        "approvePlan" => board::brief::approve(hub, str_arg(&b, "taskId")?)?,
        "revisePlan" => board::brief::revise(hub, str_arg(&b, "taskId")?, str_arg(&b, "note")?)?,
        "noteTask" => board::note(hub, str_arg(&b, "taskId")?, Actor::User, str_arg(&b, "text")?)?,
        "assignTask" => board::assign(hub, str_arg(&b, "taskId")?, str_arg(&b, "botId")?, Actor::User).await?,
        "setTaskStatus" => {
            board::set_status(hub, str_arg(&b, "taskId")?, TaskStatus::parse(str_arg(&b, "status")?)?).await?
        }
        "mergeTask" => board::merge(hub, str_arg(&b, "taskId")?).await?,
        "deleteTask" => {
            board::delete(hub, str_arg(&b, "taskId")?).await?;
            return Ok(ControlFlow::Break(json!({"ok": true})));
        }
        _ => return Ok(ControlFlow::Continue(b)),
    };
    Ok(ControlFlow::Break(json!({"task": task})))
}
