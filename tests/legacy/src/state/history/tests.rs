use super::*;

#[test]
fn a_fresh_history_has_nothing_to_say() {
    let history = History::default();
    assert!(history.is_empty());
    assert_eq!(history.rounds(), 0);
    assert_eq!(history.extremes(), None);
    assert_eq!(history.opening(), None);
    assert_eq!(history.deepest_fall(), 0);
}

#[test]
fn the_opening_balance_is_the_first_one_seen() {
    let mut history = History::default();
    history.record(1_000);
    history.record(50);
    history.record(9_000);
    assert_eq!(history.opening(), Some(1_000));
}

#[test]
fn the_peak_of_a_long_session_is_exact() {
    // The reason the toolkit series decimates by extremes rather than by
    // averaging: one spike in thousands of rounds must still be the number
    // the panel reports.
    let mut history = History::default();
    for round in 0..20_000 {
        history.record(if round == 4_321 { 250_000 } else { 1_000 });
    }
    let (low, high) = history.extremes().unwrap();
    assert_eq!(high, 250_000);
    assert_eq!(low, 1_000);
}

#[test]
fn the_deepest_fall_is_the_worst_peak_to_trough() {
    let mut history = History::default();
    for balance in [1_000, 5_000, 4_000, 800, 2_000, 1_900] {
        history.record(balance);
    }
    // 5,000 down to 800.
    assert_eq!(history.deepest_fall(), 4_200);
}

#[test]
fn a_session_that_only_climbs_has_no_fall() {
    let mut history = History::default();
    for round in 0..500 {
        history.record(1_000 + round * 10);
    }
    assert_eq!(history.deepest_fall(), 0);
}

#[test]
fn the_fall_survives_decimation_as_a_lower_bound() {
    // Once buckets merge, a peak and the trough after it can share one and
    // their order is lost. It must stay a plausible under-estimate rather
    // than collapsing to zero or inventing a larger one.
    let mut history = History::default();
    history.record(100_000);
    for _ in 0..20_000 {
        history.record(1_000);
    }
    let fall = history.deepest_fall();
    assert!(fall > 0, "the drop vanished entirely");
    assert!(fall <= 99_000, "reported more than ever happened");
}

#[test]
fn memory_stays_bounded_across_a_long_session() {
    let mut history = History::default();
    for round in 0..200_000 {
        history.record(round % 5_000);
        if round % 100 == 0 {
            history.mark(Cause::Feature, round);
        }
    }
    assert!(history.series().len() <= CAPACITY + 2);
    assert!(history.marks().len() <= MAX_MARKS);
    assert_eq!(history.rounds(), 200_000);
}

#[test]
fn a_mark_before_the_first_round_is_ignored() {
    // It would have no line to sit on, and would plot at whatever the graph
    // decided round zero meant.
    let mut history = History::default();
    history.mark(Cause::Jackpot, 5_000);
    assert!(history.marks().is_empty());
}

#[test]
fn marks_remember_when_and_how_much() {
    let mut history = History::default();
    history.record(1_000);
    history.record(900);
    history.mark(Cause::Feature, 900);
    history.record(7_400);
    history.mark(Cause::BigWin, 7_400);

    let marks = history.marks();
    assert_eq!(marks.len(), 2);
    assert_eq!(marks[0].cause, Cause::Feature);
    assert_eq!(marks[0].round, 2);
    assert_eq!(marks[1].balance, 7_400);
    assert!(marks[1].round > marks[0].round);
}

/// The bug the capture found: dropping the oldest left every surviving mark
/// bunched against the right-hand edge, so the graph had nothing to say
/// about the session it was describing.
#[test]
fn marks_stay_spread_across_the_whole_session() {
    let mut history = History::default();
    for round in 0..4_000u64 {
        history.record(1_000);
        if round % 12 == 0 {
            history.mark(Cause::Hatch, 1_000);
        }
    }
    let marks = history.marks();
    assert!(!marks.is_empty());
    assert!(marks.len() <= MAX_MARKS);

    // Something from the first quarter has to survive, or the graph is
    // describing the last few minutes and calling it the session.
    let total = history.rounds();
    assert!(
        marks.iter().any(|mark| mark.round < total / 4),
        "every mark is from the recent past"
    );
    assert!(marks.iter().any(|mark| mark.round > total * 3 / 4));
}

#[test]
fn a_jackpot_is_never_thinned_away() {
    // The rarest thing the game does, and the one mark a player would look
    // for by name.
    let mut history = History::default();
    history.record(1_000);
    history.mark(Cause::Jackpot, 50_000);
    for _ in 0..MAX_MARKS * 8 {
        history.mark(Cause::Feature, 1_000);
    }
    assert!(history
        .marks()
        .iter()
        .any(|mark| mark.cause == Cause::Jackpot));
}

#[test]
fn thinning_keeps_the_count_bounded() {
    // Including the case the jackpot exemption created: if every mark were
    // exempt, "never dropped" would mean "never bounded".
    for cause in Cause::ALL {
        let mut history = History::default();
        history.record(1_000);
        for _ in 0..20_000 {
            history.mark(cause, 1_000);
            assert!(history.marks().len() <= MAX_MARKS, "{:?}", cause);
        }
    }
}

#[test]
fn marks_stay_in_order() {
    let mut history = History::default();
    history.record(1_000);
    for round in 0..200 {
        history.record(1_000 + round);
        history.mark(Cause::Hatch, 1_000 + round);
    }
    for pair in history.marks().windows(2) {
        assert!(pair[1].round >= pair[0].round);
    }
}

#[test]
fn every_cause_is_named() {
    for cause in Cause::ALL {
        assert!(!cause.label().is_empty());
    }
}

#[test]
fn clearing_starts_the_session_over() {
    let mut history = History::default();
    history.record(1_000);
    history.mark(Cause::Wrath, 1_000);
    history.clear();

    assert!(history.is_empty());
    assert!(history.marks().is_empty());
    assert_eq!(history.opening(), None);
    assert_eq!(history.rounds(), 0);
}
