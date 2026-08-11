use super::*;

#[test]
fn every_screen_has_a_name_and_no_two_share_one() {
    // The name is what the capture harness asks for, so a duplicate would
    // silently audit one screen twice and the other never.
    let mut seen: Vec<&str> = Vec::new();
    for screen in Screen::ALL {
        assert!(!screen.id().is_empty());
        assert!(!seen.contains(&screen.id()), "{} twice", screen.id());
        seen.push(screen.id());
    }
    assert_eq!(seen.len(), Screen::ALL.len());
}

/// That a screen actually *opened* is checked where it can be: the
/// `audit:<screen>` capture scene asserts it before measuring anything, and
/// `verify.ps1` runs all fifteen. `Game` needs a GL context, so there is no
/// useful unit test here — and the runtime check is the better one anyway,
/// since it exercises the path the audit really takes.
///
/// The three that went unaudited, named as a property rather than a note.
#[test]
fn exactly_the_dealt_screens_have_no_flag() {
    let dealt: Vec<&str> = Screen::ALL
        .iter()
        .filter(|s| !s.reachable_by_flag())
        .map(|s| s.id())
        .collect();
    assert_eq!(
        dealt,
        vec!["bonus", "gamble", "wrath", "seam", "ruin", "sessionover"]
    );
}
