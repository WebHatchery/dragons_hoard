use super::*;

/// The panel must have room for everything the log keeps, or a spin the
/// game promises is checkable quietly is not.
#[test]
fn the_panel_shows_every_row_the_log_keeps() {
    const {
        assert!(
            crate::state::proof::KEPT <= VISIBLE * 2,
            "the log keeps more spins than the panel has rows for"
        )
    };
}

/// The rows have to fit between the prose and the button under them.
#[test]
fn the_rows_fit_the_panel() {
    let panel = frame::centred_at(900.0, frame::BELOW_HEADER, 580.0);
    let last = panel.y + 148.0 + (VISIBLE - 1) as f32 * ROW;
    assert!(
        last < panel.bottom() - 108.0,
        "the last row lands at {} and the button starts at {}",
        last,
        panel.bottom() - 108.0
    );
}
