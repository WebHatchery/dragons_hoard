//! The machine picker.
//!
//! Each cabinet is a whole separate maths model with its own hoard and
//! jackpots (§5.8), so this is closer to walking to a different machine than to
//! changing a theme — the panel says as much.

use crate::data::{GameData, MachineDef, MACHINES};
use crate::engine::sim::BAND_LABELS;
use crate::state::profile::{MachineProfile, ProfileBook};
use crate::ui::frame;
use crate::ui::nav::Nav;
use crate::ui::{logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    ButtonTone, Region, SurfaceStyle, TextStyle,
};

pub const ROW_HEIGHT: f32 = 140.0;
pub const COLUMNS: usize = 2;
pub const COLUMN_GAP: f32 = 20.0;
/// Wide enough that a half-width row still leaves the blurb room beside the
/// Play button.
pub const PANEL_WIDTH: f32 = 1180.0;

/// How many rows the grid needs for the cabinets that exist.
///
/// Read by the layout and by the test that holds the panel inside the screen,
/// so adding a seventh cabinet changes both together.
pub fn panel_rows() -> usize {
    MACHINES.len().div_ceil(COLUMNS)
}

pub fn draw(
    data: &GameData,
    profiles: &ProfileBook,
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

    // Six cabinets at 140px each wanted a 960px panel inside a 720px frame, so
    // the panel was clamped, the last two rows were drawn past the bottom edge
    // and the sixth cabinet could not be chosen at all (§5.50). A single column
    // cannot hold six rows this tall; the grid is the fix, and `panel_rows`
    // below is the rule that stops it happening again.
    let rows = panel_rows();
    let height = 120.0 + rows as f32 * ROW_HEIGHT;
    let panel = frame::centred(PANEL_WIDTH, height);
    let column_w = (panel.w - 40.0 - COLUMN_GAP) / COLUMNS as f32;
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
        "Choose a machine",
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
        actions.push(UiAction::ToggleMachines);
    }

    // Column-major would put Frost Wyrm below Dragon's Hoard and Emberfall at
    // the top of the second column; reading order keeps the catalog's order the
    // order on screen.
    for (index, machine) in MACHINES.iter().enumerate() {
        let row = Rect::new(
            panel.x + 20.0 + (index % COLUMNS) as f32 * (column_w + COLUMN_GAP),
            panel.y + 62.0 + (index / COLUMNS) as f32 * ROW_HEIGHT,
            column_w,
            ROW_HEIGHT - 12.0,
        );
        draw_row(data, profiles, machine, row, pointer, actions, nav);
    }

    // Wrapped rather than set on one line: at 130% text it ran 234px past the
    // panel (§5.38). A footnote is exactly the kind of long, low-priority prose
    // that should reflow instead of insisting on its width.
    draw_text_block(
        "Your credits come with you. Each machine keeps its own hoard and jackpots, and its figures are measured live over 20,000 spins, not quoted.",
        panel.x + 20.0,
        panel.bottom() - 44.0,
        panel.w - 40.0,
        38.0,
        15.0,
        3.0,
        palette::text_dim(),
    );
}

fn draw_row(
    data: &GameData,
    profiles: &ProfileBook,
    machine: &'static MachineDef,
    row: Rect,
    pointer: Pointer,
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
                    palette::gold_bright()
                } else {
                    palette::gold_dim()
                },
            )
            .with_left_accent(4.0, palette::gold()),
    );

    // The display name lives in each machine's own config, so the picker reads
    // it from there rather than duplicating it in the catalog.
    let name = machine_display_name(data, machine);
    draw_ui_text_ex(
        &name,
        row.x + 18.0,
        row.y + 32.0,
        TextStyle::new(23.0, palette::gold_bright()).params(),
    );
    // Clipped to leave the Play button alone — the longest blurb ran straight
    // under it. At half width (§5.50) it needs two lines rather than one, and
    // the profile below moves down to make room.
    draw_text_block(
        machine.blurb,
        row.x + 18.0,
        row.y + 44.0,
        row.w - 220.0,
        40.0,
        15.0,
        2.0,
        palette::text(),
    );

    draw_profile(
        profiles,
        machine.id,
        Rect::new(row.x + 18.0, row.y + 86.0, row.w - 36.0, 40.0),
    );

    if playing {
        draw_text_centered_in_box_ex(
            "Playing",
            row.right() - 180.0,
            row.y + 20.0,
            160.0,
            44.0,
            TextStyle::new(18.0, palette::text_dim()),
        );
    } else if virtual_button(
        Rect::new(row.right() - 180.0, row.y + 20.0, 160.0, 44.0),
        "Play",
        true,
        ButtonTone::Positive,
        pointer,
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
                    .with_border(1.0, palette::gold_dim()),
            );
            draw_rectangle(
                bar.x + 1.0,
                bar.y + 1.0,
                (bar.w - 2.0) * progress,
                bar.h - 2.0,
                palette::ember(),
            );
            draw_ui_text_ex(
                "measuring this cabinet...",
                bar.right() + 12.0,
                rect.y + 19.0,
                TextStyle::new(14.0, palette::text_dim()).params(),
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
        TextStyle::new(14.0, palette::text_bright()).params(),
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
        &SurfaceStyle::new(Color::new(0.0, 0.0, 0.0, 0.0)).with_border(1.0, palette::gold_dim()),
    );

    // Label only the two ends: a legend for seven bands would be longer than
    // the bar it explains.
    draw_ui_text_ex(
        BAND_LABELS[0],
        bar.x,
        bar.bottom() + 14.0,
        TextStyle::new(12.0, palette::text_dim()).params(),
    );
    draw_text_right(
        BAND_LABELS[BAND_LABELS.len() - 1],
        bar.right(),
        bar.bottom() + 14.0,
        TextStyle::new(12.0, palette::text_dim()),
    );
}

// Tests live in the crate-level integration harness.
