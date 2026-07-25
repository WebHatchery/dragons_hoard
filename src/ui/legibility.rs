//! Can the symbols be told apart without colour vision? (§5.24)
//!
//! Roughly one man in twelve has some form of red-green colour blindness. This
//! game's symbol art leans on `symbols.json`'s single `color` key, which was a
//! deliberate choice (§7.1) — one field re-themes a whole cabinet — and a
//! deliberate choice with a blind spot: **three symbols on every machine use the
//! same hexagonal gem and differ only in hue.** To a deuteranope, jade and ruby
//! are the same picture.
//!
//! The fix is not a colourblind *mode*. A mode is something a player has to know
//! to look for, and it splits the art into a version that is tested and a
//! version that is not. The fix is that shape carries the difference and colour
//! only reinforces it, which is better for everyone and needs no setting.
//!
//! What is here is the instrument: simulate the three dichromacies, measure how
//! far apart two symbols land, and let a test fail when a pair is too close.

/// The dichromacies worth checking. Anomalous trichromacy (the far more common
/// mild form) sits between these and normal vision, so a set that survives the
/// severe cases survives the mild ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vision {
    Normal,
    /// Red-blind. About 1% of men.
    Protanopia,
    /// Green-blind. About 1% of men, and the most common severe form.
    Deuteranopia,
    /// Blue-blind. Rare, and affects men and women equally.
    Tritanopia,
}

impl Vision {
    pub const ALL: [Vision; 4] = [
        Vision::Normal,
        Vision::Protanopia,
        Vision::Deuteranopia,
        Vision::Tritanopia,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Vision::Normal => "normal",
            Vision::Protanopia => "protanopia",
            Vision::Deuteranopia => "deuteranopia",
            Vision::Tritanopia => "tritanopia",
        }
    }

    /// Row-major 3x3, applied in linear RGB (Viénot/Brettel).
    fn matrix(self) -> [[f32; 3]; 3] {
        match self {
            Vision::Normal => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Vision::Protanopia => [
                [0.152_286, 1.052_583, -0.204_868],
                [0.114_503, 0.786_281, 0.099_216],
                [-0.003_882, -0.048_116, 1.051_998],
            ],
            Vision::Deuteranopia => [
                [0.367_322, 0.860_646, -0.227_968],
                [0.280_085, 0.672_501, 0.047_413],
                [-0.011_820, 0.042_940, 0.968_881],
            ],
            Vision::Tritanopia => [
                [1.255_528, -0.076_749, -0.178_779],
                [-0.078_411, 0.930_809, 0.147_602],
                [0.004_733, 0.691_367, 0.303_900],
            ],
        }
    }
}

fn to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(channel: f32) -> f32 {
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

/// How a colour looks to the given vision, in sRGB.
///
/// The simulation runs in **linear** RGB. Applying the matrix straight to sRGB
/// values is the common shortcut and it exaggerates the effect, which would make
/// this instrument fail colours that are actually fine — a test that cries wolf
/// gets turned off.
pub fn simulate(colour: [f32; 3], vision: Vision) -> [f32; 3] {
    if vision == Vision::Normal {
        return colour;
    }
    let linear = [
        to_linear(colour[0]),
        to_linear(colour[1]),
        to_linear(colour[2]),
    ];
    let matrix = vision.matrix();
    let mut out = [0.0f32; 3];
    for (row, weights) in matrix.iter().enumerate() {
        let value: f32 = weights
            .iter()
            .zip(linear.iter())
            .map(|(weight, channel)| weight * channel)
            .sum();
        out[row] = to_srgb(value.clamp(0.0, 1.0));
    }
    out
}

/// Perceptual-ish distance between two colours, 0 for identical.
///
/// Weighted Euclidean with the "redmean" correction — not a full Lab conversion,
/// but far closer to how a difference actually reads than plain RGB distance,
/// and enough to tell "two gems" from "the same gem twice".
#[cfg(test)]
pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let red_mean = (a[0] + b[0]) * 0.5;
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    ((2.0 + red_mean) * dr * dr + 4.0 * dg * dg + (3.0 - red_mean) * db * db).sqrt()
}

/// Closest any two of these colours come under the given vision.
#[cfg(test)]
pub fn closest_pair(colours: &[[f32; 3]], vision: Vision) -> (usize, usize, f32) {
    let mut worst = (0, 0, f32::INFINITY);
    for (i, first) in colours.iter().enumerate() {
        for (j, second) in colours.iter().enumerate().skip(i + 1) {
            let gap = distance(simulate(*first, vision), simulate(*second, vision));
            if gap < worst.2 {
                worst = (i, j, gap);
            }
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{GameData, MACHINES};

    /// Two symbols sharing a shape must be this far apart in colour under every
    /// vision. Set from measurement rather than theory: the gem trio that
    /// prompted this sat at 0.09 under deuteranopia, and a pair that reads as
    /// clearly different lands well above 0.3.
    const MIN_GAP: f32 = 0.30;

    #[test]
    fn the_simulation_leaves_greys_alone() {
        // A dichromat sees greys exactly as everyone else does. If the matrices
        // tinted them, every measurement below would be skewed.
        for level in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            for vision in Vision::ALL {
                let out = simulate([level, level, level], vision);
                for channel in out {
                    assert!(
                        (channel - level).abs() < 0.04,
                        "{} shifted grey {} to {}",
                        vision.label(),
                        level,
                        channel
                    );
                }
            }
        }
    }

    #[test]
    fn red_and_green_collapse_for_a_deuteranope() {
        // Sanity check on the instrument itself: the one thing everybody knows
        // about red-green colour blindness had better show up.
        let normal = distance([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let deutan = distance(
            simulate([1.0, 0.0, 0.0], Vision::Deuteranopia),
            simulate([0.0, 1.0, 0.0], Vision::Deuteranopia),
        );
        assert!(deutan < normal * 0.5, "{} vs {}", deutan, normal);
    }

    /// Symbols grouped by the art they are drawn with.
    type ByShape<'a> = Vec<(&'a str, Vec<(&'a str, [f32; 3])>)>;

    #[test]
    fn every_symbol_sharing_a_shape_is_distinguishable_by_colour() {
        // The check this module exists for. Symbols drawn with *different* art
        // are told apart by shape and need no colour gap at all; symbols sharing
        // one have nothing else to go on.
        let mut failures: Vec<String> = Vec::new();

        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let mut shapes: ByShape = Vec::new();

            for (_, def) in data.symbols.iter() {
                let entry = shapes.iter_mut().find(|(art, _)| *art == def.art.as_str());
                match entry {
                    Some((_, group)) => group.push((def.id.as_str(), def.color)),
                    None => shapes.push((def.art.as_str(), vec![(def.id.as_str(), def.color)])),
                }
            }

            for (art, group) in shapes.iter().filter(|(_, group)| group.len() > 1) {
                let colours: Vec<[f32; 3]> = group.iter().map(|(_, colour)| *colour).collect();
                for vision in Vision::ALL {
                    let (a, b, gap) = closest_pair(&colours, vision);
                    if gap < MIN_GAP {
                        failures.push(format!(
                            "{}: '{}' and '{}' both drawn as '{}' are {:.3} apart under {}",
                            machine.id,
                            group[a].0,
                            group[b].0,
                            art,
                            gap,
                            vision.label()
                        ));
                    }
                }
            }
        }

        assert!(
            failures.is_empty(),
            "symbols that share a shape and a colour cannot be told apart:\n  {}",
            failures.join("\n  ")
        );
    }
}
