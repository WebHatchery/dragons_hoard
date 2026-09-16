//! Cluster pays: wins that ignore the reels entirely (§5.35).
//!
//! # A fourth thing a win can be
//!
//! The three models so far all read the grid **reel by reel, left to right**.
//! Lines walk a fixed path across it (§3), ways pay every route through it
//! (§5.14), and a shifting cabinet changes how tall each column is (§5.20) — but
//! all three inherit the same assumption from a physical machine, that a reel is
//! a thing and order along it matters.
//!
//! A cluster does not care. A win is a **connected group of the same symbol**,
//! orthogonally adjacent, anywhere on the grid, and it pays on how many cells
//! are in it. A blob in the top-left corner spanning three columns is a win; the
//! same eight symbols scattered evenly are nothing. Reel one is not special, and
//! neither is direction.
//!
//! That makes it the first model where **the shape of the grid matters more than
//! its columns**, and the reason it belongs beside cascades (§5.15): removing a
//! cluster drops symbols into a hole with edges, which is what makes the next
//! grid genuinely different rather than a fresh deal.
//!
//! # Flood fill, with wilds
//!
//! Each unvisited cell seeds a fill over its four neighbours. Wilds join any
//! cluster they touch, which is the one rule that needs care: **a wild may
//! belong to several clusters at once** — it is one cell that reads as a gem to
//! the gems beside it and as a coin to the coins — but it must never be what
//! *starts* one, or a run of adjacent wilds would pay as a cluster of nothing.
//!
//! Ordinary cells are consumed. A given gem belongs to exactly one gem cluster,
//! because a cluster is a connected component and connected components do not
//! overlap. So the payout cannot be double-counted, which is the fault this
//! model invites and the one the tests spend most of their effort on.

use crate::data::GameData;
use crate::engine::evaluate::{Win, WinSource};
use crate::engine::reels::Grid;

/// Smallest group that pays anything.
///
/// Below five a cluster is not a shape, it is a coincidence: on a 6x5 grid with
/// eight symbols, groups of three form on most spins and paying them would make
/// the model a worse version of ways.
pub const MIN_CLUSTER: usize = 5;

/// Every paying cluster on the grid.
///
/// Returned in discovery order — top-left first, scanning columns — so a
/// rendering that highlights them does so in a stable sequence.
pub fn wins(data: &GameData, grid: &Grid, line_bet: i64, multiplier: i64) -> Vec<Win> {
    let wild = data.symbols.wild();
    let scatter = data.symbols.scatter();

    let mut seen = vec![false; grid.cell_count()];
    let mut wins = Vec::new();

    for reel in 0..grid.reel_count() {
        for row in 0..grid.rows_on(reel) {
            let flat = grid.index(reel, row);
            if seen[flat] {
                continue;
            }
            let symbol = grid.at(reel, row);

            // A wild never seeds a cluster: a group of adjacent wilds would
            // otherwise pay as a cluster of nothing. It is picked up by whatever
            // real symbol it is touching instead.
            if Some(symbol) == wild {
                continue;
            }
            // The scatter pays from anywhere and is not a cluster symbol; making
            // it one would pay it twice.
            if Some(symbol) == scatter {
                seen[flat] = true;
                continue;
            }

            let cells = fill(grid, reel, row, symbol, wild, &mut seen);
            if cells.len() < MIN_CLUSTER {
                continue;
            }

            let credits = pay(data, symbol, cells.len()) * line_bet * multiplier;
            if credits <= 0 {
                continue;
            }
            wins.push(Win {
                source: WinSource::Cluster(cells.len()),
                symbol,
                count: cells.len(),
                credits,
                cells,
            });
        }
    }
    wins
}

/// Collect the connected group containing `(reel, row)`.
///
/// Iterative rather than recursive: a grid that is one symbol end to end is a
/// perfectly ordinary spin, and thirty stack frames deep is not where a slot
/// machine should be discovering its limits.
fn fill(
    grid: &Grid,
    reel: usize,
    row: usize,
    symbol: usize,
    wild: Option<usize>,
    seen: &mut [bool],
) -> Vec<usize> {
    let mut cells = Vec::new();
    // Wilds are marked separately: one wild can belong to several clusters, so
    // consuming it in `seen` would give it to whichever was found first.
    let mut local = vec![false; grid.cell_count()];
    let mut stack = vec![(reel, row)];

    while let Some((r, c)) = stack.pop() {
        let flat = grid.index(r, c);
        if local[flat] {
            continue;
        }
        let here = grid.at(r, c);
        let is_wild = Some(here) == wild;
        if here != symbol && !is_wild {
            continue;
        }

        local[flat] = true;
        cells.push(flat);
        if !is_wild {
            seen[flat] = true;
        }

        if r > 0 && c < grid.rows_on(r - 1) {
            stack.push((r - 1, c));
        }
        if r + 1 < grid.reel_count() && c < grid.rows_on(r + 1) {
            stack.push((r + 1, c));
        }
        if c > 0 {
            stack.push((r, c - 1));
        }
        if c + 1 < grid.rows_on(r) {
            stack.push((r, c + 1));
        }
    }

    cells.sort_unstable();
    cells
}

/// What a cluster of `size` pays, as a multiple of the line bet.
///
/// The paytable is indexed by run length and stops at five (§3), which is the
/// right shape for a line and far too short for a cluster that can reach thirty.
/// So the top rung is scaled by how far past it the cluster went — the growth is
/// deliberately steep, because a fifteen-cell cluster should feel like a
/// different event from a five-cell one rather than three times one.
pub fn pay(data: &GameData, symbol: usize, size: usize) -> i64 {
    const TOP: usize = 5;
    let base = data.symbols.pay(symbol, size.min(TOP));
    if size <= TOP {
        return base;
    }
    let over = (size - TOP) as i64;
    // Doubling every four cells past the top rung, integer-only so the sim and
    // the game cannot disagree by a rounding.
    base * (4 + over * 3) / 4
}

// Tests live in the crate-level integration harness.
