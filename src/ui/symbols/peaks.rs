//! Shapes for the three mountain cabinets (§5.42).
//!
//! Frost Wyrm, Wyrmspire and Avalanche all live above the snowline and all three
//! shipped drawing Dragon's Hoard's coins and treasure chest. §5.41 made a
//! symbol set a shared, named resource; this is what that was for.
//!
//! # Two new shapes each, not nine
//!
//! A full nine-shape set is what Tidepool needed, because a rock pool has
//! nothing in common with a hoard. These three do: they are all dragons and
//! treasure at altitude, and the low symbols are coins and gems on every one of
//! them. Redrawing a coin three more times would be work with no reader.
//!
//! So each set keeps the shared low-tier silhouettes and gets **its own
//! premiums** — the symbols a player actually looks for, and the ones that carry
//! a theme. What separates the three at a glance is then the palette, which is
//! the honest answer for three cabinets that really are variations on one idea.
//!
//! # The palette does the rest, within limits
//!
//! §5.24's gate holds: two symbols sharing a silhouette must stay apart under
//! three simulated dichromacies. Within a set that is what stops the ice cabinet
//! being six blues, and it is why each of these runs a cold or a warm accent
//! against its base rather than a single hue.

use super::{Canvas, Shades};
use macroquad::prelude::*;
use macroquad_toolkit::paint::Painter;

/// A six-armed snowflake. Frost Wyrm's scatter.
pub(super) fn snowflake<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    let arms = 6;
    let step = std::f32::consts::TAU / arms as f32;
    for index in 0..arms {
        let angle = step * index as f32;
        let (dx, dy) = (angle.cos(), angle.sin());
        // The spine: a thin quad rather than a line, which vanishes at reel
        // size. Half-width is in normalised units, so it scales with the cell.
        let w = 0.045;
        canvas.quad(
            (0.5 - dy * w, 0.5 + dx * w),
            (0.5 + dy * w, 0.5 - dx * w),
            (
                0.5 + dx * 0.44 + dy * w * 0.5,
                0.5 + dy * 0.44 - dx * w * 0.5,
            ),
            (
                0.5 + dx * 0.44 - dy * w * 0.5,
                0.5 + dy * 0.44 + dx * w * 0.5,
            ),
            shades.base,
        );
        // Two barbs, which is what makes it a snowflake rather than an asterisk.
        for (at, reach) in [(0.24f32, 0.13f32), (0.34, 0.10)] {
            let root = (0.5 + dx * at, 0.5 + dy * at);
            for side in [-0.7f32, 0.7] {
                let a = angle + side;
                canvas.quad(
                    (root.0 - dy * 0.03, root.1 + dx * 0.03),
                    (root.0 + dy * 0.03, root.1 - dx * 0.03),
                    (root.0 + a.cos() * reach, root.1 + a.sin() * reach),
                    (root.0 + a.cos() * reach, root.1 + a.sin() * reach),
                    shades.light,
                );
            }
        }
    }
    canvas.circle(0.5, 0.5, 0.09, shades.light);
}

/// A hanging icicle. Frost Wyrm's premium.
pub(super) fn icicle<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // The ledge it hangs from, so it reads as hanging rather than floating.
    canvas.quad(
        (0.10, 0.12),
        (0.90, 0.12),
        (0.90, 0.24),
        (0.10, 0.24),
        shades.darker,
    );
    // Three spikes of falling length. The uneven lengths are the whole read:
    // three equal ones look like a comb.
    for (x, width, tip) in [
        (0.28f32, 0.09f32, 0.62f32),
        (0.52, 0.12, 0.92),
        (0.74, 0.07, 0.50),
    ] {
        canvas.tri((x - width, 0.22), (x + width, 0.22), (x, tip), shades.base);
        canvas.tri(
            (x - width * 0.35, 0.24),
            (x + width * 0.15, 0.24),
            (x, tip * 0.86),
            shades.light,
        );
    }
}

/// A spire of stacked stone. Wyrmspire's premium.
pub(super) fn spire<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // Four tapering courses. Drawn bottom-up so each sits on the one below.
    let courses = [
        (0.86f32, 0.72f32, 0.40f32),
        (0.70, 0.56, 0.32),
        (0.54, 0.40, 0.24),
        (0.38, 0.26, 0.16),
    ];
    for (index, (bottom, top, half)) in courses.iter().enumerate() {
        let shade = if index % 2 == 0 {
            shades.base
        } else {
            shades.dark
        };
        canvas.quad(
            (0.5 - half, *bottom),
            (0.5 + half, *bottom),
            (0.5 + half * 0.86, *top),
            (0.5 - half * 0.86, *top),
            shade,
        );
    }
    // The peak, and a lit face down the left so it is not a flat stack.
    canvas.tri((0.36, 0.26), (0.50, 0.08), (0.64, 0.26), shades.light);
    canvas.tri(
        (0.5 - 0.40, 0.86),
        (0.5 - 0.14, 0.26),
        (0.5 - 0.22, 0.86),
        shades.light,
    );
}

/// A feather, caught on the updraft. Wyrmspire's scatter.
///
/// The first attempt drew the vanes as small triangles along the shaft and
/// covered nine percent of the cell — the legibility gate (§5.25) failed it as
/// too sparse to read, which is exactly right: at reel size it was a scratch.
/// The vanes are quads spanning most of the width now, and the outline is a leaf
/// rather than a bar because they are longest in the middle.
pub(super) fn feather<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // Both vanes as solid bodies first, then the shaft over them.
    let along = |t: f32| (0.24 + (0.78 - 0.24) * t, 0.88 - (0.88 - 0.14) * t);
    let width = |t: f32| 0.30 * (1.0 - ((t - 0.42) * 1.7).abs()).max(0.18);

    for step in 0..9 {
        let (t0, t1) = (step as f32 / 9.0, (step + 1) as f32 / 9.0);
        let (a, b) = (along(t0), along(t1));
        let (wa, wb) = (width(t0), width(t1));
        // Left vane, swept back toward the quill.
        canvas.quad(
            a,
            b,
            (b.0 - wb * 0.80, b.1 - wb * 0.34),
            (a.0 - wa * 0.80, a.1 - wa * 0.34),
            shades.base,
        );
        // Right vane, shorter and lit.
        canvas.quad(
            a,
            b,
            (b.0 + wb * 0.52, b.1 + wb * 0.62),
            (a.0 + wa * 0.52, a.1 + wa * 0.62),
            shades.light,
        );
    }

    canvas.quad(
        (0.20, 0.90),
        (0.28, 0.92),
        (0.80, 0.16),
        (0.72, 0.13),
        shades.dark,
    );
}

/// A boulder mid-fall. Avalanche's premium.
pub(super) fn boulder<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    // An irregular lump: a circle reads as a coin, and this set already has one.
    canvas.tri((0.16, 0.62), (0.34, 0.22), (0.62, 0.26), shades.dark);
    canvas.tri((0.16, 0.62), (0.62, 0.26), (0.78, 0.54), shades.base);
    canvas.tri((0.16, 0.62), (0.78, 0.54), (0.66, 0.82), shades.base);
    canvas.tri((0.16, 0.62), (0.66, 0.82), (0.30, 0.84), shades.darker);
    // A lit facet and a crack, which is what stops it reading as a blob.
    canvas.tri((0.34, 0.22), (0.62, 0.26), (0.46, 0.48), shades.light);
    canvas.quad(
        (0.44, 0.50),
        (0.48, 0.50),
        (0.58, 0.78),
        (0.54, 0.78),
        shades.darker,
    );
}

/// A snow-laden pine. Avalanche's scatter.
pub(super) fn pine<P: Painter>(canvas: &mut Canvas<P>, shades: &Shades, _alpha: f32) {
    canvas.quad(
        (0.44, 0.92),
        (0.56, 0.92),
        (0.54, 0.74),
        (0.46, 0.74),
        shades.darker,
    );
    // Three tiers, widest at the base.
    for (bottom, top, half) in [
        (0.80f32, 0.56f32, 0.34f32),
        (0.62, 0.36, 0.27),
        (0.44, 0.10, 0.19),
    ] {
        canvas.tri(
            (0.5 - half, bottom),
            (0.5 + half, bottom),
            (0.5, top),
            shades.base,
        );
        // Snow sitting on the upper surface of each tier.
        canvas.tri(
            (0.5 - half * 0.55, bottom - (bottom - top) * 0.34),
            (0.5 + half * 0.55, bottom - (bottom - top) * 0.34),
            (0.5, top + (bottom - top) * 0.10),
            shades.light,
        );
    }
}
