//! The Dragon's Wrath — a hold-and-spin respin round (§5.12).
//!
//! # What it is
//!
//! Four or more Dragon Eggs on one grid wake the dragon. Those eggs lock in
//! place as coins, each stamped with a credit value, and the player is given
//! three respins. Every respin rolls the cells that are *still empty*: a coin
//! that lands locks and **resets the respins to three**, so the round only ends
//! once the board goes three spins without giving anything up. Filling all
//! fifteen cells pays a large flat bonus on top.
//!
//! # Why the trigger reuses the egg
//!
//! Hold-and-spin normally wants a dedicated coin symbol on the strips. The
//! strips *are* the RTP (§4), so adding one would mean re-cutting all five and
//! retuning everything that reads them. The Dragon Egg is already on the strips,
//! is already the game's collectible (§5.2), and four of them at once is already
//! rare — so it triggers the round while continuing to feed the hoard. A clutch
//! of eggs waking the dragon is also the reading the theme wanted anyway.
//!
//! # It adds EV, and the paytable pays for it
//!
//! Unlike the Vault Pick (§5.10), which was normalised to the payout it
//! replaced, this is a genuinely new prize and moves RTP. That is the same
//! bargain the jackpot layer made (§5.6): the sim measures the new total, the
//! paytable JSON absorbs it, and no Rust logic moves.
//!
//! Awarding the **Grand** on a full board — the iconic version of this mechanic
//! — was considered and rejected. The jackpot layer's return is asserted against
//! a closed form that assumes one bet-fair trigger per credit wagered (§5.6); a
//! second route into the same pot would invalidate that check, and losing it
//! would cost more than the moment is worth. The full board pays a configured
//! multiple of total bet instead.
//!
//! # Everything is decided by the session RNG
//!
//! Coin values and respin rolls come from the same state-owned `SeededRng` as
//! reel stops, so a round replays identically from a saved seed and the headless
//! path can play it out for the sim.

use crate::data::HoldSpinConfig;
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};

const PERMILLE: usize = 1000;

/// What a finished round paid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldSpinOutcome {
    pub credits: i64,
    pub coins: usize,
    /// True when every cell filled — the top of the feature.
    pub full_board: bool,
    /// Respins actually taken, for the summary line.
    pub respins_used: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldSpinRound {
    /// One slot per grid cell: `Some(credits)` once a coin has locked there.
    cells: Vec<Option<i64>>,
    total_bet: i64,
    respins_left: usize,
    respins_max: usize,
    respins_used: usize,
    full_board_credits: i64,
    finished: bool,
    /// Cells that locked on the most recent respin, so the UI can flash them
    /// rather than having to diff two frames.
    just_locked: Vec<usize>,
}

impl HoldSpinRound {
    /// Open a round with the triggering eggs already locked.
    ///
    /// `seeds` are the grid indices the eggs landed on. Their values are rolled
    /// here, not on the reels, because a coin's worth is part of this feature
    /// rather than part of the spin that opened it.
    pub fn new(
        cell_count: usize,
        seeds: &[usize],
        total_bet: i64,
        config: &HoldSpinConfig,
        rng: &mut SeededRng,
    ) -> Self {
        let mut cells = vec![None; cell_count];
        for index in seeds.iter().filter(|index| **index < cell_count) {
            cells[*index] = Some(roll_coin(config, total_bet, rng));
        }

        let respins = config.respins.max(1);
        Self {
            cells,
            total_bet,
            respins_left: respins,
            respins_max: respins,
            respins_used: 0,
            full_board_credits: total_bet * config.full_board_multiple,
            finished: false,
            just_locked: seeds.to_vec(),
        }
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// The credits locked in a cell, or `None` while it is still empty.
    pub fn cell(&self, index: usize) -> Option<i64> {
        self.cells.get(index).copied().flatten()
    }

    pub fn just_locked(&self, index: usize) -> bool {
        self.just_locked.contains(&index)
    }

    pub fn respins_left(&self) -> usize {
        self.respins_left
    }

    pub fn respins_max(&self) -> usize {
        self.respins_max
    }

    pub fn coins(&self) -> usize {
        self.cells.iter().filter(|cell| cell.is_some()).count()
    }

    pub fn is_full(&self) -> bool {
        self.cells.iter().all(Option::is_some)
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Credits collected so far, excluding the full-board bonus.
    pub fn collected(&self) -> i64 {
        self.cells.iter().flatten().sum()
    }

    /// Take one respin. Returns the outcome on the respin that ends the round.
    ///
    /// A round that has already finished is inert rather than an error, for the
    /// same reason a re-picked chest is (§5.10): a double input must not be able
    /// to spend something.
    pub fn respin(
        &mut self,
        config: &HoldSpinConfig,
        rng: &mut SeededRng,
    ) -> Option<HoldSpinOutcome> {
        if self.finished {
            return None;
        }

        self.just_locked.clear();
        self.respins_used += 1;

        let chance = config.coin_chance_permille.min(PERMILLE);
        for index in 0..self.cells.len() {
            if self.cells[index].is_some() {
                continue;
            }
            // Rolled for every empty cell independently — this is what makes a
            // nearly-full board feel slow and a nearly-empty one feel generous.
            if rng.below(PERMILLE) < chance {
                self.cells[index] = Some(roll_coin(config, self.total_bet, rng));
                self.just_locked.push(index);
            }
        }

        if self.just_locked.is_empty() {
            self.respins_left = self.respins_left.saturating_sub(1);
        } else {
            // Any coin at all buys the full allowance back. This is the rule the
            // whole feature turns on: the round is not a fixed three spins, it
            // is "three spins without a coin".
            self.respins_left = self.respins_max;
        }

        if self.respins_left == 0 || self.is_full() {
            self.finished = true;
            return Some(self.outcome());
        }
        None
    }

    fn outcome(&self) -> HoldSpinOutcome {
        let full_board = self.is_full();
        HoldSpinOutcome {
            credits: self.collected()
                + if full_board {
                    self.full_board_credits
                } else {
                    0
                },
            coins: self.coins(),
            full_board,
            respins_used: self.respins_used,
        }
    }
}

/// Play a round to its end without a player.
///
/// Used by the headless spin path, the sim and the capture harness. Unlike the
/// Vault Pick there is no ordering to be honest about — the player never chooses
/// anything here, so auto-play *is* the feature.
pub fn auto_play(
    round: &mut HoldSpinRound,
    config: &HoldSpinConfig,
    rng: &mut SeededRng,
) -> HoldSpinOutcome {
    // Bounded on the cell count rather than trusting the respin counter: a
    // hand-edited config with a coin chance of 1000‰ would otherwise reset the
    // allowance forever. The bound can only be reached by filling the board.
    for _ in 0..=round.cell_count() * round.respins_max() {
        if let Some(outcome) = round.respin(config, rng) {
            return outcome;
        }
        debug_assert!(!round.is_finished(), "a finished round returned no outcome");
    }
    round.finished = true;
    round.outcome()
}

/// Draw one coin value, weighted.
fn roll_coin(config: &HoldSpinConfig, total_bet: i64, rng: &mut SeededRng) -> i64 {
    let total: u32 = config.coin_values.iter().map(|value| value.weight).sum();
    if total == 0 {
        return total_bet;
    }

    let mut roll = rng.below(total as usize) as u32;
    for value in &config.coin_values {
        if roll < value.weight {
            return total_bet * value.multiple;
        }
        roll -= value.weight;
    }
    total_bet * config.coin_values.last().map_or(1, |value| value.multiple)
}

/// Mean coin value as a multiple of total bet — the weighted average of the
/// table. Half of the feature's expected value in closed form; the other half
/// (how many coins a round collects) is a Markov process with no tidy
/// expression, so the tests measure it instead.
pub fn mean_coin_multiple(config: &HoldSpinConfig) -> f64 {
    let total: u32 = config.coin_values.iter().map(|value| value.weight).sum();
    if total == 0 {
        return 0.0;
    }
    config
        .coin_values
        .iter()
        .map(|value| value.weight as f64 * value.multiple as f64)
        .sum::<f64>()
        / total as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    fn config() -> HoldSpinConfig {
        GameData::load().unwrap().holdspin
    }

    fn round(seed: u64, seeds: &[usize]) -> (HoldSpinRound, SeededRng) {
        let mut rng = SeededRng::new(seed);
        let round = HoldSpinRound::new(15, seeds, 100, &config(), &mut rng);
        (round, rng)
    }

    #[test]
    fn the_triggering_eggs_are_already_locked() {
        let (round, _) = round(1, &[0, 4, 7, 11]);

        assert_eq!(round.coins(), 4);
        for index in [0, 4, 7, 11] {
            assert!(round.cell(index).is_some(), "cell {} did not lock", index);
            assert!(round.just_locked(index));
        }
        assert!(round.cell(1).is_none());
    }

    #[test]
    fn every_locked_coin_is_worth_a_value_from_the_table() {
        let config = config();
        let multiples: Vec<i64> = config.coin_values.iter().map(|v| v.multiple).collect();

        let (mut round, mut rng) = round(2, &[0, 1, 2, 3]);
        auto_play(&mut round, &config, &mut rng);

        for index in 0..round.cell_count() {
            if let Some(credits) = round.cell(index) {
                assert!(
                    multiples.contains(&(credits / 100)),
                    "cell {} holds {} credits, off the table",
                    index,
                    credits
                );
            }
        }
    }

    #[test]
    fn a_coin_resets_the_respin_allowance() {
        // The rule the feature turns on. Built directly rather than fished for:
        // a config that always lands a coin must never lose a respin.
        let always = HoldSpinConfig {
            coin_chance_permille: 1000,
            ..config()
        };
        let mut rng = SeededRng::new(3);
        let mut round = HoldSpinRound::new(15, &[0], 100, &always, &mut rng);

        round.respin(&always, &mut rng);
        assert_eq!(round.respins_left(), round.respins_max());
        assert!(round.is_full());
        assert!(round.is_finished());
    }

    #[test]
    fn a_dry_board_runs_out_of_respins() {
        let never = HoldSpinConfig {
            coin_chance_permille: 0,
            ..config()
        };
        let mut rng = SeededRng::new(4);
        let mut round = HoldSpinRound::new(15, &[0, 1, 2, 3], 100, &never, &mut rng);

        for _ in 0..never.respins.max(1) - 1 {
            assert!(round.respin(&never, &mut rng).is_none());
        }
        let outcome = round
            .respin(&never, &mut rng)
            .expect("the round never ended");

        assert_eq!(outcome.coins, 4);
        assert!(!outcome.full_board);
        assert_eq!(outcome.credits, round.collected());
        assert_eq!(outcome.respins_used, never.respins);
    }

    #[test]
    fn a_full_board_pays_the_bonus_on_top() {
        let always = HoldSpinConfig {
            coin_chance_permille: 1000,
            ..config()
        };
        let mut rng = SeededRng::new(5);
        let mut round = HoldSpinRound::new(15, &[0], 100, &always, &mut rng);
        let outcome = auto_play(&mut round, &always, &mut rng);

        assert!(outcome.full_board);
        assert_eq!(outcome.coins, 15);
        assert_eq!(
            outcome.credits,
            round.collected() + 100 * always.full_board_multiple
        );
    }

    #[test]
    fn respinning_a_finished_round_is_ignored() {
        let config = config();
        let (mut round, mut rng) = round(6, &[0, 1, 2, 3]);
        let outcome = auto_play(&mut round, &config, &mut rng);

        assert!(round.respin(&config, &mut rng).is_none());
        assert_eq!(
            round.collected()
                + if outcome.full_board {
                    round.full_board_credits
                } else {
                    0
                },
            outcome.credits
        );
    }

    #[test]
    fn auto_play_always_terminates_even_on_a_config_that_never_runs_dry() {
        // A hand-edited 1000‰ chance resets the allowance every single respin;
        // only filling the board can stop it, and the bound guarantees it does.
        let always = HoldSpinConfig {
            coin_chance_permille: 1000,
            ..config()
        };
        let mut rng = SeededRng::new(7);
        let mut round = HoldSpinRound::new(15, &[], 100, &always, &mut rng);

        auto_play(&mut round, &always, &mut rng);
        assert!(round.is_finished());
    }

    #[test]
    fn the_same_seed_plays_the_same_round() {
        let config = config();
        let mut first = SeededRng::new(88);
        let mut second = SeededRng::new(88);

        let mut a = HoldSpinRound::new(15, &[2, 5, 9, 12], 100, &config, &mut first);
        let mut b = HoldSpinRound::new(15, &[2, 5, 9, 12], 100, &config, &mut second);

        assert_eq!(
            auto_play(&mut a, &config, &mut first),
            auto_play(&mut b, &config, &mut second)
        );
    }

    #[test]
    fn a_bigger_stake_pays_proportionally_more() {
        let config = config();
        let mut small_rng = SeededRng::new(21);
        let mut large_rng = SeededRng::new(21);

        let mut small = HoldSpinRound::new(15, &[0, 1, 2, 3], 100, &config, &mut small_rng);
        let mut large = HoldSpinRound::new(15, &[0, 1, 2, 3], 1000, &config, &mut large_rng);

        let small_outcome = auto_play(&mut small, &config, &mut small_rng);
        let large_outcome = auto_play(&mut large, &config, &mut large_rng);

        assert_eq!(large_outcome.coins, small_outcome.coins);
        assert_eq!(large_outcome.credits, small_outcome.credits * 10);
    }

    #[test]
    fn the_mean_coin_matches_the_weighted_table() {
        let config = config();
        let predicted = mean_coin_multiple(&config);

        let mut rng = SeededRng::new(9);
        let runs = 40_000;
        let total: i64 = (0..runs).map(|_| roll_coin(&config, 1, &mut rng)).sum();
        let measured = total as f64 / runs as f64;

        assert!(
            (measured - predicted).abs() < predicted * 0.05,
            "rolled a mean of {:.2}x against a table mean of {:.2}x",
            measured,
            predicted
        );
    }

    #[test]
    fn the_shipped_round_pays_within_the_designed_band() {
        // The feature's value has no closed form (the coin count is a Markov
        // process), so it is pinned by measurement. This test exists to make a
        // coin-table or chance edit visible here rather than only as a drifting
        // total RTP three tests away.
        let config = config();
        let mut rng = SeededRng::new(1234);
        let runs = 20_000;

        let mut credits = 0i64;
        let mut coins = 0usize;
        let mut full = 0usize;
        for _ in 0..runs {
            let mut round = HoldSpinRound::new(15, &[0, 1, 2, 3], 100, &config, &mut rng);
            let outcome = auto_play(&mut round, &config, &mut rng);
            credits += outcome.credits;
            coins += outcome.coins;
            full += usize::from(outcome.full_board);
        }

        let per_round = credits as f64 / runs as f64 / 100.0;
        let mean_coins = coins as f64 / runs as f64;
        println!(
            "hold&spin: {:.1}x total bet per round | {:.1} coins | full board {:.3}%",
            per_round,
            mean_coins,
            full as f64 / runs as f64 * 100.0
        );

        assert!(
            (20.0..80.0).contains(&per_round),
            "a round pays {:.1}x total bet, outside the designed 20-80x band",
            per_round
        );
        assert!(
            mean_coins >= 4.0,
            "a round collects {:.1} coins, fewer than the four it started with",
            mean_coins
        );
    }
}
