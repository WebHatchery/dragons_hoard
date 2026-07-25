//! The Vault Pick as it reaches a live session (GDD 5.10).

use super::*;
use crate::state::bonus;

/// Spin until the hoard fills and deals a board, leaving it open.
fn open_a_board(session: &mut GameSession, data: &GameData) {
    for _ in 0..5_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        session.spin_leaving_bonus(data).unwrap();
        if session.bonus.is_some() {
            return;
        }
    }
    panic!("the hoard never filled in 5,000 spins");
}

#[test]
fn filling_the_hoard_deals_a_board_instead_of_paying_out() {
    let data = data();
    let mut session = GameSession::new(&data, 21);
    open_a_board(&mut session, &data);

    let round = session.bonus.as_ref().expect("no board");
    assert_eq!(round.board_size(), data.bonus.board_size);
    assert!(round.base() > 0, "the board must be worth something");
    assert_eq!(session.stats.hatches, 1);
}

#[test]
fn an_open_board_holds_the_game() {
    // Same rule as a celebration card: the reels must not turn, and a spin must
    // not be able to start underneath it.
    let data = data();
    let mut session = GameSession::new(&data, 22);
    open_a_board(&mut session, &data);
    let balance = session.balance;

    assert!(!session.is_settled());
    assert_eq!(session.begin_spin(&data).err(), Some(SpinBlocked::Busy));
    assert_eq!(session.balance, balance, "a refused spin took a stake");

    for _ in 0..600 {
        session.update_spin(&data, 1.0 / 60.0);
    }
    assert!(session.bonus.is_some(), "the board resolved itself");
}

#[test]
fn playing_the_board_credits_the_balance_and_raises_the_card() {
    let data = data();
    let mut session = GameSession::new(&data, 23);
    open_a_board(&mut session, &data);
    session.celebrations.clear();

    let before = session.balance;
    let outcome = session.auto_play_bonus(&data).expect("no board to play");

    assert!(session.bonus.is_none());
    assert_eq!(session.balance, before + outcome.credits);
    assert!(matches!(
        session.celebrations.active().map(|card| card.kind()),
        Some(CelebrationKind::Hatch { .. })
    ));
}

#[test]
fn picking_by_hand_reaches_the_same_place_as_auto_play() {
    // The headless path the sim uses must not be a different game from the one
    // the player sees.
    let data = data();
    let mut by_hand = GameSession::new(&data, 24);
    let mut automatic = GameSession::new(&data, 24);
    open_a_board(&mut by_hand, &data);
    open_a_board(&mut automatic, &data);

    let mut hand_credits = 0;
    for index in 0..data.bonus.board_size {
        if let Some(outcome) = by_hand.pick_bonus(index, &data) {
            hand_credits = outcome.credits;
            break;
        }
    }
    let auto_credits = automatic.auto_play_bonus(&data).expect("no board").credits;

    assert_eq!(hand_credits, auto_credits);
}

#[test]
fn the_headless_spin_path_resolves_its_own_board() {
    // `spin()` must stay a single call, or the sim would stall the first time
    // the hoard filled.
    let data = data();
    let mut session = GameSession::new(&data, 25);
    let mut paid_hatches = 0;

    for _ in 0..5_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        let resolution = session.spin(&data).unwrap();
        assert!(session.bonus.is_none(), "spin() left a board open");
        if resolution.hatch_credits > 0 {
            paid_hatches += 1;
        }
    }

    assert!(paid_hatches > 0, "no hoard filled in 5,000 spins");
}

#[test]
fn the_bonus_pays_what_the_instant_hatch_used_to() {
    // The design claim in one assertion: swapping an instant payout for a pick
    // board must not move the money, only the variance.
    let data = data();
    let predicted = bonus::expected_permille(&data.bonus);

    let mut session = GameSession::new(&data, 26);
    let mut base_total = 0i64;
    let mut paid_total = 0i64;

    for _ in 0..20_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        session.spin_leaving_bonus(&data).unwrap();
        if let Some(round) = session.bonus.as_ref() {
            base_total += round.base();
            paid_total += session.auto_play_bonus(&data).unwrap().credits;
        }
    }

    assert!(base_total > 0, "no board was ever dealt");
    let ratio = paid_total as f64 / base_total as f64 * 1000.0;
    assert!(
        (ratio - predicted).abs() < 60.0,
        "boards paid {:.0} permille of their base against a predicted {:.0}",
        ratio,
        predicted
    );
}
