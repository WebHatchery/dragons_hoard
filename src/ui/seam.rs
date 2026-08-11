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

use crate::data::{GameData, RiteDef, RiteKind};
use crate::state::seam::SeamRound;
use crate::ui::naming;
use crate::ui::nav::Nav;
use crate::ui::{palette, reels, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone, Pointer, RectExt,
    Region, SurfaceStyle, TextStyle,
};

/// The banner over the reel window. Opaque for the same reason the Wrath's is —
/// the jackpot ladder sits behind it and two sets of numbers fight.
const BANNER: Color = Color::new(0.06, 0.10, 0.04, 1.0);
/// How long a cell that just turned flashes, in `ui_time` seconds.
const FLASH: f32 = 0.5;

#[allow(clippy::too_many_arguments)]
pub fn draw(
    data: &GameData,
    round: &SeamRound,
    choice: &[RiteDef],
    frame: Rect,
    pointer: Pointer,
    ui_time: f32,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    mark_cells(data, round, ui_time);
    if choice.is_empty() {
        draw_banner(data, round);
    } else {
        draw_choice(data, round, choice, frame, pointer, actions, nav);
    }
}

/// The decision, drawn over the **wager column** rather than over the board.
///
/// The board is the evidence. A gilding pays a multiple of what the grid is
/// already worth and nothing at all on a grid that won nothing, so a panel that
/// covered the symbols would be asking for a decision with the evidence hidden.
/// The wager controls are the one large area of the screen that is inert while a
/// seam waits — the bet cannot change and the reels will not turn — so the
/// choice takes their column and the whole grid stays in view.
///
/// It also buys the room the choice needs. Three buttons 62 logical pixels tall
/// clear 44 CSS pixels on a 960-wide canvas (§5.78), and nothing shorter does;
/// the strip above the reels that this was first drawn in is 94 pixels top to
/// bottom and could not hold one of them, let alone three and a heading.
fn draw_choice(
    data: &GameData,
    round: &SeamRound,
    choice: &[RiteDef],
    frame: Rect,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    let plate = frame;
    let _region = Region::on(plate, BANNER);
    draw_surface(
        plate,
        &SurfaceStyle::new(BANNER).with_border(2.0, palette::gold_bright()),
    );

    draw_ui_text_ex(
        "THE SEAM WAITS",
        plate.x + 18.0,
        plate.y + 34.0,
        TextStyle::new(23.0, palette::gold_bright()).params(),
    );
    draw_ui_text_ex(
        &format!(
            "{} cells of {}",
            round.cells().len(),
            data.symbols.get(round.symbol()).name.to_lowercase()
        ),
        plate.x + 18.0,
        plate.y + 58.0,
        TextStyle::new(16.0, palette::text_dim()).params(),
    );
    // What the board is worth **right now**, which is the one fact the decision
    // turns on and the one the player had to work out for themselves (§5.86).
    //
    // A gilding is a multiple of exactly this figure and nothing else, so a
    // board paying nothing makes it worth nothing — and the game used to leave
    // that sitting in a win line under the reels for the player to notice. It is
    // not a hint or an odds display: it is a number the game already knows and
    // already paid, stated where the decision is made.
    let baseline = round.baseline();
    draw_ui_text_ex(
        &if baseline > 0 {
            format!("The board is paying {}", naming::credits(baseline))
        } else {
            "The board is paying nothing".to_owned()
        },
        plate.x + 18.0,
        plate.y + 82.0,
        TextStyle::new(
            15.0,
            if baseline > 0 {
                palette::gold()
            } else {
                palette::ember()
            },
        )
        .params(),
    );

    // A column, not a row: a row of three inside 410 pixels gives each of them
    // 126, and a button that cannot hold its own name is not a choice either.
    const BUTTON: f32 = 62.0;
    const GAP: f32 = 46.0;
    let top = plate.y + 108.0;
    for (index, rite) in choice.iter().enumerate() {
        let button = Rect::new(
            plate.x + 18.0,
            top + index as f32 * (BUTTON + GAP),
            plate.w - 36.0,
            BUTTON,
        );
        if virtual_button(
            button,
            &rite.name.to_uppercase(),
            true,
            ButtonTone::Primary,
            pointer,
            nav,
        ) {
            actions.push(UiAction::ChooseRite(index));
        }
        // What it does, under the button rather than on it: the names are the
        // cabinet's fiction, and the fiction does not say whether a gilding is
        // worth taking on a board that has not won anything.
        draw_ui_text_ex(
            &promise(data, round, rite),
            button.x + 4.0,
            button.bottom() + 20.0,
            TextStyle::new(14.0, palette::text_dim()).params(),
        );
    }
}

/// One line per rite, each stating a **fact about the board on screen** (§5.87).
///
/// §5.86 gave the gilding an exact figure because it is the only rite that has
/// one, and left the other two describing themselves in the abstract. That was
/// half a fix: a panel showing "pays 400" beside "climbs the paytable" reads as
/// a recommendation whether or not one is meant, and biasing without informing
/// is worse than saying nothing.
///
/// So all three now answer the same question — *what does this do to the board
/// I am looking at?* — and none of them answers a different one. How many cells
/// are offered to a widening and what an enrichment would climb to are both
/// facts, already on screen and countable by eye. What the widening will
/// actually take is a roll, and it is not quoted, because that is the part the
/// player is deciding under.
fn promise(data: &GameData, round: &SeamRound, rite: &RiteDef) -> String {
    match rite.kind {
        RiteKind::Widen { .. } => match round.frontier(data) {
            0 => "walled in — no cells to take".to_owned(),
            1 => "1 cell beside it may turn".to_owned(),
            cells => format!("{} cells beside it may turn", cells),
        },
        RiteKind::Enrich { .. } => match round.next_rung(data) {
            Some(next) => format!(
                "{} cells become {}",
                round.cells().len(),
                data.symbols.get(next).name
            ),
            None => "already the richest — nothing to climb".to_owned(),
        },
        RiteKind::Gild { multiply_permille } => {
            // Every beat it has left, compounded — the multiplier the round will
            // actually reach. Asked of the round rather than worked out here, so
            // the figure carries the ceiling and the free-spin multiplier.
            let mut multiplier = 1_000i64;
            for _ in 0..round.steps_left() {
                multiplier = multiplier * multiply_permille.max(1_000) / 1_000;
            }
            match round.worth_at(data, multiplier) {
                0 => "pays nothing on this board".to_owned(),
                worth => format!("pays {} on this board", naming::credits(worth)),
            }
        }
    }
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
        &round
            .rite()
            .map_or_else(|| "THE SEAM".to_owned(), |rite| rite.name.to_uppercase()),
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

    // A gilding changes no symbols, so "moves left" is the only thing on screen
    // that would tell the player anything is happening — the multiplier is what
    // is actually moving, and it takes the plate.
    let multiplier = round.multiplier_permille();
    let label = if multiplier > 1_000 {
        format!("x{}", multiplier as f64 / 1_000.0)
    } else {
        format!("{} LEFT", round.steps_left())
    };
    let moves = Rect::new(banner.right() - 210.0, banner.y + 8.0, 96.0, 40.0);
    draw_surface(
        moves,
        &SurfaceStyle::new(Color::new(0.08, 0.07, 0.10, 1.0)).with_border(1.0, palette::gold_dim()),
    );
    draw_text_centered_in_box_ex(
        &label,
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

#[cfg(test)]
mod tests;
