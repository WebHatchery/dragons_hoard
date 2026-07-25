//! Slot engine: strips → grid → outcome. Stateless; every entry point takes the
//! data it needs and returns a result rather than mutating session state.

pub mod evaluate;
pub mod reels;
#[cfg(test)]
pub mod sim;

pub use evaluate::{evaluate, expand_wilds, EvalContext, SpinOutcome};
pub use reels::{grid_from_stops, pick_stops, resting_grid, Grid};

use crate::data::GameData;
use macroquad_toolkit::rng::SeededRng;

/// Which rule set a spin runs under. Free spins expand wilds and multiply line
/// wins; the base game does neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinMode {
    Base,
    FreeSpin,
}

/// The decided result of one spin. Animation only ever *reveals* this.
#[derive(Debug, Clone)]
pub struct SpinResult {
    /// Per-reel strip stop index. The reel animation is handed these up front so
    /// it can decelerate onto the right symbol; it never influences them.
    pub stops: Vec<usize>,
    /// The grid as evaluated — already wild-expanded during free spins.
    pub grid: Grid,
    pub outcome: SpinOutcome,
}

pub fn spin(data: &GameData, rng: &mut SeededRng, line_bet: i64, mode: SpinMode) -> SpinResult {
    let stops = pick_stops(data, rng);
    let landed = grid_from_stops(data, &stops);

    let (grid, ctx) = match mode {
        SpinMode::Base => (landed, EvalContext::base(data, line_bet)),
        SpinMode::FreeSpin => {
            let grid = if data.freespins.expanding_wilds {
                expand_wilds(data, &landed)
            } else {
                landed
            };
            (grid, EvalContext::free_spin(data, line_bet))
        }
    };

    let outcome = evaluate(data, &grid, &ctx);
    SpinResult {
        stops,
        grid,
        outcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_seed_reproduces_a_whole_spin() {
        let data = GameData::load().unwrap();
        let mut a = SeededRng::new(2024);
        let mut b = SeededRng::new(2024);

        let first = spin(&data, &mut a, 10, SpinMode::Base);
        let second = spin(&data, &mut b, 10, SpinMode::Base);

        assert_eq!(first.stops, second.stops);
        assert_eq!(first.grid, second.grid);
        assert_eq!(first.outcome, second.outcome);
    }

    #[test]
    fn free_spins_expand_wilds_before_evaluating() {
        let data = GameData::load().unwrap();
        let wild = data.symbols.wild().unwrap();

        let mut rng = SeededRng::new(99);
        for _ in 0..400 {
            let result = spin(&data, &mut rng, 10, SpinMode::FreeSpin);
            for reel in 0..result.grid.reel_count() {
                if result.grid.reel_contains(reel, wild) {
                    assert!(
                        (0..result.grid.row_count()).all(|row| result.grid.at(reel, row) == wild)
                    );
                }
            }
        }
    }
}
