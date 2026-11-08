//! The team board (`docs/features/team-board.md`): the computer's shared tasks by status. The user adds tasks, notes
//! them, hands one to a bot, merges a finished task's branch, and reopens, closes or deletes tasks; the bots claim and
//! finish them through their own tools. `task` events keep an open board current.

use crossterm::event::{KeyCode, KeyEvent};
use serde_json::{Value, json};

use super::super::app::{App, Editor, Overlay};
use super::{BoardSheet, Fields, Input, Reply, Submit};
use crate::board::{Task, TaskStatus};

/// The board's order: open, in progress, ready to merge, done, cancelled; newest first in each.
pub fn sorted(mut tasks: Vec<Task>) -> Vec<Task> {
    let rank = |s: TaskStatus| match s {
        TaskStatus::Open => 0,
        TaskStatus::Claimed => 1,
        TaskStatus::Review => 2,
        TaskStatus::Done => 3,
        TaskStatus::Cancelled => 4,
    };
    tasks.retain(|t| !t.deleted);
    tasks.sort_by_key(|t| (rank(t.status), std::cmp::Reverse(t.updated_at)));
    tasks
}

/// A status as the board's headings name it.
pub fn heading(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "Open",
        TaskStatus::Claimed => "In progress",
        TaskStatus::Review => "Ready to merge",
        TaskStatus::Done => "Done",
        TaskStatus::Cancelled => "Cancelled",
    }
}

impl App {
    pub fn open_board(&mut self) {
        self.sheet("tasks", json!({}), Reply::Board);
        self.overlays.push(Overlay::Board(BoardSheet { tasks: None, cursor: 0, expanded: None, opted_out: vec![] }));
    }

    pub(in crate::tui) fn board_key(&mut self, mut b: BoardSheet, k: KeyEvent) -> Option<Overlay> {
        let n = b.tasks.as_ref().map_or(0, Vec::len);
        let current = b.tasks.as_ref().and_then(|v| v.get(b.cursor)).cloned();
        let status = |s: &str, id: &str| json!({"taskId": id, "status": s});
        match (k.code, current) {
            (KeyCode::Esc | KeyCode::Char('q' | 'B'), _) => return None,
            (KeyCode::Down | KeyCode::Char('j'), _) => b.cursor = (b.cursor + 1).min(n.saturating_sub(1)),
            (KeyCode::Up | KeyCode::Char('k'), _) => b.cursor = b.cursor.saturating_sub(1),
            (KeyCode::Char('n'), _) => return self.push_over(b, new_task_fields()),
            (KeyCode::Char('s'), _) => return self.push_over(b, split_fields()),
            (KeyCode::Char('p'), _) => {
                let claims = self.claim_choices(&b.opted_out);
                if claims.is_empty() {
                    self.flash("No bot to take part");
                } else {
                    return self.push_over(b, claims_fields(claims));
                }
            }
            (KeyCode::Enter, Some(t)) => b.expanded = (b.expanded.as_deref() != Some(t.id.as_str())).then_some(t.id),
            (KeyCode::Char('c'), Some(t)) => return self.push_over(b, note_fields(t.id)),
            (KeyCode::Char('a'), Some(t)) if t.status == TaskStatus::Open => {
                let mut bots: Vec<(String, String)> = self
                    .bots
                    .values()
                    .filter(|x| !x.group && !x.hidden)
                    .map(|x| (x.id.clone(), x.name.clone()))
                    .collect();
                bots.sort_by_key(|(_, name)| name.to_lowercase());
                if bots.is_empty() {
                    self.flash("No bot to assign it to");
                } else {
                    return self.push_over(b, assign_fields(t.id, bots));
                }
            }
            (KeyCode::Char('m'), Some(t)) if t.status == TaskStatus::Review && t.branch.is_some() => {
                self.flash("Merging…");
                self.sheet("mergeTask", json!({"taskId": t.id}), Reply::BoardChanged);
            }
            (KeyCode::Char('o'), Some(t)) if t.status != TaskStatus::Open => {
                self.sheet("setTaskStatus", status("open", &t.id), Reply::BoardChanged);
            }
            (KeyCode::Char('d'), Some(t)) if !matches!(t.status, TaskStatus::Done | TaskStatus::Claimed) => {
                self.sheet("setTaskStatus", status("done", &t.id), Reply::BoardChanged);
            }
            (KeyCode::Char('x'), Some(t)) if t.status != TaskStatus::Cancelled => {
                self.sheet("setTaskStatus", status("cancelled", &t.id), Reply::BoardChanged);
            }
            (KeyCode::Char('X'), Some(t)) if t.status != TaskStatus::Claimed => {
                self.sheet("deleteTask", json!({"taskId": t.id}), Reply::BoardChanged);
            }
            (KeyCode::Char('a' | 'm' | 'o' | 'd' | 'x' | 'X'), Some(_)) => self.flash("Not for a task in this status"),
            _ => {}
        }
        Some(Overlay::Board(b))
    }

    /// Opens a Fields sheet over the board, which stays open under it.
    fn push_over(&mut self, b: BoardSheet, fields: Fields) -> Option<Overlay> {
        self.overlays.push(Overlay::Board(b));
        self.overlays.push(Overlay::Fields(Box::new(fields)));
        None
    }

    /// This computer's bots by name, each with whether it claims tasks on its own.
    fn claim_choices(&self, opted_out: &[String]) -> Vec<(String, String, bool)> {
        let mut bots: Vec<(String, String, bool)> = self
            .bots
            .values()
            .filter(|x| !x.group && !x.hidden)
            .map(|x| (x.id.clone(), x.name.clone(), !opted_out.contains(&x.id)))
            .collect();
        bots.sort_by_key(|(_, name, _)| name.to_lowercase());
        bots
    }

    /// Closes a Fields sheet the board opened, once its change is saved.
    pub(in crate::tui) fn close_board_fields(&mut self) {
        if matches!(self.overlays.last(), Some(Overlay::Fields(f)) if matches!(f.submit, Submit::SplitGoal | Submit::BoardClaims(_)))
        {
            self.overlays.pop();
        }
    }

    /// The `tasks` (or `setBoardClaims`) answer: shown on the open board.
    pub(in crate::tui) fn board_loaded(&mut self, v: &Value) {
        let opted_out: Option<Vec<String>> = v.get("optedOut").and_then(|o| serde_json::from_value(o.clone()).ok());
        let tasks: Option<Vec<Task>> = v["tasks"]
            .as_array()
            .map(|a| sorted(a.iter().filter_map(|t| serde_json::from_value(t.clone()).ok()).collect()));
        for o in &mut self.overlays {
            if let Overlay::Board(b) = o {
                if let Some(tasks) = &tasks {
                    b.cursor = b.cursor.min(tasks.len().saturating_sub(1));
                    b.tasks = Some(tasks.clone());
                }
                if let Some(out) = &opted_out {
                    b.opted_out.clone_from(out);
                }
            }
        }
    }

    /// A `task` event: the open board takes the newer copy (a deleted task leaves it).
    pub(in crate::tui) fn task_changed(&mut self, v: &Value) {
        let Ok(task) = serde_json::from_value::<Task>(v.clone()) else { return };
        for o in &mut self.overlays {
            let Overlay::Board(b) = o else { continue };
            let Some(tasks) = b.tasks.as_mut() else { continue };
            if tasks.iter().any(|t| t.id == task.id && t.rev > task.rev) {
                continue;
            }
            tasks.retain(|t| t.id != task.id);
            tasks.push(task.clone());
            *tasks = sorted(std::mem::take(tasks));
            b.cursor = b.cursor.min(tasks.len().saturating_sub(1));
        }
    }
}

fn input(name: &str, label: &str, required: bool, multiline: bool, placeholder: &str) -> Input {
    Input {
        name: name.into(),
        label: label.into(),
        secret: false,
        required,
        multiline,
        placeholder: placeholder.into(),
        ed: Editor::with(""),
    }
}

fn fields(title: &str, note: &str, inputs: Vec<Input>, choices: Vec<String>, submit: Submit) -> Fields {
    Fields {
        title: title.into(),
        note: note.into(),
        choices,
        choice: 0,
        inputs,
        cursor: 0,
        error: None,
        saving: false,
        submit,
    }
}

fn new_task_fields() -> Fields {
    let inputs = vec![
        input("title", "Task title", true, false, ""),
        input("detail", "Detail", false, true, "What to do, and how to check it is done"),
        input("project", "Project folder (optional)", false, false, "/absolute/path"),
    ];
    let note = "A git project gives the bot that claims it its own worktree and branch. ⇧↵ new line, ↵ adds.";
    fields("New task", note, inputs, vec![], Submit::NewTask)
}

fn split_fields() -> Fields {
    let inputs = vec![input("goal", "Goal", true, true, "e.g. prepare both kids' exam review papers")];
    let note = "Your lead bot splits it into tasks, and the bots claim them. ⇧↵ new line, ↵ hands it over.";
    fields("Split a goal into tasks", note, inputs, vec![], Submit::SplitGoal)
}

/// ←→ picks a bot; ↵ turns its claiming on or off.
fn claims_fields(bots: Vec<(String, String, bool)>) -> Fields {
    let choices = bots.iter().map(|(_, name, on)| format!("{name}: {}", if *on { "claims" } else { "off" })).collect();
    let note = "Bots that claim tasks on their own hear about open tasks while idle. ←→ picks a bot; ↵ switches it.";
    let bots = bots.into_iter().map(|(id, _, on)| (id, on)).collect();
    fields("Bots that claim tasks on their own", note, vec![], choices, Submit::BoardClaims(bots))
}

fn note_fields(task: String) -> Fields {
    let inputs = vec![input("text", "Note", true, true, "Add a note for the team")];
    fields("Add a note for the team", "", inputs, vec![], Submit::NoteTask(task))
}

/// Picks one of this computer's bots (←→); ↵ claims the task for it and starts it.
fn assign_fields(task: String, bots: Vec<(String, String)>) -> Fields {
    let (ids, names): (Vec<String>, Vec<String>) = bots.into_iter().unzip();
    let note = "←→ picks the bot; ↵ claims the task for it, and it starts in its own chat.";
    fields("Assign task", note, vec![], names, Submit::AssignTask { task, bots: ids })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, status: &str, updated_at: i64, rev: i64) -> Value {
        json!({"id": id, "title": id, "detail": "", "project": null, "status": status, "createdBy": null,
               "claimedBy": null, "branch": null, "worktree": null, "notes": [], "summary": null,
               "createdAt": 0, "updatedAt": updated_at, "rev": rev})
    }

    fn app() -> App {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        App::new(crate::tui::net::Client::new("http://127.0.0.1:1", None), tx, String::new())
    }

    #[tokio::test]
    async fn the_board_sorts_by_status_and_follows_task_events() {
        let mut app = app();
        app.overlays.push(Overlay::Board(BoardSheet { tasks: None, cursor: 0, expanded: None, opted_out: vec![] }));
        app.board_loaded(
            &json!({"tasks": [task("a", "done", 5, 5), task("b", "open", 1, 1), task("c", "open", 3, 3)]}),
        );
        let ids = |app: &App| match app.overlays.last() {
            Some(Overlay::Board(b)) => b.tasks.iter().flatten().map(|t| t.id.clone()).collect::<Vec<_>>(),
            _ => panic!("the board stays open"),
        };
        assert_eq!(ids(&app), ["c", "b", "a"], "open first, newest first");

        app.task_changed(&task("b", "claimed", 9, 9));
        assert_eq!(ids(&app), ["c", "b", "a"], "in progress after open");
        app.task_changed(&task("b", "open", 2, 2));
        let Some(Overlay::Board(b)) = app.overlays.last() else { panic!("open") };
        let b_status = b.tasks.iter().flatten().find(|t| t.id == "b").map(|t| t.status);
        assert_eq!(b_status, Some(TaskStatus::Claimed), "an older copy is ignored");

        let mut gone = task("c", "open", 10, 10);
        gone["deleted"] = true.into();
        app.task_changed(&gone);
        assert_eq!(ids(&app), ["b", "a"]);
    }

    #[tokio::test]
    async fn the_board_keeps_which_bots_claim_and_offers_each_one() {
        let mut app = app();
        app.on_msg(crate::tui::app::Msg::Event(json!({"type": "bot", "bot": {"id": "a", "name": "Ada"}})));
        app.on_msg(crate::tui::app::Msg::Event(json!({"type": "bot", "bot": {"id": "b", "name": "Bo"}})));
        app.overlays.push(Overlay::Board(BoardSheet { tasks: None, cursor: 0, expanded: None, opted_out: vec![] }));
        app.board_loaded(&json!({"tasks": [], "optedOut": ["b"]}));
        let Some(Overlay::Board(b)) = app.overlays.last() else { panic!("open") };
        assert_eq!(b.opted_out, ["b"]);
        let choices = app.claim_choices(&["b".to_owned()]);
        assert_eq!(choices, [("a".to_owned(), "Ada".to_owned(), true), ("b".to_owned(), "Bo".to_owned(), false)]);
        let fields = claims_fields(choices);
        assert_eq!(fields.choices, ["Ada: claims", "Bo: off"]);
        app.overlays.push(Overlay::Fields(Box::new(fields)));
        app.board_loaded(&json!({"optedOut": []}));
        app.close_board_fields();
        let Some(Overlay::Board(b)) = app.overlays.last() else { panic!("the sheet closed, the board stays") };
        assert!(b.opted_out.is_empty(), "a setBoardClaims answer carries no tasks: they stay");
        assert!(b.tasks.is_some());
    }
}
