use super::*;
use crate::ui::frame;

/// The bug this grid exists to stop, stated as a rule rather than a fix.
///
/// Six cabinets in one column wanted a 960px panel inside a 720px frame.
/// The panel was clamped, the last rows were drawn past the bottom edge and
/// the sixth cabinet could not be chosen — a machine present in the catalog
/// and unreachable in the picker. Nothing said the picker had to fit.
#[test]
fn every_cabinet_in_the_catalog_fits_on_the_screen() {
    let height = 120.0 + panel_rows() as f32 * ROW_HEIGHT;
    assert!(
        height <= frame::HEIGHT,
        "{} cabinets need a {}px picker in a {}px frame — the last row would \
         be drawn off the bottom and could not be clicked",
        MACHINES.len(),
        height,
        frame::HEIGHT
    );
}

/// A half-width row still has to hold its parts side by side: the Play
/// button is anchored 180px from the right, and the blurb is clipped to
/// `row.w - 220` so it stops short of it.
#[test]
fn a_column_is_wide_enough_for_a_row() {
    let column_w = (PANEL_WIDTH - 40.0 - COLUMN_GAP) / COLUMNS as f32;
    assert!(
        column_w - 220.0 >= 240.0,
        "a {}px column leaves the blurb only {}px beside the Play button",
        column_w,
        column_w - 220.0
    );
}
