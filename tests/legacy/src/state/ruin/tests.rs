use super::*;
use crate::state::GameSession;

fn data() -> GameData {
    GameData::load().unwrap()
}

fn broke(data: &GameData) -> GameSession {
    let mut session = GameSession::new(data, 0xB0_1E);
    session.balance = 0;
    session
}

#[test]
fn a_player_who_can_still_afford_the_cheapest_spin_is_not_stuck() {
    let data = data();
    let mut session = broke(&data);
    session.balance = session.cheapest_spin(&data);
    assert!(!session.is_ruined(&data));
    assert_eq!(session.lifeline(&data), None);

    // One credit short of it, and they are.
    session.balance -= 1;
    assert!(session.is_ruined(&data));
}

/// The narrowness is the point: every one of these is about to pay, and
/// rescuing them would be the game panicking on the player's behalf.
#[test]
fn a_player_mid_feature_is_never_stuck() {
    let data = data();
    let mut session = broke(&data);
    assert!(session.is_ruined(&data), "the empty case, for contrast");

    session.free_spins = Some(crate::state::FreeSpinState {
        remaining: 3,
        awarded: 8,
        line_bet: 1,
        total_won: 0,
        burned: 0,
        multiplier: 0,
    });
    assert!(
        !session.is_ruined(&data),
        "free spins cost nothing, so an empty balance is not stuck"
    );
}

#[test]
fn the_hoard_is_offered_before_the_vault() {
    let data = data();
    let mut session = broke(&data);
    session.hoard.count = 6;
    session.hoard.pot = 400;

    match session.lifeline(&data) {
        Some(Lifeline::BreakHoard { credits, pot, eggs }) => {
            assert_eq!(pot, 400);
            assert_eq!(eggs, 6);
            assert!(credits > 0 && credits < pot, "the cut has to bite");
        }
        other => panic!("expected the hoard to be offered, got {:?}", other),
    }
}

#[test]
fn an_empty_hoard_falls_through_to_the_vault() {
    let data = data();
    let session = broke(&data);
    assert!(matches!(
        session.lifeline(&data),
        Some(Lifeline::VaultStake { .. })
    ));
}

/// The books, for the lifeline that is a win.
#[test]
fn breaking_the_hoard_is_counted_as_winnings() {
    let data = data();
    let mut session = broke(&data);
    session.hoard.count = 9;
    session.hoard.pot = 1_000;
    let before = session.stats.total_won;

    let taken = session.take_lifeline(&data).expect("stuck, so offered");
    assert_eq!(session.balance, taken.credits());
    assert_eq!(session.stats.total_won, before + taken.credits());
    assert_eq!(session.hoard.count, 0);
    assert_eq!(session.hoard.pot, 0);
    assert_eq!(session.stats.staked, 0, "nothing was minted");
}

/// And for the one that is not.
#[test]
fn a_vault_stake_is_never_counted_as_winnings() {
    let data = data();
    let mut session = broke(&data);
    let before = session.stats.total_won;

    let taken = session.take_lifeline(&data).expect("stuck, so offered");
    assert_eq!(session.stats.total_won, before, "nobody won this");
    assert_eq!(session.stats.staked, taken.credits());
    assert_eq!(session.stats.vault_stakes, 1);
    assert!(
        session.balance >= session.cheapest_spin(&data),
        "a stake that does not buy a spin has not rescued anyone"
    );
}

/// A stale press from a frame where the balance had already changed must
/// not mint credits.
#[test]
fn taking_a_lifeline_that_is_not_on_offer_does_nothing() {
    let data = data();
    let mut session = GameSession::new(&data, 0xB0_1E);
    let before = session.balance;
    assert_eq!(session.take_lifeline(&data), None);
    assert_eq!(session.balance, before);
}

/// The books, over a session that actually goes broke.
///
/// §5.33's harness floats the balance before every spin so it never runs
/// out, which means it has never once exercised a lifeline. The identity it
/// checks is `opening + won - wagered == balance`; a vault stake is the one
/// credit in this game that is neither, so the identity gains a term and
/// this is the only place it is checked.
#[test]
fn the_books_balance_across_being_rescued() {
    let data = data();
    let mut session = GameSession::new(&data, 0x_B00C);
    let opening = session.balance;
    let mut rescued = 0;

    for _ in 0..3_000 {
        // A card holds the session unsettled, and `is_ruined` deliberately
        // waits for it — a rescue offered over the top of a Hatch card
        // would be the game interrupting its own good news. The player
        // dismisses them; here the loop does.
        session.celebrations.clear();
        if session.take_lifeline(&data).is_some() {
            rescued += 1;
        }
        if session.spin(&data).is_err() {
            // Not stuck and not spinnable means the bet is above the
            // balance; drop to the floor and carry on.
            session.line_bet_index = 0;
            continue;
        }
    }

    assert!(
        rescued > 0,
        "3,000 spins from {} credits without going broke once — this test is              not exercising what it claims to",
        opening
    );
    assert_eq!(
        session.balance,
        opening + session.stats.total_won + session.stats.staked - session.stats.total_wagered,
        "opening {} + won {} + staked {} - wagered {} against a balance of {}",
        opening,
        session.stats.total_won,
        session.stats.staked,
        session.stats.total_wagered,
        session.balance
    );
    assert!(session.balance >= 0, "a rescued player still went negative");
}

/// Whatever else it does, it has to leave them able to press spin.
#[test]
fn every_lifeline_buys_at_least_one_spin() {
    let data = data();
    for pot in [0, 40, 400, 4_000] {
        let mut session = broke(&data);
        session.hoard.count = 3;
        session.hoard.pot = pot;
        session.take_lifeline(&data).expect("stuck, so offered");
        assert!(
            session.balance >= session.cheapest_spin(&data),
            "a pot of {} left the player on {} against a {} spin",
            pot,
            session.balance,
            session.cheapest_spin(&data)
        );
    }
}
