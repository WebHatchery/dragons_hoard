//! An open Seam round (§5.80).
//!
//! The engine next door knows how to find a seam and how to work one beat of
//! it. This is the round: which rite was drawn, how many beats are left, the
//! board as it stands, and what the whole thing ends up being worth.
//!
//! # What it pays
//!
//! One formula, for every rite: **`board x multiplier - baseline`**, floored at
//! zero and capped.
//!
//! The baseline is what the board was worth when the reels stopped, and
//! subtracting it is the only honest thing to do — the spin has already paid for
//! the grid it landed on, and paying the final board outright would pay that
//! grid twice. The multiplier is 1 for the two rites that change symbols, and
//! is what the gilding rite raises instead of changing any (§5.81). Written as
//! one expression rather than a branch per rite because a second payment path
//! is a second place for the ceiling, the floor and the free-spin multiplier to
//! be got wrong.
//!
//! Only line/ways/cluster wins are counted. Scatters and eggs are read off the
//! landing grid by the spin that owns them and are not read again here, which is
//! the other half of the rule that a rite never mints a special symbol: the
//! feature cannot award free spins, cannot bank an egg, and cannot wake the
//! dragon. It moves money and nothing else.
//!
//! # Held like both of the rounds beside it
//!
//! A seam opens **unchosen**: the board is frozen and the player picks which
//! rite runs, which is the Vault Pick's shape (§5.10). Once picked it advances
//! on a beat and ends by itself, which is the Dragon's Wrath's (§5.12).
//!
//! The choice is only offered in the base game. During free spins or an
//! autospin run the rite is drawn by weight instead, for the reason §5.16 gives
//! for the gamble: a round that waits on a decision either stalls a chain that
//! is spinning itself or gets run straight over, and neither is a decision the
//! player actually got to make.

use crate::data::{GameData, RiteDef, RiteKind, SeamConfig};
use crate::engine::evaluate::{evaluate, EvalContext};
use crate::engine::reels::Grid;
use crate::engine::seam::{self, Seam};
use macroquad_toolkit::rng::SeededRng;

/// What a finished seam paid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeamOutcome {
    pub credits: i64,
    /// The rite that ran, for the card and the sound.
    pub rite_id: String,
    pub rite_name: String,
    /// The symbol the seam ended as. Not the one it started as, when the rite
    /// was an enrichment.
    pub symbol: usize,
    /// Cells the seam held at the end.
    pub cells: usize,
    /// Beats actually taken. Shorter than the cabinet's `steps` when a rite ran
    /// out of board or out of ladder.
    pub steps: usize,
    /// Whether the ceiling caught the payout. Worth surfacing: a capped seam is
    /// the one moment the feature's own limit is visible to the player.
    pub capped: bool,
    /// What the board was worth when the reels stopped (§5.85).
    ///
    /// Carried so a round that paid nothing can say *which* nothing it was: a
    /// gilding on a board that was not paying is a multiple of zero, and a rite
    /// on a board that was already paying and added nothing is a different
    /// disappointment. The card is the only thing that reads it, and it is
    /// cheaper to carry the figure than to have the card work out the reason
    /// from a grid it does not have.
    pub baseline: i64,
}

/// Applied to nothing until a gilding rite raises it. Permille so a cabinet can
/// ask for one-and-a-half without the game learning about floating point.
const PLAIN: i64 = 1_000;

#[derive(Debug, Clone)]
pub struct SeamRound {
    /// The board as the rite has left it. Starts as the grid the reels rested
    /// on and is what the reel window draws while the round is open.
    grid: Grid,
    seam: Seam,
    /// `None` until the rite is settled — by the player in the base game, by a
    /// weighted draw everywhere else. The round holds the game either way.
    rite: Option<RiteDef>,
    /// Every rite this cabinet offers, so the panel can lay the choice out
    /// without reaching back into `GameData`.
    offered: Vec<RiteDef>,
    /// What the board pays multiplied by, in permille (§5.81).
    multiplier: i64,
    steps_left: usize,
    steps_taken: usize,
    /// Cells the last beat changed, so the UI can flash them rather than diff
    /// two frames.
    changed: Vec<usize>,
    /// Bet and multiplier as they were on the spin that opened this, so a seam
    /// opened during free spins pays at the run's multiplier.
    ctx: EvalContext,
    /// What the board was worth before the rite touched it.
    baseline: i64,
    ceiling: i64,
    finished: bool,
}

impl SeamRound {
    /// Open a round on a grid that has already been found to hold a seam.
    ///
    /// No rite yet, and no RNG consumed: which rite runs belongs to this round
    /// rather than to the spin that opened it, and in the base game it belongs
    /// to the player. A caller that cannot ask calls
    /// [`draw_rite`](Self::draw_rite) straight afterwards.
    pub fn open(data: &GameData, grid: &Grid, seam: Seam, ctx: EvalContext) -> Option<Self> {
        let config = &data.seam;
        if config.rites.is_empty() {
            return None;
        }
        Some(Self {
            baseline: evaluate(data, grid, &ctx).win_credits,
            grid: grid.clone(),
            changed: seam.cells.clone(),
            seam,
            rite: None,
            offered: config.rites.clone(),
            multiplier: PLAIN,
            steps_left: config.steps.max(1),
            steps_taken: 0,
            ctx,
            // Multiplied by the run's own multiplier, not just by total bet.
            //
            // Without that the ceiling silently swallows the free-spin
            // multiplier: the uplift triples on a x3 run, the cap does not, and
            // a feature that pays "up to 3x the bet" is worth a third as much
            // during the part of the game where everything else is worth three
            // times as much. §5.81 claimed a seam pays at the run's multiplier
            // and this line is what made that only true below the cap (§5.83).
            ceiling: ctx.total_bet * config.max_multiple.max(1) * ctx.win_multiplier.max(1),
            finished: false,
        })
    }

    /// The rites still on offer. Empty once one has been taken, so a stale press
    /// from the frame the choice was made in cannot re-cut the deal — the same
    /// guard the free-spin shapes carry (§5.64).
    pub fn offered(&self) -> &[RiteDef] {
        if self.rite.is_some() {
            return &[];
        }
        &self.offered
    }

    /// Take the rite at this index. `false` if the choice has already been made
    /// or the index is not on the panel.
    pub fn choose(&mut self, index: usize) -> bool {
        if self.rite.is_some() {
            return false;
        }
        match self.offered.get(index) {
            Some(rite) => {
                self.rite = Some(rite.clone());
                true
            }
            None => false,
        }
    }

    /// Settle the rite by weight, for every caller that cannot ask a player.
    pub fn draw_rite(&mut self, rng: &mut SeededRng) {
        if self.rite.is_some() {
            return;
        }
        self.rite = seam::draw_weighted(&self.offered, rng).cloned();
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// What the board is being paid at, in permille. Above 1000 only while a
    /// gilding rite is running.
    pub fn multiplier_permille(&self) -> i64 {
        self.multiplier
    }

    pub fn symbol(&self) -> usize {
        self.seam.symbol
    }

    pub fn cells(&self) -> &[usize] {
        &self.seam.cells
    }

    pub fn just_changed(&self, cell: usize) -> bool {
        self.changed.contains(&cell)
    }

    /// The rite in force, once one has been settled.
    pub fn rite(&self) -> Option<&RiteDef> {
        self.rite.as_ref()
    }

    pub fn steps_left(&self) -> usize {
        self.steps_left
    }

    /// What the board was worth when the reels stopped.
    ///
    /// The one figure the choice turns on, so the panel can state it (§5.86).
    pub fn baseline(&self) -> i64 {
        self.baseline
    }

    /// Cells currently touching the seam that a widening could take.
    ///
    /// A fact about the board in front of the player, not a prediction: how many
    /// are *offered*, never how many will turn. The panel states it so the three
    /// rites each say something concrete about this board rather than one
    /// carrying a figure and two carrying adjectives (§5.87).
    pub fn frontier(&self, data: &GameData) -> usize {
        seam::frontier(data, &self.grid, &self.seam).len()
    }

    /// The symbol an enrichment would climb to, if there is one above.
    ///
    /// `None` on the top rung, which is the case the panel has to say out loud —
    /// a deepening there changes nothing and pays nothing, and it looks
    /// identical to one that would.
    pub fn next_rung(&self, data: &GameData) -> Option<usize> {
        let ladder = seam::ladder(data);
        let at = ladder.iter().position(|rung| *rung == self.seam.symbol)?;
        ladder.get(at + 1).copied()
    }

    /// What this round would pay if it ended at `multiplier` permille.
    ///
    /// For the choice panel, which can quote a gilding exactly because a gilding
    /// is decided entirely by the board already on screen. Routed through the
    /// round rather than recomputed in the UI so the quote carries the ceiling
    /// and the free-spin multiplier — the two things a hand-rolled version in a
    /// draw call would forget, and would forget silently.
    pub fn worth_at(&self, data: &GameData, multiplier: i64) -> i64 {
        let board = evaluate(data, &self.grid, &self.ctx).win_credits;
        (board * multiplier / PLAIN - self.baseline)
            .max(0)
            .min(self.ceiling)
    }

    /// What the round would pay if it ended now, before the ceiling.
    ///
    /// `board x multiplier - baseline`: the one formula, so the running total on
    /// the banner and the figure that reaches the balance are the same
    /// arithmetic rather than two versions of it.
    pub fn standing(&self, data: &GameData) -> i64 {
        let board = evaluate(data, &self.grid, &self.ctx).win_credits;
        (board * self.multiplier / PLAIN - self.baseline).max(0)
    }

    /// Work one beat. Returns the outcome on the beat that ends the round.
    ///
    /// Does nothing at all while the rite is unsettled — the round is waiting on
    /// a player, and a beat that ran anyway would spend the choice for them.
    /// A finished round is inert for the same reason a re-picked chest is
    /// (§5.10): a double input must not be able to spend something.
    pub fn step(&mut self, data: &GameData, rng: &mut SeededRng) -> Option<SeamOutcome> {
        if self.finished {
            return None;
        }
        let kind = self.rite.as_ref()?.kind;

        self.steps_taken += 1;
        self.steps_left = self.steps_left.saturating_sub(1);
        self.changed = match kind {
            RiteKind::Widen { spread_permille } => {
                seam::widen(data, &mut self.grid, &mut self.seam, spread_permille, rng)
            }
            RiteKind::Enrich { rungs } => seam::enrich(data, &mut self.grid, &mut self.seam, rungs),
            RiteKind::Gild { multiply_permille } => {
                // Clamped at a plain multiplier rather than allowed below it: a
                // hand-edited 500 would *halve* the board every beat, and a
                // feature that can take money off a settled spin is a different
                // and much worse thing than one that pays nothing.
                self.multiplier = self.multiplier * multiply_permille.max(PLAIN) / PLAIN;
                seam::gild(&self.seam)
            }
        };

        // A beat that changed nothing has nothing left to change: a widening
        // seam with no frontier is walled in, and an enriched one is on the top
        // rung. Spending the remaining beats redrawing the same board would be
        // three seconds of nothing.
        if self.steps_left == 0 || self.changed.is_empty() {
            self.finished = true;
            return Some(self.outcome(data));
        }
        None
    }

    fn outcome(&self, data: &GameData) -> SeamOutcome {
        let standing = self.standing(data);
        let rite = self.rite.as_ref();
        SeamOutcome {
            credits: standing.min(self.ceiling),
            capped: standing > self.ceiling,
            rite_id: rite.map_or_else(String::new, |rite| rite.id.clone()),
            rite_name: rite.map_or_else(String::new, |rite| rite.name.clone()),
            symbol: self.seam.symbol,
            cells: self.seam.cells.len(),
            steps: self.steps_taken,
            baseline: self.baseline,
        }
    }
}

/// Work a round to its end without a player.
///
/// Used by the headless spin path, the sim and the capture harness. As with the
/// respin round there is no ordering to be honest about — the player never
/// chooses anything, so auto-play *is* the feature.
pub fn auto_play(round: &mut SeamRound, data: &GameData, rng: &mut SeededRng) -> SeamOutcome {
    // Nobody here to ask, so the rite is drawn. This is also what makes the sim
    // measure a *representative* seam rather than whichever rite a test happened
    // to pick — the weights in the JSON are the mix the RTP figure is about.
    round.draw_rite(rng);
    // Bounded on the cell count rather than trusting the beat counter: a
    // hand-edited `steps` of zero is clamped to one, but the bound is what makes
    // it impossible for a future rite to loop here.
    for _ in 0..=round.grid.cell_count() + config_steps(&data.seam) {
        if let Some(outcome) = round.step(data, rng) {
            return outcome;
        }
    }
    round.finished = true;
    round.outcome(data)
}

fn config_steps(config: &SeamConfig) -> usize {
    config.steps.max(1)
}

#[cfg(test)]
mod tests;
