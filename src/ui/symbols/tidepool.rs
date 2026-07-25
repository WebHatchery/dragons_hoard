//! The tidepool set (§5.36).
//!
//! Tidepool shipped in §5.35 with a genuinely new win model and Dragon's Hoard's
//! symbols — a machine named for a rock pool showing treasure chests and a
//! dragon. Six cabinets drawing the same nine shapes was the weakest thing about
//! the game to look at, and the one that had least to do with what any of them
//! actually did.
//!
//! # Drawn against the same two gates
//!
//! Nothing here is free-form. Every set is held to the rules §5.24 and §5.25
//! established, and both already ran over every cabinet, so the moment this file
//! existed it was being checked:
//!
//! - **No two symbols may look alike** at the smallest cell the game draws,
//!   measured by monochrome difference rather than by eye.
//! - **Any two sharing a shape must separate by colour** under three simulated
//!   dichromacies.
//!
//! The second is the one that shapes the set. A tidepool wants to be blue and
//! green, and blue-green is exactly the axis a deuteranope loses — so the
//! silhouettes have to carry the difference, and the palette runs from sand
//! through coral to deep water rather than sitting in one band.
//!
//! # One colour drives each piece
//!
//! Same contract as the hoard set: the JSON gives one hex, [`Shades`] derives
//! the rest, and the art is built from the canvas primitives. Anything that
//! needed a texture would not survive the size the reels actually draw at.

use super::{Canvas, Shades};
use macroquad::prelude::*;
use macroquad_toolkit::paint::Painter;

/// A spiral shell. Overlapping circles of falling radius, walked around a curve.
pub(super) fn shell<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    canvas.circle(0.5, 0.54, 0.34, shades.darker);
    // Five whorls tightening toward the apex. Read as a spiral at reel size
    // without any of the cost of drawing one properly.
    let turns = [
        (0.50, 0.56, 0.30, shades.base),
        (0.44, 0.50, 0.23, shades.dark),
        (0.50, 0.45, 0.17, shades.base),
        (0.55, 0.42, 0.11, shades.light),
        (0.52, 0.39, 0.06, shades.base),
    ];
    for (x, y, r, color) in turns {
        canvas.circle(x, y, r, color);
    }
    // The lip, which is what stops it reading as a stack of coins.
    canvas.tri((0.24, 0.66), (0.50, 0.86), (0.72, 0.62), shades.light);
}

/// A pearl: a plain sphere with one hard highlight.
///
/// Deliberately the simplest thing in the set. Every set needs one symbol that
/// is unmistakable at a glance, and a circle is the only shape that cannot be
/// confused with a silhouette that has corners.
pub(super) fn pearl<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    canvas.circle(0.5, 0.5, 0.32, shades.darker);
    canvas.circle(0.5, 0.5, 0.28, shades.base);
    canvas.circle(0.46, 0.44, 0.18, shades.light);
    canvas.circle(0.42, 0.40, 0.07, WHITE);
}

/// A five-armed starfish.
pub(super) fn starfish<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // Arms as triangles from the centre, so the points stay sharp when the cell
    // is small — a five-pointed polygon rounds off into a blob.
    let arms = 5;
    let step = std::f32::consts::TAU / arms as f32;
    for index in 0..arms {
        let angle = -std::f32::consts::FRAC_PI_2 + step * index as f32;
        let tip = (0.5 + angle.cos() * 0.40, 0.5 + angle.sin() * 0.40);
        let left = (
            0.5 + (angle - 0.55).cos() * 0.16,
            0.5 + (angle - 0.55).sin() * 0.16,
        );
        let right = (
            0.5 + (angle + 0.55).cos() * 0.16,
            0.5 + (angle + 0.55).sin() * 0.16,
        );
        canvas.tri(left, tip, right, shades.base);
    }
    canvas.circle(0.5, 0.5, 0.17, shades.dark);
    canvas.circle(0.5, 0.5, 0.08, shades.light);
}

/// A sea urchin: a dark body under a ring of spines.
pub(super) fn urchin<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    let spines = 12;
    let step = std::f32::consts::TAU / spines as f32;
    for index in 0..spines {
        let angle = step * index as f32;
        let tip = (0.5 + angle.cos() * 0.44, 0.5 + angle.sin() * 0.44);
        let left = (
            0.5 + (angle - 0.20).cos() * 0.20,
            0.5 + (angle - 0.20).sin() * 0.20,
        );
        let right = (
            0.5 + (angle + 0.20).cos() * 0.20,
            0.5 + (angle + 0.20).sin() * 0.20,
        );
        canvas.tri(left, tip, right, shades.dark);
    }
    canvas.circle(0.5, 0.5, 0.22, shades.base);
    canvas.circle(0.46, 0.45, 0.09, shades.light);
}

/// An anemone: a squat column under a crown of tentacles.
pub(super) fn anemone<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // Column first, so the tentacles sit over it.
    canvas.quad(
        (0.36, 0.92),
        (0.64, 0.92),
        (0.60, 0.56),
        (0.40, 0.56),
        shades.dark,
    );
    let tips = [
        (0.20, 0.34),
        (0.32, 0.20),
        (0.50, 0.14),
        (0.68, 0.20),
        (0.80, 0.34),
    ];
    for (x, y) in tips {
        canvas.tri((0.44, 0.62), (x, y), (0.56, 0.62), shades.base);
    }
    canvas.ellipse(0.5, 0.60, 0.22, 0.10, shades.light);
}

/// Branching coral. The hoard symbol on this cabinet.
pub(super) fn coral<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // A trunk that forks twice. Rectangles rather than lines, because a line
    // one pixel wide disappears at the size the reels draw.
    canvas.quad(
        (0.42, 0.92),
        (0.58, 0.92),
        (0.56, 0.58),
        (0.44, 0.58),
        shades.dark,
    );
    let branches = [
        ((0.44, 0.66), (0.20, 0.40), 0.07),
        ((0.56, 0.66), (0.80, 0.40), 0.07),
        ((0.50, 0.58), (0.50, 0.16), 0.07),
        ((0.46, 0.44), (0.30, 0.22), 0.05),
        ((0.54, 0.44), (0.70, 0.22), 0.05),
    ];
    for ((x0, y0), (x1, y1), width) in branches {
        canvas.quad(
            (x0 - width, y0),
            (x0 + width, y0),
            (x1 + width * 0.6, y1),
            (x1 - width * 0.6, y1),
            shades.base,
        );
        canvas.circle(x1, y1, width * 1.1, shades.light);
    }
}

/// A crab, seen from above.
pub(super) fn crab<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // Legs behind the shell so they read as underneath it.
    for side in [-1.0f32, 1.0] {
        for (index, y) in [0.50f32, 0.62, 0.74].iter().enumerate() {
            let reach = 0.42 - index as f32 * 0.04;
            canvas.quad(
                (0.5 + side * 0.20, y - 0.03),
                (0.5 + side * reach, y + 0.06),
                (0.5 + side * reach, y + 0.11),
                (0.5 + side * 0.20, y + 0.03),
                shades.dark,
            );
        }
        // Claws, which are what make it a crab rather than a beetle.
        canvas.circle(0.5 + side * 0.38, 0.32, 0.10, shades.base);
        canvas.tri(
            (0.5 + side * 0.30, 0.28),
            (0.5 + side * 0.48, 0.20),
            (0.5 + side * 0.44, 0.34),
            shades.light,
        );
    }
    canvas.ellipse(0.5, 0.56, 0.30, 0.22, shades.darker);
    canvas.ellipse(0.5, 0.55, 0.26, 0.18, shades.base);
    canvas.circle(0.42, 0.48, 0.045, WHITE);
    canvas.circle(0.58, 0.48, 0.045, WHITE);
}

/// The kraken. This cabinet's wild.
///
/// The wild has to be the loudest silhouette on the reels, because it is the one
/// a player looks for. A domed head over splayed arms is nothing else in the
/// set.
pub(super) fn kraken<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    let arms = [
        (0.10, 0.86),
        (0.26, 0.94),
        (0.50, 0.96),
        (0.74, 0.94),
        (0.90, 0.86),
    ];
    for (x, y) in arms {
        canvas.tri((0.42, 0.60), (x, y), (0.58, 0.60), shades.dark);
        canvas.circle(x, y, 0.05, shades.base);
    }
    canvas.ellipse(0.5, 0.46, 0.30, 0.32, shades.darker);
    canvas.ellipse(0.5, 0.44, 0.26, 0.28, shades.base);
    canvas.ellipse(0.44, 0.34, 0.10, 0.09, shades.light);
    // Two eyes, low and wide, which is what stops it reading as an egg.
    canvas.circle(0.40, 0.50, 0.065, WHITE);
    canvas.circle(0.60, 0.50, 0.065, WHITE);
    canvas.circle(0.40, 0.51, 0.030, shades.darker);
    canvas.circle(0.60, 0.51, 0.030, shades.darker);
}

/// A breaking wave. This cabinet's scatter.
///
/// The first attempt was a circle with a darker circle inside it and a triangle
/// on top, which at reel size read unmistakably as a flying saucer. A wave is
/// not a round thing with a dome — it is a **hook**, a crest thrown forward and
/// curling back over the water under it. So it is built as a rising flank, a
/// crest that overhangs, and the lip falling back inside its own curve.
pub(super) fn wave<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // The water it breaks over.
    canvas.quad(
        (0.04, 0.94),
        (0.96, 0.94),
        (0.96, 0.74),
        (0.04, 0.80),
        shades.darker,
    );

    // The flank, rising from the left to the crest.
    canvas.tri((0.10, 0.80), (0.46, 0.20), (0.52, 0.76), shades.dark);
    canvas.tri((0.46, 0.20), (0.72, 0.30), (0.52, 0.76), shades.base);

    // The overhang: the crest thrown forward past its own base.
    canvas.tri((0.46, 0.20), (0.86, 0.34), (0.72, 0.30), shades.base);
    // And the lip falling back under it, which is the whole silhouette.
    canvas.tri((0.86, 0.34), (0.78, 0.54), (0.68, 0.36), shades.light);
    canvas.tri((0.72, 0.30), (0.86, 0.34), (0.68, 0.36), shades.light);

    // Spray thrown off the crest, ahead of the curl.
    canvas.circle(0.90, 0.22, 0.050, shades.light);
    canvas.circle(0.80, 0.14, 0.034, shades.light);
    canvas.circle(0.66, 0.12, 0.024, shades.light);
}
