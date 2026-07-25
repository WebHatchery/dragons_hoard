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
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone,
    Region, SurfaceStyle, TextStyle,
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

    let panel = Rect::new(340.0, 118.0, 600.0, 508.0);
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
        "Settings",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
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

    // Two volume rows rather than one: the music plays constantly and the
    // effects do not, so a player who wants one quiet rarely wants both quiet
    // (§5.31).
    draw_row_label(panel, y, "Sound", "master volume for every effect");
    volume_stepper(
        panel,
        y,
        prefs.shared.master_volume,
        UiAction::VolumeDown,
        UiAction::VolumeUp,
        mouse,
        actions,
        nav,
    );
    y += ROW_HEIGHT;

    draw_row_label(panel, y, "Music", "the four-track loop behind the reels");
    volume_stepper(
        panel,
        y,
        prefs.shared.music_volume,
        UiAction::MusicVolumeDown,
        UiAction::MusicVolumeUp,
        mouse,
        actions,
        nav,
    );
    y += ROW_HEIGHT;

    draw_row_label(
        panel,
        y,
        "Text Size",
        "every panel is measured at each size",
    );
    if cycle_button(
        panel,
        y,
        &format!("{}%", (prefs.text_scale() * 100.0).round()),
        mouse,
        nav,
    ) {
        actions.push(UiAction::CycleTextScale);
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

    draw_text_block(
        "Settings are kept separately from your save — a new game keeps them.",
        panel.x + 20.0,
        y + 4.0,
        panel.w - 40.0,
        38.0,
        15.0,
        3.0,
        palette::text_dim(),
    );
}

/// A minus/value/plus trio. Shared by the two volume rows so they cannot drift
/// apart in layout or in what "Muted" means.
#[allow(clippy::too_many_arguments)]
fn volume_stepper(
    panel: Rect,
    y: f32,
    value: f32,
    down: UiAction,
    up: UiAction,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    let percent = (value * 100.0).round() as i32;
    if virtual_button(
        Rect::new(panel.right() - 212.0, y + 6.0, 42.0, 36.0),
        "-",
        percent > 0,
        ButtonTone::Secondary,
        mouse,
        nav,
    ) {
        actions.push(down);
    }
    draw_text_centered_in_box_ex(
        &if percent == 0 {
            "Muted".to_owned()
        } else {
            format!("{}%", percent)
        },
        panel.right() - 166.0,
        y + 6.0,
        100.0,
        36.0,
        TextStyle::new(
            20.0,
            if percent == 0 {
                palette::text_dim()
            } else {
                palette::gold_bright()
            },
        ),
    );
    if virtual_button(
        Rect::new(panel.right() - 62.0, y + 6.0, 42.0, 36.0),
        "+",
        percent < 100,
        ButtonTone::Secondary,
        mouse,
        nav,
    ) {
        actions.push(up);
    }
}

fn draw_row_label(panel: Rect, y: f32, label: &str, hint: &str) {
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
