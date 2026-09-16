//! Telling the player the game exists (§5.28).
//!
//! # Twenty-two systems behind one button
//!
//! There are six cabinets and twenty screens (§5.50), and a new player sees a
//! Spin button. They will never find the gamble, the buy menu, the ledger or the
//! machine picker, because nothing ever mentions them. The footer lists the keys
//! in small grey text, which is where hints go to die.
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
use macroquad_toolkit::persistence::{json_key_exists, load_json_key, save_json_key};
use serde::{Deserialize, Serialize};

const HINTS_KEY: &str = "hints";
pub const HINTS_JSON: &str = macroquad_toolkit::include_json_str!("../../assets/data/hints.json");

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
    /// Times the rules panel has been opened (§5.29).
    RulesOpened,
    /// Rites taken for an open seam (§5.81). The counter is the *decision*
    /// rather than the seam: a seam that opened during free spins was drawn
    /// for the player, and a hint about choosing has not been earned by
    /// watching one happen.
    Rites,
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
    /// The screen this hint is pointing at, by [`Screen::id`] (§5.73).
    ///
    /// Every hint used to open with "Press R" or "Press C", which is no use at
    /// all on a touch device — and §5.45 gave the game touch input. Naming the
    /// screen ties the hint to the registry: it is validated to be a screen the
    /// menu offers, so a hint can only point somewhere a player can actually
    /// get to without a keyboard.
    #[serde(default)]
    pub screen: Option<String>,
}

/// Fill in the figures a hint quotes, from the data rather than the sentence.
///
/// `{cabinets}` was written as the word "five" and stayed "five" when Tidepool
/// made it six (§5.35). A number in prose is a number that goes stale, and this
/// game has a whole section about that (§5.29).
///
/// The substitution spells the count rather than setting a numeral, because
/// these are sentences: "there are six" is prose and "there are 6" is a
/// readout. The word the hint must not contain is exactly the word the
/// renderer is allowed to produce, which is the point — one of them is derived.
pub fn render(text: &str) -> String {
    text.replace("{cabinets}", spell(crate::data::MACHINES.len()))
}

/// Number words, up to more cabinets than this game will ever have. Past that
/// the numeral is honest: a hint quoting "thirteen" of anything has stopped
/// being a sentence anyway.
pub fn spell(count: usize) -> &'static str {
    const WORDS: [&str; 13] = [
        "no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve",
    ];
    WORDS.get(count).copied().unwrap_or("many")
}

/// Counters the hints read. Fed from the places that already track them rather
/// than counted twice.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HintProgress {
    pub gambles: i64,
    #[serde(default)]
    pub rites: i64,
    pub buys: i64,
    pub ledger_opened: i64,
    pub rules_opened: i64,
}

/// Everything the hint system knows, and what it has already said.
#[derive(Debug, Clone, Default)]
pub struct HintBook {
    pub defs: Vec<HintDef>,
    pub seen: Vec<String>,
    pub progress: HintProgress,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct HintSave {
    seen: Vec<String>,
    progress: HintProgress,
}

impl HintBook {
    pub fn load(config: &GameConfig) -> Result<Self, String> {
        let defs: Vec<HintDef> = macroquad_toolkit::data_loader::parse_json_labeled(
            "assets/data/hints.json",
            HINTS_JSON,
        )?;
        validate(&defs)?;

        let saved: HintSave = match load_json_key(&config.game_name, HINTS_KEY) {
            Ok(saved) => saved,
            Err(error) => {
                if json_key_exists(&config.game_name, HINTS_KEY) {
                    eprintln!("Dragon's Hoard hints could not be loaded: {error}");
                }
                HintSave::default()
            }
        };
        Ok(Self {
            defs,
            seen: saved.seen,
            progress: saved.progress,
        })
    }

    pub fn save(&self, config: &GameConfig) -> Result<(), String> {
        if !crate::state::persist::may_write() {
            return Ok(());
        }
        save_json_key(
            &config.game_name,
            HINTS_KEY,
            &HintSave {
                seen: self.seen.clone(),
                progress: self.progress.clone(),
            },
        )
    }

    /// The hint whose rendered text is longest.
    ///
    /// The capture scene photographs this one rather than whichever happens to
    /// be first due, because the bar is a fixed strip that shrinks with the
    /// window (§5.46) and the longest sentence is the only one that can run
    /// under the dismiss button. A probe has to resemble the thing it measures.
    pub fn longest(&self) -> &HintDef {
        self.defs
            .iter()
            .max_by_key(|def| render(&def.text).chars().count())
            .expect("validate rejects an empty hint set")
    }

    pub fn all(&self) -> &[HintDef] {
        &self.defs
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

    pub fn value(
        &self,
        counter: Counter,
        achievements: &AchievementProgress,
        ledger: &Ledger,
    ) -> i64 {
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
            Counter::RulesOpened => self.progress.rules_opened,
            Counter::Rites => self.progress.rites,
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
        // A hint that names a screen must name one a player can open. Pointing
        // at a screen the game only ever deals — an open Vault Pick, a gamble
        // in flight — would be advice nobody can take (§5.73).
        if let Some(id) = def.screen.as_deref() {
            let known = crate::game::screens::Screen::in_menu().any(|screen| screen.id() == id);
            if !known {
                return Err(format!(
                    "hint '{}' points at '{}', which is not a screen the menu offers",
                    def.id, id
                ));
            }
        }
        // A count written as a word is a count that goes stale. "There are
        // five" survived Tidepool arriving and became wrong (§5.73).
        // One line, and the line is 105 characters wide.
        //
        // Not a style rule — a measurement. The bar is a fixed 30px strip on
        // the footer's shortcut line, and a hint that wraps to two lines has
        // its second line clipped by the border. The first version of the buy
        // hint ran to 131 characters and did exactly that, which the capture
        // showed the moment the scene started photographing the *longest* hint
        // rather than whichever was first due (§5.73).
        //
        // The first number here was 124, taken from a hint that fit — on a bar
        // with no Open button on it. The button costs ninety pixels, the box is
        // 576px rather than 668px wide with one there, and the same 122
        // characters then wrapped. So the budget is measured *with* the door,
        // because a hint that names a screen is the case that has least room.
        const ONE_LINE: usize = 105;
        let rendered = render(&def.text);
        if rendered.chars().count() > ONE_LINE {
            return Err(format!(
                "hint '{}' is {} characters and the bar fits {} on one line",
                def.id,
                rendered.chars().count(),
                ONE_LINE
            ));
        }

        for number in ["two", "three", "four", "five", "six", "seven"] {
            if def.text.split_whitespace().any(|word| {
                word.trim_matches(|c: char| !c.is_alphabetic())
                    .eq_ignore_ascii_case(number)
            }) {
                return Err(format!(
                    "hint '{}' spells out '{}'; use a placeholder so the figure                      comes from the data",
                    def.id, number
                ));
            }
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

// Tests live in the crate-level integration harness.
