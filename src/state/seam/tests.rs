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
