//! The team board: tasks by column under their headings (the ones waiting for you first), the one under the cursor
//! unfolded on ↵ (its work order, detail, notes, summary and where its work is).

use super::board_work_view::{effort_line, work_order};
use super::{App, Buffer, Rect, centered, footer, frame_box, name_of, put, row, rput, theme, u, window};
use crate::board::{Task, TaskStatus};
use crate::tui::l10n::tr;
use crate::tui::manage::{
    BoardSheet, Column, awaits_answers, awaits_approval, task_column, task_heading, waiting_titles,
};
use crate::tui::md::truncate;
use crate::tui::view::when;

pub fn draw(buf: &mut Buffer, area: Rect, app: &App, b: &BoardSheet) {
    let t = theme();
    let r = centered(area, 96, 30);
    let title = tr("Team board");
    let inner = frame_box(buf, r, t.text, t.panel, Some((title.as_ref(), t.text)));
    match b.tasks.as_deref() {
        None => {
            put(buf, inner.x, inner.y, inner.width, &tr("Loading…"), t.dim.patch(t.panel));
        }
        Some([]) => {
            put(buf, inner.x, inner.y, inner.width, &tr("No tasks yet"), t.bold.patch(t.panel));
            let note = tr("Add a task with +, or ask a bot to split a job into tasks.").replace('+', "n");
            put(buf, inner.x, inner.y + 1, inner.width, &note, t.secondary.patch(t.panel));
        }
        Some(tasks) => {
            let lines = lines(app, tasks, b);
            let at = lines.iter().position(|l| matches!(l, Line::Task(i) if *i == b.cursor)).unwrap_or(0);
            let h = usize::from(inner.height.saturating_sub(3));
            let room = usize::from(inner.width.saturating_sub(14));
            for (y, line) in (inner.y..).zip(lines.iter().skip(window(at, h)).take(h)) {
                match line {
                    Line::Heading(column, count) => {
                        let text = format!("{} · {count}", tr(task_heading(*column)));
                        put(buf, inner.x, y, inner.width, &text, t.dim.patch(t.panel));
                    }
                    Line::Task(i) => {
                        let task = &tasks[*i];
                        let base = row(buf, inner, y, *i == b.cursor);
                        let label = format!("{} {}", mark(task.status), task.title);
                        put(buf, inner.x, y, u(room) + 2, &truncate(&label, room + 2), t.text.patch(base));
                        rput(buf, inner.right(), y, &meta(app, tasks, task), t.dim.patch(base));
                    }
                    Line::Detail(text) => {
                        let shown = truncate(text, usize::from(inner.width.saturating_sub(4)));
                        put(buf, inner.x + 4, y, inner.width.saturating_sub(4), &shown, t.secondary.patch(t.panel));
                    }
                }
            }
        }
    }
    footer(
        buf,
        inner,
        None,
        "n new · s split · p bots · ↵ more · c note · a assign · m merge · o/d/x reopen/done/cancel · X del",
    );
    // The row above the keys is free: the task under the cursor says what it waits for.
    let current = b.tasks.as_deref().and_then(|tasks| tasks.get(b.cursor));
    if let Some(hint) = action_hint(current) {
        put(buf, inner.x, inner.bottom() - 2, inner.width, &hint, t.text.patch(t.panel));
    }
}

/// The keys that act on a task waiting for the user, in the words the other apps use.
fn action_hint(task: Option<&Task>) -> Option<String> {
    let task = task?;
    if awaits_approval(task) {
        Some(format!("y {} · r {}", tr("Approve plan"), tr("Ask for changes")))
    } else if awaits_answers(task) {
        Some("i answer the questions".to_owned())
    } else {
        None
    }
}

enum Line {
    Heading(Column, usize),
    Task(usize),
    Detail(String),
}

/// The board's lines: a heading before each column's tasks, and the unfolded task's details under it.
fn lines(app: &App, tasks: &[Task], b: &BoardSheet) -> Vec<Line> {
    let mut out = vec![];
    for (i, task) in tasks.iter().enumerate() {
        let column = task_column(task);
        if i == 0 || task_column(&tasks[i - 1]) != column {
            out.push(Line::Heading(column, tasks.iter().filter(|x| task_column(x) == column).count()));
        }
        out.push(Line::Task(i));
        if b.expanded.as_deref() == Some(task.id.as_str()) {
            out.extend(details(app, task).into_iter().map(Line::Detail));
        }
    }
    out
}

/// An unfolded task: its detail, its work order, where its work is, its effort, its summary and its notes, one line
/// each.
fn details(app: &App, task: &Task) -> Vec<String> {
    let mut out: Vec<String> = task.detail.lines().filter(|l| !l.trim().is_empty()).map(str::to_owned).collect();
    out.extend(work_order(task));
    if let Some(at) = task.worktree.as_ref().or(task.project.as_ref()) {
        out.push(format!("▸ {at}"));
    }
    out.extend(effort_line(task));
    if let Some(summary) = &task.summary {
        out.push(format!("✓ {}", summary.replace('\n', " ")));
    }
    for note in &task.notes {
        let by = note.by.as_deref().map_or_else(|| tr("You").into_owned(), |b| who(app, b));
        out.push(format!("{by}: {}", note.text.replace('\n', " ")));
    }
    out
}

fn mark(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "○",
        TaskStatus::Claimed => "◐",
        TaskStatus::Checking => "◎",
        TaskStatus::Review => "⇄",
        TaskStatus::Done => "✓",
        TaskStatus::Cancelled => "×",
    }
}

/// Who holds it, what it waits for, who it is for, what waits on the user, who checks it, the task it is part of, its
/// branch, and when it last changed.
fn meta(app: &App, tasks: &[Task], task: &Task) -> String {
    let waits = waiting_titles(tasks, task);
    let waits = (!waits.is_empty()).then(|| tr("Waits for {titles}").replace("{titles}", &waits.join(", ")));
    let for_bot = (task.status == TaskStatus::Open)
        .then(|| task.assignee.as_deref().map(|b| tr("For {name}").replace("{name}", &who(app, b))))
        .flatten();
    let asks = if awaits_approval(task) {
        Some(tr("Plan waits for you").into_owned())
    } else if awaits_answers(task) {
        Some(tr("Has questions for you").into_owned())
    } else {
        None
    };
    let checker = (task.status == TaskStatus::Checking)
        .then(|| task.reviewer.as_deref().map(|b| tr("Checked by {name}").replace("{name}", &who(app, b))))
        .flatten();
    let parent =
        task.parent.as_deref().and_then(|id| tasks.iter().find(|t| t.id == id)).map(|t| format!("↳ {}", t.title));
    let parts: Vec<String> = [
        waits,
        for_bot,
        task.claimed_by.as_deref().map(|b| who(app, b)),
        asks,
        checker,
        parent,
        task.branch.clone(),
        Some(when(task.updated_at)),
    ]
    .into_iter()
    .flatten()
    .collect();
    parts.join(" · ")
}

fn who(app: &App, bot: &str) -> String {
    let name = name_of(app, bot);
    if name.is_empty() { bot.to_owned() } else { name }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_has_its_own_mark() {
        let marks = [
            TaskStatus::Open,
            TaskStatus::Claimed,
            TaskStatus::Checking,
            TaskStatus::Review,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ]
        .map(mark);
        let unique: std::collections::HashSet<_> = marks.iter().collect();
        assert_eq!(unique.len(), marks.len());
    }
}
