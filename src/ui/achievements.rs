//! The achievements overlay.
//!
//! Locked entries show their target and how close the player is, because an
//! achievement you cannot see the shape of is not a goal — it is a surprise.

use crate::state::achievements::{AchievementBook, ConditionKind};
use crate::ui::frame;
use crate::ui::nav::Nav;
use crate::ui::{logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_right, draw_ui_text_ex, ButtonTone, Region, SurfaceStyle, TextStyle,
};

const ROW_HEIGHT: f32 = 44.0;
/// Header, and the gap under the last row.
const CHROME: f32 = 108.0;
/// Gap between two columns of awards.
const COLUMN_GAP: f32 = 24.0;
/// The panel at one column and at more than one. Wider once it has to flow,
/// because two 400px columns cannot hold a name, a sentence and a tally.
const NARROW: f32 = 820.0;
const WIDE: f32 = 1180.0;

/// How the awards are laid out at this list length and screen height.
///
/// The panel used to be `108 + rows * 44` tall and centred, which is fine until
/// the list outgrows the screen — at fifteen awards it is 768 tall in a 720
/// frame, and the first and last rows are drawn off both ends of it (§5.84).
/// Nothing caught that: the rows are inside the panel, and it was the *panel*
/// that had left the building.
///
/// So the count of columns is derived from what actually fits, the same way the
/// rules panel derives its type size (§5.29). One column while the list is
/// short, which is every cabinet before this one.
fn layout(awards: usize, height: f32) -> (Rect, usize) {
    let room = ((height - CHROME - 24.0) / ROW_HEIGHT).floor().max(1.0) as usize;
    let columns = awards.div_ceil(room).max(1);
    let per_column = awards.div_ceil(columns).max(1);
    let width = if columns > 1 { WIDE } else { NARROW };
    let tall = CHROME + per_column as f32 * ROW_HEIGHT;
    // Centred against the height it was handed rather than through
    // `frame::centred`, so the arithmetic can be tested at a screen size that is
    // not the one the game happens to be running at.
    let panel = Rect::new(
        ((frame::width() - width) * 0.5).max(0.0),
        ((height - tall) * 0.5).max(0.0),
        width,
        tall,
    );
    (panel, per_column)
}

pub fn draw(book: &AchievementBook, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.82),
    );

    let (panel, per_column) = layout(book.defs().len(), frame::height());
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
        crate::ui::close_button(panel),
        "Close",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleAchievements);
    }

    let columns = book.defs().len().div_ceil(per_column).max(1);
    let column_width = (panel.w - 32.0 - COLUMN_GAP * (columns - 1) as f32) / columns as f32;
    for (index, def) in book.defs().iter().enumerate() {
        let row = Rect::new(
            panel.x + 16.0 + (index / per_column) as f32 * (column_width + COLUMN_GAP),
            panel.y + 58.0 + (index % per_column) as f32 * ROW_HEIGHT,
            column_width,
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
        ConditionKind::Wraths => progress.wraths,
        ConditionKind::Seams => progress.seams,
        ConditionKind::Jackpots => progress.jackpots,
        ConditionKind::BiggestWin => progress.biggest_win,
        ConditionKind::Balance => progress.best_balance,
        ConditionKind::MachinesPlayed => progress.machines_played.len() as i64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    /// The panel has to stay on the screen, whatever the list does.
    ///
    /// Asserted against the shipped award list and against lists far longer than
    /// it, because the fault this replaces was not "fifteen awards is too many"
    /// — it was that nothing anywhere related the panel's height to the frame's.
    #[test]
    fn the_panel_never_leaves_the_screen_however_many_awards_there_are() {
        for height in [600.0, 720.0, 900.0] {
            for awards in 1..=60 {
                let (panel, per_column) = layout(awards, height);
                assert!(
                    panel.y >= 0.0 && panel.bottom() <= height,
                    "{} awards at {}px: panel spans {}..{}",
                    awards,
                    height,
                    panel.y,
                    panel.bottom()
                );
                let columns = awards.div_ceil(per_column);
                assert!(
                    columns * per_column >= awards,
                    "{} awards do not fit in {}x{}",
                    awards,
                    columns,
                    per_column
                );
            }
        }
    }

    /// The shipped list, at the size the game actually runs at.
    ///
    /// Fifteen awards is two columns on a 720 frame, and that is the answer
    /// rather than a smaller row: the panel is a list of goals and a goal in
    /// eleven-point type is not one.
    #[test]
    fn the_shipped_list_lands_on_the_screen_in_at_most_two_columns() {
        let data = GameData::load().unwrap();
        let book = AchievementBook::load(&data.config).unwrap();
        let (panel, per_column) = layout(book.defs().len(), frame::height());

        assert!(panel.y >= 0.0 && panel.bottom() <= frame::height());
        assert!(
            book.defs().len().div_ceil(per_column) <= 2,
            "{} awards want {} columns",
            book.defs().len(),
            book.defs().len().div_ceil(per_column)
        );
    }
}
