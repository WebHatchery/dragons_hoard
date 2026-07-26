//! The hint bar (§5.28).
//!
//! A single line above the footer, with one key to dismiss it. Deliberately not
//! a modal, a popover or an overlay: a hint that interrupts play is a tutorial,
//! and a hint the player has to close is a demand rather than an offer.
//!
//! It sits where the reels are not, so it never covers the thing the player is
//! actually watching.
//!
//! # The hint opens the thing it names (§5.73)
//!
//! Every hint used to begin "Press R" or "Press C". §5.45 gave the game touch
//! input, and on a touch device that advice is not merely unhelpful — it is a
//! dead end, because the panel it names has no other door. §5.72 fixed exactly
//! this for the screens themselves and did not think to look here.
//!
//! So a hint that names a panel carries a button that opens it. The button is
//! not decoration: `hints.json` gives a [`Screen::id`], the validator checks it
//! against the ones the menu offers, and the bar looks it up in the registry.
//! A hint can therefore only point somewhere a player can actually get to, and
//! the advice can be taken from the hint rather than remembered and acted on
//! later.

use crate::game::screens::Screen;
use crate::state::hints::HintDef;
use crate::ui::nav::{self, Nav};
use crate::ui::{logical_width, palette, virtual_button, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{draw_surface, draw_text_block, ButtonTone, SurfaceStyle};

pub fn draw(hint: &HintDef, pointer: Pointer, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    // Sits on the footer's shortcut line — the small grey text that lists every
    // key and that nobody reads, which is the whole reason this exists. While a
    // hint is up it takes that space rather than fighting it for room.
    let bar = Rect::new(478.0, 668.0, logical_width() - 496.0, 30.0);
    draw_surface(
        bar,
        &SurfaceStyle::new(Color::new(0.16, 0.11, 0.03, 0.96))
            .with_border(1.0, palette::gold_dim())
            .with_left_accent(3.0, palette::ember()),
    );

    let dismiss = Rect::new(bar.right() - 88.0, bar.y + 3.0, 80.0, 24.0);

    // A hint that names a panel opens it. Looked up in the registry rather than
    // matched here, so the set of places a hint may send you is the set of
    // places the menu offers (§5.72) and nothing else.
    let opens = hint
        .screen
        .as_deref()
        .and_then(Screen::from_id)
        .filter(|screen| Screen::in_menu().any(|offered| offered == *screen));
    let open_box = Rect::new(dismiss.x - 92.0, bar.y + 3.0, 84.0, 24.0);
    let text_right = if opens.is_some() {
        open_box.x - 12.0
    } else {
        dismiss.x - 12.0
    };

    // Fitted to the space before the button rather than set at 14px and run
    // under it. On a narrow screen (§5.46) the bar shrinks and the hint does
    // not — which the collision check reported at 446px² (§5.47), and which no
    // amount of looking at a 16:9 capture would ever have shown.
    draw_text_block(
        &crate::state::hints::render(&hint.text),
        bar.x + 16.0,
        bar.y + 4.0,
        text_right - (bar.x + 16.0),
        24.0,
        14.0,
        0.0,
        palette::text_bright(),
    );

    if let Some(screen) = opens {
        if virtual_button(open_box, "Open", true, ButtonTone::Primary, pointer, nav) {
            // Opening it is taking the advice, so the hint has done its job and
            // does not need saying again.
            actions.push(UiAction::DismissHint);
            actions.push(UiAction::OpenScreen(screen));
        }
    }
    if virtual_button(dismiss, "Got it", true, ButtonTone::Secondary, pointer, nav) {
        actions.push(UiAction::DismissHint);
    }
    let _ = nav::focus_ring;
}

#[cfg(test)]
mod tests {
    use crate::game::screens::Screen;
    use crate::state::hints::{self, HintDef};

    fn shipped() -> Vec<HintDef> {
        serde_json::from_str(include_str!("../../assets/data/hints.json")).unwrap()
    }

    /// The fault this section exists for: a hint whose only instruction is a
    /// keypress is a dead end on a touch device (§5.45, §5.72).
    #[test]
    fn no_hint_leaves_a_touch_player_with_only_a_key_to_press() {
        for def in shipped() {
            let text = hints::render(&def.text);
            let names_a_key = text.contains("Press ") || text.contains("press ");
            let has_a_door = def
                .screen
                .as_deref()
                .and_then(Screen::from_id)
                .is_some_and(|screen| Screen::in_menu().any(|offered| offered == screen));
            assert!(
                has_a_door || !names_a_key,
                "hint '{}' tells the player to press a key and offers no button: {}",
                def.id,
                text
            );
        }
    }

    /// The button opens the panel the sentence is about, not some other one.
    #[test]
    fn every_door_leads_where_the_sentence_says() {
        for def in shipped() {
            let Some(id) = def.screen.as_deref() else {
                continue;
            };
            let screen = Screen::from_id(id).expect("validate accepted an unknown screen");
            let text = hints::render(&def.text).to_lowercase();
            assert!(
                text.contains(&screen.label().to_lowercase()),
                "hint '{}' opens {:?} without ever naming it: {}",
                def.id,
                screen.label(),
                text
            );
        }
    }

    /// Nothing left unrendered. A brace on screen is a placeholder that was
    /// never filled in, which is worse than the stale word it replaced.
    #[test]
    fn nothing_reaches_the_bar_with_a_placeholder_in_it() {
        for def in shipped() {
            let text = hints::render(&def.text);
            assert!(
                !text.contains('{') && !text.contains('}'),
                "hint '{}' still has a placeholder: {}",
                def.id,
                text
            );
        }
    }

    /// `from_id` is derived from the registry, so it round-trips every screen
    /// and invents none.
    #[test]
    fn the_registry_looks_itself_up() {
        for screen in Screen::ALL {
            assert_eq!(Screen::from_id(screen.id()), Some(screen));
        }
        assert_eq!(Screen::from_id("nonesuch"), None);
    }
}
