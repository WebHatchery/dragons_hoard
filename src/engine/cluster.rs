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
fn pay(data: &GameData, symbol: usize, size: usize) -> i64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{GameData, MACHINES};

    fn cluster_machine() -> Option<GameData> {
        MACHINES
            .iter()
            .map(|machine| GameData::load_machine(machine).unwrap())
            .find(|data| data.config.evaluation == crate::data::Evaluation::Cluster)
    }

    /// Two symbols that pay but are neither wild nor scatter.
    const A: usize = 0;
    const B: usize = 1;

    /// A grid of nothing, to stamp shapes onto.
    ///
    /// A checkerboard of two other symbols, which by construction has no two
    /// alike orthogonally adjacent — so the background can never form a cluster
    /// of its own and every win a test sees is one it drew.
    fn field(data: &GameData) -> Grid {
        let reels = data.config.reel_count;
        let rows = data.config.row_count;
        Grid::from_columns(
            &(0..reels)
                .map(|reel| {
                    (0..rows)
                        .map(|row| if (reel + row) % 2 == 0 { 3 } else { 4 })
                        .collect()
                })
                .collect::<Vec<Vec<usize>>>(),
        )
    }

    /// Stamp cells onto a field. Coordinates are `(reel, row)`.
    fn stamp(grid: &mut Grid, cells: &[(usize, usize)], symbol: usize) {
        for (reel, row) in cells {
            grid.set(*reel, *row, symbol);
        }
    }

    #[test]
    fn a_group_below_the_minimum_pays_nothing() {
        let Some(data) = cluster_machine() else {
            return;
        };
        // Four in a square. One short.
        let mut grid = field(&data);
        stamp(&mut grid, &[(0, 0), (0, 1), (1, 0), (1, 1)], A);
        assert!(wins(&data, &grid, 10, 1).is_empty());
    }

    #[test]
    fn a_connected_group_at_the_minimum_pays() {
        let Some(data) = cluster_machine() else {
            return;
        };
        let mut grid = field(&data);
        stamp(&mut grid, &[(0, 0), (0, 1), (1, 0), (1, 1), (2, 0)], A);
        let found = wins(&data, &grid, 10, 1);
        assert_eq!(found.len(), 1, "{:?}", found);
        assert_eq!(found[0].count, 5);
    }

    #[test]
    fn adjacency_is_orthogonal_only() {
        let Some(data) = cluster_machine() else {
            return;
        };
        // A diagonal chain of five. Touching at the corners is not touching.
        let mut grid = field(&data);
        stamp(&mut grid, &[(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)], A);
        assert!(wins(&data, &grid, 10, 1).is_empty());
    }

    #[test]
    fn no_cell_is_ever_paid_in_two_clusters() {
        // The fault this model invites. Connected components cannot overlap, so
        // every ordinary cell belongs to exactly one win.
        let Some(data) = cluster_machine() else {
            return;
        };
        let wild = data.symbols.wild();

        for seed in 0..400u64 {
            let grid = crate::engine::reels::grid_from_stops(
                &data,
                &crate::engine::reels::pick_stops(
                    &data,
                    &mut macroquad_toolkit::rng::SeededRng::new(seed),
                ),
            );
            let found = wins(&data, &grid, 10, 1);

            let mut claimed: Vec<usize> = Vec::new();
            for win in &found {
                for cell in &win.cells {
                    let (reel, row) = flat_to_cell(&grid, *cell);
                    // A wild may legitimately appear in several clusters.
                    if Some(grid.at(reel, row)) == wild {
                        continue;
                    }
                    assert!(!claimed.contains(cell), "cell {} paid twice", cell);
                    claimed.push(*cell);
                }
            }
        }
    }

    fn flat_to_cell(grid: &Grid, flat: usize) -> (usize, usize) {
        for reel in 0..grid.reel_count() {
            for row in 0..grid.rows_on(reel) {
                if grid.index(reel, row) == flat {
                    return (reel, row);
                }
            }
        }
        unreachable!()
    }

    #[test]
    fn a_wild_joins_the_cluster_beside_it() {
        let Some(data) = cluster_machine() else {
            return;
        };
        let Some(wild) = data.symbols.wild() else {
            return;
        };
        // Four of a symbol and a wild completing them.
        let mut grid = field(&data);
        stamp(&mut grid, &[(0, 0), (0, 1), (1, 0), (1, 1)], A);
        stamp(&mut grid, &[(2, 0)], wild);
        let found = wins(&data, &grid, 10, 1);
        assert_eq!(found.len(), 1, "{:?}", found);
        assert_eq!(found[0].count, 5);
        assert_eq!(found[0].symbol, A);
    }

    #[test]
    fn a_grid_of_nothing_but_wilds_pays_nothing() {
        // A wild never seeds a cluster, or adjacent wilds would pay as a group
        // of nothing at whatever the wild's own paytable row says.
        let Some(data) = cluster_machine() else {
            return;
        };
        let Some(wild) = data.symbols.wild() else {
            return;
        };
        // The whole grid, so there is no real symbol anywhere for the wilds to
        // stand in for. A blob of wilds inside a field would legitimately pay:
        // the symbols around it connect *through* it, which is substitution
        // working rather than the fault this is looking for.
        let grid = Grid::from_columns(&vec![
            vec![wild; data.config.row_count];
            data.config.reel_count
        ]);
        assert!(wins(&data, &grid, 10, 1).is_empty());
    }

    #[test]
    fn one_wild_can_serve_two_clusters_at_once() {
        // It is one cell that reads as a gem to the gems and a coin to the
        // coins. Consuming it for whichever was found first would silently
        // shrink the other below the minimum.
        let Some(data) = cluster_machine() else {
            return;
        };
        let Some(wild) = data.symbols.wild() else {
            return;
        };
        let mut grid = field(&data);
        // Four of A above, four of B below, one wild between them.
        stamp(&mut grid, &[(0, 0), (0, 1), (1, 0), (1, 1)], A);
        stamp(&mut grid, &[(0, 3), (0, 4), (1, 3), (1, 4)], B);
        stamp(&mut grid, &[(0, 2)], wild);
        let found = wins(&data, &grid, 10, 1);
        assert_eq!(found.len(), 2, "{:?}", found);
        assert!(found.iter().all(|win| win.count == 5), "{:?}", found);
    }

    #[test]
    fn the_scatter_is_not_a_cluster_symbol() {
        // It pays from anywhere already; paying it again as a group would pay it
        // twice for the same cells.
        let Some(data) = cluster_machine() else {
            return;
        };
        let Some(scatter) = data.symbols.scatter() else {
            return;
        };
        let mut grid = field(&data);
        for reel in 0..2 {
            for row in 0..data.config.row_count {
                grid.set(reel, row, scatter);
            }
        }
        assert!(wins(&data, &grid, 10, 1).is_empty());
    }

    #[test]
    fn a_bigger_cluster_always_pays_more() {
        let Some(data) = cluster_machine() else {
            return;
        };
        for (symbol, _) in data.symbols.iter() {
            if Some(symbol) == data.symbols.wild() || Some(symbol) == data.symbols.scatter() {
                continue;
            }
            let mut previous = 0;
            for size in MIN_CLUSTER..=30 {
                let credits = pay(&data, symbol, size);
                assert!(
                    credits >= previous,
                    "symbol {} pays less at {} than at {}",
                    symbol,
                    size,
                    size - 1
                );
                previous = credits;
            }
        }
    }

    #[test]
    fn the_whole_grid_as_one_symbol_is_one_win() {
        let Some(data) = cluster_machine() else {
            return;
        };
        let reels = data.config.reel_count;
        let rows = data.config.row_count;
        let grid = Grid::from_columns(&vec![vec![A; rows]; reels]);

        let found = wins(&data, &grid, 10, 1);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].count, reels * rows);
        assert!(found[0].credits > 0);
    }

    #[test]
    fn evaluation_is_deterministic() {
        let Some(data) = cluster_machine() else {
            return;
        };
        let grid = crate::engine::reels::grid_from_stops(
            &data,
            &crate::engine::reels::pick_stops(
                &data,
                &mut macroquad_toolkit::rng::SeededRng::new(7),
            ),
        );
        let first = wins(&data, &grid, 10, 1);
        let second = wins(&data, &grid, 10, 1);
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.credits, b.credits);
            assert_eq!(a.cells, b.cells);
        }
    }
}
