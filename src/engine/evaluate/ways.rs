//! Ways-to-win evaluation (§5.14).
//!
//! # What a "way" is
//!
//! A payline machine asks: does this *one path* through the grid read as five
//! chests? A ways machine asks: does **any** path? A symbol pays if it appears
//! at least once on each of reels 1..n, and the win is multiplied by how many
//! distinct paths there are — the product of its per-reel counts. Two chests on
//! reel 1, one on reel 2 and three on reel 3 is `2 × 1 × 3 = 6` ways, all paid.
//!
//! On a 5×3 grid that is `3⁵ = 243` ways, and every one of them is always
//! active. There is no line to be off, which is the whole appeal: a win is a
//! property of the grid rather than of a path drawn across it.
//!
//! # Every symbol pays, except an all-wild run
//!
//! This is the rule that separates ways from lines, and it is a deliberate
//! design decision rather than an inherited one. On a payline, the run is one
//! contest and `best_line_result` picks the single best reading of it. In a ways
//! game symbols genuinely do pay *alongside* each other — a gem run and a chest
//! run on the same grid are two different sets of paths and both are real wins.
//! That is the mechanic.
//!
//! The one thing that must not happen is the **same cells paying twice under two
//! names**, and there is exactly one way for that to arise: a run made entirely
//! of wilds reads as every symbol at once. So a symbol's run is suppressed when
//! no genuine copy of it appears anywhere in the run — the wild pays for those
//! cells itself, once.
//!
//! `W W W C C` therefore pays *both* a three-wild run and a five-chest run: the
//! chest run has real chests on reels 4 and 5, so it is not the wild's win being
//! counted twice. `W W W` pays only as wild. Whether that combination is
//! generous is a tuning question the sim answers (§4), not a correctness one.

use super::{Win, WinSource};
use crate::data::GameData;
use crate::engine::reels::Grid;

/// Minimum run length that can pay, matching the payline evaluator.
const MIN_RUN: usize = 3;

/// Every paying symbol on the grid, with its ways count and the cells that
/// formed it.
pub fn wins(data: &GameData, grid: &Grid, line_bet: i64, multiplier: i64) -> Vec<Win> {
    let mut found = Vec::new();

    for (candidate, _) in data.symbols.iter() {
        if data.symbols.is_scatter(candidate) {
            continue;
        }
        let Some(win) = symbol_win(data, grid, candidate, line_bet, multiplier) else {
            continue;
        };
        found.push(win);
    }

    found
}

/// One symbol's ways run, or `None` if it does not pay.
fn symbol_win(
    data: &GameData,
    grid: &Grid,
    candidate: usize,
    line_bet: i64,
    multiplier: i64,
) -> Option<Win> {
    let is_wild_candidate = data.symbols.is_wild(candidate);

    let mut ways = 1usize;
    let mut run = 0usize;
    let mut cells: Vec<usize> = Vec::new();
    // Tracked so an all-wild run can be suppressed: it is the wild's win, and
    // paying it under another name as well would pay the same cells twice.
    let mut genuine = 0usize;

    for reel in 0..grid.reel_count() {
        let mut matches = 0usize;
        let mut reel_cells = Vec::new();

        for row in 0..grid.rows_on(reel) {
            let symbol = grid.at(reel, row);
            // A wild candidate matches only real wilds — it does not substitute
            // for itself, which would make every run infinite.
            let hit = symbol == candidate || (!is_wild_candidate && data.symbols.is_wild(symbol));
            if hit {
                matches += 1;
                reel_cells.push(grid.index(reel, row));
                if symbol == candidate {
                    genuine += 1;
                }
            }
        }

        // The run must be unbroken from reel 1, exactly as a payline run is.
        if matches == 0 {
            break;
        }
        ways *= matches;
        run += 1;
        cells.extend(reel_cells);
    }

    if run < MIN_RUN || genuine == 0 {
        return None;
    }
    let pay = data.symbols.pay(candidate, run);
    if pay <= 0 {
        return None;
    }

    Some(Win {
        source: WinSource::Ways(ways),
        symbol: candidate,
        count: run,
        credits: pay * ways as i64 * line_bet * multiplier,
        cells,
    })
}

#[cfg(test)]
mod tests;
