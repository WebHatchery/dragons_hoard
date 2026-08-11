use super::*;
use crate::data::MACHINES;

fn profile(data: &GameData, shape: usize) -> ShapeProfile {
    let mut profiler = ShapeProfiler::new(data, shape);
    loop {
        if let Some(done) = profiler.step(data) {
            return done;
        }
    }
}

/// The claim §5.64 makes, checked against play rather than arithmetic.
///
/// The shapes are constructed to be worth the same and a test holds the
/// construction. This is the other half: actually running two thousand
/// features each way and seeing the means land together. If they do not,
/// something about the feature is not linear in the multiplier and the
/// whole premise is wrong.
#[test]
#[ignore = "two thousand features per shape; run with --ignored --release"]
fn both_shapes_return_the_same_over_two_thousand_runs() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        if data.freespins.shapes.len() < 2 {
            continue;
        }
        let measured: Vec<ShapeProfile> = (0..data.freespins.shapes.len())
            .map(|shape| profile(&data, shape))
            .collect();

        let long = measured[0].mean;
        for (index, profile) in measured.iter().enumerate() {
            let drift = (profile.mean - long).abs() / long.max(0.0001);
            println!(
                "{:>10} {:<6} mean {:7.2}x  blanks {:5.1}%  best {:8.1}x",
                machine.id,
                data.freespins.shapes[index].id,
                profile.mean,
                profile.blanks * 100.0,
                profile.best
            );
            assert!(
                drift < 0.12,
                "{}: '{}' returns {:.2}x against '{}' at {:.2}x",
                machine.id,
                data.freespins.shapes[index].id,
                profile.mean,
                data.freespins.shapes[0].id,
                long
            );
        }
    }
}
