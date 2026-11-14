//! A task's work order as board lines: its questions and answers, its acceptance criteria with the latest check's mark
//! and note, its plan, and what the work took (turns and minutes).

use crate::board::Task;
use crate::board::brief::{Approval, Check};
use crate::tui::l10n::tr;

/// The work order's lines, under the task's detail: questions, criteria, then the plan.
pub fn work_order(task: &Task) -> Vec<String> {
    let mut out = vec![];
    if !task.questions.is_empty() {
        out.push(tr("Questions for you").into_owned());
        for question in &task.questions {
            out.push(one_line(&question.text));
            if let Some(answer) = &question.answer {
                out.push(format!("  {}", one_line(answer)));
            }
        }
    }
    if !task.acceptance.is_empty() {
        out.push(tr("Acceptance criteria").into_owned());
        for criterion in &task.acceptance {
            let check = task.checks.iter().find(|c| &c.criterion == criterion);
            out.push(format!("{} {criterion}", mark(check)));
            if let Some(note) = check.map(|c| c.note.trim()).filter(|n| !n.is_empty()) {
                out.push(format!("  {}", one_line(note)));
            }
        }
    }
    if let Some(plan) = task.plan.as_deref().filter(|p| !p.trim().is_empty()) {
        let heading = if task.approval == Approval::Approved { "Approved plan" } else { "Proposed plan" };
        out.push(tr(heading).into_owned());
        out.extend(plan.lines().filter(|l| !l.trim().is_empty()).map(str::to_owned));
    }
    out
}

/// The turns a task took, and the minutes from its first claim to its finish (0 until it finishes). `None` before a
/// bot spends a turn on it.
pub fn effort_line(task: &Task) -> Option<String> {
    if task.turns == 0 {
        return None;
    }
    let minutes = match (task.started_at, task.finished_at) {
        // Rounded to the nearest minute, halves up, as the other apps show it.
        (Some(start), Some(end)) => ((end - start).max(0) + 30_000) / 60_000,
        _ => 0,
    };
    Some(
        tr("{turns} turns · {minutes} min")
            .replace("{turns}", &task.turns.to_string())
            .replace("{minutes}", &minutes.to_string()),
    )
}

/// ✓ passed, ✗ failed, ○ not checked yet.
fn mark(check: Option<&Check>) -> &'static str {
    match check {
        None => "○",
        Some(c) if c.pass => "✓",
        Some(_) => "✗",
    }
}

fn one_line(text: &str) -> String {
    text.replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn task(extra: &serde_json::Value) -> Task {
        let mut value = json!({"id": "00000000-0000-4000-8000-000000000002", "title": "Poster", "detail": "",
            "project": null, "status": "claimed", "createdBy": null, "claimedBy": "b", "branch": null,
            "worktree": null, "notes": [], "summary": null, "createdAt": 0, "updatedAt": 0, "rev": 1});
        for (key, v) in extra.as_object().into_iter().flatten() {
            value[key] = v.clone();
        }
        serde_json::from_value(value).expect("a task")
    }

    #[test]
    fn each_criterion_shows_its_latest_mark_and_note() {
        let t = task(&json!({"acceptance": ["A4", "Two colours", "Printable"],
            "checks": [{"criterion": "A4", "pass": true, "note": "210 × 297 mm"},
                       {"criterion": "Two colours", "pass": false, "note": ""}]}));
        let lines = work_order(&t);
        assert_eq!(lines[0], tr("Acceptance criteria").into_owned());
        assert_eq!(lines[1], "✓ A4");
        assert_eq!(lines[2], "  210 × 297 mm");
        assert_eq!(lines[3], "✗ Two colours");
        assert_eq!(lines[4], "○ Printable");
    }

    #[test]
    fn the_plan_and_questions_show_under_their_headings() {
        let t = task(&json!({"approval": "approved", "plan": "Draft\n\nPrint",
            "questions": [{"text": "Size?", "answer": "A4"}, {"text": "Colour?", "answer": null}]}));
        let lines = work_order(&t);
        assert_eq!(lines[0], tr("Questions for you").into_owned());
        let approved = tr("Approved plan").into_owned();
        assert_eq!(lines[1..5], ["Size?".to_owned(), "  A4".to_owned(), "Colour?".to_owned(), approved]);
        assert_eq!(lines[5..], ["Draft".to_owned(), "Print".to_owned()]);
    }

    #[test]
    fn effort_counts_turns_and_rounded_minutes() {
        let none = task(&json!({"turns": 0}));
        assert!(effort_line(&none).is_none());
        let t = task(&json!({"turns": 3, "startedAt": 0, "finishedAt": 12 * 60_000 + 20_000}));
        let line = effort_line(&t).expect("turns");
        assert!(line.contains('3') && line.contains("12"), "{line}");
    }
}
