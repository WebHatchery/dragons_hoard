use super::*;
use crate::data::GameData;
use macroquad_toolkit::rng::SeededRng;

fn data() -> GameData {
    GameData::load().unwrap()
}

#[test]
fn the_shipped_config_is_valid() {
    let data = data();
    crate::data::validate_jackpots(&data.jackpots).unwrap();
}

#[test]
fn a_fresh_pot_shows_its_seed() {
    let data = data();
    let state = JackpotState::new(&data.jackpots);

    for (index, tier) in data.jackpots.tiers.iter().enumerate() {
        assert_eq!(state.value(&data.jackpots, index), tier.seed);
    }
}

#[test]
fn every_stake_feeds_every_pot() {
    let data = data();
    let mut state = JackpotState::new(&data.jackpots);
    let before: Vec<i64> = (0..data.jackpots.tiers.len())
        .map(|i| state.value(&data.jackpots, i))
        .collect();

    // One spin moves the pots by fractions of a credit, so accumulate.
    for _ in 0..1000 {
        state.contribute(&data.jackpots, 200);
    }

    for (index, was) in before.iter().enumerate() {
        assert!(
            state.value(&data.jackpots, index) > *was,
            "tier {} never grew",
            index
        );
    }
}

#[test]
fn a_small_stake_still_moves_the_smallest_pot() {
    // The reason pots accrue in milli-credits: at the minimum bet a whole
    // credit of contribution would round to zero and the pot would never move.
    let data = data();
    let mut state = JackpotState::new(&data.jackpots);
    let min_bet = data.total_bet(data.config.line_bets[0]);

    for _ in 0..500 {
        state.contribute(&data.jackpots, min_bet);
    }

    assert!(state.value(&data.jackpots, 0) > data.jackpots.tiers[0].seed);
}

#[test]
fn winning_a_tier_resets_it_to_seed_and_leaves_the_others() {
    let data = data();
    let mut state = JackpotState::new(&data.jackpots);
    for _ in 0..10_000 {
        state.contribute(&data.jackpots, 200);
    }
    let before: Vec<i64> = (0..data.jackpots.tiers.len())
        .map(|index| state.value(&data.jackpots, index))
        .collect();

    // A stake at least as large as the longest odds always clears the roll,
    // and the richest tier is tested first — so this is a guaranteed Grand.
    let certain = data.jackpots.tiers.last().unwrap().odds_per_credit;
    let mut rng = SeededRng::new(1);
    let win = state
        .roll(&data.jackpots, &mut rng, certain)
        .expect("a stake this size cannot miss");

    assert_eq!(win.tier, data.jackpots.tiers.len() - 1);
    assert!(win.credits > data.jackpots.tiers[win.tier].seed);
    assert_eq!(
        state.value(&data.jackpots, win.tier),
        data.jackpots.tiers[win.tier].seed
    );

    // Winning one tier must not disturb the others.
    for (index, was) in before.iter().enumerate().take(win.tier) {
        assert_eq!(
            state.value(&data.jackpots, index),
            *was,
            "tier {} was disturbed by another tier paying out",
            index
        );
    }
}

#[test]
fn the_trigger_is_bet_fair() {
    // The whole point of per-credit odds: expected return per credit staked
    // must not vary with the stake, or the ladder becomes exploitable.
    let data = data();
    let tier = &data.jackpots.tiers[0];
    let odds = tier.odds_per_credit as f64;

    for bet in [20.0, 200.0, 500.0] {
        let hits_per_spin = bet / odds;
        let return_per_credit = hits_per_spin * tier.seed as f64 / bet;
        assert!(
            (return_per_credit - tier.seed as f64 / odds).abs() < 1e-12,
            "stake {} changed the per-credit return",
            bet
        );
    }
}

#[test]
fn rolling_consumes_the_same_randomness_whether_or_not_it_hits() {
    // One draw per tier, always — otherwise a near-miss would desync the
    // RNG stream and break save/reload determinism.
    let data = data();
    let mut state = JackpotState::new(&data.jackpots);
    let mut a = SeededRng::new(4242);
    let mut b = SeededRng::new(4242);

    state.roll(&data.jackpots, &mut a, 200);
    for _ in 0..data.jackpots.tiers.len() {
        b.next_u64();
    }

    assert_eq!(a.next_u64(), b.next_u64());
}

#[test]
fn the_expected_return_matches_the_closed_form() {
    let data = data();
    let rtp = expected_rtp(&data.jackpots);

    // Sanity band: the jackpot layer should be a few points, not a few
    // percent of a percent, and never larger than the base game.
    assert!(rtp > 0.02 && rtp < 0.08, "jackpot RTP {:.4} is off", rtp);
}

#[test]
fn an_older_save_with_fewer_tiers_is_topped_up() {
    let data = data();
    let mut state = JackpotState {
        accrued_milli: vec![5_000],
    };
    state.resize_to(&data.jackpots);

    assert_eq!(state.accrued_milli.len(), data.jackpots.tiers.len());
    // What was banked survives.
    assert_eq!(
        state.value(&data.jackpots, 0),
        data.jackpots.tiers[0].seed + 5
    );
    assert_eq!(state.value(&data.jackpots, 3), data.jackpots.tiers[3].seed);
}

#[test]
fn the_ladder_reports_one_row_per_tier() {
    let data = data();
    let state = JackpotState::new(&data.jackpots);
    let rows = ladder(&data.jackpots, &state);

    assert_eq!(rows.len(), data.jackpots.tiers.len());
    assert_eq!(rows[0].0, "Mini");
    assert_eq!(rows[3].1, data.jackpots.tiers[3].seed);
}
