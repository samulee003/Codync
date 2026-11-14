//! Lessons from checks: when a checker sends a bot's work back, the send-back is kept as a lesson for that bot, so it
//! doesn't repeat the mistake on later tasks. A bot's last lessons are quoted in the prompts that hand it a task (its
//! assignment, its planning, the claim's answer). Kept in kv (`board.lessons.<bot>`), newest first, at most
//! `MAX_LESSONS`; the bot's board changes are serialized by `Board::lessons`.

use crate::LockExt;
use crate::hub::Hub;
use crate::store::{Store, now_ms};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

const KEY_PREFIX: &str = "board.lessons.";
/// Lessons a bot keeps; the oldest go first.
pub const MAX_LESSONS: usize = 20;
/// Lessons a task prompt quotes, the most recent first.
pub const QUOTED: usize = 5;
/// Bytes one lesson holds.
const MAX_LESSON: usize = 400;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    pub text: String,
    /// The task it was learned on (its title).
    pub task: String,
    pub at: i64,
}

fn key(bot: &str) -> String {
    format!("{KEY_PREFIX}{bot}")
}

fn load(store: &Store, bot: &str) -> Vec<Lesson> {
    let saved: Vec<Lesson> = store.kv_get(&key(bot)).and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
    saved.into_iter().take(MAX_LESSONS).collect()
}

/// Keeps what a checker sent `bot`'s work back for, as a lesson on `task` (its title). Empty feedback keeps nothing.
/// A lesson that can't be saved is logged; the send-back itself stands.
pub fn record(hub: &Hub, bot: &str, task: &str, feedback: &str) {
    let text = clip(&feedback.split_whitespace().collect::<Vec<_>>().join(" "));
    if text.is_empty() {
        return;
    }
    let _g = hub.board.lessons.locked();
    let mut lessons = load(&hub.store, bot);
    lessons.insert(0, Lesson { text, task: task.to_owned(), at: now_ms() });
    lessons.truncate(MAX_LESSONS);
    if let Err(error) = save(&hub.store, bot, &lessons) {
        tracing::warn!(bot, error = format!("{error:#}"), "lesson not saved");
    }
}

fn save(store: &Store, bot: &str, lessons: &[Lesson]) -> Result<()> {
    store.kv_set(&key(bot), &serde_json::to_string(lessons)?)
}

/// The bot's `n` most recent lessons, newest first.
pub fn recent(hub: &Hub, bot: &str, n: usize) -> Vec<Lesson> {
    let mut lessons = load(&hub.store, bot);
    lessons.truncate(n);
    lessons
}

/// Forgets all of the bot's lessons.
pub fn forget(hub: &Hub, bot: &str) -> Result<()> {
    let _g = hub.board.lessons.locked();
    hub.store.kv_delete(&key(bot))
}

/// The block a task prompt ends with, quoting the bot's most recent lessons; empty when it has none.
pub fn prompt(hub: &Hub, bot: &str) -> String {
    block(&recent(hub, bot, QUOTED))
}

fn block(lessons: &[Lesson]) -> String {
    if lessons.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\nLessons from earlier checks of your work (don't repeat these):");
    for lesson in lessons {
        let _ = write!(out, "\n- {}", lesson.text);
    }
    out
}

/// The text cut to at most `MAX_LESSON` bytes, on a character boundary.
fn clip(text: &str) -> String {
    let end =
        text.char_indices().map(|(at, c)| at + c.len_utf8()).take_while(|&end| end <= MAX_LESSON).last().unwrap_or(0);
    text[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lesson_is_at_most_400_bytes_on_a_character_boundary() {
        assert_eq!(clip("short"), "short");
        let long = clip(&"準".repeat(200));
        assert!(long.len() <= MAX_LESSON && long.len() > MAX_LESSON - 3);
        assert_eq!(clip(&"x".repeat(MAX_LESSON + 9)).len(), MAX_LESSON);
    }

    #[test]
    fn the_block_quotes_nothing_without_lessons() {
        assert_eq!(block(&[]), "");
        let one = Lesson { text: "Add the tests".into(), task: "T".into(), at: 0 };
        assert_eq!(
            block(&[one]),
            "\n\nLessons from earlier checks of your work (don't repeat these):\n- Add the tests"
        );
    }
}
