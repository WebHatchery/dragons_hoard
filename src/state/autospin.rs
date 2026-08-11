//! Autospin: spin N times unattended, stopping early when something worth
//! looking at happens.
//!
//! The stop conditions are the point of the feature. An autospin that ploughs
//! through a free-spin trigger has taken the interesting moment away from the
//! player, so anything that would raise a celebration also stops the run.

use serde::{Deserialize, Serialize};

/// Why an autospin run ended. Surfaced so the player is told, rather than the
/// counter just quietly stopping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutospinStop {
    Completed,
    FeatureTriggered,
    Hatched,
    /// A clutch of eggs opened the Dragon's Wrath (§5.12).
    WrathWoken,
    /// The board came to rest full of one treasure (§5.80).
    SeamOpened,
    JackpotWon,
    BigWin,
    OutOfCredits,
    Cancelled,
    /// A reality check came due mid-run (§5.30). An unattended run is exactly
    /// the state the check exists to interrupt, so it does not spin on behind
    /// the panel.
    RealityCheck,
    /// A session cap bound mid-run (§5.30).
    LimitReached,
}

impl AutospinStop {
    pub fn message(self) -> &'static str {
        match self {
            AutospinStop::Completed => "Autospin finished",
            AutospinStop::FeatureTriggered => "Autospin stopped — free spins!",
            AutospinStop::Hatched => "Autospin stopped — the hoard hatched",
            AutospinStop::WrathWoken => "Autospin stopped — the dragon wakes",
            AutospinStop::SeamOpened => "Autospin stopped — a seam runs through the board",
            AutospinStop::JackpotWon => "Autospin stopped — jackpot!",
            AutospinStop::BigWin => "Autospin stopped — big win",
            AutospinStop::OutOfCredits => "Autospin stopped — out of credits",
            AutospinStop::Cancelled => "Autospin cancelled",
            AutospinStop::RealityCheck => "Autospin paused for a reality check",
            AutospinStop::LimitReached => "Autospin stopped — session limit reached",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutospinState {
    remaining: u32,
}

impl AutospinState {
    pub fn new(spins: u32) -> Self {
        Self { remaining: spins }
    }

    pub fn remaining(&self) -> u32 {
        self.remaining
    }

    /// Consume one spin from the run. Returns false when the run is spent.
    pub fn take(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        self.remaining > 0
    }
}

#[cfg(test)]
mod tests;
