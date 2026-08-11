//! The session graph (§5.32).
//!
//! # Drawing the envelope, not a line
//!
//! Each bucket knows the highest and lowest the bankroll reached inside it, so
//! the plot is a **filled band between those two** with the closing value traced
//! over it. That matters once the session is long enough for a bucket to cover
//! hundreds of rounds: a line through the closing values alone would step neatly
//! between them and imply a calm that never happened, while the band shows every
//! spike at exactly the height it reached.
//!
//! It is the same decision the toolkit's `Series` makes about what to keep, and
//! it would be wasted if the drawing then threw the extremes away.
//!
//! # The baseline is where the player started
//!
//! Not zero, and not the middle of the range. A graph whose reference line is
//! the opening balance answers "am I up or down" without arithmetic, and the
//! area between the line and that reference is the answer at a glance.

use crate::state::history::{Cause, History};
use crate::state::ledger::Ledger;
use crate::ui::frame;
use crate::ui::naming;
use crate::ui::nav::Nav;
use crate::ui::{logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    ButtonTone, Region, SurfaceStyle, TextStyle,
};

/// Sized per frame, now that the screen can change shape (§5.46).
fn panel() -> Rect {
    frame::centred_at(960.0, 90.0, 540.0)
}

pub fn draw(
    history: &History,
    ledger: &Ledger,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.80),
    );
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::on(panel(), palette::stone());
    draw_surface(
        panel(),
        &SurfaceStyle::new(palette::stone())
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "This Session",
        panel().x + 20.0,
        panel().y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );
    draw_text_right(
        &format!(
            "{} rounds across every cabinet",
            naming::count(history.rounds())
        ),
        panel().right() - 140.0,
        panel().y + 31.0,
        TextStyle::new(14.0, palette::text_dim()),
    );
    if virtual_button(
        crate::ui::close_button(panel()),
        "Close",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleHistory);
    }

    let plot = Rect::new(panel().x + 20.0, panel().y + 62.0, panel().w - 40.0, 300.0);
    if history.is_empty() {
        draw_text_centered_in_box_ex(
            "Play a round and the shape of it appears here.",
            plot.x,
            plot.y,
            plot.w,
            plot.h,
            TextStyle::new(18.0, palette::text_dim()),
        );
        return;
    }

    draw_plot(history, plot);
    draw_figures(
        history,
        ledger,
        Rect::new(
            panel().x + 20.0,
            plot.bottom() + 16.0,
            panel().w - 40.0,
            60.0,
        ),
    );

    draw_text_block(
        "The band is the highest and lowest the bankroll reached; the line closes each step. A \
         long slow decline broken by occasional spikes is what this machine does — the spikes are \
         where the return comes from, and there is no way to know which round is one.",
        panel().x + 20.0,
        plot.bottom() + 84.0,
        panel().w - 40.0,
        56.0,
        16.0,
        4.0,
        palette::text_dim(),
    );
}

/// Vertical range, padded so the line never rides the frame.
fn range(history: &History) -> (f32, f32) {
    let (low, high) = history.extremes().unwrap_or((0, 1));
    let opening = history.opening().unwrap_or(low);
    // The opening balance is always in view, because "am I up or down" is the
    // question the graph exists to answer and the reference has to be visible
    // for the answer to mean anything.
    let low = (low.min(opening)) as f32;
    let high = (high.max(opening)) as f32;
    let pad = ((high - low) * 0.08).max(1.0);
    (low - pad, high + pad)
}

fn draw_plot(history: &History, plot: Rect) {
    draw_surface(
        plot,
        &SurfaceStyle::new(Color::new(0.05, 0.045, 0.05, 1.0))
            .with_border(1.0, palette::gold_dim()),
    );

    let (low, high) = range(history);
    let span = (high - low).max(1.0);
    let y_of = |value: f32| plot.bottom() - (value - low) / span * plot.h;

    // The opening balance, drawn first so everything else sits over it.
    if let Some(opening) = history.opening() {
        let y = y_of(opening as f32);
        draw_line(
            plot.x,
            y,
            plot.right(),
            y,
            1.0,
            Color::new(1.0, 1.0, 1.0, 0.22),
        );
        draw_ui_text_ex(
            &format!("start {}", naming::credits(opening)),
            plot.x + 6.0,
            y - 5.0,
            TextStyle::new(12.0, palette::text_dim()).params(),
        );
    }

    let buckets = history.series().buckets();
    let step = plot.w / buckets.len().max(1) as f32;

    for (index, bucket) in buckets.iter().enumerate() {
        let x = plot.x + index as f32 * step;
        let top = y_of(bucket.max);
        let bottom = y_of(bucket.min);
        // At least a pixel: a bucket whose range is a single value still has to
        // be visible, or a flat stretch would read as missing data.
        draw_rectangle(
            x,
            top,
            step.max(1.0),
            (bottom - top).max(1.0),
            Color::new(0.90, 0.74, 0.36, 0.34),
        );
    }

    // The closing line over the band.
    for (index, pair) in buckets.windows(2).enumerate() {
        let x = plot.x + index as f32 * step + step * 0.5;
        draw_line(
            x,
            y_of(pair[0].last),
            x + step,
            y_of(pair[1].last),
            1.6,
            palette::gold_bright(),
        );
    }

    draw_marks(history, plot, step, &y_of);
}

/// Where each marked moment happened. Placed by round against the series' own
/// clock rather than by bucket index, so a mark stays put as the graph decimates
/// under it.
fn draw_marks(history: &History, plot: Rect, step: f32, y_of: &dyn Fn(f32) -> f32) {
    let rounds = history.rounds().max(1) as f32;
    let buckets = history.series().buckets().len().max(1) as f32;

    for mark in history.marks() {
        let progress = (mark.round as f32 / rounds).clamp(0.0, 1.0);
        let x = plot.x + progress * buckets * step;
        let y = y_of(mark.balance as f32);
        let colour = mark_colour(mark.cause);

        draw_line(
            x,
            plot.y,
            x,
            plot.bottom(),
            1.0,
            Color { a: 0.16, ..colour },
        );
        draw_circle(x, y, 3.5, colour);
    }
}

fn mark_colour(cause: Cause) -> Color {
    match cause {
        Cause::Feature => palette::jade(),
        Cause::Hatch => palette::ember(),
        Cause::Wrath => Color::new(0.85, 0.35, 0.85, 1.0),
        Cause::Seam => Color::new(0.30, 0.85, 0.95, 1.0),
        Cause::Jackpot => palette::gold_bright(),
        Cause::BigWin => Color::new(0.45, 0.70, 1.0, 1.0),
    }
}

/// The numbers under the plot, and the legend for the marks.
fn draw_figures(history: &History, ledger: &Ledger, row: Rect) {
    let (low, high) = history.extremes().unwrap_or((0, 0));
    let opening = history.opening().unwrap_or(0);
    let now = history.series().last().unwrap_or(0.0) as i64;
    let net = now - opening;

    let figures = [
        ("Peak", naming::credits(high), palette::text_bright()),
        ("Trough", naming::credits(low), palette::text_bright()),
        (
            "Deepest fall",
            // "At least", because once buckets merge a peak and the trough after
            // it can share one and their order is no longer known (§5.32).
            format!("at least {}", naming::credits(history.deepest_fall())),
            palette::ember(),
        ),
        (
            "Net",
            naming::net(net),
            match net.signum() {
                1 => palette::jade(),
                -1 => palette::ember(),
                _ => palette::text_bright(),
            },
        ),
    ];

    let column = row.w / figures.len() as f32;
    for (index, (label, value, colour)) in figures.iter().enumerate() {
        let x = row.x + index as f32 * column;
        draw_text_centered_in_box_ex(
            label,
            x,
            row.y,
            column,
            20.0,
            TextStyle::new(14.0, palette::text_dim()),
        );
        draw_text_centered_in_box_ex(
            value,
            x,
            row.y + 20.0,
            column,
            26.0,
            TextStyle::new(21.0, *colour),
        );
    }

    // Legend, and the resolution — a player reading a bucket as one round would
    // misjudge how long a decline actually lasted.
    let mut x = row.x;
    for cause in Cause::ALL {
        draw_circle(x + 5.0, row.bottom() - 4.0, 3.5, mark_colour(cause));
        draw_ui_text_ex(
            cause.label(),
            x + 14.0,
            row.bottom(),
            TextStyle::new(13.0, palette::text_dim()).params(),
        );
        x += 14.0 + cause.label().len() as f32 * 7.0 + 14.0;
    }
    let resolution = history.series().resolution();
    if resolution > 1 {
        draw_text_right(
            &format!(
                "each step covers {} rounds",
                naming::count(resolution as u64)
            ),
            row.right(),
            row.bottom(),
            TextStyle::new(13.0, palette::text_dim()),
        );
    }
    let _ = ledger;
}

#[cfg(test)]
mod tests;
