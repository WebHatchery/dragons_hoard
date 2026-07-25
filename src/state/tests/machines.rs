//! The machine catalog: loading, isolation and distinctness (GDD 5.8).

use super::*;
use crate::data::{Evaluation, MACHINES};

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

#[test]
fn the_catalog_offers_more_than_one_win_model() {
    // Two cabinets that differ only in their numbers are two tunings of one
    // game. A ways machine is a different game, and this is the assertion that
    // says the catalog contains one (§5.14).
    let models: Vec<Evaluation> = MACHINES
        .iter()
        .map(|machine| GameData::load_machine(machine).unwrap().config.evaluation)
        .collect();

    assert!(models.contains(&Evaluation::Lines));
    assert!(models.contains(&Evaluation::Ways));
}

#[test]
fn a_ways_machine_declares_no_paylines_and_a_lines_machine_declares_some() {
    // The two are mutually exclusive by construction, and `validate` enforces
    // it — a ways cabinet that shipped paylines would evaluate by ways and
    // silently charge for lines that do nothing.
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        match data.config.evaluation {
            Evaluation::Lines => {
                assert!(!data.paylines.is_empty(), "{} has no lines", machine.id);
                assert_eq!(
                    data.bet_units(),
                    data.paylines.len(),
                    "{} should buy one unit per line",
                    machine.id
                );
            }
            Evaluation::Ways => {
                assert!(data.paylines.is_empty(), "{} declared paylines", machine.id);
                assert!(data.bet_units() > 0);
                match data.config.reel_heights {
                    // A shifting cabinet's ways change every spin (§5.20), so it
                    // has no fixed figure to quote — only a ceiling.
                    Some(_) => {
                        assert_eq!(data.ways_count(), None);
                        assert!(data.max_ways().unwrap() > 243);
                    }
                    None => assert_eq!(data.ways_count(), Some(243)),
                }
            }
        }
    }
}
