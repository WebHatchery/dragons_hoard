use super::*;

fn choices() -> LimitChoices {
    LimitChoices::load().unwrap()
}

fn state() -> LimitState {
    LimitState::with_defaults(&choices())
}

#[test]
fn the_shipped_choices_load_and_can_all_be_turned_off() {
    let choices = choices();
    assert!(choices.validate().is_ok());
    assert!(choices.time_minutes.contains(&0));
    assert!(choices.losses.contains(&0));
    assert!(choices.spins.contains(&0));
    assert!(choices.reality_check_minutes.contains(&0));
}

#[test]
fn a_choice_set_with_no_off_switch_is_rejected() {
    let mut choices = choices();
    choices.losses.retain(|value| *value != 0);
    assert!(choices.validate().is_err());
}

#[test]
fn limits_start_off() {
    // Opt-in. A game that capped play by default would be making a decision
    // that is not its to make.
    let state = state();
    assert_eq!(state.in_force(Cap::Time), None);
    assert_eq!(state.in_force(Cap::Loss), None);
    assert_eq!(state.in_force(Cap::Spins), None);
    assert!(state.breach().is_none());
}

#[test]
fn tightening_takes_effect_immediately() {
    // Deciding you have had enough should never involve waiting.
    let mut state = state();
    assert!(state.request(Cap::Loss, Some(5_000)));
    assert_eq!(state.in_force(Cap::Loss), Some(5_000));

    assert!(state.request(Cap::Loss, Some(1_000)));
    assert_eq!(state.in_force(Cap::Loss), Some(1_000));
    assert!(!state.deferred(Cap::Loss));
}

#[test]
fn loosening_waits_for_the_next_session() {
    // The only rule here that does real work: it moves the decision to raise
    // a limit out of the moment that made you want to raise it.
    let mut state = state();
    state.request(Cap::Loss, Some(1_000));

    assert!(!state.request(Cap::Loss, Some(9_000)));
    assert_eq!(state.in_force(Cap::Loss), Some(1_000));
    assert_eq!(state.requested(Cap::Loss), Some(9_000));
    assert!(state.deferred(Cap::Loss));

    state.new_session();
    assert_eq!(state.in_force(Cap::Loss), Some(9_000));
    assert!(!state.deferred(Cap::Loss));
}

#[test]
fn turning_a_limit_off_is_the_loosest_move_there_is() {
    let mut state = state();
    state.request(Cap::Spins, Some(50));
    assert!(!state.request(Cap::Spins, None));
    assert_eq!(state.in_force(Cap::Spins), Some(50));

    state.new_session();
    assert_eq!(state.in_force(Cap::Spins), None);
}

#[test]
fn a_first_cap_is_always_a_tightening() {
    let mut state = state();
    assert!(state.request(Cap::Time, Some(30)));
    assert_eq!(state.in_force(Cap::Time), Some(30));
}

#[test]
fn the_three_caps_are_independent() {
    let mut state = state();
    state.request(Cap::Loss, Some(1_000));
    state.request(Cap::Spins, Some(100));
    assert_eq!(state.in_force(Cap::Loss), Some(1_000));
    assert_eq!(state.in_force(Cap::Spins), Some(100));
    assert_eq!(state.in_force(Cap::Time), None);
}

#[test]
fn a_loss_limit_binds_on_net_position() {
    // Net, not turnover: a player who has staked a fortune and got it back
    // has not lost anything.
    let mut state = state();
    state.request(Cap::Loss, Some(1_000));

    for _ in 0..10 {
        state.clock.record(500, 500);
    }
    assert!(state.evaluate().is_none(), "even money is not a loss");

    state.clock.record(1_000, 0);
    assert_eq!(state.evaluate(), Some(Breach::Loss(1_000)));
}

#[test]
fn winning_back_does_not_lift_a_breach() {
    // Sticky on purpose. "Play until you are even" is the exact thought the
    // limit was set to interrupt.
    let mut state = state();
    state.request(Cap::Loss, Some(1_000));
    state.clock.record(2_000, 0);
    assert!(state.evaluate().is_some());

    state.clock.record(0, 50_000);
    assert_eq!(state.evaluate(), Some(Breach::Loss(1_000)));
}

#[test]
fn a_time_limit_binds_on_the_clock() {
    let mut state = state();
    state.request(Cap::Time, Some(2));
    state.clock.tick(119.0);
    assert!(state.evaluate().is_none());
    state.clock.tick(2.0);
    assert_eq!(state.evaluate(), Some(Breach::Time(2)));
}

#[test]
fn a_spin_limit_binds_on_paid_spins() {
    let mut state = state();
    state.request(Cap::Spins, Some(3));
    for _ in 0..2 {
        state.clock.record(100, 0);
    }
    assert!(state.evaluate().is_none());
    // A free spin costs nothing and is part of the round that bought it, so
    // it is not a spin against the cap (§5.18).
    state.clock.record(0, 5_000);
    assert!(state.evaluate().is_none());

    state.clock.record(100, 0);
    assert_eq!(state.evaluate(), Some(Breach::Spins(3)));
}

#[test]
fn a_new_game_clears_a_breach() {
    let mut state = state();
    state.request(Cap::Spins, Some(1));
    state.clock.record(100, 0);
    assert!(state.evaluate().is_some());

    state.new_session();
    assert!(state.breach().is_none());
    assert_eq!(state.clock.spins, 0);
}

#[test]
fn tightening_below_where_you_already_are_binds_at_once() {
    // Setting a 10-spin cap after 40 spins should stop play, not wait for
    // spin 50.
    let mut state = state();
    for _ in 0..40 {
        state.clock.record(100, 0);
    }
    state.request(Cap::Spins, Some(10));
    assert_eq!(state.evaluate(), Some(Breach::Spins(10)));
}

#[test]
fn the_reality_check_repeats_on_an_interval() {
    let mut state = state();
    state.reality_check_minutes = 5;
    assert!(!state.clock.check_due(5));

    state.clock.tick(5.0 * 60.0);
    assert!(state.clock.check_due(5));

    state.clock.acknowledge();
    assert!(!state.clock.check_due(5));

    // And comes back an interval later rather than at the next total.
    state.clock.tick(4.0 * 60.0);
    assert!(!state.clock.check_due(5));
    state.clock.tick(60.0);
    assert!(state.clock.check_due(5));
}

#[test]
fn a_reality_check_of_zero_never_fires() {
    let mut state = state();
    state.clock.tick(10_000.0);
    assert!(!state.clock.check_due(0));
}

#[test]
fn acknowledging_does_not_reset_the_session_totals() {
    // The check reports the session, not the interval — otherwise every
    // check would say the player had just arrived.
    let mut state = state();
    state.clock.record(1_000, 200);
    state.clock.tick(600.0);
    state.clock.acknowledge();
    assert_eq!(state.clock.staked, 1_000);
    assert_eq!(state.clock.net(), -800);
}

#[test]
fn the_clock_reports_what_it_measured() {
    let mut state = state();
    state.clock.record(1_000, 400);
    state.clock.record(1_000, 1_600);
    assert_eq!(state.clock.spins, 2);
    assert_eq!(state.clock.staked, 2_000);
    assert_eq!(state.clock.returned, 2_000);
    assert_eq!(state.clock.net(), 0);
    assert!((state.clock.rtp() - 1.0).abs() < 1e-9);
}

#[test]
fn an_empty_session_reports_no_return_rather_than_dividing_by_zero() {
    assert_eq!(SessionClock::default().rtp(), 0.0);
    assert_eq!(SessionClock::default().net(), 0);
}

#[test]
fn every_breach_says_which_cap_and_what_to_do() {
    for breach in [Breach::Time(30), Breach::Loss(5_000), Breach::Spins(200)] {
        let message = breach.message();
        assert!(message.contains("new game"), "{}", message);
        assert!(message.len() > 30);
    }
}

#[test]
fn tightness_is_ordered_the_way_a_player_would_expect() {
    assert!(is_tighter(None, Some(10)));
    assert!(is_tighter(Some(10), Some(5)));
    assert!(!is_tighter(Some(5), Some(10)));
    assert!(!is_tighter(Some(5), None));
    assert!(!is_tighter(None, None));
    // Equal is not tighter, so re-picking the current value is a no-op
    // rather than a deferral that would look like a pending change.
    assert!(!is_tighter(Some(5), Some(5)));
}

#[test]
fn re_picking_the_same_value_leaves_nothing_pending() {
    let mut state = state();
    state.request(Cap::Loss, Some(1_000));
    state.request(Cap::Loss, Some(1_000));
    assert!(!state.deferred(Cap::Loss));
}
