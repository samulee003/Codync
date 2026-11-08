//! The team board: tasks by status under their headings, the one under the cursor unfolded on ↵ (its detail, notes,
//! summary and where its work is).

use super::{App, Buffer, Rect, centered, footer, frame_box, name_of, put, row, rput, theme, u, window};
use crate::board::{Task, TaskStatus};
use crate::tui::l10n::tr;
use crate::tui::manage::{BoardSheet, task_heading};
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
                    Line::Heading(status, count) => {
                        let text = format!("{} · {count}", tr(task_heading(*status)));
                        put(buf, inner.x, y, inner.width, &text, t.dim.patch(t.panel));
                    }
                    Line::Task(i) => {
                        let task = &tasks[*i];
                        let base = row(buf, inner, y, *i == b.cursor);
                        let label = format!("{} {}", mark(task.status), task.title);
                        put(buf, inner.x, y, u(room) + 2, &truncate(&label, room + 2), t.text.patch(base));
                        rput(buf, inner.right(), y, &meta(app, task), t.dim.patch(base));
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
}

enum Line {
    Heading(TaskStatus, usize),
    Task(usize),
    Detail(String),
}

/// The board's lines: a heading before each status's tasks, and the unfolded task's details under it.
fn lines(app: &App, tasks: &[Task], b: &BoardSheet) -> Vec<Line> {
    let mut out = vec![];
    for (i, task) in tasks.iter().enumerate() {
        if i == 0 || tasks[i - 1].status != task.status {
            out.push(Line::Heading(task.status, tasks.iter().filter(|x| x.status == task.status).count()));
        }
        out.push(Line::Task(i));
        if b.expanded.as_deref() == Some(task.id.as_str()) {
            out.extend(details(app, task).into_iter().map(Line::Detail));
        }
    }
    out
}

/// An unfolded task: its detail, where its work is, its summary and its notes, one line each.
fn details(app: &App, task: &Task) -> Vec<String> {
    let mut out: Vec<String> = task.detail.lines().filter(|l| !l.trim().is_empty()).map(str::to_owned).collect();
    if let Some(at) = task.worktree.as_ref().or(task.project.as_ref()) {
        out.push(format!("▸ {at}"));
    }
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
        TaskStatus::Review => "⇄",
        TaskStatus::Done => "✓",
        TaskStatus::Cancelled => "×",
    }
}

/// Who holds it, its branch, and when it last changed.
fn meta(app: &App, task: &Task) -> String {
    let parts: Vec<String> =
        [task.claimed_by.as_deref().map(|b| who(app, b)), task.branch.clone(), Some(when(task.updated_at))]
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
        let marks =
            [TaskStatus::Open, TaskStatus::Claimed, TaskStatus::Review, TaskStatus::Done, TaskStatus::Cancelled]
                .map(mark);
        let unique: std::collections::HashSet<_> = marks.iter().collect();
        assert_eq!(unique.len(), marks.len());
    }
}
