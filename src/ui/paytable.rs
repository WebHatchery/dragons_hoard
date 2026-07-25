//! The paytable overlay: what every symbol pays, and the rules in prose.

use crate::ui::nav::Nav;
use crate::ui::{
    palette, symbols, virtual_button, UiAction, UiContext, LOGICAL_HEIGHT, LOGICAL_WIDTH,
};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    ButtonTone, Region, SurfaceStyle, TextStyle,
};

pub fn draw(ctx: &UiContext<'_>, mouse: Vec2, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.72),
    );

    // Tall enough for four paragraphs of rules under nine symbol rows. The
    // Dragon's Wrath note (§5.12) overflowed the old 580 and spilled onto the
    // footer behind the overlay.
    let rect = Rect::new(180.0, 44.0, 920.0, 632.0);
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::new(rect);
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

    // The rules used to be four hardcoded paragraphs here, and they described
    // a two-cabinet game we stopped shipping three cabinets ago (§5.29). They
    // live in `state::rules` now, derived from this machine's own config.
    draw_text_block(
        &format!(
            "Every payout above is a multiple of the line bet, at the {} line bet you have set.
             Press R for how {} plays — its win model, its features, and what each of them is worth.",
            ctx.session.line_bet(ctx.data),
            ctx.data.config.display_name,
        ),
        rect.x + 20.0,
        y + 10.0,
        rect.w - 40.0,
        56.0,
        16.0,
        6.0,
        palette::TEXT_DIM,
    );

    if virtual_button(
        Rect::new(rect.right() - 130.0, rect.y + 10.0, 110.0, 30.0),
        "Close",
        true,
        ButtonTone::Danger,
        mouse,
        nav,
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
