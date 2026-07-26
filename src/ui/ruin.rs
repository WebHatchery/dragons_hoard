//! The panel a player sees when they cannot afford a spin (§5.53).
//!
//! Not a modal the player opens — one the game deals, like the Vault Pick and
//! the gamble. It is up because the reels cannot turn, and it comes down when
//! they can, so there is no close button: dismissing it would leave the player
//! looking at a cabinet that ignores the spin button.
//!
//! The tone matters here more than anywhere else in the game. This is the
//! moment a player has lost everything they were staking, and a panel that
//! congratulated them, or hurried them, or hid what the offer costs, would be
//! the one screen in this cabinet that was working against them. So the cut is
//! stated in credits rather than a percentage, the eggs given up are counted,
//! and lowering the bet is offered alongside — it is often the better move and
//! the game should say so.

use crate::state::ruin::Lifeline;
use crate::ui::nav::Nav;
use crate::ui::{frame, logical_width, naming, palette, virtual_button, UiAction, LOGICAL_HEIGHT};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_ui_text_ex, ButtonTone,
    Pointer, Region, SurfaceStyle, TextStyle,
};

const PANEL: Color = Color::new(0.13, 0.09, 0.10, 1.0);

pub fn draw(
    lifeline: Lifeline,
    balance: i64,
    cheapest: i64,
    pointer: Pointer,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        logical_width(),
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.86),
    );

    let panel = frame::centred_at(660.0, frame::BELOW_HEADER + 66.0, 372.0);
    let _region = Region::on(panel, PANEL);
    draw_surface(
        panel,
        &SurfaceStyle::new(PANEL)
            .with_border(2.0, palette::gold())
            .with_header(48.0, palette::stone_header())
            .with_header_divider(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        "Out of credits",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::gold_bright()).params(),
    );

    // Say where they are before saying what is on offer. A player who has just
    // lost their stack is owed the number.
    draw_text_block(
        &format!(
            "{} left, and the cheapest spin on this cabinet is {}.",
            naming::credits(balance),
            naming::credits(cheapest)
        ),
        panel.x + 20.0,
        panel.y + 66.0,
        panel.w - 40.0,
        44.0,
        17.0,
        3.0,
        palette::text(),
    );

    let (heading, body, verb) = describe(lifeline);
    let button = format!("{} {}", verb, naming::credits(lifeline.credits()));
    let offer = Rect::new(panel.x + 20.0, panel.y + 118.0, panel.w - 40.0, 124.0);
    draw_surface(
        offer,
        &SurfaceStyle::new(Color::new(0.10, 0.08, 0.05, 1.0)).with_border(1.0, palette::gold_dim()),
    );
    draw_ui_text_ex(
        heading,
        offer.x + 16.0,
        offer.y + 28.0,
        TextStyle::new(19.0, palette::gold_bright()).params(),
    );
    draw_text_block(
        &body,
        offer.x + 16.0,
        offer.y + 42.0,
        offer.w - 32.0,
        76.0,
        16.0,
        3.0,
        palette::text(),
    );

    if virtual_button(
        Rect::new(panel.x + 20.0, panel.bottom() - 104.0, panel.w - 40.0, 46.0),
        &button,
        true,
        ButtonTone::Positive,
        pointer,
        nav,
    ) {
        actions.push(UiAction::TakeLifeline);
    }

    // Offered beside it because it is frequently the better move, and a game
    // that only showed the door it profits from would be a different kind of
    // game to this one.
    draw_text_centered_in_box_ex(
        "Lowering the bet may buy more spins than either.",
        panel.x,
        panel.bottom() - 54.0,
        panel.w,
        20.0,
        TextStyle::new(15.0, palette::text_dim()),
    );

    if virtual_button(
        Rect::new(panel.center().x - 90.0, panel.bottom() - 32.0, 180.0, 26.0),
        "New game",
        true,
        ButtonTone::Danger,
        pointer,
        nav,
    ) {
        actions.push(UiAction::NewGame);
    }
}

/// What the offer is, in the player's own terms.
///
/// The cut is stated as the two numbers rather than a rate: "500 of a 1,000
/// pot" is a thing a person can weigh, and "50% salvage" is jargon that hides
/// what is being given up.
fn describe(lifeline: Lifeline) -> (&'static str, String, &'static str) {
    match lifeline {
        Lifeline::BreakHoard { credits, pot, eggs } => (
            "Break the hoard",
            format!(
                "Your hoard holds {} behind {}. Break it open now and the vault \
                 takes its cut: you keep {}, and the eggs are gone — the meter \
                 starts again from nothing.",
                naming::credits(pot),
                naming::eggs(eggs),
                naming::credits(credits)
            ),
            "Break the hoard for",
        ),
        Lifeline::VaultStake { credits } => (
            "A stake from the vault",
            format!(
                "There is nothing left in the hoard to break. The vault will \
                 advance you {} to keep playing. It is recorded — staked credits \
                 are not winnings, and the session figures count them apart.",
                naming::credits(credits)
            ),
            "Take",
        ),
    }
}
