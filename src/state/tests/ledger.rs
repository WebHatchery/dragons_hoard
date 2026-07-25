//! Round accounting as it reaches a live session (GDD 5.18).

use super::*;

/// Play `n` paid spins headlessly, draining closed rounds as the orchestrator
/// would, and return them.
fn play(session: &mut GameSession, data: &GameData, spins: usize) -> Vec<(i64, i64, bool)> {
    let mut rounds = Vec::new();
    for _ in 0..spins {
        session.balance = 1_000_000;
        session.celebrations.clear();
        session.spin(data).unwrap();
        if let Some(round) = session.closed_round.take() {
            rounds.push((round.wagered, round.credits, round.feature));
        }
    }
    rounds
}

#[test]
fn a_round_opens_on_a_paid_spin_and_closes_on_the_next() {
    let data = data();
    let mut session = GameSession::new(&data, 7_100);

    // The first spin opens a round and closes nothing.
    session.balance = 1_000_000;
    session.spin(&data).unwrap();
    assert!(
        session.closed_round.is_none(),
        "the first spin closed a round"
    );
    assert_eq!(session.open_round.wagered, session.total_bet(&data));

    // The second closes the first.
    session.balance = 1_000_000;
    session.celebrations.clear();
    session.spin(&data).unwrap();
    let closed = session.closed_round.take().expect("no round closed");
    assert_eq!(closed.wagered, session.total_bet(&data));
}

#[test]
fn a_free_spin_never_opens_a_round_of_its_own() {
    // The definition the whole comparison rests on: a free spin is part of the
    // return on the spin that bought it. Counting it separately would inflate
    // the round count and deflate every ratio on the panel.
    let data = data();
    let mut session = GameSession::new(&data, 7_101);
    let mut paid = 0usize;
    let mut closed = 0usize;

    for _ in 0..4_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        let free = session.in_free_spins();
        session.spin(&data).unwrap();
        if !free {
            paid += 1;
        }
        if session.closed_round.take().is_some() {
            closed += 1;
        }
    }

    assert!(
        session.stats.free_spins_played > 0,
        "no free spins in sample"
    );
    // Every paid spin but the one still open closed exactly one round.
    assert_eq!(closed, paid - 1);
}

#[test]
fn a_rounds_credits_include_the_free_spins_it_bought() {
    let data = data();
    let mut session = GameSession::new(&data, 7_102);

    for _ in 0..8_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        let resolution = session.spin(&data).unwrap();
        if resolution.outcome().free_spins_awarded == 0 {
            session.closed_round.take();
            continue;
        }

        // Play the feature out; it must all land on the open round.
        let trigger_credits = session.open_round.credits;
        let mut feature_credits = 0i64;
        while session.in_free_spins() {
            session.celebrations.clear();
            let free = session.spin(&data).unwrap();
            feature_credits += free.spin_credits;
        }

        assert!(
            session.open_round.feature,
            "the round is not marked a feature"
        );
        assert!(
            session.open_round.credits >= trigger_credits + feature_credits,
            "free spins paid {} but the round only holds {}",
            feature_credits,
            session.open_round.credits - trigger_credits
        );
        return;
    }
    panic!("no free spins in 8,000 spins");
}

#[test]
fn every_closed_round_carries_the_stake_that_paid_for_it() {
    let data = data();
    let mut session = GameSession::new(&data, 7_103);
    let rounds = play(&mut session, &data, 500);

    assert!(!rounds.is_empty());
    for (wagered, credits, _) in &rounds {
        assert!(*wagered > 0, "a round closed with no stake");
        assert!(*credits >= 0);
    }
}

#[test]
fn a_ledger_built_from_play_matches_the_rounds_it_saw() {
    // The orchestrator drains rounds into the ledger; this is that loop, with
    // the arithmetic checked against the rounds themselves.
    use crate::state::ledger::Ledger;

    let data = data();
    let mut session = GameSession::new(&data, 7_104);
    let rounds = play(&mut session, &data, 800);

    let mut ledger = Ledger::default();
    for (wagered, credits, feature) in &rounds {
        ledger.record(data.machine_id(), *wagered, *credits, *feature);
    }

    let entry = ledger.get(data.machine_id()).unwrap();
    assert_eq!(entry.rounds(), rounds.len() as u64);
    assert_eq!(entry.wagered, rounds.iter().map(|r| r.0).sum::<i64>());
    assert_eq!(entry.won, rounds.iter().map(|r| r.1).sum::<i64>());
    assert_eq!(entry.hits, rounds.iter().filter(|r| r.1 > 0).count() as u64);
}

#[test]
fn a_bought_feature_does_not_close_a_round_it_never_opened() {
    // A Feature Buy (§5.13) is a stake, but it is not a spin: it must not leave
    // a half-formed round behind for the next paid spin to close.
    let data = data();
    let mut session = GameSession::new(&data, 7_105);
    session.balance = 10_000_000;

    session.buy_feature(0, &data).unwrap();
    assert!(session.closed_round.is_none());
}
