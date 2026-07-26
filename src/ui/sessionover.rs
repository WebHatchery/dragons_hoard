//! What happened, when a cap the player set brings the session to an end
//! (§5.68).
//!
//! # The most important moment in the feature had a greyed-out button
//!
//! §5.30 lets a player cap their losses, their time or their spins, and the
//! caps work: play stops the moment one binds. What happened then was that the
//! SPIN button went grey and a notification said
//!
//! > You have reached your 5,000 credit loss limit. Start a new game to play on.
//!
//! and that was all of it. A line of text, dismissed in a few seconds, telling
//! someone who had just hit a limit they set for themselves how to get around
//! it. The single moment where the game most owes the player a clear account of
//! what happened, and it was a toast.
//!
//! # An account, not a funnel
//!
//! So this states the session: what was staked, what came back, how long it
//! took, the best moment in it, and what the vault advanced. It is deliberately
//! not an invitation. There is a way to start a new session, because the caps
//! are the player's own and §5.30's asymmetry already governs it — a loosened
//! cap waits for the next session, a tightened one binds now — but the screen
//! does not lead with it and does not dress it up.
//!
//! The figures are the ones the session actually recorded, from the same clock
//! the cap was measured against. A summary that quoted anything else would be a
//! second opinion about the thing that just stopped play.

use crate::state::limits::{Breach, SessionClock};
use crate::state::GameSession;
use crate::ui::nav::Nav;
use crate::ui::{frame, logical_width, naming, palette, virtual_button, UiAction, LOGICAL_HEIGHT};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    ButtonTone, Pointer, Region, SurfaceStyle, TextStyle,
};

const PANEL: Color = Color::new(0.10, 0.10, 0.12, 1.0);

pub fn draw(
    breach: Breach,
    clock: &SessionClock,
    session: &GameSession,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.9),
    );

    let panel = frame::centred_at(720.0, frame::BELOW_HEADER + 40.0, 452.0);
    let _region = Region::on(panel, PANEL);
    draw_surface(
        panel,
        &SurfaceStyle::new(PANEL)
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "That is the session",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );

    // The cap in the player's own words first. They set it; they are owed the
    // reason play stopped before they are shown anything else.
    draw_text_block(
        &reason(breach),
        panel.x + 22.0,
        panel.y + 64.0,
        panel.w - 44.0,
        44.0,
        17.0,
        3.0,
        palette::text(),
    );

    let mut y = panel.y + 116.0;
    for (label, value) in figures(clock, session) {
        draw_ui_text_ex(
            &label,
            panel.x + 24.0,
            y + 15.0,
            TextStyle::new(16.0, palette::text_dim()).params(),
        );
        draw_text_right(
            &value,
            panel.right() - 24.0,
            y + 15.0,
            TextStyle::new(17.0, palette::text_bright()),
        );
        y += 26.0;
    }

    // Said plainly rather than buried: a session's worth of play says almost
    // nothing about a machine, and the Ledger makes the same point at length.
    draw_text_centered_in_box_ex(
        "One session is far too little play to say anything about the cabinet.",
        panel.x,
        panel.bottom() - 86.0,
        panel.w,
        20.0,
        TextStyle::new(14.0, palette::text_dim()),
    );

    if virtual_button(
        Rect::new(panel.x + 20.0, panel.bottom() - 58.0, 200.0, 34.0),
        "Close",
        true,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::DismissSessionOver);
    }
    // Second, smaller, and on the other side. The caps are the player's own and
    // starting again is theirs to choose — but a screen that led with it would
    // be selling the next session rather than closing this one.
    if virtual_button(
        Rect::new(panel.right() - 220.0, panel.bottom() - 58.0, 200.0, 34.0),
        "New session",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::NewGame);
    }
}

fn reason(breach: Breach) -> String {
    match breach {
        Breach::Time(minutes) => format!(
            "You set a {}-minute limit on this session, and it is up.",
            minutes
        ),
        Breach::Loss(credits) => format!(
            "You set a loss limit of {} for this session, and it has been reached.",
            naming::credits(credits)
        ),
        Breach::Spins(spins) => format!(
            "You set a limit of {} spins on this session, and they are played.",
            naming::count(spins as u64)
        ),
    }
}

/// What the session actually did, from the clock the cap was measured against.
fn figures(clock: &SessionClock, session: &GameSession) -> Vec<(String, String)> {
    let mut rows = vec![
        ("Played for".to_owned(), minutes(clock)),
        ("Spins".to_owned(), naming::count(clock.spins as u64)),
        ("Staked".to_owned(), naming::credits(clock.staked)),
        ("Came back".to_owned(), naming::credits(clock.returned)),
        ("Net".to_owned(), naming::net(clock.net())),
    ];
    if session.stats.biggest_win > 0 {
        rows.push((
            "Best single win".to_owned(),
            naming::credits(session.stats.biggest_win),
        ));
    }
    // Only when there were any: a row reading "Staked by the vault: 0" would
    // invite a question nobody asked (§5.53).
    if session.stats.staked > 0 {
        rows.push((
            "Advanced by the vault".to_owned(),
            naming::credits(session.stats.staked),
        ));
    }
    rows
}

fn minutes(clock: &SessionClock) -> String {
    match clock.minutes() {
        0 => "under a minute".to_owned(),
        1 => "1 minute".to_owned(),
        many => format!("{} minutes", many),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> SessionClock {
        // Built by recording play rather than by setting fields, because the
        // clock owns a private cursor for the reality check and the summary
        // must read the same clock the cap was measured against.
        let mut clock = SessionClock::default();
        for _ in 0..120 {
            clock.record(4_000 / 120, 2_750 / 120);
        }
        clock.elapsed = 605.0;
        clock
    }

    /// Every cap says which cap it was, in the words the player set it in.
    #[test]
    fn each_reason_names_the_limit_that_bound() {
        assert!(reason(Breach::Time(30)).contains("30"));
        assert!(reason(Breach::Loss(5_000)).contains("5,000"));
        assert!(reason(Breach::Spins(200)).contains("200"));
        for breach in [Breach::Time(30), Breach::Loss(5_000), Breach::Spins(200)] {
            let text = reason(breach);
            assert!(
                text.contains("You set"),
                "{:?} does not say it was the player's own: {}",
                breach,
                text
            );
        }
    }

    /// The account has to balance, or the screen is a second opinion about the
    /// thing that just stopped play.
    #[test]
    fn the_figures_come_from_the_clock_the_cap_was_measured_against() {
        let clock = clock();
        let data = crate::data::GameData::load().unwrap();
        let session = GameSession::new(&data, 1);
        let rows = figures(&clock, &session);

        let find = |label: &str| {
            rows.iter()
                .find(|(name, _)| name == label)
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| panic!("no {} row", label))
        };
        assert_eq!(find("Staked"), naming::credits(clock.staked));
        assert_eq!(find("Came back"), naming::credits(clock.returned));
        assert_eq!(find("Net"), naming::net(clock.net()));
        assert_eq!(find("Spins"), "120");
        assert!(clock.net() < 0, "the fixture should be down on the session");
        assert_eq!(find("Played for"), "10 minutes");
    }

    /// Rows that would only invite a question nobody asked stay off.
    #[test]
    fn a_session_that_was_never_staked_says_nothing_about_the_vault() {
        let data = crate::data::GameData::load().unwrap();
        let mut session = GameSession::new(&data, 1);
        assert!(figures(&clock(), &session)
            .iter()
            .all(|(label, _)| label != "Advanced by the vault"));

        session.stats.staked = 200;
        assert!(figures(&clock(), &session)
            .iter()
            .any(|(label, _)| label == "Advanced by the vault"));
    }
}
