//! Unit tests for the payline evaluator.
//!
//! Line-win rules are tested against [`best_line_result`] directly rather than
//! through a whole grid: on a 20-payline machine any filler you pick to isolate
//! one line will itself form runs on the weaving lines, so a grid-level fixture
//! cannot assert "exactly one win" without encoding the whole payline set.

use super::*;
use crate::engine::reels::Grid;

struct Fixture {
    data: GameData,
}

impl Fixture {
    fn new() -> Self {
        Self {
            data: GameData::load().unwrap(),
        }
    }

    fn id(&self, id: &str) -> usize {
        self.data.symbols.index_of(id).unwrap()
    }

    /// Read one payline exactly as `evaluate` would.
    fn line(&self, ids: &[&str]) -> Option<(usize, usize, i64)> {
        let cells: Vec<usize> = ids.iter().map(|id| self.id(id)).collect();
        best_line_result(&self.data, &cells)
    }

    fn grid(&self, columns: &[[&str; 3]]) -> Grid {
        let columns: Vec<Vec<usize>> = columns
            .iter()
            .map(|column| column.iter().map(|id| self.id(id)).collect())
            .collect();
        Grid::from_columns(&columns)
    }
}

#[test]
fn three_of_a_kind_pays_its_paytable_multiplier() {
    let fixture = Fixture::new();

    let (symbol, count, multiplier) = fixture
        .line(&["chest", "chest", "chest", "copper", "gold"])
        .unwrap();

    assert_eq!(symbol, fixture.id("chest"));
    assert_eq!(count, 3);
    assert_eq!(multiplier, fixture.data.symbols.pay(symbol, 3));
}

#[test]
fn two_of_a_kind_does_not_pay() {
    let fixture = Fixture::new();

    assert!(fixture
        .line(&["chest", "chest", "copper", "gold", "jade"])
        .is_none());
}

#[test]
fn runs_must_start_on_reel_one() {
    let fixture = Fixture::new();

    assert!(fixture
        .line(&["copper", "chest", "chest", "chest", "gold"])
        .is_none());
}

#[test]
fn the_longest_run_wins_over_a_shorter_prefix() {
    let fixture = Fixture::new();

    let (symbol, count, _) = fixture
        .line(&["copper", "copper", "copper", "copper", "gold"])
        .unwrap();

    assert_eq!(symbol, fixture.id("copper"));
    assert_eq!(count, 4);
}

#[test]
fn wilds_substitute_to_complete_a_run() {
    let fixture = Fixture::new();

    let (symbol, count, _) = fixture
        .line(&["chest", "dragon", "chest", "copper", "gold"])
        .unwrap();

    assert_eq!(symbol, fixture.id("chest"));
    assert_eq!(count, 3);
}

#[test]
fn wild_led_lines_pay_the_best_interpretation() {
    let fixture = Fixture::new();
    let symbols = &fixture.data.symbols;

    // Readable as three wilds or five chests; the higher payout must win.
    let (symbol, count, multiplier) = fixture
        .line(&["dragon", "dragon", "dragon", "chest", "chest"])
        .unwrap();

    assert_eq!(symbol, fixture.id("chest"));
    assert_eq!(count, 5);
    assert_eq!(multiplier, symbols.pay(fixture.id("chest"), 5));
    assert!(multiplier > symbols.pay(fixture.id("dragon"), 3));
}

#[test]
fn an_all_wild_line_pays_the_wild_value() {
    let fixture = Fixture::new();
    let dragon = fixture.id("dragon");

    let (symbol, count, multiplier) = fixture
        .line(&["dragon", "dragon", "dragon", "dragon", "dragon"])
        .unwrap();

    assert_eq!(symbol, dragon);
    assert_eq!(count, 5);
    assert_eq!(multiplier, fixture.data.symbols.pay(dragon, 5));
}

#[test]
fn wilds_never_substitute_for_the_scatter() {
    let fixture = Fixture::new();

    // A scatter on reel one kills the line outright.
    assert!(fixture
        .line(&["fire", "dragon", "dragon", "copper", "gold"])
        .is_none());
    // And wilds cannot reach through a scatter to build a run.
    assert!(fixture
        .line(&["dragon", "dragon", "fire", "fire", "fire"])
        .is_none());
}

#[test]
fn win_credits_scale_with_the_line_bet() {
    let fixture = Fixture::new();
    let grid = fixture.grid(&[
        ["ruby", "chest", "ruby"],
        ["ruby", "chest", "ruby"],
        ["ruby", "chest", "ruby"],
        ["ruby", "copper", "ruby"],
        ["ruby", "copper", "ruby"],
    ]);

    let one = evaluate(&fixture.data, &grid, &EvalContext::base(&fixture.data, 1));
    let ten = evaluate(&fixture.data, &grid, &EvalContext::base(&fixture.data, 10));

    assert!(one.win_credits > 0);
    assert_eq!(ten.win_credits, one.win_credits * 10);
}

#[test]
fn scatters_pay_anywhere_off_the_paylines() {
    let fixture = Fixture::new();
    let grid = fixture.grid(&[
        ["fire", "copper", "gold"],
        ["copper", "gold", "fire"],
        ["gold", "fire", "copper"],
        ["copper", "gold", "copper"],
        ["gold", "copper", "gold"],
    ]);

    let outcome = evaluate(&fixture.data, &grid, &EvalContext::base(&fixture.data, 10));

    assert_eq!(outcome.scatter_count, 3);
    // Scatters multiply the total bet, which is 20 lines x 10.
    assert_eq!(
        outcome.scatter_credits,
        fixture.data.symbols.pay(fixture.id("fire"), 3) * 200
    );
    assert_eq!(outcome.free_spins_awarded, 10);
}

#[test]
fn fewer_than_three_scatters_award_nothing() {
    let fixture = Fixture::new();
    let grid = fixture.grid(&[
        ["fire", "copper", "gold"],
        ["copper", "gold", "fire"],
        ["gold", "copper", "copper"],
        ["copper", "gold", "copper"],
        ["gold", "copper", "gold"],
    ]);

    let outcome = evaluate(&fixture.data, &grid, &EvalContext::base(&fixture.data, 10));

    assert_eq!(outcome.scatter_count, 2);
    assert_eq!(outcome.scatter_credits, 0);
    assert_eq!(outcome.free_spins_awarded, 0);
}

#[test]
fn eggs_on_the_grid_are_counted_for_the_hoard() {
    let fixture = Fixture::new();
    let grid = fixture.grid(&[
        ["egg", "copper", "copper"],
        ["copper", "egg", "copper"],
        ["copper", "copper", "copper"],
        ["copper", "copper", "egg"],
        ["copper", "copper", "copper"],
    ]);

    let outcome = evaluate(&fixture.data, &grid, &EvalContext::base(&fixture.data, 1));

    assert_eq!(outcome.egg_count, 3);
}

#[test]
fn the_free_spin_multiplier_lifts_wins_but_not_scatters() {
    let fixture = Fixture::new();
    let grid = fixture.grid(&[
        ["fire", "chest", "copper"],
        ["copper", "chest", "fire"],
        ["fire", "chest", "copper"],
        ["copper", "copper", "copper"],
        ["copper", "copper", "copper"],
    ]);

    let base = evaluate(&fixture.data, &grid, &EvalContext::base(&fixture.data, 10));
    let free = evaluate(
        &fixture.data,
        &grid,
        &EvalContext::free_spin_at(&fixture.data, 10, fixture.data.freespins.multiplier),
    );

    assert!(base.win_credits > 0);
    assert!(base.scatter_credits > 0);
    assert_eq!(
        free.win_credits,
        base.win_credits * fixture.data.freespins.multiplier
    );
    assert_eq!(free.scatter_credits, base.scatter_credits);
}

#[test]
fn expanding_wilds_fill_the_whole_reel() {
    let fixture = Fixture::new();
    let dragon = fixture.id("dragon");
    let copper = fixture.id("copper");
    let grid = expand_wilds(
        &fixture.data,
        &fixture.grid(&[
            ["copper", "dragon", "copper"],
            ["copper", "copper", "copper"],
            ["dragon", "copper", "copper"],
            ["copper", "copper", "copper"],
            ["copper", "copper", "copper"],
        ]),
    );

    assert_eq!(grid.count_of(dragon), 6);
    for row in 0..3 {
        assert_eq!(grid.at(0, row), dragon);
        assert_eq!(grid.at(2, row), dragon);
        assert_eq!(grid.at(1, row), copper);
    }
}

#[test]
fn the_total_is_the_sum_of_line_and_scatter_credits() {
    let fixture = Fixture::new();
    let grid = fixture.grid(&[
        ["fire", "chest", "copper"],
        ["copper", "chest", "fire"],
        ["fire", "chest", "copper"],
        ["copper", "copper", "copper"],
        ["copper", "copper", "copper"],
    ]);

    let outcome = evaluate(&fixture.data, &grid, &EvalContext::base(&fixture.data, 5));

    assert_eq!(
        outcome.total_credits,
        outcome.win_credits + outcome.scatter_credits
    );
    assert_eq!(
        outcome.win_credits,
        outcome.wins.iter().map(|win| win.credits).sum::<i64>()
    );
}
