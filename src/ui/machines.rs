//! The machine picker.
//!
//! Each cabinet is a whole separate maths model with its own balance, hoard and
//! jackpots (§5.8), so this is closer to walking to a different machine than to
//! changing a theme — the panel says as much.

use crate::data::{GameData, MachineDef, MACHINES};
use crate::engine::sim::BAND_LABELS;
use crate::state::profile::{MachineProfile, ProfileBook};
use crate::ui::nav::Nav;
use crate::ui::{palette, virtual_button, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    ButtonTone, Region, SurfaceStyle, TextStyle,
};

const ROW_HEIGHT: f32 = 140.0;

pub fn draw(
    data: &GameData,
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

    let height = 120.0 + MACHINES.len() as f32 * ROW_HEIGHT;
    let panel = Rect::new(280.0, (LOGICAL_HEIGHT - height) * 0.5, 720.0, height);
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::new(panel);
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::STONE)
            .with_border(2.0, palette::GOLD)
            .with_header(48.0, palette::STONE_HEADER)
            .with_header_divider(1.0, palette::GOLD_DIM),
    );
    draw_ui_text_ex(
        "Choose a machine",
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
        actions.push(UiAction::ToggleMachines);
    }

    for (index, machine) in MACHINES.iter().enumerate() {
        let row = Rect::new(
            panel.x + 20.0,
            panel.y + 62.0 + index as f32 * ROW_HEIGHT,
            panel.w - 40.0,
            ROW_HEIGHT - 12.0,
        );
        draw_row(data, profiles, machine, row, mouse, actions, nav);
    }

    // Wrapped rather than set on one line: at 130% text it ran 234px past the
    // panel (§5.38). A footnote is exactly the kind of long, low-priority prose
    // that should reflow instead of insisting on its width.
    draw_text_block(
        "Each machine keeps its own balance, hoard and jackpots. Figures are measured live over          20,000 spins, not quoted.",
        panel.x + 20.0,
        panel.bottom() - 44.0,
        panel.w - 40.0,
        38.0,
        15.0,
        3.0,
        palette::TEXT_DIM,
    );
}

fn draw_row(
    data: &GameData,
    profiles: &ProfileBook,
    machine: &'static MachineDef,
    row: Rect,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    let playing = machine.id == data.machine_id();
    let fill = if playing {
        Color::new(0.16, 0.13, 0.06, 1.0)
    } else {
        Color::new(0.09, 0.082, 0.095, 1.0)
    };
    draw_surface(
        row,
        &SurfaceStyle::new(fill)
            .with_border(
                if playing { 2.0 } else { 1.0 },
                if playing {
                    palette::GOLD_BRIGHT
                } else {
                    palette::GOLD_DIM
                },
            )
            .with_left_accent(4.0, palette::GOLD),
    );

    // The display name lives in each machine's own config, so the picker reads
    // it from there rather than duplicating it in the catalog.
    let name = machine_display_name(data, machine);
    draw_ui_text_ex(
        &name,
        row.x + 18.0,
        row.y + 32.0,
        TextStyle::new(23.0, palette::GOLD_BRIGHT).params(),
    );
    // Clipped to leave the Play button alone — the longest blurb ran straight
    // under it.
    draw_text_block(
        machine.blurb,
        row.x + 18.0,
        row.y + 46.0,
        row.w - 220.0,
        22.0,
        15.0,
        2.0,
        palette::TEXT,
    );

    draw_profile(
        profiles,
        machine.id,
        Rect::new(row.x + 18.0, row.y + 68.0, row.w - 200.0, 56.0),
    );

    if playing {
        draw_text_centered_in_box_ex(
            "Playing",
            row.right() - 180.0,
            row.y + 20.0,
            160.0,
            44.0,
            TextStyle::new(18.0, palette::TEXT_DIM),
        );
    } else if virtual_button(
        Rect::new(row.right() - 180.0, row.y + 20.0, 160.0, 44.0),
        "Play",
        true,
        ButtonTone::Positive,
        mouse,
        nav,
    ) {
        actions.push(UiAction::SelectMachine(
            MACHINES
                .iter()
                .position(|entry| entry.id == machine.id)
                .unwrap_or(0),
        ));
    }
}

/// The active machine's name is already loaded; others are read from the
/// catalog entry's own config so the picker never goes stale against the JSON.
fn machine_display_name(data: &GameData, machine: &'static MachineDef) -> String {
    if machine.id == data.machine_id() {
        return data.config.display_name.clone();
    }
    GameData::load_machine(machine)
        .map(|other| other.config.display_name)
        .unwrap_or_else(|_| machine.id.to_owned())
}

/// What the profiler has measured about this cabinet, or how far it has got.
///
/// A row that simply showed nothing until the numbers arrived would look
/// broken, so an unmeasured machine says so and shows its progress.
fn draw_profile(profiles: &ProfileBook, machine_id: &str, rect: Rect) {
    match profiles.get(machine_id) {
        Some(profile) => draw_measured(profile, rect),
        None => {
            let progress = profiles.progress(machine_id);
            let bar = Rect::new(rect.x, rect.y + 10.0, rect.w * 0.45, 10.0);
            draw_surface(
                bar,
                &SurfaceStyle::new(Color::new(0.07, 0.06, 0.07, 1.0))
                    .with_border(1.0, palette::GOLD_DIM),
            );
            draw_rectangle(
                bar.x + 1.0,
                bar.y + 1.0,
                (bar.w - 2.0) * progress,
                bar.h - 2.0,
                palette::EMBER,
            );
            draw_ui_text_ex(
                "measuring this cabinet...",
                bar.right() + 12.0,
                rect.y + 19.0,
                TextStyle::new(14.0, palette::TEXT_DIM).params(),
            );
        }
    }
}

fn draw_measured(profile: &MachineProfile, rect: Rect) {
    // Deliberately no return percentage. A 20,000-round sample measures hit
    // frequency and the band shape well and RTP not at all — the first attempt
    // put Dragon's Hoard at 87.4% against a true 92.7%, and Frost Wyrm at 95.5%
    // against 91.2%. A slot's return needs millions of rounds to settle, so
    // quoting one here would be inventing precision. What the sample *can*
    // support is printed instead.
    draw_ui_text_ex(
        &format!(
            "Pays on {:.0}% of spins   ·   {} volatility   ·   best seen {:.0}x",
            profile.hit_frequency * 100.0,
            profile.volatility_label(),
            profile.best_round
        ),
        rect.x,
        rect.y + 14.0,
        TextStyle::new(14.0, palette::TEXT_BRIGHT).params(),
    );

    // The bands are what actually communicate volatility: two cabinets can both
    // return 95% and feel nothing alike, and the shape of this bar is the
    // difference.
    let bar = Rect::new(rect.x, rect.y + 24.0, rect.w, 16.0);
    let mut x = bar.x;
    for (index, share) in profile.bands.iter().enumerate() {
        let width = bar.w * *share as f32;
        if width < 0.5 {
            continue;
        }
        // Nothing is the flattest colour; the bigger the band the hotter it is.
        let heat = index as f32 / (profile.bands.len() as f32 - 1.0);
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
        &SurfaceStyle::new(Color::new(0.0, 0.0, 0.0, 0.0)).with_border(1.0, palette::GOLD_DIM),
    );

    // Label only the two ends: a legend for seven bands would be longer than
    // the bar it explains.
    draw_ui_text_ex(
        BAND_LABELS[0],
        bar.x,
        bar.bottom() + 14.0,
        TextStyle::new(12.0, palette::TEXT_DIM).params(),
    );
    draw_text_right(
        BAND_LABELS[BAND_LABELS.len() - 1],
        bar.right(),
        bar.bottom() + 14.0,
        TextStyle::new(12.0, palette::TEXT_DIM),
    );
}
