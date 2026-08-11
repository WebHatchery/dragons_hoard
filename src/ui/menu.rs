//! A way in that is not a key (§5.72).
//!
//! # Four screens a touch player could not open
//!
//! §5.45 gave the game touch input. §5.50 gave it a registry of every screen.
//! Nothing ever asked whether the two agreed, and they did not: four screens
//! could only be opened with a keyboard, because their only entry point was a
//! binding in `shortcuts.rs`.
//!
//! - the **Ledger** (§5.18), the panel that compares the player against the
//!   cabinet;
//! - the **session graph** (§5.32);
//! - the **sound panel** (§5.19), which is fairly called a development tool;
//! - and the **colour-vision panel** (§5.24) — an *accessibility* feature that
//!   could not be reached without a keyboard, which is close to the worst place
//!   for this fault to land.
//!
//! §5.70's session log was nearly a fifth: its only door was the screen a
//! session cap puts up, so a player who never set a cap could not see it.
//!
//! # Derived, not listed
//!
//! The rows come from [`Screen::in_menu`], which filters the registry rather
//! than repeating it. A screen added to `Screen::ALL` appears here the moment it
//! exists, so it can never again be registered, audited and unreachable at the
//! same time — which is exactly the state four of them were in.
//!
//! That is the same move as the audit sweep (§5.50) and the harness reading the
//! screen list rather than keeping one (§5.53). The registry earns its keep a
//! third time.

use crate::game::screens::Screen;
use crate::ui::nav::Nav;
use crate::ui::{frame, logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone, Pointer, Region,
    SurfaceStyle, TextStyle,
};

const PANEL: Color = Color::new(0.10, 0.09, 0.11, 1.0);
/// Rows across. Two columns of a ten-row list beats one column of twenty.
const COLUMNS: usize = 2;

pub fn draw(pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.88),
    );

    let screens: Vec<Screen> = Screen::in_menu().collect();
    let rows = screens.len().div_ceil(COLUMNS);
    let panel = frame::centred_at(
        720.0,
        frame::BELOW_HEADER + 20.0,
        116.0 + rows as f32 * 52.0,
    );
    let _region = Region::on(panel, PANEL);
    draw_surface(
        panel,
        &SurfaceStyle::new(PANEL)
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "Everything else",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );
    if virtual_button(
        crate::ui::close_button(panel),
        "Close",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleMenu);
    }

    let width = (panel.w - 48.0 - 12.0) / COLUMNS as f32;
    for (index, screen) in screens.iter().enumerate() {
        let slot = Rect::new(
            panel.x + 24.0 + (index % COLUMNS) as f32 * (width + 12.0),
            panel.y + 62.0 + (index / COLUMNS) as f32 * 52.0,
            width,
            44.0,
        );
        if virtual_button(
            slot,
            screen.label(),
            true,
            ButtonTone::Secondary,
            pointer,
            nav,
        ) {
            actions.push(UiAction::OpenScreen(*screen));
        }
    }

    draw_text_centered_in_box_ex(
        "Every one of these has a keyboard shortcut too; none of them needs one.",
        panel.x,
        panel.bottom() - 30.0,
        panel.w,
        20.0,
        TextStyle::new(14.0, palette::text_dim()),
    );
}

#[cfg(test)]
mod tests;
