//! The hoard set: coins, gems, a chest, an egg, the dragon and its fire.
//!
//! Split out of `symbols.rs` when the tidepool set arrived (§5.36) and the file
//! would have gone past the size limit. The seam is the obvious one — everything
//! here takes a [`Canvas`] and a [`Shades`] and draws, and none of it knows what
//! a symbol or a machine is.

use super::{mix, scale, Canvas, Shades};
use macroquad::prelude::*;
use macroquad_toolkit::paint::Painter;

pub(super) fn coin<P: Painter>(
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

pub(super) fn coin_stack<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
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
pub(super) fn hex_vertex(index: usize, radius: f32) -> (f32, f32) {
    const NARROW: f32 = 0.66;
    let angle = (90.0 + index as f32 * 60.0).to_radians();
    (
        0.5 + angle.cos() * radius * NARROW,
        0.5 - angle.sin() * radius,
    )
}

pub(super) fn gem<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
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
pub(super) fn gem_round<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
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
pub(super) fn gem_step<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
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

pub(super) fn chest<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
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

pub(super) fn egg<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
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

pub(super) fn dragon<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
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

pub(super) fn flame<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, alpha: f32) {
    let outer = shades.base;
    let mid = mix(shades.base, Color::new(1.0, 0.85, 0.25, 1.0), 0.6);
    let core = Color::new(1.0, 0.96, 0.80, alpha);

    canvas.circle(0.50, 0.60, 0.28, outer);
    canvas.tri((0.50, 0.08), (0.24, 0.64), (0.76, 0.64), outer);

    canvas.circle(0.50, 0.63, 0.18, mid);
    canvas.tri((0.50, 0.26), (0.34, 0.66), (0.66, 0.66), mid);

    canvas.circle(0.50, 0.66, 0.085, core);
}

/// An anvil. The forge symbol on the ember set (§5.41).
///
/// Lives with the hoard shapes rather than in a file of its own because it is
/// one routine, and a module per symbol would be filing for its own sake.
pub(super) fn anvil<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // Base, waisted stem, then the body — read bottom-up, which is how an anvil
    // is recognised: the horn is the only part that matters at reel size.
    canvas.quad(
        (0.24, 0.88),
        (0.76, 0.88),
        (0.70, 0.78),
        (0.30, 0.78),
        shades.darker,
    );
    canvas.quad(
        (0.40, 0.78),
        (0.60, 0.78),
        (0.56, 0.52),
        (0.44, 0.52),
        shades.dark,
    );
    canvas.quad(
        (0.20, 0.52),
        (0.78, 0.52),
        (0.78, 0.36),
        (0.20, 0.36),
        shades.base,
    );
    // The horn, tapering off the left.
    canvas.tri((0.20, 0.36), (0.02, 0.42), (0.20, 0.50), shades.base);
    // A struck highlight along the face.
    canvas.quad(
        (0.24, 0.38),
        (0.74, 0.38),
        (0.74, 0.34),
        (0.24, 0.34),
        shades.light,
    );
}
