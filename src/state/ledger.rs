//! The Ledger: what this player has actually seen (§5.18).
//!
//! # The other half of §5.17
//!
//! The profiler measures what a cabinet *does*, over twenty thousand rounds it
//! runs itself. The ledger measures what the player has *seen*, over however
//! many rounds they have actually played — and it does it with the same
//! `RoundStats` the profiler uses, so the two are directly comparable.
//!
//! That comparison is the whole point, and it is meant to be uncomfortable. A
//! few hundred spins will not look anything like the machine's own figures, and
//! a player who has been unlucky will have the numbers to prove it while the
//! machine sits there being exactly what it always was. §5.17 learned that a
//! twenty-thousand-round sample cannot measure RTP; a three-hundred-round one
//! cannot measure much of anything. Showing both, side by side, says that better
//! than any amount of prose.
//!
//! # A round is a paid spin and everything it led to
//!
//! Free spins cost nothing, so they are part of the return on the spin that
//! bought them rather than rounds of their own. Same for a Vault Pick or a
//! Dragon's Wrath the round opened. This is exactly the definition the profiler
//! uses, and it has to be, or the two columns would be measuring different
//! things and the comparison would be a lie.
//!
//! **The gamble (§5.16) is deliberately left out.** It is the player's decision,
//! not the machine's behaviour, and the profile has no gamble in it. Folding it
//! in would make one column answer a different question from the other.
//!
//! # Per machine, and it outlives a New Game
//!
//! Each cabinet keeps its own ledger, because each is a different game (§5.8)
//! and averaging them would describe none of them. It persists under its own key
//! alongside preferences and achievements rather than inside a save slot, so it
//! is a record of what the player has seen rather than of one bankroll.

use crate::data::GameConfig;
use crate::engine::sim::{RoundStats, BAND_COUNT};
use macroquad_toolkit::persistence::{load_json_key, save_json_key};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const LEDGER_KEY: &str = "ledger";

/// One cabinet's record.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineLedger {
    pub stats: RoundStats,
    pub wagered: i64,
    pub won: i64,
    /// Rounds that returned anything at all.
    pub hits: u64,
    /// Largest single round, in multiples of total bet.
    pub best_round: f64,
    pub features: u64,
}

impl MachineLedger {
    pub fn rounds(&self) -> u64 {
        self.stats.rounds
    }

    /// The player's own measured return. Honest but nearly meaningless at small
    /// sample sizes, which is the lesson the panel is there to teach.
    pub fn rtp(&self) -> f64 {
        if self.wagered == 0 {
            return 0.0;
        }
        self.won as f64 / self.wagered as f64
    }

    pub fn hit_frequency(&self) -> f64 {
        if self.stats.rounds == 0 {
            return 0.0;
        }
        self.hits as f64 / self.stats.rounds as f64
    }

    pub fn bands(&self) -> [f64; BAND_COUNT] {
        self.stats.band_shares()
    }

    /// Rough guide to how far a sample of this size can be trusted.
    ///
    /// The standard error of the mean return falls as `1/sqrt(n)`, so this is
    /// the machine's own volatility scaled by the sample — a number that says
    /// "your return could plausibly be this far out" rather than pretending the
    /// measurement is the truth.
    pub fn margin(&self, volatility: f64) -> f64 {
        if self.stats.rounds < 2 {
            return f64::INFINITY;
        }
        volatility / (self.stats.rounds as f64).sqrt()
    }
}

/// Every cabinet's record, keyed by machine id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    machines: HashMap<String, MachineLedger>,
}

impl Ledger {
    pub fn load(config: &GameConfig) -> Self {
        load_json_key(&config.game_name, LEDGER_KEY).unwrap_or_default()
    }

    pub fn save(&self, config: &GameConfig) -> Result<(), String> {
        if !crate::state::persist::may_write() {
            return Ok(());
        }
        save_json_key(&config.game_name, LEDGER_KEY, self)
    }

    pub fn get(&self, machine_id: &str) -> Option<&MachineLedger> {
        self.machines.get(machine_id)
    }

    /// Record one finished round.
    ///
    /// `credits` must exclude anything the gamble did — see the module note.
    pub fn record(&mut self, machine_id: &str, wagered: i64, credits: i64, feature: bool) {
        if wagered <= 0 {
            return;
        }
        let entry = self.machines.entry(machine_id.to_owned()).or_default();

        entry.stats.record(credits, wagered);
        entry.wagered += wagered;
        entry.won += credits;
        if credits > 0 {
            entry.hits += 1;
        }
        if feature {
            entry.features += 1;
        }
        entry.best_round = entry.best_round.max(credits as f64 / wagered as f64);
    }

    /// Rounds that paid anything, across every cabinet. What the hints (§5.28)
    /// mean by "wins" — a round rather than a spin, so a feature that paid once
    /// over twelve free spins counts once.
    pub fn total_hits(&self) -> i64 {
        self.machines.values().map(|entry| entry.hits as i64).sum()
    }

    /// Total rounds across every cabinet, for the panel header.
    pub fn total_rounds(&self) -> u64 {
        self.machines.values().map(|entry| entry.rounds()).sum()
    }
}

/// A round in progress: the stake that opened it and everything credited since.
///
/// Lives on the session because only the session knows when one paid spin ends
/// and the next begins — a free spin, a bonus board and a respin round all land
/// in between, and all of them belong to the round that bought them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenRound {
    pub wagered: i64,
    pub credits: i64,
    pub feature: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    fn config() -> GameConfig {
        GameData::load().unwrap().config
    }

    #[test]
    fn a_fresh_ledger_knows_nothing() {
        let ledger = Ledger::default();
        assert!(ledger.get("dragon").is_none());
        assert_eq!(ledger.total_rounds(), 0);
    }

    #[test]
    fn recording_accumulates_the_shape_of_the_play() {
        let mut ledger = Ledger::default();
        // Three rounds at 200: nothing, exactly the stake back, ten times it.
        ledger.record("dragon", 200, 0, false);
        ledger.record("dragon", 200, 200, false);
        ledger.record("dragon", 200, 2_000, true);

        let entry = ledger.get("dragon").unwrap();
        assert_eq!(entry.rounds(), 3);
        assert_eq!(entry.wagered, 600);
        assert_eq!(entry.won, 2_200);
        assert_eq!(entry.hits, 2);
        assert_eq!(entry.features, 1);
        assert!((entry.best_round - 10.0).abs() < 1e-9);
        assert!((entry.hit_frequency() - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn each_cabinet_keeps_its_own_record() {
        // Averaging four machines that are four different games would describe
        // none of them (§5.8).
        let mut ledger = Ledger::default();
        ledger.record("dragon", 200, 400, false);
        ledger.record("frost", 200, 0, false);

        assert_eq!(ledger.get("dragon").unwrap().rounds(), 1);
        assert_eq!(ledger.get("frost").unwrap().rounds(), 1);
        assert_eq!(ledger.get("dragon").unwrap().won, 400);
        assert_eq!(ledger.get("frost").unwrap().won, 0);
        assert_eq!(ledger.total_rounds(), 2);
    }

    #[test]
    fn a_round_with_no_stake_is_ignored() {
        // A free spin is part of the round that bought it, never a round of its
        // own; one that arrived here alone would be counted as a round the
        // player never paid for and would inflate every figure on the panel.
        let mut ledger = Ledger::default();
        ledger.record("dragon", 0, 5_000, false);
        assert!(ledger.get("dragon").is_none());
    }

    #[test]
    fn the_bands_account_for_every_round() {
        let mut ledger = Ledger::default();
        for credits in [0, 100, 300, 800, 3_000, 12_000, 40_000] {
            ledger.record("dragon", 200, credits, false);
        }

        let bands = ledger.get("dragon").unwrap().bands();
        assert!((bands.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        // One round in each band, by construction.
        assert!(bands.iter().all(|share| *share > 0.0));
    }

    #[test]
    fn the_margin_shrinks_as_the_sample_grows() {
        // The panel leans on this to say how far a small sample can be trusted.
        let mut few = Ledger::default();
        let mut many = Ledger::default();
        for _ in 0..10 {
            few.record("dragon", 200, 200, false);
        }
        for _ in 0..1_000 {
            many.record("dragon", 200, 200, false);
        }

        let narrow = many.get("dragon").unwrap().margin(8.0);
        let wide = few.get("dragon").unwrap().margin(8.0);
        assert!(narrow < wide);
        assert!(narrow > 0.0);
    }

    #[test]
    fn a_single_round_has_no_meaningful_margin() {
        let mut ledger = Ledger::default();
        ledger.record("dragon", 200, 200, false);
        assert!(ledger.get("dragon").unwrap().margin(8.0).is_infinite());
    }

    #[test]
    fn a_ledger_round_trips_through_its_own_key() {
        let config = config();
        let mut ledger = Ledger::default();
        ledger.record("dragon", 200, 1_400, true);
        ledger.record("frost", 500, 0, false);
        ledger.save(&config).unwrap();

        let restored = Ledger::load(&config);
        assert_eq!(restored.get("dragon").unwrap().won, 1_400);
        assert_eq!(restored.get("dragon").unwrap().features, 1);
        assert_eq!(restored.get("frost").unwrap().rounds(), 1);
    }
}
