//! The wager panel: bet ladder, spin button, and the session buttons under it.
//!
//! Split out of `ui.rs` on size. It is the one part of the screen that is a
//! *form* rather than a display — every control here changes something, which is
//! also why it holds most of the game's focusable controls (§5.27).

use super::{palette, virtual_button, ButtonTone, Color, Rect, TextStyle, UiAction, UiContext};
use crate::ui::naming;
use crate::ui::nav::Nav;
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    RectExt, Region, SurfaceStyle,
};

pub fn draw_control_panel(
    ctx: &UiContext<'_>,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    let rect = Rect::new(852.0, 96.0, 410.0, 520.0);
    // The wager panel (§5.37).
    let _region = Region::on(rect, palette::stone());
    draw_surface(
        rect,
        &SurfaceStyle::new(palette::stone())
            .with_border(1.0, palette::gold_dim())
            .with_header(44.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        if ctx.session.in_free_spins() {
            "Free Spins"
        } else {
            "Wager"
        },
        rect.x + 18.0,
        rect.y + 30.0,
        TextStyle::new(19.0, palette::gold()).params(),
    );

    // The wager readout flows from the top and the buttons are anchored to the
    // bottom, so the free-spin banner can claim the space between them without
    // pushing anything off the panel.
    let content = rect.inset(18.0);
    let mut y = content.y + 44.0;
    y = draw_win_readout(ctx, content, y);
    y = draw_bet_controls(ctx, content, y, pointer, actions, nav);
    draw_feature_banner(ctx, content, y);

    let buttons_top = draw_session_buttons(ctx, content, pointer, actions, nav);
    draw_spin_button(ctx, content, buttons_top, pointer, actions, nav);
}

fn draw_win_readout(ctx: &UiContext<'_>, content: Rect, y: f32) -> f32 {
    let rect = Rect::new(content.x, y, content.w, 62.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(Color::new(0.07, 0.06, 0.05, 1.0)).with_border(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "WIN",
        rect.x + 14.0,
        rect.y + 38.0,
        TextStyle::new(18.0, palette::text_dim()).params(),
    );
    let win = ctx.session.displayed_win();
    draw_text_right(
        &naming::credits(win),
        rect.right() - 14.0,
        rect.y + 42.0,
        TextStyle::new(
            30.0,
            if win > 0 {
                palette::gold_bright()
            } else {
                palette::text_dim()
            },
        ),
    );
    y + 76.0
}

fn draw_bet_controls(
    ctx: &UiContext<'_>,
    content: Rect,
    y: f32,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) -> f32 {
    let line_bet = ctx.session.line_bet(ctx.data);
    let enabled = !ctx.session.bet_locked();

    draw_ui_text_ex(
        "Line Bet",
        content.x,
        y + 24.0,
        TextStyle::new(18.0, palette::text()).params(),
    );

    let button = 38.0;
    if virtual_button(
        Rect::new(content.right() - button * 2.0 - 92.0, y, button, button),
        "-",
        enabled,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::BetDown);
    }
    draw_text_centered_in_box_ex(
        &line_bet.to_string(),
        content.right() - button - 92.0,
        y,
        92.0,
        button,
        TextStyle::new(22.0, palette::gold_bright()),
    );
    if virtual_button(
        Rect::new(content.right() - button, y, button, button),
        "+",
        enabled,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::BetUp);
    }

    let y = y + 48.0;
    draw_text_block(
        &format!(
            "{}\nTotal Bet: {}",
            // A ways cabinet has no lines to count, and "Lines: 0" would read as
            // a fault rather than as a different machine.
            match (ctx.data.ways_count(), ctx.data.max_ways()) {
                (Some(ways), _) => format!("{} ways   (all active)", ways),
                // A shifting cabinet (§5.20) has a different number of ways
                // every spin, so it reads the board rather than the config.
                // Quoting the ceiling alone would be advertising a grid the
                // player is almost never looking at.
                (None, Some(ceiling)) => format!(
                    "{} ways this spin   (up to {})",
                    ctx.session.display_grid().ways(),
                    ceiling
                ),
                // A cluster cabinet has neither, and "Lines: 0" would read as a
                // fault rather than as a different machine (§5.35).
                _ if ctx.data.config.evaluation == crate::data::Evaluation::Cluster => format!(
                    "{} cells   (groups of {}+ pay)",
                    ctx.data.config.reel_count * ctx.data.config.row_count,
                    crate::engine::cluster::MIN_CLUSTER
                ),
                _ => format!("Lines: {}   (all active)", ctx.data.paylines.len()),
            },
            ctx.data.total_bet(line_bet)
        ),
        content.x,
        y,
        content.w,
        46.0,
        17.0,
        4.0,
        palette::text_dim(),
    );

    y + 54.0
}

/// Free spins take the banner slot; an autospin run gets it when they are not
/// running, so the panel always says what is driving the reels.
fn draw_feature_banner(ctx: &UiContext<'_>, content: Rect, y: f32) {
    let banner = match ctx.session.free_spins.as_ref() {
        Some(free_spins) => Some((
            Color::new(0.24, 0.11, 0.04, 1.0),
            palette::ember(),
            format!("{} free spins left", free_spins.remaining),
            format!(
                "x{} wilds expand  |  won {}",
                ctx.data.freespins.multiplier, free_spins.total_won
            ),
        )),
        None if ctx.session.autospin_remaining() > 0 => Some((
            Color::new(0.09, 0.14, 0.19, 1.0),
            palette::gold(),
            format!("Autospin — {} left", ctx.session.autospin_remaining()),
            "Stops on a feature, a hatch or a big win".to_owned(),
        )),
        None => None,
    };

    let Some((fill, accent, title, subtitle)) = banner else {
        return;
    };

    let rect = Rect::new(content.x, y, content.w, 58.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(fill)
            .with_border(2.0, accent)
            .with_left_accent(4.0, palette::gold_bright()),
    );
    draw_ui_text_ex(
        &title,
        rect.x + 14.0,
        rect.y + 26.0,
        TextStyle::new(19.0, palette::gold_bright()).params(),
    );
    draw_ui_text_ex(
        &subtitle,
        rect.x + 14.0,
        rect.y + 46.0,
        TextStyle::new(15.0, palette::text()).params(),
    );
}

/// Spin block, sitting immediately above the bottom-anchored session buttons.
fn draw_spin_button(
    ctx: &UiContext<'_>,
    content: Rect,
    below: f32,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    let secondary_y = below - 12.0 - 38.0;
    let spin_y = secondary_y - 10.0 - 70.0;

    // The rules panel (§5.29) gets its own full-width button in the gap above
    // the spin block, and gets it because R alone is not an affordance — a
    // shortcut nobody is told about is the problem the panel exists to fix.
    if virtual_button(
        Rect::new(content.x, spin_y - 46.0, content.w, 34.0),
        &format!("How {} plays", ctx.data.config.display_name),
        true,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleRules);
    }

    // A bound cap owns the button's label as well as its state (§5.30): a
    // greyed-out SPIN says the game is busy, which is the wrong answer.
    let label = if ctx.limits.breach().is_some() {
        "SESSION ENDED"
    } else if ctx.session.phase.is_busy() {
        "SPINNING"
    } else if ctx.session.in_free_spins() {
        "FREE SPIN"
    } else {
        "S P I N"
    };
    if virtual_button(
        Rect::new(content.x, spin_y, content.w, 70.0),
        label,
        ctx.session.can_spin(ctx.data) && ctx.limits.breach().is_none(),
        ButtonTone::Positive,
        pointer,
        nav,
    ) {
        actions.push(UiAction::Spin);
    }

    let third = (content.w - 16.0) / 3.0;
    if virtual_button(
        Rect::new(content.x, secondary_y, third, 38.0),
        "Max Bet",
        !ctx.session.bet_locked(),
        ButtonTone::Primary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::MaxBet);
    }

    let running = ctx.session.autospin_remaining();
    let (auto_label, auto_tone) = if running > 0 {
        (format!("Stop {}", running), ButtonTone::Danger)
    } else {
        (
            format!(
                "Auto {}",
                ctx.session.preferences.autospin_spins(&ctx.data.config)
            ),
            ButtonTone::Secondary,
        )
    };
    if virtual_button(
        Rect::new(content.x + third + 8.0, secondary_y, third, 38.0),
        &auto_label,
        // Stopping is always allowed; starting needs a settled, affordable game.
        running > 0 || (ctx.session.can_spin(ctx.data) && !ctx.session.in_free_spins()),
        auto_tone,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleAutospin);
    }

    if virtual_button(
        Rect::new(content.x + (third + 8.0) * 2.0, secondary_y, third, 38.0),
        "Paytable",
        true,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::TogglePaytable);
    }
}

/// Save/load/new/delete, anchored to the bottom of the panel. Returns the top
/// of the block so the spin controls can sit on top of it.
fn draw_session_buttons(
    ctx: &UiContext<'_>,
    content: Rect,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) -> f32 {
    let half = (content.w - 10.0) / 2.0;
    let bottom_row = content.bottom() - 34.0;
    let top_row = bottom_row - 42.0;

    // Saving mid-feature would bank a session whose free spins are not
    // persisted, and loading mid-spin would strand a committed stake.
    let storage_ready = ctx.session.is_settled() && !ctx.session.in_free_spins();

    if virtual_button(
        Rect::new(content.x, top_row, half, 34.0),
        "Save",
        storage_ready,
        ButtonTone::Positive,
        pointer,
        nav,
    ) {
        actions.push(UiAction::Save);
    }
    if virtual_button(
        Rect::new(content.x + half + 10.0, top_row, half, 34.0),
        "Load",
        storage_ready && ctx.save_exists,
        ButtonTone::Primary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::Load);
    }
    if virtual_button(
        Rect::new(content.x, bottom_row, half, 34.0),
        "New Game",
        ctx.session.phase.is_idle(),
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::NewGame);
    }
    if virtual_button(
        Rect::new(content.x + half + 10.0, bottom_row, half, 34.0),
        "Delete Save",
        ctx.session.phase.is_idle() && ctx.save_exists,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::DeleteSave);
    }

    top_row
}
