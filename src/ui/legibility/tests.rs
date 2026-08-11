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
