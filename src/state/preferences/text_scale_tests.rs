use super::*;

#[test]
fn the_smallest_offered_size_is_the_design_size() {
    // Every panel was laid out at 1.0, so it has to be the default and the
    // floor. A player who has never touched the setting must see exactly
    // what the layout was drawn for.
    assert_eq!(TEXT_SCALES[0], 1.0);
    assert_eq!(Preferences::default().text_scale(), 1.0);
}

#[test]
fn the_sizes_only_go_up() {
    // Down is the toolkit's business, not a setting: text smaller than the
    // design size fails the legibility floor §5.25 measures art against.
    for pair in TEXT_SCALES.windows(2) {
        assert!(pair[1] > pair[0], "{:?}", TEXT_SCALES);
    }
    assert!(TEXT_SCALES.iter().all(|scale| *scale >= 1.0));
}

#[test]
fn cycling_reaches_every_size_and_comes_back() {
    let mut prefs = Preferences::default();
    let mut seen = Vec::new();
    for _ in 0..TEXT_SCALES.len() {
        seen.push(prefs.text_scale());
        prefs.cycle_text_scale();
    }
    for scale in TEXT_SCALES {
        assert!(seen.contains(&scale), "{} unreachable", scale);
    }
    assert_eq!(prefs.text_scale(), TEXT_SCALES[0]);
}

#[test]
fn a_saved_index_past_the_end_falls_back_rather_than_panicking() {
    // The list can shrink under a saved preference; an out-of-range index
    // must not take the game down on the frame it loads.
    let prefs = Preferences {
        text_scale: 999,
        ..Preferences::default()
    };
    assert_eq!(prefs.text_scale(), TEXT_SCALES[0]);
}

#[test]
fn no_offered_size_is_larger_than_the_panels_were_measured_at() {
    // The layout audit (§5.37) is run at every value in this list before it
    // ships. Widening the list without re-running it is the mistake this
    // note exists to make loud.
    assert!(TEXT_SCALES.iter().all(|scale| *scale <= 1.3));
}
