use super::*;

/// A pointer nowhere near anything, so a test measures focus rather than
/// hit-testing.
fn away() -> Pointer {
    Pointer {
        position: Vec2::new(-100.0, -100.0),
        ..Pointer::default()
    }
}

/// Step the nav without touching the keyboard, which a test has no access
/// to — `begin` reads real input, so these drive the state directly.
fn frame(nav: &mut Nav, controls: usize, step: i32, activate: bool) -> Vec<Hit> {
    *nav = Nav {
        seen: 0,
        step,
        activate,
        engaged: true,
        ..nav.clone()
    };
    // What `begin` does with the keys, minus the reading of them.
    if step != 0 && nav.previous > 0 {
        let count = nav.previous as i32;
        nav.index = (nav.index as i32 + step).rem_euclid(count) as usize;
    }

    let hits = (0..controls)
        .map(|i| nav.control(Rect::new(i as f32 * 10.0, 0.0, 5.0, 5.0), true, away()))
        .collect();
    nav.finish();
    hits
}

#[test]
fn focus_starts_on_the_first_control() {
    let mut nav = Nav::default();
    let hits = frame(&mut nav, 4, 0, false);
    assert!(hits[0].focused);
    assert!(hits[1..].iter().all(|hit| !hit.focused));
}

#[test]
fn exactly_one_control_is_focused() {
    let mut nav = Nav::default();
    for step in 0..12 {
        let hits = frame(&mut nav, 5, if step == 0 { 0 } else { 1 }, false);
        assert_eq!(hits.iter().filter(|hit| hit.focused).count(), 1);
    }
}

#[test]
fn focus_wraps_in_both_directions() {
    let mut nav = Nav::default();
    frame(&mut nav, 3, 0, false);
    // Forward past the end.
    for _ in 0..3 {
        frame(&mut nav, 3, 1, false);
    }
    assert!(frame(&mut nav, 3, 0, false)[0].focused);
    // Backward past the start.
    let hits = frame(&mut nav, 3, -1, false);
    assert!(hits[2].focused);
}

#[test]
fn activating_fires_the_focused_control_and_nothing_else() {
    let mut nav = Nav::default();
    frame(&mut nav, 4, 0, false);
    frame(&mut nav, 4, 1, false);

    let hits = frame(&mut nav, 4, 0, true);
    assert!(hits[1].activated);
    assert_eq!(hits.iter().filter(|hit| hit.activated).count(), 1);
}

#[test]
fn a_disabled_control_is_skipped_entirely() {
    // Stepping onto a greyed-out button and pressing Enter to no effect
    // reads as a broken key rather than a disabled control.
    let mut nav = Nav {
        engaged: true,
        activate: true,
        ..Nav::default()
    };

    let away = away();
    let disabled = nav.control(Rect::new(0.0, 0.0, 5.0, 5.0), false, away);
    let enabled = nav.control(Rect::new(10.0, 0.0, 5.0, 5.0), true, away);
    nav.finish();

    assert!(!disabled.focused && !disabled.activated);
    assert!(enabled.focused && enabled.activated);
}

#[test]
fn focus_resets_when_a_panel_opens_or_closes() {
    // The control order is only stable while the same panels are open.
    // Keeping an index across a change would move focus somewhere arbitrary.
    let mut nav = Nav::default();
    frame(&mut nav, 6, 0, false);
    for _ in 0..4 {
        frame(&mut nav, 6, 1, false);
    }

    // A panel opens. The count is only known once everything has drawn, so
    // the reset lands on the frame after — which is fine, because nobody has
    // pressed anything in between.
    frame(&mut nav, 9, 0, false);
    assert!(frame(&mut nav, 9, 0, false)[0].focused);
}

#[test]
fn a_frame_with_no_controls_leaves_focus_somewhere_valid() {
    let mut nav = Nav::default();
    frame(&mut nav, 5, 1, false);
    frame(&mut nav, 0, 0, false);
    // And the next frame with controls starts from the top rather than an
    // index left over from before.
    assert!(frame(&mut nav, 3, 0, false)[0].focused);
}
