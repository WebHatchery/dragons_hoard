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
