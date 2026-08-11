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
        &crate::engine::reels::pick_stops(&data, &mut macroquad_toolkit::rng::SeededRng::new(7)),
    );
    let first = wins(&data, &grid, 10, 1);
    let second = wins(&data, &grid, 10, 1);
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.credits, b.credits);
        assert_eq!(a.cells, b.cells);
    }
}
