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
use crate::ui::{frame, logical_width, naming, palette, virtual_button, UiAction};
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
        frame::height(),
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

    // The way to the log (§5.70). Beside the account rather than inside it: this
    // session is one row of a longer story, and a player is more likely to want
    // the story here than anywhere else in the game.
    if virtual_button(
        Rect::new(panel.center().x - 100.0, panel.bottom() - 64.0, 200.0, 44.0),
        "Sessions before this",
        true,
        ButtonTone::Secondary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleSessions);
    }

    if virtual_button(
        Rect::new(panel.x + 20.0, panel.bottom() - 64.0, 200.0, 44.0),
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
        Rect::new(panel.right() - 220.0, panel.bottom() - 64.0, 200.0, 44.0),
        "New session",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::NewGame);
    }
}

pub fn reason(breach: Breach) -> String {
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
pub fn figures(clock: &SessionClock, session: &GameSession) -> Vec<(String, String)> {
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

// Tests live in the crate-level integration harness.
