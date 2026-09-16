use super::*;
use crate::data::GameData;

fn config() -> HoldSpinConfig {
    GameData::load().unwrap().holdspin
}

fn round(seed: u64, seeds: &[usize]) -> (HoldSpinRound, SeededRng) {
    let mut rng = SeededRng::new(seed);
    let round = HoldSpinRound::new(15, seeds, 100, &config(), &mut rng);
    (round, rng)
}

#[test]
fn the_triggering_eggs_are_already_locked() {
    let (round, _) = round(1, &[0, 4, 7, 11]);

    assert_eq!(round.coins(), 4);
    for index in [0, 4, 7, 11] {
        assert!(round.cell(index).is_some(), "cell {} did not lock", index);
        assert!(round.just_locked(index));
    }
    assert!(round.cell(1).is_none());
}

#[test]
fn every_locked_coin_is_worth_a_value_from_the_table() {
    let config = config();
    let multiples: Vec<i64> = config.coin_values.iter().map(|v| v.multiple).collect();

    let (mut round, mut rng) = round(2, &[0, 1, 2, 3]);
    auto_play(&mut round, &config, &mut rng);

    for index in 0..round.cell_count() {
        if let Some(credits) = round.cell(index) {
            assert!(
                multiples.contains(&(credits / 100)),
                "cell {} holds {} credits, off the table",
                index,
                credits
            );
        }
    }
}

#[test]
fn a_coin_resets_the_respin_allowance() {
    // The rule the feature turns on. Built directly rather than fished for:
    // a config that always lands a coin must never lose a respin.
    let always = HoldSpinConfig {
        coin_chance_permille: 1000,
        ..config()
    };
    let mut rng = SeededRng::new(3);
    let mut round = HoldSpinRound::new(15, &[0], 100, &always, &mut rng);

    round.respin(&always, &mut rng);
    assert_eq!(round.respins_left(), round.respins_max());
    assert!(round.is_full());
    assert!(round.is_finished());
}

#[test]
fn a_dry_board_runs_out_of_respins() {
    let never = HoldSpinConfig {
        coin_chance_permille: 0,
        ..config()
    };
    let mut rng = SeededRng::new(4);
    let mut round = HoldSpinRound::new(15, &[0, 1, 2, 3], 100, &never, &mut rng);

    for _ in 0..never.respins.max(1) - 1 {
        assert!(round.respin(&never, &mut rng).is_none());
    }
    let outcome = round
        .respin(&never, &mut rng)
        .expect("the round never ended");

    assert_eq!(outcome.coins, 4);
    assert!(!outcome.full_board);
    assert_eq!(outcome.credits, round.collected());
    assert_eq!(outcome.respins_used, never.respins);
}

#[test]
fn a_full_board_pays_the_bonus_on_top() {
    let always = HoldSpinConfig {
        coin_chance_permille: 1000,
        ..config()
    };
    let mut rng = SeededRng::new(5);
    let mut round = HoldSpinRound::new(15, &[0], 100, &always, &mut rng);
    let outcome = auto_play(&mut round, &always, &mut rng);

    assert!(outcome.full_board);
    assert_eq!(outcome.coins, 15);
    assert_eq!(
        outcome.credits,
        round.collected() + 100 * always.full_board_multiple
    );
}

#[test]
fn respinning_a_finished_round_is_ignored() {
    let config = config();
    let (mut round, mut rng) = round(6, &[0, 1, 2, 3]);
    let outcome = auto_play(&mut round, &config, &mut rng);

    assert!(round.respin(&config, &mut rng).is_none());
    assert_eq!(
        round.collected()
            + if outcome.full_board {
                round.full_board_credits
            } else {
                0
            },
        outcome.credits
    );
}

#[test]
fn auto_play_always_terminates_even_on_a_config_that_never_runs_dry() {
    // A hand-edited 1000‰ chance resets the allowance every single respin;
    // only filling the board can stop it, and the bound guarantees it does.
    let always = HoldSpinConfig {
        coin_chance_permille: 1000,
        ..config()
    };
    let mut rng = SeededRng::new(7);
    let mut round = HoldSpinRound::new(15, &[], 100, &always, &mut rng);

    auto_play(&mut round, &always, &mut rng);
    assert!(round.is_finished());
}

#[test]
fn the_same_seed_plays_the_same_round() {
    let config = config();
    let mut first = SeededRng::new(88);
    let mut second = SeededRng::new(88);

    let mut a = HoldSpinRound::new(15, &[2, 5, 9, 12], 100, &config, &mut first);
    let mut b = HoldSpinRound::new(15, &[2, 5, 9, 12], 100, &config, &mut second);

    assert_eq!(
        auto_play(&mut a, &config, &mut first),
        auto_play(&mut b, &config, &mut second)
    );
}

#[test]
fn a_bigger_stake_pays_proportionally_more() {
    let config = config();
    let mut small_rng = SeededRng::new(21);
    let mut large_rng = SeededRng::new(21);

    let mut small = HoldSpinRound::new(15, &[0, 1, 2, 3], 100, &config, &mut small_rng);
    let mut large = HoldSpinRound::new(15, &[0, 1, 2, 3], 1000, &config, &mut large_rng);

    let small_outcome = auto_play(&mut small, &config, &mut small_rng);
    let large_outcome = auto_play(&mut large, &config, &mut large_rng);

    assert_eq!(large_outcome.coins, small_outcome.coins);
    assert_eq!(large_outcome.credits, small_outcome.credits * 10);
}

#[test]
fn the_mean_coin_matches_the_weighted_table() {
    let config = config();
    let predicted = mean_coin_multiple(&config);

    let mut rng = SeededRng::new(9);
    let runs = 40_000;
    let total: i64 = (0..runs).map(|_| roll_coin(&config, 1, &mut rng)).sum();
    let measured = total as f64 / runs as f64;

    assert!(
        (measured - predicted).abs() < predicted * 0.05,
        "rolled a mean of {:.2}x against a table mean of {:.2}x",
        measured,
        predicted
    );
}

#[test]
fn the_shipped_round_pays_within_the_designed_band() {
    // The feature's value has no closed form (the coin count is a Markov
    // process), so it is pinned by measurement. This test exists to make a
    // coin-table or chance edit visible here rather than only as a drifting
    // total RTP three tests away.
    let config = config();
    let mut rng = SeededRng::new(1234);
    let runs = 20_000;

    let mut credits = 0i64;
    let mut coins = 0usize;
    let mut full = 0usize;
    for _ in 0..runs {
        let mut round = HoldSpinRound::new(15, &[0, 1, 2, 3], 100, &config, &mut rng);
        let outcome = auto_play(&mut round, &config, &mut rng);
        credits += outcome.credits;
        coins += outcome.coins;
        full += usize::from(outcome.full_board);
    }

    let per_round = credits as f64 / runs as f64 / 100.0;
    let mean_coins = coins as f64 / runs as f64;
    println!(
        "hold&spin: {:.1}x total bet per round | {:.1} coins | full board {:.3}%",
        per_round,
        mean_coins,
        full as f64 / runs as f64 * 100.0
    );

    assert!(
        (20.0..80.0).contains(&per_round),
        "a round pays {:.1}x total bet, outside the designed 20-80x band",
        per_round
    );
    assert!(
        mean_coins >= 4.0,
        "a round collects {:.1} coins, fewer than the four it started with",
        mean_coins
    );
}
