//! Achievements: long-horizon goals that outlive any one session or machine.
//!
//! The registry itself is the toolkit's [`Achievements`] — definitions, unlock
//! state, `sync_definitions` for adding new ones to an old save. What lives here
//! is the part the toolkit cannot know: **what counts as earning one.**
//!
//! # Progress is cumulative and cross-machine
//!
//! `SessionStats` is per-machine, because it is part of a machine's save slot
//! (§5.8). Achievements are a property of the *player*, so counting from session
//! stats would mean "1,000 spins" silently reset every time someone walked to
//! another cabinet. [`AchievementProgress`] therefore keeps its own running
//! totals, updated from every settled spin whichever machine raised it, and
//! persists under its own key alongside preferences (§5.7).

use crate::data::GameConfig;
use crate::state::SpinResolution;
use macroquad_toolkit::achievements::{Achievement, Achievements};
use macroquad_toolkit::persistence::{load_json_key, save_json_key};
use serde::{Deserialize, Serialize};

const ACHIEVEMENTS_KEY: &str = "achievements";
const ACHIEVEMENTS_JSON: &str =
    macroquad_toolkit::include_json_str!("../../assets/data/achievements.json");

/// What a definition measures. Adding a kind means adding a counter, which is
/// why they are few and blunt rather than an expression language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionKind {
    Spins,
    FreeSpins,
    Hatches,
    /// Dragon's Wrath rounds played out (§5.12).
    Wraths,
    /// Seams worked (§5.80).
    Seams,
    Jackpots,
    /// Largest single spin, ever, on any machine.
    BiggestWin,
    /// Credits held at once — a high-water mark, not a total.
    Balance,
    /// Distinct machines played.
    MachinesPlayed,
}

/// A feature that opens a round of its own and pays when the round ends.
///
/// Closed, so a fourth one cannot be added without someone deciding here
/// whether the awards book should know about it — which is the decision that
/// was never made for the first three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureRound {
    /// The hoard filled and its Vault Pick was played out (§5.10).
    Hatch,
    /// A Dragon's Wrath respin round ended (§5.12).
    Wrath,
    /// A seam finished working the board (§5.80).
    Seam,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockCondition {
    pub kind: ConditionKind,
    pub at_least: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub condition: UnlockCondition,
}

/// Running totals across every machine and every session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AchievementProgress {
    pub spins: i64,
    pub free_spins: i64,
    pub hatches: i64,
    /// `default` on both so a save written before §5.84 still loads (§5.49).
    #[serde(default)]
    pub wraths: i64,
    #[serde(default)]
    pub seams: i64,
    pub jackpots: i64,
    pub biggest_win: i64,
    pub best_balance: i64,
    /// Machine ids seen, in first-played order.
    pub machines_played: Vec<String>,
}

impl AchievementProgress {
    fn value(&self, kind: ConditionKind) -> i64 {
        match kind {
            ConditionKind::Spins => self.spins,
            ConditionKind::FreeSpins => self.free_spins,
            ConditionKind::Hatches => self.hatches,
            ConditionKind::Wraths => self.wraths,
            ConditionKind::Seams => self.seams,
            ConditionKind::Jackpots => self.jackpots,
            ConditionKind::BiggestWin => self.biggest_win,
            ConditionKind::Balance => self.best_balance,
            ConditionKind::MachinesPlayed => self.machines_played.len() as i64,
        }
    }

    /// Fold one settled spin in. `balance` is read after crediting, so the
    /// high-water mark includes the spin that produced it.
    ///
    /// **A feature round is not counted here** (§5.84). Every round in this
    /// game — the Vault Pick, the Dragon's Wrath, the Seam — credits itself
    /// *after* the spin has settled and the resolution has gone out, so its
    /// figure is zero at the moment this reads it. Counting hatches from
    /// `hatch_credits` is what it used to do, and the two achievements resting
    /// on that counter could not be unlocked by playing the game.
    fn observe(&mut self, machine_id: &str, resolution: &SpinResolution, balance: i64) {
        if resolution.was_free_spin {
            self.free_spins += 1;
        } else {
            self.spins += 1;
        }
        if resolution.jackpot.is_some() {
            self.jackpots += 1;
        }
        self.biggest_win = self.biggest_win.max(resolution.total_credits());
        self.best_balance = self.best_balance.max(balance);
        self.note_machine(machine_id);
    }

    fn note_machine(&mut self, machine_id: &str) {
        if !self.machines_played.iter().any(|id| id == machine_id) {
            self.machines_played.push(machine_id.to_owned());
        }
    }
}

/// The definitions, the unlock state, and the counters behind them.
#[derive(Debug, Clone)]
pub struct AchievementBook {
    defs: Vec<AchievementDef>,
    unlocked: Achievements,
    progress: AchievementProgress,
}

/// What gets written to disk. Kept separate from the book so the definitions —
/// which live in JSON and may change — are never persisted alongside state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct AchievementSave {
    unlocked_ids: Vec<String>,
    progress: AchievementProgress,
}

impl AchievementBook {
    pub fn load(config: &GameConfig) -> Result<Self, String> {
        let defs: Vec<AchievementDef> = serde_json::from_str(ACHIEVEMENTS_JSON)
            .map_err(|err| format!("achievements.json: {}", err))?;
        validate(&defs)?;

        let saved: AchievementSave =
            load_json_key(&config.game_name, ACHIEVEMENTS_KEY).unwrap_or_default();

        // Definitions come from JSON every time; only the unlock flags and the
        // counters are restored. Renaming an achievement therefore takes effect
        // immediately, and adding one cannot invalidate a save.
        let mut unlocked = Achievements::from_definitions(
            defs.iter()
                .map(|def| Achievement::new(&def.id, &def.name, &def.description))
                .collect(),
        );
        for id in &saved.unlocked_ids {
            unlocked.unlock(id);
        }

        Ok(Self {
            defs,
            unlocked,
            progress: saved.progress,
        })
    }

    pub fn save(&self, config: &GameConfig) -> Result<(), String> {
        if !crate::state::persist::may_write() {
            return Ok(());
        }
        let snapshot = AchievementSave {
            unlocked_ids: self
                .unlocked
                .iter()
                .filter(|entry| entry.unlocked)
                .map(|entry| entry.id.clone())
                .collect(),
            progress: self.progress.clone(),
        };
        save_json_key(&config.game_name, ACHIEVEMENTS_KEY, &snapshot)
    }

    /// The counters, writable. Only the capture harness needs this: a scene
    /// that wants a particular hint on screen has to put the player where that
    /// hint applies, and sixty scripted spins will not reach every threshold
    /// (§5.73). Deliberately not gated to native — the scene wiring compiles
    /// on wasm even though nothing there ever calls it, and a `cfg` here just
    /// moves the breakage to the one build nobody runs locally.
    pub fn progress_mut(&mut self) -> &mut AchievementProgress {
        &mut self.progress
    }

    pub fn progress(&self) -> &AchievementProgress {
        &self.progress
    }

    pub fn defs(&self) -> &[AchievementDef] {
        &self.defs
    }

    pub fn is_unlocked(&self, id: &str) -> bool {
        self.unlocked.is_unlocked(id)
    }

    /// Unlocked count and total, for the overlay header.
    pub fn tally(&self) -> (usize, usize) {
        self.unlocked.progress()
    }

    /// Record a settled spin and return anything it just earned.
    ///
    /// Returns definitions rather than ids so the caller can announce them
    /// without a second lookup.
    pub fn observe(
        &mut self,
        machine_id: &str,
        resolution: &SpinResolution,
        balance: i64,
    ) -> Vec<AchievementDef> {
        self.progress.observe(machine_id, resolution, balance);
        self.check()
    }

    /// Note a feature round that has just paid out (§5.84).
    ///
    /// Separate from [`observe`](Self::observe) because a round finishes on its
    /// own clock — a chest turned over, a respin allowance run down, a rite
    /// worked out — and there is no spin resolution left to hang it on by then.
    pub fn note_round(&mut self, round: FeatureRound) -> Vec<AchievementDef> {
        match round {
            FeatureRound::Hatch => self.progress.hatches += 1,
            FeatureRound::Wrath => self.progress.wraths += 1,
            FeatureRound::Seam => self.progress.seams += 1,
        }
        self.check()
    }

    /// Note a machine as played without a spin — walking to a cabinet counts.
    pub fn note_machine(&mut self, machine_id: &str) -> Vec<AchievementDef> {
        self.progress.note_machine(machine_id);
        self.check()
    }

    fn check(&mut self) -> Vec<AchievementDef> {
        let mut earned = Vec::new();
        for def in &self.defs {
            if self.unlocked.is_unlocked(&def.id) {
                continue;
            }
            if self.progress.value(def.condition.kind) >= def.condition.at_least {
                // `unlock` returns false for an unknown id; ignore either way,
                // the definition list is the source of truth here.
                self.unlocked.unlock(&def.id);
                earned.push(def.clone());
            }
        }
        earned
    }
}

fn validate(defs: &[AchievementDef]) -> Result<(), String> {
    if defs.is_empty() {
        return Err("achievements.json declared no achievements".to_owned());
    }
    for def in defs {
        if def.condition.at_least <= 0 {
            return Err(format!(
                "achievement '{}' has a threshold of {}, which unlocks instantly",
                def.id, def.condition.at_least
            ));
        }
    }

    let mut ids: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    if ids.len() != count {
        return Err("achievements.json has duplicate ids".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
