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
        let rite = draw_weighted(&data.seam.rites, &mut rng).unwrap();
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
