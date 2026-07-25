//! The machine catalog: loading, isolation and distinctness (GDD 5.8).

use super::*;

#[test]
fn every_machine_in_the_catalog_loads_and_validates() {
    for machine in crate::data::MACHINES {
        GameData::load_machine(machine)
            .unwrap_or_else(|err| panic!("machine '{}' is invalid: {}", machine.id, err));
    }
}

#[test]
fn machines_have_distinct_ids_and_save_slots() {
    // A shared slot would mean two cabinets silently overwriting each other's
    // balance and hoard.
    let mut slots = Vec::new();
    for machine in crate::data::MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        slots.push(data.save_slot());
    }

    let mut unique = slots.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        slots.len(),
        "duplicate save slots: {:?}",
        slots
    );
}

#[test]
fn an_unknown_machine_id_falls_back_rather_than_failing() {
    // Removing a machine from the catalog must not strand a player whose
    // preferences still name it.
    let fallback = crate::data::machine_by_id("a-machine-that-was-removed");
    assert_eq!(fallback.id, crate::data::MACHINES[0].id);
}

#[test]
fn each_machine_runs_its_own_maths() {
    // Not a reskin: the catalog must actually offer different symbol sets and
    // strips, or the picker is decoration.
    let mut signatures = Vec::new();
    for machine in crate::data::MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let ids: Vec<&str> = data
            .symbols
            .iter()
            .map(|(_, def)| def.id.as_str())
            .collect();
        signatures.push((data.reels[0].len(), ids.join(",")));
    }

    let mut unique = signatures.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        signatures.len(),
        "two machines share a symbol set and strip length"
    );
}

#[test]
fn a_session_is_shaped_by_the_machine_it_belongs_to() {
    let dragon = GameData::load_machine(&crate::data::MACHINES[0]).unwrap();
    let frost = GameData::load_machine(&crate::data::MACHINES[1]).unwrap();

    let dragon_session = GameSession::new(&dragon, 1);
    let frost_session = GameSession::new(&frost, 1);

    // The hoard capacity differs, so the two sessions are not interchangeable.
    assert_ne!(dragon.config.hoard_capacity, frost.config.hoard_capacity);
    assert_eq!(dragon_session.grid.reel_count(), dragon.config.reel_count);
    assert_eq!(frost_session.grid.reel_count(), frost.config.reel_count);
}
