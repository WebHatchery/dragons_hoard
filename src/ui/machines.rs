//! The machine picker.
//!
//! Each cabinet is a whole separate maths model with its own balance, hoard and
//! jackpots (§5.8), so this is closer to walking to a different machine than to
//! changing a theme — the panel says as much.

use crate::data::{GameData, MachineDef, MACHINES};
use crate::ui::{palette, virtual_button, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone, SurfaceStyle,
    TextStyle,
};

const ROW_HEIGHT: f32 = 96.0;

pub fn draw(data: &GameData, mouse: Vec2, actions: &mut Vec<UiAction>) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.82),
    );

    let height = 120.0 + MACHINES.len() as f32 * ROW_HEIGHT;
    let panel = Rect::new(280.0, (LOGICAL_HEIGHT - height) * 0.5, 720.0, height);
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
        draw_row(data, machine, index, row, mouse, actions);
    }

    draw_ui_text_ex(
        "Each machine keeps its own balance, hoard and jackpots.",
        panel.x + 20.0,
        panel.bottom() - 20.0,
        TextStyle::new(15.0, palette::TEXT_DIM).params(),
    );
}

fn draw_row(
    data: &GameData,
    machine: &'static MachineDef,
    index: usize,
    row: Rect,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
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
    draw_ui_text_ex(
        machine.blurb,
        row.x + 18.0,
        row.y + 58.0,
        TextStyle::new(15.0, palette::TEXT).params(),
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
    ) {
        actions.push(UiAction::SelectMachine(index));
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
