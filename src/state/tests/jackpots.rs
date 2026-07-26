//! Progressive jackpots, end to end against a live session (GDD 5.6).

use super::*;

/// Drive a session until a jackpot lands, topping the balance up so a losing
/// streak cannot stall it. Returns the winning resolution.
fn spin_until_jackpot(session: &mut GameSession, data: &GameData) -> SpinResolution {
    for _ in 0..200_000 {
        session.balance = 1_000_000_000;
        session.celebrations.clear();
        let resolution = session.spin(data).unwrap();
        if resolution.jackpot.is_some() {
            return resolution;
        }
    }
    panic!("no jackpot landed in 200,000 spins");
}

#[test]
fn every_paid_spin_grows_the_pots() {
    let data = data();
    let mut session = GameSession::new(&data, 11);
    let before = session.jackpots.value(&data.jackpots, 0);

    for _ in 0..2_000 {
        session.balance = 1_000_000;
        session.spin(&data).unwrap();
    }

    assert!(session.jackpots.value(&data.jackpots, 0) > before);
}

#[test]
fn a_free_spin_neither_feeds_nor_draws_a_jackpot() {
    // A free spin staked nothing, so it must not top the pots up and must not
    // be allowed to win them.
    let data = data();
    let mut session = GameSession::new(&data, 12);
    session.free_spins = Some(FreeSpinState {
        remaining: 20,
        awarded: 20,
        line_bet: 25,
        total_won: 0,
        burned: 0,
        multiplier: 0,
    });
    let before = session.jackpots.value(&data.jackpots, 0);

    while session.in_free_spins() {
        let resolution = session.spin(&data).unwrap();
        assert!(resolution.jackpot.is_none(), "a free spin won a jackpot");
    }

    assert_eq!(session.jackpots.value(&data.jackpots, 0), before);
}

#[test]
fn winning_a_jackpot_credits_it_and_resets_that_tier() {
    let data = data();
    let mut session = GameSession::new(&data, 909);
    let resolution = spin_until_jackpot(&mut session, &data);
    let win = resolution.jackpot.clone().unwrap();

    assert!(win.credits >= data.jackpots.tiers[win.tier].seed);
    assert_eq!(resolution.jackpot_credits(), win.credits);
    assert!(resolution.total_credits() >= win.credits);
    assert_eq!(
        session.jackpots.value(&data.jackpots, win.tier),
        data.jackpots.tiers[win.tier].seed,
        "the won tier should be back at its seed"
    );
    assert_eq!(session.stats.jackpots, 1);
}

#[test]
fn a_jackpot_raises_its_own_card_and_suppresses_the_big_win_one() {
    let data = data();
    let mut session = GameSession::new(&data, 707);
    spin_until_jackpot(&mut session, &data);

    let mut kinds = Vec::new();
    while let Some(kind) = session
        .celebrations
        .active()
        .map(|card| card.kind().clone())
    {
        kinds.push(kind);
        session.celebrations.skip();
    }

    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, CelebrationKind::Jackpot { .. })),
        "no jackpot card was raised, saw {:?}",
        kinds
    );
    assert!(
        !kinds
            .iter()
            .any(|kind| matches!(kind, CelebrationKind::BigWin { .. })),
        "a big-win card doubled up on the jackpot"
    );
}

#[test]
fn autospin_stops_on_a_jackpot() {
    let data = data();
    let mut session = GameSession::new(&data, 4321);

    for _ in 0..200_000 {
        session.balance = 1_000_000_000;
        session.celebrations.clear();
        if session.autospin.is_none() && !session.in_free_spins() {
            session.start_autospin(100_000);
        }
        if session.spin(&data).unwrap().jackpot.is_some() {
            assert!(
                session.autospin.is_none(),
                "autospin ran straight past a jackpot"
            );
            return;
        }
    }
    panic!("no jackpot landed in 200,000 spins");
}

#[test]
fn the_pots_survive_a_save_and_reload() {
    let data = data();
    let mut session = GameSession::new(&data, 88);
    for _ in 0..5_000 {
        session.balance = 1_000_000;
        session.spin(&data).unwrap();
    }
    let banked: Vec<i64> = (0..data.jackpots.tiers.len())
        .map(|i| session.jackpots.value(&data.jackpots, i))
        .collect();

    let reloaded = GameSession::from_save(&data, session.to_save(&data.config.version));

    for (index, expected) in banked.iter().enumerate() {
        assert_eq!(
            reloaded.jackpots.value(&data.jackpots, index),
            *expected,
            "tier {} lost its accrual across a reload",
            index
        );
    }
}
