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
    let bounds = grid_rect_for(&data);

    for reel in 0..data.config.reel_count {
        for row in 0..data.config.row_count {
            let center = cell_center(&data, reel, row);
            assert!(bounds.contains(center), "cell {},{} escaped", reel, row);
        }
    }
}

#[test]
fn every_cabinet_keeps_its_active_grid_shape_in_portrait() {
    let (width, height) = crate::ui::frame::logical_size(720.0, 960.0);
    crate::ui::frame::set_width(width);
    crate::ui::frame::set_height(height);

    for machine in crate::data::MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let grid = grid_rect_for(&data);
        let wanted = data.config.reel_count.max(1) as f32 / data.config.row_count.max(1) as f32;
        let actual = grid.w / grid.h;
        assert!(
            grid.w > 100.0 && grid.h > 60.0,
            "{} collapsed: {:?}",
            machine.id,
            grid
        );
        assert!(
            (actual - wanted).abs() < 0.02,
            "{} is {:.2}, wanted {:.2}",
            machine.id,
            actual,
            wanted
        );
    }

    crate::ui::frame::set_width(crate::ui::frame::DESIGN_WIDTH);
    crate::ui::frame::set_height(crate::ui::frame::HEIGHT);
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
