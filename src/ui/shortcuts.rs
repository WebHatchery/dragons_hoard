//! The keyboard, as a table rather than a list of `if`s (§5.29).
//!
//! # Two keys, one letter
//!
//! The shortcuts were a run of twenty `is_key_pressed` branches and the footer
//! text listing them was a hand-written string. They drifted, and then they
//! collided: `L` opened the Ledger (§5.18) *and* triggered Load, so pressing it
//! threw away the bankroll on the way to a statistics panel. Nothing could catch
//! that, because a duplicated key is two branches that are individually correct.
//!
//! So the bindings are a table. `actions_from_keys` walks it, the footer renders
//! its own line from it, and a test asserts no key is bound twice. The list a
//! player reads is now the list the game obeys, by construction.
//!
//! # Advertised and unadvertised
//!
//! Not every binding belongs in the footer. The waveform and colour-vision
//! panels are development tools (§5.19, §5.24) and Save/Load are on buttons a
//! foot away. Those carry no `label` — still bound, still checked for
//! collisions, just not offered.

use crate::ui::UiAction;
use macroquad::prelude::*;

/// One binding.
pub struct Shortcut {
    /// Every key that fires it. More than one where the same intent has an
    /// obvious second home — `+`/`-` alongside the arrows.
    pub keys: &'static [KeyCode],
    /// Key into `presentation.json`, or `None` for a binding not worth
    /// advertising.
    pub label_key: Option<&'static str>,
    pub action: UiAction,
}

/// The whole keyboard, in the order the footer lists it.
///
/// Space is absent: it is the one key whose meaning depends on what the game is
/// waiting on, and [`actions_from_keys`] handles it directly.
pub static SHORTCUTS: &[Shortcut] = &[
    Shortcut {
        keys: &[KeyCode::Up, KeyCode::Equal],
        label_key: Some("bet"),
        action: UiAction::BetUp,
    },
    Shortcut {
        keys: &[KeyCode::Down, KeyCode::Minus],
        label_key: None, // Advertised by the line above; two halves of one control.
        action: UiAction::BetDown,
    },
    Shortcut {
        keys: &[KeyCode::M],
        label_key: Some("max_bet"),
        action: UiAction::MaxBet,
    },
    Shortcut {
        keys: &[KeyCode::A],
        label_key: Some("autospin"),
        action: UiAction::ToggleAutospin,
    },
    Shortcut {
        keys: &[KeyCode::G],
        label_key: Some("gamble"),
        action: UiAction::OfferGamble,
    },
    Shortcut {
        keys: &[KeyCode::B],
        label_key: Some("buy"),
        action: UiAction::ToggleFeatureBuy,
    },
    Shortcut {
        keys: &[KeyCode::R],
        label_key: Some("rules"),
        action: UiAction::ToggleRules,
    },
    Shortcut {
        keys: &[KeyCode::P],
        label_key: Some("paytable"),
        action: UiAction::TogglePaytable,
    },
    Shortcut {
        keys: &[KeyCode::L],
        label_key: Some("ledger"),
        action: UiAction::ToggleLedger,
    },
    Shortcut {
        keys: &[KeyCode::C],
        label_key: Some("machines"),
        action: UiAction::ToggleMachines,
    },
    Shortcut {
        keys: &[KeyCode::V],
        label_key: Some("awards"),
        action: UiAction::ToggleAchievements,
    },
    Shortcut {
        keys: &[KeyCode::O],
        label_key: Some("settings"),
        action: UiAction::ToggleSettings,
    },
    Shortcut {
        keys: &[KeyCode::H],
        label_key: Some("session"),
        action: UiAction::ToggleHistory,
    },
    Shortcut {
        keys: &[KeyCode::T],
        label_key: Some("limits"),
        action: UiAction::ToggleLimits,
    },
    Shortcut {
        // The footer line is already full and the menu offers this one by name
        // (§5.72), so it carries no label — the key is a convenience, not the
        // way in.
        keys: &[KeyCode::J],
        label_key: None,
        action: UiAction::ToggleProofs,
    },
    Shortcut {
        keys: &[KeyCode::S],
        label_key: None,
        action: UiAction::Save,
    },
    Shortcut {
        // Was `L`, which also opened the ledger — pressing it to read the
        // statistics discarded the bankroll they described.
        keys: &[KeyCode::K],
        label_key: None,
        action: UiAction::Load,
    },
    Shortcut {
        keys: &[KeyCode::W],
        label_key: None,
        action: UiAction::ToggleWaveforms,
    },
    Shortcut {
        keys: &[KeyCode::N],
        label_key: None,
        action: UiAction::ToggleVision,
    },
];

/// The footer's shortcut line, built from the bindings that actually exist.
pub fn footer_line(text: &crate::data::ShortcutText) -> String {
    std::iter::once(text.spin.clone())
        .chain(SHORTCUTS.iter().filter_map(|shortcut| {
            shortcut
                .label_key
                .and_then(|key| text.labels.get(key).cloned())
        }))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Keyboard shortcuts, mapped to the same intents the buttons produce.
///
/// While a celebration is showing, the spin key dismisses it instead — one key
/// to move the game forward, whatever it is currently waiting on.
pub fn actions_from_keys(celebrating: bool) -> Vec<UiAction> {
    let mut actions = Vec::new();
    if is_key_pressed(KeyCode::Space) {
        actions.push(if celebrating {
            UiAction::DismissCelebration
        } else {
            UiAction::Spin
        });
    }
    for shortcut in SHORTCUTS {
        if shortcut.keys.iter().copied().any(is_key_pressed) {
            actions.push(shortcut.action);
        }
    }
    actions
}

// Tests live in the crate-level integration harness.
