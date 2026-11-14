//! The board's work orders (`host/src/board/brief.rs`): a plan waiting for the user's approval, the questions a claimer
//! asks before it plans, and the acceptance criteria a new task starts with. The forms are Fields sheets
//! (`manage/fields.rs` sends them).

use serde_json::{Map, Value, json};

use super::super::app::App;
use super::board::{fields, input};
use super::{Fields, Input, Reply, Submit};
use crate::board::brief::Approval;
use crate::board::{Task, TaskStatus};

/// Whether a task's plan waits for the user's approval.
pub fn awaits_approval(task: &Task) -> bool {
    task.status == TaskStatus::Claimed && task.approval == Approval::Proposed
}

/// Whether a task's questions wait for the user's answers.
pub fn awaits_answers(task: &Task) -> bool {
    task.status == TaskStatus::Claimed && task.approval == Approval::Asking
}

/// Whether the task waits for the user (its plan or its questions): it leads the board.
pub fn needs_user(task: &Task) -> bool {
    awaits_approval(task) || awaits_answers(task)
}

/// Whether a finished task counts as out of the way for the tasks waiting on it: done, cancelled, or finished with no
/// branch left to merge.
pub fn finished_for_others(task: &Task) -> bool {
    match task.status {
        TaskStatus::Done | TaskStatus::Cancelled => true,
        TaskStatus::Review => task.branch.is_none(),
        TaskStatus::Open | TaskStatus::Claimed | TaskStatus::Checking => false,
    }
}

/// The titles of the tasks an open task still waits for (a deleted one no longer counts).
pub fn waiting_titles(tasks: &[Task], task: &Task) -> Vec<String> {
    if task.status != TaskStatus::Open {
        return vec![];
    }
    task.after
        .iter()
        .filter_map(|id| tasks.iter().find(|t| &t.id == id))
        .filter(|t| !finished_for_others(t))
        .map(|t| t.title.clone())
        .collect()
}

/// Acceptance criteria typed on one line, separated by `;`. Blank ones are dropped.
pub fn criteria_from(text: &str) -> Vec<String> {
    text.split(';').map(str::trim).filter(|c| !c.is_empty()).map(str::to_owned).collect()
}

/// A new task's body: the form's values, with its criteria sent as a list.
pub fn new_task_body(mut values: Map<String, Value>) -> Value {
    if let Some(criteria) = values.get("acceptance").and_then(Value::as_str).map(criteria_from) {
        values.insert("acceptance".into(), json!(criteria));
    }
    Value::Object(values)
}

/// Asks for changes to a proposed plan.
pub fn revise_fields(task: String) -> Fields {
    let inputs = vec![input("note", "What to change in the plan", true, true, "")];
    let note = "The bot proposes its plan again with what you say. ⇧↵ new line, ↵ sends.";
    fields("Ask for changes", note, inputs, vec![], Submit::RevisePlan(task))
}

/// The questions a claimer asked, one at a time. `None` when it asked none.
pub fn answer_fields(task: &Task) -> Option<Fields> {
    let questions: Vec<String> = task.questions.iter().map(|q| q.text.clone()).collect();
    let first = questions.first()?;
    let inputs = vec![answer_input(first)];
    let note = "Each answer goes to the next question; the last one sends them all.";
    let submit = Submit::AnswerQuestions { task: task.id.clone(), questions, answers: vec![] };
    Some(fields("Questions for you", note, inputs, vec![], submit))
}

fn answer_input(question: &str) -> Input {
    input("answer", question, true, false, "Your answer")
}

impl App {
    /// Keeps one answer. The next question takes the form; the last answer sends them all.
    pub(in crate::tui::manage) fn answer_next(&mut self, form: &mut Fields, answer: String) {
        let Submit::AnswerQuestions { task, questions, answers } = &mut form.submit else { return };
        answers.push(answer);
        if let Some(next) = questions.get(answers.len()) {
            form.inputs = vec![answer_input(next)];
            form.cursor = 0;
            form.saving = false;
            return;
        }
        let body = json!({"taskId": task, "answers": answers});
        form.saving = true;
        self.sheet("answerQuestions", body, Reply::FieldsSaved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::test_app;

    fn asking(questions: &[&str]) -> Task {
        let questions: Vec<Value> = questions.iter().map(|q| json!({"text": q, "answer": null})).collect();
        serde_json::from_value(json!({"id": "00000000-0000-4000-8000-000000000001", "title": "Poster", "detail": "",
            "project": null, "status": "claimed", "createdBy": null, "claimedBy": "b", "branch": null,
            "worktree": null, "notes": [], "summary": null, "approval": "asking", "questions": questions,
            "createdAt": 0, "updatedAt": 0, "rev": 1}))
        .expect("a task")
    }

    #[test]
    fn criteria_are_split_on_semicolons_and_blank_ones_drop() {
        assert_eq!(criteria_from(" Tests pass ; ;A4 poster;"), ["Tests pass", "A4 poster"]);
        assert_eq!(criteria_from(" ; "), Vec::<String>::new());
    }

    #[test]
    fn a_new_task_sends_its_criteria_as_a_list() {
        let mut values = Map::new();
        values.insert("title".into(), "Poster".into());
        values.insert("acceptance".into(), "A4; two colours".into());
        let body = new_task_body(values);
        assert_eq!(body["acceptance"], json!(["A4", "two colours"]));
        assert_eq!(body["title"], "Poster");
    }

    #[tokio::test]
    async fn each_answer_takes_the_next_question_and_the_last_sends_them() {
        let mut app = test_app();
        let mut form = answer_fields(&asking(&["Size?", "Colour?"])).expect("two questions");
        assert_eq!(form.inputs[0].label, "Size?");
        app.answer_next(&mut form, "A4".into());
        assert_eq!(form.inputs[0].label, "Colour?", "the next question takes the form");
        assert!(!form.saving);
        let Submit::AnswerQuestions { answers, .. } = &form.submit else { panic!("answers") };
        assert_eq!(answers, &["A4"]);
        app.answer_next(&mut form, "Blue".into());
        assert!(form.saving, "the last answer sends them all");
        let Submit::AnswerQuestions { answers, .. } = &form.submit else { panic!("answers") };
        assert_eq!(answers, &["A4", "Blue"]);
    }

    #[test]
    fn no_questions_no_form() {
        assert!(answer_fields(&asking(&[])).is_none());
    }
}
