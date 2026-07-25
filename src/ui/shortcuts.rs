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
    /// How the footer names it, or `None` for a binding not worth advertising.
    pub label: Option<&'static str>,
    pub action: UiAction,
}

/// The whole keyboard, in the order the footer lists it.
///
/// Space is absent: it is the one key whose meaning depends on what the game is
/// waiting on, and [`actions_from_keys`] handles it directly.
pub static SHORTCUTS: &[Shortcut] = &[
    Shortcut {
        keys: &[KeyCode::Up, KeyCode::Equal],
        label: Some("Up/Down bet"),
        action: UiAction::BetUp,
    },
    Shortcut {
        keys: &[KeyCode::Down, KeyCode::Minus],
        label: None, // Advertised by the line above; two halves of one control.
        action: UiAction::BetDown,
    },
    Shortcut {
        keys: &[KeyCode::M],
        label: Some("M max"),
        action: UiAction::MaxBet,
    },
    Shortcut {
        keys: &[KeyCode::A],
        label: Some("A autospin"),
        action: UiAction::ToggleAutospin,
    },
    Shortcut {
        keys: &[KeyCode::G],
        label: Some("G gamble"),
        action: UiAction::OfferGamble,
    },
    Shortcut {
        keys: &[KeyCode::B],
        label: Some("B buy"),
        action: UiAction::ToggleFeatureBuy,
    },
    Shortcut {
        keys: &[KeyCode::R],
        label: Some("R rules"),
        action: UiAction::ToggleRules,
    },
    Shortcut {
        keys: &[KeyCode::P],
        label: Some("P paytable"),
        action: UiAction::TogglePaytable,
    },
    Shortcut {
        keys: &[KeyCode::L],
        label: Some("L ledger"),
        action: UiAction::ToggleLedger,
    },
    Shortcut {
        keys: &[KeyCode::C],
        label: Some("C machines"),
        action: UiAction::ToggleMachines,
    },
    Shortcut {
        keys: &[KeyCode::V],
        label: Some("V awards"),
        action: UiAction::ToggleAchievements,
    },
    Shortcut {
        keys: &[KeyCode::O],
        label: Some("O settings"),
        action: UiAction::ToggleSettings,
    },
    Shortcut {
        keys: &[KeyCode::T],
        label: Some("T limits"),
        action: UiAction::ToggleLimits,
    },
    Shortcut {
        keys: &[KeyCode::S],
        label: None,
        action: UiAction::Save,
    },
    Shortcut {
        // Was `L`, which also opened the ledger — pressing it to read the
        // statistics discarded the bankroll they described.
        keys: &[KeyCode::K],
        label: None,
        action: UiAction::Load,
    },
    Shortcut {
        keys: &[KeyCode::W],
        label: None,
        action: UiAction::ToggleWaveforms,
    },
    Shortcut {
        keys: &[KeyCode::N],
        label: None,
        action: UiAction::ToggleVision,
    },
];

/// The footer's shortcut line, built from the bindings that actually exist.
pub fn footer_line() -> String {
    std::iter::once("Space spins".to_owned())
        .chain(SHORTCUTS.iter().filter_map(|s| s.label.map(str::to_owned)))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this table exists to make impossible.
    #[test]
    fn no_key_is_bound_twice() {
        let mut seen: Vec<KeyCode> = Vec::new();
        for shortcut in SHORTCUTS {
            for key in shortcut.keys {
                assert!(
                    !seen.contains(key),
                    "{:?} is bound to {:?} and something else",
                    key,
                    shortcut.action
                );
                seen.push(*key);
            }
        }
    }

    /// Space is handled outside the table, so the table must not claim it —
    /// and Enter belongs to the focus ring (§5.27), which would otherwise both
    /// press the focused button and spin.
    #[test]
    fn the_table_leaves_the_reserved_keys_alone() {
        for shortcut in SHORTCUTS {
            for key in shortcut.keys {
                assert!(!matches!(
                    key,
                    KeyCode::Space | KeyCode::Enter | KeyCode::KpEnter | KeyCode::Tab
                ));
            }
        }
    }

    #[test]
    fn no_action_is_bound_twice() {
        // Two keys for one intent is fine inside a `keys` list and a mistake
        // across rows: the second row would be unreachable from the footer.
        let mut seen: Vec<UiAction> = Vec::new();
        for shortcut in SHORTCUTS {
            assert!(!seen.contains(&shortcut.action), "{:?}", shortcut.action);
            seen.push(shortcut.action);
        }
    }

    #[test]
    fn every_advertised_shortcut_is_really_bound() {
        // The footer used to be a hand-written string; this is what stops it
        // going stale again.
        let line = footer_line();
        for shortcut in SHORTCUTS {
            if let Some(label) = shortcut.label {
                assert!(line.contains(label), "{} missing from the footer", label);
            }
        }
        assert!(line.starts_with("Space spins"));
    }

    #[test]
    fn the_footer_line_advertises_the_panels_a_player_needs() {
        // Every overlay a player would want and could not otherwise find. The
        // development panels are deliberately absent.
        let line = footer_line();
        for expected in [
            "rules", "paytable", "ledger", "machines", "buy", "gamble", "limits",
        ] {
            assert!(line.contains(expected), "{} unlisted", expected);
        }
        assert!(!line.contains("waveform"));
    }
}
