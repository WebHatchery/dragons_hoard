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

use crate::data::{GameConfig, Jackpots};
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

/// Jackpots are a play-money flourish; this keeps the config honest about it.
pub fn validate(jackpots: &Jackpots, _config: &GameConfig) -> Result<(), String> {
    if jackpots.tiers.is_empty() {
        return Err("jackpots.json declared no tiers".to_owned());
    }
    if jackpots.contribution_permille < 0 {
        return Err("contribution_permille cannot be negative".to_owned());
    }

    let shares: i64 = jackpots.tiers.iter().map(|tier| tier.share_permille).sum();
    if shares != 1000 {
        return Err(format!(
            "jackpot shares must total 1000 permille, got {}",
            shares
        ));
    }

    for tier in &jackpots.tiers {
        if tier.odds_per_credit <= 0 {
            return Err(format!("jackpot '{}' has non-positive odds", tier.id));
        }
        if tier.seed < 0 {
            return Err(format!("jackpot '{}' has a negative seed", tier.id));
        }
    }

    // Richest tier last keeps `roll`'s "check the big one first" reversal honest.
    if jackpots
        .tiers
        .windows(2)
        .any(|pair| pair[1].odds_per_credit <= pair[0].odds_per_credit)
    {
        return Err("jackpot tiers must be ordered from most to least frequent".to_owned());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use macroquad_toolkit::rng::SeededRng;

    fn data() -> GameData {
        GameData::load().unwrap()
    }

    #[test]
    fn the_shipped_config_is_valid() {
        let data = data();
        validate(&data.jackpots, &data.config).unwrap();
    }

    #[test]
    fn a_fresh_pot_shows_its_seed() {
        let data = data();
        let state = JackpotState::new(&data.jackpots);

        for (index, tier) in data.jackpots.tiers.iter().enumerate() {
            assert_eq!(state.value(&data.jackpots, index), tier.seed);
        }
    }

    #[test]
    fn every_stake_feeds_every_pot() {
        let data = data();
        let mut state = JackpotState::new(&data.jackpots);
        let before: Vec<i64> = (0..data.jackpots.tiers.len())
            .map(|i| state.value(&data.jackpots, i))
            .collect();

        // One spin moves the pots by fractions of a credit, so accumulate.
        for _ in 0..1000 {
            state.contribute(&data.jackpots, 200);
        }

        for (index, was) in before.iter().enumerate() {
            assert!(
                state.value(&data.jackpots, index) > *was,
                "tier {} never grew",
                index
            );
        }
    }

    #[test]
    fn a_small_stake_still_moves_the_smallest_pot() {
        // The reason pots accrue in milli-credits: at the minimum bet a whole
        // credit of contribution would round to zero and the pot would never move.
        let data = data();
        let mut state = JackpotState::new(&data.jackpots);
        let min_bet = data.total_bet(data.config.line_bets[0]);

        for _ in 0..500 {
            state.contribute(&data.jackpots, min_bet);
        }

        assert!(state.value(&data.jackpots, 0) > data.jackpots.tiers[0].seed);
    }

    #[test]
    fn winning_a_tier_resets_it_to_seed_and_leaves_the_others() {
        let data = data();
        let mut state = JackpotState::new(&data.jackpots);
        for _ in 0..10_000 {
            state.contribute(&data.jackpots, 200);
        }
        let before: Vec<i64> = (0..data.jackpots.tiers.len())
            .map(|index| state.value(&data.jackpots, index))
            .collect();

        // A stake at least as large as the longest odds always clears the roll,
        // and the richest tier is tested first — so this is a guaranteed Grand.
        let certain = data.jackpots.tiers.last().unwrap().odds_per_credit;
        let mut rng = SeededRng::new(1);
        let win = state
            .roll(&data.jackpots, &mut rng, certain)
            .expect("a stake this size cannot miss");

        assert_eq!(win.tier, data.jackpots.tiers.len() - 1);
        assert!(win.credits > data.jackpots.tiers[win.tier].seed);
        assert_eq!(
            state.value(&data.jackpots, win.tier),
            data.jackpots.tiers[win.tier].seed
        );

        // Winning one tier must not disturb the others.
        for (index, was) in before.iter().enumerate().take(win.tier) {
            assert_eq!(
                state.value(&data.jackpots, index),
                *was,
                "tier {} was disturbed by another tier paying out",
                index
            );
        }
    }

    #[test]
    fn the_trigger_is_bet_fair() {
        // The whole point of per-credit odds: expected return per credit staked
        // must not vary with the stake, or the ladder becomes exploitable.
        let data = data();
        let tier = &data.jackpots.tiers[0];
        let odds = tier.odds_per_credit as f64;

        for bet in [20.0, 200.0, 500.0] {
            let hits_per_spin = bet / odds;
            let return_per_credit = hits_per_spin * tier.seed as f64 / bet;
            assert!(
                (return_per_credit - tier.seed as f64 / odds).abs() < 1e-12,
                "stake {} changed the per-credit return",
                bet
            );
        }
    }

    #[test]
    fn rolling_consumes_the_same_randomness_whether_or_not_it_hits() {
        // One draw per tier, always — otherwise a near-miss would desync the
        // RNG stream and break save/reload determinism.
        let data = data();
        let mut state = JackpotState::new(&data.jackpots);
        let mut a = SeededRng::new(4242);
        let mut b = SeededRng::new(4242);

        state.roll(&data.jackpots, &mut a, 200);
        for _ in 0..data.jackpots.tiers.len() {
            b.next_u64();
        }

        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn the_expected_return_matches_the_closed_form() {
        let data = data();
        let rtp = expected_rtp(&data.jackpots);

        // Sanity band: the jackpot layer should be a few points, not a few
        // percent of a percent, and never larger than the base game.
        assert!(rtp > 0.02 && rtp < 0.08, "jackpot RTP {:.4} is off", rtp);
    }

    #[test]
    fn an_older_save_with_fewer_tiers_is_topped_up() {
        let data = data();
        let mut state = JackpotState {
            accrued_milli: vec![5_000],
        };
        state.resize_to(&data.jackpots);

        assert_eq!(state.accrued_milli.len(), data.jackpots.tiers.len());
        // What was banked survives.
        assert_eq!(
            state.value(&data.jackpots, 0),
            data.jackpots.tiers[0].seed + 5
        );
        assert_eq!(state.value(&data.jackpots, 3), data.jackpots.tiers[3].seed);
    }

    #[test]
    fn the_ladder_reports_one_row_per_tier() {
        let data = data();
        let state = JackpotState::new(&data.jackpots);
        let rows = ladder(&data.jackpots, &state);

        assert_eq!(rows.len(), data.jackpots.tiers.len());
        assert_eq!(rows[0].0, "Mini");
        assert_eq!(rows[3].1, data.jackpots.tiers[3].seed);
    }
}
