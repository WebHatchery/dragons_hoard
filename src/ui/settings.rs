//! The settings overlay.
//!
//! Deliberately small and keyboard-reachable. The one setting that matters most
//! is the volume: the effects are synthesised (§7.1) and have never been heard
//! by their author, so being able to turn them down — or off — is not a nicety.

use crate::data::GameConfig;
use crate::state::preferences::Preferences;
use crate::ui::nav::Nav;
use crate::ui::{palette, virtual_button, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone, SurfaceStyle,
    TextStyle,
};

const ROW_HEIGHT: f32 = 52.0;

pub fn draw(
    config: &GameConfig,
    prefs: &Preferences,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.78),
    );

    let panel = Rect::new(340.0, 140.0, 600.0, 400.0);
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::STONE)
            .with_border(2.0, palette::GOLD)
            .with_header(48.0, palette::STONE_HEADER)
            .with_header_divider(1.0, palette::GOLD_DIM),
    );
    draw_ui_text_ex(
        "Settings",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::GOLD_BRIGHT).params(),
    );
    if virtual_button(
        Rect::new(panel.right() - 120.0, panel.y + 9.0, 100.0, 30.0),
        "Close",
        true,
        ButtonTone::Danger,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleSettings);
    }

    let mut y = panel.y + 66.0;

    // Volume gets a pair of steppers; everything else is a single cycling
    // button, which keeps the panel to one column and one interaction verb.
    draw_row_label(panel, y, "Sound", "master volume for every effect");
    let volume = (prefs.shared.master_volume * 100.0).round() as i32;
    if virtual_button(
        Rect::new(panel.right() - 212.0, y + 6.0, 42.0, 36.0),
        "-",
        volume > 0,
        ButtonTone::Secondary,
        mouse,
        nav,
    ) {
        actions.push(UiAction::VolumeDown);
    }
    draw_text_centered_in_box_ex(
        &if volume == 0 {
            "Muted".to_owned()
        } else {
            format!("{}%", volume)
        },
        panel.right() - 166.0,
        y + 6.0,
        100.0,
        36.0,
        TextStyle::new(
            20.0,
            if volume == 0 {
                palette::TEXT_DIM
            } else {
                palette::GOLD_BRIGHT
            },
        ),
    );
    if virtual_button(
        Rect::new(panel.right() - 62.0, y + 6.0, 42.0, 36.0),
        "+",
        volume < 100,
        ButtonTone::Secondary,
        mouse,
        nav,
    ) {
        actions.push(UiAction::VolumeUp);
    }
    y += ROW_HEIGHT;

    draw_row_label(panel, y, "Spin Speed", "how long the reels take to land");
    if cycle_button(panel, y, prefs.spin_speed.label(), mouse, nav) {
        actions.push(UiAction::CycleSpinSpeed);
    }
    y += ROW_HEIGHT;

    draw_row_label(
        panel,
        y,
        "Autospin Length",
        "spins an unattended run is worth",
    );
    if cycle_button(
        panel,
        y,
        &prefs.autospin_spins(config).to_string(),
        mouse,
        nav,
    ) {
        actions.push(UiAction::CycleAutospinLength);
    }
    y += ROW_HEIGHT;

    draw_row_label(panel, y, "Screen Shake", "camera kick on big wins");
    if toggle_button(panel, y, prefs.shared.screen_shake, mouse, nav) {
        actions.push(UiAction::ToggleShake);
    }
    y += ROW_HEIGHT;

    draw_row_label(panel, y, "Particles", "bursts on wins and features");
    if toggle_button(panel, y, prefs.particles, mouse, nav) {
        actions.push(UiAction::ToggleParticles);
    }
    y += ROW_HEIGHT;

    // Limits get their own panel rather than four more rows here: they are the
    // one group where a row can be showing two values at once (§5.30).
    if virtual_button(
        Rect::new(panel.x + 20.0, y, panel.w - 40.0, 38.0),
        "Session Limits — time, loss and spin caps",
        true,
        ButtonTone::Secondary,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleLimits);
    }
    y += 46.0;

    draw_ui_text_ex(
        "Settings are kept separately from your save — a new game keeps them.",
        panel.x + 20.0,
        y + 18.0,
        TextStyle::new(15.0, palette::TEXT_DIM).params(),
    );
}

fn draw_row_label(panel: Rect, y: f32, label: &str, hint: &str) {
    draw_ui_text_ex(
        label,
        panel.x + 20.0,
        y + 22.0,
        TextStyle::new(19.0, palette::TEXT_BRIGHT).params(),
    );
    draw_ui_text_ex(
        hint,
        panel.x + 20.0,
        y + 40.0,
        TextStyle::new(14.0, palette::TEXT_DIM).params(),
    );
}

fn cycle_button(panel: Rect, y: f32, value: &str, mouse: Vec2, nav: &mut Nav) -> bool {
    virtual_button(
        Rect::new(panel.right() - 212.0, y + 6.0, 192.0, 36.0),
        value,
        true,
        ButtonTone::Primary,
        mouse,
        nav,
    )
}

fn toggle_button(panel: Rect, y: f32, on: bool, mouse: Vec2, nav: &mut Nav) -> bool {
    virtual_button(
        Rect::new(panel.right() - 212.0, y + 6.0, 192.0, 36.0),
        if on { "On" } else { "Off" },
        true,
        if on {
            ButtonTone::Positive
        } else {
            ButtonTone::Secondary
        },
        mouse,
        nav,
    )
}
