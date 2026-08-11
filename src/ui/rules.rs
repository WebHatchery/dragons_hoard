//! The rules overlay: how this cabinet works (§5.29).
//!
//! Everything here comes from [`crate::state::rules`], which derives it from the
//! machine's own config — so the panel cannot describe a cabinet the player is
//! not sitting at.
//!
//! # Fitting an unknown number of rules
//!
//! A cabinet produces somewhere between eight and thirteen rules depending on
//! what it has, and a sixth cabinet could produce more. Rather than a scroll bar
//! — state to hold, input to plumb, and a panel most players would never reach
//! the bottom of — the whole set is measured first and the type size chosen to
//! make it fit, flowed into two columns. Everything is on screen at once, which
//! is what a rules panel is for.
//!
//! # Measuring is not laying out
//!
//! `wrap_text` needs a loaded font, so it only works inside a running game. If
//! the layout called it directly, none of the arithmetic below could be tested —
//! and the arithmetic is the part that decides whether the panel overflows.
//!
//! So a [`Measure`] is passed in. The game hands over the real font; a test
//! hands over a deliberately pessimistic estimate, which makes the fit it proves
//! a lower bound on the real one. What the tests cannot check — that the chosen
//! size is legible, that the columns look like columns — is what the capture
//! harness is for.

use crate::state::rules::{self, Rule};
use crate::ui::frame;
use crate::ui::nav::Nav;
use crate::ui::{logical_width, palette, virtual_button, UiAction, UiContext};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_ui_text_ex, wrap_text, ButtonTone, Region, SurfaceStyle, TextStyle,
};

/// Sized per frame, now that the screen can change shape (§5.46).
fn panel() -> Rect {
    // Wider than it was: the panel is two fixed columns and §5.57 added a
    // sentence that pushed Frost Wyrm into a third. Every other overlay in the
    // game is already this wide, and the extra 140px is the cheapest room
    // available — shrinking the type instead runs into the readability floor.
    frame::centred_at(1180.0, frame::BELOW_HEADER, 620.0)
}
const COLUMN_GAP: f32 = 28.0;
const PADDING: f32 = 22.0;
const HEADER: f32 = 62.0;
/// Largest body type we would ever want, and the smallest still worth reading.
const MAX_BODY: f32 = 16.0;
const MIN_BODY: f32 = 11.0;

/// How many lines a body of text wraps to at a given width and size.
type Measure<'a> = &'a dyn Fn(&str, f32, f32) -> usize;

pub fn draw(ctx: &UiContext<'_>, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        frame::height(),
        Color::new(0.0, 0.0, 0.0, 0.72),
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
        &format!("How {} plays", ctx.data.config.display_name),
        panel().x + PADDING,
        panel().y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );

    let rules = rules::rules(ctx.data);
    let (column_width, available) = geometry();
    let measure: Measure<'_> = &|text, width, size| wrap_text(text, width, size).len();

    let body = fitting_size(&rules, column_width, available, measure);
    for (rule, slot) in rules.iter().zip(plan(&rules, body, measure)) {
        let x = panel().x + PADDING + slot.column as f32 * (column_width + COLUMN_GAP);
        draw_ui_text_ex(
            &rule.title,
            x,
            slot.y + body + 2.0,
            TextStyle::new(body + 3.0, palette::gold()).params(),
        );
        let mut line_y = slot.y + body + 12.0 + body;
        // This panel wraps its own text and then draws it a line at a time, so
        // it owns the expansion contract that `draw_text_block` handles
        // internally: expand once, here, and suppress it for the draws below
        // (§5.39). Without the guard every line came out separately bracketed
        // and padded, measuring a width no translation would produce.
        let wrapped = wrap_text(&rule.text, column_width, body);
        let _once = macroquad_toolkit::ui::PseudoOnce::new();
        for line in wrapped {
            draw_ui_text_ex(
                &line,
                x,
                line_y,
                TextStyle::new(body, palette::text_dim()).params(),
            );
            line_y += body + 4.0;
        }
    }

    if virtual_button(
        crate::ui::close_button(panel()),
        "Close",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::ToggleRules);
    }
}

/// Columns the panel is laid out in.
///
/// Two until §5.64 added a thirteenth topic and Wyrmspire — which has the most
/// mechanics of any cabinet, six of them — stopped fitting even at the minimum
/// readable size. Three until §5.80 added the Seam, at which point Tidepool
/// stopped fitting for the same reason. Four is still the honest answer: the
/// alternative was to stop explaining something, and this panel exists because
/// the game used to do exactly that (§5.29).
///
/// The count is what gives, not the readability floor. `MIN_BODY` is 11px
/// because that is where the type stops being worth reading, and a panel that
/// bought its layout by going below it would be a panel nobody reads.
const COLUMNS: usize = 4;

/// Column width and the vertical room a column has.
fn geometry() -> (f32, f32) {
    let column_width =
        (panel().w - PADDING * 2.0 - COLUMN_GAP * (COLUMNS as f32 - 1.0)) / COLUMNS as f32;
    let available = panel().bottom() - (panel().y + HEADER) - PADDING;
    (column_width, available)
}

/// Where one rule ends up.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Slot {
    column: usize,
    y: f32,
}

/// Flow the rules down the first column and into the next when one will not
/// fit. A rule is never split across columns — a short column reads better than
/// a sentence that restarts somewhere else.
fn plan(rules: &[Rule], body: f32, measure: Measure<'_>) -> Vec<Slot> {
    let (column_width, available) = geometry();
    let top = panel().y + HEADER;

    let mut slots = Vec::with_capacity(rules.len());
    let mut column = 0;
    let mut y = top;
    for rule in rules {
        let height = block_height(rule, column_width, body, measure);
        if y + height > top + available && y > top {
            column += 1;
            y = top;
        }
        slots.push(Slot { column, y });
        y += height;
    }
    slots
}

/// Height one rule takes: its heading, its wrapped body, and the gap after it.
fn block_height(rule: &Rule, width: f32, body: f32, measure: Measure<'_>) -> f32 {
    let lines = measure(&rule.text, width, body) as f32;
    body + 12.0 + lines * (body + 4.0) + 10.0
}

/// The largest body size at which every rule fits in the two columns.
///
/// Measured against the real wrapped line count rather than an estimate, so a
/// cabinet that grows a mechanic shrinks the type instead of running off the
/// bottom of the panel — which is how the old paytable prose ended up drawing
/// over the footer.
fn fitting_size(rules: &[Rule], _column_width: f32, _available: f32, measure: Measure<'_>) -> f32 {
    let mut size = MAX_BODY;
    while size > MIN_BODY {
        if columns_used(rules, size, measure) <= COLUMNS {
            return size;
        }
        size -= 0.5;
    }
    MIN_BODY
}

/// How many columns this set takes at `body`. Up to `COLUMNS` is a fit.
fn columns_used(rules: &[Rule], body: f32, measure: Measure<'_>) -> usize {
    plan(rules, body, measure)
        .last()
        .map_or(1, |slot| slot.column + 1)
}

#[cfg(test)]
mod tests;
