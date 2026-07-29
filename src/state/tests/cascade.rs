//! Cascading reels as they reach a live session (GDD 5.15).

use super::*;
use crate::data::MACHINES;

fn avalanche() -> GameData {
    GameData::load_machine(
        MACHINES
            .iter()
            .find(|machine| machine.id == "avalanche")
            .expect("no cascading machine"),
    )
    .unwrap()
}

/// Begin spins until one produces a chain longer than a single grid.
fn start_a_chain(session: &mut GameSession, data: &GameData) -> usize {
    for _ in 0..4_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        if session.begin_spin(data).is_err() {
            continue;
        }
        let steps = session.pending_cascade_len();
        if steps > 1 {
            return steps;
        }
        // Not a chain — run it out and deal again.
        for _ in 0..3_000 {
            if session.phase.is_idle() {
                break;
            }
            session.update_spin(data, 1.0 / 60.0);
        }
    }
    panic!("no cascade in 4,000 spins");
}

#[test]
fn a_cascade_is_revealed_grid_by_grid_before_anything_is_credited() {
    // The whole point of the phase: the player must see the collapses that
    // earned the money before the money arrives.
    let data = avalanche();
    let mut session = GameSession::new(&data, 8_100);
    let steps = start_a_chain(&mut session, &data);

    let balance_at_start = session.balance;
    let mut seen = Vec::new();

    for _ in 0..6_000 {
        if let Some(reveal) = session.phase.cascade() {
            let step = reveal.step();
            if !seen.contains(&step) {
                seen.push(step);
                assert_eq!(
                    session.balance, balance_at_start,
                    "credits arrived while the chain was still running"
                );
            }
        }
        session.update_spin(&data, 1.0 / 60.0);
        if session.phase.cascade().is_none() && !seen.is_empty() {
            break;
        }
    }

    assert_eq!(
        seen.len(),
        steps,
        "the reveal skipped a grid: saw {:?} of {} steps",
        seen,
        steps
    );
    assert!(session.balance > balance_at_start, "the chain paid nothing");
}

#[test]
fn the_board_comes_to_rest_on_the_last_grid_of_the_chain() {
    let data = avalanche();
    let mut session = GameSession::new(&data, 8_101);
    start_a_chain(&mut session, &data);

    let last = session.pending_last_grid();
    for _ in 0..6_000 {
        session.update_spin(&data, 1.0 / 60.0);
        // A seam opens on the grid the chain came to rest on and then changes
        // it (§5.80), so the comparison has to be made before the rite starts —
        // otherwise this would be asserting that a *different* feature left the
        // board alone.
        if session.phase.is_idle() || session.celebrations.is_active() || session.seam.is_some() {
            break;
        }
    }

    assert_eq!(
        session.grid, last,
        "the reels settled on a grid the chain had already replaced"
    );
}

#[test]
fn the_animated_and_headless_paths_agree_on_a_cascading_machine() {
    // The invariant §8.2 exists for: the reveal must consume no randomness. A
    // cascade is the easiest place in the game to break it, because the obvious
    // implementation rolls fresh symbols for every refill.
    let data = avalanche();

    let mut headless = GameSession::new(&data, 4_242);
    let mut animated = GameSession::new(&data, 4_242);

    for _ in 0..60 {
        headless.balance = 1_000_000;
        animated.balance = 1_000_000;
        headless.celebrations.clear();
        animated.celebrations.clear();

        let resolution = headless.spin(&data).unwrap();
        animated.begin_spin(&data).unwrap();
        for _ in 0..12_000 {
            animated.update_spin(&data, 1.0 / 60.0);
            if animated.phase.is_idle()
                && animated.bonus.is_none()
                && animated.holdspin.is_none()
                && animated.seam.is_none()
            {
                break;
            }
            animated.celebrations.clear();
        }
        // `spin()` resolves any feature the grid opened, so the animated side
        // has to as well or the two are not comparing the same event.
        animated.auto_play_bonus(&data);
        animated.auto_play_holdspin(&data);
        animated.auto_play_seam(&data);

        assert_eq!(
            animated.grid, headless.grid,
            "the two paths came to rest on different grids"
        );
        assert_eq!(
            animated.balance, headless.balance,
            "the two paths paid differently"
        );
        assert!(!resolution.result.cascades.is_empty());
    }
}

#[test]
fn a_cascading_spin_credits_the_sum_of_its_steps() {
    let data = avalanche();
    let mut session = GameSession::new(&data, 8_102);

    for _ in 0..2_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        let before = session.balance;
        let resolution = session.spin(&data).unwrap();

        let stepped: i64 = resolution
            .result
            .cascades
            .iter()
            .map(|step| step.credits)
            .sum();
        assert_eq!(
            resolution.result.outcome.win_credits, stepped,
            "the outcome disagrees with the chain that produced it"
        );
        // A free spin takes no stake, so the accounting has to ask which it was.
        let stake = if resolution.was_free_spin {
            0
        } else {
            session.total_bet(&data)
        };
        assert_eq!(session.balance, before - stake + resolution.total_credits());
    }
}

#[test]
fn a_machine_without_cascades_still_produces_one_step() {
    // Consumers read `cascades` unconditionally, so the non-cascading cabinets
    // must produce the same shape rather than an empty vector.
    let data = data();
    assert!(data.cascade.is_none());

    let mut session = GameSession::new(&data, 8_103);
    for _ in 0..200 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        let resolution = session.spin(&data).unwrap();
        assert_eq!(resolution.result.cascades.len(), 1);
        assert_eq!(resolution.result.cascades[0].multiplier, 1);
        assert_eq!(&resolution.result.cascades[0].grid, &resolution.result.grid);
    }
}

#[test]
fn a_cascading_spin_is_never_settled_mid_chain() {
    // `is_settled` gates spinning and saving. A chain that read as settled would
    // let a second stake be taken while the first was still collapsing.
    let data = avalanche();
    let mut session = GameSession::new(&data, 8_104);
    start_a_chain(&mut session, &data);

    for _ in 0..6_000 {
        if session.phase.cascade().is_some() {
            assert!(!session.is_settled());
            assert_eq!(session.begin_spin(&data), Err(SpinBlocked::Busy));
        }
        session.update_spin(&data, 1.0 / 60.0);
        if session.phase.is_idle() {
            break;
        }
    }
}
