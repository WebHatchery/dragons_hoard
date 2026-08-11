use super::*;
use crate::data::GameData;

fn config() -> BonusConfig {
    GameData::load().unwrap().bonus
}

fn round(seed: u64) -> BonusRound {
    let mut rng = SeededRng::new(seed);
    BonusRound::new(1000, &config(), &mut rng)
}

#[test]
fn the_board_holds_exactly_the_configured_blanks() {
    let config = config();
    let round = round(1);

    assert_eq!(round.board_size(), config.board_size);
    let blanks = round
        .board
        .iter()
        .filter(|cell| **cell == BonusCell::Blank)
        .count();
    assert_eq!(blanks, config.blanks);
}

#[test]
fn nothing_is_visible_before_it_is_picked() {
    // The UI reads cells through `revealed_cell`, so a bug there would let
    // the board be read ahead of the player.
    let round = round(2);
    for index in 0..round.board_size() {
        assert!(round.revealed_cell(index).is_none());
    }
}

#[test]
fn a_round_ends_on_the_last_blank_and_not_before() {
    let mut round = round(3);
    let needed = round.blanks_needed();
    let mut ended_on = None;

    for index in 0..round.board_size() {
        let before = round.blanks_found();
        if let Some(outcome) = round.pick(index) {
            ended_on = Some((index, outcome));
            break;
        }
        assert!(round.blanks_found() <= needed);
        assert!(round.blanks_found() >= before);
    }

    let (_, outcome) = ended_on.expect("the round never ended");
    assert_eq!(round.blanks_found(), needed);
    assert!(round.is_finished());
    assert_eq!(outcome.credits, round.running_credits());
}

#[test]
fn picking_the_same_chest_twice_costs_nothing() {
    // A double click must not be able to spend a blank.
    let mut round = round(4);
    round.pick(0);
    let blanks = round.blanks_found();
    let collected = round.collected_permille();

    assert_eq!(round.pick(0), None);
    assert_eq!(round.blanks_found(), blanks);
    assert_eq!(round.collected_permille(), collected);
}

#[test]
fn picking_after_the_round_is_over_is_ignored() {
    let mut round = round(5);
    auto_play(&mut round);
    let collected = round.collected_permille();

    for index in 0..round.board_size() {
        assert_eq!(round.pick(index), None);
    }
    assert_eq!(round.collected_permille(), collected);
}

#[test]
fn auto_play_always_terminates_and_matches_the_running_total() {
    for seed in 0..50 {
        let mut round = round(seed);
        let outcome = auto_play(&mut round);

        assert!(round.is_finished());
        assert_eq!(outcome.credits, round.running_credits());
        assert_eq!(outcome.prizes_taken, round.prizes_taken);
    }
}

#[test]
fn the_same_seed_deals_the_same_board() {
    let mut a = SeededRng::new(99);
    let mut b = SeededRng::new(99);
    let config = config();

    let first = BonusRound::new(500, &config, &mut a);
    let second = BonusRound::new(500, &config, &mut b);

    assert_eq!(first.board, second.board);
}

#[test]
fn the_expected_payout_matches_the_hatch_it_replaced() {
    // The whole design rests on this: a round should collect 1000 permille
    // on average, so the bonus pays what the instant hatch used to pay and
    // the RTP does not move.
    let config = config();
    let predicted = expected_permille(&config);
    assert!(
        (predicted - 1000.0).abs() < 120.0,
        "the prize table averages {:.0} permille, not ~1000",
        predicted
    );

    let mut rng = SeededRng::new(7);
    let mut total = 0i64;
    let runs = 20_000;
    for _ in 0..runs {
        let mut round = BonusRound::new(1000, &config, &mut rng);
        total += auto_play(&mut round).collected_permille;
    }
    let measured = total as f64 / runs as f64;

    assert!(
        (measured - predicted).abs() < 60.0,
        "measured {:.0} permille against a predicted {:.0}",
        measured,
        predicted
    );
}

#[test]
fn a_bigger_pot_pays_proportionally_more() {
    let config = config();
    let mut small = BonusRound::new(1_000, &config, &mut SeededRng::new(11));
    let mut large = BonusRound::new(10_000, &config, &mut SeededRng::new(11));

    let small_outcome = auto_play(&mut small);
    let large_outcome = auto_play(&mut large);

    assert_eq!(
        small_outcome.collected_permille,
        large_outcome.collected_permille
    );
    assert_eq!(large_outcome.credits, small_outcome.credits * 10);
}

#[test]
fn a_board_with_more_blanks_than_cells_still_deals() {
    // Guards a hand-edited config from producing a round that can never end.
    let config = BonusConfig {
        board_size: 2,
        blanks: 9,
        prizes_permille: vec![100],
    };
    let mut round = BonusRound::new(100, &config, &mut SeededRng::new(1));

    assert!(round.board_size() > round.blanks_needed());
    auto_play(&mut round);
    assert!(round.is_finished());
}
