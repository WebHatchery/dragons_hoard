use super::*;
use macroquad_toolkit::ui::{contrast_ratio, Level};

/// Every pairing the interface actually draws: a text colour on a surface.
fn pairings(theme: &Theme) -> Vec<(&'static str, Color, Color, f32)> {
    let surfaces = [
        ("panel", theme.stone),
        ("header", theme.stone_header),
        ("background", theme.background),
    ];
    let inks = [
        ("text_bright", theme.text_bright, 15.0),
        ("text", theme.text, 15.0),
        ("text_dim", theme.text_dim, 14.0),
        ("gold", theme.gold, 19.0),
        ("gold_bright", theme.gold_bright, 21.0),
        ("ember", theme.ember, 21.0),
        ("jade", theme.jade, 21.0),
    ];
    let mut out = Vec::new();
    for (_, surface) in surfaces {
        for (name, ink, size) in inks {
            out.push((name, ink, surface, size));
        }
    }
    out
}

/// The gate that makes retheming safe (§5.40).
///
/// Changing colours across a whole interface by hand has invisible failures:
/// a label legible on stone is not legible on ice, and nothing says so.
#[test]
fn every_theme_is_readable() {
    for (id, theme) in THEMES {
        for (ink, fg, bg, size) in pairings(&theme) {
            let ratio = contrast_ratio(fg, bg);
            let required = Level::for_size(size).ratio();
            assert!(
                ratio >= required,
                "{}: {} reads {:.2}:1 against {:.1} needed",
                id,
                ink,
                ratio,
                required
            );
        }
    }
}

#[test]
fn no_theme_is_the_same_as_another() {
    // Two names for one palette is two cabinets that look alike, which is
    // what this section exists to end.
    for (index, (a_id, a)) in THEMES.iter().enumerate() {
        for (b_id, b) in THEMES.iter().skip(index + 1) {
            let apart = (a.stone.r - b.stone.r).abs()
                + (a.stone.g - b.stone.g).abs()
                + (a.stone.b - b.stone.b).abs()
                + (a.gold.r - b.gold.r).abs()
                + (a.gold.g - b.gold.g).abs()
                + (a.gold.b - b.gold.b).abs();
            assert!(apart > 0.05, "{} and {} are the same theme", a_id, b_id);
        }
    }
}

#[test]
fn every_accent_leaves_the_hue_of_its_theme() {
    // An ice cabinet whose "wrong way" colour is also blue has no wrong-way
    // colour. The ember accent has to be visible *as an accent*, which means
    // it cannot sit on the same axis as the surround (§5.24).
    for (id, theme) in THEMES {
        let warm = theme.ember.r - theme.ember.b;
        assert!(warm > 0.3, "{}: the ember accent is not warm", id);
        let cool = theme.jade.g - theme.jade.r;
        assert!(cool > 0.2, "{}: the jade accent is not cool", id);
    }
}

#[test]
fn a_theme_nobody_declared_falls_back_rather_than_failing() {
    // A cabinet with an odd palette is playable; one that refuses to load
    // is not.
    let fallback = by_name("no such theme");
    assert!((fallback.stone.r - HOARD.stone.r).abs() < 1e-6);
}

#[test]
fn every_named_theme_resolves_to_itself() {
    for (id, theme) in THEMES {
        let found = by_name(id);
        assert!((found.gold.r - theme.gold.r).abs() < 1e-6, "{}", id);
    }
}

#[test]
fn the_current_theme_can_be_set_and_read() {
    set(FROST);
    assert!((current().stone.b - FROST.stone.b).abs() < 1e-6);
    set(HOARD);
    assert!((current().stone.b - HOARD.stone.b).abs() < 1e-6);
}
