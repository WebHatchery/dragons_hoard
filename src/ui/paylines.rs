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
const BEAT: f32 = 1.6;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::engine::evaluate::SpinOutcome;

    fn data() -> GameData {
        GameData::load().unwrap()
    }

    /// Every line has to name a row that exists on every reel, or the polyline
    /// would be drawn off the grid.
    #[test]
    fn every_payline_stays_on_the_grid() {
        let data = data();
        for (index, payline) in data.paylines.iter().enumerate() {
            assert_eq!(
                payline.rows.len(),
                data.config.reel_count,
                "line {} covers {} reels of {}",
                index + 1,
                payline.rows.len(),
                data.config.reel_count
            );
            for row in &payline.rows {
                assert!(
                    *row < data.config.row_count,
                    "line {} names row {} on a {}-row grid",
                    index + 1,
                    row,
                    data.config.row_count
                );
            }
        }
    }

    /// The names are the point of this; a blank one would silently fall back to
    /// the number it is replacing.
    #[test]
    fn every_line_has_a_name_worth_showing() {
        let data = data();
        for index in 0..data.paylines.len() {
            let shown = name(&data, index);
            assert!(!shown.is_empty());
            assert!(
                !shown.starts_with("line "),
                "line {} has no name and fell back to its index",
                index + 1
            );
        }
    }

    /// Several wins at once take turns, and every one of them gets a turn.
    ///
    /// Drawing six lines together is a cat's cradle; drawing only the first
    /// would quietly hide the rest, which is worse — a player would see one
    /// line and a payout that did not match it.
    #[test]
    fn every_line_win_gets_its_turn() {
        let data = data();
        let mut session = crate::state::GameSession::new(&data, 0x11E5);
        session.last_outcome = Some(SpinOutcome {
            wins: (0..3)
                .map(|line| Win {
                    source: WinSource::Line(line),
                    symbol: 0,
                    count: 3,
                    credits: 10,
                    cells: vec![0, 3, 6],
                })
                .collect(),
            ..SpinOutcome::default()
        });

        let mut seen = std::collections::HashSet::new();
        // Two full cycles, sampled inside each beat rather than on its edge.
        for step in 0..6 {
            let at = BEAT * step as f32 + BEAT * 0.5;
            let (_, line) = showing(&session, at).expect("a line win is showing");
            seen.insert(line);
        }
        assert_eq!(seen.len(), 3, "only {:?} of three lines were shown", seen);
    }

    /// A cabinet without paylines draws nothing here, whatever it won.
    #[test]
    fn a_ways_win_draws_no_line() {
        let data = data();
        let mut session = crate::state::GameSession::new(&data, 0x11E5);
        session.last_outcome = Some(SpinOutcome {
            wins: vec![Win {
                source: WinSource::Ways(6),
                symbol: 0,
                count: 3,
                credits: 10,
                cells: vec![0, 3, 6],
            }],
            ..SpinOutcome::default()
        });
        assert!(showing(&session, 0.0).is_none());
        assert!(showing(&session, 12.5).is_none());
    }

    /// Colours cycle rather than running out, and neighbouring lines differ —
    /// two wins shown one after the other must not look like one line moving.
    #[test]
    fn neighbouring_lines_are_drawn_in_different_colours() {
        for line in 0..20 {
            let here = line_colour(line);
            let next = line_colour(line + 1);
            assert!(
                (here.r - next.r).abs() + (here.g - next.g).abs() + (here.b - next.b).abs() > 0.2,
                "lines {} and {} are the same colour",
                line,
                line + 1
            );
        }
    }
}
