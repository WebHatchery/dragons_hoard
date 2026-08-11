use super::*;

fn clock() -> SessionClock {
    // Built by recording play rather than by setting fields, because the
    // clock owns a private cursor for the reality check and the summary
    // must read the same clock the cap was measured against.
    let mut clock = SessionClock::default();
    for _ in 0..120 {
        clock.record(4_000 / 120, 2_750 / 120);
    }
    clock.elapsed = 605.0;
    clock
}

/// Every cap says which cap it was, in the words the player set it in.
#[test]
fn each_reason_names_the_limit_that_bound() {
    assert!(reason(Breach::Time(30)).contains("30"));
    assert!(reason(Breach::Loss(5_000)).contains("5,000"));
    assert!(reason(Breach::Spins(200)).contains("200"));
    for breach in [Breach::Time(30), Breach::Loss(5_000), Breach::Spins(200)] {
        let text = reason(breach);
        assert!(
            text.contains("You set"),
            "{:?} does not say it was the player's own: {}",
            breach,
            text
        );
    }
}

/// The account has to balance, or the screen is a second opinion about the
/// thing that just stopped play.
#[test]
fn the_figures_come_from_the_clock_the_cap_was_measured_against() {
    let clock = clock();
    let data = crate::data::GameData::load().unwrap();
    let session = GameSession::new(&data, 1);
    let rows = figures(&clock, &session);

    let find = |label: &str| {
        rows.iter()
            .find(|(name, _)| name == label)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| panic!("no {} row", label))
    };
    assert_eq!(find("Staked"), naming::credits(clock.staked));
    assert_eq!(find("Came back"), naming::credits(clock.returned));
    assert_eq!(find("Net"), naming::net(clock.net()));
    assert_eq!(find("Spins"), "120");
    assert!(clock.net() < 0, "the fixture should be down on the session");
    assert_eq!(find("Played for"), "10 minutes");
}

/// Rows that would only invite a question nobody asked stay off.
#[test]
fn a_session_that_was_never_staked_says_nothing_about_the_vault() {
    let data = crate::data::GameData::load().unwrap();
    let mut session = GameSession::new(&data, 1);
    assert!(figures(&clock(), &session)
        .iter()
        .all(|(label, _)| label != "Advanced by the vault"));

    session.stats.staked = 200;
    assert!(figures(&clock(), &session)
        .iter()
        .any(|(label, _)| label == "Advanced by the vault"));
}
