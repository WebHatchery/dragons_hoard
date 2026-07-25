//! The Vault Pick — the second-screen bonus the Hoard now opens into.
//!
//! # It replaces a payout rather than adding one
//!
//! Filling the hoard used to pay `pot × hatch_pot_multiplier` instantly. It now
//! opens a board of chests instead: the player picks until three are empty, and
//! each prize revealed adds a share of that same figure.
//!
//! Every prize is **permille of the hatch base**, and the table is built so the
//! expected sum is 1000‰. The bonus therefore pays what the instant hatch paid,
//! on average, and only the *variance* changes. That is the whole reason it
//! could be added to a game already tuned to 0.9567 and 0.9475 (§4) without
//! re-cutting a single reel strip — a deliberate contrast with the jackpot layer
//! (§5.6), which cost a full paytable retune.
//!
//! It also means one shared `bonus.json` serves both machines: each cabinet's
//! own `hatch_pot_multiplier` already scales the base, so the board does not
//! need to know which machine it is on.
//!
//! # The board is dealt at trigger, not at pick
//!
//! Contents are drawn and shuffled from the session RNG the moment the bonus
//! opens, exactly like reel stops (§8.2). Picking only *reveals* a decided
//! board, so the headless path can auto-play it and the sim measures the real
//! feature. Because the board is shuffled, picking in index order is
//! statistically identical to picking at random — which is what makes the sim's
//! auto-play honest rather than a convenient fiction.

use crate::data::BonusConfig;
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};

/// Permille denominator. A prize of 150 is 15% of the hatch base.
const PERMILLE: i64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BonusCell {
    /// A share of the hatch base, in permille.
    Prize(i64),
    /// One of the three that end the round.
    Blank,
}

/// What a finished round paid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BonusOutcome {
    pub credits: i64,
    /// Total permille collected, for the summary line.
    pub collected_permille: i64,
    pub prizes_taken: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusRound {
    /// The hatch prize this board is dividing up.
    base: i64,
    board: Vec<BonusCell>,
    revealed: Vec<bool>,
    blanks_needed: usize,
    blanks_found: usize,
    collected_permille: i64,
    prizes_taken: usize,
}

impl BonusRound {
    /// Deal a board. Prizes are drawn from the configured table with
    /// replacement, then shuffled in with the blanks.
    pub fn new(base: i64, config: &BonusConfig, rng: &mut SeededRng) -> Self {
        let size = config.board_size.max(config.blanks + 1);
        let blanks = config.blanks.min(size.saturating_sub(1)).max(1);

        let mut board = Vec::with_capacity(size);
        for _ in 0..(size - blanks) {
            let prize = config
                .prizes_permille
                .get(rng.below(config.prizes_permille.len().max(1)))
                .copied()
                .unwrap_or(PERMILLE);
            board.push(BonusCell::Prize(prize));
        }
        for _ in 0..blanks {
            board.push(BonusCell::Blank);
        }

        // Fisher-Yates from the session RNG, so the deal is reproducible from a
        // saved seed like everything else that decides money.
        for index in (1..board.len()).rev() {
            board.swap(index, rng.below(index + 1));
        }

        Self {
            base,
            revealed: vec![false; board.len()],
            board,
            blanks_needed: blanks,
            blanks_found: 0,
            collected_permille: 0,
            prizes_taken: 0,
        }
    }

    pub fn board_size(&self) -> usize {
        self.board.len()
    }

    pub fn is_revealed(&self, index: usize) -> bool {
        self.revealed.get(index).copied().unwrap_or(false)
    }

    /// The cell's contents, but only once it has been turned over — the UI must
    /// not be able to read the board ahead of the player.
    pub fn revealed_cell(&self, index: usize) -> Option<BonusCell> {
        if self.is_revealed(index) {
            self.board.get(index).copied()
        } else {
            None
        }
    }

    pub fn base(&self) -> i64 {
        self.base
    }

    pub fn blanks_found(&self) -> usize {
        self.blanks_found
    }

    pub fn blanks_needed(&self) -> usize {
        self.blanks_needed
    }

    pub fn collected_permille(&self) -> i64 {
        self.collected_permille
    }

    /// Credits the round would pay if it ended now.
    pub fn running_credits(&self) -> i64 {
        self.base * self.collected_permille / PERMILLE
    }

    pub fn is_finished(&self) -> bool {
        self.blanks_found >= self.blanks_needed
    }

    /// Turn a chest over. Returns the outcome on the pick that ends the round.
    ///
    /// Re-picking a revealed cell, or picking after the round is over, is
    /// ignored rather than treated as an error: a double click should not cost
    /// the player a blank.
    pub fn pick(&mut self, index: usize) -> Option<BonusOutcome> {
        if self.is_finished() || self.is_revealed(index) {
            return None;
        }
        let cell = self.board.get(index).copied()?;

        self.revealed[index] = true;
        match cell {
            BonusCell::Prize(permille) => {
                self.collected_permille += permille;
                self.prizes_taken += 1;
            }
            BonusCell::Blank => self.blanks_found += 1,
        }

        self.is_finished().then(|| self.outcome())
    }

    /// The first unrevealed cell, for auto-play.
    pub fn next_unrevealed(&self) -> Option<usize> {
        self.revealed.iter().position(|seen| !seen)
    }

    fn outcome(&self) -> BonusOutcome {
        BonusOutcome {
            credits: self.running_credits(),
            collected_permille: self.collected_permille,
            prizes_taken: self.prizes_taken,
        }
    }
}

/// Play the whole board out without a player.
///
/// Used by the headless spin path, the sim and the capture harness. Picking in
/// index order is legitimate because the board was shuffled at deal time.
pub fn auto_play(round: &mut BonusRound) -> BonusOutcome {
    while let Some(index) = round.next_unrevealed() {
        if let Some(outcome) = round.pick(index) {
            return outcome;
        }
    }
    // Unreachable with a valid config (blanks < board_size), but returning the
    // running total is safer than panicking mid-spin.
    BonusOutcome {
        credits: round.running_credits(),
        collected_permille: round.collected_permille,
        prizes_taken: round.prizes_taken,
    }
}

/// Expected permille a round collects, in closed form.
///
/// With `p` prizes and `b` blanks shuffled together, the expected number of
/// prizes revealed before the `b`-th blank is `p · b / (b + 1)`. Multiply by the
/// mean prize and you have the feature's expected value without simulating it —
/// the same trick §5.6 uses for the jackpot layer, and the check that the table
/// really does average out to the instant hatch it replaced.
pub fn expected_permille(config: &BonusConfig) -> f64 {
    if config.prizes_permille.is_empty() {
        return 0.0;
    }
    let blanks = config.blanks.max(1) as f64;
    let prizes = config.board_size.saturating_sub(config.blanks) as f64;
    let mean: f64 =
        config.prizes_permille.iter().sum::<i64>() as f64 / config.prizes_permille.len() as f64;

    prizes * blanks / (blanks + 1.0) * mean
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    fn config() -> BonusConfig {
        GameData::load().unwrap().bonus
    }

    fn round(seed: u64) -> BonusRound {
        let mut rng = SeededRng::new(seed);
        BonusRound::new(1000, &config(), &mut rng)
    }

    #[test]
    fn the_board_holds_exactly_the_configured_blanks() {
        let config = config();
        let round = round(1);

        assert_eq!(round.board_size(), config.board_size);
        let blanks = round
            .board
            .iter()
            .filter(|cell| **cell == BonusCell::Blank)
            .count();
        assert_eq!(blanks, config.blanks);
    }

    #[test]
    fn nothing_is_visible_before_it_is_picked() {
        // The UI reads cells through `revealed_cell`, so a bug there would let
        // the board be read ahead of the player.
        let round = round(2);
        for index in 0..round.board_size() {
            assert!(round.revealed_cell(index).is_none());
        }
    }

    #[test]
    fn a_round_ends_on_the_last_blank_and_not_before() {
        let mut round = round(3);
        let needed = round.blanks_needed();
        let mut ended_on = None;

        for index in 0..round.board_size() {
            let before = round.blanks_found();
            if let Some(outcome) = round.pick(index) {
                ended_on = Some((index, outcome));
                break;
            }
            assert!(round.blanks_found() <= needed);
            assert!(round.blanks_found() >= before);
        }

        let (_, outcome) = ended_on.expect("the round never ended");
        assert_eq!(round.blanks_found(), needed);
        assert!(round.is_finished());
        assert_eq!(outcome.credits, round.running_credits());
    }

    #[test]
    fn picking_the_same_chest_twice_costs_nothing() {
        // A double click must not be able to spend a blank.
        let mut round = round(4);
        round.pick(0);
        let blanks = round.blanks_found();
        let collected = round.collected_permille();

        assert_eq!(round.pick(0), None);
        assert_eq!(round.blanks_found(), blanks);
        assert_eq!(round.collected_permille(), collected);
    }

    #[test]
    fn picking_after_the_round_is_over_is_ignored() {
        let mut round = round(5);
        auto_play(&mut round);
        let collected = round.collected_permille();

        for index in 0..round.board_size() {
            assert_eq!(round.pick(index), None);
        }
        assert_eq!(round.collected_permille(), collected);
    }

    #[test]
    fn auto_play_always_terminates_and_matches_the_running_total() {
        for seed in 0..50 {
            let mut round = round(seed);
            let outcome = auto_play(&mut round);

            assert!(round.is_finished());
            assert_eq!(outcome.credits, round.running_credits());
            assert_eq!(outcome.prizes_taken, round.prizes_taken);
        }
    }

    #[test]
    fn the_same_seed_deals_the_same_board() {
        let mut a = SeededRng::new(99);
        let mut b = SeededRng::new(99);
        let config = config();

        let first = BonusRound::new(500, &config, &mut a);
        let second = BonusRound::new(500, &config, &mut b);

        assert_eq!(first.board, second.board);
    }

    #[test]
    fn the_expected_payout_matches_the_hatch_it_replaced() {
        // The whole design rests on this: a round should collect 1000 permille
        // on average, so the bonus pays what the instant hatch used to pay and
        // the RTP does not move.
        let config = config();
        let predicted = expected_permille(&config);
        assert!(
            (predicted - 1000.0).abs() < 120.0,
            "the prize table averages {:.0} permille, not ~1000",
            predicted
        );

        let mut rng = SeededRng::new(7);
        let mut total = 0i64;
        let runs = 20_000;
        for _ in 0..runs {
            let mut round = BonusRound::new(1000, &config, &mut rng);
            total += auto_play(&mut round).collected_permille;
        }
        let measured = total as f64 / runs as f64;

        assert!(
            (measured - predicted).abs() < 60.0,
            "measured {:.0} permille against a predicted {:.0}",
            measured,
            predicted
        );
    }

    #[test]
    fn a_bigger_pot_pays_proportionally_more() {
        let config = config();
        let mut small = BonusRound::new(1_000, &config, &mut SeededRng::new(11));
        let mut large = BonusRound::new(10_000, &config, &mut SeededRng::new(11));

        let small_outcome = auto_play(&mut small);
        let large_outcome = auto_play(&mut large);

        assert_eq!(
            small_outcome.collected_permille,
            large_outcome.collected_permille
        );
        assert_eq!(large_outcome.credits, small_outcome.credits * 10);
    }

    #[test]
    fn a_board_with_more_blanks_than_cells_still_deals() {
        // Guards a hand-edited config from producing a round that can never end.
        let config = BonusConfig {
            board_size: 2,
            blanks: 9,
            prizes_permille: vec![100],
        };
        let mut round = BonusRound::new(100, &config, &mut SeededRng::new(1));

        assert!(round.board_size() > round.blanks_needed());
        auto_play(&mut round);
        assert!(round.is_finished());
    }
}
