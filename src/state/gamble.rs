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
mod tests;
