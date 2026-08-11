use crate::data::GameData;
use crate::state::{migrate_save_value, GameSession};

/// Play until there is something worth keeping, then bank it.
fn a_played_session(data: &GameData) -> GameSession {
    let mut session = GameSession::new(data, 0x0DDBA11);
    for _ in 0..400 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        let _ = session.spin(data);
    }
    session
}

#[test]
fn everything_a_player_accumulates_survives_a_save_and_a_load() {
    let data = GameData::load().unwrap();
    let played = a_played_session(&data);

    // The run has to have produced something, or this passes on nothing —
    // the exact failure mode that let the bug live.
    assert!(played.stats.total_spins > 0);
    assert!(
        played.hoard.count > 0 || played.hoard.pot > 0,
        "400 spins collected no eggs; this test is not checking what it claims"
    );

    let json = serde_json::to_value(played.to_save(&data.config.version)).unwrap();
    let reloaded = migrate_save_value(Some(data.config.version.clone()), json, &data)
        .expect("a save this game just wrote did not load");
    let opened = GameSession::from_save(&data, reloaded);

    assert_eq!(opened.hoard.count, played.hoard.count, "eggs on the meter");
    assert_eq!(opened.hoard.pot, played.hoard.pot, "the banked pot");
    assert_eq!(opened.stats.total_spins, played.stats.total_spins);
    assert_eq!(opened.stats.total_wagered, played.stats.total_wagered);
    assert_eq!(opened.stats.biggest_win, played.stats.biggest_win);
    assert_eq!(opened.stats.hatches, played.stats.hatches);
}

/// The pots specifically, because they are the ones that were resetting and
/// the ones a player would never notice resetting — a progressive that goes
/// back to its seed looks exactly like a progressive nobody has fed.
#[test]
fn the_progressive_pots_survive_a_save_and_a_load() {
    let data = GameData::load().unwrap();
    let played = a_played_session(&data);

    let grown: Vec<usize> = (0..data.jackpots.tiers.len())
        .filter(|tier| played.jackpots.accrued_milli(*tier) > 0)
        .collect();
    assert!(
        !grown.is_empty(),
        "400 spins fed no pot at all, so nothing here is being checked"
    );

    let json = serde_json::to_value(played.to_save(&data.config.version)).unwrap();
    let reloaded = migrate_save_value(Some(data.config.version.clone()), json, &data).unwrap();
    let opened = GameSession::from_save(&data, reloaded);

    for tier in grown {
        assert_eq!(
            opened.jackpots.accrued_milli(tier),
            played.jackpots.accrued_milli(tier),
            "the {} pot went back to its seed",
            data.jackpots.tiers[tier].id
        );
    }
}
