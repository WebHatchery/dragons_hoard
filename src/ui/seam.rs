//! The Seam banner and its highlights (§5.80).
//!
//! Almost nothing is drawn here, and that is the design. The reel window is
//! already showing the seam's board — `GameSession::display_grid` hands the
//! round's grid to the ordinary reel renderer while a rite runs — so the
//! symbols the player is watching change are the real ones, drawn by the same
//! code that draws every other spin. Overlaying a second board would mean two
//! renderers that could disagree about what a jade looks like.
//!
//! What is left is the part the grid cannot say: which cells the seam holds,
//! which of them moved on this beat, what the rite is called, and what the
//! board is worth so far. There is nothing to click; like the respin round
//! (§5.12) this module returns no `UiAction` at all.

use crate::data::GameData;
use crate::state::seam::SeamRound;
use crate::ui::naming;
use crate::ui::{palette, reels};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, RectExt, Region, SurfaceStyle,
    TextStyle,
};

/// The banner over the reel window. Opaque for the same reason the Wrath's is —
/// the jackpot ladder sits behind it and two sets of numbers fight.
const BANNER: Color = Color::new(0.06, 0.10, 0.04, 1.0);
/// How long a cell that just turned flashes, in `ui_time` seconds.
const FLASH: f32 = 0.5;

pub fn draw(data: &GameData, round: &SeamRound, ui_time: f32) {
    mark_cells(data, round, ui_time);
    draw_banner(data, round);
}

/// Ring the cells the seam holds, and flare the ones this beat took.
///
/// A ring rather than a fill: the symbol underneath is the whole point, and the
/// first version of this drew a translucent wash over it that made a gold and a
/// copper indistinguishable at cell size — which is the exact failure §5.24
/// exists to catch.
fn mark_cells(data: &GameData, round: &SeamRound, ui_time: f32) {
    let grid = round.grid();
    let flare = (1.0 - (ui_time % FLASH) / FLASH).clamp(0.0, 1.0);

    for reel in 0..grid.reel_count() {
        for row in 0..grid.rows_on(reel) {
            let flat = grid.index(reel, row);
            if !round.cells().contains(&flat) {
                continue;
            }
            let cell = reels::cell_slot_for(data, reel, row).inset(4.0);
            let fresh = round.just_changed(flat);
            let colour = if fresh {
                Color::new(
                    palette::gold_bright().r,
                    palette::gold_bright().g,
                    palette::gold_bright().b,
                    0.55 + 0.45 * flare,
                )
            } else {
                Color::new(
                    palette::jade().r,
                    palette::jade().g,
                    palette::jade().b,
                    0.75,
                )
            };
            draw_rectangle_lines(
                cell.x,
                cell.y,
                cell.w,
                cell.h,
                if fresh { 4.0 + 2.0 * flare } else { 2.0 },
                colour,
            );
        }
    }
}

/// What is happening, how much of it is left, and what it is worth so far.
fn draw_banner(data: &GameData, round: &SeamRound) {
    let grid = reels::grid_rect();
    // Sized off the jackpot ladder it covers rather than off the reel grid.
    //
    // Both because `grid_rect` squares its cells off — so it is narrower than
    // the ladder above it and a banner drawn to the grid leaves the outermost
    // plate showing beside it — and because the audit only treats a surface as
    // *hiding* text when it clears the whole line (§5.47). A banner whose
    // bottom edge lands on the ladder's baseline is reported as cutting the
    // numbers in half, which is the finding this arrived with, and the five
    // pixels of margin at each end are what make the claim "this covers the
    // ladder" true rather than nearly true.
    let strip = reels::jackpot_strip_rect();
    let banner = Rect::new(strip.x, strip.y - 5.0, strip.w, strip.h + 10.0);
    debug_assert!(banner.bottom() < grid.y, "the banner is over the reels");
    let _region = Region::on(banner, BANNER);
    draw_surface(
        banner,
        &SurfaceStyle::new(BANNER).with_border(2.0, palette::jade()),
    );

    draw_ui_text_ex(
        &round.rite().name.to_uppercase(),
        banner.x + 16.0,
        banner.y + 24.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );
    // The symbol's own name, because "the seam" means nothing until the player
    // is told which treasure it is made of — and the rite may have changed it.
    draw_ui_text_ex(
        &format!(
            "{} cells of {}",
            round.cells().len(),
            data.symbols.get(round.symbol()).name.to_lowercase()
        ),
        banner.x + 16.0,
        banner.y + 45.0,
        TextStyle::new(15.0, palette::text_dim()).params(),
    );

    let moves = Rect::new(banner.right() - 210.0, banner.y + 8.0, 96.0, 40.0);
    draw_surface(
        moves,
        &SurfaceStyle::new(Color::new(0.08, 0.07, 0.10, 1.0)).with_border(1.0, palette::gold_dim()),
    );
    draw_text_centered_in_box_ex(
        &format!("{} LEFT", round.steps_left()),
        moves.x,
        moves.y,
        moves.w,
        moves.h,
        TextStyle::new(19.0, palette::gold_bright()),
    );

    let standing = Rect::new(banner.right() - 106.0, banner.y + 8.0, 96.0, 40.0);
    draw_surface(
        standing,
        &SurfaceStyle::new(Color::new(0.06, 0.14, 0.07, 1.0)).with_border(1.0, palette::gold_dim()),
    );
    draw_text_centered_in_box_ex(
        &naming::credits(round.standing(data)),
        standing.x,
        standing.y,
        standing.w,
        standing.h,
        TextStyle::new(20.0, palette::text_bright()),
    );
}
