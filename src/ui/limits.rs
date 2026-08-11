//! The session limits panel (§5.30).
//!
//! Four cycling buttons, in the same one-column shape as the settings overlay.
//! What is different is that a row can be showing two values at once: the cap in
//! force now, and the looser one filed for the next session. Hiding that would
//! make the asymmetry feel like the button was broken.

use crate::state::limits::{Cap, LimitChoices, LimitState};
use crate::ui::frame;
use crate::ui::nav::Nav;
use crate::ui::{logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_ui_text_ex, ButtonTone, Region, SurfaceStyle, TextStyle,
};

const ROW_HEIGHT: f32 = 62.0;

pub fn draw(
    state: &LimitState,
    choices: &LimitChoices,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.78),
    );

    // Sized to its content: four rows and the rule that governs them.
    let panel = frame::centred_at(680.0, 130.0, 396.0);
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::on(panel, palette::stone());
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::stone())
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "Session Limits",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );
    if virtual_button(
        crate::ui::close_button(panel),
        "Close",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleLimits);
    }

    let mut y = panel.y + 62.0;

    row(
        panel,
        y,
        "Reality Check",
        "how often the game states the session figures",
        &minutes_label(state.reality_check_minutes),
        None,
        pointer,
        actions,
        nav,
        UiAction::CycleRealityCheck,
    );
    y += ROW_HEIGHT;

    for (cap, title, hint, action) in [
        (
            Cap::Time,
            "Time Limit",
            "play stops after this long",
            UiAction::CycleLimit(Cap::Time),
        ),
        (
            Cap::Loss,
            "Loss Limit",
            "play stops once you are down this much",
            UiAction::CycleLimit(Cap::Loss),
        ),
        (
            Cap::Spins,
            "Spin Limit",
            "play stops after this many paid spins",
            UiAction::CycleLimit(Cap::Spins),
        ),
    ] {
        let deferred = state
            .deferred(cap)
            .then(|| format!("from next game: {}", cap_label(cap, state.requested(cap))));
        row(
            panel,
            y,
            title,
            hint,
            &cap_label(cap, state.in_force(cap)),
            deferred.as_deref(),
            pointer,
            actions,
            nav,
            action,
        );
        y += ROW_HEIGHT;
    }

    // The rule, stated where the buttons that obey it are. A player who tries to
    // raise a limit and sees the old one stay put needs this on the same screen.
    draw_text_block(
        "A limit takes effect the moment you tighten it. Raising or removing one waits for your \
         next game — so the decision to play on is never made in the moment that prompted it.",
        panel.x + 20.0,
        y + 8.0,
        panel.w - 40.0,
        56.0,
        16.0,
        4.0,
        palette::text_dim(),
    );

    let _ = choices;
}

/// One row: heading, hint, the value in force, and what is waiting if anything.
#[allow(clippy::too_many_arguments)]
fn row(
    panel: Rect,
    y: f32,
    label: &str,
    hint: &str,
    value: &str,
    deferred: Option<&str>,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
    action: UiAction,
) {
    draw_ui_text_ex(
        label,
        panel.x + 20.0,
        y + 22.0,
        TextStyle::new(19.0, palette::text_bright()).params(),
    );
    draw_ui_text_ex(
        hint,
        panel.x + 20.0,
        y + 40.0,
        TextStyle::new(14.0, palette::text_dim()).params(),
    );
    if let Some(deferred) = deferred {
        draw_ui_text_ex(
            deferred,
            panel.x + 20.0,
            y + 56.0,
            TextStyle::new(14.0, palette::ember()).params(),
        );
    }

    if virtual_button(
        Rect::new(panel.right() - 232.0, y + 4.0, 212.0, 44.0),
        value,
        true,
        if value == "Off" {
            ButtonTone::Secondary
        } else {
            ButtonTone::Primary
        },
        pointer,
        nav,
    ) {
        actions.push(action);
    }
}

fn minutes_label(minutes: u32) -> String {
    if minutes == 0 {
        "Off".to_owned()
    } else {
        format!("Every {} min", minutes)
    }
}

/// How a cap reads on its button. `None` is off, which is the default and has
/// to be plainly legible as "no limit" rather than a blank.
pub fn cap_label(cap: Cap, value: Option<i64>) -> String {
    match (cap, value) {
        (_, None) => "Off".to_owned(),
        (Cap::Time, Some(minutes)) => format!("{} minutes", minutes),
        (Cap::Loss, Some(credits)) => format!("{} credits", super::naming::credits(credits)),
        (Cap::Spins, Some(spins)) => format!("{} spins", spins),
    }
}

/// Step to the next offering for a cap, wrapping. `0` in the data means "off",
/// which is why this returns an `Option` rather than the raw number.
pub fn next_choice(choices: &[i64], current: Option<i64>) -> Option<i64> {
    let current_value = current.unwrap_or(0);
    let index = choices
        .iter()
        .position(|value| *value == current_value)
        // A saved cap whose offering was removed from the data lands here and
        // steps to the first choice rather than sticking forever.
        .unwrap_or(choices.len() - 1);
    let next = choices[(index + 1) % choices.len()];
    (next != 0).then_some(next)
}

#[cfg(test)]
mod tests;
