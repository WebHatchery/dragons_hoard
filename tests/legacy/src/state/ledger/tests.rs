use super::*;
use crate::data::GameData;

fn config() -> GameConfig {
    GameData::load().unwrap().config
}

#[test]
fn a_fresh_ledger_knows_nothing() {
    let ledger = Ledger::default();
    assert!(ledger.get("dragon").is_none());
    assert_eq!(ledger.total_rounds(), 0);
}

#[test]
fn recording_accumulates_the_shape_of_the_play() {
    let mut ledger = Ledger::default();
    // Three rounds at 200: nothing, exactly the stake back, ten times it.
    ledger.record("dragon", 200, 0, false);
    ledger.record("dragon", 200, 200, false);
    ledger.record("dragon", 200, 2_000, true);

    let entry = ledger.get("dragon").unwrap();
    assert_eq!(entry.rounds(), 3);
    assert_eq!(entry.wagered, 600);
    assert_eq!(entry.won, 2_200);
    assert_eq!(entry.hits, 2);
    assert_eq!(entry.features, 1);
    assert!((entry.best_round - 10.0).abs() < 1e-9);
    assert!((entry.hit_frequency() - 2.0 / 3.0).abs() < 1e-9);
}

#[test]
fn each_cabinet_keeps_its_own_record() {
    // Averaging four machines that are four different games would describe
    // none of them (§5.8).
    let mut ledger = Ledger::default();
    ledger.record("dragon", 200, 400, false);
    ledger.record("frost", 200, 0, false);

    assert_eq!(ledger.get("dragon").unwrap().rounds(), 1);
    assert_eq!(ledger.get("frost").unwrap().rounds(), 1);
    assert_eq!(ledger.get("dragon").unwrap().won, 400);
    assert_eq!(ledger.get("frost").unwrap().won, 0);
    assert_eq!(ledger.total_rounds(), 2);
}

#[test]
fn a_round_with_no_stake_is_ignored() {
    // A free spin is part of the round that bought it, never a round of its
    // own; one that arrived here alone would be counted as a round the
    // player never paid for and would inflate every figure on the panel.
    let mut ledger = Ledger::default();
    ledger.record("dragon", 0, 5_000, false);
    assert!(ledger.get("dragon").is_none());
}

#[test]
fn the_bands_account_for_every_round() {
    let mut ledger = Ledger::default();
    for credits in [0, 100, 300, 800, 3_000, 12_000, 40_000] {
        ledger.record("dragon", 200, credits, false);
    }

    let bands = ledger.get("dragon").unwrap().bands();
    assert!((bands.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    // One round in each band, by construction.
    assert!(bands.iter().all(|share| *share > 0.0));
}

#[test]
fn the_margin_shrinks_as_the_sample_grows() {
    // The panel leans on this to say how far a small sample can be trusted.
    let mut few = Ledger::default();
    let mut many = Ledger::default();
    for _ in 0..10 {
        few.record("dragon", 200, 200, false);
    }
    for _ in 0..1_000 {
        many.record("dragon", 200, 200, false);
    }

    let narrow = many.get("dragon").unwrap().margin(8.0);
    let wide = few.get("dragon").unwrap().margin(8.0);
    assert!(narrow < wide);
    assert!(narrow > 0.0);
}

#[test]
fn a_single_round_has_no_meaningful_margin() {
    let mut ledger = Ledger::default();
    ledger.record("dragon", 200, 200, false);
    assert!(ledger.get("dragon").unwrap().margin(8.0).is_infinite());
}

#[test]
fn a_ledger_round_trips_through_its_own_key() {
    let config = config();
    let mut ledger = Ledger::default();
    ledger.record("dragon", 200, 1_400, true);
    ledger.record("frost", 500, 0, false);
    ledger.save(&config).unwrap();

    let restored = Ledger::load(&config);
    assert_eq!(restored.get("dragon").unwrap().won, 1_400);
    assert_eq!(restored.get("dragon").unwrap().features, 1);
    assert_eq!(restored.get("frost").unwrap().rounds(), 1);
}
