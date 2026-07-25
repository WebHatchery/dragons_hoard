//! The Dragon's Gamble as it reaches a live session (GDD 5.16).

use super::*;
use crate::state::gamble::Scale;

/// Spin until one pays, leaving the session settled on a win.
fn win_a_spin(session: &mut GameSession, data: &GameData) -> i64 {
    for _ in 0..20_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        let resolution = session.spin(data).unwrap();
        if resolution.total_credits() > 0 && session.can_gamble() {
            return session.last_win;
        }
    }
    panic!("no winning spin in 20,000");
}

#[test]
fn staking_a_win_takes_it_back_out_of_the_balance() {
    // The win was credited when the spin settled. Gambling it has to remove it
    // again, or a losing gamble would cost nothing and a winning one would pay
    // the original twice.
    let data = data();
    let mut session = GameSession::new(&data, 6_100);
    let win = win_a_spin(&mut session, &data);

    let before = session.balance;
    let staked = session.begin_gamble(&data).unwrap();

    assert_eq!(staked, win);
    assert_eq!(session.balance, before - win);
    assert_eq!(session.gamble.as_ref().unwrap().stake(), win);
}

#[test]
fn taking_a_gamble_that_was_never_flipped_returns_exactly_the_win() {
    // Opening the panel and backing out must be free.
    let data = data();
    let mut session = GameSession::new(&data, 6_101);
    let win = win_a_spin(&mut session, &data);
    let before = session.balance;

    session.begin_gamble(&data).unwrap();
    let taken = session.take_gamble().unwrap();

    assert_eq!(taken, win);
    assert_eq!(session.balance, before);
    assert!(session.gamble.is_none());
}

#[test]
fn a_won_flip_doubles_what_reaches_the_balance() {
    let data = data();
    let mut session = GameSession::new(&data, 6_102);

    for _ in 0..400 {
        let win = win_a_spin(&mut session, &data);
        let before = session.balance;
        session.begin_gamble(&data).unwrap();

        let flip = session.flip_gamble(Scale::Ember, false, &data).unwrap();
        if !flip.won {
            continue;
        }
        let taken = session.take_gamble().unwrap();
        assert_eq!(taken, win * 2);
        assert_eq!(session.balance, before + win);
        return;
    }
    panic!("no winning flip in 400 attempts");
}

#[test]
fn a_lost_flip_closes_the_round_and_leaves_nothing() {
    let data = data();
    let mut session = GameSession::new(&data, 6_103);

    for _ in 0..400 {
        let win = win_a_spin(&mut session, &data);
        let before = session.balance;
        session.begin_gamble(&data).unwrap();

        let flip = session.flip_gamble(Scale::Ember, false, &data).unwrap();
        if flip.won {
            session.take_gamble();
            continue;
        }

        assert!(
            session.gamble.is_none(),
            "a busted round must close itself rather than ask for a Take on zero"
        );
        assert_eq!(session.balance, before - win);
        assert_eq!(session.last_win, 0);
        assert_eq!(session.stats.gambles_lost, 1);
        return;
    }
    panic!("no losing flip in 400 attempts");
}

#[test]
fn a_gamble_is_not_offered_without_a_win() {
    let data = data();
    let mut session = GameSession::new(&data, 6_104);
    session.balance = 1_000_000;

    // A fresh session has never won anything.
    assert!(!session.can_gamble());
    assert!(session.begin_gamble(&data).is_err());
    assert!(session.gamble.is_none());
}

#[test]
fn a_gamble_is_not_offered_during_free_spins_or_an_autospin_run() {
    // Both would have the game spinning itself underneath a decision the player
    // is still making.
    let data = data();
    let mut session = GameSession::new(&data, 6_105);
    win_a_spin(&mut session, &data);
    assert!(session.can_gamble());

    session.start_autospin(10);
    assert!(!session.can_gamble(), "offered mid-autospin");
    session.stop_autospin(crate::state::autospin::AutospinStop::Cancelled);

    session.free_spins = Some(crate::state::FreeSpinState {
        remaining: 3,
        awarded: 3,
        line_bet: session.line_bet(&data),
        total_won: 0,
    });
    assert!(!session.can_gamble(), "offered mid-feature");
}

#[test]
fn an_open_gamble_holds_the_reels() {
    let data = data();
    let mut session = GameSession::new(&data, 6_106);
    win_a_spin(&mut session, &data);
    session.begin_gamble(&data).unwrap();

    assert!(!session.is_settled());
    let balance = session.balance;
    assert_eq!(session.begin_spin(&data), Err(SpinBlocked::Busy));
    assert_eq!(session.balance, balance, "a refused spin took a stake");
}

#[test]
fn a_half_gamble_keeps_half_out_of_reach() {
    let data = data();
    let mut session = GameSession::new(&data, 6_107);

    for _ in 0..400 {
        let win = win_a_spin(&mut session, &data);
        if win < 2 {
            continue;
        }
        let before = session.balance;
        session.begin_gamble(&data).unwrap();
        let flip = session.flip_gamble(Scale::Ember, true, &data).unwrap();

        let kept = win - win / 2;
        if flip.won {
            let taken = session.take_gamble().unwrap();
            assert_eq!(taken, kept + (win / 2) * 2);
        } else {
            // Half survived, so the round is still standing and takeable.
            let taken = session.take_gamble().unwrap();
            assert_eq!(taken, kept);
            assert_eq!(session.balance, before - win + kept);
        }
        return;
    }
    panic!("no win large enough to halve in 400 spins");
}

#[test]
fn a_gamble_is_not_written_to_the_save() {
    // In-flight features are never persisted (§9); a reload lands back in the
    // base game rather than owing the player a decision.
    let data = data();
    let mut session = GameSession::new(&data, 6_108);
    win_a_spin(&mut session, &data);
    session.begin_gamble(&data).unwrap();

    let save = session.to_save(&data.config.version);
    let restored = GameSession::from_save(&data, save);
    assert!(restored.gamble.is_none());
}
