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
mod tests {
    use super::*;
    use crate::data::MACHINES;
    use crate::engine::seam::ladder;

    fn dragon() -> GameData {
        GameData::load().unwrap()
    }

    fn flooded(data: &GameData, symbol: usize) -> Grid {
        let columns: Vec<Vec<usize>> = (0..data.config.reel_count)
            .map(|_| vec![symbol; data.config.row_count])
            .collect();
        Grid::from_columns(&columns)
    }

    fn round_on(data: &GameData, grid: &Grid, _rng: &mut SeededRng) -> SeamRound {
        let seam = seam::find(data, grid, &data.seam).expect("no seam on this board");
        SeamRound::open(data, grid, seam, EvalContext::base(data, 10)).unwrap()
    }

    /// A round with one rite forced, for the tests that are about that rite
    /// rather than about the mix.
    fn round_running(data: &GameData, grid: &Grid, kind: RiteKind) -> SeamRound {
        let seam = seam::find(data, grid, &data.seam).expect("no seam on this board");
        let mut round = SeamRound::open(data, grid, seam, EvalContext::base(data, 10)).unwrap();
        round.rite = Some(RiteDef {
            id: "forced".to_owned(),
            name: "Forced".to_owned(),
            weight: 1,
            kind,
        });
        round
    }

    #[test]
    fn a_round_never_pays_for_the_board_the_spin_already_paid_for() {
        // A grid that is already one symbol end to end pays a great deal, and
        // all of it belongs to the spin. A rite that only widens has nothing
        // left to take, so the uplift is zero rather than the whole board again.
        let data = dragon();
        let copper = data.symbols.index_of("copper").unwrap();
        let grid = flooded(&data, copper);
        let mut rng = SeededRng::new(3);

        let mut round = round_on(&data, &grid, &mut rng);
        let baseline = round.baseline;
        assert!(baseline > 0, "a flooded board should pay something");

        let outcome = auto_play(&mut round, &data, &mut rng);
        if outcome.rite_id == "widen" {
            assert_eq!(outcome.credits, 0);
        }
    }

    #[test]
    fn an_enrichment_is_worth_the_climb_and_nothing_else() {
        let data = dragon();
        let rungs = ladder(&data);
        let grid = flooded(&data, rungs[0]);
        let ctx = EvalContext::base(&data, 10);

        let mut rng = SeededRng::new(5);
        let seam = seam::find(&data, &grid, &data.seam).unwrap();
        let mut round = SeamRound::open(&data, &grid, seam, ctx).unwrap();
        round.rite = Some(RiteDef {
            id: "enrich".to_owned(),
            name: "test".to_owned(),
            weight: 1,
            kind: RiteKind::Enrich { rungs: 1 },
        });

        let outcome = auto_play(&mut round, &data, &mut rng);

        // Every rung above the bottom one, until the ladder runs out or the
        // ceiling does.
        assert!(outcome.credits > 0);
        assert_eq!(outcome.symbol, rungs[outcome.steps.min(rungs.len() - 1)]);
    }

    #[test]
    fn a_rite_can_never_mint_a_wild_a_scatter_or_an_egg() {
        // The rule the whole feature rests on, asserted on every cabinet against
        // a board the rites are given every chance to take.
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let rungs = ladder(&data);
            let wild = data.symbols.wild().unwrap();
            let scatter = data.symbols.scatter().unwrap();
            let hoard = data.symbols.hoard();

            for seed in 0..40u64 {
                let mut rng = SeededRng::new(seed);
                let mut grid = flooded(&data, rungs[0]);
                // Salt the board with the three symbols a rite must not touch.
                grid.set(0, 0, wild);
                grid.set(1, 0, scatter);
                if let Some(egg) = hoard {
                    grid.set(2, 0, egg);
                }
                let before = (
                    grid.count_of(wild),
                    grid.count_of(scatter),
                    hoard.map(|egg| grid.count_of(egg)),
                );

                let mut round = round_on(&data, &grid, &mut rng);
                auto_play(&mut round, &data, &mut rng);
                let after = (
                    round.grid().count_of(wild),
                    round.grid().count_of(scatter),
                    hoard.map(|egg| round.grid().count_of(egg)),
                );

                assert_eq!(
                    before, after,
                    "{} seed {} moved a special",
                    machine.id, seed
                );
            }
        }
    }

    #[test]
    fn the_ceiling_holds_on_every_cabinet() {
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let rungs = ladder(&data);
            let ceiling = data.total_bet(10) * data.seam.max_multiple;

            for seed in 0..30u64 {
                let mut rng = SeededRng::new(seed);
                let grid = flooded(&data, rungs[0]);
                let mut round = round_on(&data, &grid, &mut rng);
                let outcome = auto_play(&mut round, &data, &mut rng);
                assert!(
                    outcome.credits <= ceiling,
                    "{} paid {} over a ceiling of {}",
                    machine.id,
                    outcome.credits,
                    ceiling
                );
            }
        }
    }

    #[test]
    fn a_seam_opens_unchosen_and_will_not_move_until_a_rite_is_taken() {
        let data = dragon();
        let rungs = ladder(&data);
        let grid = flooded(&data, rungs[0]);
        let mut rng = SeededRng::new(11);

        let mut round = round_on(&data, &grid, &mut rng);
        assert_eq!(round.offered().len(), data.seam.rites.len());

        // Stepping an unchosen round is a no-op, not a beat: the board would
        // otherwise spend the decision the player has not made yet.
        for _ in 0..10 {
            assert!(round.step(&data, &mut rng).is_none());
        }
        assert_eq!(round.steps_left(), data.seam.steps.max(1));

        assert!(round.choose(0));
        assert!(
            round.offered().is_empty(),
            "the deal must not be re-cuttable"
        );
        assert!(!round.choose(1), "a second press cannot change the rite");
        assert_eq!(round.rite().map(|rite| rite.id.as_str()), Some("widen"));
    }

    #[test]
    fn a_gilding_changes_no_symbol_and_multiplies_what_the_board_already_pays() {
        let data = dragon();
        let rungs = ladder(&data);
        // A flooded board pays a great deal, all of it the spin's. A gilding
        // takes a multiple of exactly that.
        let grid = flooded(&data, rungs[0]);
        let mut rng = SeededRng::new(13);

        let mut round = round_running(
            &data,
            &grid,
            RiteKind::Gild {
                multiply_permille: 2_000,
            },
        );
        let baseline = round.baseline;
        let outcome = auto_play(&mut round, &data, &mut rng);

        assert_eq!(round.grid(), &grid, "a gilding must not move a symbol");
        // Two beats at x2 is x4, so the round pays three times the board.
        let doublings = data.seam.steps.max(1) as u32;
        let expected = baseline * (2i64.pow(doublings) - 1);
        assert_eq!(outcome.credits, expected.min(round.ceiling));
    }

    #[test]
    fn a_gilding_pays_nothing_on_a_board_that_was_paying_nothing() {
        // The whole reason the choice is a decision rather than a preference
        // (§5.81): a multiple of nothing is nothing, and the player can see
        // which board they have before they pick.
        let data = dragon();
        let rungs = ladder(&data);
        let mut grid = flooded(&data, rungs[0]);
        // Break every line by alternating two symbols down each reel, keeping
        // enough of the first to still be a seam.
        for reel in 0..data.config.reel_count {
            if reel % 2 == 1 {
                for row in 0..data.config.row_count {
                    grid.set(reel, row, rungs[1]);
                }
            }
        }

        let mut rng = SeededRng::new(17);
        let mut round = round_running(
            &data,
            &grid,
            RiteKind::Gild {
                multiply_permille: 3_000,
            },
        );
        assert_eq!(round.baseline, 0, "this board was supposed to pay nothing");

        let outcome = auto_play(&mut round, &data, &mut rng);
        assert_eq!(outcome.credits, 0);
    }

    #[test]
    fn a_round_that_has_finished_cannot_be_stepped_again() {
        let data = dragon();
        let rungs = ladder(&data);
        let grid = flooded(&data, rungs[0]);
        let mut rng = SeededRng::new(9);

        let mut round = round_on(&data, &grid, &mut rng);
        auto_play(&mut round, &data, &mut rng);

        assert!(round.step(&data, &mut rng).is_none());
    }

    #[test]
    fn a_bigger_stake_pays_proportionally_more() {
        let data = dragon();
        let rungs = ladder(&data);
        let grid = flooded(&data, rungs[0]);
        let seam = seam::find(&data, &grid, &data.seam).unwrap();

        let mut small_rng = SeededRng::new(21);
        let mut large_rng = SeededRng::new(21);
        let mut small =
            SeamRound::open(&data, &grid, seam.clone(), EvalContext::base(&data, 1)).unwrap();
        let mut large = SeamRound::open(&data, &grid, seam, EvalContext::base(&data, 10)).unwrap();

        let small_outcome = auto_play(&mut small, &data, &mut small_rng);
        let large_outcome = auto_play(&mut large, &data, &mut large_rng);

        assert_eq!(large_outcome.cells, small_outcome.cells);
        assert_eq!(large_outcome.credits, small_outcome.credits * 10);
    }
}
