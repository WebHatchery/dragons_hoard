//! Progressive jackpots — four pots that grow off every stake and pay out at
//! random, independently of what the reels do.
//!
//! # Why the trigger is a roll, not a symbol
//!
//! Tying jackpots to a reel combination would mean re-cutting the strips, and
//! the strips *are* the RTP (§4) — every tier would drag the base game around
//! with it. A mystery trigger keeps the two mathematically separate: the reels
//! pay what they always paid, and the jackpot layer adds an exactly computable
//! slice on top.
//!
//! # Why odds are "per credit wagered"
//!
//! Expressing a tier as *one hit per N credits of turnover* makes it **bet-fair**
//! by construction. Doubling the stake doubles the chance, so expected return per
//! credit is identical at every rung of the bet ladder — the same principle that
//! makes the hoard bank each egg at its landing bet (§3), and the reason a player
//! cannot farm a jackpot cheaply and cash it in expensive.
//!
//! It also makes the maths closed-form. A tier returns
//! `seed / odds + contribution_rate × share` of turnover, because exactly `odds`
//! credits are wagered between wins by definition. That is what lets §4 predict
//! the jackpot RTP rather than only measure it.
//!
//! # Money is integers
//!
//! Pots accrue in **milli-credits** so a 20-credit spin still moves the smallest
//! tier. Nothing here uses floating point (§12).

use crate::data::Jackpots;
use serde::{Deserialize, Serialize};

/// Milli-credits per credit.
const MILLI: i64 = 1_000;

/// A jackpot that has just been won.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JackpotWin {
    pub tier: usize,
    pub name: String,
    pub credits: i64,
}

/// Accrued value of every tier, in milli-credits above its seed.
///
/// Stored as a plain vector so a save from an older build with fewer tiers still
/// loads — [`resize_to`] tops it up rather than failing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JackpotState {
    accrued_milli: Vec<i64>,
}

impl JackpotState {
    pub fn new(jackpots: &Jackpots) -> Self {
        Self {
            accrued_milli: vec![0; jackpots.tiers.len()],
        }
    }

    /// Grow or shrink to match the current tier list, keeping what is already
    /// banked. Called on load so editing `jackpots.json` never strands a save.
    pub fn resize_to(&mut self, jackpots: &Jackpots) {
        self.accrued_milli.resize(jackpots.tiers.len(), 0);
    }

    /// What a tier has banked, in milli-credits above its seed.
    ///
    /// Exposed so the floor store (§5.57) can move a shared pot between
    /// cabinets without knowing anything else about a ladder.
    pub fn accrued_milli(&self, tier: usize) -> i64 {
        self.accrued_milli.get(tier).copied().unwrap_or(0)
    }

    pub fn set_accrued_milli(&mut self, tier: usize, milli: i64) {
        if let Some(slot) = self.accrued_milli.get_mut(tier) {
            *slot = milli;
        }
    }

    /// Current headline value of a tier: its seed plus everything banked since
    /// it last paid out.
    pub fn value(&self, jackpots: &Jackpots, tier: usize) -> i64 {
        let Some(def) = jackpots.tiers.get(tier) else {
            return 0;
        };
        def.seed + self.accrued_milli.get(tier).copied().unwrap_or(0) / MILLI
    }

    /// Feed every pot from one paid spin.
    pub fn contribute(&mut self, jackpots: &Jackpots, total_bet: i64) {
        if total_bet <= 0 {
            return;
        }
        for (index, tier) in jackpots.tiers.iter().enumerate() {
            let Some(slot) = self.accrued_milli.get_mut(index) else {
                continue;
            };
            // total_bet credits × contribution ‰ × share ‰, in milli-credits.
            *slot += total_bet * jackpots.contribution_permille * tier.share_permille / MILLI;
        }
    }

    /// Roll for a win, richest tier first so a Grand is never masked by a Mini
    /// landing on the same spin. Consumes exactly one RNG draw per tier, so the
    /// sequence stays reproducible.
    pub fn roll(
        &mut self,
        jackpots: &Jackpots,
        rng: &mut macroquad_toolkit::rng::SeededRng,
        total_bet: i64,
    ) -> Option<JackpotWin> {
        if total_bet <= 0 {
            return None;
        }

        let mut hit = None;
        for (index, tier) in jackpots.tiers.iter().enumerate().rev() {
            let odds = tier.odds_per_credit.max(1) as u64;
            let roll = rng.next_u64() % odds;
            if hit.is_none() && roll < total_bet as u64 {
                hit = Some(index);
            }
        }

        let tier = hit?;
        let credits = self.value(jackpots, tier);
        if let Some(slot) = self.accrued_milli.get_mut(tier) {
            *slot = 0;
        }

        Some(JackpotWin {
            tier,
            name: jackpots.tiers[tier].name.clone(),
            credits,
        })
    }
}

/// Expected return of the whole jackpot layer as a fraction of turnover.
///
/// Closed form, so §4's numbers can be predicted from the data rather than only
/// measured: a tier returns its seed once per `odds` credits wagered, plus its
/// slice of every stake.
pub fn expected_rtp(jackpots: &Jackpots) -> f64 {
    jackpots
        .tiers
        .iter()
        .map(|tier| {
            let seed_share = tier.seed as f64 / tier.odds_per_credit.max(1) as f64;
            let contribution = jackpots.contribution_permille as f64 / 1000.0
                * (tier.share_permille as f64 / 1000.0);
            seed_share + contribution
        })
        .sum()
}

/// A tier's value if it were won right now, for the ladder display.
pub fn ladder(jackpots: &Jackpots, state: &JackpotState) -> Vec<(String, i64)> {
    jackpots
        .tiers
        .iter()
        .enumerate()
        .map(|(index, tier)| (tier.name.clone(), state.value(jackpots, index)))
        .collect()
}

#[cfg(test)]
mod tests;
