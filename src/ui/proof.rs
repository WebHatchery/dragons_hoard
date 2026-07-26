//! Checking a spin (§5.74).
//!
//! The panel behind [`crate::state::proof`]. A row per recent spin: which
//! cabinet, what it paid, and the sixteen hex digits that decided it. **Check
//! them all** re-runs every row through the engine and marks each one.
//!
//! # Deliberately unexciting
//!
//! There is no animation on the tick and no celebration when the row goes
//! green. A verification that performs is a verification asking to be trusted,
//! which is the opposite of the point. The ticks appear, and that is all.
//!
//! # It says what it does not prove
//!
//! The line at the foot is not a disclaimer bolted on for safety — it is the
//! honest half of the claim. This shows an outcome was not altered after the
//! fact. It cannot show the seed was chosen fairly to begin with, because there
//! is no server here to publish a hash before play and reveal it after. A panel
//! that let a row of ticks imply the stronger claim would be doing the thing
//! this whole section exists to stop.

use crate::state::proof::{Commitment, ProofLog, Verdict};
use crate::ui::nav::Nav;
use crate::ui::{frame, logical_width, palette, virtual_button, UiAction, LOGICAL_HEIGHT};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    ButtonTone, Pointer, Region, SurfaceStyle, TextStyle,
};

const PANEL: Color = Color::new(0.09, 0.09, 0.10, 1.0);
const ROW: f32 = 26.0;
/// Rows the panel has room for. The log keeps more than this only if someone
/// raises `proof::KEPT` without looking here, which the tests catch.
const VISIBLE: usize = 12;

pub fn draw(
    log: &ProofLog,
    checked: &[(u64, Verdict)],
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.88),
    );

    let panel = frame::centred_at(900.0, frame::BELOW_HEADER, 580.0);
    let _region = Region::on(panel, PANEL);
    draw_surface(
        panel,
        &SurfaceStyle::new(PANEL)
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "Check a spin",
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
        actions.push(UiAction::ToggleProofs);
    }

    if log.is_empty() {
        draw_text_block(
            "Nothing to check yet. Every spin writes down the number that decides it \
             before the reels turn; play a few and they will be listed here.",
            panel.x + 24.0,
            panel.y + 82.0,
            panel.w - 48.0,
            80.0,
            17.0,
            4.0,
            palette::text_dim(),
        );
        footnote(panel);
        return;
    }

    draw_text_block(
        "Every spin is decided by one number, and the game writes that number down \
         before it draws the reels. Given it back, the same engine runs the same spin \
         again. If anything had been changed afterwards, it would not.",
        panel.x + 24.0,
        panel.y + 62.0,
        panel.w - 48.0,
        56.0,
        16.0,
        3.0,
        palette::text_dim(),
    );

    let top = panel.y + 148.0;
    heading(panel, top - 22.0);

    for (index, entry) in log.entries().iter().take(VISIBLE).enumerate() {
        row(
            panel,
            top + index as f32 * ROW,
            entry,
            verdict(checked, entry),
        );
    }

    let button = Rect::new(panel.x + 24.0, panel.bottom() - 116.0, 180.0, 32.0);
    if virtual_button(
        button,
        "Check them all",
        true,
        ButtonTone::Primary,
        pointer,
        nav,
    ) {
        actions.push(UiAction::CheckProofs);
    }
    if !checked.is_empty() {
        let failed = checked
            .iter()
            .filter(|(_, verdict)| !verdict.is_match())
            .count();
        let (text, colour) = if failed == 0 {
            (
                format!("{} of {} re-ran identically.", checked.len(), checked.len()),
                palette::jade(),
            )
        } else {
            (
                format!("{} of {} did not match.", failed, checked.len()),
                palette::ember(),
            )
        };
        draw_ui_text_ex(
            &text,
            button.right() + 16.0,
            button.y + 22.0,
            TextStyle::new(16.0, colour).params(),
        );
    }

    footnote(panel);
}

fn heading(panel: Rect, y: f32) {
    let style = TextStyle::new(13.0, palette::text_dim());
    draw_ui_text_ex("Spin", panel.x + 24.0, y, style.params());
    draw_ui_text_ex("Cabinet", panel.x + 92.0, y, style.params());
    draw_ui_text_ex("Decided by", panel.x + 268.0, y, style.params());
    draw_text_right("Bet", panel.x + 560.0, y, style);
    draw_text_right("Paid", panel.x + 664.0, y, style);
    draw_ui_text_ex("Re-run", panel.x + 692.0, y, style.params());
}

fn row(panel: Rect, y: f32, entry: &Commitment, verdict: Option<&Verdict>) {
    let style = TextStyle::new(15.0, palette::text_bright());
    draw_ui_text_ex(&entry.seq.to_string(), panel.x + 24.0, y, style.params());
    // Fitted to its column. "Emberfall — 243 Ways" is already long and the 40%
    // pseudolocale (§5.39) pushes it into the hex beside it — the same fault
    // the footer had, in the panel written one section earlier (§5.76).
    draw_text_block(
        &entry.machine_name,
        panel.x + 92.0,
        y - 12.0,
        168.0,
        18.0,
        15.0,
        0.0,
        palette::text_dim(),
    );
    // Monospaced by nothing but luck, so it is left alone rather than padded:
    // sixteen hex digits is already a fixed width.
    //
    // `text`, not `gold_dim`. The dim gold reads as 2.8:1 on this cabinet's
    // palette against the 4.5 the contrast gate wants (§5.40), and it went
    // unnoticed for a whole section because the audit happened to be running
    // under a different cabinet's theme — the one a stray harness run had left
    // in the preferences. This is the number the player is invited to copy out
    // and check by hand; it is the last thing that should be hard to read.
    draw_ui_text_ex(
        &entry.state_hex(),
        panel.x + 268.0,
        y,
        TextStyle::new(15.0, palette::text()).params(),
    );
    draw_text_right(&entry.line_bet.to_string(), panel.x + 560.0, y, style);
    draw_text_right(
        &entry.win.to_string(),
        panel.x + 664.0,
        y,
        TextStyle::new(
            15.0,
            if entry.win > 0 {
                palette::jade()
            } else {
                palette::text_dim()
            },
        ),
    );

    let Some(verdict) = verdict else {
        return;
    };
    let (mark, colour) = if verdict.is_match() {
        ("same spin", palette::jade())
    } else {
        (verdict.message(), palette::ember())
    };
    draw_ui_text_ex(
        mark,
        panel.x + 692.0,
        y,
        TextStyle::new(14.0, colour).params(),
    );
}

fn verdict<'a>(checked: &'a [(u64, Verdict)], entry: &Commitment) -> Option<&'a Verdict> {
    checked
        .iter()
        .find(|(seq, _)| *seq == entry.seq)
        .map(|(_, verdict)| verdict)
}

/// The half of the claim that is a limit rather than a promise.
///
/// Three lines of room for one line of text.
///
/// The pseudolocale gate (§5.39) grows every string by 40%. At one line this
/// sentence ran out through the bottom of the panel and onto the stats behind
/// it by 1201px²; at two it still did by 220px². The gate is worth its cost for
/// exactly this — the sentence looks comfortable in English and is not.
fn footnote(panel: Rect) {
    draw_text_centered_in_box_ex(
        "This shows a spin was not changed after it was drawn. It does not show the \
         first number was picked fairly — that would need a server, and there is none.",
        panel.x + 24.0,
        panel.bottom() - 72.0,
        panel.w - 48.0,
        64.0,
        TextStyle::new(13.0, palette::text_dim()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The panel must have room for everything the log keeps, or a spin the
    /// game promises is checkable quietly is not.
    #[test]
    fn the_panel_shows_every_row_the_log_keeps() {
        const {
            assert!(
                crate::state::proof::KEPT <= VISIBLE * 2,
                "the log keeps more spins than the panel has rows for"
            )
        };
    }

    /// The rows have to fit between the prose and the button under them.
    #[test]
    fn the_rows_fit_the_panel() {
        let panel = frame::centred_at(900.0, frame::BELOW_HEADER, 580.0);
        let last = panel.y + 148.0 + (VISIBLE - 1) as f32 * ROW;
        assert!(
            last < panel.bottom() - 108.0,
            "the last row lands at {} and the button starts at {}",
            last,
            panel.bottom() - 108.0
        );
    }
}
