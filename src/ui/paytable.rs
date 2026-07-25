//! The paytable overlay: what every symbol pays, and the rules in prose.

use crate::state::{bonus, jackpot};
use crate::ui::{
    palette, symbols, virtual_button, UiAction, UiContext, LOGICAL_HEIGHT, LOGICAL_WIDTH,
};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    ButtonTone, SurfaceStyle, TextStyle,
};

pub fn draw(ctx: &UiContext<'_>, mouse: Vec2, actions: &mut Vec<UiAction>) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.72),
    );

    let rect = Rect::new(180.0, 70.0, 920.0, 580.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(palette::STONE)
            .with_border(2.0, palette::GOLD)
            .with_header(48.0, palette::STONE_HEADER)
            .with_header_divider(1.0, palette::GOLD_DIM),
    );
    draw_ui_text_ex(
        "Paytable — multipliers of the line bet",
        rect.x + 20.0,
        rect.y + 32.0,
        TextStyle::new(21.0, palette::GOLD_BRIGHT).params(),
    );

    let mut y = rect.y + 76.0;
    for (index, def) in ctx.data.symbols.iter() {
        let row = Rect::new(rect.x + 20.0, y, rect.w - 40.0, 44.0);
        let swatch = Rect::new(row.x, row.y - 2.0, 46.0, 42.0);
        let tint = Color::new(def.color[0], def.color[1], def.color[2], 1.0);
        draw_surface(
            swatch,
            &SurfaceStyle::new(Color::new(
                0.055 + tint.r * 0.14,
                0.05 + tint.g * 0.14,
                0.065 + tint.b * 0.14,
                1.0,
            ))
            .with_border(1.0, palette::GOLD_DIM),
        );
        if !symbols::draw(def, swatch, 0.0) {
            draw_text_centered_in_box_ex(
                &def.short,
                swatch.x,
                swatch.y,
                swatch.w,
                swatch.h,
                TextStyle::new(18.0, palette::TEXT_BRIGHT),
            );
        }
        draw_ui_text_ex(
            &def.name,
            row.x + 68.0,
            row.y + 25.0,
            TextStyle::new(18.0, palette::TEXT_BRIGHT).params(),
        );
        draw_ui_text_ex(
            &symbol_note(ctx, index),
            row.x + 270.0,
            row.y + 25.0,
            TextStyle::new(15.0, palette::TEXT_DIM).params(),
        );
        draw_text_right(
            &format!(
                "x3 {}    x4 {}    x5 {}",
                pay_label(ctx, index, 3),
                pay_label(ctx, index, 4),
                pay_label(ctx, index, 5)
            ),
            row.right(),
            row.y + 25.0,
            TextStyle::new(17.0, palette::GOLD),
        );
        y += 46.0;
    }

    draw_text_block(
        &format!(
            "Wins pay left to right from reel 1 on all 20 lines. The Dragon is wild and pays the best reading of a line. Dragon Fire scatters pay anywhere and 3+ award free spins with expanding wilds.\n\
             Progressives: {}% of every stake feeds the four pots, which pay at random on any paid spin. Bigger stakes win them proportionally more often, so the return per credit is the same at every bet — worth {:.1}% of all play.
             The Vault Pick: filling the hoard deals {} chests. Keep picking until {} come up empty; each prize is a share of the hoard, and a board is worth about {:.0}% of it.",
            ctx.data.jackpots.contribution_permille as f32 / 10.0,
            jackpot::expected_rtp(&ctx.data.jackpots) * 100.0,
            ctx.data.bonus.board_size,
            ctx.data.bonus.blanks,
            bonus::expected_permille(&ctx.data.bonus) / 10.0,
        ),
        rect.x + 20.0,
        y + 6.0,
        rect.w - 40.0,
        92.0,
        15.0,
        4.0,
        palette::TEXT_DIM,
    );

    if virtual_button(
        Rect::new(rect.right() - 130.0, rect.y + 10.0, 110.0, 30.0),
        "Close",
        true,
        ButtonTone::Danger,
        mouse,
    ) {
        actions.push(UiAction::TogglePaytable);
    }
}

/// A run that does not pay reads as a dash, not a zero — the lowest symbols
/// deliberately start at four of a kind.
fn pay_label(ctx: &UiContext<'_>, index: usize, count: usize) -> String {
    match ctx.data.symbols.pay(index, count) {
        0 => "-".to_owned(),
        value => value.to_string(),
    }
}

fn symbol_note(ctx: &UiContext<'_>, index: usize) -> String {
    let def = ctx.data.symbols.get(index);
    if def.is_wild {
        "Wild — substitutes for all but the scatter".to_owned()
    } else if def.is_scatter {
        "Scatter — pays total bet, anywhere".to_owned()
    } else if def.is_hoard {
        "Fills the Dragon's Hoard meter".to_owned()
    } else {
        format!("{} tier", def.tier)
    }
}
