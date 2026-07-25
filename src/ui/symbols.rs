//! Procedural symbol art.
//!
//! The reels draw real shapes — coins, cut gems, a chest, an egg, a dragon head,
//! a flame — built from macroquad primitives rather than sampled from PNGs.
//!
//! That is a deliberate choice, not a stand-in for art that never arrived. It
//! keeps the game to a single binary with no image assets to load, ship or
//! version; it scales to any cell size without a mipmap; every symbol is tinted
//! from the one `color` already in `symbols.json`, so re-theming is a data edit;
//! and the win highlight can brighten the art itself instead of washing a sprite.
//!
//! Which shape a symbol uses is data too: `symbols.json` carries an `art` key.
//! An unrecognised value falls back to the three-letter code, so adding a symbol
//! can never render nothing.

mod hoard;
mod legible;
mod peaks;
mod tidepool;

use crate::data::SymbolDef;
use macroquad::prelude::*;
use macroquad_toolkit::paint::{Painter, ScreenPainter};

/// The shapes the renderer knows how to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolArt {
    Coin,
    CoinStack,
    Gem,
    GemRound,
    GemStep,
    Chest,
    Egg,
    Dragon,
    Flame,
    Anvil,
    // The mountain cabinets (§5.42).
    Snowflake,
    Icicle,
    Spire,
    Feather,
    Boulder,
    Pine,
    // The tidepool set (§5.36).
    Shell,
    Pearl,
    Starfish,
    Urchin,
    Anemone,
    Coral,
    Crab,
    Kraken,
    Wave,
}

impl SymbolArt {
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "coin" => SymbolArt::Coin,
            "coin_stack" => SymbolArt::CoinStack,
            "gem" => SymbolArt::Gem,
            "gem_round" => SymbolArt::GemRound,
            "gem_step" => SymbolArt::GemStep,
            "chest" => SymbolArt::Chest,
            "egg" => SymbolArt::Egg,
            "dragon" => SymbolArt::Dragon,
            "flame" => SymbolArt::Flame,
            "anvil" => SymbolArt::Anvil,
            "snowflake" => SymbolArt::Snowflake,
            "icicle" => SymbolArt::Icicle,
            "spire" => SymbolArt::Spire,
            "feather" => SymbolArt::Feather,
            "boulder" => SymbolArt::Boulder,
            "pine" => SymbolArt::Pine,
            "shell" => SymbolArt::Shell,
            "pearl" => SymbolArt::Pearl,
            "starfish" => SymbolArt::Starfish,
            "urchin" => SymbolArt::Urchin,
            "anemone" => SymbolArt::Anemone,
            "coral" => SymbolArt::Coral,
            "crab" => SymbolArt::Crab,
            "kraken" => SymbolArt::Kraken,
            "wave" => SymbolArt::Wave,
            _ => return None,
        })
    }
}

/// Shades derived from the symbol's single configured colour, so one hex in the
/// JSON drives the whole piece of art.
#[derive(Debug, Clone, Copy)]
pub(super) struct Shades {
    base: Color,
    dark: Color,
    darker: Color,
    light: Color,
    lighter: Color,
}

impl Shades {
    fn new(color: [f32; 3], lit: f32, alpha: f32) -> Self {
        // A winning cell lifts every shade rather than overlaying a tint, so the
        // art reads brighter without losing its own colour.
        let boost = 1.0 + 0.35 * lit;
        let base = Color::new(
            (color[0] * boost).min(1.0),
            (color[1] * boost).min(1.0),
            (color[2] * boost).min(1.0),
            alpha.clamp(0.0, 1.0),
        );
        Self {
            base,
            dark: scale(base, 0.72),
            darker: scale(base, 0.5),
            light: mix(base, WHITE, 0.28),
            lighter: mix(base, WHITE, 0.6),
        }
    }
}

pub(super) fn scale(color: Color, factor: f32) -> Color {
    Color::new(
        color.r * factor,
        color.g * factor,
        color.b * factor,
        color.a,
    )
}

/// Mixes colour only — `a`'s alpha is preserved, so tinting toward white does
/// not quietly make a motion-blur pass opaque again.
pub(super) fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a,
    )
}

/// Maps normalised 0..1 art coordinates onto a cell, and hands the result to a
/// [`Painter`] (§5.25).
///
/// Generic over the painter so the same art routines draw to the screen in the
/// game and into a pixel buffer in a test. Nothing below this line knows which.
pub(super) struct Canvas<'a, P: Painter> {
    rect: Rect,
    unit: f32,
    painter: &'a mut P,
}

impl<'a, P: Painter> Canvas<'a, P> {
    fn new(rect: Rect, painter: &'a mut P) -> Self {
        let unit = rect.w.min(rect.h);
        Self {
            rect,
            unit,
            painter,
        }
    }

    fn x(&self, x: f32) -> f32 {
        self.rect.x + self.rect.w * x
    }

    fn y(&self, y: f32) -> f32 {
        self.rect.y + self.rect.h * y
    }

    fn p(&self, x: f32, y: f32) -> Vec2 {
        vec2(self.x(x), self.y(y))
    }

    /// Normalised length → pixels, using the smaller axis so art stays round.
    fn s(&self, size: f32) -> f32 {
        self.unit * size
    }

    fn tri(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), color: Color) {
        let (a, b, c) = (self.p(a.0, a.1), self.p(b.0, b.1), self.p(c.0, c.1));
        self.painter.tri(a, b, c, color);
    }

    /// A quad as two triangles, for facets and slabs.
    fn quad(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32), color: Color) {
        self.tri(a, b, c, color);
        self.tri(a, c, d, color);
    }

    fn circle(&mut self, x: f32, y: f32, radius: f32, color: Color) {
        let (center, radius) = (self.p(x, y), self.s(radius));
        self.painter.circle(center, radius, color);
    }

    /// `rx`/`ry` are radii, matching macroquad's own `draw_ellipse`.
    fn ellipse(&mut self, x: f32, y: f32, rx: f32, ry: f32, color: Color) {
        let (center, rx, ry) = (self.p(x, y), self.s(rx), self.s(ry));
        self.painter.ellipse(center, rx, ry, color);
    }

    /// A regular polygon, as a fan of triangles.
    ///
    /// The one primitive that used to reach past the canvas to macroquad
    /// directly, which meant the art could not be drawn without a GL context —
    /// and so could not be measured at all (§5.25).
    fn poly(&mut self, x: f32, y: f32, sides: usize, radius: f32, rotation: f32, color: Color) {
        let step = std::f32::consts::TAU / sides.max(3) as f32;
        let offset = rotation.to_radians();
        let point = |index: usize| {
            let angle = offset + step * index as f32;
            (x + angle.cos() * radius, y + angle.sin() * radius)
        };
        for index in 0..sides.max(3) {
            self.tri((x, y), point(index), point(index + 1), color);
        }
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let at = self.p(x, y);
        let size = vec2(self.rect.w * w, self.rect.h * h);
        self.painter.rect(at, size, color);
    }
}

/// Draw `def`'s art into `rect`. `lit` is 0.0 for a resting cell up to 1.0 at
/// the peak of a win pulse. Returns false when the symbol has no art routine,
/// so the caller can fall back to its short code.
pub fn draw(def: &SymbolDef, rect: Rect, lit: f32) -> bool {
    draw_with_alpha(def, rect, lit, 1.0)
}

/// As [`draw`], but translucent — one pass of a motion-blurred reel.
pub fn draw_with_alpha(def: &SymbolDef, rect: Rect, lit: f32, alpha: f32) -> bool {
    paint(def, rect, lit, alpha, &mut ScreenPainter)
}

/// As [`draw_with_alpha`], but into any [`Painter`] — the screen in the game, a
/// pixel buffer in a test (§5.25).
pub fn paint<P: Painter>(
    def: &SymbolDef,
    rect: Rect,
    lit: f32,
    alpha: f32,
    painter: &mut P,
) -> bool {
    let Some(art) = SymbolArt::from_id(&def.art) else {
        return false;
    };

    let mut canvas = Canvas::new(rect, painter);
    let shades = Shades::new(def.color, lit, alpha);

    match art {
        SymbolArt::Coin => hoard::coin(&mut canvas, &shades, alpha, 0.5, 0.5, 0.30),
        SymbolArt::CoinStack => hoard::coin_stack(&mut canvas, &shades, alpha),
        SymbolArt::Gem => hoard::gem(&mut canvas, &shades, alpha),
        SymbolArt::GemRound => hoard::gem_round(&mut canvas, &shades, alpha),
        SymbolArt::GemStep => hoard::gem_step(&mut canvas, &shades, alpha),
        SymbolArt::Chest => hoard::chest(&mut canvas, &shades, alpha),
        SymbolArt::Egg => hoard::egg(&mut canvas, &shades, alpha),
        SymbolArt::Dragon => hoard::dragon(&mut canvas, &shades, alpha),
        SymbolArt::Flame => hoard::flame(&mut canvas, &shades, alpha),
        SymbolArt::Anvil => hoard::anvil(&mut canvas, &shades, alpha),
        SymbolArt::Snowflake => peaks::snowflake(&mut canvas, &shades, alpha),
        SymbolArt::Icicle => peaks::icicle(&mut canvas, &shades, alpha),
        SymbolArt::Spire => peaks::spire(&mut canvas, &shades, alpha),
        SymbolArt::Feather => peaks::feather(&mut canvas, &shades, alpha),
        SymbolArt::Boulder => peaks::boulder(&mut canvas, &shades, alpha),
        SymbolArt::Pine => peaks::pine(&mut canvas, &shades, alpha),
        SymbolArt::Shell => tidepool::shell(&mut canvas, &shades, alpha),
        SymbolArt::Pearl => tidepool::pearl(&mut canvas, &shades, alpha),
        SymbolArt::Starfish => tidepool::starfish(&mut canvas, &shades, alpha),
        SymbolArt::Urchin => tidepool::urchin(&mut canvas, &shades, alpha),
        SymbolArt::Anemone => tidepool::anemone(&mut canvas, &shades, alpha),
        SymbolArt::Coral => tidepool::coral(&mut canvas, &shades, alpha),
        SymbolArt::Crab => tidepool::crab(&mut canvas, &shades, alpha),
        SymbolArt::Kraken => tidepool::kraken(&mut canvas, &shades, alpha),
        SymbolArt::Wave => tidepool::wave(&mut canvas, &shades, alpha),
    }

    true
}

#[cfg(test)]
mod tests {
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
}
