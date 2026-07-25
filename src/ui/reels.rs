//! Reel window rendering: symbol cells, spin scroll and blur, win highlights.
//!
//! Pure view code — it reads the session and draws; it never mutates anything.
//! While a reel is turning it renders straight from the strip at a fractional
//! position, so the symbols scrolling past are the real ones either side of the
//! stop rather than a decorative loop.

use crate::data::GameData;
use crate::state::{jackpot, GameSession};
use crate::ui::{palette, symbols};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, RectExt, SurfaceStyle, TextStyle,
};

const CELL_PADDING: f32 = 8.0;
/// Radians per second of the winning-cell highlight pulse.
const PULSE_RATE: f32 = 6.0;

pub fn panel_rect() -> Rect {
    Rect::new(18.0, 96.0, 812.0, 520.0)
}

/// The jackpot ladder, sitting between the panel title and the reels — where a
/// real cabinet puts it, and where the player sees it on every spin.
pub fn jackpot_strip_rect() -> Rect {
    let panel = panel_rect();
    Rect::new(panel.x + 22.0, panel.y + 52.0, panel.w - 44.0, 46.0)
}

/// The symbol window inside the reels panel.
pub fn grid_rect() -> Rect {
    let panel = panel_rect();
    let strip = jackpot_strip_rect();
    let top = strip.bottom() + 10.0;
    Rect::new(
        panel.x + 22.0,
        top,
        panel.w - 44.0,
        panel.bottom() - 36.0 - top,
    )
}

fn cell_size(data: &GameData) -> Vec2 {
    let rect = grid_rect();
    vec2(
        rect.w / data.config.reel_count.max(1) as f32,
        rect.h / data.config.row_count.max(1) as f32,
    )
}

/// Full cell slot, before the padding that separates the drawn tiles.
fn cell_slot(data: &GameData, reel: usize, row: f32) -> Rect {
    let rect = grid_rect();
    let size = cell_size(data);
    Rect::new(
        rect.x + reel as f32 * size.x,
        rect.y + row * size.y,
        size.x,
        size.y,
    )
}

pub fn cell_center(data: &GameData, reel: usize, row: usize) -> Vec2 {
    let slot = cell_slot(data, reel, row as f32);
    vec2(slot.x + slot.w * 0.5, slot.y + slot.h * 0.5)
}

pub fn grid_center() -> Vec2 {
    let rect = grid_rect();
    vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5)
}

/// Bottom edge of a reel, where its landing dust puffs up from.
pub fn reel_foot(data: &GameData, reel: usize) -> Vec2 {
    let rect = grid_rect();
    let size = cell_size(data);
    vec2(rect.x + (reel as f32 + 0.5) * size.x, rect.bottom() - 4.0)
}

pub fn draw_reels(data: &GameData, session: &GameSession, shake: Vec2, ui_time: f32) {
    // Free spins re-skin the whole cabinet, so the player can tell at a glance
    // that the rules on screen are not the base-game rules.
    let feature = session.in_free_spins();
    let panel = panel_rect().offset(shake);
    let (surface, chrome, title) = if feature {
        (
            Color::new(0.14, 0.070, 0.045, 0.97),
            palette::EMBER,
            "The Vault — Free Spins  ·  wilds expand  ·  line wins doubled",
        )
    } else {
        (palette::STONE, palette::GOLD_DIM, "The Vault")
    };

    let style = SurfaceStyle::new(surface)
        .with_border(if feature { 2.0 } else { 1.0 }, chrome)
        .with_inner_border(4.0, 1.0, Color::new(1.0, 0.85, 0.5, 0.06))
        .with_header(44.0, palette::STONE_HEADER)
        .with_header_divider(1.0, chrome);
    draw_surface(panel, &style);

    draw_ui_text_ex(
        title,
        panel.x + 18.0,
        panel.y + 30.0,
        TextStyle::new(
            19.0,
            if feature {
                palette::GOLD_BRIGHT
            } else {
                palette::GOLD
            },
        )
        .params(),
    );

    draw_jackpot_ladder(data, session, shake, ui_time);

    let bounds = grid_rect().offset(shake);
    let highlights = winning_cells(data, session);
    let pulse = 0.55 + 0.45 * (ui_time * PULSE_RATE).sin();

    for reel in 0..data.config.reel_count {
        match spinning_position(session, reel) {
            Some(position) => draw_spinning_reel(data, session, reel, position, shake, bounds),
            None => draw_resting_reel(data, session, reel, &highlights, pulse, shake),
        }
    }

    draw_win_summary(data, session, bounds);
}

/// The progressive ladder: one plate per tier, richest on the right, each
/// showing what it would pay right now. The plates brighten with tier so the
/// eye lands on the Grand.
fn draw_jackpot_ladder(data: &GameData, session: &GameSession, shake: Vec2, ui_time: f32) {
    let rows = jackpot::ladder(&data.jackpots, &session.jackpots);
    if rows.is_empty() {
        return;
    }

    let strip = jackpot_strip_rect().offset(shake);
    let gap = 8.0;
    let width = (strip.w - gap * (rows.len() as f32 - 1.0)) / rows.len() as f32;
    // A slow shimmer so the ladder reads as live rather than painted on.
    let shimmer = 0.5 + 0.5 * (ui_time * 1.6).sin();

    for (index, (name, credits)) in rows.iter().enumerate() {
        let plate = Rect::new(
            strip.x + index as f32 * (width + gap),
            strip.y,
            width,
            strip.h,
        );
        // 0.0 for the smallest tier up to 1.0 for the richest.
        let rank = index as f32 / (rows.len() as f32 - 1.0).max(1.0);
        let fill = Color::new(
            0.09 + 0.10 * rank,
            0.075 + 0.075 * rank,
            0.05 + 0.02 * rank,
            1.0,
        );
        let border = Color::new(
            palette::GOLD.r,
            palette::GOLD.g,
            palette::GOLD.b,
            0.35 + 0.5 * rank * shimmer,
        );

        draw_surface(
            plate,
            &SurfaceStyle::new(fill)
                .with_border(1.0, border)
                .with_top_highlight(2.0, Color::new(1.0, 0.86, 0.45, 0.15 + 0.35 * rank)),
        );
        draw_text_centered_in_box_ex(
            &name.to_uppercase(),
            plate.x,
            plate.y + 2.0,
            plate.w,
            18.0,
            TextStyle::new(13.0, palette::TEXT_DIM),
        );
        draw_text_centered_in_box_ex(
            &format_credits(*credits),
            plate.x,
            plate.y + 16.0,
            plate.w,
            26.0,
            TextStyle::new(
                21.0,
                if rank > 0.6 {
                    palette::GOLD_BRIGHT
                } else {
                    palette::GOLD
                },
            ),
        );
    }
}

/// Thousands separators — a five-figure Grand is unreadable without them.
fn format_credits(credits: i64) -> String {
    let digits = credits.abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if credits < 0 {
        format!("-{}", out)
    } else {
        out
    }
}

/// Fractional strip position when this reel is still turning.
fn spinning_position(session: &GameSession, reel: usize) -> Option<f32> {
    let spinner = session.phase.spinner()?;
    spinner.is_moving(reel).then(|| spinner.position(reel))
}

fn draw_resting_reel(
    data: &GameData,
    session: &GameSession,
    reel: usize,
    highlights: &[bool],
    pulse: f32,
    shake: Vec2,
) {
    let rows = session.grid.row_count();
    for row in 0..rows {
        let cell = cell_slot(data, reel, row as f32)
            .offset(shake)
            .inset(CELL_PADDING);
        let winning = highlights.get(reel * rows + row).copied().unwrap_or(false);
        draw_symbol_cell(
            data,
            cell,
            session.grid.at(reel, row),
            if winning { pulse } else { 0.0 },
            true,
        );
    }
}

/// Draws the strip scrolling past, clipped to the window. One extra row is
/// drawn above and below so the window is never seen to be empty.
fn draw_spinning_reel(
    data: &GameData,
    session: &GameSession,
    reel: usize,
    position: f32,
    shake: Vec2,
    bounds: Rect,
) {
    let strip = &data.reels[reel];
    let rows = data.config.row_count as i32;
    let top = position.floor();
    let offset = position - top;
    let blurred = session
        .phase
        .spinner()
        .is_some_and(|spinner| spinner.is_blurred(reel));

    for row in -1..=rows {
        let slot = cell_slot(data, reel, row as f32 - offset).offset(shake);
        let Some(visible) = clip(slot.inset(CELL_PADDING), bounds) else {
            continue;
        };

        let index = (top as i64 + row as i64).rem_euclid(strip.len() as i64) as usize;
        let whole = (visible.h - slot.inset(CELL_PADDING).h).abs() < 0.5;
        draw_symbol_cell(data, visible, strip[index], 0.0, whole && !blurred);
    }
}

/// Intersection of a cell with the reel window, or `None` when fully outside.
fn clip(cell: Rect, bounds: Rect) -> Option<Rect> {
    let top = cell.y.max(bounds.y);
    let bottom = cell.bottom().min(bounds.bottom());
    (bottom - top > 1.0).then(|| Rect::new(cell.x, top, cell.w, bottom - top))
}

/// A cell: a stone tile tinted by the symbol, with the symbol's art on top.
/// `highlight` is 0.0 for a resting cell up to 1.0 at the peak of a win pulse.
/// `detailed` is false for cells that are clipped or blurred, where the art
/// would be sliced or smeared — those keep a stronger tile tint instead, so a
/// spinning reel still reads as a band of colours.
fn draw_symbol_cell(data: &GameData, rect: Rect, symbol: usize, highlight: f32, detailed: bool) {
    let def = data.symbols.get(symbol);
    let tint = Color::new(def.color[0], def.color[1], def.color[2], 1.0);
    let strength = if detailed {
        0.14 + 0.24 * highlight
    } else {
        0.42
    };
    let fill = Color::new(
        0.055 + tint.r * strength,
        0.05 + tint.g * strength,
        0.065 + tint.b * strength,
        1.0,
    );

    let border = if highlight > 0.0 {
        Color::new(
            palette::GOLD_BRIGHT.r,
            palette::GOLD_BRIGHT.g,
            palette::GOLD_BRIGHT.b,
            0.35 + 0.65 * highlight,
        )
    } else {
        Color::new(0.0, 0.0, 0.0, 0.55)
    };

    draw_surface(
        rect,
        &SurfaceStyle::new(fill)
            .with_border(if highlight > 0.0 { 3.0 } else { 1.0 }, border)
            .with_top_highlight(3.0, Color::new(1.0, 1.0, 1.0, 0.12)),
    );

    if !detailed {
        return;
    }

    if symbols::draw(def, rect, highlight) {
        return;
    }

    // Only reached when a symbol names art the renderer does not have.
    let text_color = if highlight > 0.5 {
        palette::GOLD_BRIGHT
    } else {
        palette::TEXT_BRIGHT
    };
    draw_text_centered_in_box_ex(
        &def.short,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        TextStyle::new(30.0, text_color),
    );
}

fn draw_win_summary(data: &GameData, session: &GameSession, rect: Rect) {
    let strip = Rect::new(rect.x, rect.bottom() + 6.0, rect.w, 24.0);

    if session.phase.spinner().is_some() {
        draw_text_centered_in_box_ex(
            "Spinning...",
            strip.x,
            strip.y,
            strip.w,
            strip.h,
            TextStyle::new(16.0, palette::TEXT_DIM),
        );
        return;
    }

    let Some(outcome) = session.last_outcome.as_ref() else {
        return;
    };

    let text = if outcome.line_wins.is_empty() && outcome.scatter_credits == 0 {
        "No win — spin again".to_owned()
    } else {
        let mut parts: Vec<String> = outcome
            .line_wins
            .iter()
            .take(3)
            .map(|win| {
                format!(
                    "{} x{} on line {}",
                    data.symbols.get(win.symbol).short,
                    win.count,
                    data.paylines[win.line].id
                )
            })
            .collect();
        if outcome.line_wins.len() > 3 {
            parts.push(format!("+{} more", outcome.line_wins.len() - 3));
        }
        if outcome.scatter_credits > 0 {
            parts.push(format!("{} scatters", outcome.scatter_count));
        }
        parts.join("   ")
    };

    draw_text_centered_in_box_ex(
        &text,
        strip.x,
        strip.y,
        strip.w,
        strip.h,
        TextStyle::new(16.0, palette::TEXT_DIM),
    );
}

/// Flat `reel * rows + row` mask of cells that took part in a win.
fn winning_cells(data: &GameData, session: &GameSession) -> Vec<bool> {
    let grid = &session.grid;
    let mut mask = vec![false; grid.reel_count() * grid.row_count()];

    let Some(outcome) = session.last_outcome.as_ref() else {
        return mask;
    };

    for win in &outcome.line_wins {
        let Some(payline) = data.paylines.get(win.line) else {
            continue;
        };
        for (reel, row) in payline.rows.iter().enumerate().take(win.count) {
            mask[reel * grid.row_count() + row] = true;
        }
    }

    if outcome.scatter_credits > 0 {
        if let Some(scatter) = data.symbols.scatter() {
            for (reel, row, symbol) in grid.cells() {
                if symbol == scatter {
                    mask[reel * grid.row_count() + row] = true;
                }
            }
        }
    }

    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::evaluate::{LineWin, SpinOutcome};

    #[test]
    fn winning_cells_cover_exactly_the_paying_run() {
        let data = GameData::load().unwrap();
        let chest = data.symbols.index_of("chest").unwrap();

        // Payline index 0 is the middle row, paid as three chests.
        let mut session = GameSession::new(&data, 1);
        session.last_outcome = Some(SpinOutcome {
            line_wins: vec![LineWin {
                line: 0,
                symbol: chest,
                count: 3,
                credits: 40,
            }],
            line_credits: 40,
            total_credits: 40,
            ..SpinOutcome::default()
        });

        let mask = winning_cells(&data, &session);
        let rows = session.grid.row_count();
        for reel in 0..3 {
            assert!(mask[reel * rows + 1], "reel {} should be highlighted", reel);
        }
        // The run stopped at reel 3, and rows off the payline stay dark.
        assert!(!mask[3 * rows + 1]);
        assert!(!mask[0]);
        assert!(!mask[2]);
    }

    #[test]
    fn winning_cells_are_empty_before_the_first_spin() {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data, 1);

        assert!(winning_cells(&data, &session).iter().all(|cell| !cell));
    }

    #[test]
    fn every_cell_sits_inside_the_reel_window() {
        let data = GameData::load().unwrap();
        let bounds = grid_rect();

        for reel in 0..data.config.reel_count {
            for row in 0..data.config.row_count {
                let center = cell_center(&data, reel, row);
                assert!(bounds.contains(center), "cell {},{} escaped", reel, row);
            }
        }
    }

    #[test]
    fn a_cell_scrolled_off_the_top_is_clipped_away() {
        let bounds = grid_rect();
        let above = Rect::new(bounds.x, bounds.y - 200.0, 100.0, 100.0);
        let straddling = Rect::new(bounds.x, bounds.y - 40.0, 100.0, 100.0);

        assert!(clip(above, bounds).is_none());
        let clipped = clip(straddling, bounds).unwrap();
        assert_eq!(clipped.y, bounds.y);
        assert!(clipped.h < 100.0);
    }
}
