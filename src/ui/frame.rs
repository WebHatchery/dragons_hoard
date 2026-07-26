//! Where the big pieces go, for a window of any shape (§5.46).
//!
//! # Letterboxing is a decision nobody made
//!
//! The game drew into a fixed 1280×720 and let the virtual UI letterbox it. On a
//! 16:9 monitor that is invisible and correct. On anything else it is bars —
//! and on a phone held sideways, where the aspect is nearer 20:9, it is bars
//! *and* controls a third the size they need to be (§5.45), because the whole
//! layout has been shrunk to fit a shape it does not have.
//!
//! # Fixed height, flexible width
//!
//! The height stays 720. It is what every panel's vertical layout was written
//! against, and a reel window has a natural height that a taller screen should
//! not stretch.
//!
//! The **width follows the window**. A wide screen gets a wider logical space —
//! genuinely more room, not a scaled-up copy — and a narrow one gets less. So
//! nothing is ever letterboxed left or right, and on a narrow screen every
//! control is a larger fraction of it, which is the same thing as being bigger.
//!
//! Clamped at both ends. Below about 4:3 the wager panel and the reels stop
//! fitting side by side; above about 21:9 the two drift so far apart that the
//! player is looking at two different screens. Outside the clamp the letterbox
//! comes back, which is the right failure: bars are better than a broken layout.
//!
//! # One place that knows the shape
//!
//! Every panel used to carry its own `Rect::new(340.0, ...)` with numbers chosen
//! against 1280. Those are all offsets from an edge or a centre, and a constant
//! is only the right way to write one when the edges never move.

use macroquad::prelude::*;
use std::cell::RefCell;

/// The height everything is laid out against.
pub const HEIGHT: f32 = 720.0;
/// The width the game was designed at, and what it uses when the window is
/// 16:9. Every panel size is still chosen against this.
pub const DESIGN_WIDTH: f32 = 1280.0;

/// Narrowest and widest logical width, as multiples of the height.
///
/// 4:3 is where the wager panel and the reels stop fitting beside each other;
/// 21:9 is where they are far enough apart to read as two screens.
const MIN_ASPECT: f32 = 4.0 / 3.0;
const MAX_ASPECT: f32 = 21.0 / 9.0;

/// The short side of the layout when the window is taller than it is wide.
///
/// Portrait is not landscape with bars (§5.79). Below 4:3 the layout turns:
/// the reels take the full width and the wager controls sit under them. 720 is
/// the same number the landscape layout uses for its own short side, so a
/// control is the same size in logical pixels either way — and on a tablet held
/// upright, 810 CSS pixels against a 720-wide layout puts a 44-pixel control at
/// 49, which is the first time portrait has cleared the touch standard at all.
pub const PORTRAIT_WIDTH: f32 = 720.0;

/// Tallest portrait layout, as a multiple of the width. Past this the reels and
/// the controls drift apart the way §5.46 describes for very wide screens, and
/// the letterbox is the better answer.
const MAX_PORTRAIT: f32 = 16.0 / 9.0;

/// The logical size to draw into, for a window of this shape.
///
/// Returns width and height, because the height is no longer a constant. It was
/// one for seventy-eight sections, and "the height stays 720" is exactly the
/// sentence that made a portrait layout impossible — the frame did not refuse
/// portrait, it simply had no way to express one.
pub fn logical_size(window_width: f32, window_height: f32) -> (f32, f32) {
    if window_height <= 0.0 || !window_width.is_finite() || !window_height.is_finite() {
        return (DESIGN_WIDTH, HEIGHT);
    }
    let aspect = window_width / window_height;
    if aspect >= MIN_ASPECT {
        return ((HEIGHT * aspect.min(MAX_ASPECT)).round(), HEIGHT);
    }
    let tall = (PORTRAIT_WIDTH / aspect.max(1.0 / MAX_PORTRAIT)).round();
    (PORTRAIT_WIDTH, tall)
}

/// Is the layout turned (§5.79)?
pub fn is_portrait() -> bool {
    height() > HEIGHT
}

thread_local! {
    static CURRENT: RefCell<f32> = const { RefCell::new(DESIGN_WIDTH) };
    static CURRENT_HEIGHT: RefCell<f32> = const { RefCell::new(HEIGHT) };
}

/// Record the height this frame is drawing at (§5.79).
pub fn set_height(height: f32) {
    CURRENT_HEIGHT.with(|slot| *slot.borrow_mut() = height);
}

/// The height being drawn at. `HEIGHT` in landscape, taller in portrait.
pub fn height() -> f32 {
    CURRENT_HEIGHT.with(|slot| *slot.borrow())
}

/// Record the width this frame is drawing at.
///
/// A thread-local for the same reason the palette is one (§5.43): the
/// alternative is a parameter on every panel that would say the same thing every
/// time. Written once per frame, before anything draws.
pub fn set_width(width: f32) {
    CURRENT.with(|slot| *slot.borrow_mut() = width);
}

/// The width being drawn at.
pub fn width() -> f32 {
    CURRENT.with(|slot| *slot.borrow())
}

/// Centre an overlay of this size on the current width.
pub fn centred(w: f32, h: f32) -> Rect {
    Frame::sized(width(), height()).panel(w, h)
}

/// Centre horizontally, pin the top.
pub fn centred_at(w: f32, top: f32, h: f32) -> Rect {
    Frame::sized(width(), height()).panel_at(w, top, h)
}

/// Where the four fixed regions sit at a given width.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub width: f32,
    pub header: Rect,
    pub reels: Rect,
    pub wager: Rect,
    pub footer: Rect,
}

/// Gap between the panels and the screen edge, and between panels.
const MARGIN: f32 = 18.0;
/// The highest row an overlay panel may occupy.
///
/// The header is drawn behind every overlay and is never covered by one, so a
/// panel starting above this line slices the cabinet name in half — which three
/// of them did, unnoticed, until every screen was audited the same way (§5.50).
/// A test below holds this to the header rather than to the number.
pub const BELOW_HEADER: f32 = 84.0;
/// The wager panel's width at the design size. It holds a fixed set of controls
/// and gains nothing from being wider, so extra width goes to the reels.
const WAGER_WIDTH: f32 = 410.0;

impl Frame {
    /// A wide frame at this width, for the tests that describe landscape.
    #[cfg(test)]
    fn sized_wide(width: f32) -> Self {
        Self::sized(width, HEIGHT)
    }

    /// The four regions, for a logical space of this shape (§5.79).
    ///
    /// Two arrangements, chosen by which way up the space is. They share a
    /// header and a footer and differ in the middle, because that is the only
    /// part where "beside" and "below" is a real question.
    pub fn sized(width: f32, height: f32) -> Self {
        if height > HEIGHT {
            return Self::portrait(width, height);
        }

        let width = width.max(MIN_ASPECT * HEIGHT);
        let header = Rect::new(MARGIN, 16.0, width - MARGIN * 2.0, 64.0);
        let footer = Rect::new(MARGIN, 632.0, width - MARGIN * 2.0, 70.0);

        // The wager panel is anchored to the right edge at a fixed width: it is
        // a column of controls, and a wider one is only a column with more space
        // between the words. Everything left over is the reel window, which is
        // the part that genuinely benefits.
        let body_top = 96.0;
        let body_height = 520.0;
        let wager = Rect::new(
            width - MARGIN - WAGER_WIDTH,
            body_top,
            WAGER_WIDTH,
            body_height,
        );
        let reels = Rect::new(MARGIN, body_top, wager.x - MARGIN * 2.0, body_height);

        Self {
            width,
            header,
            reels,
            wager,
            footer,
        }
    }

    /// Turned: reels across the top, controls underneath (§5.79).
    ///
    /// The wager panel keeps the height it has in landscape rather than taking
    /// a share of whatever is going. It holds a fixed set of controls that were
    /// sized for a finger (§5.78) and they do not get better with more room;
    /// the reels do, so the reels take the slack. On a very tall screen that
    /// means a tall reel window, which is what a slot machine looks like.
    fn portrait(width: f32, height: f32) -> Self {
        // Two rows, not one. The header's four buttons and three badges want
        // 946 pixels beside the cabinet name and a turned frame is 720 across
        // — so the name and the badges take the top row and the buttons take
        // the one below, rather than the buttons sliding off the left edge,
        // which is what the landscape anchors did.
        let header = Rect::new(MARGIN, 16.0, width - MARGIN * 2.0, 118.0);
        let footer = Rect::new(MARGIN, height - 88.0, width - MARGIN * 2.0, 70.0);

        let body_top = header.bottom() + MARGIN;
        let body_bottom = footer.y - MARGIN;
        // The controls keep the height they have in landscape when there is
        // room, because they are a fixed set sized for a finger (§5.78) and
        // gain nothing from more. The reels take the rest, down to a floor —
        // below which a reel window stops being one.
        let wager_height = 520.0_f32.min((body_bottom - body_top) - 300.0).max(430.0);
        let wager = Rect::new(
            MARGIN,
            body_bottom - wager_height,
            width - MARGIN * 2.0,
            wager_height,
        );
        let reels = Rect::new(
            MARGIN,
            body_top,
            width - MARGIN * 2.0,
            (wager.y - MARGIN) - body_top,
        );

        Self {
            width,
            header,
            reels,
            wager,
            footer,
        }
    }

    /// Centre an overlay of this size, clamped to fit.
    ///
    /// Panels were written as absolute rects against a 1280 screen, which is a
    /// centre offset with the centre baked in. This is the same intent stated so
    /// it survives the screen changing shape.
    pub fn panel(&self, width: f32, height: f32) -> Rect {
        let width = width.min(self.width - MARGIN * 2.0);
        let height = height.min(HEIGHT - BELOW_HEADER - MARGIN);
        // A tall panel centres above the header and slices the cabinet name in
        // half. Three panels were doing it and a fourth only showed it under a
        // 40% translation, so the rule belongs here rather than in each of them
        // (§5.50).
        let top = ((HEIGHT - height) * 0.5).round().max(BELOW_HEADER);
        Rect::new(((self.width - width) * 0.5).round(), top, width, height)
    }

    /// Centre an overlay horizontally but pin its top, for panels that are tall
    /// enough that centring would push them under the header.
    pub fn panel_at(&self, width: f32, top: f32, height: f32) -> Rect {
        let width = width.min(self.width - MARGIN * 2.0);
        let height = height.min(HEIGHT - top - MARGIN);
        Rect::new(((self.width - width) * 0.5).round(), top, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_width_is_remembered() {
        set_width(1600.0);
        assert!((width() - 1600.0).abs() < 0.01);
        assert!((centred(400.0, 200.0).center().x - 800.0).abs() < 1.0);
        set_width(DESIGN_WIDTH);
    }

    #[test]
    fn a_sixteen_by_nine_window_gets_the_design_width() {
        assert!((logical_size(1920.0, 1080.0).0 - DESIGN_WIDTH).abs() < 1.0);
        assert!((logical_size(1280.0, 720.0).0 - DESIGN_WIDTH).abs() < 1.0);
    }

    #[test]
    fn a_wider_window_gets_a_wider_space_rather_than_a_bigger_picture() {
        // The whole point: more room, not a scaled-up copy.
        // 1920x900 is 2.13:1, inside the clamp. 2560x1080 is 2.37 and would be
        // clamped, which is a different property (below).
        let wide = logical_size(1920.0, 900.0).0;
        assert!(wide > DESIGN_WIDTH, "{}", wide);
        assert!((wide / HEIGHT - 1920.0 / 900.0).abs() < 0.01);
    }

    #[test]
    fn a_narrower_window_gets_a_narrower_space() {
        let narrow = logical_size(1024.0, 768.0).0;
        assert!(narrow < DESIGN_WIDTH, "{}", narrow);
    }

    #[test]
    fn extreme_shapes_are_clamped_rather_than_obeyed() {
        // Outside the clamp the letterbox comes back, which is the right
        // failure: bars are better than a layout that does not work.
        let absurdly_wide = logical_size(10_000.0, 500.0).0;
        assert!((absurdly_wide / HEIGHT - MAX_ASPECT).abs() < 0.01);

        // A tall window is no longer clamped into a wide layout — it gets a
        // turned one instead (§5.79). This used to assert the opposite, and
        // that assertion was the whole reason portrait could not exist.
        let (w, h) = logical_size(400.0, 900.0);
        assert_eq!(w, PORTRAIT_WIDTH);
        assert!(h > w, "a tall window produced a wide layout: {}x{}", w, h);
    }

    #[test]
    fn a_turned_frame_stacks_the_reels_above_the_controls() {
        let (w, h) = logical_size(810.0, 1080.0);
        let frame = Frame::sized(w, h);
        assert!(
            frame.reels.bottom() <= frame.wager.y,
            "the reels overlap the controls: {:?} {:?}",
            frame.reels,
            frame.wager
        );
        assert!(
            frame.header.bottom() <= frame.reels.y,
            "the header overlaps the reels"
        );
        assert!(
            frame.wager.bottom() <= frame.footer.y,
            "the controls overlap the footer"
        );
        assert_eq!(
            frame.reels.w, frame.wager.w,
            "a turned frame gives both panels the full width"
        );
    }

    /// Landscape must be untouched by all of this. Every panel in the game is
    /// positioned against it.
    #[test]
    fn a_wide_frame_is_exactly_what_it_was() {
        let old = Frame::sized_wide(DESIGN_WIDTH);
        let (w, h) = logical_size(1280.0, 720.0);
        assert_eq!((w, h), (DESIGN_WIDTH, HEIGHT));
        let now = Frame::sized(w, h);
        assert_eq!(old.header, now.header);
        assert_eq!(old.reels, now.reels);
        assert_eq!(old.wager, now.wager);
        assert_eq!(old.footer, now.footer);
        assert!(
            now.reels.bottom() > now.wager.y,
            "landscape stacked the panels instead of setting them side by side"
        );
    }

    /// The point of the turn: a control keeps its size in logical pixels, so on
    /// a narrower canvas it is a *larger* share of it.
    #[test]
    fn turning_makes_a_tablet_clear_the_touch_standard() {
        let (w, _) = logical_size(810.0, 1080.0);
        // 44 logical pixels, drawn into an 810 CSS-pixel canvas.
        let css = 44.0 * 810.0 / w;
        assert!(
            css >= 44.0,
            "a 44px control comes out at {:.1} CSS pixels on a tablet held upright",
            css
        );
    }

    #[test]
    fn a_degenerate_window_falls_back_rather_than_dividing_by_zero() {
        assert_eq!(logical_size(1280.0, 0.0).0, DESIGN_WIDTH);
        assert_eq!(logical_size(f32::NAN, 720.0).0, DESIGN_WIDTH);
    }

    /// Overlays clear the header at every width, so none of them can slice the
    /// cabinet name the way three did before §5.50 measured every screen.
    #[test]
    fn an_overlay_starting_at_the_line_clears_the_header() {
        for width in [960.0, DESIGN_WIDTH, 1680.0] {
            let frame = Frame::sized_wide(width);
            assert!(
                BELOW_HEADER >= frame.header.bottom(),
                "an overlay at {} cuts into a header ending at {} (width {})",
                BELOW_HEADER,
                frame.header.bottom(),
                width
            );
        }
        // And it leaves a usable panel behind, or the rule would be satisfied by
        // pushing every overlay off the bottom of the screen.
        const { assert!(HEIGHT - BELOW_HEADER >= 560.0) };
    }

    #[test]
    fn the_regions_tile_the_screen_without_overlapping() {
        for width in [960.0, DESIGN_WIDTH, 1680.0] {
            let frame = Frame::sized_wide(width);
            // Header above the body, body above the footer.
            assert!(frame.header.bottom() <= frame.reels.y);
            assert!(frame.reels.bottom() <= frame.footer.y);
            // Reels and wager side by side with a gap.
            assert!(
                frame.reels.right() < frame.wager.x,
                "{} against {}",
                frame.reels.right(),
                frame.wager.x
            );
            // Nothing off the edge.
            for rect in [frame.header, frame.reels, frame.wager, frame.footer] {
                assert!(rect.x >= 0.0);
                assert!(rect.right() <= width + 0.01, "{:?} at {}", rect, width);
                assert!(rect.bottom() <= HEIGHT + 0.01);
            }
        }
    }

    #[test]
    fn extra_width_goes_to_the_reels() {
        // The wager panel is a column of controls; a wider one is the same
        // column with more space between the words.
        let narrow = Frame::sized_wide(1100.0);
        let wide = Frame::sized_wide(1680.0);
        assert!((narrow.wager.w - wide.wager.w).abs() < 0.01);
        assert!(wide.reels.w > narrow.reels.w + 500.0);
    }

    #[test]
    fn the_wager_panel_stays_against_the_right_edge() {
        for width in [960.0, DESIGN_WIDTH, 1680.0] {
            let frame = Frame::sized_wide(width);
            assert!((frame.wager.right() - (width - MARGIN)).abs() < 0.01);
        }
    }

    #[test]
    fn an_overlay_is_centred_at_every_width() {
        for width in [960.0, DESIGN_WIDTH, 1680.0] {
            let frame = Frame::sized_wide(width);
            let panel = frame.panel(600.0, 400.0);
            assert!((panel.center().x - width * 0.5).abs() < 1.0, "{}", width);
            assert!((panel.center().y - HEIGHT * 0.5).abs() < 1.0);
        }
    }

    #[test]
    fn an_overlay_too_wide_for_the_screen_is_clamped_rather_than_clipped() {
        // A panel written at 1040 wide has to survive a 960-wide screen.
        let frame = Frame::sized_wide(960.0);
        let panel = frame.panel(1040.0, 400.0);
        assert!(panel.x >= 0.0);
        assert!(panel.right() <= 960.0);
    }

    #[test]
    fn a_top_pinned_overlay_keeps_its_top_and_stays_on_screen() {
        let frame = Frame::sized_wide(DESIGN_WIDTH);
        let panel = frame.panel_at(900.0, 44.0, 900.0);
        assert!((panel.y - 44.0).abs() < 0.01);
        assert!(panel.bottom() <= HEIGHT);
    }
}
