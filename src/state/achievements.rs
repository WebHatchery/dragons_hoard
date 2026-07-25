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
const ACHIEVEMENTS_JSON: &str = include_str!("../../assets/data/achievements.json");

/// What a definition measures. Adding a kind means adding a counter, which is
/// why they are few and blunt rather than an expression language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionKind {
    Spins,
    FreeSpins,
    Hatches,
    Jackpots,
    /// Largest single spin, ever, on any machine.
    BiggestWin,
    /// Credits held at once — a high-water mark, not a total.
    Balance,
    /// Distinct machines played.
    MachinesPlayed,
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
            ConditionKind::Jackpots => self.jackpots,
            ConditionKind::BiggestWin => self.biggest_win,
            ConditionKind::Balance => self.best_balance,
            ConditionKind::MachinesPlayed => self.machines_played.len() as i64,
        }
    }

    /// Fold one settled spin in. `balance` is read after crediting, so the
    /// high-water mark includes the spin that produced it.
    fn observe(&mut self, machine_id: &str, resolution: &SpinResolution, balance: i64) {
        if resolution.was_free_spin {
            self.free_spins += 1;
        } else {
            self.spins += 1;
        }
        if resolution.hatch_credits > 0 {
            self.hatches += 1;
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
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::GameSession;

    fn book() -> AchievementBook {
        let defs: Vec<AchievementDef> = serde_json::from_str(ACHIEVEMENTS_JSON).unwrap();
        validate(&defs).unwrap();
        let unlocked = Achievements::from_definitions(
            defs.iter()
                .map(|def| Achievement::new(&def.id, &def.name, &def.description))
                .collect(),
        );
        AchievementBook {
            defs,
            unlocked,
            progress: AchievementProgress::default(),
        }
    }

    /// A settled spin, built by actually spinning so the shape stays honest.
    fn a_spin(session: &mut GameSession, data: &GameData) -> SpinResolution {
        session.balance = 1_000_000;
        session.spin(data).unwrap()
    }

    #[test]
    fn the_shipped_definitions_are_valid() {
        let defs: Vec<AchievementDef> = serde_json::from_str(ACHIEVEMENTS_JSON).unwrap();
        validate(&defs).unwrap();
        assert!(
            defs.len() >= 8,
            "the set should be worth opening a panel for"
        );
    }

    #[test]
    fn a_threshold_of_zero_is_rejected() {
        // It would unlock before the player did anything.
        let defs = vec![AchievementDef {
            id: "free".to_owned(),
            name: "Free".to_owned(),
            description: "Nothing".to_owned(),
            condition: UnlockCondition {
                kind: ConditionKind::Spins,
                at_least: 0,
            },
        }];
        assert!(validate(&defs).is_err());
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let def = AchievementDef {
            id: "same".to_owned(),
            name: "Same".to_owned(),
            description: "Same".to_owned(),
            condition: UnlockCondition {
                kind: ConditionKind::Spins,
                at_least: 1,
            },
        };
        assert!(validate(&[def.clone(), def]).is_err());
    }

    #[test]
    fn nothing_is_unlocked_before_playing() {
        let book = book();
        let (unlocked, total) = book.tally();

        assert_eq!(unlocked, 0);
        assert_eq!(total, book.defs().len());
    }

    #[test]
    fn the_first_spin_earns_the_first_achievement() {
        let data = GameData::load().unwrap();
        let mut session = GameSession::new(&data, 7);
        let mut book = book();

        let resolution = a_spin(&mut session, &data);
        let earned = book.observe(data.machine_id(), &resolution, session.balance);

        assert!(
            earned.iter().any(|def| def.id == "first_pull"),
            "expected First Pull, got {:?}",
            earned.iter().map(|d| &d.id).collect::<Vec<_>>()
        );
        assert!(book.is_unlocked("first_pull"));
    }

    #[test]
    fn an_achievement_is_only_ever_earned_once() {
        let data = GameData::load().unwrap();
        let mut session = GameSession::new(&data, 8);
        let mut book = book();

        let mut first_pull_awards = 0;
        for _ in 0..20 {
            let resolution = a_spin(&mut session, &data);
            let earned = book.observe(data.machine_id(), &resolution, session.balance);
            first_pull_awards += earned.iter().filter(|def| def.id == "first_pull").count();
        }

        assert_eq!(first_pull_awards, 1, "the toast would repeat every spin");
    }

    #[test]
    fn progress_counts_free_spins_separately_from_paid_ones() {
        let mut progress = AchievementProgress::default();
        let data = GameData::load().unwrap();
        let mut session = GameSession::new(&data, 9);

        let paid = a_spin(&mut session, &data);
        progress.observe("dragon", &paid, 100);
        assert_eq!(progress.spins, 1);
        assert_eq!(progress.free_spins, 0);

        session.free_spins = Some(crate::state::FreeSpinState {
            remaining: 3,
            awarded: 3,
            line_bet: 1,
            total_won: 0,
            burned: 0,
        });
        let free = a_spin(&mut session, &data);
        progress.observe("dragon", &free, 100);
        assert_eq!(progress.spins, 1, "a free spin is not a paid one");
        assert_eq!(progress.free_spins, 1);
    }

    #[test]
    fn the_balance_condition_is_a_high_water_mark() {
        // Reaching 25,000 once should earn Hoarder even if it is lost again;
        // otherwise the player is punished for continuing to play.
        let mut progress = AchievementProgress::default();
        let data = GameData::load().unwrap();
        let mut session = GameSession::new(&data, 10);
        let resolution = a_spin(&mut session, &data);

        progress.observe("dragon", &resolution, 30_000);
        progress.observe("dragon", &resolution, 40);

        assert_eq!(progress.best_balance, 30_000);
        assert_eq!(progress.value(ConditionKind::Balance), 30_000);
    }

    #[test]
    fn progress_is_cumulative_across_machines() {
        // The reason this state exists at all: `SessionStats` is per-machine,
        // so counting from it would reset every time the player switched.
        let mut progress = AchievementProgress::default();
        let data = GameData::load().unwrap();
        let mut session = GameSession::new(&data, 11);

        for _ in 0..3 {
            let resolution = a_spin(&mut session, &data);
            progress.observe("dragon", &resolution, 100);
        }
        for _ in 0..2 {
            let resolution = a_spin(&mut session, &data);
            progress.observe("frost", &resolution, 100);
        }

        assert_eq!(progress.spins, 5);
        assert_eq!(progress.machines_played, vec!["dragon", "frost"]);
    }

    #[test]
    fn playing_every_machine_earns_the_floor_walker() {
        let mut book = book();

        assert!(book.note_machine("dragon").is_empty());
        let earned = book.note_machine("frost");

        assert!(
            earned.iter().any(|def| def.id == "floor_walker"),
            "walking to the second cabinet should earn it"
        );
    }

    #[test]
    fn a_machine_is_only_counted_once_however_often_it_is_played() {
        let mut progress = AchievementProgress::default();
        for _ in 0..5 {
            progress.note_machine("dragon");
        }

        assert_eq!(progress.machines_played.len(), 1);
    }

    #[test]
    fn the_save_shape_round_trips() {
        let mut book = book();
        book.note_machine("dragon");
        book.progress.spins = 512;
        book.progress.biggest_win = 9_001;
        book.unlocked.unlock("first_pull");

        let snapshot = AchievementSave {
            unlocked_ids: book
                .unlocked
                .iter()
                .filter(|entry| entry.unlocked)
                .map(|entry| entry.id.clone())
                .collect(),
            progress: book.progress.clone(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: AchievementSave = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.progress, book.progress);
        assert!(restored.unlocked_ids.contains(&"first_pull".to_owned()));
    }

    #[test]
    fn a_save_predating_an_achievement_still_loads() {
        // Definitions are read from JSON every load and only the unlock flags
        // are restored, so adding an achievement cannot invalidate a save.
        let saved: AchievementSave =
            serde_json::from_str(r#"{"unlocked_ids":["first_pull"]}"#).unwrap();

        assert_eq!(saved.progress, AchievementProgress::default());
        assert_eq!(saved.unlocked_ids, vec!["first_pull".to_owned()]);
    }
}
