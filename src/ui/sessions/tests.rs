use super::*;
use crate::state::limits::{Breach, SessionClock};

fn log(count: usize) -> SessionLog {
    let mut log = SessionLog::default();
    for index in 0..count {
        let mut clock = SessionClock::default();
        for _ in 0..(10 + index) {
            clock.record(20, 14);
        }
        clock.elapsed = 600.0;
        log.record(&clock, 500, 0, Some(Breach::Time(10)));
    }
    log
}

/// The title counts, and reads properly at one.
#[test]
fn the_title_says_how_many_there_are() {
    assert_eq!(title(&SessionLog::default()), "Your sessions");
    assert_eq!(title(&log(1)), "Your last session");
    assert_eq!(title(&log(3)), "Your last 3 sessions");
}

/// Numbered newest-highest, so the ordinal does not change meaning as the
/// log fills — session #7 stays the seventh one recorded.
#[test]
fn the_newest_session_carries_the_highest_number() {
    let log = log(4);
    let first = log.recent().next().unwrap();
    assert_eq!(row(first, 4, false)[0].0, "#4");
}

/// Every session says why it stopped, including the ones nothing stopped.
#[test]
fn every_row_says_how_the_session_ended() {
    for session in log(3).recent() {
        assert!(!ended(session, false).is_empty());
    }
    let left = Session {
        seconds: 300,
        spins: 20,
        staked: 400,
        returned: 380,
        best: 40,
        staked_by_vault: 0,
        ended_by: None,
    };
    assert_eq!(ended(&left, false), "you stopped");
    assert_eq!(
        ended(&left, true),
        "playing now",
        "the row someone is in the middle of must not claim to be over"
    );
}

/// The columns are laid out by right edge and must not run into each other
/// — a row is seven figures and the panel is not wide.
#[test]
fn the_columns_are_in_order_and_do_not_collide() {
    let session = *log(1).recent().next().unwrap();
    let cells = row(&session, 1, false);
    for pair in cells.windows(2) {
        assert!(
            pair[1].1 > pair[0].1,
            "columns at {} and {} are out of order",
            pair[0].1,
            pair[1].1
        );
    }
    assert!(cells.last().unwrap().1 < 880.0 - 48.0);
}
