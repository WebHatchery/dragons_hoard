use super::*;

fn shipped() -> Vec<HintDef> {
    macroquad_toolkit::data_loader::parse_json_labeled(
        "assets/data/hints.json",
        macroquad_toolkit::include_json_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/data/hints.json"
        )),
    )
    .unwrap()
}

/// The hints that shipped, held to both rules at once.
#[test]
fn the_shipped_hints_point_somewhere_and_quote_nothing_stale() {
    validate(&shipped()).expect("the shipped hints do not satisfy their own rules");
}

/// The fault this exists for, reproduced: "There are five" outlived
/// Tidepool arriving and became wrong (§5.35, §5.73).
#[test]
fn a_hint_that_spells_out_a_count_is_refused() {
    let mut defs = shipped();
    defs[0].text = "Press C to change cabinet. There are five of them.".to_owned();
    let refused = validate(&defs).expect_err("a spelled-out count was accepted");
    assert!(refused.contains("five"), "{}", refused);
}

/// And the other half: a hint may not point at a screen a player cannot
/// open, which on a touch device was every screen it pointed at.
#[test]
fn a_hint_pointing_somewhere_unreachable_is_refused() {
    let mut defs = shipped();
    defs[0].screen = Some("wrath".to_owned());
    let refused = validate(&defs).expect_err("a dealt screen was accepted as a destination");
    assert!(refused.contains("wrath"), "{}", refused);

    defs[0].screen = Some("nonesuch".to_owned());
    assert!(validate(&defs).is_err(), "an unknown screen was accepted");
}

/// The count comes from the catalog, so a seventh cabinet cannot leave a
/// sentence behind.
#[test]
fn the_cabinet_count_is_read_from_the_catalog() {
    let rendered = render("There are {cabinets} of them.");
    assert_eq!(
        rendered,
        format!("There are {} of them.", spell(crate::data::MACHINES.len()))
    );
    assert!(
        !rendered.chars().any(|c| c.is_ascii_digit()),
        "a hint quoted a numeral where it wanted a word: {}",
        rendered
    );
    assert!(
        !rendered.contains('{'),
        "a placeholder survived: {}",
        rendered
    );
}

/// The cap is a measurement, and the sentence that overran it is the one
/// that produced the number.
#[test]
fn a_hint_too_long_for_the_bar_is_refused() {
    let mut defs = shipped();
    defs[0].text = "x".repeat(106);
    let refused = validate(&defs).expect_err("a hint that wraps was accepted");
    assert!(refused.contains("one line"), "{}", refused);

    defs[0].text = "x".repeat(105);
    validate(&defs).expect("a hint that fits was refused");
}

/// Every hint that names a panel must name one the menu offers, so the
/// advice can be taken without a keyboard (§5.72).
#[test]
fn every_destination_is_in_the_menu() {
    for def in shipped() {
        let Some(id) = def.screen.as_deref() else {
            continue;
        };
        assert!(
            crate::game::screens::Screen::in_menu().any(|screen| screen.id() == id),
            "hint '{}' points at '{}', which the menu does not offer",
            def.id,
            id
        );
    }
}
