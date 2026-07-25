//! Telling the player the game exists (§5.28).
//!
//! # Twenty-two systems behind one button
//!
//! There are five cabinets, twelve overlays and twelve keyboard shortcuts, and a
//! new player sees a Spin button. They will never find the gamble, the buy menu,
//! the ledger or the machine picker, because nothing ever mentions them. The
//! footer lists the keys in small grey text, which is where hints go to die.
//!
//! # Not a tutorial
//!
//! A scripted tour is the obvious answer and the wrong one. It arrives before
//! the player wants any of it, it is skipped, and it never comes back. Worse, it
//! has to be maintained against a game that has grown a new system every
//! iteration.
//!
//! This is data instead. A hint has a **condition** — the same counter-and-
//! threshold shape the achievements use (§5.9) — and an **earned** counter that
//! retires it. "You have won twenty times and never gambled" is a fact the game
//! already knows; the hint is just that fact, said out loud, once.
//!
//! So a hint arrives when the player is ready for it rather than when the game
//! loaded, and disappears the moment they act. Nothing has to be skipped, and a
//! player who works something out on their own is never told about it at all.
//!
//! # One at a time, and never again
//!
//! Two hints at once is a tutorial by another name. Which one shows is decided
//! by the order in `hints.json`, so a designer controls the teaching order by
//! moving a line. Dismissals persist under their own key alongside preferences
//! and achievements — a hint the player has read is done with, whatever happens
//! to their bankroll.

use crate::data::GameConfig;
use crate::state::achievements::AchievementProgress;
use crate::state::ledger::Ledger;
use macroquad_toolkit::persistence::{load_json_key, save_json_key};
use serde::{Deserialize, Serialize};

const HINTS_KEY: &str = "hints";
const HINTS_JSON: &str = include_str!("../../assets/data/hints.json");

/// A counter a hint can watch. Deliberately a small, closed set: a hint that
/// needed a new counter is usually a hint about something the game should have
/// made obvious anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Counter {
    Spins,
    /// Rounds that paid anything, across every cabinet.
    Wins,
    FreeSpins,
    Hatches,
    /// Times the player has staked a win on the scale (§5.16).
    Gambles,
    /// Features bought outright (§5.13).
    Buys,
    /// Distinct cabinets played (§5.8).
    MachinesPlayed,
    /// Times the ledger has been opened (§5.18).
    LedgerOpened,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HintDef {
    pub id: String,
    /// What the hint says. One sentence; a paragraph is a manual.
    pub text: String,
    /// Shows once this counter reaches `after`.
    pub when: Counter,
    pub after: i64,
    /// Retires once this counter reaches `until` — normally the thing the hint
    /// is telling the player to do.
    pub earns: Counter,
    pub until: i64,
}

/// Counters the hints read. Fed from the places that already track them rather
/// than counted twice.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HintProgress {
    pub gambles: i64,
    pub buys: i64,
    pub ledger_opened: i64,
}

/// Everything the hint system knows, and what it has already said.
#[derive(Debug, Clone, Default)]
pub struct HintBook {
    defs: Vec<HintDef>,
    seen: Vec<String>,
    progress: HintProgress,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct HintSave {
    seen: Vec<String>,
    progress: HintProgress,
}

impl HintBook {
    pub fn load(config: &GameConfig) -> Result<Self, String> {
        let defs: Vec<HintDef> =
            serde_json::from_str(HINTS_JSON).map_err(|err| format!("hints.json: {}", err))?;
        validate(&defs)?;

        let saved: HintSave = load_json_key(&config.game_name, HINTS_KEY).unwrap_or_default();
        Ok(Self {
            defs,
            seen: saved.seen,
            progress: saved.progress,
        })
    }

    pub fn save(&self, config: &GameConfig) -> Result<(), String> {
        save_json_key(
            &config.game_name,
            HINTS_KEY,
            &HintSave {
                seen: self.seen.clone(),
                progress: self.progress.clone(),
            },
        )
    }

    pub fn progress_mut(&mut self) -> &mut HintProgress {
        &mut self.progress
    }

    /// Mark a hint as read. It never returns.
    pub fn dismiss(&mut self, id: &str) {
        if !self.seen.iter().any(|seen| seen == id) {
            self.seen.push(id.to_owned());
        }
    }

    /// The hint to show, if any.
    ///
    /// First in file order that has come due and not yet been earned or
    /// dismissed — so the teaching order is a data decision, and moving a line
    /// in `hints.json` changes what a new player is told first.
    pub fn current(&self, achievements: &AchievementProgress, ledger: &Ledger) -> Option<&HintDef> {
        self.defs.iter().find(|def| {
            !self.seen.contains(&def.id)
                && self.value(def.when, achievements, ledger) >= def.after
                && self.value(def.earns, achievements, ledger) < def.until
        })
    }

    fn value(&self, counter: Counter, achievements: &AchievementProgress, ledger: &Ledger) -> i64 {
        match counter {
            Counter::Spins => achievements.spins,
            Counter::FreeSpins => achievements.free_spins,
            Counter::Hatches => achievements.hatches,
            Counter::MachinesPlayed => achievements.machines_played.len() as i64,
            // Wins are the ledger's business: it is the only thing that counts a
            // round rather than a spin (§5.18).
            Counter::Wins => ledger.total_hits(),
            Counter::Gambles => self.progress.gambles,
            Counter::Buys => self.progress.buys,
            Counter::LedgerOpened => self.progress.ledger_opened,
        }
    }
}

/// Reject a hint set that could never appear or never leave.
pub fn validate(defs: &[HintDef]) -> Result<(), String> {
    if defs.is_empty() {
        return Err("hints.json declared no hints".to_owned());
    }
    let mut seen: Vec<&str> = Vec::new();
    for def in defs {
        if seen.contains(&def.id.as_str()) {
            return Err(format!("duplicate hint '{}'", def.id));
        }
        seen.push(&def.id);

        if def.text.trim().is_empty() {
            return Err(format!("hint '{}' says nothing", def.id));
        }
        if def.until <= 0 {
            // A hint earned at zero is already earned and would never show.
            return Err(format!("hint '{}' can never appear", def.id));
        }
        if def.when == def.earns && def.after >= def.until {
            // Showing at 20 spins and retiring at 10 is the same fault, spelled
            // differently: the condition is met only once it is already over.
            return Err(format!("hint '{}' retires before it appears", def.id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    fn book() -> HintBook {
        HintBook::load(&GameData::load().unwrap().config).unwrap()
    }

    /// A player who has done plenty of everything the hints watch, so whichever
    /// one is first in file order is the one that comes due.
    fn busy_player(spins: i64, hatches: i64) -> AchievementProgress {
        AchievementProgress {
            spins,
            free_spins: spins,
            hatches,
            machines_played: vec!["dragon".to_owned()],
            ..AchievementProgress::default()
        }
    }

    fn defs() -> Vec<HintDef> {
        serde_json::from_str(HINTS_JSON).unwrap()
    }

    #[test]
    fn the_shipped_hints_validate() {
        assert!(validate(&defs()).is_ok());
        assert!(!defs().is_empty());
    }

    #[test]
    fn a_hint_that_could_never_appear_is_rejected() {
        let mut defs = defs();
        defs[0].until = 0;
        assert!(validate(&defs).is_err());
    }

    #[test]
    fn a_hint_that_retires_before_it_appears_is_rejected() {
        let mut defs = defs();
        defs[0].when = Counter::Spins;
        defs[0].earns = Counter::Spins;
        defs[0].after = 50;
        defs[0].until = 10;
        assert!(validate(&defs).is_err());
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let mut defs = defs();
        let first = defs[0].clone();
        defs.push(first);
        assert!(validate(&defs).is_err());
    }

    #[test]
    fn a_fresh_player_is_told_nothing() {
        // Every hint waits for the player to have done something. Firing on the
        // first frame would be the tutorial this deliberately is not.
        let book = book();
        let progress = AchievementProgress::default();
        let ledger = Ledger::default();
        assert!(book.current(&progress, &ledger).is_none());
    }

    #[test]
    fn a_hint_appears_once_its_condition_is_met() {
        let mut book = book();
        // Enough of everything to bring the first hint due.
        let progress = busy_player(10_000, 100);

        let mut ledger = Ledger::default();
        for _ in 0..500 {
            ledger.record("dragon", 200, 400, false);
        }

        let hint = book
            .current(&progress, &ledger)
            .expect("nothing came due")
            .id
            .clone();

        // And it goes away for good once read.
        book.dismiss(&hint);
        assert!(book
            .current(&progress, &ledger)
            .is_none_or(|next| next.id != hint));
    }

    #[test]
    fn acting_on_a_hint_retires_it_without_dismissing() {
        // The point of `earns`: a player who works it out on their own is never
        // told, and one who follows the hint is not thanked twice.
        let mut book = book();
        let progress = busy_player(10_000, 100);
        let mut ledger = Ledger::default();
        for _ in 0..500 {
            ledger.record("dragon", 200, 400, false);
        }

        let Some(hint) = book.current(&progress, &ledger).cloned() else {
            panic!("nothing came due");
        };

        // Satisfy whatever it was asking for.
        let counters = book.progress_mut();
        match hint.earns {
            Counter::Gambles => counters.gambles = hint.until,
            Counter::Buys => counters.buys = hint.until,
            Counter::LedgerOpened => counters.ledger_opened = hint.until,
            _ => return, // Satisfied from elsewhere; the dismiss test covers it.
        }

        assert!(book
            .current(&progress, &ledger)
            .is_none_or(|next| next.id != hint.id));
    }

    #[test]
    fn only_one_hint_shows_at_a_time() {
        // `current` returns an Option by construction, but the *order* matters:
        // it must be the first in file order, so a designer controls the
        // teaching sequence by moving a line.
        let book = book();
        let progress = busy_player(100_000, 1_000);
        let mut ledger = Ledger::default();
        for _ in 0..5_000 {
            ledger.record("dragon", 200, 400, false);
        }

        let shown = book.current(&progress, &ledger).unwrap();
        let expected = book
            .defs
            .iter()
            .find(|def| {
                book.value(def.when, &progress, &ledger) >= def.after
                    && book.value(def.earns, &progress, &ledger) < def.until
            })
            .unwrap();
        assert_eq!(shown.id, expected.id);
    }

    #[test]
    fn dismissals_round_trip() {
        let config = GameData::load().unwrap().config;
        let mut book = book();
        book.dismiss("a-hint-that-does-not-exist");
        book.progress_mut().gambles = 3;
        book.save(&config).unwrap();

        let restored = HintBook::load(&config).unwrap();
        assert!(restored
            .seen
            .iter()
            .any(|id| id == "a-hint-that-does-not-exist"));
        assert_eq!(restored.progress.gambles, 3);
    }
}
