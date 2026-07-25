//! The Ledger panel: what you have seen, against what the machine does (§5.18).
//!
//! Two rows of the same chart. The top one is the cabinet, measured over twenty
//! thousand rounds it ran itself (§5.17). The bottom is the player, over however
//! many they have played. They will not match, and the panel says so rather than
//! letting the player draw the wrong conclusion from the gap.

use crate::data::GameData;
use crate::engine::sim::BAND_LABELS;
use crate::state::ledger::{Ledger, MachineLedger};
use crate::state::profile::{MachineProfile, ProfileBook};
use crate::ui::nav::Nav;
use crate::ui::{palette, virtual_button, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_right, draw_ui_text_ex, ButtonTone, Region,
    SurfaceStyle, TextStyle,
};

pub fn draw(
    data: &GameData,
    ledger: &Ledger,
    profiles: &ProfileBook,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.82),
    );

    // Sized to the caveat, which is the last thing on it — the first pass left
    // a hand's width of empty stone under the text.
    let panel = Rect::new(190.0, 92.0, 900.0, 516.0);
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
        "Your ledger",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );
    draw_text_right(
        &format!("{} rounds across the catalog", ledger.total_rounds()),
        panel.right() - 140.0,
        panel.y + 32.0,
        TextStyle::new(15.0, palette::text_dim()),
    );

    let entry = ledger.get(data.machine_id());
    let profile = profiles.get(data.machine_id());

    draw_ui_text_ex(
        &format!("On {}", data.config.display_name),
        panel.x + 20.0,
        panel.y + 78.0,
        TextStyle::new(19.0, palette::gold()).params(),
    );

    match entry {
        Some(entry) if entry.rounds() > 0 => {
            draw_figures(
                entry,
                profile,
                Rect::new(panel.x + 20.0, panel.y + 92.0, panel.w - 40.0, 96.0),
            );
            draw_comparison(
                entry,
                profile,
                Rect::new(panel.x + 20.0, panel.y + 200.0, panel.w - 40.0, 190.0),
            );
            draw_caveat(
                entry,
                profile,
                Rect::new(panel.x + 20.0, panel.y + 402.0, panel.w - 40.0, 96.0),
            );
        }
        _ => {
            draw_ui_text_ex(
                "Nothing played on this cabinet yet. Spin a few times and come back.",
                panel.x + 20.0,
                panel.y + 130.0,
                TextStyle::new(16.0, palette::text_dim()).params(),
            );
        }
    }

    if virtual_button(
        Rect::new(panel.right() - 130.0, panel.y + 10.0, 110.0, 30.0),
        "Close",
        true,
        ButtonTone::Danger,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleLedger);
    }
}

/// The headline numbers, with the machine's alongside where it is known.
fn draw_figures(entry: &MachineLedger, profile: Option<&MachineProfile>, rect: Rect) {
    let machine_hit = profile.map_or(String::from("—"), |profile| {
        format!("{:.0}%", profile.hit_frequency * 100.0)
    });

    let cells = [
        (
            "Rounds played",
            format!("{}", entry.rounds()),
            String::new(),
        ),
        (
            "Paid you on",
            format!("{:.0}%", entry.hit_frequency() * 100.0),
            format!("machine {}", machine_hit),
        ),
        (
            "Your return",
            format!("{:.0}%", entry.rtp() * 100.0),
            String::from("excl. jackpots"),
        ),
        (
            "Best round",
            format!("{:.0}x", entry.best_round),
            format!("{} features", entry.features),
        ),
    ];

    let gap = 12.0;
    let width = (rect.w - gap * (cells.len() as f32 - 1.0)) / cells.len() as f32;
    for (index, (label, value, note)) in cells.iter().enumerate() {
        let cell = Rect::new(rect.x + index as f32 * (width + gap), rect.y, width, rect.h);
        draw_surface(
            cell,
            &SurfaceStyle::new(Color::new(0.075, 0.068, 0.055, 1.0))
                .with_border(1.0, palette::gold_dim()),
        );
        draw_ui_text_ex(
            label,
            cell.x + 14.0,
            cell.y + 24.0,
            TextStyle::new(13.0, palette::text_dim()).params(),
        );
        draw_ui_text_ex(
            value,
            cell.x + 14.0,
            cell.y + 58.0,
            TextStyle::new(30.0, palette::gold_bright()).params(),
        );
        if !note.is_empty() {
            draw_ui_text_ex(
                note,
                cell.x + 14.0,
                cell.y + 80.0,
                TextStyle::new(12.0, palette::text_dim()).params(),
            );
        }
    }
}

/// The two band charts, stacked so the shapes can be read against each other.
fn draw_comparison(entry: &MachineLedger, profile: Option<&MachineProfile>, rect: Rect) {
    draw_ui_text_ex(
        "How often each size of win turns up",
        rect.x,
        rect.y + 4.0,
        TextStyle::new(15.0, palette::text()).params(),
    );

    let bar_height = 34.0;
    if let Some(profile) = profile {
        draw_bands(
            &profile.bands,
            "the machine",
            Rect::new(rect.x, rect.y + 22.0, rect.w, bar_height),
        );
    }
    draw_bands(
        &entry.bands(),
        "you",
        Rect::new(rect.x, rect.y + 86.0, rect.w, bar_height),
    );

    // Only the ends are labelled: a legend for seven bands would be longer than
    // the bars it explains.
    draw_ui_text_ex(
        BAND_LABELS[0],
        rect.x,
        rect.y + 148.0,
        TextStyle::new(12.0, palette::text_dim()).params(),
    );
    draw_text_right(
        BAND_LABELS[BAND_LABELS.len() - 1],
        rect.right(),
        rect.y + 148.0,
        TextStyle::new(12.0, palette::text_dim()),
    );
}

fn draw_bands(bands: &[f64], label: &str, rect: Rect) {
    draw_ui_text_ex(
        label,
        rect.x,
        rect.y - 4.0,
        TextStyle::new(13.0, palette::text_dim()).params(),
    );

    let bar = Rect::new(rect.x, rect.y + 2.0, rect.w, rect.h - 6.0);
    let mut x = bar.x;
    for (index, share) in bands.iter().enumerate() {
        let width = bar.w * *share as f32;
        if width < 0.5 {
            continue;
        }
        let heat = index as f32 / (bands.len() as f32 - 1.0);
        draw_rectangle(
            x,
            bar.y,
            width,
            bar.h,
            Color::new(0.10 + 0.72 * heat, 0.09 + 0.30 * heat, 0.10, 1.0),
        );
        x += width;
    }
    draw_surface(
        bar,
        &SurfaceStyle::new(Color::new(0.0, 0.0, 0.0, 0.0)).with_border(1.0, palette::gold_dim()),
    );
}

/// The part that stops the numbers above being read as a verdict.
///
/// A few hundred rounds cannot measure a slot — §5.17 established that twenty
/// *thousand* could not measure RTP to five points. Saying so here is the whole
/// reason the panel is worth having: without it, a player on a cold run has a
/// chart that looks like evidence.
fn draw_caveat(entry: &MachineLedger, profile: Option<&MachineProfile>, rect: Rect) {
    let text = match profile {
        Some(profile) if entry.rounds() >= 2 => {
            let margin = entry.margin(profile.volatility);
            format!(
                "Over {} rounds your return could plausibly sit {:.0} points either side of the machine's, \
                 purely by chance. The gap between the two bars above is variance, not the cabinet changing \
                 its mind. It narrows with the square root of how much you play, which is slowly.",
                entry.rounds(),
                (margin * 100.0).min(999.0)
            )
        }
        _ => String::from(
            "Play a few more rounds and this will start to mean something. Not much, but something.",
        ),
    };

    draw_text_block(
        &text,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        15.0,
        4.0,
        palette::text_dim(),
    );
}
