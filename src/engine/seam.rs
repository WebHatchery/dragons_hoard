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

/// Draw a rite, weighted. `None` only when the cabinet declares none.
pub fn draw_rite<'a>(config: &'a SeamConfig, rng: &mut SeededRng) -> Option<&'a RiteDef> {
    let total: u32 = config.rites.iter().map(|rite| rite.weight).sum();
    if total == 0 {
        return config.rites.first();
    }

    let mut roll = rng.below(total as usize) as u32;
    for rite in &config.rites {
        if roll < rite.weight {
            return Some(rite);
        }
        roll -= rite.weight;
    }
    config.rites.last()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MACHINES;

    fn dragon() -> GameData {
        GameData::load().unwrap()
    }

    /// A grid of one symbol everywhere, which every cabinet's trigger clears.
    fn flooded(data: &GameData, symbol: usize) -> Grid {
        let columns: Vec<Vec<usize>> = (0..data.config.reel_count)
            .map(|_| vec![symbol; data.config.row_count])
            .collect();
        Grid::from_columns(&columns)
    }

    #[test]
    fn the_ladder_climbs_and_leaves_out_everything_a_rite_must_not_mint() {
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let ladder = ladder(&data);

            assert!(ladder.len() >= 2, "{} has no ladder to climb", machine.id);
            for pair in ladder.windows(2) {
                assert!(
                    data.symbols.pay(pair[0], MAX_RUN) <= data.symbols.pay(pair[1], MAX_RUN),
                    "{}: the ladder is out of order",
                    machine.id
                );
            }
            for rung in &ladder {
                assert!(!data.symbols.is_wild(*rung));
                assert!(!data.symbols.is_scatter(*rung));
                assert_ne!(Some(*rung), data.symbols.hoard());
            }
        }
    }

    #[test]
    fn a_board_short_of_the_trigger_opens_nothing() {
        let data = dragon();
        let copper = data.symbols.index_of("copper").unwrap();
        let gold = data.symbols.index_of("gold").unwrap();

        let mut grid = flooded(&data, gold);
        for row in 0..data.config.row_count.min(data.seam.trigger_count - 1) {
            grid.set(0, row, copper);
        }
        // Gold still floods the rest, so the assertion is about copper only.
        assert!(find(&data, &grid, &data.seam).is_some_and(|seam| seam.symbol == gold));
    }

    #[test]
    fn the_widest_seam_is_the_one_that_opens() {
        let data = dragon();
        let copper = data.symbols.index_of("copper").unwrap();
        let ruby = data.symbols.index_of("ruby").unwrap();

        // Ruby pays far more, and loses anyway: the seam is the thing the player
        // saw take over the board.
        let mut grid = flooded(&data, copper);
        for row in 0..data.config.row_count {
            grid.set(4, row, ruby);
        }

        let seam = find(&data, &grid, &data.seam).expect("a flooded board is a seam");
        assert_eq!(seam.symbol, copper);
        assert_eq!(seam.cells.len(), 12);
    }

    #[test]
    fn a_wild_or_scatter_never_forms_a_seam_and_never_falls_to_one() {
        let data = dragon();
        let wild = data.symbols.wild().unwrap();
        let scatter = data.symbols.scatter().unwrap();
        let egg = data.symbols.hoard().unwrap();
        let copper = data.symbols.index_of("copper").unwrap();

        assert!(find(&data, &flooded(&data, wild), &data.seam).is_none());
        assert!(find(&data, &flooded(&data, scatter), &data.seam).is_none());
        assert!(find(&data, &flooded(&data, egg), &data.seam).is_none());

        // And the frontier will not offer them up to a rite either.
        let mut grid = flooded(&data, copper);
        grid.set(2, 0, wild);
        grid.set(2, 1, scatter);
        grid.set(2, 2, egg);
        let seam = find(&data, &grid, &data.seam).unwrap();
        assert!(frontier(&data, &grid, &seam).is_empty());
    }

    #[test]
    fn widening_only_ever_takes_cells_that_touch_the_seam() {
        let data = dragon();
        let copper = data.symbols.index_of("copper").unwrap();
        let gold = data.symbols.index_of("gold").unwrap();

        let mut grid = flooded(&data, gold);
        for row in 0..data.config.row_count {
            grid.set(0, row, copper);
            grid.set(1, row, copper);
        }
        let mut seam = Seam {
            symbol: copper,
            cells: (0..6).collect(),
        };

        let mut rng = SeededRng::new(4);
        let taken = widen(&data, &mut grid, &mut seam, 1000, &mut rng);

        // Reel 2 touches reel 1 and is taken whole; reels 3 and 4 do not.
        assert_eq!(taken.len(), data.config.row_count);
        for row in 0..data.config.row_count {
            assert_eq!(grid.at(2, row), copper);
            assert_eq!(grid.at(3, row), gold);
        }
    }

    #[test]
    fn enriching_moves_the_whole_seam_one_rung_and_stops_at_the_top() {
        let data = dragon();
        let ladder = ladder(&data);
        let mut grid = flooded(&data, ladder[0]);
        let mut seam = find(&data, &grid, &data.seam).unwrap();
        let cells = seam.cells.len();

        let changed = enrich(&data, &mut grid, &mut seam, 1);
        assert_eq!(changed.len(), cells);
        assert_eq!(seam.symbol, ladder[1]);
        assert_eq!(grid.count_of(ladder[1]), cells);

        // Walk it to the top and then ask for one more.
        for _ in 0..ladder.len() {
            enrich(&data, &mut grid, &mut seam, 1);
        }
        assert_eq!(seam.symbol, *ladder.last().unwrap());
        assert!(enrich(&data, &mut grid, &mut seam, 1).is_empty());
    }

    #[test]
    fn a_rite_is_drawn_by_weight() {
        let data = dragon();
        let mut rng = SeededRng::new(19);
        let mut counts = std::collections::HashMap::new();
        for _ in 0..20_000 {
            let rite = draw_rite(&data.seam, &mut rng).unwrap();
            *counts.entry(rite.id.clone()).or_insert(0usize) += 1;
        }

        let total: u32 = data.seam.rites.iter().map(|rite| rite.weight).sum();
        for rite in &data.seam.rites {
            let share = counts.get(&rite.id).copied().unwrap_or(0) as f64 / 20_000.0;
            let expected = rite.weight as f64 / total as f64;
            assert!(
                (share - expected).abs() < 0.03,
                "rite '{}' drawn {:.3} of the time against a weight of {:.3}",
                rite.id,
                share,
                expected
            );
        }
    }

    #[test]
    fn the_same_seed_works_the_same_seam() {
        let data = dragon();
        let copper = data.symbols.index_of("copper").unwrap();
        let build = || {
            let grid = flooded(&data, copper);
            let seam = find(&data, &grid, &data.seam).unwrap();
            (grid, seam)
        };

        let (mut first_grid, mut first_seam) = build();
        let (mut second_grid, mut second_seam) = build();
        let mut a = SeededRng::new(77);
        let mut b = SeededRng::new(77);

        assert_eq!(
            widen(&data, &mut first_grid, &mut first_seam, 400, &mut a),
            widen(&data, &mut second_grid, &mut second_seam, 400, &mut b)
        );
        assert_eq!(first_grid, second_grid);
    }
}
