//! The Dragon's Wrath as it reaches a live session (GDD 5.12).

use super::*;
use crate::state::celebration::CelebrationKind;

/// Spin until a clutch of eggs opens a round, leaving it open.
fn wake_the_dragon(session: &mut GameSession, data: &GameData) {
    for _ in 0..200_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        session.spin_leaving_bonus(data).unwrap();
        // The hoard can fill on the same grid; clear it so the respin round is
        // what is left standing.
        session.auto_play_bonus(data);
        if session.holdspin.is_some() {
            return;
        }
    }
    panic!("the dragon never woke in 200,000 spins");
}

#[test]
fn a_clutch_of_eggs_opens_a_round_with_them_already_locked() {
    let data = data();
    let mut session = GameSession::new(&data, 4242);
    wake_the_dragon(&mut session, &data);

    let round = session.holdspin.as_ref().unwrap();
    assert_eq!(
        round.cell_count(),
        data.config.reel_count * data.config.row_count,
        "the round should cover the whole reel window"
    );
    assert!(
        round.coins() >= data.holdspin.trigger_eggs,
        "opened with {} coins from a {}-egg trigger",
        round.coins(),
        data.holdspin.trigger_eggs
    );
    assert!(
        round.collected() > 0,
        "locked coins should be worth something"
    );
}

#[test]
fn an_open_round_holds_the_game_and_refuses_a_spin() {
    // The same rule the Vault Pick and the celebration cards follow: while a
    // feature is on screen the base game does not run underneath it.
    let data = data();
    let mut session = GameSession::new(&data, 4243);
    wake_the_dragon(&mut session, &data);

    assert!(!session.is_settled());
    let balance = session.balance;
    assert_eq!(session.begin_spin(&data), Err(SpinBlocked::Busy));
    assert_eq!(
        session.balance, balance,
        "a refused spin must not take a stake"
    );
}

#[test]
fn the_round_advances_on_its_own_beat_and_pays_out() {
    let data = data();
    let mut session = GameSession::new(&data, 4244);
    wake_the_dragon(&mut session, &data);

    let before = session.balance;
    let mut finished = None;
    for _ in 0..4_000 {
        for event in session.update_spin(&data, 1.0 / 60.0) {
            if let SpinEvent::HoldSpinFinished(outcome) = event {
                finished = Some(outcome);
            }
        }
        if finished.is_some() {
            break;
        }
    }

    let outcome = finished.expect("the round never ended on its own");
    assert!(session.holdspin.is_none());
    assert_eq!(
        session.balance,
        before + outcome.credits,
        "the round's credits should reach the balance exactly once"
    );
    assert!(matches!(
        session.celebrations.active().map(|card| card.kind()),
        Some(CelebrationKind::Wrath { .. })
    ));
}

#[test]
fn waking_the_dragon_stops_an_autospin_run() {
    // A run that survived would resume the instant the round ended, and the
    // player would never get the board back.
    let data = data();
    let mut session = GameSession::new(&data, 4245);
    session.start_autospin(500);

    for _ in 0..200_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        if session.autospin.is_none() && session.holdspin.is_none() {
            session.start_autospin(500);
        }
        session.spin_leaving_bonus(&data).unwrap();
        session.auto_play_bonus(&data);
        if session.holdspin.is_some() {
            assert!(session.autospin.is_none(), "autospin ran past the feature");
            return;
        }
    }
    panic!("the dragon never woke in 200,000 spins");
}

#[test]
fn the_headless_spin_resolves_its_own_round() {
    // Without this the sim would stall on the first clutch of eggs, and every
    // RTP figure after it would be measured on a game that had stopped.
    let data = data();
    let mut session = GameSession::new(&data, 4246);
    let mut paid = 0i64;

    for _ in 0..200_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        let resolution = session.spin(&data).unwrap();
        assert!(
            session.holdspin.is_none(),
            "spin() left a round open for the next call to trip over"
        );
        paid += resolution.wrath_credits;
        if paid > 0 {
            return;
        }
    }
    panic!("the dragon never woke in 200,000 spins");
}

#[test]
fn a_round_survives_nothing_but_still_leaves_the_save_loadable() {
    // Rounds are deliberately *not* saved — a reload lands back in the base game
    // like an in-flight free spin does (§9). What must survive is the counter.
    let data = data();
    let mut session = GameSession::new(&data, 4247);
    wake_the_dragon(&mut session, &data);
    session.auto_play_holdspin(&data);

    let save = session.to_save(&data.config.version);
    assert_eq!(save.stats.wrath_rounds, 1);

    let restored = GameSession::from_save(&data, save);
    assert!(restored.holdspin.is_none());
    assert_eq!(restored.stats.wrath_rounds, 1);
}
