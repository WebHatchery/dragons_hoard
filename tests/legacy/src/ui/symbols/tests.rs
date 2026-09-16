use super::*;
use crate::data::GameData;

#[test]
fn every_symbol_declares_art_the_renderer_knows() {
    let data = GameData::load().unwrap();

    for (_, def) in data.symbols.iter() {
        assert!(
            SymbolArt::from_id(&def.art).is_some(),
            "symbol '{}' asks for unknown art '{}'",
            def.id,
            def.art
        );
    }
}

#[test]
fn an_unknown_art_id_is_reported_rather_than_drawn() {
    assert!(SymbolArt::from_id("sphinx").is_none());
}

#[test]
fn shades_stay_inside_the_colour_range_even_when_lit() {
    let shades = Shades::new([0.95, 0.9, 0.85], 1.0, 1.0);

    for color in [
        shades.base,
        shades.dark,
        shades.darker,
        shades.light,
        shades.lighter,
    ] {
        for channel in [color.r, color.g, color.b] {
            assert!(
                (0.0..=1.0).contains(&channel),
                "channel {} escaped the range",
                channel
            );
        }
    }
}

#[test]
fn a_lit_symbol_is_brighter_than_a_resting_one() {
    let resting = Shades::new([0.4, 0.3, 0.2], 0.0, 1.0);
    let lit = Shades::new([0.4, 0.3, 0.2], 1.0, 1.0);

    assert!(lit.base.r > resting.base.r);
    assert!(lit.base.g > resting.base.g);
}

#[test]
fn the_canvas_maps_normalised_coordinates_onto_the_cell() {
    let mut painter = macroquad_toolkit::paint::Buffer::new(1, 1);
    let canvas = Canvas::new(Rect::new(100.0, 200.0, 60.0, 90.0), &mut painter);

    assert_eq!(canvas.p(0.0, 0.0), vec2(100.0, 200.0));
    assert_eq!(canvas.p(1.0, 1.0), vec2(160.0, 290.0));
    assert_eq!(canvas.p(0.5, 0.5), vec2(130.0, 245.0));
    // Sizes use the shorter axis so circles do not become ellipses.
    assert_eq!(canvas.s(1.0), 60.0);
}
