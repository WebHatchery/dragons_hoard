//! The wager panel: bet ladder, spin button, and compact gameplay decisions.
//!
//! Split out of `ui.rs` on size. It is the one part of the screen that is a
//! *form* rather than a display — every control here changes something, which is
//! also why it holds most of the game's focusable controls (§5.27).

use super::{palette, virtual_button, ButtonTone, Color, Rect, TextStyle, UiAction, UiContext};
use crate::state::featurebuy::cheapest as cheapest_feature;
use crate::ui::frame;
use crate::ui::naming;
use crate::ui::nav::Nav;
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_ui_text_ex, RectExt, Region,
    SurfaceStyle,
};

pub fn draw_control_panel(
    ctx: &UiContext<'_>,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    let rect = ctx.frame.wager;
    // The wager panel (§5.37).
    let _region = Region::on(rect, palette::stone());
    draw_surface(
        rect,
        &SurfaceStyle::new(palette::stone()).with_border(1.0, palette::gold_dim()),
    );

    // The wager readout flows from the top and the buttons are anchored to the
    // bottom, so the free-spin banner can claim the space between them without
    // pushing anything off the panel.
    let content = rect.inset(18.0);
    let mut y = content.y;
    y = draw_win_readout(ctx, content, y, pointer, actions, nav);
    if !ctx.session.in_free_spins() {
        y = draw_bet_controls(ctx, content, y, pointer, actions, nav);
    }
    draw_feature_banner(ctx, content, y);

    draw_spin_button(ctx, content, content.bottom(), pointer, actions, nav);
}

fn draw_win_readout(
    ctx: &UiContext<'_>,
    content: Rect,
    y: f32,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) -> f32 {
    // The result is supporting information, not another strong panel. Gamble is
    // offered beside an eligible settled win so its stake and consequence are
    // visible at the moment the player decides whether to risk it.
    let rect = Rect::new(content.x, y, content.w, 56.0);
    draw_ui_text_ex(
        "Last result",
        rect.x,
        rect.y + 23.0,
        TextStyle::new(16.0, palette::text_dim()).params(),
    );
    draw_ui_text_ex(
        &naming::credits(ctx.session.displayed_win()),
        rect.x + 116.0,
        rect.y + 23.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );
    if ctx.session.can_gamble() {
        let win = ctx.session.last_win;
        let button = Rect::new(rect.right() - 166.0, rect.y, 166.0, 44.0);
        if virtual_button(
            button,
            &format!("Gamble {} · 2×", naming::credits(win)),
            true,
            ButtonTone::Secondary,
            pointer,
            nav,
        ) {
            actions.push(UiAction::OfferGamble);
        }
        draw_ui_text_ex(
            "Risk the win for double or nothing",
            rect.x,
            rect.y + 51.0,
            TextStyle::new(13.0, palette::text_dim()).params(),
        );
    }
    if ctx.session.can_gamble() {
        y + 64.0
    } else {
        y + 56.0
    }
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

    // 44, the touch standard (§5.78). These two are the smallest controls a
    // player uses often — the bet step — and were square at 38.
    let button = 44.0;
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
    if !frame::is_portrait() {
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
                ctx.session.staked(ctx.data)
            ),
            content.x,
            y,
            content.w,
            42.0,
            17.0,
            4.0,
            palette::text_dim(),
        );
        y + 46.0
    } else {
        draw_ui_text_ex(
            &format!(
                "{} · Total {}",
                compact_line_summary(ctx),
                ctx.session.staked(ctx.data)
            ),
            content.x,
            y + 20.0,
            TextStyle::new(15.0, palette::text_dim()).params(),
        );
        y + 28.0
    }
}

fn compact_line_summary(ctx: &UiContext<'_>) -> String {
    match (ctx.data.ways_count(), ctx.data.max_ways()) {
        (Some(ways), _) => format!("{} ways", ways),
        (None, Some(ceiling)) => format!(
            "{} ways (up to {})",
            ctx.session.display_grid().ways(),
            ceiling
        ),
        _ if ctx.data.config.evaluation == crate::data::Evaluation::Cluster => format!(
            "{} cells ({}+ groups)",
            ctx.data.config.reel_count * ctx.data.config.row_count,
            crate::engine::cluster::MIN_CLUSTER
        ),
        _ => format!("Lines: {}", ctx.data.paylines.len()),
    }
}

/// The ante switch (§5.75), in the secondary row (§5.78).
///
/// It began on the Total Bet line, right-aligned, which was the right idea —
/// that is the number it changes, and 200 becomes 281 as you press it. At 26
/// pixels tall it fitted there. At 44 it does not, and it overlapped the figure
/// it was explaining. The row below had a slot that a less consequential
/// control was using.
///
/// Absent entirely on a cabinet that sells no ante — a greyed-out switch would
/// read as something the player had failed to unlock, when it is really a
/// cabinet on which the bet could not be priced honestly (Avalanche). The
/// paytable takes the slot back on those.
fn draw_ante(
    ctx: &UiContext<'_>,
    content: Rect,
    y: f32,
    width: f32,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    let slot = Rect::new(content.x + (width + 8.0) * 2.0, y, width, 44.0);
    let Some(ante) = ctx.data.ante() else {
        if virtual_button(slot, "Paytable", true, ButtonTone::Secondary, pointer, nav) {
            actions.push(UiAction::TogglePaytable);
        }
        return;
    };
    let on = ctx.session.ante(ctx.data);
    if virtual_button(
        slot,
        &if on {
            format!("Ante {:.2}x", ante.cost_permille as f64 / 1000.0)
        } else {
            "Ante off".to_owned()
        },
        // `is_settled`, not `can_spin`. A player who has just made themselves
        // unable to afford an ante spin has to be able to switch it back off,
        // and gating on affordability would lock them into the more expensive
        // bet at exactly the moment it stopped being affordable.
        ctx.session.is_settled(),
        if on {
            ButtonTone::Primary
        } else {
            ButtonTone::Secondary
        },
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleAnte);
    }
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
                free_spins.multiplier(ctx.data),
                free_spins.total_won
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
    // Both rows are 44 now (§5.78), so the offsets that positioned them have
    // to grow with them or the block climbs into the bet controls above.
    let secondary_y = below - 10.0 - 44.0;
    let spin_y = secondary_y - 8.0 - 70.0;
    draw_spin_choices(ctx, content, spin_y, pointer, actions, nav);
    draw_spin_controls(ctx, content, secondary_y, spin_y, pointer, actions, nav);
}

fn draw_spin_choices(
    ctx: &UiContext<'_>,
    content: Rect,
    spin_y: f32,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    // The free-spin choice takes the rules button's place while it is open
    // (§5.64). It is the only thing on screen worth pressing at that moment,
    // and it closes itself the instant the first spin goes — so it cannot sit
    // there stale, and pressing SPIN instead is a valid answer that takes the
    // cabinet's own shape.
    let shapes = ctx.session.free_spin_shapes(ctx.data);
    if !shapes.is_empty() {
        let width = (content.w - 8.0) / shapes.len() as f32;
        for (index, shape) in shapes.iter().enumerate() {
            let spins = ctx
                .session
                .free_spins
                .as_ref()
                .map_or(0, |run| shape.spins(run.awarded));
            let slot = Rect::new(
                content.x + index as f32 * (width + 8.0),
                spin_y - 66.0,
                width,
                44.0,
            );
            if virtual_button(
                slot,
                &format!("{} spins at x{}", spins, shape.multiplier),
                true,
                ButtonTone::Primary,
                pointer,
                nav,
            ) {
                actions.push(UiAction::ChooseFreeSpinShape(index));
            }
            // What the shape actually feels like, measured (§5.65). The two are
            // worth the same and saying so is not reassurance — a player has
            // been handed two numbers and asked to trust a third they cannot
            // see. This is the third one.
            let measured = ctx
                .profiles
                .shape(ctx.data.machine_id(), index)
                .map(|profile| {
                    format!(
                        "blank {:.0}%  ·  best {:.0}x",
                        profile.blanks * 100.0,
                        profile.best
                    )
                })
                .unwrap_or_else(|| "measuring...".to_owned());
            draw_text_centered_in_box_ex(
                &measured,
                slot.x,
                slot.bottom() + 2.0,
                slot.w,
                16.0,
                TextStyle::new(13.0, palette::text_dim()),
            );
        }
    } else {
        // Rules and Buy are both secondary gameplay decisions, so they share
        // this quiet row above Spin rather than competing with navigation.
        let width = (content.w - 8.0) / 2.0;
        if virtual_button(
            Rect::new(content.x, spin_y - 52.0, width, 44.0),
            &format!("How {} plays", ctx.data.config.display_name),
            true,
            ButtonTone::Secondary,
            pointer,
            nav,
        ) {
            actions.push(UiAction::ToggleRules);
        }
        let buy_label = cheapest_feature(&ctx.data.featurebuy, ctx.session.total_bet(ctx.data))
            .map_or_else(
                || "Buy".to_owned(),
                |price| format!("Buy {}", naming::credits(price)),
            );
        if virtual_button(
            Rect::new(content.x + width + 8.0, spin_y - 52.0, width, 44.0),
            &buy_label,
            ctx.session.is_settled() && !ctx.session.in_free_spins(),
            ButtonTone::Secondary,
            pointer,
            nav,
        ) {
            actions.push(UiAction::ToggleFeatureBuy);
        }
    }
}

fn draw_spin_controls(
    ctx: &UiContext<'_>,
    content: Rect,
    secondary_y: f32,
    spin_y: f32,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
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
        Rect::new(content.x, secondary_y, third, 44.0),
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
        Rect::new(content.x + third + 8.0, secondary_y, third, 44.0),
        &auto_label,
        // Stopping is always allowed; starting needs a settled, affordable game.
        running > 0 || (ctx.session.can_spin(ctx.data) && !ctx.session.in_free_spins()),
        auto_tone,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleAutospin);
    }

    draw_ante(ctx, content, secondary_y, third, pointer, actions, nav);
}
