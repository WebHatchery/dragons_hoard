use super::*;

/// Nothing the panel draws for itself may sit on the symbol window.
///
/// The grid is what the player is watching. The cascade badge sat on the
/// top-right cell for six iterations with §15 naming it every time, and no
/// gate could see it: §5.47's collision audit compares text against *text*,
/// and a symbol is art. The rule is about geometry rather than pixels, so
/// it belongs here where it can be checked without a window (§5.61).
#[test]
fn no_panel_furniture_is_drawn_over_the_reels() {
    for width in [960.0, 1280.0, 1680.0] {
        crate::ui::frame::set_width(crate::ui::frame::logical_size(width, 720.0).0);
        let grid = grid_rect();
        for (what, rect) in [
            ("the cascade multiplier badge", cascade_badge_rect()),
            ("the jackpot ladder", jackpot_strip_rect()),
        ] {
            let overlaps = rect.x < grid.right()
                && rect.right() > grid.x
                && rect.y < grid.bottom()
                && rect.bottom() > grid.y;
            assert!(
                !overlaps,
                "{} is drawn over the symbol window at {}px: {:?} against {:?}",
                what, width, rect, grid
            );
        }
    }
    crate::ui::frame::set_width(crate::ui::frame::DESIGN_WIDTH);
}

/// And it has to be somewhere a player will look, not merely somewhere else.
#[test]
fn the_badge_stays_inside_the_panel() {
    for width in [960.0, 1280.0, 1680.0] {
        crate::ui::frame::set_width(crate::ui::frame::logical_size(width, 720.0).0);
        let panel = panel_rect();
        let badge = cascade_badge_rect();
        assert!(badge.x >= panel.x, "{:?} left of {:?}", badge, panel);
        assert!(badge.right() <= panel.right(), "{:?}", badge);
        assert!(badge.bottom() <= panel.bottom(), "{:?}", badge);
    }
    crate::ui::frame::set_width(crate::ui::frame::DESIGN_WIDTH);
}
