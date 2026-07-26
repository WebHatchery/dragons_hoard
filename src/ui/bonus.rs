//! The Vault Pick board.
//!
//! Twelve chests; pick until three are empty. Unrevealed cells are drawn from
//! nothing but their index — the renderer is only ever handed
//! `revealed_cell`, so it cannot leak the board even by accident.

use crate::state::bonus::{BonusCell, BonusRound};
use crate::ui::nav::{self, Nav};
use crate::ui::{logical_width, palette, symbols, UiAction, LOGICAL_HEIGHT};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, RectExt, Region, SurfaceStyle,
    TextStyle,
};

const COLUMNS: usize = 4;
const CELL: f32 = 128.0;
const GAP: f32 = 14.0;
/// Opaque on purpose: the board hides the reels rather than tinting them.
const BOARD: Color = Color::new(0.14, 0.10, 0.04, 1.0);

pub fn draw(
    round: &BonusRound,
    chest: Option<&crate::data::SymbolDef>,
    pointer: Pointer,
    ui_time: f32,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.86),
    );

    let rows = round.board_size().div_ceil(COLUMNS);
    let grid_w = COLUMNS as f32 * CELL + (COLUMNS as f32 - 1.0) * GAP;
    let grid_h = rows as f32 * CELL + (rows as f32 - 1.0) * GAP;
    let panel = Rect::new(
        (logical_width() - grid_w) * 0.5 - 32.0,
        (LOGICAL_HEIGHT - grid_h) * 0.5 - 92.0,
        grid_w + 64.0,
        grid_h + 152.0,
    );

    // The board is opaque and covers the reel window, the jackpot ladder and the
    // win line. Saying so is what lets the audit tell a real collision from the
    // layer underneath (§5.50) — and is why this screen was never measurable.
    let _region = Region::on(panel, BOARD);
    draw_surface(
        panel,
        &SurfaceStyle::new(BOARD)
            .with_border(2.0, palette::gold())
            .with_header(52.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "THE VAULT PICK",
        panel.x + 22.0,
        panel.y + 34.0,
        TextStyle::new(22.0, palette::gold_bright()).params(),
    );
    draw_text_centered_in_box_ex(
        &format!(
            "{} of {} empty — keep picking",
            round.blanks_found(),
            round.blanks_needed()
        ),
        panel.right() - 340.0,
        panel.y + 12.0,
        320.0,
        30.0,
        TextStyle::new(17.0, palette::text()),
    );

    let origin = vec2(
        panel.x + 32.0,
        panel.y + 52.0 + (panel.h - 52.0 - grid_h - 56.0) * 0.5,
    );
    for index in 0..round.board_size() {
        let cell = Rect::new(
            origin.x + (index % COLUMNS) as f32 * (CELL + GAP),
            origin.y + (index / COLUMNS) as f32 * (CELL + GAP),
            CELL,
            CELL,
        );
        if draw_cell(round, chest, index, cell, pointer, ui_time, nav) {
            actions.push(UiAction::PickBonus(index));
        }
    }

    draw_text_centered_in_box_ex(
        &format!(
            "Collected {} credits  ({}% of the hoard)",
            round.running_credits(),
            round.collected_permille() / 10
        ),
        panel.x,
        panel.bottom() - 46.0,
        panel.w,
        32.0,
        TextStyle::new(20.0, palette::gold_bright()),
    );
}

/// Returns true when this chest was clicked.
fn draw_cell(
    round: &BonusRound,
    chest: Option<&crate::data::SymbolDef>,
    index: usize,
    cell: Rect,
    pointer: Pointer,
    ui_time: f32,
    nav: &mut Nav,
) -> bool {
    match round.revealed_cell(index) {
        Some(BonusCell::Prize(permille)) => {
            draw_surface(
                cell,
                &SurfaceStyle::new(Color::new(0.20, 0.16, 0.05, 1.0))
                    .with_border(2.0, palette::gold()),
            );
            draw_text_centered_in_box_ex(
                &format!("{}", round.base() * permille / 1000),
                cell.x,
                cell.y - 6.0,
                cell.w,
                cell.h,
                TextStyle::new(30.0, palette::gold_bright()),
            );
            draw_text_centered_in_box_ex(
                &format!("{}%", permille / 10),
                cell.x,
                cell.y + cell.h * 0.5,
                cell.w,
                cell.h * 0.4,
                TextStyle::new(15.0, palette::text_dim()),
            );
            false
        }
        Some(BonusCell::Blank) => {
            draw_surface(
                cell,
                &SurfaceStyle::new(Color::new(0.10, 0.07, 0.07, 1.0))
                    .with_border(1.0, Color::new(0.5, 0.2, 0.2, 0.8)),
            );
            draw_text_centered_in_box_ex(
                "EMPTY",
                cell.x,
                cell.y,
                cell.w,
                cell.h,
                TextStyle::new(20.0, Color::new(0.72, 0.38, 0.36, 1.0)),
            );
            false
        }
        None => {
            // Closed. A hover lift is the only affordance; the contents are not
            // available to this function at all.
            let hit = nav.control(cell, !round.is_finished(), pointer);
            let hovered =
                !round.is_finished() && pointer.hovering_over(cell) || pointer.pressing(cell);
            let glow = 0.5 + 0.5 * (ui_time * 2.2 + index as f32 * 0.6).sin();
            draw_surface(
                cell,
                &SurfaceStyle::new(if hovered {
                    Color::new(0.22, 0.17, 0.07, 1.0)
                } else {
                    Color::new(0.13, 0.11, 0.06, 1.0)
                })
                .with_border(
                    if hovered { 3.0 } else { 1.0 },
                    Color::new(
                        palette::gold().r,
                        palette::gold().g,
                        palette::gold().b,
                        0.35 + 0.45 * glow,
                    ),
                ),
            );
            if let Some(def) = chest {
                symbols::draw(def, cell.inset(18.0), if hovered { 0.6 } else { 0.0 });
            }
            // Through the nav (§5.27), not a raw hit test. An open board holds
            // the game, so a chest that could only be clicked was a soft-lock
            // for anyone without a pointer.
            if hit.focused {
                nav::focus_ring(cell);
            }
            hit.activated
        }
    }
}
