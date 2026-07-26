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
//!
//! §5.31 added the music, written by the same deaf author, and it goes here for
//! the same reason. What the plots show that a test cannot is the *shape*: four
//! tracks side by side reveal at a glance which one is carrying the loop and
//! whether the drum is a pulse or a wash. The live level beside each is the mix
//! the game is asking for right now, which is the only way to see a mood change
//! actually happening.

use crate::audio::{config, voices_for, Sfx};
use crate::music::{self, Track};
use crate::ui::frame;
use crate::ui::nav::Nav;
use crate::ui::{logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::synth::render_waveform;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_right, draw_ui_text_ex, ButtonTone, Region, SurfaceStyle, TextStyle,
};

/// Seed the panel renders at. The same one the sound bank uses for its first
/// effect, so the noise drawn here is noise the player would actually hear.
const PLOT_SEED: u64 = 0xA11CE;

pub fn draw(
    levels: [f32; Track::ALL.len()],
    mood: music::Mood,
    arrangement: music::Arrangement,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.86),
    );

    let panel = frame::centred_at(980.0, frame::BELOW_HEADER, 612.0);
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::on(panel, palette::stone());
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::stone())
            .with_border(2.0, palette::gold())
            .with_header(44.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "Sound — waveforms",
        panel.x + 20.0,
        panel.y + 29.0,
        TextStyle::new(20.0, palette::gold_bright()).params(),
    );
    draw_text_right(
        &format!(
            "{} · {:?}   ·   peak · length · shape",
            arrangement.id, mood
        ),
        panel.right() - 130.0,
        panel.y + 29.0,
        TextStyle::new(14.0, palette::text_dim()),
    );

    let config = config();
    let rows = Sfx::ALL.len() + Track::ALL.len();
    let row_height = (panel.h - 76.0) / rows as f32;
    let row_at = |index: usize| {
        Rect::new(
            panel.x + 20.0,
            panel.y + 56.0 + index as f32 * row_height,
            panel.w - 40.0,
            row_height - 6.0,
        )
    };

    for (index, sfx) in Sfx::ALL.iter().enumerate() {
        let wave = render_waveform(&voices_for(*sfx), &config, PLOT_SEED);
        draw_row(
            &format!("{:?}", sfx),
            &format!(
                "peak {:.2}   ·   {:.2}s",
                peak_of(&wave),
                seconds_of(&wave, config.sample_rate)
            ),
            &wave,
            row_at(index),
            LONGEST_SECONDS * config.sample_rate as f32,
        );
    }

    // The music below the effects, on the same scales, so the two can be
    // compared — the loop has to sit under them without fighting them (§5.31).
    for (index, track) in Track::ALL.iter().enumerate() {
        let wave = music::waveform(*track);
        let level = levels[index];
        draw_row(
            &format!("{} (music)", track.label()),
            &format!(
                "peak {:.2}   ·   {:.1}s   ·   now {:.0}%   ·   {}",
                peak_of(&wave),
                seconds_of(&wave, config.sample_rate),
                level * 100.0,
                // Every mood's target for this track, so the whole arrangement
                // is readable at once rather than one mix at a time — which is
                // what an author who cannot hear it needs (§5.31).
                music::Mood::ALL
                    .iter()
                    .map(|mood| {
                        // Initials: the full names ran past the plot and the
                        // last mood was cut off, which is the one place this
                        // line had to be complete.
                        let initial = format!("{:?}", mood).chars().next().unwrap_or('?');
                        format!("{}{:.0}", initial, arrangement.gain(*mood, *track) * 100.0)
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            &wave,
            row_at(Sfx::ALL.len() + index),
            // Every track is exactly one loop, so one loop is the right width
            // and the four are directly comparable (§5.31).
            wave.len() as f32,
        );
    }

    if virtual_button(
        crate::ui::close_button(panel),
        "Close",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleWaveforms);
    }
}

fn peak_of(wave: &[f32]) -> f32 {
    wave.iter().fold(0.0f32, |peak, s| peak.max(s.abs()))
}

fn seconds_of(wave: &[f32], sample_rate: u32) -> f32 {
    wave.len() as f32 / sample_rate as f32
}

/// `span` is the number of samples the plot's full width represents.
///
/// Passed in rather than derived so a family of sounds shares one scale and is
/// therefore comparable — two effects that look alike really are alike. The
/// music has its own span because an eleven-second loop drawn on the effects'
/// sub-second scale would show its first bar and nothing else.
fn draw_row(name: &str, detail: &str, wave: &[f32], row: Rect, span: f32) {
    draw_ui_text_ex(
        name,
        row.x,
        row.y + 14.0,
        TextStyle::new(14.0, palette::gold()).params(),
    );
    draw_ui_text_ex(
        detail,
        row.x,
        row.y + 30.0,
        TextStyle::new(12.0, palette::text_dim()).params(),
    );

    // Every plot is drawn on the same time and amplitude scale, so the effects
    // can be compared against each other rather than each filling its own box.
    // Two sounds that look alike here really are alike.
    // Wide enough for the music rows, which carry every mood's gain
    // alongside the current one (§5.31).
    let plot = Rect::new(row.x + 268.0, row.y, row.w - 268.0, row.h);
    draw_surface(
        plot,
        &SurfaceStyle::new(Color::new(0.05, 0.045, 0.05, 1.0))
            .with_border(1.0, palette::gold_dim()),
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
    let longest = span;
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
