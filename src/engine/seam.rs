//! The Seam (§5.80): a board taken over by one treasure, and what it does next.
//!
//! # What it is
//!
//! When the reels come to rest with enough cells showing the same paying
//! symbol, that symbol is a **seam** running through the hoard, and it is
//! worked out over a few beats before the spin is done with. A seam either
//! **widens** — taking the cells around it — or **deepens**, every cell of it
//! climbing a rung of the pay ladder. The board is then read again, and what
//! the rite added is paid.
//!
//! # Why the trigger is a count and not a full reel
//!
//! The obvious reading of "a whole reel of one symbol" is the stacked reel, and
//! it was the first thing tried. The strips do not carry stacks: across the six
//! cabinets, a window of one symbol occurs on 0 of 200 strip positions on
//! Dragon's Hoard and only for a single low symbol on the four that have any at
//! all. A full-reel trigger would have been a feature the default cabinet could
//! never open.
//!
//! Weaving stacks in would fix that and re-cut every strip in the game — and
//! the strips *are* the RTP (§4). §5.12 made the same call for the same reason
//! when it declined to add a coin symbol for the hold-and-spin round, and reused
//! the egg instead. So a seam is a **count on the grid**, exactly as a clutch of
//! four eggs is, and each cabinet states its own count because six of one symbol
//! is a once-in-a-hundred board on a 5x3 grid and a near-certainty on a 6x5 one.
//!
//! # Nothing here decides when it happens
//!
//! Every function is pure but for the ones handed an rng, and those consume it
//! **as the round runs** rather than at commit — like the respin round (§5.12)
//! and unlike the cascade chain (§5.15). The difference is that there is nothing
//! for the player to do here either, so there is nothing to hide from them.

use crate::data::{GameData, RiteDef, SeamConfig, MAX_RUN};
use crate::engine::reels::Grid;
use macroquad_toolkit::rng::SeededRng;

/// A seam found on a settled grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seam {
    pub symbol: usize,
    /// Flat cells the symbol holds, ascending.
    pub cells: Vec<usize>,
}

/// Can a seam be made of this symbol, or turn a cell into it?
///
/// The three specials are excluded in both directions, and that is the rule the
/// whole feature rests on. A rite that could mint a scatter would award free
/// spins after the spin that awarded them had already settled; one that could
/// mint the hoard symbol would bank eggs the reels never landed and could wake
/// the dragon from inside another feature. Neither is a payout question — both
/// are a rite reaching into a system that has already closed its books.
pub fn seamable(data: &GameData, symbol: usize) -> bool {
    !data.symbols.is_wild(symbol)
        && !data.symbols.is_scatter(symbol)
        && Some(symbol) != data.symbols.hoard()
        && data.symbols.pay(symbol, MAX_RUN) > 0
}

/// Every symbol a seam can be made of, poorest rung first.
///
/// Ordered by what the symbol pays for a full run rather than by the order the
/// symbol set happens to list them in, so "one rung richer" is a claim about the
/// paytable and stays true when a cabinet reprices its symbols. Ties break on
/// index, so the ladder is stable for a given cabinet.
pub fn ladder(data: &GameData) -> Vec<usize> {
    let mut rungs: Vec<usize> = data
        .symbols
        .iter()
        .map(|(index, _)| index)
        .filter(|index| seamable(data, *index))
        .collect();
    rungs.sort_by_key(|index| (data.symbols.pay(*index, MAX_RUN), *index));
    rungs
}

/// The seam on this grid, if any symbol holds enough of it.
///
/// The **widest** seam wins, and the richer symbol breaks a tie. Taking the
/// widest rather than the richest is what keeps the trigger legible: the thing
/// the player noticed is the board being full of one treasure, and that is the
/// one the feature should be about.
pub fn find(data: &GameData, grid: &Grid, config: &SeamConfig) -> Option<Seam> {
    if config.trigger_count == 0 {
        return None;
    }

    let mut best: Option<Seam> = None;
    for (symbol, _) in data.symbols.iter() {
        if !seamable(data, symbol) {
            continue;
        }
        let cells = cells_of(grid, symbol);
        if cells.len() < config.trigger_count {
            continue;
        }
        let better = match best.as_ref() {
            None => true,
            Some(current) => {
                (cells.len(), data.symbols.pay(symbol, MAX_RUN))
                    > (
                        current.cells.len(),
                        data.symbols.pay(current.symbol, MAX_RUN),
                    )
            }
        };
        if better {
            best = Some(Seam { symbol, cells });
        }
    }
    best
}

/// Flat cells holding a symbol, ascending.
fn cells_of(grid: &Grid, symbol: usize) -> Vec<usize> {
    let mut cells: Vec<usize> = grid
        .cells()
        .filter(|(_, _, held)| *held == symbol)
        .map(|(reel, row, _)| grid.index(reel, row))
        .collect();
    cells.sort_unstable();
    cells
}

/// Cells orthogonally touching the seam that a rite is allowed to take.
///
/// Orthogonal only, matching the cluster model (§5.35) — a rite that spread
/// diagonally would take a grid in half the beats and read as a different
/// mechanic on the two cabinets that already pay by adjacency.
pub fn frontier(data: &GameData, grid: &Grid, seam: &Seam) -> Vec<usize> {
    let mut held = vec![false; grid.cell_count()];
    for cell in &seam.cells {
        if let Some(slot) = held.get_mut(*cell) {
            *slot = true;
        }
    }

    let mut frontier = Vec::new();
    for reel in 0..grid.reel_count() {
        for row in 0..grid.rows_on(reel) {
            let flat = grid.index(reel, row);
            if held[flat] || !seamable(data, grid.at(reel, row)) {
                continue;
            }
            if neighbours(grid, reel, row).any(|(r, c)| held[grid.index(r, c)]) {
                frontier.push(flat);
            }
        }
    }
    frontier
}

/// The four cells touching this one, skipping any the ragged grid does not have
/// (§5.20).
fn neighbours(grid: &Grid, reel: usize, row: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
    let left = (reel > 0 && row < grid.rows_on(reel.wrapping_sub(1))).then(|| (reel - 1, row));
    let right =
        (reel + 1 < grid.reel_count() && row < grid.rows_on(reel + 1)).then_some((reel + 1, row));
    let up = (row > 0).then(|| (reel, row - 1));
    let down = (row + 1 < grid.rows_on(reel)).then_some((reel, row + 1));
    [left, right, up, down].into_iter().flatten()
}

/// Take some of the cells touching the seam. Returns the cells that turned.
///
/// Every frontier cell is rolled independently, which is what makes a seam that
/// already spans the grid spread fast and a narrow one spread slowly — the same
/// shape as the respin round's per-cell roll (§5.12), and for the same reason.
pub fn widen(
    data: &GameData,
    grid: &mut Grid,
    seam: &mut Seam,
    spread_permille: usize,
    rng: &mut SeededRng,
) -> Vec<usize> {
    const PERMILLE: usize = 1000;

    let chance = spread_permille.min(PERMILLE);
    let mut taken = Vec::new();
    for flat in frontier(data, grid, seam) {
        if rng.below(PERMILLE) < chance {
            taken.push(flat);
        }
    }

    for reel in 0..grid.reel_count() {
        for row in 0..grid.rows_on(reel) {
            if taken.contains(&grid.index(reel, row)) {
                grid.set(reel, row, seam.symbol);
            }
        }
    }
    seam.cells.extend(taken.iter().copied());
    seam.cells.sort_unstable();
    taken
}

/// Climb the seam up the pay ladder. Returns the cells that changed, which is
/// every cell of the seam or none of them.
///
/// A seam already on the top rung has nowhere to go, and says so by changing
/// nothing — the round reads that as being finished rather than spending its
/// remaining beats redrawing the same board.
pub fn enrich(data: &GameData, grid: &mut Grid, seam: &mut Seam, rungs: usize) -> Vec<usize> {
    let ladder = ladder(data);
    let Some(at) = ladder.iter().position(|rung| *rung == seam.symbol) else {
        return Vec::new();
    };
    let next = (at + rungs.max(1)).min(ladder.len().saturating_sub(1));
    if next == at {
        return Vec::new();
    }

    seam.symbol = ladder[next];
    for reel in 0..grid.reel_count() {
        for row in 0..grid.rows_on(reel) {
            if seam.cells.contains(&grid.index(reel, row)) {
                grid.set(reel, row, seam.symbol);
            }
        }
    }
    seam.cells.clone()
}

/// Gild the seam: change nothing, and report the cells that were stamped.
///
/// A function with no effect on the grid looks like a mistake until you know
/// what it is for. The round's payment is `board x multiplier - baseline`, and
/// gilding raises the multiplier; the cells come back so the beat has something
/// to flash and so the round can tell a rite that did something from one that
/// has run out of board (§5.81).
pub fn gild(seam: &Seam) -> Vec<usize> {
    seam.cells.clone()
}

/// Draw a rite, weighted. `None` only when the list is empty.
pub fn draw_weighted<'a>(rites: &'a [RiteDef], rng: &mut SeededRng) -> Option<&'a RiteDef> {
    let total: u32 = rites.iter().map(|rite| rite.weight).sum();
    if total == 0 {
        return rites.first();
    }

    let mut roll = rng.below(total as usize) as u32;
    for rite in rites {
        if roll < rite.weight {
            return Some(rite);
        }
        roll -= rite.weight;
    }
    rites.last()
}

#[cfg(test)]
mod tests;
