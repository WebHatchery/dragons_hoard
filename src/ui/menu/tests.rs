use super::*;

/// The property this screen exists for: everything a player can open is
/// offered here, so nothing can be registered and unreachable again.
#[test]
fn the_menu_offers_every_screen_a_player_can_open() {
    let offered: Vec<Screen> = Screen::in_menu().collect();
    for screen in Screen::ALL {
        if !screen.reachable_by_flag() || screen == Screen::Menu {
            continue;
        }
        assert!(
            offered.contains(&screen),
            "{} can be opened and the menu does not offer it",
            screen.id()
        );
    }
}

/// And nothing else. A menu row for a screen the game deals — an open Vault
/// Pick, a gamble in flight — would be a button that does nothing.
#[test]
fn the_menu_offers_nothing_the_game_deals() {
    for screen in Screen::in_menu() {
        assert!(
            screen.reachable_by_flag(),
            "{} is dealt, not opened, and cannot be a menu row",
            screen.id()
        );
        assert_ne!(screen, Screen::Menu, "the menu offers itself");
    }
}

/// Every row needs words on it, and no two rows may read the same.
#[test]
fn every_row_is_named_and_distinct() {
    let mut seen: Vec<&str> = Vec::new();
    for screen in Screen::ALL {
        let label = screen.label();
        assert!(!label.is_empty(), "{} has no label", screen.id());
        assert!(!seen.contains(&label), "{:?} is used twice", label);
        seen.push(label);
    }
}

/// The panel grows with the list, so a twenty-first screen does not spill
/// off the bottom of the frame.
#[test]
fn the_panel_holds_every_row_it_offers() {
    let rows = Screen::in_menu().count().div_ceil(COLUMNS);
    let height = 116.0 + rows as f32 * 52.0;
    assert!(
        frame::BELOW_HEADER + 20.0 + height <= frame::HEIGHT,
        "{} rows need {}px and the frame is {}",
        rows,
        height,
        frame::HEIGHT
    );
}
