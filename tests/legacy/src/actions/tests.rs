use super::*;

fn setup() -> (GameData, GameSession) {
    let data = GameData::load().unwrap();
    let session = GameSession::new(&data, 31337);
    (data, session)
}

#[test]
fn spin_intent_commits_the_stake_and_starts_the_reels() {
    let (data, mut session) = setup();
    let mut paytable = false;
    let start = session.balance;
    let total_bet = session.total_bet(&data);

    let outcome = apply(&data, &mut session, &mut paytable, UiAction::Spin);

    assert!(matches!(outcome, ActionOutcome::SpinStarted));
    assert!(session.phase.is_busy());
    // Paid for up front; nothing is credited until the reels land.
    assert_eq!(session.balance, start - total_bet);
    assert_eq!(session.stats.total_spins, 0);
}

#[test]
fn a_second_spin_intent_mid_spin_is_refused() {
    let (data, mut session) = setup();
    let mut paytable = false;

    apply(&data, &mut session, &mut paytable, UiAction::Spin);
    let balance = session.balance;
    let outcome = apply(&data, &mut session, &mut paytable, UiAction::Spin);

    assert!(matches!(
        outcome,
        ActionOutcome::SpinBlocked(SpinBlocked::Busy)
    ));
    // Critically, the refused spin must not take a second stake.
    assert_eq!(session.balance, balance);
}

#[test]
fn spin_intent_reports_an_empty_balance_instead_of_spinning() {
    let (data, mut session) = setup();
    session.balance = 0;
    let mut paytable = false;

    let outcome = apply(&data, &mut session, &mut paytable, UiAction::Spin);

    assert!(matches!(
        outcome,
        ActionOutcome::SpinBlocked(SpinBlocked::InsufficientBalance)
    ));
    assert_eq!(session.stats.total_spins, 0);
}

#[test]
fn bet_intents_walk_the_ladder_and_stop_at_the_ends() {
    let (data, mut session) = setup();
    session.line_bet_index = 0;
    let mut paytable = false;

    assert!(matches!(
        apply(&data, &mut session, &mut paytable, UiAction::BetUp),
        ActionOutcome::BetChanged(_)
    ));
    assert_eq!(session.line_bet_index, 1);

    assert!(matches!(
        apply(&data, &mut session, &mut paytable, UiAction::MaxBet),
        ActionOutcome::BetChanged(_)
    ));
    assert_eq!(session.line_bet_index, data.config.line_bets.len() - 1);

    assert!(matches!(
        apply(&data, &mut session, &mut paytable, UiAction::BetUp),
        ActionOutcome::Ignored
    ));
}

#[test]
fn the_paytable_intent_toggles_both_ways() {
    let (data, mut session) = setup();
    let mut paytable = false;

    apply(&data, &mut session, &mut paytable, UiAction::TogglePaytable);
    assert!(paytable);
    apply(&data, &mut session, &mut paytable, UiAction::TogglePaytable);
    assert!(!paytable);
}

#[test]
fn save_intents_are_handed_back_to_the_orchestrator() {
    let (data, mut session) = setup();
    let mut paytable = false;

    assert!(matches!(
        apply(&data, &mut session, &mut paytable, UiAction::Save),
        ActionOutcome::Session(SessionRequest::Save)
    ));
    assert!(matches!(
        apply(&data, &mut session, &mut paytable, UiAction::DeleteSave),
        ActionOutcome::Session(SessionRequest::DeleteSave)
    ));
}
