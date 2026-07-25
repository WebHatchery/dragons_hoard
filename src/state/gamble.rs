//! The Dragon's Gamble — double or nothing on a win (§5.16).
//!
//! # The only decision in the game that can lose money
//!
//! Every other feature either happens to the player (free spins, the Dragon's
//! Wrath) or reveals something already decided (the Vault Pick). Even the
//! Feature Buy is a purchase at a fixed price. This is the first system that
//! asks the player to put something at risk and lets them get it wrong.
//!
//! After a paying spin the win can be staked on the colour of a dragon scale —
//! **ember** or **ash**. Guess right and it doubles; guess wrong and it is gone.
//! A correct guess can be gambled again, up to a step cap and a ceiling
//! expressed in total bets. `take` banks whatever is standing.
//!
//! # It is exactly fair, and that is the whole point
//!
//! A double-or-nothing at exactly even odds has expected value
//! `0.5 × 2x + 0.5 × 0 = x`. **The gamble therefore cannot move RTP at all** —
//! it only moves variance. That is a third distinct relationship to the maths,
//! after the jackpots that *added* EV and had to be paid for out of the paytable
//! (§5.6), the Vault Pick that was *normalised* to the payout it replaced
//! (§5.10), and the Feature Buy that is *priced* at expected value (§5.13).
//!
//! Real cabinets often shave the gamble — a 47.5% win chance dressed as a coin
//! flip. There is no reason to do that here: the game is play money (§1), the
//! house edge already lives in the paytable, and a gamble that is provably fair
//! is a better thing to be able to say. `the_gamble_returns_what_it_risks`
//! asserts it over a large sample, and `the_scale_is_fair` asserts the coin
//! itself.
//!
//! What it *does* change is how quickly a balance can end up at zero, which is a
//! real cost even at neutral EV. That is why the ladder is capped and why `take`
//! is the default-looking button.
//!
//! # Half gamble
//!
//! Banking half and risking half is the same fair bet on a smaller stake, so it
//! is EV-neutral too. It exists because "all or nothing" is a worse decision to
//! be offered than "how much".

use crate::data::GambleConfig;
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};

/// The two faces of the scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scale {
    Ember,
    Ash,
}

impl Scale {
    pub fn label(self) -> &'static str {
        match self {
            Scale::Ember => "EMBER",
            Scale::Ash => "ASH",
        }
    }

    fn other(self) -> Self {
        match self {
            Scale::Ember => Scale::Ash,
            Scale::Ash => Scale::Ember,
        }
    }
}

/// What one press of the gamble button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GambleFlip {
    pub picked: Scale,
    pub landed: Scale,
    pub won: bool,
    /// Credits at risk *after* the flip: doubled, or zero.
    pub stake: i64,
}

/// Why a gamble could not go ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GambleBlocked {
    /// Nothing is on offer, or the round is already over.
    NotOffered,
    /// The step cap or the ceiling has been reached.
    LimitReached,
    /// Half-gamble asked for on a machine that does not allow it, or on a stake
    /// too small to halve.
    CannotHalve,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GambleRound {
    /// Credits currently at risk.
    stake: i64,
    /// Credits set aside by half-gambles, no longer at risk.
    banked: i64,
    /// What the round started with, for the summary line.
    opening: i64,
    steps: usize,
    /// Highest stake this round may reach, in credits.
    ceiling: i64,
    max_steps: usize,
    allow_half: bool,
    /// The most recent flip, so the UI can show the reveal before the round
    /// moves on.
    last: Option<GambleFlip>,
    finished: bool,
}

impl GambleRound {
    pub fn new(win: i64, total_bet: i64, config: &GambleConfig) -> Self {
        Self {
            stake: win,
            banked: 0,
            opening: win,
            steps: 0,
            ceiling: total_bet.saturating_mul(config.ceiling_multiple.max(1)),
            max_steps: config.max_steps.max(1),
            allow_half: config.allow_half,
            last: None,
            finished: false,
        }
    }

    pub fn stake(&self) -> i64 {
        self.stake
    }

    pub fn banked(&self) -> i64 {
        self.banked
    }

    pub fn opening(&self) -> i64 {
        self.opening
    }

    pub fn steps(&self) -> usize {
        self.steps
    }

    pub fn max_steps(&self) -> usize {
        self.max_steps
    }

    pub fn last_flip(&self) -> Option<GambleFlip> {
        self.last
    }

    /// Everything the player walks away with if they stop now.
    pub fn standing(&self) -> i64 {
        self.stake + self.banked
    }

    pub fn allows_half(&self) -> bool {
        self.allow_half && self.stake >= 2
    }

    /// Whether another flip is permitted. A round that cannot continue is over
    /// in every way except that the player still has to press Take.
    pub fn can_flip(&self) -> bool {
        !self.finished
            && self.stake > 0
            && self.steps < self.max_steps
            && self.stake <= self.ceiling
    }

    /// Risk the whole stake on a colour.
    pub fn flip(
        &mut self,
        picked: Scale,
        rng: &mut SeededRng,
    ) -> Result<GambleFlip, GambleBlocked> {
        self.flip_inner(picked, self.stake, rng)
    }

    /// Bank half and risk the rest. The odd credit goes to the player, because
    /// rounding against them on their own money is a bad look for one credit.
    pub fn flip_half(
        &mut self,
        picked: Scale,
        rng: &mut SeededRng,
    ) -> Result<GambleFlip, GambleBlocked> {
        if !self.allows_half() {
            return Err(GambleBlocked::CannotHalve);
        }
        let risked = self.stake / 2;
        let kept = self.stake - risked;
        self.banked += kept;
        self.flip_inner(picked, risked, rng)
    }

    fn flip_inner(
        &mut self,
        picked: Scale,
        risked: i64,
        rng: &mut SeededRng,
    ) -> Result<GambleFlip, GambleBlocked> {
        if self.finished || self.stake <= 0 {
            return Err(GambleBlocked::NotOffered);
        }
        if self.steps >= self.max_steps || self.stake > self.ceiling {
            return Err(GambleBlocked::LimitReached);
        }

        // One draw decides it, win or lose, so a gamble consumes the same
        // randomness either way — the rule §5.6 already follows for the jackpot
        // roll, and what keeps a saved seed replaying identically.
        let landed = if rng.below(2) == 0 {
            picked
        } else {
            picked.other()
        };
        let won = landed == picked;

        self.stake = if won { risked * 2 } else { 0 };
        self.steps += 1;
        // Losing ends it; so does running out of ladder. Either way the player
        // still presses Take, so `standing` is always what they get.
        if !won || self.steps >= self.max_steps || self.stake > self.ceiling {
            self.finished = true;
        }

        let flip = GambleFlip {
            picked,
            landed,
            won,
            stake: self.stake,
        };
        self.last = Some(flip);
        Ok(flip)
    }

    /// End the round and hand back everything standing.
    pub fn take(&mut self) -> i64 {
        self.finished = true;
        let total = self.standing();
        self.stake = 0;
        self.banked = 0;
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    fn config() -> GambleConfig {
        GameData::load().unwrap().gamble
    }

    fn round(win: i64) -> GambleRound {
        GambleRound::new(win, 200, &config())
    }

    #[test]
    fn a_correct_guess_doubles_and_a_wrong_one_takes_everything() {
        let config = config();
        let mut rng = SeededRng::new(1);
        let mut doubled = 0;
        let mut busted = 0;

        for _ in 0..400 {
            let mut round = GambleRound::new(100, 200, &config);
            let flip = round.flip(Scale::Ember, &mut rng).unwrap();
            if flip.won {
                assert_eq!(round.stake(), 200);
                doubled += 1;
            } else {
                assert_eq!(round.stake(), 0);
                assert!(!round.can_flip(), "a loss must end the round");
                busted += 1;
            }
        }

        assert!(doubled > 0 && busted > 0, "the sample was one-sided");
    }

    #[test]
    fn the_scale_is_fair() {
        // A shaved gamble is the standard way this feature is made profitable.
        // This asserts it is not.
        let config = config();
        let mut rng = SeededRng::new(20_260_725);
        let rounds = 200_000;
        let mut wins = 0;

        for _ in 0..rounds {
            let mut round = GambleRound::new(100, 200, &config);
            if round.flip(Scale::Ember, &mut rng).unwrap().won {
                wins += 1;
            }
        }

        let rate = wins as f64 / rounds as f64;
        assert!(
            (rate - 0.5).abs() < 0.01,
            "the scale landed ember {:.4} of the time",
            rate
        );
    }

    #[test]
    fn the_gamble_returns_what_it_risks() {
        // The assertion the whole design rests on: an even-money double cannot
        // move RTP, only variance. Every round is pushed as far as the ladder
        // allows, which is the worst case for the claim.
        let config = config();
        let mut rng = SeededRng::new(4242);
        let rounds = 200_000;
        let stake = 100i64;

        let mut risked = 0i64;
        let mut returned = 0i64;
        for _ in 0..rounds {
            let mut round = GambleRound::new(stake, 200, &config);
            risked += stake;
            while round.can_flip() {
                let _ = round.flip(Scale::Ember, &mut rng);
            }
            returned += round.take();
        }

        let ratio = returned as f64 / risked as f64;
        assert!(
            (ratio - 1.0).abs() < 0.03,
            "gambling returned {:.4} of what it risked",
            ratio
        );
    }

    #[test]
    fn a_half_gamble_is_fair_too() {
        let config = config();
        let mut rng = SeededRng::new(99);
        let rounds = 200_000;
        let stake = 1_000i64;

        let mut risked = 0i64;
        let mut returned = 0i64;
        for _ in 0..rounds {
            let mut round = GambleRound::new(stake, 200, &config);
            risked += stake;
            let _ = round.flip_half(Scale::Ash, &mut rng);
            returned += round.take();
        }

        let ratio = returned as f64 / risked as f64;
        assert!(
            (ratio - 1.0).abs() < 0.03,
            "half-gambling returned {:.4} of what it risked",
            ratio
        );
    }

    #[test]
    fn a_half_gamble_banks_half_out_of_reach() {
        let config = config();
        let mut rng = SeededRng::new(3);

        for seed_step in 0..80 {
            let mut round = GambleRound::new(400, 200, &config);
            let _ = round.flip_half(Scale::Ember, &mut rng);
            assert!(
                round.standing() >= 200,
                "seed step {}: banked half was lost",
                seed_step
            );
        }
    }

    #[test]
    fn the_odd_credit_goes_to_the_player() {
        let config = config();
        let mut rng = SeededRng::new(5);
        let mut round = GambleRound::new(101, 200, &config);
        round.flip_half(Scale::Ember, &mut rng).unwrap();

        // 101 halves to 50 risked and 51 kept.
        assert_eq!(round.banked(), 51);
    }

    #[test]
    fn the_ladder_stops_at_its_cap() {
        let config = config();
        let mut rng = SeededRng::new(7);

        for _ in 0..500 {
            let mut round = GambleRound::new(1, 1_000_000_000, &config);
            while round.can_flip() {
                let _ = round.flip(Scale::Ember, &mut rng);
            }
            assert!(round.steps() <= round.max_steps());
        }
    }

    #[test]
    fn a_stake_over_the_ceiling_cannot_be_gambled_again() {
        // The ceiling is what stops a lucky run compounding without bound.
        let mut config = config();
        config.ceiling_multiple = 2;
        config.max_steps = 20;

        let mut rng = SeededRng::new(11);
        // Ceiling is 2 x 200 = 400 credits.
        let mut round = GambleRound::new(500, 200, &config);
        assert!(!round.can_flip());
        assert_eq!(
            round.flip(Scale::Ember, &mut rng),
            Err(GambleBlocked::LimitReached)
        );
    }

    #[test]
    fn a_finished_round_cannot_be_gambled() {
        let config = config();
        let mut rng = SeededRng::new(13);
        let mut round = GambleRound::new(100, 200, &config);
        let taken = round.take();

        assert_eq!(taken, 100);
        assert_eq!(
            round.flip(Scale::Ember, &mut rng),
            Err(GambleBlocked::NotOffered)
        );
        assert_eq!(round.take(), 0, "taking twice must not pay twice");
    }

    #[test]
    fn half_is_refused_when_the_machine_forbids_it() {
        let mut config = config();
        config.allow_half = false;
        let mut rng = SeededRng::new(17);
        let mut round = GambleRound::new(100, 200, &config);

        assert_eq!(
            round.flip_half(Scale::Ember, &mut rng),
            Err(GambleBlocked::CannotHalve)
        );
        assert_eq!(round.stake(), 100, "the refused half must not have staked");
    }

    #[test]
    fn a_flip_consumes_the_same_randomness_whether_it_wins_or_loses() {
        // A near-miss that drew a different amount of randomness would desync
        // the stream and break save/reload determinism — the same rule the
        // jackpot roll follows (§5.6).
        let config = config();
        let mut a = SeededRng::new(555);
        let mut b = SeededRng::new(555);

        for _ in 0..200 {
            let mut win = GambleRound::new(100, 200, &config);
            let mut lose = GambleRound::new(100, 200, &config);
            let _ = win.flip(Scale::Ember, &mut a);
            let _ = lose.flip(Scale::Ash, &mut b);
        }
        assert_eq!(a.next_u64(), b.next_u64(), "the streams diverged");
    }

    #[test]
    fn the_same_seed_replays_the_same_round() {
        let mut a = SeededRng::new(808);
        let mut b = SeededRng::new(808);

        let mut first = round(500);
        let mut second = round(500);
        while first.can_flip() {
            let _ = first.flip(Scale::Ember, &mut a);
        }
        while second.can_flip() {
            let _ = second.flip(Scale::Ember, &mut b);
        }
        assert_eq!(first.take(), second.take());
    }
}
