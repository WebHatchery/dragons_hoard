//! The achievements overlay.
//!
//! Locked entries show their target and how close the player is, because an
//! achievement you cannot see the shape of is not a goal — it is a surprise.

use crate::state::achievements::{AchievementBook, ConditionKind};
use crate::ui::nav::Nav;
use crate::ui::{palette, virtual_button, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_right, draw_ui_text_ex, ButtonTone, Region, SurfaceStyle, TextStyle,
};

const ROW_HEIGHT: f32 = 44.0;

pub fn draw(book: &AchievementBook, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.82),
    );

    let height = 108.0 + book.defs().len() as f32 * ROW_HEIGHT;
    let panel = Rect::new(230.0, (LOGICAL_HEIGHT - height) * 0.5, 820.0, height);
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::on(panel, palette::stone());
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::stone())
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );

    let (unlocked, total) = book.tally();
    draw_ui_text_ex(
        &format!("Achievements — {} of {}", unlocked, total),
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );
    if virtual_button(
        Rect::new(panel.right() - 120.0, panel.y + 9.0, 100.0, 30.0),
        "Close",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleAchievements);
    }

    for (index, def) in book.defs().iter().enumerate() {
        let row = Rect::new(
            panel.x + 16.0,
            panel.y + 58.0 + index as f32 * ROW_HEIGHT,
            panel.w - 32.0,
            ROW_HEIGHT - 6.0,
        );
        let earned = book.is_unlocked(&def.id);

        draw_surface(
            row,
            &SurfaceStyle::new(if earned {
                Color::new(0.15, 0.125, 0.05, 1.0)
            } else {
                Color::new(0.075, 0.07, 0.08, 1.0)
            })
            .with_left_accent(
                4.0,
                if earned {
                    palette::gold_bright()
                } else {
                    palette::text_dim()
                },
            ),
        );

        let title = if earned {
            palette::gold_bright()
        } else {
            palette::text()
        };
        draw_ui_text_ex(
            &def.name,
            row.x + 16.0,
            row.y + 17.0,
            TextStyle::new(18.0, title).params(),
        );
        draw_ui_text_ex(
            &def.description,
            row.x + 16.0,
            row.y + 33.0,
            TextStyle::new(14.0, palette::text_dim()).params(),
        );

        let have = current(book, def.condition.kind);
        let want = def.condition.at_least;
        draw_text_right(
            &if earned {
                "Earned".to_owned()
            } else {
                format!("{} / {}", have.min(want), want)
            },
            row.right() - 16.0,
            row.y + 26.0,
            TextStyle::new(
                17.0,
                if earned {
                    palette::gold()
                } else {
                    palette::text_dim()
                },
            ),
        );
    }
}

/// The player's running total for a condition, so a locked row can show how far
/// along it is rather than just sitting there greyed out.
fn current(book: &AchievementBook, kind: ConditionKind) -> i64 {
    let progress = book.progress();
    match kind {
        ConditionKind::Spins => progress.spins,
        ConditionKind::FreeSpins => progress.free_spins,
        ConditionKind::Hatches => progress.hatches,
        ConditionKind::Jackpots => progress.jackpots,
        ConditionKind::BiggestWin => progress.biggest_win,
        ConditionKind::Balance => progress.best_balance,
        ConditionKind::MachinesPlayed => progress.machines_played.len() as i64,
    }
}
