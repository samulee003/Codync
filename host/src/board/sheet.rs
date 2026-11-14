//! Each bot's character sheet, as a tactics game shows a unit: the class the user gives it, and a level and five
//! abilities that grow with what it does on the board (its deeds). A class sets where the abilities start, and it is
//! promoted at levels 5 and 10 (a new title, every ability up). The deeds are counted as they happen (`record`) and
//! kept in kv (`board.sheet.<bot>`), so deleting a task takes nothing back.

use super::dispatch;
use crate::LockExt;
use crate::hub::Hub;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const KEY_PREFIX: &str = "board.sheet.";
/// An ability's ceiling, and the most its deeds add to where its class starts it.
const MAX_ABILITY: u32 = 100;
const MAX_GROWTH: u32 = 25;
/// Finished tasks after which accuracy is the bot's own record alone.
const ACCURACY_TASKS: u32 = 10;
/// Experience a level costs: level `n` starts at `LEVEL_XP * (n - 1)²`.
const LEVEL_XP: u32 = 20;
/// The levels a class is promoted at, each a rank higher.
const PROMOTIONS: [u32; 2] = [5, 10];
/// What each promotion adds to every ability.
const PROMOTION_BONUS: u32 = 6;

/// What a bot is for on the team (wire values in camelCase).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BotClass {
    /// Plans and splits work.
    Strategist,
    /// Leads the team: coordinates the parts and assembles the result.
    Ruler,
    /// Builds the big pieces: code, systems.
    General,
    /// Does steady, well-defined work: small tasks, documents, material.
    Infantry,
    /// Finds what is wrong: checks finished work.
    MartialArtist,
}

impl BotClass {
    pub fn parse(value: &str) -> Result<Self> {
        serde_json::from_value(json!(value)).map_err(|_| anyhow::anyhow!("unknown class: {value}"))
    }

    /// The class as other bots read it in a prompt.
    pub fn word(self) -> &'static str {
        match self {
            Self::Strategist => "strategist: plans and splits work",
            Self::Ruler => "ruler: leads, coordinates and assembles",
            Self::General => "general: builds code and systems",
            Self::Infantry => "infantry: steady, well-defined tasks and material",
            Self::MartialArtist => "martial artist: checks finished work",
        }
    }

    /// Where its abilities start.
    fn base(self) -> Abilities {
        let [might, intellect, command, accuracy, insight] = match self {
            Self::Strategist => [30, 80, 60, 50, 55],
            Self::Ruler => [50, 65, 85, 50, 45],
            Self::General => [80, 45, 55, 55, 35],
            Self::Infantry => [60, 40, 35, 70, 40],
            Self::MartialArtist => [55, 50, 30, 60, 85],
        };
        Abilities { might, intellect, command, accuracy, insight }
    }
}

/// What a bot has done on the board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Deeds {
    /// Tasks it finished that were accepted.
    pub finished: u32,
    /// Of those, accepted without being sent back.
    pub first_try: u32,
    /// Times a checker sent its work back.
    pub sent_back: u32,
    /// Tasks it checked.
    pub checked: u32,
    /// Of those, sent back with something to fix.
    pub caught: u32,
    /// Subtasks it added to a task it coordinates.
    pub planned: u32,
    /// Notes it left for the team.
    pub notes: u32,
}

/// One thing a bot just did.
#[derive(Clone, Copy)]
pub enum Deed {
    Finished { first_try: bool },
    SentBack,
    Checked { caught: bool },
    Planned,
    Note,
}

/// A bot's five abilities, each 0–100.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Abilities {
    /// Getting work done.
    pub might: u32,
    /// Planning and sharing what it learns.
    pub intellect: u32,
    /// Coordinating others.
    pub command: u32,
    /// Getting it right the first time.
    pub accuracy: u32,
    /// Finding what is wrong in others' work.
    pub insight: u32,
}

/// What kv keeps for a bot.
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Saved {
    class: Option<BotClass>,
    deeds: Deeds,
}

fn load(hub: &Hub, bot: &str) -> Saved {
    hub.store.kv_get(&format!("{KEY_PREFIX}{bot}")).and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default()
}

fn save(hub: &Hub, bot: &str, saved: &Saved) -> Result<()> {
    hub.store.kv_set(&format!("{KEY_PREFIX}{bot}"), &serde_json::to_string(saved)?)
}

/// A bot's class, if the user gave it one.
pub fn class_of(hub: &Hub, bot: &str) -> Option<BotClass> {
    load(hub, bot).class
}

/// The user gives a bot its class, or takes it away.
pub fn set_class(hub: &Hub, bot: &str, class: Option<BotClass>) -> Result<Value> {
    ensure!(dispatch::eligible(hub)?.iter().any(|c| c.id == bot), "unknown bot");
    let _g = hub.board.sheets.locked();
    let mut saved = load(hub, bot);
    saved.class = class;
    save(hub, bot, &saved)?;
    Ok(sheet(bot, &saved))
}

/// Counts a deed on the bot's sheet. A sheet that can't be saved loses the deed, never the board change it follows.
pub fn record(hub: &Hub, bot: &str, deed: Deed) {
    let _g = hub.board.sheets.locked();
    let mut saved = load(hub, bot);
    let deeds = &mut saved.deeds;
    match deed {
        Deed::Finished { first_try } => {
            deeds.finished += 1;
            deeds.first_try += u32::from(first_try);
        }
        Deed::SentBack => deeds.sent_back += 1,
        Deed::Checked { caught } => {
            deeds.checked += 1;
            deeds.caught += u32::from(caught);
        }
        Deed::Planned => deeds.planned += 1,
        Deed::Note => deeds.notes += 1,
    }
    if let Err(error) = save(hub, bot, &saved) {
        tracing::warn!(bot, error = format!("{error:#}"), "deed not saved");
    }
}

/// Every bot's sheet, for the apps.
pub fn sheets(hub: &Hub) -> Result<Vec<Value>> {
    Ok(dispatch::eligible(hub)?.iter().map(|c| sheet(&c.id, &load(hub, &c.id))).collect())
}

fn sheet(bot: &str, saved: &Saved) -> Value {
    let xp = experience(&saved.deeds);
    let level = level(xp);
    json!({
        "botId": bot,
        "class": saved.class,
        "level": level,
        "xp": xp,
        "levelXp": level_start(level),
        "nextLevelXp": level_start(level + 1),
        "rank": rank(level),
        "nextPromotion": PROMOTIONS.into_iter().find(|&at| at > level),
        "abilities": abilities(saved.class, &saved.deeds, rank(level)),
        "deeds": saved.deeds,
    })
}

fn experience(deeds: &Deeds) -> u32 {
    deeds.finished * 10 + deeds.first_try * 5 + deeds.checked * 4 + deeds.caught * 3 + deeds.planned * 6 + deeds.notes
}

/// The level `xp` reaches: 1 at none.
fn level(xp: u32) -> u32 {
    1 + (xp / LEVEL_XP).isqrt()
}

/// The class's rank at `level`: 1, then one more at each promotion.
fn rank(level: u32) -> u32 {
    1 + u32::try_from(PROMOTIONS.iter().filter(|&&at| level >= at).count()).expect("two promotions")
}

/// The experience at which `level` starts.
fn level_start(level: u32) -> u32 {
    LEVEL_XP * (level - 1).pow(2)
}

/// What `count` deeds add to an ability: quick at first, then slower, up to `MAX_GROWTH`.
fn growth(count: u32) -> u32 {
    (5 * count.isqrt()).min(MAX_GROWTH)
}

/// The abilities a class starts with (40 each with none), grown by the deeds. Accuracy moves from the class's start to
/// the bot's own first-try rate as it finishes tasks.
fn abilities(class: Option<BotClass>, deeds: &Deeds, rank: u32) -> Abilities {
    let base =
        class.map_or(Abilities { might: 40, intellect: 40, command: 40, accuracy: 40, insight: 40 }, BotClass::base);
    let bonus = PROMOTION_BONUS * (rank - 1);
    let grown = |start: u32, count: u32| (start + growth(count) + bonus).min(MAX_ABILITY);
    let weight = deeds.finished.min(ACCURACY_TASKS);
    let own = (deeds.first_try * MAX_ABILITY).checked_div(deeds.finished).unwrap_or(0);
    Abilities {
        might: grown(base.might, deeds.finished),
        intellect: grown(base.intellect, deeds.planned + deeds.notes / 2),
        command: grown(base.command, deeds.planned),
        accuracy: ((base.accuracy * (ACCURACY_TASKS - weight) + own * weight) / ACCURACY_TASKS + bonus)
            .min(MAX_ABILITY),
        insight: grown(base.insight, deeds.checked + deeds.caught),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_cost_more_each_time() {
        assert_eq!([0, 19, 20, 79, 80, 180].map(level), [1, 1, 2, 2, 3, 4]);
        assert_eq!((level_start(1), level_start(2), level_start(3)), (0, 20, 80));
    }

    #[test]
    fn deeds_grow_the_abilities_a_class_starts() {
        let fresh = abilities(Some(BotClass::General), &Deeds::default(), 1);
        assert_eq!(fresh, BotClass::General.base());
        let deeds = Deeds { finished: 4, first_try: 3, checked: 1, caught: 1, planned: 9, notes: 0, sent_back: 1 };
        let grown = abilities(Some(BotClass::General), &deeds, 1);
        assert_eq!((grown.might, grown.command, grown.insight), (80 + 10, 55 + 15, 35 + 5));
        assert_eq!(grown.accuracy, (55 * 6 + 75 * 4) / 10, "four tasks in: partly its own 75%");
        let veteran = Deeds { finished: 400, first_try: 400, ..Deeds::default() };
        let top = abilities(Some(BotClass::General), &veteran, 3);
        assert_eq!((top.might, top.accuracy), (100, 100), "growth stops at the ceiling");
        assert_eq!(abilities(None, &Deeds::default(), 1).might, 40);
    }

    #[test]
    fn a_class_is_promoted_at_levels_five_and_ten() {
        assert_eq!([1, 4, 5, 9, 10, 30].map(rank), [1, 1, 2, 2, 3, 3]);
        let promoted = abilities(Some(BotClass::Infantry), &Deeds::default(), 3);
        assert_eq!((promoted.might, promoted.accuracy), (60 + 12, 70 + 12), "two promotions: +6 each");
        let sheet =
            sheet("b", &Saved { class: Some(BotClass::Ruler), deeds: Deeds { finished: 13, ..Deeds::default() } });
        assert_eq!((&sheet["level"], &sheet["rank"], &sheet["nextPromotion"]), (&json!(3), &json!(1), &json!(5)));
    }

    #[test]
    fn experience_counts_every_deed() {
        let deeds = Deeds { finished: 2, first_try: 1, sent_back: 1, checked: 3, caught: 1, planned: 2, notes: 4 };
        assert_eq!(experience(&deeds), 20 + 5 + 12 + 3 + 12 + 4);
        assert!(BotClass::parse("martialArtist").is_ok() && BotClass::parse("wizard").is_err());
    }
}
