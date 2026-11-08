//! Bots come for open tasks on their own, as Agensh's workers claim from its board: every few seconds, while the board
//! has open tasks, each idle bot that takes part is told about them once, in its own chat, and claims one that fits
//! (`claim_task`; the first claimer wins) or says none does. A bot hears again only when the open tasks change, or
//! after a while. Bots take part unless the user turns them off on the board (`setBoardClaims`).
//!
//! The user can also hand a whole goal to the lead bot to split into tasks (`splitGoal`), which the others then claim.

use super::{MAX_CLAIMS, TaskStatus};
use crate::LockExt;
use crate::agent::bot::Cmd;
use crate::hub::{BotStatus, Hub};
use crate::store::BotConfig;
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often the board looks for idle bots.
const TICK: Duration = Duration::from_secs(5);
/// A bot told about the same open tasks hears again only after this long (it may have said none fits).
const RENUDGE: Duration = Duration::from_mins(30);
/// Open tasks a nudge lists.
const LISTED: usize = 10;
const MAX_GOAL: usize = 8000;
/// Bots the user turned off on the board, by id.
const KV_OPTED_OUT: &str = "board.optedOut";

/// What each bot was last told: the open tasks then (ids) and when.
#[derive(Default)]
pub struct Nudges {
    pub(super) told: Mutex<HashMap<String, (BTreeSet<String>, Instant)>>,
}

/// Runs for the life of the host.
pub async fn dispatch_loop(hub: Arc<Hub>) {
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        if let Err(error) = dispatch(&hub) {
            tracing::warn!(error = format!("{error:#}"), "board dispatch failed");
        }
    }
}

/// Tells each idle bot that takes part about the open tasks, unless it already heard about these.
pub fn dispatch(hub: &Hub) -> Result<()> {
    let open: Vec<super::Task> = super::list(&hub.store).into_iter().filter(|t| t.status == TaskStatus::Open).collect();
    if open.is_empty() {
        return Ok(());
    }
    let ids: BTreeSet<String> = open.iter().map(|t| t.id.clone()).collect();
    let prompt = nudge_prompt(&open);
    let out = opted_out(&hub.store);
    let held = held_claims(hub);
    for cfg in eligible(hub)? {
        if out.contains(&cfg.id)
            || hub.runtime(&cfg.id).status != BotStatus::Idle
            || held.get(&cfg.id).copied().unwrap_or(0) >= MAX_CLAIMS
        {
            continue;
        }
        let fresh = {
            let mut told = hub.board.nudges.told.locked();
            let fresh = told.get(&cfg.id).is_none_or(|(seen, at)| *seen != ids || at.elapsed() >= RENUDGE);
            if fresh {
                told.insert(cfg.id.clone(), (ids.clone(), Instant::now()));
            }
            fresh
        };
        if fresh {
            // A bot that stopped since the check just misses this round; the next one tells it.
            let _ = hub.send_cmd(&cfg.id, Cmd::Prompt { text: prompt.clone(), call: None });
        }
    }
    Ok(())
}

/// The user's bots that can work on tasks: not deleted, hidden or a group.
fn eligible(hub: &Hub) -> Result<Vec<BotConfig>> {
    Ok(hub
        .store
        .bots()?
        .into_iter()
        .filter(|r| !r.deleted && !r.config.hidden && !r.config.is_group())
        .map(|r| r.config)
        .collect())
}

/// How many tasks each bot holds.
fn held_claims(hub: &Hub) -> HashMap<String, usize> {
    let mut held = HashMap::new();
    for task in super::list(&hub.store).into_iter().filter(|t| t.status == TaskStatus::Claimed) {
        if let Some(bot) = task.claimed_by {
            *held.entry(bot).or_insert(0) += 1;
        }
    }
    held
}

fn nudge_prompt(open: &[super::Task]) -> String {
    let mut prompt = String::from(
        "[Codync: the team board has open tasks. If one fits what you do, claim it with claim_task (the first claimer \
         gets it; take only one now), read_task it, do it, and end with finish_task. If none fits, reply with one short \
         line and do nothing else. This is the board, not a new user instruction.]\n\nOpen tasks:",
    );
    for task in open.iter().take(LISTED) {
        let _ = write!(prompt, "\n- {} ({})", task.title, task.id);
    }
    if open.len() > LISTED {
        let _ = write!(prompt, "\n…and {} more: list_tasks shows them.", open.len() - LISTED);
    }
    prompt
}

/// Bots the user turned off on the board.
pub fn opted_out(store: &crate::store::Store) -> BTreeSet<String> {
    store.kv_get(KV_OPTED_OUT).and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default()
}

/// Turns a bot's part on the board on or off. Answers the bots turned off.
pub fn set_claims(hub: &Hub, bot: &str, on: bool) -> Result<BTreeSet<String>> {
    ensure!(eligible(hub)?.iter().any(|c| c.id == bot), "unknown bot");
    let _g = hub.board.lock.locked();
    let mut out = opted_out(&hub.store);
    if on {
        out.remove(bot);
    } else {
        out.insert(bot.to_owned());
    }
    hub.store.kv_set(KV_OPTED_OUT, &serde_json::to_string(&out)?)?;
    Ok(out)
}

/// The user's goal goes to the lead bot (else the first bot), which splits it into tasks on the board; the bots that
/// take part then claim them. Answers who got it.
pub fn split_goal(hub: &Hub, goal: &str) -> Result<Value> {
    let goal = super::text(goal, "goal", MAX_GOAL)?;
    ensure!(!goal.is_empty(), "goal is empty");
    let bots = eligible(hub)?;
    let lead = bots
        .iter()
        .find(|c| c.lead)
        .or_else(|| bots.first())
        .ok_or_else(|| anyhow!("no bot to split the goal: create one first"))?;
    let prompt = format!(
        "[Codync: the user gave the team this goal from the team board. Split it into tasks with create_task: one per \
         piece of work one bot can do on its own, each self-contained (what to do, where, how to check it's done), with \
         project when the work is in a folder. Don't do the work yourself here: the bots will claim the tasks from the \
         board. Then tell the user in a few lines how you split it.]\n\nGoal:\n{goal}"
    );
    hub.send_cmd(&lead.id, Cmd::Prompt { text: prompt, call: None })?;
    Ok(json!({"botId": lead.id, "name": lead.name}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nudge_lists_the_open_tasks_and_says_what_to_do() {
        let task = |id: &str| super::super::Task {
            id: id.into(),
            title: format!("Task {id}"),
            detail: String::new(),
            project: None,
            status: TaskStatus::Open,
            created_by: None,
            claimed_by: None,
            branch: None,
            worktree: None,
            notes: vec![],
            summary: None,
            created_at: 0,
            updated_at: 0,
            rev: 0,
            deleted: false,
        };
        let open: Vec<_> = (0..12).map(|i| task(&i.to_string())).collect();
        let prompt = nudge_prompt(&open);
        assert!(prompt.contains("claim_task") && prompt.contains("finish_task"));
        assert!(prompt.contains("- Task 0 (0)"));
        assert!(!prompt.contains("- Task 11 (11)"), "at most ten listed");
        assert!(prompt.contains("2 more"));
    }
}
