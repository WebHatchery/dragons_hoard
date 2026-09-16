use super::*;
use crate::data::MACHINES;

fn profile(data: &GameData, tier: usize) -> TierProfile {
    let mut profiler = TierProfiler::new(data, tier);
    for _ in 0..10_000 {
        if let Some(profile) = profiler.step(data) {
            return profile;
        }
    }
    panic!("the tier profiler never finished");
}

#[test]
fn every_tier_on_every_cabinet_profiles_near_its_price() {
    // The tier profiler and `simulate_buys` are two loops over the same
    // purchase. Both must land on the machine's target, because that is
    // what the price was chosen to make true (§5.13).
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let target = data.featurebuy.target_rtp_permille as f64 / 1000.0;

        for (tier, def) in data.featurebuy.tiers.iter().enumerate() {
            let profile = profile(&data, tier);
            assert_eq!(profile.buys, TIER_ROUNDS);
            assert!(
                (profile.rtp - target).abs() < 0.30,
                "{}/{} profiled at {:.3} against {:.3}",
                machine.id,
                def.id,
                profile.rtp,
                target
            );
        }
    }
}

#[test]
fn a_fairly_priced_buy_still_loses_most_of_the_time() {
    // The point of the whole section. A tier priced at its expected value
    // hands back less than it cost on most purchases, because a few large
    // returns carry the average. If this ever came out near zero the
    // headline figure would be worthless and something would be wrong with
    // either the pricing or the measurement.
    let data = GameData::load().unwrap();
    for (tier, def) in data.featurebuy.tiers.iter().enumerate() {
        let profile = profile(&data, tier);
        assert!(
            (0.2..0.95).contains(&profile.below_cost),
            "{} comes back under the price {:.3} of the time",
            def.id,
            profile.below_cost
        );
    }
}

#[test]
fn the_bands_account_for_every_buy() {
    let data = GameData::load().unwrap();
    let profile = profile(&data, 0);
    assert!((profile.bands.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    assert!(profile.best > 0.0);
}

#[test]
fn each_tier_is_measured_on_its_own_stream() {
    // Sharing one seed across three tiers would make them three views of the
    // same run of luck, and the comparison between them meaningless.
    let data = GameData::load().unwrap();
    let first = profile(&data, 0);
    let second = profile(&data, 1);
    assert_ne!(first.bands, second.bands);
}

#[test]
fn profiling_a_tier_does_not_touch_the_players_session() {
    // Same rule as the machine profiler: opening the buy menu must not
    // consume a draw the player's next spin was going to use.
    let data = GameData::load().unwrap();
    let mut untouched = GameSession::new(&data, 7_777);
    let mut watched = GameSession::new(&data, 7_777);

    let mut profiler = TierProfiler::new(&data, 0);
    for _ in 0..3 {
        profiler.step(&data);
    }

    for _ in 0..40 {
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
