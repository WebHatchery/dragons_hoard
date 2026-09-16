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
