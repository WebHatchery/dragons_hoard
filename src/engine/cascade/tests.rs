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
    let rows = landed.rows_on(0);

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

    let rows = landed.rows_on(0);
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
