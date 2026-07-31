//! The jackpot ladder, above the reels.
//!
//! Four plates showing what each progressive tier is worth right now, sitting
//! between the panel title and the symbol window — where a real cabinet puts
//! them, and where the player sees them on every spin (§5.6).
//!
//! Split out of `reels.rs` when it crossed 800 lines (§5.79). The cut is a real
//! one: everything left next door is about symbols moving, and none of this is.

use super::jackpot_strip_rect;
use crate::data::GameData;
use crate::state::{jackpot, GameSession};
use crate::ui::{naming, palette, TextStyle};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{draw_surface, draw_text_centered_in_box_ex, SurfaceStyle};

/// The progressive ladder: one plate per tier, richest on the right, each
/// showing what it would pay right now. The plates brighten with tier so the
/// eye lands on the Grand.
pub(super) fn draw_jackpot_ladder(
    data: &GameData,
    session: &GameSession,
    shake: Vec2,
    ui_time: f32,
) {
    let rows = jackpot::ladder(&data.jackpots, &session.jackpots);
    if rows.is_empty() {
        return;
    }

    let strip = jackpot_strip_rect().offset(shake);
    let gap = 8.0;
    let width = (strip.w - gap * (rows.len() as f32 - 1.0)) / rows.len() as f32;
    // A slow shimmer so the ladder reads as live rather than painted on.
    let shimmer = 0.5 + 0.5 * (ui_time * 1.6).sin();

    for (index, (name, credits)) in rows.iter().enumerate() {
        let plate = Rect::new(
            strip.x + index as f32 * (width + gap),
            strip.y,
            width,
            strip.h,
        );
        // 0.0 for the smallest tier up to 1.0 for the richest.
        let rank = index as f32 / (rows.len() as f32 - 1.0).max(1.0);
        let fill = Color::new(
            0.09 + 0.10 * rank,
            0.075 + 0.075 * rank,
            0.05 + 0.02 * rank,
            1.0,
        );
        // A pot the whole floor feeds is lit differently rather than labelled:
        // the plate is barely a hundred pixels wide and a second word on it
        // would not survive 130% text, let alone a translation (§5.57). The
        // rules panel carries the explanation.
        let shared = data
            .jackpots
            .tiers
            .get(index)
            .is_some_and(|tier| tier.shared);
        let border = if shared {
            Color::new(
                palette::ember().r,
                palette::ember().g,
                palette::ember().b,
                0.55 + 0.45 * shimmer,
            )
        } else {
            Color::new(
                palette::gold().r,
                palette::gold().g,
                palette::gold().b,
                0.35 + 0.5 * rank * shimmer,
            )
        };

        draw_surface(
            plate,
            &SurfaceStyle::new(fill)
                .with_border(if shared { 2.0 } else { 1.0 }, border)
                .with_top_highlight(2.0, Color::new(1.0, 0.86, 0.45, 0.15 + 0.35 * rank)),
        );
        draw_text_centered_in_box_ex(
            &name.to_uppercase(),
            plate.x,
            plate.y + 2.0,
            plate.w,
            18.0,
            TextStyle::new(13.0, palette::text_dim()),
        );
        draw_text_centered_in_box_ex(
            &naming::credits(*credits),
            plate.x,
            plate.y + 16.0,
            plate.w,
            26.0,
            TextStyle::new(
                21.0,
                if rank > 0.6 {
                    palette::gold_bright()
                } else {
                    palette::gold()
                },
            ),
        );
    }
}
