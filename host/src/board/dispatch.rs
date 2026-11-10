//! Bots come for open tasks on their own, as Agensh's workers claim from its board: every few seconds, while the board
//! has open tasks, each idle bot that takes part is told about them once, in its own chat, and claims one that fits
//! (`claim_task`; the first claimer wins) or says none does. A bot hears again only when the open tasks change, or
//! after a while. Bots take part unless the user turns them off on the board (`setBoardClaims`).
//!
//! The user can also hand a whole goal to the lead bot to split into tasks (`splitGoal`): it becomes a task the lead
//! plans and coordinates (`flow::plan`), whose subtasks the others claim.

use super::{MAX_CLAIMS, TaskStatus};
use crate::LockExt;
use crate::agent::bot::Cmd;
use crate::hub::{BotStatus, Hub};
use crate::store::BotConfig;
use anyhow::{Result, ensure};
use serde_json::Value;
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
/// A goal's title, in bytes, before the ellipsis (`super::MAX_TITLE` holds it with room to spare).
const TITLE_BYTES: usize = 180;
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

/// Tells each idle bot that takes part about the open tasks, unless it already heard about these or is too tired
/// (`flow::TIRED`).
pub fn dispatch(hub: &Hub) -> Result<()> {
    let open: Vec<super::Task> = super::list(&hub.store).into_iter().filter(|t| t.status == TaskStatus::Open).collect();
    if open.is_empty() {
        return Ok(());
    }
    let ids: BTreeSet<String> = open.iter().map(|t| t.id.clone()).collect();
    let out = opted_out(&hub.store);
    let held = held_claims(hub);
    for cfg in eligible(hub)? {
        if out.contains(&cfg.id)
            || !free(hub.runtime(&cfg.id).status)
            || held.get(&cfg.id).copied().unwrap_or(0) >= MAX_CLAIMS
            || super::flow::tired(hub, &cfg.backend)
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
            let text = nudge_prompt(&open, &super::flow::teammates(hub, &cfg.id));
            let _ = hub.send_cmd(&cfg.id, Cmd::Prompt { text, call: None });
        }
    }
    Ok(())
}

/// Whether a bot in this state can take board work now: idle, or stopped by a failed turn (an error ends that turn;
/// a new prompt starts afresh, so a bot that failed once, e.g. before its agent was signed in, isn't left out for good).
pub(super) fn free(status: BotStatus) -> bool {
    matches!(status, BotStatus::Idle | BotStatus::Error)
}

/// The user's bots that can work on tasks: not deleted, hidden or a group.
pub(super) fn eligible(hub: &Hub) -> Result<Vec<BotConfig>> {
    Ok(hub
        .store
        .bots()?
        .into_iter()
        .filter(|r| !r.deleted && !r.config.hidden && !r.config.is_group())
        .map(|r| r.config)
        .collect())
}

/// How many tasks each bot holds.
pub(super) fn held_claims(hub: &Hub) -> HashMap<String, usize> {
    let mut held = HashMap::new();
    for task in super::list(&hub.store).into_iter().filter(|t| t.status == TaskStatus::Claimed) {
        if let Some(bot) = task.claimed_by {
            *held.entry(bot).or_insert(0) += 1;
        }
    }
    held
}

fn nudge_prompt(open: &[super::Task], teammates: &str) -> String {
    let mut prompt = String::from(
        "[Codync: the team board has open tasks. Claim one only if it fits what you are good at better than your \
         teammates (below), and leave the rest for them: claim_task it (the first claimer gets it; take only one now), \
         read_task it, do it, and end with finish_task. If it turns out to hold big parts others could do side by side \
         (each real work, not minutes), don't do it all alone: add them as subtasks (create_task with its parentId), \
         coordinate, and assemble them when you are told they are finished. If none fits, reply with one short line \
         and do nothing else. This is the board, not a new user instruction.]\n\nOpen tasks:",
    );
    for task in open.iter().take(LISTED) {
        let _ = write!(prompt, "\n- {} ({})", task.title, task.id);
    }
    if open.len() > LISTED {
        let _ = write!(prompt, "\n…and {} more: list_tasks shows them.", open.len() - LISTED);
    }
    let _ = write!(prompt, "\n\n{teammates}");
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

/// The bot that plans the user's tasks: the lead, else the first bot.
pub(super) fn lead(hub: &Hub) -> Result<BotConfig> {
    let mut bots = eligible(hub)?;
    let at = bots.iter().position(|c| c.lead).unwrap_or(0);
    ensure!(!bots.is_empty(), "no bot to plan the task: create one first");
    Ok(bots.swap_remove(at))
}

/// The user's goal becomes a task (titled by its first line) that the lead splits into subtasks the bots claim, and
/// coordinates. Answers who got it.
pub fn split_goal(hub: &Hub, goal: &str) -> Result<Value> {
    let goal = super::text(goal, "goal", MAX_GOAL)?;
    ensure!(!goal.is_empty(), "goal is empty");
    lead(hub)?;
    let task = super::create(
        hub,
        super::Actor::User,
        &super::NewTask { title: &goal_title(&goal), detail: &goal, project: None, parent: None },
    )?;
    super::flow::plan(hub, &task.id, true)
}

/// A goal's first line, cut to fit a title.
fn goal_title(goal: &str) -> String {
    let line = goal.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Goal");
    let mut title = String::new();
    for c in line.chars() {
        if title.len() + c.len_utf8() > TITLE_BYTES {
            title.push('…');
            break;
        }
        title.push(c);
    }
    title
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
            parent: None,
            reviewer: None,
            send_backs: 0,
            created_at: 0,
            updated_at: 0,
            rev: 0,
            deleted: false,
        };
        let open: Vec<_> = (0..12).map(|i| task(&i.to_string())).collect();
        let prompt = nudge_prompt(&open, "Teammates:\n- Ada (codex)");
        assert!(prompt.contains("claim_task") && prompt.contains("finish_task") && prompt.contains("parentId"));
        assert!(prompt.ends_with("- Ada (codex)"), "the teammates close the nudge");
        assert!(prompt.contains("- Task 0 (0)"));
        assert!(!prompt.contains("- Task 11 (11)"), "at most ten listed");
        assert!(prompt.contains("2 more"));
    }

    #[test]
    fn a_goal_is_titled_by_its_first_line_cut_to_fit() {
        assert_eq!(goal_title("\n  Ship it  \nthen more"), "Ship it");
        let long = goal_title(&"準".repeat(100));
        assert!(long.len() <= TITLE_BYTES + '…'.len_utf8() && long.ends_with('…'));
    }
}
