//! Drawing the line that won (§5.59).
//!
//! # "Ruby ×3 on line 17"
//!
//! That sentence has been under the reels since the game shipped, and line 17 is
//! a number. Twenty paylines cross the grid in twenty different shapes and the
//! player had no way to see any of them: the winning *cells* were lit, which
//! says which symbols paid, and nothing at all said what path they were on.
//!
//! The data has known all along. Each payline carries a name — "Middle", "Top",
//! "Bottom" — and the game was printing an index instead. A slot machine that
//! cannot show you its lines is asking to be taken on trust, which is the one
//! thing this cabinet has spent fifty-odd systems refusing to do.
//!
//! # The winning run is the line, and the rest of it is context
//!
//! A [`Win`](crate::engine::evaluate::Win) already carries the cells that formed
//! it, in reel order, because ways and cluster wins have no line to look up and
//! both models were made to report their own cells (§5.14). So the bright path
//! needs no payline lookup at all — it is a polyline through the cells that
//! paid, and it is correct by construction.
//!
//! The *rest* of the line is drawn dim behind it, from the payline definition.
//! A three-of-five win stops at reel three, and showing where the line would
//! have continued is what makes "line 17" a shape rather than a fragment.
//!
//! # One at a time
//!
//! Six simultaneous line wins drawn at once is a cat's cradle. They take turns,
//! on the same beat the win readout uses, so the line on screen is always the
//! win being named underneath it.

use crate::data::GameData;
use crate::engine::evaluate::{Win, WinSource};
use crate::state::GameSession;
use crate::ui::reels;
use macroquad::prelude::*;

/// How long each win holds the screen before the next takes over.
pub const BEAT: f32 = 1.6;

/// Line thickness, and the halo behind it that keeps it legible over art.
const WIDTH: f32 = 3.0;
const HALO: f32 = 6.0;

/// Which win is being shown right now, and its payline.
///
/// `None` when the cabinet has no lines, when nothing won, or when no line win
/// is among the wins — a ways cabinet lights cells and draws nothing here.
pub fn showing(session: &GameSession, ui_time: f32) -> Option<(&Win, usize)> {
    let outcome = session.last_outcome.as_ref()?;
    let lines: Vec<(&Win, usize)> = outcome
        .wins
        .iter()
        .filter_map(|win| match win.source {
            WinSource::Line(index) => Some((win, index)),
            _ => None,
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    let step = (ui_time / BEAT) as usize % lines.len();
    lines.get(step).copied()
}

/// Draw the line the named win was paid on.
pub fn draw(data: &GameData, session: &GameSession, shake: Vec2, ui_time: f32) {
    let Some((win, line)) = showing(session, ui_time) else {
        return;
    };
    let Some(payline) = data.paylines.get(line) else {
        return;
    };

    let colour = line_colour(line);
    // The whole shape first and dim, so the bright run reads as a part of it
    // rather than as the only thing there.
    let full: Vec<Vec2> = payline
        .rows
        .iter()
        .enumerate()
        .map(|(reel, row)| centre(data, reel, *row, shake))
        .collect();
    stroke(&full, Color::new(colour.r, colour.g, colour.b, 0.28), WIDTH);

    // Then the cells that actually paid. A pulse rather than a static line:
    // the win readout is counting up underneath and a still line beside a
    // moving number reads as a decoration.
    let pulse = 0.72 + 0.28 * (ui_time * 4.0).sin();
    let paid: Vec<Vec2> = win
        .cells
        .iter()
        .filter_map(|cell| cell_centre(data, *cell, shake))
        .collect();
    stroke(&paid, Color::new(0.0, 0.0, 0.0, 0.5 * pulse), WIDTH + HALO);
    stroke(
        &paid,
        Color::new(colour.r, colour.g, colour.b, pulse),
        WIDTH,
    );

    for point in &paid {
        draw_circle(
            point.x,
            point.y,
            WIDTH * 1.6,
            Color::new(0.0, 0.0, 0.0, 0.5),
        );
        draw_circle(
            point.x,
            point.y,
            WIDTH * 1.1,
            Color::new(colour.r, colour.g, colour.b, pulse),
        );
    }
}

/// What this cabinet calls the line, for the readout underneath.
///
/// The names are in `paylines.json` and were never once shown. "Bottom" is a
/// thing a player can picture; "line 17" is a thing they have to take on faith.
pub fn name(data: &GameData, line: usize) -> String {
    match data.paylines.get(line) {
        Some(payline) if !payline.name.is_empty() => payline.name.clone(),
        _ => format!("line {}", line + 1),
    }
}

fn stroke(points: &[Vec2], colour: Color, width: f32) {
    for pair in points.windows(2) {
        draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, width, colour);
    }
}

fn centre(data: &GameData, reel: usize, row: usize, shake: Vec2) -> Vec2 {
    let slot = reels::cell_slot_for(data, reel, row);
    vec2(slot.x + slot.w * 0.5, slot.y + slot.h * 0.5) + shake
}

fn cell_centre(data: &GameData, cell: usize, shake: Vec2) -> Option<Vec2> {
    let rows = data.config.row_count.max(1);
    let reel = cell / rows;
    let row = cell % rows;
    (reel < data.config.reel_count).then(|| centre(data, reel, row, shake))
}

/// A hue per line, so two wins on screen in successive beats are visibly
/// different lines rather than the same gold path moving.
pub fn line_colour(line: usize) -> Color {
    const WHEEL: [Color; 5] = [
        Color::new(1.00, 0.84, 0.40, 1.0),
        Color::new(0.55, 0.85, 1.00, 1.0),
        Color::new(0.60, 1.00, 0.65, 1.0),
        Color::new(1.00, 0.62, 0.55, 1.0),
        Color::new(0.85, 0.70, 1.00, 1.0),
    ];
    WHEEL[line % WHEEL.len()]
}

// Tests live in the crate-level integration harness.
