use super::*;
use crate::data::GameData;

fn config() -> GambleConfig {
    GameData::load().unwrap().gamble
}

fn round(win: i64) -> GambleRound {
    GambleRound::new(win, 200, &config())
}

#[test]
fn a_correct_guess_doubles_and_a_wrong_one_takes_everything() {
    let config = config();
    let mut rng = SeededRng::new(1);
    let mut doubled = 0;
    let mut busted = 0;

    for _ in 0..400 {
        let mut round = GambleRound::new(100, 200, &config);
        let flip = round.flip(Scale::Ember, &mut rng).unwrap();
        if flip.won {
            assert_eq!(round.stake(), 200);
            doubled += 1;
        } else {
            assert_eq!(round.stake(), 0);
            assert!(!round.can_flip(), "a loss must end the round");
            busted += 1;
        }
    }

    assert!(doubled > 0 && busted > 0, "the sample was one-sided");
}

#[test]
fn the_scale_is_fair() {
    // A shaved gamble is the standard way this feature is made profitable.
    // This asserts it is not.
    let config = config();
    let mut rng = SeededRng::new(20_260_725);
    let rounds = 200_000;
    let mut wins = 0;

    for _ in 0..rounds {
        let mut round = GambleRound::new(100, 200, &config);
        if round.flip(Scale::Ember, &mut rng).unwrap().won {
            wins += 1;
        }
    }

    let rate = wins as f64 / rounds as f64;
    assert!(
        (rate - 0.5).abs() < 0.01,
        "the scale landed ember {:.4} of the time",
        rate
    );
}

#[test]
fn the_gamble_returns_what_it_risks() {
    // The assertion the whole design rests on: an even-money double cannot
    // move RTP, only variance. Every round is pushed as far as the ladder
    // allows, which is the worst case for the claim.
    let config = config();
    let mut rng = SeededRng::new(4242);
    let rounds = 200_000;
    let stake = 100i64;

    let mut risked = 0i64;
    let mut returned = 0i64;
    for _ in 0..rounds {
        let mut round = GambleRound::new(stake, 200, &config);
        risked += stake;
        while round.can_flip() {
            let _ = round.flip(Scale::Ember, &mut rng);
        }
        returned += round.take();
    }

    let ratio = returned as f64 / risked as f64;
    assert!(
        (ratio - 1.0).abs() < 0.03,
        "gambling returned {:.4} of what it risked",
        ratio
    );
}

#[test]
fn a_half_gamble_is_fair_too() {
    let config = config();
    let mut rng = SeededRng::new(99);
    let rounds = 200_000;
    let stake = 1_000i64;

    let mut risked = 0i64;
    let mut returned = 0i64;
    for _ in 0..rounds {
        let mut round = GambleRound::new(stake, 200, &config);
        risked += stake;
        let _ = round.flip_half(Scale::Ash, &mut rng);
        returned += round.take();
    }

    let ratio = returned as f64 / risked as f64;
    assert!(
        (ratio - 1.0).abs() < 0.03,
        "half-gambling returned {:.4} of what it risked",
        ratio
    );
}

#[test]
fn a_half_gamble_banks_half_out_of_reach() {
    let config = config();
    let mut rng = SeededRng::new(3);

    for seed_step in 0..80 {
        let mut round = GambleRound::new(400, 200, &config);
        let _ = round.flip_half(Scale::Ember, &mut rng);
        assert!(
            round.standing() >= 200,
            "seed step {}: banked half was lost",
            seed_step
        );
    }
}

#[test]
fn the_odd_credit_goes_to_the_player() {
    let config = config();
    let mut rng = SeededRng::new(5);
    let mut round = GambleRound::new(101, 200, &config);
    round.flip_half(Scale::Ember, &mut rng).unwrap();

    // 101 halves to 50 risked and 51 kept.
    assert_eq!(round.banked(), 51);
}

#[test]
fn the_ladder_stops_at_its_cap() {
    let config = config();
    let mut rng = SeededRng::new(7);

    for _ in 0..500 {
        let mut round = GambleRound::new(1, 1_000_000_000, &config);
        while round.can_flip() {
            let _ = round.flip(Scale::Ember, &mut rng);
        }
        assert!(round.steps() <= round.max_steps());
    }
}

#[test]
fn a_stake_over_the_ceiling_cannot_be_gambled_again() {
    // The ceiling is what stops a lucky run compounding without bound.
    let mut config = config();
    config.ceiling_multiple = 2;
    config.max_steps = 20;

    let mut rng = SeededRng::new(11);
    // Ceiling is 2 x 200 = 400 credits.
    let mut round = GambleRound::new(500, 200, &config);
    assert!(!round.can_flip());
    assert_eq!(
        round.flip(Scale::Ember, &mut rng),
        Err(GambleBlocked::LimitReached)
    );
}

#[test]
fn a_finished_round_cannot_be_gambled() {
    let config = config();
    let mut rng = SeededRng::new(13);
    let mut round = GambleRound::new(100, 200, &config);
    let taken = round.take();

    assert_eq!(taken, 100);
    assert_eq!(
        round.flip(Scale::Ember, &mut rng),
        Err(GambleBlocked::NotOffered)
    );
    assert_eq!(round.take(), 0, "taking twice must not pay twice");
}

#[test]
fn half_is_refused_when_the_machine_forbids_it() {
    let mut config = config();
    config.allow_half = false;
    let mut rng = SeededRng::new(17);
    let mut round = GambleRound::new(100, 200, &config);

    assert_eq!(
        round.flip_half(Scale::Ember, &mut rng),
        Err(GambleBlocked::CannotHalve)
    );
    assert_eq!(round.stake(), 100, "the refused half must not have staked");
}

#[test]
fn a_flip_consumes_the_same_randomness_whether_it_wins_or_loses() {
    // A near-miss that drew a different amount of randomness would desync
    // the stream and break save/reload determinism — the same rule the
    // jackpot roll follows (§5.6).
    let config = config();
    let mut a = SeededRng::new(555);
    let mut b = SeededRng::new(555);

    for _ in 0..200 {
        let mut win = GambleRound::new(100, 200, &config);
        let mut lose = GambleRound::new(100, 200, &config);
        let _ = win.flip(Scale::Ember, &mut a);
        let _ = lose.flip(Scale::Ash, &mut b);
    }
    assert_eq!(a.next_u64(), b.next_u64(), "the streams diverged");
}

#[test]
fn the_same_seed_replays_the_same_round() {
    let mut a = SeededRng::new(808);
    let mut b = SeededRng::new(808);

    let mut first = round(500);
    let mut second = round(500);
    while first.can_flip() {
        let _ = first.flip(Scale::Ember, &mut a);
    }
    while second.can_flip() {
        let _ = second.flip(Scale::Ember, &mut b);
    }
    assert_eq!(first.take(), second.take());
}
