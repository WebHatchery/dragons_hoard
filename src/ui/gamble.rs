//! The Dragon's Gamble panel (§5.16).
//!
//! Two colours, a ladder, and a Take button that is deliberately the calmest
//! thing on screen. The panel never shows odds as anything other than even,
//! because they are even.
//!
//! The offer and the round are the same panel in two states: before the first
//! flip it asks whether to gamble at all, and after it asks whether to go again.

use crate::data::GameData;
use crate::state::gamble::{GambleRound, Scale};
use crate::state::GameSession;
use crate::ui::frame;
use crate::ui::naming;
use crate::ui::nav::{self, Nav};
use crate::ui::{logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone, Region, SurfaceStyle,
    TextStyle,
};

const EMBER_FILL: Color = Color::new(0.42, 0.11, 0.03, 1.0);
const ASH_FILL: Color = Color::new(0.13, 0.13, 0.16, 1.0);

pub fn draw(
    data: &GameData,
    session: &GameSession,
    round: &GambleRound,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.82),
    );

    let panel = frame::centred_at(600.0, 110.0, 508.0);
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::on(panel, palette::stone());
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::stone())
            .with_border(2.0, palette::ember())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "The Dragon's Gamble",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );

    // What is at risk, in the largest type on the panel — it is the number the
    // decision is about.
    let stake = Rect::new(panel.x + 20.0, panel.y + 64.0, panel.w - 40.0, 78.0);
    draw_surface(
        stake,
        &SurfaceStyle::new(Color::new(0.07, 0.06, 0.04, 1.0)).with_border(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "AT RISK",
        stake.x + 16.0,
        stake.y + 26.0,
        TextStyle::new(14.0, palette::text_dim()).params(),
    );
    draw_text_centered_in_box_ex(
        &naming::credits(round.stake()),
        stake.x,
        stake.y + 22.0,
        stake.w,
        44.0,
        TextStyle::new(38.0, palette::gold_bright()),
    );
    if round.banked() > 0 {
        draw_ui_text_ex(
            &format!("{} banked and safe", round.banked()),
            stake.x + 16.0,
            stake.bottom() - 10.0,
            TextStyle::new(14.0, palette::text_bright()).params(),
        );
    }

    // Every row below is placed from the one above it. Fixed offsets from the
    // panel put Take underneath the half-gamble buttons the first time this was
    // drawn, and nothing but the capture would have shown it.
    let ladder = Rect::new(panel.x + 20.0, stake.bottom() + 12.0, panel.w - 40.0, 26.0);
    draw_ladder(round, ladder);
    let flip_line = Rect::new(panel.x + 20.0, ladder.bottom() + 8.0, panel.w - 40.0, 30.0);
    draw_last_flip(round, flip_line);

    let can_flip = round.can_flip();
    let row = Rect::new(
        panel.x + 20.0,
        flip_line.bottom() + 14.0,
        panel.w - 40.0,
        92.0,
    );
    let half_width = (row.w - 16.0) * 0.5;

    draw_colour_button(
        Rect::new(row.x, row.y, half_width, row.h),
        Scale::Ember,
        EMBER_FILL,
        can_flip,
        pointer,
        actions,
        nav,
    );
    draw_colour_button(
        Rect::new(row.right() - half_width, row.y, half_width, row.h),
        Scale::Ash,
        ASH_FILL,
        can_flip,
        pointer,
        actions,
        nav,
    );

    // Half-gamble sits under the colours because it changes *how much*, not
    // which — a different question from the one above it.
    let half_row = Rect::new(panel.x + 20.0, row.bottom() + 12.0, panel.w - 40.0, 44.0);
    if round.allows_half() && can_flip {
        let half = (half_row.w - 16.0) * 0.5;
        if virtual_button(
            Rect::new(half_row.x, half_row.y, half, half_row.h),
            &format!("Half on Ember ({})", round.stake() / 2),
            true,
            ButtonTone::Secondary,
            pointer,
            nav,
        ) {
            actions.push(UiAction::GambleHalf(Scale::Ember));
        }
        if virtual_button(
            Rect::new(half_row.right() - half, half_row.y, half, half_row.h),
            &format!("Half on Ash ({})", round.stake() / 2),
            true,
            ButtonTone::Secondary,
            pointer,
            nav,
        ) {
            actions.push(UiAction::GambleHalf(Scale::Ash));
        }
    }

    let take = Rect::new(
        panel.x + 20.0,
        half_row.bottom() + 24.0,
        panel.w - 40.0,
        54.0,
    );
    if virtual_button(
        take,
        &format!("Take {}", round.standing()),
        true,
        ButtonTone::Primary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::TakeGamble);
    }

    if !can_flip {
        draw_ui_text_ex(
            "The ladder is spent — take the win.",
            panel.x + 20.0,
            take.y - 10.0,
            TextStyle::new(14.0, palette::text_dim()).params(),
        );
    }

    let _ = (data, session);
}

/// One pip per rung, filled up to where the round has climbed.
fn draw_ladder(round: &GambleRound, rect: Rect) {
    let steps = round.max_steps().max(1);
    let gap = 6.0;
    let width = (rect.w - gap * (steps as f32 - 1.0)) / steps as f32;

    for step in 0..steps {
        let pip = Rect::new(rect.x + step as f32 * (width + gap), rect.y, width, rect.h);
        let climbed = step < round.steps();
        draw_surface(
            pip,
            &SurfaceStyle::new(if climbed {
                Color::new(0.36, 0.16, 0.03, 1.0)
            } else {
                Color::new(0.08, 0.07, 0.08, 1.0)
            })
            .with_border(
                1.0,
                if climbed {
                    palette::ember()
                } else {
                    palette::gold_dim()
                },
            ),
        );
    }
}

/// What the last flip turned up, so a win is read before the next decision.
fn draw_last_flip(round: &GambleRound, rect: Rect) {
    let Some(flip) = round.last_flip() else {
        draw_ui_text_ex(
            "Pick a scale. Even money, double or nothing.",
            rect.x,
            rect.y + 20.0,
            TextStyle::new(15.0, palette::text_dim()).params(),
        );
        return;
    };

    draw_ui_text_ex(
        &format!(
            "{} landed — {}",
            flip.landed.label(),
            if flip.won { "doubled" } else { "gone" }
        ),
        rect.x,
        rect.y + 20.0,
        TextStyle::new(
            16.0,
            if flip.won {
                palette::gold_bright()
            } else {
                palette::text_dim()
            },
        )
        .params(),
    );
}

fn draw_colour_button(
    rect: Rect,
    scale: Scale,
    fill: Color,
    enabled: bool,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    // Registered with the nav (§5.27) so the colours can be chosen without a
    // pointer; the whole panel was otherwise unreachable from the keyboard.
    let hit = nav.control(rect, enabled, pointer);
    let hovered = enabled && pointer.hovering_over(rect) || pointer.pressing(rect);
    draw_surface(
        rect,
        &SurfaceStyle::new(if enabled {
            fill
        } else {
            Color::new(fill.r * 0.4, fill.g * 0.4, fill.b * 0.4, 1.0)
        })
        .with_border(
            if hovered { 3.0 } else { 2.0 },
            if enabled {
                palette::gold_bright()
            } else {
                palette::gold_dim()
            },
        ),
    );
    draw_text_centered_in_box_ex(
        scale.label(),
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        TextStyle::new(
            30.0,
            if enabled {
                palette::text_bright()
            } else {
                palette::text_dim()
            },
        ),
    );

    if hit.focused {
        nav::focus_ring(rect);
    }
    if hit.activated {
        actions.push(UiAction::Gamble(scale));
    }
}
