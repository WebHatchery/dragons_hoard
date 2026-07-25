//! Ways-to-win evaluation (§5.14).
//!
//! # What a "way" is
//!
//! A payline machine asks: does this *one path* through the grid read as five
//! chests? A ways machine asks: does **any** path? A symbol pays if it appears
//! at least once on each of reels 1..n, and the win is multiplied by how many
//! distinct paths there are — the product of its per-reel counts. Two chests on
//! reel 1, one on reel 2 and three on reel 3 is `2 × 1 × 3 = 6` ways, all paid.
//!
//! On a 5×3 grid that is `3⁵ = 243` ways, and every one of them is always
//! active. There is no line to be off, which is the whole appeal: a win is a
//! property of the grid rather than of a path drawn across it.
//!
//! # Every symbol pays, except an all-wild run
//!
//! This is the rule that separates ways from lines, and it is a deliberate
//! design decision rather than an inherited one. On a payline, the run is one
//! contest and `best_line_result` picks the single best reading of it. In a ways
//! game symbols genuinely do pay *alongside* each other — a gem run and a chest
//! run on the same grid are two different sets of paths and both are real wins.
//! That is the mechanic.
//!
//! The one thing that must not happen is the **same cells paying twice under two
//! names**, and there is exactly one way for that to arise: a run made entirely
//! of wilds reads as every symbol at once. So a symbol's run is suppressed when
//! no genuine copy of it appears anywhere in the run — the wild pays for those
//! cells itself, once.
//!
//! `W W W C C` therefore pays *both* a three-wild run and a five-chest run: the
//! chest run has real chests on reels 4 and 5, so it is not the wild's win being
//! counted twice. `W W W` pays only as wild. Whether that combination is
//! generous is a tuning question the sim answers (§4), not a correctness one.

use super::{Win, WinSource};
use crate::data::GameData;
use crate::engine::reels::Grid;

/// Minimum run length that can pay, matching the payline evaluator.
const MIN_RUN: usize = 3;

/// Every paying symbol on the grid, with its ways count and the cells that
/// formed it.
pub fn wins(data: &GameData, grid: &Grid, line_bet: i64, multiplier: i64) -> Vec<Win> {
    let mut found = Vec::new();

    for (candidate, _) in data.symbols.iter() {
        if data.symbols.is_scatter(candidate) {
            continue;
        }
        let Some(win) = symbol_win(data, grid, candidate, line_bet, multiplier) else {
            continue;
        };
        found.push(win);
    }

    found
}

/// One symbol's ways run, or `None` if it does not pay.
fn symbol_win(
    data: &GameData,
    grid: &Grid,
    candidate: usize,
    line_bet: i64,
    multiplier: i64,
) -> Option<Win> {
    let is_wild_candidate = data.symbols.is_wild(candidate);

    let mut ways = 1usize;
    let mut run = 0usize;
    let mut cells: Vec<usize> = Vec::new();
    // Tracked so an all-wild run can be suppressed: it is the wild's win, and
    // paying it under another name as well would pay the same cells twice.
    let mut genuine = 0usize;

    for reel in 0..grid.reel_count() {
        let mut matches = 0usize;
        let mut reel_cells = Vec::new();

        for row in 0..grid.rows_on(reel) {
            let symbol = grid.at(reel, row);
            // A wild candidate matches only real wilds — it does not substitute
            // for itself, which would make every run infinite.
            let hit = symbol == candidate || (!is_wild_candidate && data.symbols.is_wild(symbol));
            if hit {
                matches += 1;
                reel_cells.push(grid.index(reel, row));
                if symbol == candidate {
                    genuine += 1;
                }
            }
        }

        // The run must be unbroken from reel 1, exactly as a payline run is.
        if matches == 0 {
            break;
        }
        ways *= matches;
        run += 1;
        cells.extend(reel_cells);
    }

    if run < MIN_RUN || genuine == 0 {
        return None;
    }
    let pay = data.symbols.pay(candidate, run);
    if pay <= 0 {
        return None;
    }

    Some(Win {
        source: WinSource::Ways(ways),
        symbol: candidate,
        count: run,
        credits: pay * ways as i64 * line_bet * multiplier,
        cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{GameData, MACHINES};

    /// The ways cabinet, since the other two evaluate by line.
    fn data() -> GameData {
        GameData::load_machine(
            MACHINES
                .iter()
                .find(|machine| machine.id == "ways")
                .expect("no ways machine in the catalog"),
        )
        .unwrap()
    }

    /// Build a grid from column-major short ids.
    fn grid(data: &GameData, columns: [[&str; 3]; 5]) -> Grid {
        let resolve = |id: &str| {
            data.symbols
                .iter()
                .find(|(_, def)| def.id == id)
                .map(|(index, _)| index)
                .unwrap_or_else(|| panic!("no symbol '{}'", id))
        };
        Grid::from_columns(
            &columns
                .iter()
                .map(|column| column.iter().map(|id| resolve(id)).collect())
                .collect::<Vec<Vec<usize>>>(),
        )
    }

    fn win_for<'a>(wins: &'a [Win], data: &GameData, id: &str) -> Option<&'a Win> {
        wins.iter()
            .find(|win| data.symbols.get(win.symbol).id == id)
    }

    #[test]
    fn one_of_each_across_three_reels_is_a_single_way() {
        let data = data();
        let grid = grid(
            &data,
            [
                ["chest", "copper", "copper"],
                ["copper", "chest", "copper"],
                ["copper", "copper", "chest"],
                ["jade", "jade", "jade"],
                ["jade", "jade", "jade"],
            ],
        );
        let wins = wins(&data, &grid, 10, 1);

        let chest = win_for(&wins, &data, "chest").expect("chest did not pay");
        assert_eq!(chest.count, 3);
        assert_eq!(chest.source, WinSource::Ways(1));
    }

    #[test]
    fn ways_multiply_across_reels() {
        // Two on reel 1, one on reel 2, three on reel 3 — six distinct paths,
        // and the payout must be six times the single-way value.
        let data = data();
        let two_one_three = grid(
            &data,
            [
                ["chest", "chest", "copper"],
                ["chest", "copper", "copper"],
                ["chest", "chest", "chest"],
                ["jade", "jade", "jade"],
                ["jade", "jade", "jade"],
            ],
        );
        let single = grid(
            &data,
            [
                ["chest", "copper", "copper"],
                ["chest", "copper", "copper"],
                ["chest", "copper", "copper"],
                ["jade", "jade", "jade"],
                ["jade", "jade", "jade"],
            ],
        );

        let many = win_for(&wins(&data, &two_one_three, 10, 1), &data, "chest")
            .expect("chest did not pay")
            .clone();
        let one = win_for(&wins(&data, &single, 10, 1), &data, "chest")
            .expect("chest did not pay")
            .clone();

        assert_eq!(many.source, WinSource::Ways(6));
        assert_eq!(one.source, WinSource::Ways(1));
        assert_eq!(many.credits, one.credits * 6);
    }

    #[test]
    fn a_run_must_start_on_reel_one() {
        // The same rule the payline evaluator enforces: reel 1 is where a win
        // begins, so a symbol missing there pays nothing however dense it is.
        let data = data();
        let grid = grid(
            &data,
            [
                ["copper", "copper", "copper"],
                ["chest", "chest", "chest"],
                ["chest", "chest", "chest"],
                ["chest", "chest", "chest"],
                ["chest", "chest", "chest"],
            ],
        );
        assert!(win_for(&wins(&data, &grid, 10, 1), &data, "chest").is_none());
    }

    #[test]
    fn several_symbols_pay_at_once() {
        // The whole point of the mechanic, and the thing a payline machine
        // cannot do: two different symbols, two different sets of paths, both
        // real wins on one grid.
        let data = data();
        let grid = grid(
            &data,
            [
                ["chest", "jade", "copper"],
                ["chest", "jade", "copper"],
                ["chest", "jade", "copper"],
                ["ruby", "ruby", "ruby"],
                ["ruby", "ruby", "ruby"],
            ],
        );
        let wins = wins(&data, &grid, 10, 1);

        assert!(win_for(&wins, &data, "chest").is_some());
        assert!(win_for(&wins, &data, "jade").is_some());
    }

    #[test]
    fn wilds_substitute_and_the_genuine_symbol_still_pays() {
        let data = data();
        let grid = grid(
            &data,
            [
                ["dragon", "copper", "copper"],
                ["dragon", "copper", "copper"],
                ["chest", "copper", "copper"],
                ["jade", "jade", "jade"],
                ["jade", "jade", "jade"],
            ],
        );
        let wins = wins(&data, &grid, 10, 1);

        let chest = win_for(&wins, &data, "chest").expect("wilds did not substitute");
        assert_eq!(chest.count, 3);
        // Two wilds is short of a paying wild run of its own.
        assert!(win_for(&wins, &data, "dragon").is_none());
    }

    #[test]
    fn an_all_wild_run_pays_only_as_the_wild() {
        // The one double-count the design has to prevent: three wilds read as
        // every symbol at once, and paying each of them would pay the same three
        // cells eight times over.
        //
        // Reels 4 and 5 are a wall of scatters. This is the ways version of the
        // isolation problem §11 records for paylines: any ordinary filler forms
        // its own genuine run — that is the mechanic — so the only way to leave
        // exactly one win standing is a symbol that cannot be substituted for
        // and cannot be run into.
        let data = data();
        let grid = grid(
            &data,
            [
                ["dragon", "dragon", "dragon"],
                ["dragon", "dragon", "dragon"],
                ["dragon", "dragon", "dragon"],
                ["fire", "fire", "fire"],
                ["fire", "fire", "fire"],
            ],
        );
        let wins = wins(&data, &grid, 10, 1);

        assert!(win_for(&wins, &data, "dragon").is_some());
        assert!(
            win_for(&wins, &data, "chest").is_none(),
            "an all-wild run paid as a substituted symbol too"
        );
        assert_eq!(
            wins.iter()
                .filter(|win| !data.symbols.is_scatter(win.symbol))
                .count(),
            1,
            "only the wild should have paid"
        );
    }

    #[test]
    fn a_wild_led_run_with_real_symbols_behind_it_pays_both() {
        // `W W W C C`: the chest run reaches five because reels 4 and 5 hold
        // genuine chests, so it is not the wild's win under another name.
        let data = data();
        let grid = grid(
            &data,
            [
                ["dragon", "copper", "copper"],
                ["dragon", "copper", "copper"],
                ["dragon", "copper", "copper"],
                ["chest", "copper", "copper"],
                ["chest", "copper", "copper"],
            ],
        );
        let wins = wins(&data, &grid, 10, 1);

        let chest = win_for(&wins, &data, "chest").expect("chest did not pay");
        assert_eq!(chest.count, 5);
        assert!(win_for(&wins, &data, "dragon").is_some());
    }

    #[test]
    fn a_wild_never_substitutes_for_the_scatter() {
        let data = data();
        let grid = grid(
            &data,
            [
                ["dragon", "dragon", "dragon"],
                ["dragon", "dragon", "dragon"],
                ["dragon", "dragon", "dragon"],
                ["dragon", "dragon", "dragon"],
                ["dragon", "dragon", "dragon"],
            ],
        );
        let wins = wins(&data, &grid, 10, 1);
        assert!(wins.iter().all(|win| !data.symbols.is_scatter(win.symbol)));
    }

    #[test]
    fn a_full_grid_of_one_symbol_pays_every_way() {
        // 3^5 = 243, the headline number on the cabinet.
        let data = data();
        let grid = grid(
            &data,
            [
                ["chest", "chest", "chest"],
                ["chest", "chest", "chest"],
                ["chest", "chest", "chest"],
                ["chest", "chest", "chest"],
                ["chest", "chest", "chest"],
            ],
        );
        let found = wins(&data, &grid, 10, 1);
        let chest = win_for(&found, &data, "chest").unwrap();

        assert_eq!(chest.source, WinSource::Ways(243));
        assert_eq!(chest.count, 5);
    }

    #[test]
    fn every_reported_cell_really_holds_the_symbol_or_a_wild() {
        // The cells drive the win highlight; a wrong one lights a symbol that
        // took no part in the win.
        let data = data();
        let grid = grid(
            &data,
            [
                ["chest", "dragon", "copper"],
                ["copper", "chest", "copper"],
                ["chest", "chest", "jade"],
                ["jade", "jade", "jade"],
                ["ruby", "ruby", "ruby"],
            ],
        );

        for win in wins(&data, &grid, 10, 1) {
            for cell in &win.cells {
                let symbol = grid.at(cell / 3, cell % 3);
                assert!(
                    symbol == win.symbol || data.symbols.is_wild(symbol),
                    "cell {} took no part in the {} win",
                    cell,
                    data.symbols.get(win.symbol).id
                );
            }
        }
    }
}
