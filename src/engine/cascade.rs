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
    let mut mask = vec![false; grid.cell_count()];
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
pub fn collapse(
    data: &GameData,
    grid: &Grid,
    cleared: &[usize],
    stops: &[usize],
    consumed: &mut [usize],
) -> Grid {
    let mut columns: Vec<Vec<usize>> = Vec::with_capacity(grid.reel_count());

    for (reel, used) in consumed.iter_mut().enumerate().take(grid.reel_count()) {
        // Survivors, top to bottom, keeping their order — they fall, they do
        // not shuffle.
        let rows = grid.rows_on(reel);
        let mut column: Vec<usize> = (0..rows)
            .filter(|row| !cleared.contains(&grid.index(reel, *row)))
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

// Tests live in the crate-level integration harness.
