//! A look at the sounds, since nobody has heard them (§5.19).
//!
//! The synthesis has been covered by tests since it was written — well-formed
//! header, right rate, audible peak, nothing clipping, deterministic output —
//! and every one of those proves the *bytes* are right without saying anything
//! about whether the effects are any good. The GDD has carried "not verified:
//! how the sound actually sounds" for a dozen iterations.
//!
//! It still cannot be listened to here. But a waveform can be *looked* at, and a
//! surprising amount of what was unverifiable turns out to be visible: whether
//! an effect has a tail that will smear when it repeats, whether the attack is
//! so slow the sound arrives late, whether one effect is twice as loud as the
//! rest, whether anything is riding the limiter.
//!
//! So this draws each one. It is a developer instrument rather than a player
//! screen, reached from the settings panel and photographed by the capture
//! harness.

use crate::audio::{config, voices_for, Sfx};
use crate::ui::nav::Nav;
use crate::ui::{palette, virtual_button, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::synth::render_waveform;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_right, draw_ui_text_ex, ButtonTone, SurfaceStyle, TextStyle,
};

/// Seed the panel renders at. The same one the sound bank uses for its first
/// effect, so the noise drawn here is noise the player would actually hear.
const PLOT_SEED: u64 = 0xA11CE;

pub fn draw(mouse: Vec2, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.86),
    );

    let panel = Rect::new(150.0, 48.0, 980.0, 624.0);
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::STONE)
            .with_border(2.0, palette::GOLD)
            .with_header(44.0, palette::STONE_HEADER)
            .with_header_divider(1.0, palette::GOLD_DIM),
    );
    draw_ui_text_ex(
        "Sound — waveforms",
        panel.x + 20.0,
        panel.y + 29.0,
        TextStyle::new(20.0, palette::GOLD_BRIGHT).params(),
    );
    draw_text_right(
        "peak · length · shape",
        panel.right() - 130.0,
        panel.y + 29.0,
        TextStyle::new(14.0, palette::TEXT_DIM),
    );

    let config = config();
    let rows = Sfx::ALL.len();
    let row_height = (panel.h - 76.0) / rows as f32;

    for (index, sfx) in Sfx::ALL.iter().enumerate() {
        let row = Rect::new(
            panel.x + 20.0,
            panel.y + 56.0 + index as f32 * row_height,
            panel.w - 40.0,
            row_height - 6.0,
        );
        let wave = render_waveform(&voices_for(*sfx), &config, PLOT_SEED);
        draw_row(*sfx, &wave, config.sample_rate, row);
    }

    if virtual_button(
        Rect::new(panel.right() - 120.0, panel.y + 8.0, 100.0, 28.0),
        "Close",
        true,
        ButtonTone::Danger,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleWaveforms);
    }
}

fn draw_row(sfx: Sfx, wave: &[f32], sample_rate: u32, row: Rect) {
    let seconds = wave.len() as f32 / sample_rate as f32;
    let peak = wave.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));

    draw_ui_text_ex(
        &format!("{:?}", sfx),
        row.x,
        row.y + 14.0,
        TextStyle::new(14.0, palette::GOLD).params(),
    );
    draw_ui_text_ex(
        &format!("peak {:.2}   ·   {:.2}s", peak, seconds),
        row.x,
        row.y + 30.0,
        TextStyle::new(12.0, palette::TEXT_DIM).params(),
    );

    // Every plot is drawn on the same time and amplitude scale, so the effects
    // can be compared against each other rather than each filling its own box.
    // Two sounds that look alike here really are alike.
    let plot = Rect::new(row.x + 150.0, row.y, row.w - 150.0, row.h);
    draw_surface(
        plot,
        &SurfaceStyle::new(Color::new(0.05, 0.045, 0.05, 1.0)).with_border(1.0, palette::GOLD_DIM),
    );

    let mid = plot.y + plot.h * 0.5;
    draw_line(
        plot.x,
        mid,
        plot.right(),
        mid,
        1.0,
        Color::new(1.0, 1.0, 1.0, 0.10),
    );

    // One vertical bar per pixel column spanning that slice's min and max, which
    // is how an audio editor draws it — a per-pixel sample would alias into a
    // meaningless scribble at this width.
    let columns = plot.w as usize;
    let longest = LONGEST_SECONDS * sample_rate as f32;
    for column in 0..columns {
        let from = (column as f32 / columns as f32 * longest) as usize;
        let to = ((column + 1) as f32 / columns as f32 * longest) as usize;
        if from >= wave.len() {
            break;
        }
        let slice = &wave[from..to.min(wave.len()).max(from + 1)];

        let low = slice.iter().fold(0.0f32, |low, s| low.min(*s));
        let high = slice.iter().fold(0.0f32, |high, s| high.max(*s));
        let x = plot.x + column as f32;
        // Scaled to the axis below rather than to full scale: nothing here
        // peaks above about a third, so a +/-1.0 axis drew every effect as a
        // thin line along the middle and the shapes could not be read at all.
        let top = mid - (high / AXIS_PEAK) * plot.h * 0.46;
        let bottom = mid - (low / AXIS_PEAK) * plot.h * 0.46;

        // Hot where it is loud, so a glance finds the peaks.
        let heat = high.max(-low);
        draw_line(
            x,
            top,
            x,
            bottom.max(top + 1.0),
            1.0,
            Color::new(0.45 + 0.5 * heat, 0.30 + 0.35 * heat, 0.12, 1.0),
        );
    }
}

/// Time axis for every plot, in seconds. The longest effect is the Hatch at
/// about 0.78s; a fixed axis is what makes the rows comparable.
const LONGEST_SECONDS: f32 = 0.8;
/// Amplitude the plots are drawn against. Shared by every row, so the heights
/// stay comparable; anything louder than this would be riding the limiter and
/// a test would already have failed.
const AXIS_PEAK: f32 = 0.4;
