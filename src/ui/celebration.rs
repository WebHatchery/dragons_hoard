//! Full-screen celebration card rendering.

use crate::state::celebration::{Celebration, CelebrationKind};
use crate::ui::{logical_width, palette, LOGICAL_HEIGHT};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{draw_surface, draw_text_centered_in_box_ex, SurfaceStyle, TextStyle};

const CARD_WIDTH: f32 = 720.0;
const CARD_HEIGHT: f32 = 300.0;

pub fn draw(celebration: &Celebration) {
    let alpha = celebration.alpha().clamp(0.0, 1.0);
    let scale = celebration.scale();
    let kind = celebration.kind();

    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        crate::ui::frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.88 * alpha),
    );

    let width = CARD_WIDTH * scale;
    let height = CARD_HEIGHT * scale;
    let card = Rect::new(
        (logical_width() - width) * 0.5,
        (LOGICAL_HEIGHT - height) * 0.5,
        width,
        height,
    );

    let (fill, accent) = card_colors(kind);
    draw_surface(
        card,
        &SurfaceStyle::new(fade(fill, alpha))
            .with_border(3.0, fade(accent, alpha))
            .with_top_highlight(4.0, fade(palette::gold_bright(), alpha * 0.9)),
    );

    draw_text_centered_in_box_ex(
        kind.heading(),
        card.x,
        card.y + 30.0 * scale,
        card.w,
        50.0 * scale,
        TextStyle::new(26.0 * scale, fade(accent, alpha)),
    );
    draw_text_centered_in_box_ex(
        &kind.title(),
        card.x,
        card.y + 88.0 * scale,
        card.w,
        90.0 * scale,
        TextStyle::new(64.0 * scale, fade(palette::gold_bright(), alpha)),
    );
    draw_text_centered_in_box_ex(
        &kind.subtitle(),
        card.x,
        card.y + 190.0 * scale,
        card.w,
        40.0 * scale,
        TextStyle::new(19.0 * scale, fade(palette::text(), alpha)),
    );
    draw_text_centered_in_box_ex(
        "press space to continue",
        card.x,
        card.bottom() - 44.0 * scale,
        card.w,
        30.0 * scale,
        TextStyle::new(14.0 * scale, fade(palette::text_dim(), alpha * 0.8)),
    );
}

/// Where a card's particles should burst from.
pub fn card_center() -> Vec2 {
    vec2(logical_width() * 0.5, LOGICAL_HEIGHT * 0.5)
}

fn card_colors(kind: &CelebrationKind) -> (Color, Color) {
    match kind {
        CelebrationKind::Hatch { .. } => (Color::new(0.20, 0.09, 0.03, 1.0), palette::ember()),
        CelebrationKind::Jackpot { .. } => {
            (Color::new(0.16, 0.13, 0.02, 1.0), palette::gold_bright())
        }
        CelebrationKind::FreeSpinsSummary { .. } => {
            (Color::new(0.08, 0.09, 0.13, 1.0), palette::gold())
        }
        _ => (Color::new(0.15, 0.07, 0.12, 1.0), palette::gold_bright()),
    }
}

fn fade(color: Color, alpha: f32) -> Color {
    Color::new(color.r, color.g, color.b, color.a * alpha)
}
