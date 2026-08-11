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
mod tests;
