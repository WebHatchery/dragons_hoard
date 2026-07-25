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

mod legible;

use crate::data::SymbolDef;
use crate::ui::paint::{Painter, ScreenPainter};
use macroquad::prelude::*;

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
            _ => return None,
        })
    }
}

/// Shades derived from the symbol's single configured colour, so one hex in the
/// JSON drives the whole piece of art.
#[derive(Debug, Clone, Copy)]
struct Shades {
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

fn scale(color: Color, factor: f32) -> Color {
    Color::new(
        color.r * factor,
        color.g * factor,
        color.b * factor,
        color.a,
    )
}

/// Mixes colour only — `a`'s alpha is preserved, so tinting toward white does
/// not quietly make a motion-blur pass opaque again.
fn mix(a: Color, b: Color, t: f32) -> Color {
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
struct Canvas<'a, P: Painter> {
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
        SymbolArt::Coin => coin(&mut canvas, &shades, alpha, 0.5, 0.5, 0.30),
        SymbolArt::CoinStack => coin_stack(&mut canvas, &shades, alpha),
        SymbolArt::Gem => gem(&mut canvas, &shades, alpha),
        SymbolArt::GemRound => gem_round(&mut canvas, &shades, alpha),
        SymbolArt::GemStep => gem_step(&mut canvas, &shades, alpha),
        SymbolArt::Chest => chest(&mut canvas, &shades, alpha),
        SymbolArt::Egg => egg(&mut canvas, &shades, alpha),
        SymbolArt::Dragon => dragon(&mut canvas, &shades, alpha),
        SymbolArt::Flame => flame(&mut canvas, &shades, alpha),
    }

    true
}

fn coin<P: Painter>(
    canvas: &mut Canvas<P>,
    shades: &Shades,
    alpha: f32,
    cx: f32,
    cy: f32,
    radius: f32,
) {
    canvas.circle(cx, cy, radius, shades.darker);
    canvas.circle(cx, cy, radius * 0.88, shades.base);
    canvas.circle(cx, cy, radius * 0.66, shades.dark);
    // An embossed hexagon reads as a stamped face at reel size.
    canvas.poly(cx, cy, 6, radius * 0.40, 15.0, shades.light);
    canvas.circle(
        cx - radius * 0.34,
        cy - radius * 0.36,
        radius * 0.16,
        Color::new(1.0, 1.0, 1.0, 0.4 * alpha),
    );
}

fn coin_stack<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
    // A shallow pile: two coins at the back, one leaning in front.
    for (cx, cy, radius) in [(0.33, 0.62, 0.21), (0.67, 0.60, 0.21), (0.50, 0.44, 0.23)] {
        canvas.ellipse(cx, cy + 0.09, radius * 1.1, radius * 0.34, shades.darker);
        coin(canvas, shades, alpha, cx, cy, radius);
    }
}

/// Vertex `i` of a six-sided gem outline, points up and down, flats at the
/// sides — the silhouette that reads as "cut stone" at reel size.
/// A vertex of the hexagonal cut.
///
/// Squeezed horizontally on purpose. A regular hexagon at this radius is
/// indistinguishable from a circle once the cell is 64 pixels tall — the
/// silhouette test (§5.25) put it 2.6% away from the copper coin — so the stone
/// is narrow and pointed top and bottom, which reads as *cut* rather than
/// *round* at any size.
fn hex_vertex(index: usize, radius: f32) -> (f32, f32) {
    const NARROW: f32 = 0.66;
    let angle = (90.0 + index as f32 * 60.0).to_radians();
    (
        0.5 + angle.cos() * radius * NARROW,
        0.5 - angle.sin() * radius,
    )
}

fn gem<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
    const R: f32 = 0.36;
    let v: Vec<(f32, f32)> = (0..6).map(|i| hex_vertex(i, R)).collect();
    let center = (0.5, 0.5);

    // Facets, brightest at the top-right where the light falls.
    let facets = [
        shades.lighter, // upper right
        shades.light,   // upper left
        shades.base,    // left
        shades.dark,    // lower left
        shades.darker,  // lower right
        shades.base,    // right
    ];
    for (index, shade) in facets.iter().enumerate() {
        canvas.tri(center, v[index], v[(index + 1) % 6], *shade);
    }

    // Table: the flat top face, catching the most light.
    let table: Vec<(f32, f32)> = (0..6).map(|i| hex_vertex(i, R * 0.42)).collect();
    for index in 0..6 {
        canvas.tri(center, table[index], table[(index + 1) % 6], shades.lighter);
    }

    canvas.circle(0.44, 0.42, 0.032, Color::new(1.0, 1.0, 1.0, 0.8 * alpha));
}

/// A brilliant cut: round, with many narrow facets radiating from the table.
///
/// One of three gem shapes (§5.24). Three stones that differed only in hue were
/// the same picture to a deuteranope; the cut carries the difference now and the
/// colour only reinforces it.
fn gem_round<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
    const R: f32 = 0.36;
    // Wider than it is tall. A round brilliant is a circle, and a circle is the
    // copper coin — the silhouette test (§5.25) put the two 4.1% apart, which is
    // no distance at all. An oval is unmistakably neither.
    const SQUASH: f32 = 0.62;
    const FACETS: usize = 12;
    let center = (0.5, 0.5);
    let rim: Vec<(f32, f32)> = (0..FACETS)
        .map(|i| {
            let angle =
                i as f32 / FACETS as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            (0.5 + angle.cos() * R, 0.5 + angle.sin() * R * SQUASH)
        })
        .collect();

    // Alternating shades so the narrow facets read as facets rather than a disc.
    for index in 0..FACETS {
        let shade = match index {
            0..=2 => shades.lighter,
            3..=5 => shades.base,
            6..=8 => shades.darker,
            _ => shades.light,
        };
        canvas.tri(center, rim[index], rim[(index + 1) % FACETS], shade);
    }

    let table: Vec<(f32, f32)> = (0..FACETS)
        .map(|i| {
            let angle = i as f32 / FACETS as f32 * std::f32::consts::TAU;
            (
                0.5 + angle.cos() * R * 0.40,
                0.5 + angle.sin() * R * 0.40 * SQUASH,
            )
        })
        .collect();
    for index in 0..FACETS {
        canvas.tri(
            center,
            table[index],
            table[(index + 1) % FACETS],
            shades.lighter,
        );
    }

    canvas.circle(0.43, 0.41, 0.030, Color::new(1.0, 1.0, 1.0, 0.85 * alpha));
}

/// An emerald cut: a rectangle with clipped corners and stepped facets.
///
/// Deliberately the least round of the three, so the trio reads as
/// hexagon / circle / rectangle even at a glance and even in monochrome.
fn gem_step<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
    const W: f32 = 0.25;
    const H: f32 = 0.33;
    const CHAMFER: f32 = 0.09;
    let center = (0.5, 0.5);

    // Octagon: a rectangle with its corners cut off.
    let outline = [
        (0.5 - W + CHAMFER, 0.5 - H),
        (0.5 + W - CHAMFER, 0.5 - H),
        (0.5 + W, 0.5 - H + CHAMFER),
        (0.5 + W, 0.5 + H - CHAMFER),
        (0.5 + W - CHAMFER, 0.5 + H),
        (0.5 - W + CHAMFER, 0.5 + H),
        (0.5 - W, 0.5 + H - CHAMFER),
        (0.5 - W, 0.5 - H + CHAMFER),
    ];
    let shades_by_edge = [
        shades.lighter,
        shades.lighter,
        shades.light,
        shades.dark,
        shades.darker,
        shades.darker,
        shades.dark,
        shades.base,
    ];
    for index in 0..outline.len() {
        canvas.tri(
            center,
            outline[index],
            outline[(index + 1) % outline.len()],
            shades_by_edge[index],
        );
    }

    // The stepped table, drawn as two nested rectangles.
    for (inset, shade) in [(0.62, shades.light), (0.34, shades.lighter)] {
        let w = W * inset;
        let h = H * inset;
        canvas.tri(
            (0.5 - w, 0.5 - h),
            (0.5 + w, 0.5 - h),
            (0.5 + w, 0.5 + h),
            shade,
        );
        canvas.tri(
            (0.5 - w, 0.5 - h),
            (0.5 + w, 0.5 + h),
            (0.5 - w, 0.5 + h),
            shade,
        );
    }

    canvas.circle(0.44, 0.40, 0.026, Color::new(1.0, 1.0, 1.0, 0.8 * alpha));
}

fn chest<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
    let gold = Color::new(0.92, 0.76, 0.32, alpha);

    // Coins spilling over the back edge, drawn first so the lid overlaps them.
    canvas.circle(0.36, 0.30, 0.055, gold);
    canvas.circle(0.52, 0.26, 0.06, gold);
    canvas.circle(0.66, 0.31, 0.05, gold);

    // Lid, then body.
    canvas.rect(0.16, 0.32, 0.68, 0.14, shades.light);
    canvas.rect(0.16, 0.44, 0.68, 0.04, shades.darker);
    canvas.rect(0.16, 0.46, 0.68, 0.30, shades.base);
    canvas.rect(0.16, 0.72, 0.68, 0.06, shades.dark);

    // Iron bands and the lock plate.
    canvas.rect(0.28, 0.32, 0.05, 0.46, shades.darker);
    canvas.rect(0.67, 0.32, 0.05, 0.46, shades.darker);
    canvas.rect(0.45, 0.42, 0.10, 0.16, gold);
    canvas.circle(0.50, 0.53, 0.035, scale(gold, 0.45));
}

fn egg<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
    // Stacked ellipses give a proper egg profile — fat at the base, narrowing
    // smoothly to a rounded crown. A cone tapered to a point read as a teardrop.
    canvas.ellipse(0.50, 0.60, 0.25, 0.27, shades.base);
    canvas.ellipse(0.50, 0.42, 0.215, 0.24, shades.base);
    canvas.ellipse(0.50, 0.29, 0.155, 0.18, shades.base);

    // Shading down the right-hand side.
    canvas.ellipse(0.58, 0.52, 0.155, 0.30, shades.dark);

    for (x, y, r) in [
        (0.42, 0.64, 0.042),
        (0.58, 0.60, 0.032),
        (0.47, 0.46, 0.036),
        (0.40, 0.34, 0.026),
    ] {
        canvas.circle(x, y, r, shades.darker);
    }

    canvas.ellipse(
        0.40,
        0.36,
        0.06,
        0.09,
        Color::new(1.0, 1.0, 1.0, 0.26 * alpha),
    );
}

fn dragon<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
    // Horns behind the skull.
    canvas.tri((0.58, 0.34), (0.80, 0.08), (0.70, 0.36), shades.dark);
    canvas.tri((0.46, 0.32), (0.58, 0.12), (0.56, 0.36), shades.dark);

    // Skull and snout.
    canvas.quad(
        (0.16, 0.44),
        (0.50, 0.28),
        (0.78, 0.34),
        (0.64, 0.60),
        shades.base,
    );
    canvas.tri((0.16, 0.44), (0.64, 0.60), (0.30, 0.68), shades.dark);
    canvas.tri((0.10, 0.52), (0.16, 0.44), (0.30, 0.68), shades.darker);

    // Jaw.
    canvas.tri((0.22, 0.62), (0.56, 0.66), (0.40, 0.82), shades.darker);

    // Eye and nostril.
    canvas.circle(0.56, 0.42, 0.06, Color::new(1.0, 0.86, 0.30, alpha));
    canvas.circle(0.56, 0.42, 0.024, Color::new(0.10, 0.05, 0.08, alpha));
    canvas.circle(0.22, 0.50, 0.022, shades.darker);
}

fn flame<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
    let outer = shades.base;
    let mid = mix(shades.base, Color::new(1.0, 0.85, 0.25, 1.0), 0.6);
    let core = Color::new(1.0, 0.96, 0.80, alpha);

    canvas.circle(0.50, 0.60, 0.28, outer);
    canvas.tri((0.50, 0.08), (0.24, 0.64), (0.76, 0.64), outer);

    canvas.circle(0.50, 0.63, 0.18, mid);
    canvas.tri((0.50, 0.26), (0.34, 0.66), (0.66, 0.66), mid);

    canvas.circle(0.50, 0.66, 0.085, core);
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
        let mut painter = crate::ui::paint::Buffer::new(1, 1);
        let canvas = Canvas::new(Rect::new(100.0, 200.0, 60.0, 90.0), &mut painter);

        assert_eq!(canvas.p(0.0, 0.0), vec2(100.0, 200.0));
        assert_eq!(canvas.p(1.0, 1.0), vec2(160.0, 290.0));
        assert_eq!(canvas.p(0.5, 0.5), vec2(130.0, 245.0));
        // Sizes use the shorter axis so circles do not become ellipses.
        assert_eq!(canvas.s(1.0), 60.0);
    }
}
