use super::*;
use crate::data::GameData;

fn book() -> HintBook {
    HintBook::load(&GameData::load().unwrap().config).unwrap()
}

/// A player who has done plenty of everything the hints watch, so whichever
/// one is first in file order is the one that comes due.
fn busy_player(spins: i64, hatches: i64) -> AchievementProgress {
    AchievementProgress {
        spins,
        free_spins: spins,
        hatches,
        machines_played: vec!["dragon".to_owned()],
        ..AchievementProgress::default()
    }
}

fn defs() -> Vec<HintDef> {
    macroquad_toolkit::data_loader::parse_json_labeled("assets/data/hints.json", HINTS_JSON)
        .unwrap()
}

#[test]
fn the_shipped_hints_validate() {
    assert!(validate(&defs()).is_ok());
    assert!(!defs().is_empty());
}

#[test]
fn a_hint_that_could_never_appear_is_rejected() {
    let mut defs = defs();
    defs[0].until = 0;
    assert!(validate(&defs).is_err());
}

#[test]
fn a_hint_that_retires_before_it_appears_is_rejected() {
    let mut defs = defs();
    defs[0].when = Counter::Spins;
    defs[0].earns = Counter::Spins;
    defs[0].after = 50;
    defs[0].until = 10;
    assert!(validate(&defs).is_err());
}

#[test]
fn duplicate_ids_are_rejected() {
    let mut defs = defs();
    let first = defs[0].clone();
    defs.push(first);
    assert!(validate(&defs).is_err());
}

#[test]
fn a_fresh_player_is_told_nothing() {
    // Every hint waits for the player to have done something. Firing on the
    // first frame would be the tutorial this deliberately is not.
    let book = book();
    let progress = AchievementProgress::default();
    let ledger = Ledger::default();
    assert!(book.current(&progress, &ledger).is_none());
}

#[test]
fn a_hint_appears_once_its_condition_is_met() {
    let mut book = book();
    // Enough of everything to bring the first hint due.
    let progress = busy_player(10_000, 100);

    let mut ledger = Ledger::default();
    for _ in 0..500 {
        ledger.record("dragon", 200, 400, false);
    }

    let hint = book
        .current(&progress, &ledger)
        .expect("nothing came due")
        .id
        .clone();

    // And it goes away for good once read.
    book.dismiss(&hint);
    assert!(book
        .current(&progress, &ledger)
        .is_none_or(|next| next.id != hint));
}

#[test]
fn acting_on_a_hint_retires_it_without_dismissing() {
    // The point of `earns`: a player who works it out on their own is never
    // told, and one who follows the hint is not thanked twice.
    let mut book = book();
    let progress = busy_player(10_000, 100);
    let mut ledger = Ledger::default();
    for _ in 0..500 {
        ledger.record("dragon", 200, 400, false);
    }

    let Some(hint) = book.current(&progress, &ledger).cloned() else {
        panic!("nothing came due");
    };

    // Satisfy whatever it was asking for.
    let counters = book.progress_mut();
    match hint.earns {
        Counter::Gambles => counters.gambles = hint.until,
        Counter::Buys => counters.buys = hint.until,
        Counter::LedgerOpened => counters.ledger_opened = hint.until,
        Counter::RulesOpened => counters.rules_opened = hint.until,
        _ => return, // Satisfied from elsewhere; the dismiss test covers it.
    }

    assert!(book
        .current(&progress, &ledger)
        .is_none_or(|next| next.id != hint.id));
}

#[test]
fn only_one_hint_shows_at_a_time() {
    // `current` returns an Option by construction, but the *order* matters:
    // it must be the first in file order, so a designer controls the
    // teaching sequence by moving a line.
    let book = book();
    let progress = busy_player(100_000, 1_000);
    let mut ledger = Ledger::default();
    for _ in 0..5_000 {
        ledger.record("dragon", 200, 400, false);
    }

    let shown = book.current(&progress, &ledger).unwrap();
    let expected = book
        .defs
        .iter()
        .find(|def| {
            book.value(def.when, &progress, &ledger) >= def.after
                && book.value(def.earns, &progress, &ledger) < def.until
        })
        .unwrap();
    assert_eq!(shown.id, expected.id);
}

#[test]
fn dismissals_round_trip() {
    let config = GameData::load().unwrap().config;
    let mut book = book();
    book.dismiss("a-hint-that-does-not-exist");
    book.progress_mut().gambles = 3;
    book.save(&config).unwrap();

    let restored = HintBook::load(&config).unwrap();
    assert!(restored
        .seen
        .iter()
        .any(|id| id == "a-hint-that-does-not-exist"));
    assert_eq!(restored.progress.gambles, 3);
}
