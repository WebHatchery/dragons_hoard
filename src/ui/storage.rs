//! Save and recovery controls, separated from the wager decision.

use crate::ui::nav::Nav;
use crate::ui::{frame, logical_width, palette, virtual_button, UiAction, UiContext};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_ui_text_ex, ButtonTone, Pointer, Region, SurfaceStyle,
    TextStyle,
};

const PANEL: Color = Color::new(0.09, 0.085, 0.10, 1.0);

pub fn draw(ctx: &UiContext<'_>, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.80),
    );

    let panel = frame::centred_at(620.0, frame::BELOW_HEADER + 20.0, 356.0);
    let _region = Region::on(panel, PANEL);
    draw_surface(
        panel,
        &SurfaceStyle::new(PANEL)
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "Save & Recovery",
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
        actions.push(UiAction::ToggleStorage);
    }

    let storage_ready = ctx.session.is_settled() && !ctx.session.in_free_spins();
    draw_text_block(
        if storage_ready {
            "Storage is available while the cabinet is settled."
        } else {
            "Finish the current spin or feature before changing storage."
        },
        panel.x + 20.0,
        panel.y + 78.0,
        panel.w - 40.0,
        42.0,
        16.0,
        4.0,
        palette::text(),
    );

    let row_w = (panel.w - 52.0) / 2.0;
    let first_row = panel.y + 128.0;
    if virtual_button(
        Rect::new(panel.x + 20.0, first_row, row_w, 52.0),
        "Save",
        storage_ready,
        ButtonTone::Positive,
        pointer,
        nav,
    ) {
        actions.push(UiAction::Save);
    }
    if virtual_button(
        Rect::new(panel.x + 32.0 + row_w, first_row, row_w, 52.0),
        "Load",
        storage_ready && ctx.save_exists,
        ButtonTone::Primary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::Load);
    }

    let second_row = first_row + 64.0;
    if virtual_button(
        Rect::new(panel.x + 20.0, second_row, row_w, 52.0),
        "New Game",
        ctx.session.phase.is_idle(),
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::NewGame);
    }
    if virtual_button(
        Rect::new(panel.x + 32.0 + row_w, second_row, row_w, 52.0),
        "Delete Save",
        ctx.session.phase.is_idle() && ctx.save_exists,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::DeleteSave);
    }

    draw_text_block(
        &format!(
            "Save slot: {}  ·  Load: {}",
            if ctx.save_exists { "ready" } else { "empty" },
            if ctx.save_exists {
                "restores the slot"
            } else {
                "no save yet"
            }
        ),
        panel.x + 20.0,
        panel.bottom() - 34.0,
        panel.w - 40.0,
        20.0,
        14.0,
        0.0,
        palette::text_dim(),
    );
}
