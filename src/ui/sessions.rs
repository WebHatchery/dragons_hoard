//! The log of sessions played (§5.70).
//!
//! Deliberately plain. This is a page of figures about the player's own
//! evenings, and the one thing it must not do is editorialise: no colour for a
//! good night, no encouragement after a bad one, no streaks. A row is a row.
//!
//! The only judgement it makes is the one §5.18 makes at length and this one
//! repeats in a sentence — that none of it says anything about the cabinets.
//! Twenty sessions is a rounding error against the twenty thousand spins a
//! machine profile needs, and a player looking at a column of red would
//! otherwise be entitled to conclude something about the machine that is not in
//! the numbers.

use crate::state::sessions::{EndedBy, Session, SessionLog};
use crate::ui::nav::Nav;
use crate::ui::{frame, logical_width, naming, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    ButtonTone, Pointer, Region, SurfaceStyle, TextStyle,
};

const PANEL: Color = Color::new(0.09, 0.09, 0.10, 1.0);
const ROW: f32 = 26.0;

pub fn draw(log: &SessionLog, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.88),
    );

    let panel = frame::centred_at(880.0, frame::BELOW_HEADER, 600.0);
    let _region = Region::on(panel, PANEL);
    draw_surface(
        panel,
        &SurfaceStyle::new(PANEL)
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        &title(log),
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
        actions.push(UiAction::ToggleSessions);
    }

    if log.is_empty() {
        draw_text_block(
            "Nothing here yet. A session is recorded when you stop playing — either \
             because a limit you set has been reached, or because you start a new one.",
            panel.x + 24.0,
            panel.y + 82.0,
            panel.w - 48.0,
            80.0,
            17.0,
            4.0,
            palette::text(),
        );
        return;
    }

    let columns = [
        ("", 0.0),
        ("Played", 150.0),
        ("Spins", 250.0),
        ("Staked", 380.0),
        ("Came back", 520.0),
        ("Net", 650.0),
        ("Ended", 780.0),
    ];
    for (label, offset) in columns {
        if label.is_empty() {
            continue;
        }
        draw_text_right(
            label,
            panel.x + 24.0 + offset,
            panel.y + 74.0,
            TextStyle::new(13.0, palette::text_dim()),
        );
    }

    let mut y = panel.y + 84.0;
    for (index, session) in log.recent().enumerate() {
        // The session being played is lit, because it is the only row that is
        // still changing and a reader is owed that (§5.71).
        let open = index == 0 && log.first_is_open();
        let tone = if open {
            palette::gold_bright()
        } else {
            palette::text()
        };
        for (value, offset) in row(session, log.len() - index, open) {
            draw_text_right(
                &value,
                panel.x + 24.0 + offset,
                y + 17.0,
                TextStyle::new(15.0, tone),
            );
        }
        y += ROW;
    }

    let (spins, staked, returned) = log.totals();
    draw_text_right(
        &format!(
            "{} sessions  ·  {} spins  ·  {} staked  ·  {} back",
            log.len(),
            naming::count(spins as u64),
            naming::credits(staked),
            naming::credits(returned)
        ),
        panel.right() - 24.0,
        panel.bottom() - 54.0,
        TextStyle::new(15.0, palette::text_bright()),
    );
    draw_text_centered_in_box_ex(
        "Twenty sessions is far too little play to say anything about the cabinets.",
        panel.x,
        panel.bottom() - 34.0,
        panel.w,
        20.0,
        TextStyle::new(14.0, palette::text_dim()),
    );
}

fn title(log: &SessionLog) -> String {
    match log.len() {
        0 => "Your sessions".to_owned(),
        1 => "Your last session".to_owned(),
        many => format!("Your last {} sessions", many),
    }
}

/// One line of the log, as `(text, right edge)` pairs.
fn row(session: &Session, ordinal: usize, open: bool) -> Vec<(String, f32)> {
    vec![
        (format!("#{}", ordinal), 40.0),
        (minutes(session), 150.0),
        (naming::count(session.spins as u64), 250.0),
        (naming::credits(session.staked), 380.0),
        (naming::credits(session.returned), 520.0),
        (naming::net(session.net()), 650.0),
        (ended(session, open), 780.0),
    ]
}

fn minutes(session: &Session) -> String {
    match session.minutes() {
        0 => "<1 min".to_owned(),
        many => format!("{} min", many),
    }
}

/// Why it stopped, in as few words as the column has room for.
fn ended(session: &Session, open: bool) -> String {
    if open {
        // It has not ended, and saying "you stopped" about the evening someone
        // is in the middle of would be the one plainly false thing on the page.
        return "playing now".to_owned();
    }
    match session.ended_by {
        Some(EndedBy::Time) => "time limit".to_owned(),
        Some(EndedBy::Loss) => "loss limit".to_owned(),
        Some(EndedBy::Spins) => "spin limit".to_owned(),
        None => "you stopped".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::limits::{Breach, SessionClock};

    fn log(count: usize) -> SessionLog {
        let mut log = SessionLog::default();
        for index in 0..count {
            let mut clock = SessionClock::default();
            for _ in 0..(10 + index) {
                clock.record(20, 14);
            }
            clock.elapsed = 600.0;
            log.record(&clock, 500, 0, Some(Breach::Time(10)));
        }
        log
    }

    /// The title counts, and reads properly at one.
    #[test]
    fn the_title_says_how_many_there_are() {
        assert_eq!(title(&SessionLog::default()), "Your sessions");
        assert_eq!(title(&log(1)), "Your last session");
        assert_eq!(title(&log(3)), "Your last 3 sessions");
    }

    /// Numbered newest-highest, so the ordinal does not change meaning as the
    /// log fills — session #7 stays the seventh one recorded.
    #[test]
    fn the_newest_session_carries_the_highest_number() {
        let log = log(4);
        let first = log.recent().next().unwrap();
        assert_eq!(row(first, 4, false)[0].0, "#4");
    }

    /// Every session says why it stopped, including the ones nothing stopped.
    #[test]
    fn every_row_says_how_the_session_ended() {
        for session in log(3).recent() {
            assert!(!ended(session, false).is_empty());
        }
        let left = Session {
            seconds: 300,
            spins: 20,
            staked: 400,
            returned: 380,
            best: 40,
            staked_by_vault: 0,
            ended_by: None,
        };
        assert_eq!(ended(&left, false), "you stopped");
        assert_eq!(
            ended(&left, true),
            "playing now",
            "the row someone is in the middle of must not claim to be over"
        );
    }

    /// The columns are laid out by right edge and must not run into each other
    /// — a row is seven figures and the panel is not wide.
    #[test]
    fn the_columns_are_in_order_and_do_not_collide() {
        let session = *log(1).recent().next().unwrap();
        let cells = row(&session, 1, false);
        for pair in cells.windows(2) {
            assert!(
                pair[1].1 > pair[0].1,
                "columns at {} and {} are out of order",
                pair[0].1,
                pair[1].1
            );
        }
        assert!(cells.last().unwrap().1 < 880.0 - 48.0);
    }
}
