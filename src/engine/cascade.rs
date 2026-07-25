//! Cascading reels (§5.15).
//!
//! # A spin becomes a sequence
//!
//! On a cascading cabinet a win is not the end of a spin. The winning symbols
//! are cleared, everything above them falls into the gap, fresh symbols drop in
//! from above, and the new grid is evaluated again. As long as each grid pays,
//! the chain continues — and a multiplier ladder climbs with it, so the fourth
//! drop in a row is worth several times the first.
//!
//! # The whole chain is decided at commit
//!
//! §8.2's invariant is that the animation only ever *reveals* a decision already
//! made. A cascade could easily have broken that: the obvious implementation
//! draws fresh symbols from the RNG each time a grid clears, which would make
//! the reveal consume randomness and put the animated and headless paths on
//! different streams.
//!
//! So refills are **not** rolled. Each reel keeps reading *up* its own strip
//! from where it stopped, exactly as a physical cascade would show — the symbols
//! that drop in are the ones that were already above the window. The entire
//! chain is therefore a function of the stop indices alone, which are picked
//! once, at commit. A test spins the whole sequence twice from one seed and
//! asserts every step matches.
//!
//! # Features are decided on the landing grid
//!
//! Scatters, free-spin triggers and eggs are read from the first grid only;
//! only *payouts* cascade. A retrigger that could arrive on a refill would make
//! the free-spin count depend on how long a chain ran, and the Dragon's Wrath
//! (§5.12) would open from symbols the reels never actually landed. Keeping
//! features on the landing grid leaves every other system in the game reading
//! exactly what it read before.

use crate::data::{CascadeConfig, GameData};
use crate::engine::evaluate::{evaluate, EvalContext, SpinOutcome};
use crate::engine::reels::Grid;

/// One grid in a cascade chain, and what it paid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadeStep {
    /// The grid as evaluated at this step.
    pub grid: Grid,
    /// Cells cleared by this step's wins, flat `reel * rows + row`. Empty on the
    /// final step, which by definition did not pay.
    pub cleared: Vec<usize>,
    /// Multiplier in force, from the ladder.
    pub multiplier: i64,
    /// Credits this step paid, already multiplied.
    pub credits: i64,
    pub outcome: SpinOutcome,
}

/// Resolve a whole chain from a landing grid.
///
/// Step 0 is always present and holds the landing grid, so a cabinet with
/// cascades disabled and one with a chain that paid nothing produce the same
/// shape. `bound` caps the chain against a pathological data set — a strip that
/// refilled into a win every time would otherwise never terminate.
pub fn resolve(
    data: &GameData,
    config: &CascadeConfig,
    landed: &Grid,
    stops: &[usize],
    ctx: &EvalContext,
) -> Vec<CascadeStep> {
    let mut steps = Vec::new();
    let mut grid = landed.clone();
    // How far up each reel's strip has been consumed. The window already shows
    // `row_count` symbols, so refills start immediately above them.
    let mut consumed: Vec<usize> = vec![0; grid.reel_count()];

    for step in 0..config.max_steps.max(1) {
        let multiplier = config.multiplier_at(step);
        let outcome = evaluate(data, &grid, ctx);
        let credits = outcome.win_credits * multiplier;

        let cleared = if step + 1 < config.max_steps.max(1) {
            cleared_cells(&grid, &outcome)
        } else {
            // The last permitted step cannot cascade further, so it clears
            // nothing — otherwise the chain would end showing holes.
            Vec::new()
        };

        let finished = cleared.is_empty();
        steps.push(CascadeStep {
            grid: grid.clone(),
            cleared: cleared.clone(),
            multiplier,
            credits,
            outcome,
        });
        if finished {
            break;
        }

        grid = collapse(data, &grid, &cleared, stops, &mut consumed);
    }

    steps
}

/// Union of the cells every win on this grid used.
fn cleared_cells(grid: &Grid, outcome: &SpinOutcome) -> Vec<usize> {
    let mut mask = vec![false; grid.reel_count() * grid.row_count()];
    for win in &outcome.wins {
        for cell in &win.cells {
            if let Some(lit) = mask.get_mut(*cell) {
                *lit = true;
            }
        }
    }
    // Scatter pays do not clear. A scatter that vanished would change the
    // trigger count read from the landing grid after the fact.
    mask.iter()
        .enumerate()
        .filter(|(_, lit)| **lit)
        .map(|(cell, _)| cell)
        .collect()
}

/// Drop survivors down and refill from above.
fn collapse(
    data: &GameData,
    grid: &Grid,
    cleared: &[usize],
    stops: &[usize],
    consumed: &mut [usize],
) -> Grid {
    let rows = grid.row_count();
    let mut columns: Vec<Vec<usize>> = Vec::with_capacity(grid.reel_count());

    for (reel, used) in consumed.iter_mut().enumerate().take(grid.reel_count()) {
        // Survivors, top to bottom, keeping their order — they fall, they do
        // not shuffle.
        let mut column: Vec<usize> = (0..rows)
            .filter(|row| !cleared.contains(&(reel * rows + row)))
            .map(|row| grid.at(reel, row))
            .collect();

        let missing = rows - column.len();
        let strip = &data.reels[reel];
        let stop = stops.get(reel).copied().unwrap_or(0);

        // Refill from the strip *above* the window, one symbol further up each
        // time. No RNG: the chain has to stay a function of the stops (§8.2).
        let mut fresh = Vec::with_capacity(missing);
        for _ in 0..missing {
            *used += 1;
            let index = (stop as i64 - *used as i64).rem_euclid(strip.len() as i64);
            fresh.push(strip[index as usize]);
        }
        // The newest symbol sits highest, so the run reads as a stack falling in.
        fresh.reverse();
        fresh.append(&mut column);
        columns.push(fresh);
    }

    Grid::from_columns(&columns)
}

/// Total credits a chain paid.
pub fn total_credits(steps: &[CascadeStep]) -> i64 {
    steps.iter().map(|step| step.credits).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{GameData, MACHINES};
    use crate::engine::reels::{grid_from_stops, pick_stops};
    use macroquad_toolkit::rng::SeededRng;

    fn data() -> GameData {
        GameData::load_machine(
            MACHINES
                .iter()
                .find(|machine| machine.id == "avalanche")
                .expect("no cascading machine in the catalog"),
        )
        .unwrap()
    }

    fn chain(data: &GameData, seed: u64) -> Vec<CascadeStep> {
        let mut rng = SeededRng::new(seed);
        let stops = pick_stops(data, &mut rng);
        let landed = grid_from_stops(data, &stops);
        let config = data.cascade.as_ref().unwrap();
        resolve(data, config, &landed, &stops, &EvalContext::base(data, 10))
    }

    #[test]
    fn a_chain_always_has_at_least_the_landing_grid() {
        let data = data();
        for seed in 0..40 {
            let steps = chain(&data, seed);
            assert!(!steps.is_empty(), "seed {} produced no steps", seed);
        }
    }

    #[test]
    fn the_chain_ends_on_a_grid_that_cleared_nothing() {
        // A chain that stopped with cells still marked would leave holes on
        // screen, and the player would be looking at a board mid-collapse.
        let data = data();
        for seed in 0..60 {
            let steps = chain(&data, seed);
            assert!(
                steps.last().unwrap().cleared.is_empty(),
                "seed {} ended mid-collapse",
                seed
            );
        }
    }

    #[test]
    fn every_step_but_the_last_actually_paid() {
        // The chain continues *because* a grid paid. A step that cleared cells
        // without paying would be a cascade the player was never rewarded for.
        let data = data();
        for seed in 0..60 {
            let steps = chain(&data, seed);
            for step in steps.iter().take(steps.len().saturating_sub(1)) {
                assert!(!step.cleared.is_empty());
                assert!(step.credits > 0, "seed {} cascaded without paying", seed);
            }
        }
    }

    #[test]
    fn the_multiplier_climbs_with_the_chain() {
        let data = data();
        let config = data.cascade.as_ref().unwrap();

        for seed in 0..200 {
            let steps = chain(&data, seed);
            if steps.len() < 2 {
                continue;
            }
            for (index, step) in steps.iter().enumerate() {
                assert_eq!(step.multiplier, config.multiplier_at(index));
            }
            assert!(steps[1].multiplier >= steps[0].multiplier);
            return;
        }
        panic!("no seed in 200 produced a cascade");
    }

    #[test]
    fn a_chain_is_a_function_of_the_stops_alone() {
        // The invariant the whole design rests on (§8.2): the reveal must not
        // consume randomness, or the animated and headless paths would diverge.
        let data = data();
        for seed in 0..30 {
            let first = chain(&data, seed);
            let second = chain(&data, seed);
            assert_eq!(first, second, "seed {} did not replay", seed);
        }
    }

    #[test]
    fn survivors_fall_and_keep_their_order() {
        // A collapse that reshuffled the column would be inventing symbols the
        // reels never landed.
        let data = data();
        let mut rng = SeededRng::new(7);
        let stops = pick_stops(&data, &mut rng);
        let landed = grid_from_stops(&data, &stops);
        let rows = landed.row_count();

        // Clear the *bottom* cell of reel 0. Clearing the top would prove
        // nothing — the hole is already above the survivors, so nothing falls.
        let mut consumed = vec![0; landed.reel_count()];
        let after = collapse(&data, &landed, &[rows - 1], &stops, &mut consumed);

        for row in 1..rows {
            assert_eq!(
                after.at(0, row),
                landed.at(0, row - 1),
                "row {} did not fall by one",
                row
            );
        }
        for reel in 1..landed.reel_count() {
            for row in 0..rows {
                assert_eq!(
                    after.at(reel, row),
                    landed.at(reel, row),
                    "untouched reel moved"
                );
            }
        }
    }

    #[test]
    fn a_refill_comes_from_the_strip_above_the_stop() {
        let data = data();
        let mut rng = SeededRng::new(11);
        let stops = pick_stops(&data, &mut rng);
        let landed = grid_from_stops(&data, &stops);

        let rows = landed.row_count();
        let mut consumed = vec![0; landed.reel_count()];
        let after = collapse(&data, &landed, &[rows - 1], &stops, &mut consumed);

        let strip = &data.reels[0];
        let expected = strip[(stops[0] as i64 - 1).rem_euclid(strip.len() as i64) as usize];
        assert_eq!(after.at(0, 0), expected);
        assert_eq!(consumed[0], 1, "the strip should have advanced by one");
    }

    #[test]
    fn a_chain_cannot_run_forever() {
        // Guards a hand-edited strip that refills into a win every time.
        let data = data();
        let config = data.cascade.as_ref().unwrap();
        for seed in 0..300 {
            assert!(chain(&data, seed).len() <= config.max_steps.max(1));
        }
    }

    #[test]
    fn total_credits_is_the_sum_of_the_steps() {
        let data = data();
        for seed in 0..40 {
            let steps = chain(&data, seed);
            let expected: i64 = steps.iter().map(|step| step.credits).sum();
            assert_eq!(total_credits(&steps), expected);
        }
    }

    #[test]
    fn a_step_pays_its_wins_times_its_multiplier() {
        let data = data();
        for seed in 0..200 {
            for step in chain(&data, seed) {
                assert_eq!(step.credits, step.outcome.win_credits * step.multiplier);
            }
        }
    }
}
