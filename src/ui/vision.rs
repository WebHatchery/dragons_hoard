//! The symbol set as four different people see it (§5.24).
//!
//! `legibility` proves a *number* — that no two symbols sharing a shape are too
//! close in colour. This shows the thing itself, one row per vision, so the
//! claim can be checked by looking rather than believed on the strength of a
//! matrix.
//!
//! Colours are simulated; the art is not redrawn. A dichromat sees the same
//! shapes as everyone else, and the shapes are what the fix relies on.

use crate::data::GameData;
use crate::ui::frame;
use crate::ui::legibility::{simulate, Vision};
use crate::ui::nav::Nav;
use crate::ui::{logical_width, palette, symbols, virtual_button, UiAction, LOGICAL_HEIGHT};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_ui_text_ex, ButtonTone, RectExt, Region, SurfaceStyle, TextStyle,
};

pub fn draw(data: &GameData, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.88),
    );

    let panel = frame::centred_at(1100.0, 96.0, 500.0);
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::on(panel, palette::stone());
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::stone())
            .with_border(2.0, palette::gold())
            .with_header(44.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "Symbols — colour vision",
        panel.x + 20.0,
        panel.y + 29.0,
        TextStyle::new(20.0, palette::gold_bright()).params(),
    );

    let count = data.symbols.iter().count().max(1);
    let cell = ((panel.w - 190.0) / count as f32).min(76.0);
    let row_height = (panel.h - 70.0) / Vision::ALL.len() as f32;

    for (row, vision) in Vision::ALL.iter().enumerate() {
        let top = panel.y + 56.0 + row as f32 * row_height;
        draw_ui_text_ex(
            vision.label(),
            panel.x + 20.0,
            top + row_height * 0.5,
            TextStyle::new(15.0, palette::text_dim()).params(),
        );

        for (index, (_, def)) in data.symbols.iter().enumerate() {
            let rect = Rect::new(
                panel.x + 160.0 + index as f32 * cell,
                top,
                cell,
                row_height - 8.0,
            )
            .inset(3.0);

            // Only the palette is simulated. The art is drawn from the same
            // routines the reels use, so what is on screen here is what is on
            // screen there.
            let seen = simulate(def.color, *vision);
            let shifted = crate::data::SymbolDef {
                color: seen,
                ..def.clone()
            };
            draw_surface(
                rect,
                &SurfaceStyle::new(Color::new(0.06, 0.055, 0.06, 1.0))
                    .with_border(1.0, Color::new(0.0, 0.0, 0.0, 0.5)),
            );
            symbols::draw(&shifted, rect, 0.0);
        }
    }

    draw_ui_text_ex(
        "Every symbol has its own shape. Colour reinforces it; nothing depends on it.",
        panel.x + 20.0,
        panel.bottom() - 14.0,
        TextStyle::new(14.0, palette::text_dim()).params(),
    );

    if virtual_button(
        Rect::new(panel.right() - 120.0, panel.y + 8.0, 100.0, 28.0),
        "Close",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleVision);
    }
}
