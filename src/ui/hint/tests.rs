use crate::game::screens::Screen;
use crate::state::hints::{self, HintDef};

fn shipped() -> Vec<HintDef> {
    macroquad_toolkit::data_loader::parse_json_labeled(
        "assets/data/hints.json",
        macroquad_toolkit::include_json_str!("../../../assets/data/hints.json"),
    )
    .unwrap()
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
