//! Reel window rendering: symbol cells, spin scroll and blur, win highlights.
//!
//! Pure view code — it reads the session and draws; it never mutates anything.
//! While a reel is turning it renders straight from the strip at a fractional
//! position, so the symbols scrolling past are the real ones either side of the
//! stop rather than a decorative loop.

use crate::data::GameData;
use crate::state::{jackpot, GameSession};
use crate::ui::{naming, palette, symbols};
use macroquad::prelude::*;
use macroquad_toolkit::strip::blur_offsets;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, RectExt, SurfaceStyle, TextStyle,
};

const CELL_PADDING: f32 = 8.0;
/// Copies drawn per blurred reel. Enough to read as motion, few enough that
/// five reels of it stay cheap.
const BLUR_PASSES: usize = 5;
/// Radians per second of the winning-cell highlight pulse.
const PULSE_RATE: f32 = 6.0;

pub fn panel_rect() -> Rect {
    // Whatever is left once the wager panel has its column (§5.46). A wider
    // window gives the reels more room, which is the part that benefits.
    crate::ui::frame::Frame::new(crate::ui::frame::width()).reels
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
    cell_size_on(data, data.config.row_count)
}

/// Cell size on a reel showing `rows` symbols.
///
/// A shifting cabinet (§5.20) gives every reel its own height, and each one
/// divides the *same* window between however many symbols it is showing — so a
/// two-row reel draws two tall cells beside a seven-row reel drawing seven
/// short ones. That is what makes the shape of the board readable at a glance.
fn cell_size_on(data: &GameData, rows: usize) -> Vec2 {
    let rect = grid_rect();
    vec2(
        rect.w / data.config.reel_count.max(1) as f32,
        rect.h / rows.max(1) as f32,
    )
}

/// A cell's slot, for anything outside this module that needs to draw on the
/// grid — the payline overlay (§5.59) needs cell centres and must get them from
/// the same arithmetic the symbols are drawn with, or the line would miss.
pub fn cell_slot_for(data: &GameData, reel: usize, row: usize) -> Rect {
    cell_slot(data, reel, row as f32)
}

/// Full cell slot, before the padding that separates the drawn tiles.
fn cell_slot(data: &GameData, reel: usize, row: f32) -> Rect {
    cell_slot_on(data, reel, row, data.config.row_count)
}

fn cell_slot_on(data: &GameData, reel: usize, row: f32, rows: usize) -> Rect {
    let rect = grid_rect();
    let size = cell_size_on(data, rows);
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
            palette::ember(),
            feature_title(data, session),
        )
    } else {
        (
            palette::stone(),
            palette::gold_dim(),
            String::from("The Vault"),
        )
    };

    let style = SurfaceStyle::new(surface)
        .with_border(if feature { 2.0 } else { 1.0 }, chrome)
        .with_inner_border(4.0, 1.0, Color::new(1.0, 0.85, 0.5, 0.06))
        .with_header(44.0, palette::stone_header())
        .with_header_divider(1.0, chrome);
    draw_surface(panel, &style);

    draw_ui_text_ex(
        &title,
        panel.x + 18.0,
        panel.y + 30.0,
        TextStyle::new(
            19.0,
            if feature {
                palette::gold_bright()
            } else {
                palette::gold()
            },
        )
        .params(),
    );

    draw_jackpot_ladder(data, session, shake, ui_time);

    let bounds = grid_rect().offset(shake);
    let highlights = winning_cells(data, session);
    let clearing = session.cascade_clearing();
    let pulse = 0.55 + 0.45 * (ui_time * PULSE_RATE).sin();

    for reel in 0..data.config.reel_count {
        match spinning_position(session, reel) {
            Some(position) => {
                draw_spinning_reel(data, session, reel, position, shake, bounds);
            }
            None => draw_resting_reel(data, session, reel, &highlights, pulse, shake, clearing),
        }
    }

    // After the cells, not before — drawn first it sat *underneath* the top-right
    // symbol and was invisible.
    draw_cascade_badge(session, shake);
    // Over the symbols so the path reads, under the summary that names it
    // (§5.59).
    crate::ui::paylines::draw(data, session, shake, ui_time);
    draw_win_summary(data, session, bounds);
}

/// The banner across the reel cabinet during a feature.
///
/// On a refining cabinet (§5.21) it names what the feature has burned off the
/// strips, because the escalation is invisible otherwise — the reels simply feel
/// luckier and the player has no way to know why.
fn feature_title(data: &GameData, session: &GameSession) -> String {
    let base = format!(
        "The Vault — Free Spins  ·  wilds expand  ·  line wins x{}",
        data.freespins.multiplier.max(1)
    );
    let Some(refine) = data.freespins.refine.as_ref() else {
        return base;
    };
    let burned = session.free_spins.as_ref().map_or(0, |state| state.burned);
    if burned == 0 {
        return base;
    }

    format!(
        "{}  ·  burned {}",
        base,
        naming::burned(data, &refine.order, burned)
    )
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
        // A pot the whole floor feeds is lit differently rather than labelled:
        // the plate is barely a hundred pixels wide and a second word on it
        // would not survive 130% text, let alone a translation (§5.57). The
        // rules panel carries the explanation.
        let shared = data
            .jackpots
            .tiers
            .get(index)
            .is_some_and(|tier| tier.shared);
        let border = if shared {
            Color::new(
                palette::ember().r,
                palette::ember().g,
                palette::ember().b,
                0.55 + 0.45 * shimmer,
            )
        } else {
            Color::new(
                palette::gold().r,
                palette::gold().g,
                palette::gold().b,
                0.35 + 0.5 * rank * shimmer,
            )
        };

        draw_surface(
            plate,
            &SurfaceStyle::new(fill)
                .with_border(if shared { 2.0 } else { 1.0 }, border)
                .with_top_highlight(2.0, Color::new(1.0, 0.86, 0.45, 0.15 + 0.35 * rank)),
        );
        draw_text_centered_in_box_ex(
            &name.to_uppercase(),
            plate.x,
            plate.y + 2.0,
            plate.w,
            18.0,
            TextStyle::new(13.0, palette::text_dim()),
        );
        draw_text_centered_in_box_ex(
            &naming::credits(*credits),
            plate.x,
            plate.y + 16.0,
            plate.w,
            26.0,
            TextStyle::new(
                21.0,
                if rank > 0.6 {
                    palette::gold_bright()
                } else {
                    palette::gold()
                },
            ),
        );
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
    clearing: &[usize],
) {
    // `display_grid`, not `grid`: a reel that has landed shows what it landed
    // on even while its neighbours are still turning.
    let grid = session.display_grid();
    let rows = grid.rows_on(reel);
    for row in 0..rows {
        let cell = cell_slot_on(data, reel, row as f32, rows)
            .offset(shake)
            .inset(CELL_PADDING);
        let index = grid.index(reel, row);
        // Mid-cascade the cells about to be cleared are lit at full brightness
        // rather than pulsed: they are on their way out, and a pulse would read
        // as "still in play" (§5.15).
        let doomed = clearing.contains(&index);
        let winning = highlights.get(index).copied().unwrap_or(false);
        draw_symbol_cell(
            data,
            cell,
            grid.at(reel, row),
            if doomed {
                1.0
            } else if winning {
                pulse
            } else {
                0.0
            },
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
    // The height this reel will land on; decided at commit like everything else.
    let rows = session.display_grid().rows_on(reel).max(1);
    let spinner = session.phase.spinner();
    let blurred = spinner.is_some_and(|spinner| spinner.is_blurred(reel));
    let anticipating = spinner.is_some_and(|spinner| spinner.is_held(reel));

    if anticipating {
        draw_anticipation_frame(data, reel, shake, bounds);
    }

    // Real motion blur: the strip is drawn several times across the distance it
    // covers in one frame, each pass at a fraction of the alpha. Previously a
    // fast reel simply dropped its labels, which read as "the art vanished"
    // rather than "the reel is moving".
    let smear = if blurred {
        spinner.map_or(0.0, |spinner| spinner.blur_symbols(reel))
    } else {
        0.0
    };
    let passes = if smear > 0.05 { BLUR_PASSES } else { 1 };
    let alpha = 1.0 / passes as f32;

    // Tiles are laid once, at the reel's true position, under every art pass.
    // Stacking one translucent tile per pass summed to near-white and bleached
    // the whole vault; drawing them on the leading pass instead left them
    // trailing the art they were supposed to sit beneath. The reel face is a
    // surface, and a surface does not smear — only what is printed on it does.
    draw_strip_pass(
        data,
        reel,
        position,
        StripPass {
            shake,
            bounds,
            alpha: 1.0,
            layer: StripLayer::Tiles,
            rows,
        },
    );

    // Spread either side of the reel's current position, so the streak covers
    // the ground it crossed this frame.
    for lag in blur_offsets(smear, passes) {
        draw_strip_pass(
            data,
            reel,
            position + lag,
            StripPass {
                shake,
                bounds,
                alpha,
                layer: StripLayer::Art,
                rows,
            },
        );
    }
}

/// Which half of a cell a strip pass draws. Splitting them is what keeps motion
/// blur from bleaching the reels: the tiles go down once, the art many times.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StripLayer {
    Tiles,
    Art,
}

/// Everything one pass of a spinning strip needs to place itself.
///
/// Bundled because the argument list reached eight once reels stopped all being
/// the same height (§5.20), and six of them are the same for every pass of a
/// given reel anyway.
#[derive(Clone, Copy)]
struct StripPass {
    shake: Vec2,
    bounds: Rect,
    alpha: f32,
    layer: StripLayer,
    /// Rows this reel is landing on.
    rows: usize,
}

/// One pass of the strip at a given fractional position.
fn draw_strip_pass(data: &GameData, reel: usize, position: f32, pass: StripPass) {
    let StripPass {
        shake,
        bounds,
        alpha,
        layer,
        rows,
    } = pass;
    let strip = &data.reels[reel];
    // The height this reel is landing on, not the configured maximum: on a
    // shifting cabinet (§5.20) the two differ, and spinning at one then settling
    // at the other makes every reel jump as it stops.
    let visible_rows = rows as i32;
    let top = position.floor();
    let offset = position - top;

    for row in -1..=visible_rows {
        let slot = cell_slot_on(data, reel, row as f32 - offset, rows).offset(shake);
        let Some(visible) = clip(slot.inset(CELL_PADDING), bounds) else {
            continue;
        };

        let index = (top as i64 + row as i64).rem_euclid(strip.len() as i64) as usize;
        // A clipped cell still skips its art — half a dragon drawn into the
        // panel edge reads as a glitch. Blur is no longer a reason to skip it.
        let whole = (visible.h - slot.inset(CELL_PADDING).h).abs() < 0.5;
        match layer {
            StripLayer::Tiles => draw_cell_tile(data, visible, strip[index], 0.0, alpha),
            // A clipped cell keeps its tile but skips its art — half a dragon
            // sliced into the panel edge reads as a glitch.
            StripLayer::Art if whole => draw_cell_art(data, visible, strip[index], 0.0, alpha),
            StripLayer::Art => {}
        }
    }
}

/// The held-back reel gets a frame of its own, so the pause reads as the game
/// making something of the moment rather than as a stutter.
fn draw_anticipation_frame(data: &GameData, reel: usize, shake: Vec2, bounds: Rect) {
    let size = cell_size(data);
    let column = Rect::new(
        grid_rect().offset(shake).x + reel as f32 * size.x,
        bounds.y,
        size.x,
        bounds.h,
    );
    draw_surface(
        column.inset(2.0),
        &SurfaceStyle::new(Color::new(0.30, 0.14, 0.03, 0.55)).with_border(3.0, palette::ember()),
    );
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
    draw_cell_tile(data, rect, symbol, highlight, 1.0);
    if detailed {
        draw_cell_art(data, rect, symbol, highlight, 1.0);
    }
}

/// The tile a symbol sits on: stone, tinted by the symbol's colour. Drawn once
/// per cell however many blur passes follow.
fn draw_cell_tile(data: &GameData, rect: Rect, symbol: usize, highlight: f32, alpha: f32) {
    let def = data.symbols.get(symbol);
    let tint = Color::new(def.color[0], def.color[1], def.color[2], 1.0);
    let strength = 0.14 + 0.24 * highlight;
    let fill = Color::new(
        0.055 + tint.r * strength,
        0.05 + tint.g * strength,
        0.065 + tint.b * strength,
        alpha,
    );

    let border = if highlight > 0.0 {
        Color::new(
            palette::gold_bright().r,
            palette::gold_bright().g,
            palette::gold_bright().b,
            (0.35 + 0.65 * highlight) * alpha,
        )
    } else {
        Color::new(0.0, 0.0, 0.0, 0.55 * alpha)
    };

    draw_surface(
        rect,
        &SurfaceStyle::new(fill)
            .with_border(if highlight > 0.0 { 3.0 } else { 1.0 }, border)
            .with_top_highlight(3.0, Color::new(1.0, 1.0, 1.0, 0.12 * alpha)),
    );
}

/// The symbol itself. This is the only part that repeats across blur passes, so
/// a fast reel reads as streaked art on a solid reel face.
fn draw_cell_art(data: &GameData, rect: Rect, symbol: usize, highlight: f32, alpha: f32) {
    let def = data.symbols.get(symbol);
    if symbols::draw_with_alpha(def, rect, highlight, alpha) {
        return;
    }

    // Only reached when a symbol names art the renderer does not have.
    let text_color = if highlight > 0.5 {
        Color::new(
            palette::gold_bright().r,
            palette::gold_bright().g,
            palette::gold_bright().b,
            alpha,
        )
    } else {
        Color::new(
            palette::text_bright().r,
            palette::text_bright().g,
            palette::text_bright().b,
            alpha,
        )
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
            TextStyle::new(16.0, palette::text_dim()),
        );
        return;
    }

    let Some(outcome) = session.last_outcome.as_ref() else {
        return;
    };

    let text = naming::wins(data, outcome);

    draw_text_centered_in_box_ex(
        &text,
        strip.x,
        strip.y,
        strip.w,
        strip.h,
        TextStyle::new(16.0, palette::text_dim()),
    );
}

/// The climbing multiplier over a cascading board (§5.15). Absent at x1, so a
/// chain that has not yet built shows nothing rather than a badge saying x1.
/// Where the cascade multiplier is drawn, whatever is behind it.
///
/// Split out so the rule below can be checked without a window: the geometry is
/// the thing that was wrong, and the drawing is not.
fn cascade_badge_rect() -> Rect {
    let panel = panel_rect();
    Rect::new(panel.right() - 140.0, panel.y + 9.0, 118.0, 34.0)
}

fn draw_cascade_badge(session: &GameSession, shake: Vec2) {
    let Some(multiplier) = session.cascade_multiplier() else {
        return;
    };
    // In the panel's title row, not over the top-right symbol (§5.61).
    //
    // It sat on a cell for six iterations with the design document naming it
    // each time — "a real cabinet would find it somewhere of its own" — and the
    // collision audit could not see it, because §5.47 measures text against
    // *text* and a symbol is art. The badge covering a symbol is not a near
    // miss the player is entitled to see; on a cascading cabinet it is a cell
    // that is about to be cleared, which is exactly what they are watching.
    let badge = cascade_badge_rect().offset(shake);

    draw_surface(
        badge,
        &SurfaceStyle::new(Color::new(0.34, 0.12, 0.02, 0.95)).with_border(2.0, palette::ember()),
    );
    draw_text_centered_in_box_ex(
        &format!("x{}", multiplier),
        badge.x,
        badge.y,
        badge.w,
        badge.h,
        TextStyle::new(26.0, palette::gold_bright()),
    );
}

/// Flat `reel * rows + row` mask of cells that took part in a win.
fn winning_cells(data: &GameData, session: &GameSession) -> Vec<bool> {
    let grid = &session.grid;
    let mut mask = vec![false; grid.cell_count()];

    let Some(outcome) = session.last_outcome.as_ref() else {
        return mask;
    };

    // Wins carry their own cells, so this works for paylines and for ways
    // without knowing which model produced them.
    for win in &outcome.wins {
        for cell in &win.cells {
            if let Some(lit) = mask.get_mut(*cell) {
                *lit = true;
            }
        }
    }

    if outcome.scatter_credits > 0 {
        if let Some(scatter) = data.symbols.scatter() {
            for (reel, row, symbol) in grid.cells() {
                if symbol == scatter {
                    mask[grid.index(reel, row)] = true;
                }
            }
        }
    }

    mask
}

#[cfg(test)]
mod chrome {
    use super::*;

    /// Nothing the panel draws for itself may sit on the symbol window.
    ///
    /// The grid is what the player is watching. The cascade badge sat on the
    /// top-right cell for six iterations with §15 naming it every time, and no
    /// gate could see it: §5.47's collision audit compares text against *text*,
    /// and a symbol is art. The rule is about geometry rather than pixels, so
    /// it belongs here where it can be checked without a window (§5.61).
    #[test]
    fn no_panel_furniture_is_drawn_over_the_reels() {
        for width in [960.0, 1280.0, 1680.0] {
            crate::ui::frame::set_width(crate::ui::frame::logical_width(width, 720.0));
            let grid = grid_rect();
            for (what, rect) in [
                ("the cascade multiplier badge", cascade_badge_rect()),
                ("the jackpot ladder", jackpot_strip_rect()),
            ] {
                let overlaps = rect.x < grid.right()
                    && rect.right() > grid.x
                    && rect.y < grid.bottom()
                    && rect.bottom() > grid.y;
                assert!(
                    !overlaps,
                    "{} is drawn over the symbol window at {}px: {:?} against {:?}",
                    what, width, rect, grid
                );
            }
        }
        crate::ui::frame::set_width(crate::ui::frame::DESIGN_WIDTH);
    }

    /// And it has to be somewhere a player will look, not merely somewhere else.
    #[test]
    fn the_badge_stays_inside_the_panel() {
        for width in [960.0, 1280.0, 1680.0] {
            crate::ui::frame::set_width(crate::ui::frame::logical_width(width, 720.0));
            let panel = panel_rect();
            let badge = cascade_badge_rect();
            assert!(badge.x >= panel.x, "{:?} left of {:?}", badge, panel);
            assert!(badge.right() <= panel.right(), "{:?}", badge);
            assert!(badge.bottom() <= panel.bottom(), "{:?}", badge);
        }
        crate::ui::frame::set_width(crate::ui::frame::DESIGN_WIDTH);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::evaluate::{SpinOutcome, Win, WinSource};

    #[test]
    fn winning_cells_cover_exactly_the_paying_run() {
        let data = GameData::load().unwrap();
        let chest = data.symbols.index_of("chest").unwrap();

        // Payline index 0 is the middle row, paid as three chests. The win
        // carries its own cells now, so the fixture states them directly rather
        // than relying on a payline lookup.
        let rows = data.config.row_count;
        let mut session = GameSession::new(&data, 1);
        session.last_outcome = Some(SpinOutcome {
            wins: vec![Win {
                source: WinSource::Line(0),
                symbol: chest,
                count: 3,
                credits: 40,
                cells: (0..3).map(|reel| reel * rows + 1).collect(),
            }],
            win_credits: 40,
            total_credits: 40,
            ..SpinOutcome::default()
        });

        let mask = winning_cells(&data, &session);
        let rows = session.grid.rows_on(0);
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
