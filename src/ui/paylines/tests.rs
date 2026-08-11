use super::*;
use crate::data::GameData;
use crate::engine::evaluate::SpinOutcome;

fn data() -> GameData {
    GameData::load().unwrap()
}

/// Every line has to name a row that exists on every reel, or the polyline
/// would be drawn off the grid.
#[test]
fn every_payline_stays_on_the_grid() {
    let data = data();
    for (index, payline) in data.paylines.iter().enumerate() {
        assert_eq!(
            payline.rows.len(),
            data.config.reel_count,
            "line {} covers {} reels of {}",
            index + 1,
            payline.rows.len(),
            data.config.reel_count
        );
        for row in &payline.rows {
            assert!(
                *row < data.config.row_count,
                "line {} names row {} on a {}-row grid",
                index + 1,
                row,
                data.config.row_count
            );
        }
    }
}

/// The names are the point of this; a blank one would silently fall back to
/// the number it is replacing.
#[test]
fn every_line_has_a_name_worth_showing() {
    let data = data();
    for index in 0..data.paylines.len() {
        let shown = name(&data, index);
        assert!(!shown.is_empty());
        assert!(
            !shown.starts_with("line "),
            "line {} has no name and fell back to its index",
            index + 1
        );
    }
}

/// Several wins at once take turns, and every one of them gets a turn.
///
/// Drawing six lines together is a cat's cradle; drawing only the first
/// would quietly hide the rest, which is worse — a player would see one
/// line and a payout that did not match it.
#[test]
fn every_line_win_gets_its_turn() {
    let data = data();
    let mut session = crate::state::GameSession::new(&data, 0x11E5);
    session.last_outcome = Some(SpinOutcome {
        wins: (0..3)
            .map(|line| Win {
                source: WinSource::Line(line),
                symbol: 0,
                count: 3,
                credits: 10,
                cells: vec![0, 3, 6],
            })
            .collect(),
        ..SpinOutcome::default()
    });

    let mut seen = std::collections::HashSet::new();
    // Two full cycles, sampled inside each beat rather than on its edge.
    for step in 0..6 {
        let at = BEAT * step as f32 + BEAT * 0.5;
        let (_, line) = showing(&session, at).expect("a line win is showing");
        seen.insert(line);
    }
    assert_eq!(seen.len(), 3, "only {:?} of three lines were shown", seen);
}

/// A cabinet without paylines draws nothing here, whatever it won.
#[test]
fn a_ways_win_draws_no_line() {
    let data = data();
    let mut session = crate::state::GameSession::new(&data, 0x11E5);
    session.last_outcome = Some(SpinOutcome {
        wins: vec![Win {
            source: WinSource::Ways(6),
            symbol: 0,
            count: 3,
            credits: 10,
            cells: vec![0, 3, 6],
        }],
        ..SpinOutcome::default()
    });
    assert!(showing(&session, 0.0).is_none());
    assert!(showing(&session, 12.5).is_none());
}

/// Colours cycle rather than running out, and neighbouring lines differ —
/// two wins shown one after the other must not look like one line moving.
#[test]
fn neighbouring_lines_are_drawn_in_different_colours() {
    for line in 0..20 {
        let here = line_colour(line);
        let next = line_colour(line + 1);
        assert!(
            (here.r - next.r).abs() + (here.g - next.g).abs() + (here.b - next.b).abs() > 0.2,
            "lines {} and {} are the same colour",
            line,
            line + 1
        );
    }
}
