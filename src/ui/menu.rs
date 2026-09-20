//! Player-facing routes for the screens that are not part of the next spin.
//!
//! The registry remains the reachability source, but the presentation is no
//! longer a flat dump of every flag. Common play, records, saving/settings and
//! developer-facing inspection each get a predictable home (§5.72).

use crate::game::screens::Screen;
use crate::ui::nav::Nav;
use crate::ui::{frame, logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone, Pointer, Region,
    SurfaceStyle, TextStyle,
};

const PANEL: Color = Color::new(0.10, 0.09, 0.11, 1.0);
const GROUPS: [(&str, &[Screen]); 4] = [
    (
        "Play & learn",
        &[
            Screen::Machines,
            Screen::Rules,
            Screen::Paytable,
            Screen::Lines,
            Screen::FeatureBuy,
        ],
    ),
    (
        "Records",
        &[
            Screen::Ledger,
            Screen::History,
            Screen::Sessions,
            Screen::Achievements,
        ],
    ),
    (
        "Save & settings",
        &[
            Screen::Storage,
            Screen::Settings,
            Screen::Limits,
            Screen::RealityCheck,
            Screen::Vision,
        ],
    ),
    ("Advanced inspection", &[Screen::Proofs, Screen::Waveforms]),
];

/// Buttons within each category use two columns where the frame permits it.
pub const COLUMNS: usize = 2;

pub fn draw(pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.88),
    );

    let available: Vec<Screen> = Screen::in_menu().collect();
    let groups: Vec<(&str, Vec<Screen>)> = GROUPS
        .iter()
        .map(|(title, screens)| {
            (
                *title,
                screens
                    .iter()
                    .copied()
                    .filter(|screen| available.contains(screen))
                    .collect::<Vec<_>>(),
            )
        })
        .filter(|(_, screens)| !screens.is_empty())
        .collect();

    let columns = if frame::is_portrait() { 1 } else { 2 };
    let mut column_heights = vec![0.0; columns];
    for (index, (_, screens)) in groups.iter().enumerate() {
        let rows = screens.len().div_ceil(COLUMNS);
        column_heights[index % columns] += 28.0 + rows as f32 * 52.0 + 14.0;
    }
    let panel_height = 102.0 + column_heights.iter().copied().fold(0.0, f32::max);
    let panel = frame::centred_at(
        if frame::is_portrait() { 720.0 } else { 760.0 },
        frame::BELOW_HEADER + 20.0,
        panel_height,
    );
    let _region = Region::on(panel, PANEL);
    draw_surface(
        panel,
        &SurfaceStyle::new(PANEL)
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "More",
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
        actions.push(UiAction::ToggleMenu);
    }

    let column_width = (panel.w - 48.0 - (columns - 1) as f32 * 24.0) / columns as f32;
    let button_width = (column_width - 12.0) / COLUMNS as f32;
    let mut column_y = vec![panel.y + 62.0; columns];
    for (index, (title, screens)) in groups.iter().enumerate() {
        let column = index % columns;
        let x = panel.x + 24.0 + column as f32 * (column_width + 24.0);
        let y = column_y[column];
        draw_ui_text_ex(
            title,
            x,
            y + 18.0,
            TextStyle::new(17.0, palette::gold()).params(),
        );
        for (screen_index, screen) in screens.iter().enumerate() {
            let slot = Rect::new(
                x + (screen_index % COLUMNS) as f32 * (button_width + 12.0),
                y + 28.0 + (screen_index / COLUMNS) as f32 * 52.0,
                button_width,
                44.0,
            );
            if virtual_button(
                slot,
                screen.label(),
                true,
                ButtonTone::Secondary,
                pointer,
                nav,
            ) {
                actions.push(UiAction::OpenScreen(*screen));
            }
        }
        column_y[column] += 28.0 + screens.len().div_ceil(COLUMNS) as f32 * 52.0 + 14.0;
    }

    draw_text_centered_in_box_ex(
        "Choose a destination; advanced inspection is kept at the end.",
        panel.x,
        panel.bottom() - 30.0,
        panel.w,
        20.0,
        TextStyle::new(14.0, palette::text_dim()),
    );
}

// Tests live in the crate-level integration harness.
