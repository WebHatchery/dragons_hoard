//! Every line this cabinet pays on, drawn (§5.60).
//!
//! # Learning a line by winning on it is not learning
//!
//! §5.59 put the winning payline on the grid and gave it its name back, so
//! "Ruby ×3 on Mid Trough" is now a sentence with a picture attached. That
//! answers the question *after* it has been asked. A player who wants to know
//! what the twenty lines are — before one of them pays, or to understand why a
//! near miss was not a win — still had nowhere to look.
//!
//! The paytable is the obvious place and does not have the room: it ends with
//! about a hundred spare pixels, and twenty diagrams need five times that. A
//! screen of its own is not a consolation prize here, it is the right shape for
//! the content, and it comes with a property this project cares about — being in
//! [`Screen::ALL`](crate::game::screens::Screen) means the layout, contrast,
//! collision and touch audits pick it up without anyone remembering to add it
//! (§5.50).
//!
//! # The same colours the reels use
//!
//! A line is drawn here in exactly the colour it will be drawn in over the
//! grid, from the same function. That is the whole point of the screen: the
//! player sees a blue zig-zag called Mid Trough here, and when it pays they see
//! a blue zig-zag over the reels. Two independent palettes would have made this
//! a decoration rather than a reference.

use crate::data::GameData;
use crate::ui::nav::Nav;
use crate::ui::{frame, logical_width, palette, paylines, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone,
    Pointer, Region, SurfaceStyle, TextStyle,
};

const PANEL: Color = Color::new(0.09, 0.085, 0.10, 1.0);
/// Diagrams per row. Five across leaves each one wide enough that a five-reel
/// grid is still readable at a glance.
pub const COLUMNS: usize = 5;

pub fn draw(data: &GameData, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        crate::ui::frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.88),
    );

    let panel = frame::centred_at(1180.0, frame::BELOW_HEADER, 600.0);
    let _region = Region::on(panel, PANEL);
    draw_surface(
        panel,
        &SurfaceStyle::new(PANEL)
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        &title(data),
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
        actions.push(UiAction::ToggleLines);
    }

    // A cabinet without paylines is not a broken screen, it is a cabinet that
    // pays differently — and saying which is more use than an empty panel.
    if data.paylines.is_empty() {
        draw_text_block(
            &empty_note(data),
            panel.x + 24.0,
            panel.y + 80.0,
            panel.w - 48.0,
            120.0,
            18.0,
            5.0,
            palette::text(),
        );
        return;
    }

    let rows = data.paylines.len().div_ceil(COLUMNS);
    let cell = vec2(
        (panel.w - 48.0) / COLUMNS as f32,
        (panel.h - 92.0) / rows as f32,
    );
    for index in 0..data.paylines.len() {
        let slot = Rect::new(
            panel.x + 24.0 + (index % COLUMNS) as f32 * cell.x,
            panel.y + 68.0 + (index / COLUMNS) as f32 * cell.y,
            cell.x,
            cell.y,
        );
        draw_diagram(data, index, slot);
    }
}

pub fn title(data: &GameData) -> String {
    match data.paylines.len() {
        0 => "Lines".to_owned(),
        count => format!("The {} lines", count),
    }
}

pub fn empty_note(data: &GameData) -> String {
    format!(
        "{} has no paylines. Wins are read straight off the grid instead — press R for how this \
         cabinet pays.",
        data.config.display_name
    )
}

/// One line: a miniature of the grid with the line's own path on it.
fn draw_diagram(data: &GameData, index: usize, slot: Rect) {
    let Some(payline) = data.paylines.get(index) else {
        return;
    };
    let reels = data.config.reel_count.max(1);
    let rows = data.config.row_count.max(1);

    // Sized to fit whichever way round the slot is, and centred in it, so the
    // grid stays square-ish on a narrow window as well as a wide one.
    let pip = ((slot.w - 24.0) / reels as f32)
        .min((slot.h - 34.0) / rows as f32)
        .max(4.0);
    let board = Rect::new(
        slot.x + (slot.w - pip * reels as f32) * 0.5,
        slot.y + 4.0,
        pip * reels as f32,
        pip * rows as f32,
    );

    for reel in 0..reels {
        for row in 0..rows {
            let cell = Rect::new(
                board.x + reel as f32 * pip,
                board.y + row as f32 * pip,
                pip - 2.0,
                pip - 2.0,
            );
            let on = payline
                .rows
                .get(reel)
                .is_some_and(|line_row| *line_row == row);
            draw_rectangle(
                cell.x,
                cell.y,
                cell.w,
                cell.h,
                if on {
                    Color::new(0.20, 0.19, 0.14, 1.0)
                } else {
                    Color::new(0.11, 0.105, 0.115, 1.0)
                },
            );
        }
    }

    // The path itself, in the colour the reels will draw it in.
    let colour = paylines::line_colour(index);
    let points: Vec<Vec2> = payline
        .rows
        .iter()
        .enumerate()
        .map(|(reel, row)| {
            vec2(
                board.x + (reel as f32 + 0.5) * pip - 1.0,
                board.y + (*row as f32 + 0.5) * pip - 1.0,
            )
        })
        .collect();
    for pair in points.windows(2) {
        draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, 2.0, colour);
    }
    for point in &points {
        draw_circle(point.x, point.y, 2.6, colour);
    }

    draw_text_centered_in_box_ex(
        &paylines::name(data, index),
        slot.x,
        board.bottom() + 4.0,
        slot.w,
        20.0,
        TextStyle::new(14.0, palette::text()),
    );
}

// Tests live in the crate-level integration harness.
