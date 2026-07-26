//! Whether the free-spin choice can be wrong (§5.64).
//!
//! The whole design rests on one claim: the shapes are worth the same, so
//! picking one is a choice about *variance* and never about return. That claim
//! is arithmetic — spins × multiplier — and arithmetic is exactly the kind of
//! thing that is true when it is written and false three data edits later.

use crate::data::{GameData, MACHINES};
use crate::state::GameSession;

fn every_machine() -> Vec<GameData> {
    MACHINES
        .iter()
        .map(|machine| GameData::load_machine(machine).unwrap())
        .collect()
}

/// The claim, on every cabinet that makes it.
#[test]
fn every_shape_is_worth_the_same_as_every_other() {
    let mut offered = 0;
    for data in every_machine() {
        let Some(first) = data.freespins.shapes.first() else {
            continue;
        };
        offered += 1;
        for shape in &data.freespins.shapes {
            assert_eq!(
                shape.value(),
                first.value(),
                "{}: '{}' is worth {} against '{}' at {}",
                data.machine.id,
                shape.id,
                shape.value(),
                first.id,
                first.value()
            );
        }
    }
    assert!(
        offered > 0,
        "no cabinet offers a choice, so this checks nothing"
    );
}

/// And it has to be a real choice, not two names for the same run.
#[test]
fn the_shapes_actually_differ() {
    for data in every_machine() {
        if data.freespins.shapes.len() < 2 {
            continue;
        }
        let multipliers: Vec<i64> = data.freespins.shapes.iter().map(|s| s.multiplier).collect();
        let lengths: Vec<i64> = data
            .freespins
            .shapes
            .iter()
            .map(|s| s.spin_permille)
            .collect();
        assert!(
            multipliers.windows(2).any(|pair| pair[0] != pair[1]),
            "{} offers shapes that all pay the same multiplier",
            data.machine.id
        );
        assert!(
            lengths.windows(2).any(|pair| pair[0] != pair[1]),
            "{} offers shapes that all run for the same length",
            data.machine.id
        );
    }
}

/// The claim that actually matters, on the awards the cabinet really gives.
///
/// `spins × multiplier` being equal between *shapes* is not enough, because the
/// spins a shape grants are a rounded fraction of the award. On a fifteen-spin
/// award, half of fifteen is seven — and seven at ×4 is twenty-eight against
/// fifteen at ×2 being thirty. The player loses seven percent of the feature by
/// choosing, which is precisely the thing this design promises cannot happen.
///
/// Caught by looking at the button, not by the arithmetic test above.
#[test]
fn every_shape_delivers_the_same_value_for_every_award_the_cabinet_gives() {
    for data in every_machine() {
        let Some(long) = data.freespins.shapes.first() else {
            continue;
        };
        for awarded in data.freespins.awards.values().copied() {
            if awarded == 0 {
                continue;
            }
            let expected = long.spins(awarded) as i64 * long.multiplier;
            for shape in &data.freespins.shapes {
                let delivered = shape.spins(awarded) as i64 * shape.multiplier;
                assert_eq!(
                    delivered, expected,
                    "{}: an award of {} run as '{}' is worth {} against '{}' at {}",
                    data.machine.id, awarded, shape.id, delivered, long.id, expected
                );
            }
        }
    }
}

/// A refining cabinet must not offer the choice, because it cannot price it:
/// burning symbols off the strips makes a late spin worth more than an early
/// one, so trading spins for multiplier would move the return.
#[test]
fn a_refining_cabinet_offers_no_choice() {
    let mut refining = 0;
    for data in every_machine() {
        if data.freespins.refine.is_some() {
            refining += 1;
            assert!(
                data.freespins.shapes.is_empty(),
                "{} refines and offers shapes; the trade would move its return",
                data.machine.id
            );
        }
    }
    assert!(refining > 0, "no cabinet refines, so this checks nothing");
}

/// Every shape leaves a feature worth having. One granting no spins would take
/// it away entirely, whatever the multiplier.
#[test]
fn no_shape_can_reduce_a_run_to_nothing() {
    for data in every_machine() {
        for shape in &data.freespins.shapes {
            for awarded in [1u32, 3, 10, 15, 20] {
                assert!(
                    shape.spins(awarded) >= 1,
                    "{}: '{}' turns {} spins into none",
                    data.machine.id,
                    shape.id,
                    awarded
                );
            }
        }
    }
}

/// The choice is open while the run has not started and shut the moment it has.
///
/// Otherwise a player could take the long odds and switch to the short ones as
/// soon as they looked bad, which is a different game.
#[test]
fn the_deal_cannot_be_re_cut_once_the_feature_is_running() {
    let data = GameData::load().unwrap();
    let mut session = GameSession::new(&data, 0xFEED);

    for _ in 0..40_000 {
        if session.in_free_spins() {
            break;
        }
        session.balance = 1_000_000;
        session.celebrations.clear();
        let _ = session.spin(&data);
    }
    assert!(session.in_free_spins(), "never reached the feature");

    assert!(
        !session.free_spin_shapes(&data).is_empty(),
        "a run that has not started should offer the choice"
    );
    let spins = session
        .choose_free_spin_shape(1, &data)
        .expect("the second shape is on offer");
    assert!(spins >= 1);

    session.balance = 1_000_000;
    session.celebrations.clear();
    let _ = session.spin(&data);
    assert!(
        session.choose_free_spin_shape(0, &data).is_none(),
        "the shape was re-cut after the run had started"
    );
}
