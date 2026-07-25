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
use crate::ui::nav::Nav;
use crate::ui::{palette, virtual_button, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right, draw_ui_text_ex,
    ButtonTone, SurfaceStyle, TextStyle,
};

const PANEL: Rect = Rect::new(160.0, 90.0, 960.0, 540.0);

pub fn draw(
    history: &History,
    ledger: &Ledger,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.80),
    );
    draw_surface(
        PANEL,
        &SurfaceStyle::new(palette::STONE)
            .with_border(2.0, palette::GOLD)
            .with_header(48.0, palette::STONE_HEADER)
            .with_header_divider(1.0, palette::GOLD_DIM),
    );
    draw_ui_text_ex(
        "This Session",
        PANEL.x + 20.0,
        PANEL.y + 32.0,
        TextStyle::new(21.0, palette::GOLD_BRIGHT).params(),
    );
    draw_text_right(
        &format!("{} rounds across every cabinet", history.rounds()),
        PANEL.right() - 140.0,
        PANEL.y + 31.0,
        TextStyle::new(14.0, palette::TEXT_DIM),
    );
    if virtual_button(
        Rect::new(PANEL.right() - 130.0, PANEL.y + 9.0, 110.0, 30.0),
        "Close",
        true,
        ButtonTone::Danger,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleHistory);
    }

    let plot = Rect::new(PANEL.x + 20.0, PANEL.y + 62.0, PANEL.w - 40.0, 300.0);
    if history.is_empty() {
        draw_text_centered_in_box_ex(
            "Play a round and the shape of it appears here.",
            plot.x,
            plot.y,
            plot.w,
            plot.h,
            TextStyle::new(18.0, palette::TEXT_DIM),
        );
        return;
    }

    draw_plot(history, plot);
    draw_figures(
        history,
        ledger,
        Rect::new(PANEL.x + 20.0, plot.bottom() + 16.0, PANEL.w - 40.0, 60.0),
    );

    draw_text_block(
        "The band is the highest and lowest the bankroll reached; the line closes each step. A \
         long slow decline broken by occasional spikes is what this machine does — the spikes are \
         where the return comes from, and there is no way to know which round is one.",
        PANEL.x + 20.0,
        plot.bottom() + 84.0,
        PANEL.w - 40.0,
        56.0,
        16.0,
        4.0,
        palette::TEXT_DIM,
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
        &SurfaceStyle::new(Color::new(0.05, 0.045, 0.05, 1.0)).with_border(1.0, palette::GOLD_DIM),
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
            &format!("start {}", opening),
            plot.x + 6.0,
            y - 5.0,
            TextStyle::new(12.0, palette::TEXT_DIM).params(),
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
            palette::GOLD_BRIGHT,
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
        Cause::Feature => palette::JADE,
        Cause::Hatch => palette::EMBER,
        Cause::Wrath => Color::new(0.85, 0.35, 0.85, 1.0),
        Cause::Jackpot => palette::GOLD_BRIGHT,
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
        ("Peak", high.to_string(), palette::TEXT_BRIGHT),
        ("Trough", low.to_string(), palette::TEXT_BRIGHT),
        (
            "Deepest fall",
            // "At least", because once buckets merge a peak and the trough after
            // it can share one and their order is no longer known (§5.32).
            format!("at least {}", history.deepest_fall()),
            palette::EMBER,
        ),
        (
            "Net",
            format!("{}{}", if net > 0 { "+" } else { "" }, net),
            match net.signum() {
                1 => palette::JADE,
                -1 => palette::EMBER,
                _ => palette::TEXT_BRIGHT,
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
            TextStyle::new(14.0, palette::TEXT_DIM),
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
            TextStyle::new(13.0, palette::TEXT_DIM).params(),
        );
        x += 14.0 + cause.label().len() as f32 * 7.0 + 14.0;
    }
    let resolution = history.series().resolution();
    if resolution > 1 {
        draw_text_right(
            &format!("each step covers {} rounds", resolution),
            row.right(),
            row.bottom(),
            TextStyle::new(13.0, palette::TEXT_DIM),
        );
    }
    let _ = ledger;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn played(rounds: usize) -> History {
        let mut history = History::default();
        let mut balance = 1_000i64;
        for round in 0..rounds {
            balance -= 20;
            if round % 137 == 0 {
                balance += 3_000;
            }
            history.record(balance.max(0));
        }
        history
    }

    #[test]
    fn the_opening_balance_is_always_in_view() {
        // The reference the whole graph is read against. A session that only
        // ever climbed would otherwise scroll its own baseline off the bottom.
        let mut history = History::default();
        history.record(1_000);
        for round in 0..500 {
            history.record(50_000 + round);
        }
        let (low, high) = range(&history);
        assert!(low <= 1_000.0, "the start fell off the bottom");
        assert!(high >= 50_499.0);
    }

    #[test]
    fn the_range_always_has_height() {
        // A session where the balance never moved would divide by zero.
        let mut history = History::default();
        for _ in 0..50 {
            history.record(1_000);
        }
        let (low, high) = range(&history);
        assert!(high > low);
    }

    #[test]
    fn the_range_covers_every_extreme() {
        let history = played(4_000);
        let (low, high) = range(&history);
        let (min, max) = history.extremes().unwrap();
        assert!(low <= min as f32);
        assert!(high >= max as f32);
    }

    #[test]
    fn every_bucket_lands_inside_the_plot() {
        // The band is drawn from bucket extremes, so a range that did not cover
        // them would draw outside the frame rather than clip.
        let plot = Rect::new(0.0, 0.0, 900.0, 300.0);
        for rounds in [1, 2, 17, 500, 20_000] {
            let history = played(rounds);
            let (low, high) = range(&history);
            let span = (high - low).max(1.0);
            let y_of = |v: f32| plot.bottom() - (v - low) / span * plot.h;

            for bucket in history.series().buckets() {
                for value in [bucket.min, bucket.max, bucket.last] {
                    let y = y_of(value);
                    assert!(
                        y >= plot.y && y <= plot.bottom(),
                        "{} at {} rounds",
                        y,
                        rounds
                    );
                }
            }
        }
    }

    #[test]
    fn a_mark_lands_inside_the_plot_however_long_the_session() {
        // Marks are placed by round against the series clock while the band is
        // placed by bucket index. The two have to agree as the graph decimates.
        let plot = Rect::new(0.0, 0.0, 900.0, 300.0);
        for rounds in [10usize, 900, 50_000] {
            let mut history = History::default();
            for round in 0..rounds {
                history.record(1_000 + (round as i64 % 400));
                if round % (rounds / 8).max(1) == 0 {
                    history.mark(Cause::Feature, 1_000 + (round as i64 % 400));
                }
            }

            let buckets = history.series().buckets().len().max(1) as f32;
            let step = plot.w / buckets;
            let total = history.rounds().max(1) as f32;
            for mark in history.marks() {
                let x = plot.x + (mark.round as f32 / total).clamp(0.0, 1.0) * buckets * step;
                assert!(x >= plot.x - 0.5, "{} before the plot", x);
                assert!(x <= plot.right() + 0.5, "{} past the plot", x);
            }
        }
    }

    #[test]
    fn every_cause_has_a_colour_of_its_own() {
        // Two marks that share a colour are two marks the legend cannot explain.
        for (index, left) in Cause::ALL.iter().enumerate() {
            for right in Cause::ALL.iter().skip(index + 1) {
                let (a, b) = (mark_colour(*left), mark_colour(*right));
                let apart = (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
                assert!(apart > 0.25, "{:?} and {:?} look alike", left, right);
            }
        }
    }

    #[test]
    fn the_legend_fits_across_the_panel() {
        // Drawn by measuring each label; a sixth cause would run off the edge
        // silently rather than wrap.
        let width: f32 = Cause::ALL
            .iter()
            .map(|cause| 14.0 + cause.label().len() as f32 * 7.0 + 14.0)
            .sum();
        assert!(width < PANEL.w - 40.0 - 200.0, "legend is {}px", width);
    }
}
