//! Refining free spins as they reach a live session (GDD 5.21).

use super::*;
use crate::data::MACHINES;

fn frost() -> GameData {
    GameData::load_machine(
        MACHINES
            .iter()
            .find(|machine| machine.id == "frost")
            .expect("no frost machine"),
    )
    .unwrap()
}

/// Spin until the feature triggers, leaving it about to run.
fn trigger(session: &mut GameSession, data: &GameData) {
    for _ in 0..80_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        session.spin(data).unwrap();
        if session.in_free_spins() {
            return;
        }
    }
    panic!("no free spins in 80,000 spins");
}

#[test]
fn the_burn_deepens_one_symbol_per_free_spin_and_stops_at_the_order() {
    let data = frost();
    let depth = data.refine_depth();
    assert!(depth > 0, "frost should refine");

    let mut session = GameSession::new(&data, 9_100);
    trigger(&mut session, &data);
    assert_eq!(
        session.free_spins.as_ref().unwrap().burned,
        0,
        "the burn should not start until the first free spin runs"
    );

    let mut seen = Vec::new();
    while session.in_free_spins() {
        session.balance = 1_000_000;
        session.celebrations.clear();
        session.spin(&data).unwrap();
        if let Some(state) = session.free_spins.as_ref() {
            seen.push(state.burned);
        }
    }

    assert!(!seen.is_empty());
    // Rises by one a spin, then holds at the length of the order.
    for (index, burned) in seen.iter().enumerate() {
        assert_eq!(*burned, (index + 1).min(depth), "spin {}", index);
    }
}

#[test]
fn a_refined_strip_really_loses_the_symbol() {
    // The whole mechanic. If the strips came back unchanged the feature would
    // just be N ordinary spins with a longer banner.
    let data = frost();
    let refine = data.freespins.refine.as_ref().unwrap();

    for burned in 1..=refine.order.len() {
        let strips = data.refined_reels(burned);
        for id in refine.order.iter().take(burned) {
            let symbol = data.symbols.index_of(id).unwrap();
            for (reel, strip) in strips.iter().enumerate() {
                assert!(
                    !strip.contains(&symbol),
                    "'{}' survived on reel {} at burn {}",
                    id,
                    reel,
                    burned
                );
            }
        }
    }
}

#[test]
fn a_strip_never_burns_down_to_nothing() {
    // A reel with no symbols left could not be spun at all. Validation cannot
    // catch it — whether it happens depends on one designer's reel layout.
    let data = frost();
    for burned in 0..=data.refine_depth() + 3 {
        for strip in data.refined_reels(burned) {
            assert!(!strip.is_empty());
        }
    }
}

#[test]
fn nothing_is_burned_outside_the_feature() {
    let data = frost();
    assert_eq!(data.refined_reels(0), data.reels);

    let mut session = GameSession::new(&data, 9_101);
    for _ in 0..200 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        if session.in_free_spins() {
            session.spin(&data).unwrap();
            continue;
        }
        let resolution = session.spin(&data).unwrap();
        if resolution.was_free_spin {
            continue;
        }
        // A base spin's grid can only hold symbols the raw strips carry, which
        // is trivially true — what matters is that it *can* hold a burned one.
        assert!(!session.in_free_spins() || session.free_spins.as_ref().unwrap().burned == 0);
    }
}

#[test]
fn a_retrigger_does_not_take_the_reels_back() {
    // Extra spins must not be a punishment. Rebuilding the strips on a
    // retrigger would undo everything the feature had burned.
    let data = frost();
    let mut session = GameSession::new(&data, 9_102);
    trigger(&mut session, &data);

    let mut highest = 0usize;
    while session.in_free_spins() {
        session.balance = 1_000_000;
        session.celebrations.clear();
        session.spin(&data).unwrap();
        if let Some(state) = session.free_spins.as_ref() {
            assert!(
                state.burned >= highest,
                "the burn went backwards: {} after {}",
                state.burned,
                highest
            );
            highest = state.burned;
        }
    }
    assert!(highest > 0);
}

#[test]
fn a_cabinet_without_a_refine_order_is_untouched() {
    // Every other machine has to behave exactly as it did.
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        if data.freespins.refine.is_some() {
            continue;
        }
        assert_eq!(data.refine_depth(), 0);
        for burned in 0..4 {
            assert_eq!(data.refined_reels(burned), data.reels, "{}", machine.id);
        }
    }
}

#[test]
fn refining_never_burns_the_wild_the_scatter_or_the_egg() {
    // Validation rejects it, but this states the reason: burning the scatter
    // would kill the retrigger, the wild would kill substitution, and the egg
    // feeds the hoard.
    let data = frost();
    let refine = data.freespins.refine.as_ref().unwrap();
    for id in &refine.order {
        let index = data.symbols.index_of(id).unwrap();
        assert!(!data.symbols.is_wild(index));
        assert!(!data.symbols.is_scatter(index));
        assert_ne!(Some(index), data.symbols.hoard());
    }
}
