use super::*;

fn data() -> GameData {
    GameData::load().unwrap()
}

#[test]
fn a_save_round_trips_the_session() {
    let data = data();
    let mut session = GameSession::new(&data, 77);
    for _ in 0..25 {
        if session.spin(&data).is_err() {
            break;
        }
    }

    let save = session.to_save(&data.config.version);
    let json = serde_json::to_value(&save).unwrap();
    let restored = migrate_save_value(Some("1.0.0".to_owned()), json, &data).unwrap();
    let reloaded = GameSession::from_save(&data, restored);

    assert_eq!(reloaded.balance, session.balance);
    assert_eq!(reloaded.line_bet_index, session.line_bet_index);
    assert_eq!(reloaded.hoard.count, session.hoard.count);
    assert_eq!(reloaded.hoard.pot, session.hoard.pot);
    assert_eq!(reloaded.stats.total_spins, session.stats.total_spins);
}

#[test]
fn a_reloaded_session_keeps_producing_the_same_spins() {
    let data = data();
    let mut session = GameSession::new(&data, 4242);
    session.spin(&data).unwrap();

    let save = session.to_save(&data.config.version);
    let mut reloaded = GameSession::from_save(&data, save);

    let a = session.spin(&data).unwrap();
    let b = reloaded.spin(&data).unwrap();
    assert_eq!(a.result.grid, b.result.grid);
}

#[test]
fn a_legacy_save_migrates_to_the_current_shape() {
    let data = data();
    let value = serde_json::json!({
        "points": 640,
        "line_bet_index": 99,
        "hoard_count": 6,
        "hoard_pot": 60
    });

    let migrated = migrate_save_value(Some("0.1.0".to_owned()), value, &data).unwrap();

    assert_eq!(migrated.version, "1.0.0");
    assert_eq!(migrated.balance, 640);
    assert_eq!(migrated.line_bet_index, data.config.line_bets.len() - 1);
    assert_eq!(migrated.hoard.count, 6);
}

#[test]
fn a_save_never_carries_an_in_flight_feature() {
    let data = data();
    let mut session = GameSession::new(&data, 12);
    session.free_spins = Some(crate::state::FreeSpinState {
        remaining: 7,
        awarded: 10,
        line_bet: 5,
        total_won: 300,
        burned: 0,
        multiplier: 0,
    });

    let reloaded = GameSession::from_save(&data, session.to_save(&data.config.version));

    assert!(!reloaded.in_free_spins());
    assert!(reloaded.phase.is_idle());
}
