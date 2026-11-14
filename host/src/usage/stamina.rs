//! Each bot's "stamina": how much of its agent's plan is left, for the bar beside the bot in every roster. It is the
//! tightest limit the agent reports (the window with the most used), so a bot with a full week but an empty session
//! shows empty. Keyed by every name a bot's `backend` may use for the agent (its harness id and its ACP registry id),
//! so the apps look a bot up by its `backend` alone.

use super::{Provider, Window};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stamina {
    /// Share of the tightest limit still left, 0–100.
    pub left: u8,
    /// That limit's name ("Weekly", "Session", "Claude 7d").
    pub label: String,
    /// When it resets: epoch milliseconds, else the agent's own words.
    pub resets_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_text: Option<String>,
}

/// The tightest limit of each agent with a reading, under each name its bots may use.
pub fn of(providers: &[Provider]) -> BTreeMap<String, Stamina> {
    let mut out = BTreeMap::new();
    for provider in providers {
        let Some(stamina) = tightest(&provider.windows) else { continue };
        for name in std::iter::once(provider.id).chain(provider.registry) {
            out.insert(name.to_owned(), stamina.clone());
        }
    }
    out
}

/// A window that is a share of a cap, the most used; a note ("$4.20") never counts.
fn tightest(windows: &[Window]) -> Option<Stamina> {
    let window = windows.iter().filter(|w| w.text.is_none()).max_by(|a, b| a.percent.total_cmp(&b.percent))?;
    // A percent is 0–100 by the readers' contract; clamping keeps a misread from wrapping.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0–100 first")]
    let left = (100.0 - window.percent).clamp(0.0, 100.0).round() as u8;
    Some(Stamina {
        left,
        label: window.label.clone(),
        resets_at: window.resets_at,
        resets_text: window.resets_text.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(id: &'static str, registry: Option<&'static str>, windows: Vec<Window>) -> Provider {
        Provider { id, name: id, registry, windows, source: "test", updated_at: 0 }
    }

    #[test]
    fn the_tightest_limit_is_the_stamina_under_every_name() {
        let mut note = Window::new("cost", "Cost", 0.0, None);
        note.text = Some("$4.20".into());
        let providers = vec![
            provider(
                "codex",
                Some("codex-acp"),
                vec![
                    Window::new("session", "Session", 50.0, Some(1)),
                    Window::new("weekly", "Weekly", 67.4, Some(2)),
                    note,
                ],
            ),
            provider("amp", None, vec![]),
        ];
        let stamina = of(&providers);
        assert_eq!(stamina.len(), 2, "codex under both names; amp has no limit");
        let codex = &stamina["codex-acp"];
        assert_eq!((codex.left, codex.label.as_str(), codex.resets_at), (33, "Weekly", Some(2)));
        assert_eq!(stamina["codex"], *codex);
        let over = of(&[provider("x", None, vec![Window::new("w", "W", 130.0, None)])]);
        assert_eq!(over["x"].left, 0, "a misread over 100% shows empty, not wrapped");
    }
}
