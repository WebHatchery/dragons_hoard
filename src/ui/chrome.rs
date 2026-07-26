//! The furniture around the game: the header bar and the footer strip.
//!
//! Neither is a panel. They are drawn behind every overlay and never covered by
//! one, which is why §5.50 gave overlays a floor to start below — and why the
//! turned layout (§5.79) has to give the header a second row rather than let
//! its buttons slide off a 720-wide frame.
//!
//! Split out of `ui.rs` when it crossed 800 lines (§5.79).

use super::{
    cheapest_feature, frame, naming, palette, shortcuts, virtual_button, ButtonTone, Color,
    Pointer, Rect, Region, TextStyle, UiAction, UiContext,
};
use crate::ui::nav::Nav;
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_badge, draw_surface, draw_text_block, draw_ui_text_ex, meter, SurfaceStyle,
};

pub(super) fn draw_header(
    ctx: &UiContext<'_>,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    let rect = ctx.frame.header;
    // The header, where the cabinet name ran into the Buy button (§5.35).
    let _region = Region::on(rect, palette::stone_header());
    draw_surface(
        rect,
        &SurfaceStyle::new(palette::stone_header())
            .with_border(1.0, palette::gold_dim())
            .with_top_highlight(2.0, palette::gold()),
    );

    // Fitted to the space the buttons leave, not set at 31px and hoped for.
    // The buttons are anchored 946 logical pixels from the right edge, so on a
    // narrow screen (§5.46) they arrive exactly where the title was — and the
    // layout audit could not see it, because the title never crossed its
    // *region's* edge, only collided with something inside it.
    // Turned frames put the buttons on a second row (§5.79), so the title has
    // the whole width rather than what 946 pixels of controls leave.
    let portrait = frame::is_portrait();
    let buttons_y = if portrait {
        rect.y + 62.0
    } else {
        rect.y + 10.0
    };
    // The badges keep their right-hand anchors on the top row, so the title
    // still has to stop short of them — just by less than the buttons wanted.
    let right = rect.right() - if portrait { 486.0 } else { 946.0 };
    let title_span = right - (rect.x + 18.0) - 12.0;
    if title_span >= 120.0 {
        draw_text_block(
            &ctx.data.config.display_name,
            rect.x + 18.0,
            rect.y + 12.0,
            title_span,
            38.0,
            31.0,
            0.0,
            palette::gold_bright(),
        );
    }

    // 44 tall, not 28 (§5.78).
    //
    // Every button in this game was drawn between 26 and 38 logical pixels,
    // because it was laid out against a mouse pointer. A finger is not a
    // pointer: WCAG 2.5.5 and Apple both ask for 44, and the header's four
    // buttons are the game's primary navigation — the first thing a tablet
    // player reaches for. The header is 64 tall and they now sit centred in it.
    //
    // The badges beside them keep their height: nothing presses a badge.
    //
    // The header has the only spare width on screen, and these should be
    // reachable from anywhere rather than buried in the wager panel.
    // The button carries the entry price, so the cost of the cheapest feature
    // is visible without opening anything — and it moves with the bet ladder,
    // which is the quickest way to see that the menu is priced per stake.
    let from = cheapest_feature(&ctx.data.featurebuy, ctx.session.total_bet(ctx.data));
    if virtual_button(
        nav_slot(rect, portrait, buttons_y, 0, 946.0),
        &match from {
            Some(price) => format!("Buy {}", naming::credits(price)),
            None => "Buy".to_owned(),
        },
        true,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleFeatureBuy);
    }
    // The door to everything the header has no room for (§5.72). It takes the
    // slot Awards used to have: the header is full by design — the cabinet name
    // gets whatever the buttons leave — and Awards has a row in the menu like
    // everything else, whereas four screens had no door at all. One of them was
    // the colour-vision panel, an accessibility feature that needed a keyboard.
    if virtual_button(
        nav_slot(rect, portrait, buttons_y, 1, 828.0),
        "More",
        true,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleMenu);
    }
    if virtual_button(
        nav_slot(rect, portrait, buttons_y, 2, 710.0),
        "Machines",
        true,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleMachines);
    }
    if virtual_button(
        nav_slot(rect, portrait, buttons_y, 3, 592.0),
        "Settings",
        true,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleSettings);
    }

    let hoard = &ctx.session.hoard;
    draw_badge(
        Rect::new(rect.right() - 470.0, rect.y + 18.0, 200.0, 28.0),
        &format!(
            "Hoard {}/{}  pot {}",
            hoard.count,
            ctx.data.config.hoard_capacity,
            naming::credits(hoard.pot)
        ),
        Color::new(0.22, 0.16, 0.10, 1.0),
        palette::text(),
    );
    draw_badge(
        Rect::new(rect.right() - 258.0, rect.y + 18.0, 152.0, 28.0),
        &format!("Balance {}", naming::credits(ctx.session.balance)),
        Color::new(0.16, 0.20, 0.13, 1.0),
        palette::text_bright(),
    );
    draw_badge(
        Rect::new(rect.right() - 96.0, rect.y + 18.0, 78.0, 28.0),
        &format!("v{}", ctx.data.config.version),
        Color::new(0.18, 0.15, 0.22, 1.0),
        palette::text_dim(),
    );
}

/// Bottom strip: hoard progress and session stats. The far right is left clear
/// for the notification stack, which anchors bottom-right.
pub(super) fn draw_footer(ctx: &UiContext<'_>) {
    let rect = ctx.frame.footer;
    // The footer, where the generated shortcut line clipped (§5.29).
    let _region = Region::on(rect, Color::new(0.07, 0.06, 0.07, 1.0));
    draw_surface(
        rect,
        &SurfaceStyle::new(Color::new(0.07, 0.06, 0.07, 0.96))
            .with_border(1.0, palette::gold_dim()),
    );

    let hoard = &ctx.session.hoard;
    meter(
        Rect::new(rect.x + 18.0, rect.y + 14.0, 420.0, 22.0),
        hoard.count as f32,
        ctx.data.config.hoard_capacity as f32,
        palette::ember(),
        // Machine-agnostic: the Frost cabinet has a hoard too.
        Some(&format!(
            "Hoard {}/{}",
            hoard.count, ctx.data.config.hoard_capacity
        )),
    );
    // Fitted to the room before the shortcut line, not set at 15px and left to
    // run. The shortcut line beside it has been width-fitted since §5.29; this
    // sentence never was, so under the 40% pseudolocale (§5.39) it ran straight
    // through it — 315px² of overlap that nothing saw for as long as the sweep
    // was running on a cabinet whose pot happened to be a shorter number
    // (§5.76).
    draw_text_block(
        &format!(
            "Fill the hoard to hatch a prize worth {}x the banked pot ({}).",
            ctx.data.config.hatch_pot_multiplier, hoard.pot
        ),
        rect.x + 18.0,
        rect.y + 44.0,
        SHORTCUT_LINE_X - 30.0,
        22.0,
        15.0,
        0.0,
        palette::text_dim(),
    );

    let stats = &ctx.session.stats;
    // Staked credits appear here the moment there are any, and nowhere else in
    // the line is conditional. That is the point: a stipend that was never
    // mentioned again would quietly make every other figure on this row a lie
    // (§5.53).
    let staked = if stats.staked > 0 {
        format!("   Staked {}", naming::credits(stats.staked))
    } else {
        String::new()
    };
    draw_ui_text_ex(
        &format!(
            "Spins {}   Best win {}   Free spins played {}   Hatches {}{}",
            stats.total_spins,
            naming::credits(stats.biggest_win),
            stats.free_spins_played,
            stats.hatches,
            staked
        ),
        rect.x + 470.0,
        rect.y + 30.0,
        TextStyle::new(16.0, palette::text()).params(),
    );
    // A hint (§5.28) takes this line while it is showing. The two say the same
    // sort of thing and only one of them gets read.
    if ctx.hint.is_none() {
        // Sized to fit rather than set at 15: the line is generated from the
        // shortcut table now (§5.29), so adding a binding lengthens it and a
        // fixed size would quietly clip the last one off the right edge.
        let left = rect.x + SHORTCUT_LINE_X;
        draw_text_block(
            &shortcuts::footer_line(),
            left,
            rect.y + 42.0,
            rect.right() - left - 8.0,
            18.0,
            15.0,
            0.0,
            palette::text_dim(),
        );
    }
}

/// Where the shortcut line starts, measured from the footer's left edge.
///
/// Named because two things depend on it and they used to disagree: the
/// shortcut line was fitted to the space from here rightwards, and the hoard
/// sentence to its left was not fitted to anything at all.
const SHORTCUT_LINE_X: f32 = 470.0;

/// Where one of the header's four navigation buttons goes.
///
/// Landscape anchors them from the right edge, where they have always been.
/// A turned frame is 720 across and those anchors reach 946 — the buttons slid
/// straight off the left of the screen — so portrait spreads the four evenly
/// across a second header row instead (§5.79).
fn nav_slot(header: Rect, portrait: bool, y: f32, index: usize, landscape_offset: f32) -> Rect {
    if !portrait {
        return Rect::new(header.right() - landscape_offset, y, 108.0, 44.0);
    }
    let gap = 8.0;
    let width = (header.w - 36.0 - gap * 3.0) / 4.0;
    Rect::new(
        header.x + 18.0 + (width + gap) * index as f32,
        y,
        width,
        44.0,
    )
}
