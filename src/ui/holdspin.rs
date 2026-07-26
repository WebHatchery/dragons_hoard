//! The Dragon's Wrath board (§5.12).
//!
//! Unlike the Vault Pick, this is not a second screen — it is the reel window
//! itself, frozen. Coins lock into the very cells their eggs landed in, so the
//! player can see the round grew out of the spin rather than replacing it. The
//! panel behind stays visible for the same reason.
//!
//! There is nothing to click. The round advances on a beat and the renderer is
//! a pure readout, which is why this module returns no `UiAction` at all.

use crate::data::GameData;
use crate::state::holdspin::HoldSpinRound;
use crate::ui::naming;
use crate::ui::{logical_width, palette, reels, LOGICAL_HEIGHT};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, RectExt, Region, SurfaceStyle,
    TextStyle,
};

const CELL_PADDING: f32 = 8.0;
/// The frozen reel window and its banner. Both opaque — see `draw_banner`.
const WINDOW: Color = Color::new(0.10, 0.04, 0.01, 1.0);
const BANNER: Color = Color::new(0.16, 0.06, 0.01, 1.0);
/// How long a freshly locked coin flashes, in `ui_time` seconds.
const FLASH: f32 = 0.45;

pub fn draw(data: &GameData, round: &HoldSpinRound, ui_time: f32) {
    // Dim everything but the reel window. The board is the whole game while it
    // is up, and the bet controls beside it are inert.
    let grid = reels::grid_rect();
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        LOGICAL_HEIGHT,
        Color::new(0.02, 0.0, 0.0, 0.62),
    );
    let window = grid.inset(-10.0);
    // Opaque, and now declared so (§5.50). The reels and the win line under this
    // are gone, not dimmed, which is why the coins are not colliding with them.
    let _region = Region::on(window, WINDOW);
    draw_surface(
        window,
        &SurfaceStyle::new(WINDOW).with_border(3.0, palette::ember()),
    );

    let rows = data.config.row_count.max(1);
    let size = vec2(
        grid.w / data.config.reel_count.max(1) as f32,
        grid.h / rows as f32,
    );

    for index in 0..round.cell_count() {
        let cell = Rect::new(
            grid.x + (index / rows) as f32 * size.x,
            grid.y + (index % rows) as f32 * size.y,
            size.x,
            size.y,
        )
        .inset(CELL_PADDING);

        match round.cell(index) {
            Some(credits) => draw_coin(cell, credits, round.just_locked(index), ui_time),
            None => draw_empty(cell),
        }
    }

    draw_banner(round, grid);
}

/// A locked coin: a gold disc carrying its own credit value.
fn draw_coin(cell: Rect, credits: i64, fresh: bool, ui_time: f32) {
    // A coin that just landed flares and settles. The pulse is driven by
    // `ui_time` rather than wall-clock so a capture is reproducible.
    let flash = if fresh {
        (1.0 - (ui_time % FLASH) / FLASH).clamp(0.0, 1.0)
    } else {
        0.0
    };

    draw_surface(
        cell,
        &SurfaceStyle::new(Color::new(
            0.30 + 0.35 * flash,
            0.20 + 0.30 * flash,
            0.03,
            1.0,
        ))
        .with_border(2.0 + 2.0 * flash, palette::gold_bright()),
    );

    let centre = vec2(cell.x + cell.w * 0.5, cell.y + cell.h * 0.5);
    let radius = cell.w.min(cell.h) * 0.36;
    draw_circle(centre.x, centre.y, radius, palette::gold_dim());
    draw_circle(centre.x, centre.y, radius * 0.86, palette::gold());
    draw_circle_lines(centre.x, centre.y, radius, 2.0, palette::gold_bright());

    // The label sits on the gold disc, not on the dark window behind it. Saying
    // which surface it is on is the difference between 1.1:1 and legible — and
    // the bounds are the disc rather than the cell on purpose, so the claim
    // below that the type shrinks to fit is the thing being measured.
    let disc = Rect::new(
        centre.x - radius,
        centre.y - radius,
        radius * 2.0,
        radius * 2.0,
    );
    let _region = Region::on(disc, palette::gold());

    // Long numbers have to fit inside the disc, so the type shrinks with the
    // digit count rather than spilling over the rim.
    let label = naming::credits(credits);
    let size = match label.len() {
        0..=3 => 26.0,
        4 => 22.0,
        5 => 18.0,
        _ => 15.0,
    };
    draw_text_centered_in_box_ex(
        &label,
        cell.x,
        cell.y,
        cell.w,
        cell.h,
        TextStyle::new(size, Color::new(0.16, 0.09, 0.0, 1.0)),
    );
}

/// A cell still in play — deliberately plain, so the eye goes to the coins.
fn draw_empty(cell: Rect) {
    draw_surface(
        cell,
        &SurfaceStyle::new(Color::new(0.06, 0.05, 0.06, 1.0))
            .with_border(1.0, Color::new(0.0, 0.0, 0.0, 0.55)),
    );
}

/// Title, respins left and the running total, over the reel window.
fn draw_banner(round: &HoldSpinRound, grid: Rect) {
    let banner = Rect::new(grid.x, grid.y - 66.0, grid.w, 56.0);
    // Fully opaque: at 0.96 the jackpot ladder underneath read straight through
    // the banner and the two sets of numbers fought. That was fixed in pixels
    // and never told to the audit, so the ladder kept "colliding" with the
    // banner it sits behind — the region is the half that was missing (§5.50).
    let _banner_region = Region::on(banner, BANNER);
    draw_surface(
        banner,
        &SurfaceStyle::new(BANNER).with_border(2.0, palette::ember()),
    );

    draw_ui_text_ex(
        "THE DRAGON'S WRATH",
        banner.x + 16.0,
        banner.y + 24.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );
    draw_ui_text_ex(
        &format!("{} of {} coins locked", round.coins(), round.cell_count()),
        banner.x + 16.0,
        banner.y + 45.0,
        TextStyle::new(15.0, palette::text_dim()).params(),
    );

    // The respin counter is the tension in the feature, so it gets the loudest
    // treatment on the banner and goes red on the last one.
    let urgent = round.respins_left() <= 1;
    let respins = Rect::new(banner.right() - 210.0, banner.y + 8.0, 96.0, 40.0);
    draw_surface(
        respins,
        &SurfaceStyle::new(if urgent {
            Color::new(0.34, 0.06, 0.04, 1.0)
        } else {
            Color::new(0.08, 0.07, 0.10, 1.0)
        })
        .with_border(1.0, palette::gold_dim()),
    );
    // Words, not a glyph: macroquad's default font has no arrows, and a `↻`
    // here rendered as tofu — the same trap §7.1 records for emoji.
    draw_text_centered_in_box_ex(
        &format!("{} LEFT", round.respins_left()),
        respins.x,
        respins.y,
        respins.w,
        respins.h,
        TextStyle::new(19.0, palette::gold_bright()),
    );

    let total = Rect::new(banner.right() - 106.0, banner.y + 8.0, 96.0, 40.0);
    draw_surface(
        total,
        &SurfaceStyle::new(Color::new(0.06, 0.14, 0.07, 1.0)).with_border(1.0, palette::gold_dim()),
    );
    draw_text_centered_in_box_ex(
        &naming::credits(round.collected()),
        total.x,
        total.y,
        total.w,
        total.h,
        TextStyle::new(20.0, palette::text_bright()),
    );
}
