use super::*;
use crate::engine::evaluate::{SpinOutcome, Win, WinSource};

#[test]
fn winning_cells_cover_exactly_the_paying_run() {
    let data = GameData::load().unwrap();
    let chest = data.symbols.index_of("chest").unwrap();

    // Payline index 0 is the middle row, paid as three chests. The win
    // carries its own cells now, so the fixture states them directly rather
    // than relying on a payline lookup.
    let rows = data.config.row_count;
    let mut session = GameSession::new(&data, 1);
    session.last_outcome = Some(SpinOutcome {
        wins: vec![Win {
            source: WinSource::Line(0),
            symbol: chest,
            count: 3,
            credits: 40,
            cells: (0..3).map(|reel| reel * rows + 1).collect(),
        }],
        win_credits: 40,
        total_credits: 40,
        ..SpinOutcome::default()
    });

    let mask = winning_cells(&data, &session);
    let rows = session.grid.rows_on(0);
    for reel in 0..3 {
        assert!(mask[reel * rows + 1], "reel {} should be highlighted", reel);
    }
    // The run stopped at reel 3, and rows off the payline stay dark.
    assert!(!mask[3 * rows + 1]);
    assert!(!mask[0]);
    assert!(!mask[2]);
}

#[test]
fn winning_cells_are_empty_before_the_first_spin() {
    let data = GameData::load().unwrap();
    let session = GameSession::new(&data, 1);

    assert!(winning_cells(&data, &session).iter().all(|cell| !cell));
}

#[test]
fn every_cell_sits_inside_the_reel_window() {
    let data = GameData::load().unwrap();
    let bounds = grid_rect();

    for reel in 0..data.config.reel_count {
        for row in 0..data.config.row_count {
            let center = cell_center(&data, reel, row);
            assert!(bounds.contains(center), "cell {},{} escaped", reel, row);
        }
    }
}

#[test]
fn a_cell_scrolled_off_the_top_is_clipped_away() {
    let bounds = grid_rect();
    let above = Rect::new(bounds.x, bounds.y - 200.0, 100.0, 100.0);
    let straddling = Rect::new(bounds.x, bounds.y - 40.0, 100.0, 100.0);

    assert!(clip(above, bounds).is_none());
    let clipped = clip(straddling, bounds).unwrap();
    assert_eq!(clipped.y, bounds.y);
    assert!(clipped.h < 100.0);
}
