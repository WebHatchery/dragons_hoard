//! The hint bar (§5.28).
//!
//! A single line above the footer, with one key to dismiss it. Deliberately not
//! a modal, a popover or an overlay: a hint that interrupts play is a tutorial,
//! and a hint the player has to close is a demand rather than an offer.
//!
//! It sits where the reels are not, so it never covers the thing the player is
//! actually watching.

use crate::state::hints::HintDef;
use crate::ui::nav::{self, Nav};
use crate::ui::{logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{draw_surface, draw_text_block, ButtonTone, SurfaceStyle};

pub fn draw(hint: &HintDef, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    // Sits on the footer's shortcut line — the small grey text that lists every
    // key and that nobody reads, which is the whole reason this exists. While a
    // hint is up it takes that space rather than fighting it for room.
    let bar = Rect::new(478.0, 668.0, logical_width() - 496.0, 30.0);
    draw_surface(
        bar,
        &SurfaceStyle::new(Color::new(0.16, 0.11, 0.03, 0.96))
            .with_border(1.0, palette::gold_dim())
            .with_left_accent(3.0, palette::ember()),
    );

    let dismiss = Rect::new(bar.right() - 88.0, bar.y + 3.0, 80.0, 24.0);

    // Fitted to the space before the button rather than set at 14px and run
    // under it. On a narrow screen (§5.46) the bar shrinks and the hint does
    // not — which the collision check reported at 446px² (§5.47), and which no
    // amount of looking at a 16:9 capture would ever have shown.
    draw_text_block(
        &hint.text,
        bar.x + 16.0,
        bar.y + 4.0,
        (dismiss.x - 12.0) - (bar.x + 16.0),
        24.0,
        14.0,
        0.0,
        palette::text_bright(),
    );

    if virtual_button(dismiss, "Got it", true, ButtonTone::Secondary, pointer, nav) {
        actions.push(UiAction::DismissHint);
    }
    let _ = nav::focus_ring;
}
