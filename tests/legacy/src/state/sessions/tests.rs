use super::*;

fn clock(spins: u32, staked: i64, returned: i64) -> SessionClock {
    let mut clock = SessionClock::default();
    for _ in 0..spins {
        clock.record(staked / spins.max(1) as i64, returned / spins.max(1) as i64);
    }
    clock.elapsed = spins as f32 * 12.0;
    clock
}

#[test]
fn a_session_worth_keeping_is_kept_and_reads_back() {
    let mut log = SessionLog::default();
    assert!(log.record(&clock(120, 2_400, 1_800), 900, 0, Some(Breach::Loss(600))));

    let session = *log.recent().next().expect("one session");
    assert_eq!(session.spins, 120);
    assert_eq!(session.staked, 2_400);
    assert_eq!(session.returned, 1_800);
    assert_eq!(session.net(), -600);
    assert_eq!(session.best, 900);
    assert_eq!(session.ended_by, Some(EndedBy::Loss));
}

/// Opening the game and closing it is not an evening.
#[test]
fn a_session_of_almost_nothing_is_not_recorded() {
    let mut log = SessionLog::default();
    assert!(!log.record(&clock(2, 40, 0), 0, 0, None));
    assert!(log.is_empty());
}

/// The log has a ceiling, and it drops the oldest rather than the newest —
/// the failure here would be a log that fills up and then stops recording.
#[test]
fn the_oldest_session_goes_when_the_log_is_full() {
    let mut log = SessionLog::default();
    for spins in 0..(KEPT as u32 + 5) {
        log.record(&clock(10 + spins, 100, 50), 0, 0, None);
    }
    assert_eq!(log.len(), KEPT);

    let newest = log.recent().next().unwrap().spins;
    let oldest = log.recent().last().unwrap().spins;
    assert_eq!(newest, 10 + KEPT as u32 + 4, "the newest was dropped");
    assert!(oldest > 10, "the oldest survived a full log");
}

/// Newest first: a log read oldest-first would put the session someone just
/// finished at the bottom of the screen.
#[test]
fn the_log_reads_newest_first() {
    let mut log = SessionLog::default();
    log.record(&clock(10, 100, 50), 0, 0, None);
    log.record(&clock(99, 100, 50), 0, 0, None);
    assert_eq!(log.recent().next().unwrap().spins, 99);
}

/// §5.49's rule: a log written before a field existed still loads.
#[test]
fn a_log_written_before_the_vault_was_counted_still_loads() {
    let log: SessionLog = serde_json::from_value(serde_json::json!({
        "sessions": [{ "seconds": 600, "spins": 50, "staked": 1000, "returned": 900 }]
    }))
    .unwrap();
    let session = *log.recent().next().unwrap();
    assert_eq!(session.staked_by_vault, 0);
    assert_eq!(session.ended_by, None);
    assert_eq!(session.net(), -100);
}

/// The hole §5.71 exists for: a game closed mid-session used to record
/// nothing at all, because the log only appended when the player pressed
/// "New session" — the rarest way an evening ends.
#[test]
fn a_session_the_game_was_closed_during_survives() {
    let mut log = SessionLog::default();
    assert!(log.hold(&clock(60, 1_200, 900), 300, 0, None));

    // Written to disk mid-session, then the window goes away.
    let written = serde_json::to_value(&log).unwrap();
    let mut reopened: SessionLog = serde_json::from_value(written).unwrap();
    assert_eq!(
        reopened.len(),
        1,
        "the evening was there when it was written"
    );

    // `load` seals whatever it finds open, which is what a fresh boot does.
    reopened.seal();
    let session = *reopened.recent().next().expect("the evening survived");
    assert_eq!(session.spins, 60);
    assert_eq!(session.net(), -300);
    assert!(
        !reopened.first_is_open(),
        "it is over, and should read as over"
    );
}

/// Holding is not appending: an evening updated forty times is one row.
#[test]
fn holding_the_same_session_does_not_fill_the_log() {
    let mut log = SessionLog::default();
    for spins in 10..50 {
        log.hold(&clock(spins, 100, 50), 0, 0, None);
    }
    assert_eq!(log.len(), 1);
    assert_eq!(log.recent().next().unwrap().spins, 49);
    assert!(log.first_is_open());

    log.seal();
    assert_eq!(log.len(), 1);
    assert!(!log.first_is_open());
}

#[test]
fn the_totals_are_the_sessions_added_up() {
    let mut log = SessionLog::default();
    log.record(&clock(10, 200, 100), 0, 0, None);
    log.record(&clock(20, 400, 500), 0, 0, None);
    assert_eq!(log.totals(), (30, 600, 600));
}
