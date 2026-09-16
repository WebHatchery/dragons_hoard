use super::*;
use crate::data::{GameData, MACHINES};

fn data() -> GameData {
    GameData::load().unwrap()
}

/// The point of the whole thing: a pot filled on one cabinet is visible on
/// another.
#[test]
fn a_pot_filled_on_one_cabinet_shows_up_on_the_next() {
    let dragon = data();
    let frost = GameData::load_machine(crate::data::machine_by_id("frost")).unwrap();

    let mut playing = JackpotState::new(&dragon.jackpots);
    for _ in 0..500 {
        playing.contribute(&dragon.jackpots, 20);
    }

    let mut floor = Floor::default();
    floor.take_from(&dragon.jackpots, &playing);

    let mut arriving = JackpotState::new(&frost.jackpots);
    let before = arriving.value(&frost.jackpots, grand_index(&frost.jackpots));
    floor.lend_to(&frost.jackpots, &mut arriving);
    let after = arriving.value(&frost.jackpots, grand_index(&frost.jackpots));

    assert!(
        after > before,
        "500 spins on Dragon's Hoard left the Grand on Frost Wyrm at {} \
         (was {}) — the floor pot is not shared",
        after,
        before
    );
}

/// And the tiers that are *not* shared stay with their cabinet, or every
/// pot would be pooled and the ladder would mean nothing.
#[test]
fn a_cabinets_own_tiers_do_not_travel() {
    let dragon = data();
    let mut playing = JackpotState::new(&dragon.jackpots);
    for _ in 0..500 {
        playing.contribute(&dragon.jackpots, 20);
    }
    let mut floor = Floor::default();
    floor.take_from(&dragon.jackpots, &playing);

    let frost = GameData::load_machine(crate::data::machine_by_id("frost")).unwrap();
    let mut arriving = JackpotState::new(&frost.jackpots);
    floor.lend_to(&frost.jackpots, &mut arriving);

    for (index, tier) in frost.jackpots.tiers.iter().enumerate() {
        if tier.shared {
            continue;
        }
        assert_eq!(
            arriving.accrued_milli(index),
            0,
            "{} travelled between cabinets and should not have",
            tier.id
        );
    }
}

/// Addressed by id, so a cabinet with a different ladder cannot be handed
/// another cabinet's money by position.
#[test]
fn the_floor_is_keyed_by_tier_id() {
    let dragon = data();
    let mut playing = JackpotState::new(&dragon.jackpots);
    playing.contribute(&dragon.jackpots, 20_000);
    let mut floor = Floor::default();
    floor.take_from(&dragon.jackpots, &playing);

    let shared: Vec<&str> = dragon
        .jackpots
        .tiers
        .iter()
        .filter(|t| t.shared)
        .map(|t| t.id.as_str())
        .collect();
    assert!(!shared.is_empty(), "no cabinet declares a shared tier");
    for id in shared {
        assert!(floor.accrued_milli.contains_key(id), "{} not banked", id);
    }
}

/// Every cabinet has to agree about which tier is the floor's, or a player
/// walking between them would see the pot appear and vanish.
#[test]
fn every_cabinet_shares_the_same_tier() {
    let mut expected: Option<Vec<String>> = None;
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let shared: Vec<String> = data
            .jackpots
            .tiers
            .iter()
            .filter(|t| t.shared)
            .map(|t| t.id.clone())
            .collect();
        match &expected {
            None => expected = Some(shared),
            Some(first) => assert_eq!(
                &shared, first,
                "{} shares {:?} where the others share {:?}",
                machine.id, shared, first
            ),
        }
    }
    assert_eq!(expected.as_deref(), Some(["grand".to_owned()].as_slice()));
}

fn grand_index(jackpots: &Jackpots) -> usize {
    jackpots
        .tiers
        .iter()
        .position(|tier| tier.shared)
        .expect("a shared tier")
}
