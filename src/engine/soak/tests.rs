use super::*;
use crate::data::MACHINES;
use crate::engine::sim::{self, SimConfig};

fn every_machine() -> Vec<GameData> {
    MACHINES
        .iter()
        .map(|machine| GameData::load_machine(machine).unwrap())
        .collect()
}

#[test]
fn the_books_balance_on_every_cabinet() {
    // The one that would catch a credit arriving from nowhere, or a stake
    // taken twice. `balance` and the two stat counters are maintained by
    // different code, so agreeing is evidence rather than arithmetic.
    for data in every_machine() {
        let tally = play(&data, 0x5EED_1234, 400)
            .unwrap_or_else(|fault| panic!("{}: {:?}", data.machine.id, fault));
        assert_eq!(tally.rounds, 400);
        assert!(tally.wagered > 0);
    }
}

#[test]
fn no_round_gets_stuck() {
    // Every feature in the game holds the reels in some way: a card, an open
    // board, a respin beat, a cascade. A round that never comes back would
    // freeze the real game too.
    for data in every_machine() {
        let tally = play(&data, 0xBEEF_0007, 300).expect("a round hung");
        assert!(tally.frames > 0);
        // A sane average. A round that took thousands of frames would mean
        // something is animating far longer than it looks.
        assert!(
            tally.frames / tally.rounds < 400,
            "{} averages {} frames a round",
            data.machine.id,
            tally.frames / tally.rounds
        );
    }
}

#[test]
fn the_interactive_path_pays_what_the_sim_says_it_does() {
    // The headline. Everything the GDD claims about return comes from the
    // headless path, which resolves features through `auto_play_*`; a player
    // goes through `pick_bonus` and `tick_holdspin`. If the two ever
    // diverged, the sim would keep measuring itself correctly forever and
    // every published figure would be about a game nobody plays.
    // Raised from 6,000 when the Seam (§5.80) landed. The two paths do not
    // share a stream — a player picks chests in a different order than
    // `auto_play` does — so this comparison has always been variance-limited
    // rather than exact, and 6,000 rounds left it thin enough that a single
    // progressive falling on one side and not the other read as a structural
    // difference. It was 0.06 apart across the catalog before; a third
    // feature was simply the straw.
    for data in every_machine() {
        let rounds = 24_000;
        let live = play(&data, 0xA11CE, rounds).expect("interactive run failed");
        let headless = sim::run(
            &data,
            SimConfig {
                spins: rounds,
                seed: 0xA11CE,
                ..SimConfig::default()
            },
        );

        let apart = (live.rtp() - headless.rtp()).abs();
        println!(
            "{:>9}: interactive {:.4} against headless {:.4} ({:+.4})",
            data.machine.id,
            live.rtp(),
            headless.rtp(),
            live.rtp() - headless.rtp()
        );
        assert!(
            apart < 0.22,
            "{}: interactive {:.4} against headless {:.4}",
            data.machine.id,
            live.rtp(),
            headless.rtp()
        );
    }
}

#[test]
fn both_paths_hit_at_the_same_rate() {
    // Hit frequency is far tighter than return at this sample size, because
    // it does not depend on the rare, enormous payouts. A divergence here is
    // a structural difference rather than variance.
    for data in every_machine() {
        let rounds = 6_000;
        let live = play(&data, 0x1234_5678, rounds).expect("interactive run failed");
        let headless = sim::run(
            &data,
            SimConfig {
                spins: rounds,
                seed: 0x1234_5678,
                ..SimConfig::default()
            },
        );

        // Round-level on both sides: band 0 is "the round paid nothing".
        let sim_hits = 1.0 - headless.stats.band_shares()[0];
        let apart = (live.hit_frequency() - sim_hits).abs();
        assert!(
            apart < 0.05,
            "{}: interactive hits {:.4} against headless {:.4}",
            data.machine.id,
            live.hit_frequency(),
            sim_hits
        );
    }
}

#[test]
fn the_run_is_deterministic() {
    // A soak test that cannot be reproduced is a soak test that cannot be
    // debugged when it eventually fails.
    let data = &every_machine()[0];
    let first = play(data, 0xF00D, 250).unwrap();
    let second = play(data, 0xF00D, 250).unwrap();
    assert_eq!(first.won, second.won);
    assert_eq!(first.wagered, second.wagered);
    assert_eq!(first.frames, second.frames);
}

#[test]
fn a_different_seed_is_a_different_run() {
    let data = &every_machine()[0];
    let first = play(data, 1, 250).unwrap();
    let second = play(data, 2, 250).unwrap();
    assert_ne!(first.won, second.won);
}

#[test]
fn every_cabinet_reaches_its_features() {
    // A run that never opened a board or woke the dragon would be checking
    // the base game and calling it the whole machine.
    for data in every_machine() {
        let tally = play(&data, 0xC0FFEE, 3_000).expect("run failed");
        assert!(
            tally.features > 0,
            "{} never reached a feature in 3000 rounds",
            data.machine.id
        );
    }
}

/// The long version, and the real proof.
///
/// At two hundred thousand rounds the hit frequency is all but free of
/// sampling noise, so the two paths agreeing on it to within a percentage
/// point is structural rather than lucky — measured, they agree to 0.0016
/// on every cabinet. Return still swings, being dominated by the rare
/// enormous payouts, which is why it gets the wider tolerance.
///
/// The residual gaps rank in **volatility order**: Frost and Dragon's Hoard,
/// the two with the fattest tails, sit about 1.4% apart, and Wyrmspire and
/// Avalanche within 0.1%. That is the signature of sampling error rather
/// than of a difference between the paths, which would not care how volatile
/// the cabinet was.
#[test]
#[ignore]
fn soak() {
    const ROUNDS: u64 = 200_000;
    for data in every_machine() {
        let live = play(&data, 0x50AC, ROUNDS)
            .unwrap_or_else(|fault| panic!("{}: {:?}", data.machine.id, fault));
        let headless = sim::run(
            &data,
            SimConfig {
                spins: ROUNDS,
                ante: false,
                seed: 0x50AC,
                ..SimConfig::default()
            },
        );

        // Round-level on both sides — see `Tally::hit_frequency`.
        let sim_hits = 1.0 - headless.stats.band_shares()[0];
        println!(
            "{:10} rtp {:.4} (sim {:.4})  hits {:.4} (sim {:.4})  features {}  frames/round {}",
            data.machine.id,
            live.rtp(),
            headless.rtp(),
            live.hit_frequency(),
            sim_hits,
            live.features,
            live.frames / live.rounds
        );

        assert!(
            (live.hit_frequency() - sim_hits).abs() < 0.01,
            "{}: hit rates differ structurally",
            data.machine.id
        );
        assert!(
            (live.rtp() - headless.rtp()).abs() < 0.06,
            "{}: returns differ by more than variance explains",
            data.machine.id
        );
    }
}
