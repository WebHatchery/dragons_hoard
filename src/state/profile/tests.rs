use super::*;
use crate::data::MACHINES;

/// Run a profile to completion, as the UI would over many frames.
fn profile(data: &GameData) -> MachineProfile {
    let mut profiler = Profiler::new(data);
    for _ in 0..10_000 {
        if let Some(profile) = profiler.step(data) {
            return profile;
        }
    }
    panic!("the profiler never finished");
}

#[test]
fn every_cabinet_profiles_to_something_plausible() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let profile = profile(&data);

        assert_eq!(profile.rounds, PROFILE_ROUNDS);
        assert!(
            (0.5..1.6).contains(&profile.rtp),
            "{} profiled at RTP {:.3}",
            machine.id,
            profile.rtp
        );
        assert!(
            (0.05..0.95).contains(&profile.hit_frequency),
            "{} profiled at hit {:.3}",
            machine.id,
            profile.hit_frequency
        );
        assert!(profile.volatility > 0.0);
        assert!(profile.best_round > 0.0);
    }
}

#[test]
fn the_bands_account_for_every_round() {
    // A band table that did not sum to one would mean rounds falling
    // through the classifier, and the bar chart would quietly under-report.
    let data = GameData::load().unwrap();
    let profile = profile(&data);

    let total: f64 = profile.bands.iter().sum();
    assert!(
        (total - 1.0).abs() < 1e-9,
        "the bands sum to {:.6}, not 1",
        total
    );
}

#[test]
fn a_profile_agrees_with_the_long_run_sim() {
    // The profiler and the batch sim are two different loops over the same
    // engine. If they disagreed, one of them would be lying to somebody.
    let data = GameData::load().unwrap();
    let profile = profile(&data);
    let batch = crate::engine::sim::run(
        &data,
        crate::engine::sim::SimConfig {
            spins: PROFILE_ROUNDS,
            ante: false,
            line_bet_index: 0,
            seed: PROFILE_SEED,
        },
    );

    assert!(
        (profile.hit_frequency - batch.hit_frequency()).abs() < 0.02,
        "profiler {:.3} against batch {:.3}",
        profile.hit_frequency,
        batch.hit_frequency()
    );
    assert!(
        (profile.volatility - batch.stats.volatility()).abs() < 1.5,
        "profiler {:.2} against batch {:.2}",
        profile.volatility,
        batch.stats.volatility()
    );
}

#[test]
fn profiling_does_not_touch_the_players_session() {
    // The whole reason the profiler owns a scratch session. If it drew from
    // the live RNG, opening the machine picker would change the spins that
    // came after it — a save-and-reload divergence the player could see and
    // nobody could explain.
    let data = GameData::load().unwrap();

    let mut untouched = GameSession::new(&data, 4_242);
    let mut watched = GameSession::new(&data, 4_242);

    let mut profiler = Profiler::new(&data);
    for _ in 0..10 {
        profiler.step(&data);
    }

    for _ in 0..50 {
        untouched.balance = 1_000_000;
        watched.balance = 1_000_000;
        untouched.celebrations.clear();
        watched.celebrations.clear();
        let a = untouched.spin(&data).unwrap();
        let b = watched.spin(&data).unwrap();
        assert_eq!(a.result.grid, b.result.grid);
    }
    assert_eq!(untouched.balance, watched.balance);
}

#[test]
fn the_same_cabinet_profiles_the_same_way_twice() {
    // A figure that wobbled between viewings would read as a fault rather
    // than as a measurement.
    let data = GameData::load().unwrap();
    assert_eq!(profile(&data), profile(&data));
}

#[test]
fn the_catalog_spans_more_than_one_volatility() {
    // If every cabinet profiled the same, the picker would be telling the
    // player about a choice that does not exist.
    let mut labels: Vec<&str> = Vec::new();
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let measured = profile(&data);
        println!(
            "{:>9}: volatility {:.2} -> {}",
            machine.id,
            measured.volatility,
            measured.volatility_label()
        );
        labels.push(measured.volatility_label());
    }
    labels.sort_unstable();
    labels.dedup();
    assert!(labels.len() > 1, "every cabinet profiled as {:?}", labels);
}
