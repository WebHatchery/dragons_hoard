use super::*;
use crate::data::MACHINES;

/// Every cabinet has something to show here, including the ones with no
/// lines at all — an empty panel would look like a bug.
#[test]
fn every_cabinet_has_something_to_say() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        assert!(!title(&data).is_empty());
        if data.paylines.is_empty() {
            let note = empty_note(&data);
            assert!(note.contains(&data.config.display_name));
            assert!(note.len() > 40, "{}: {}", machine.id, note);
        }
    }
}

/// The grid fits the panel at every window width the game supports, which
/// is what stops twenty diagrams becoming twenty overlapping ones.
#[test]
fn the_diagrams_tile_without_overlapping() {
    let data = GameData::load().unwrap();
    for width in [960.0, 1280.0, 1680.0] {
        frame::set_width(frame::logical_size(width, 720.0).0);
        let panel = frame::centred_at(1180.0, frame::BELOW_HEADER, 600.0);
        let rows = data.paylines.len().div_ceil(COLUMNS);
        let cell = vec2(
            (panel.w - 48.0) / COLUMNS as f32,
            (panel.h - 92.0) / rows as f32,
        );
        assert!(cell.x > 60.0, "diagrams {}px wide at {}", cell.x, width);
        assert!(cell.y > 40.0, "diagrams {}px tall at {}", cell.y, width);

        let last = Rect::new(
            panel.x + 24.0 + (COLUMNS - 1) as f32 * cell.x,
            panel.y + 68.0 + (rows - 1) as f32 * cell.y,
            cell.x,
            cell.y,
        );
        assert!(last.right() <= panel.right() + 0.01, "{:?}", last);
        assert!(last.bottom() <= panel.bottom() + 0.01, "{:?}", last);
    }
    frame::set_width(frame::DESIGN_WIDTH);
}
