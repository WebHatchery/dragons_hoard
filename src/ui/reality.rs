//! The reality check (§5.30).
//!
//! Three numbers and a sentence. It holds the game — the reels do not turn while
//! it is up — because a notification that can be played through is one that will
//! be played through.
//!
//! # What it does not do
//!
//! It does not congratulate, warn, or advise. A player up on the session is told
//! they are up; a player down is told how far. The figures are the same ones the
//! ledger uses (§5.18), so nothing here is a second opinion.
//!
//! The one editorial line is that a few hundred spins cannot measure a return —
//! the same thing §5.17 learned about twenty thousand rounds, said where it is
//! most likely to be misread.

use crate::state::limits::SessionClock;
use crate::ui::naming;
use crate::ui::nav::Nav;
use crate::ui::{palette, virtual_button, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone,
    Region, SurfaceStyle, TextStyle,
};

pub fn draw(clock: &SessionClock, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    // Darker than the other overlays. This one is meant to interrupt.
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.86),
    );

    let panel = Rect::new(340.0, 176.0, 600.0, 368.0);
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::on(panel, palette::stone());
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::stone())
            .with_border(2.0, palette::gold_bright())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "Reality Check",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );

    draw_ui_text_ex(
        &format!(
            "You have been playing for {}.",
            duration(clock.elapsed, clock.minutes())
        ),
        panel.x + 24.0,
        panel.y + 84.0,
        TextStyle::new(18.0, palette::text_bright()).params(),
    );

    let net = clock.net();
    let figures = [
        (
            "Spins",
            naming::count(clock.spins as u64),
            palette::text_bright(),
        ),
        (
            "Staked",
            naming::credits(clock.staked),
            palette::text_bright(),
        ),
        (
            "Returned",
            naming::credits(clock.returned),
            palette::text_bright(),
        ),
        (
            "Net",
            naming::net(net),
            // The one coloured figure, and it is coloured by fact rather than by
            // sentiment: down is ember, up is jade, level is neither.
            match net.signum() {
                1 => palette::jade(),
                -1 => palette::ember(),
                _ => palette::text_bright(),
            },
        ),
    ];

    let column = (panel.w - 48.0) / figures.len() as f32;
    for (index, (label, value, colour)) in figures.iter().enumerate() {
        let x = panel.x + 24.0 + index as f32 * column;
        draw_text_centered_in_box_ex(
            label,
            x,
            panel.y + 112.0,
            column,
            22.0,
            TextStyle::new(15.0, palette::text_dim()),
        );
        draw_text_centered_in_box_ex(
            value,
            x,
            panel.y + 134.0,
            column,
            34.0,
            TextStyle::new(27.0, *colour),
        );
    }

    draw_text_block(
        &format!(
            "That is a return of {:.1}% so far this session — over {} spins it says almost \
             nothing about the machine, which pays what it pays whatever today looked like.\n\
             Every credit here is play money. Nothing you win or lose is real, and nothing in \
             this game can be bought.",
            clock.rtp() * 100.0,
            naming::count(clock.spins.max(1) as u64),
        ),
        panel.x + 24.0,
        panel.y + 186.0,
        panel.w - 48.0,
        104.0,
        16.0,
        5.0,
        palette::text_dim(),
    );

    if virtual_button(
        Rect::new(panel.x + 24.0, panel.bottom() - 66.0, panel.w - 48.0, 44.0),
        "Continue playing",
        true,
        ButtonTone::Primary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::AcknowledgeRealityCheck);
    }
}

/// Elapsed time in the units a person would use. Seconds below a minute, so a
/// check that fires early does not claim "0 minutes".
fn duration(elapsed: f32, minutes: u32) -> String {
    match minutes {
        0 => format!("{} seconds", elapsed as u32),
        1 => "1 minute".to_owned(),
        60 => "1 hour".to_owned(),
        m if m < 60 => format!("{} minutes", m),
        m if m % 60 == 0 => format!("{} hours", m / 60),
        m => format!("{}h {}m", m / 60, m % 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_sessions_are_reported_in_seconds() {
        // "You have been playing for 0 minutes" is worse than saying nothing.
        assert_eq!(duration(42.0, 0), "42 seconds");
    }

    #[test]
    fn minutes_and_hours_read_as_english() {
        assert_eq!(duration(60.0, 1), "1 minute");
        assert_eq!(duration(1_500.0, 25), "25 minutes");
        assert_eq!(duration(3_600.0, 60), "1 hour");
        assert_eq!(duration(7_200.0, 120), "2 hours");
        assert_eq!(duration(5_400.0, 90), "1h 30m");
    }

    #[test]
    fn every_duration_is_something_a_person_would_say() {
        for minutes in 0..400u32 {
            let text = duration(minutes as f32 * 60.0, minutes);
            assert!(!text.is_empty());
            assert!(!text.starts_with("0 minute"), "{}", text);
            assert!(!text.contains("0h"), "{}", text);
        }
    }
}
