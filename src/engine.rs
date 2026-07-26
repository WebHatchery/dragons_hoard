//! Slot engine: strips → grid → outcome. Stateless; every entry point takes the
//! data it needs and returns a result rather than mutating session state.

pub mod cascade;
pub mod cluster;
pub mod evaluate;
pub mod reels;
pub mod sim;
/// The interactive path, driven headless (§5.33). Test-only, like the parts of
/// `sim` it is checked against: it exists to prove the shipped game pays what
/// the published figures say, not to be part of the shipped game.
#[cfg(test)]
pub mod soak;

pub use cascade::CascadeStep;
pub use evaluate::{evaluate, expand_wilds, EvalContext, SpinOutcome};
#[cfg(test)]
pub use reels::grid_from_stops;
pub use reels::{resting_grid, Grid};

use crate::data::GameData;
use macroquad_toolkit::rng::SeededRng;

/// Which rule set a spin runs under. Free spins expand wilds and multiply line
/// wins; the base game does neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinMode {
    Base,
    /// A free spin, and how many symbols the refine order has burned off the
    /// strips by the time it runs (§5.21). Zero on a cabinet without one.
    FreeSpin {
        burned: usize,
        /// What this run multiplies wins by (§5.64). Zero means the cabinet's
        /// own, which is what every caller wanted before the choice existed.
        multiplier: i64,
    },
}

/// The decided result of one spin. Animation only ever *reveals* this.
#[derive(Debug, Clone)]
pub struct SpinResult {
    /// Per-reel strip stop index. The reel animation is handed these up front so
    /// it can decelerate onto the right symbol; it never influences them.
    pub stops: Vec<usize>,
    /// The grid as evaluated — already wild-expanded during free spins. On a
    /// cascading cabinet this is the *landing* grid, the one the reels reveal;
    /// the chain that follows is in `cascades`.
    pub grid: Grid,
    /// Features and scatters as read from the landing grid, plus the **total**
    /// credits across every cascade step. Everything downstream of the engine
    /// reads this and needs to know nothing about cascades.
    pub outcome: SpinOutcome,
    /// The decided cascade chain (§5.15). One step on a cabinet that does not
    /// cascade, so consumers do not branch.
    pub cascades: Vec<CascadeStep>,
}

impl SpinResult {
    /// The grid the reels come to rest showing — the last in the chain.
    pub fn resting_grid(&self) -> &Grid {
        self.cascades.last().map_or(&self.grid, |step| &step.grid)
    }
}

pub fn spin(data: &GameData, rng: &mut SeededRng, line_bet: i64, mode: SpinMode) -> SpinResult {
    // The strips a free spin turns are not necessarily the strips the base
    // game turns (§5.21), and the stops have to be drawn against whichever set
    // is actually spinning.
    let reels = match mode {
        SpinMode::Base => data.reels.clone(),
        SpinMode::FreeSpin { burned, .. } => data.refined_reels(burned),
    };
    let stops = reels::pick_stops_on(&reels, rng);
    // Heights are drawn *after* the stops so a fixed cabinet's stream is
    // unchanged — `pick_heights` consumes nothing when there is no range. They
    // are not stored: the grid they produce *is* the record of the shape.
    let heights = reels::pick_heights(data, rng);
    let landed = reels::grid_on(data, &reels, &stops, &heights);

    let (grid, ctx) = match mode {
        SpinMode::Base => (landed, EvalContext::base(data, line_bet)),
        SpinMode::FreeSpin { multiplier, .. } => {
            let grid = if data.freespins.expanding_wilds {
                expand_wilds(data, &landed)
            } else {
                landed
            };
            let multiplier = if multiplier > 0 {
                multiplier
            } else {
                data.freespins.multiplier
            };
            (grid, EvalContext::free_spin_at(data, line_bet, multiplier))
        }
    };

    let mut outcome = evaluate(data, &grid, &ctx);

    // A cascading cabinet resolves its whole chain here, at commit, so the
    // animation reveals a decided sequence and consumes no randomness (§8.2).
    let cascades = match data.cascade.as_ref() {
        Some(config) => cascade::resolve(data, config, &grid, &stops, &ctx),
        None => vec![CascadeStep {
            grid: grid.clone(),
            cleared: Vec::new(),
            multiplier: 1,
            credits: outcome.win_credits,
            outcome: outcome.clone(),
        }],
    };

    // Wins are summed across the chain; features stay as the landing grid read
    // them, so scatters cannot arrive on a refill.
    outcome.win_credits = cascade::total_credits(&cascades);
    outcome.wins = cascades
        .iter()
        .flat_map(|step| step.outcome.wins.iter().cloned())
        .collect();
    outcome.total_credits = outcome.win_credits + outcome.scatter_credits;

    SpinResult {
        stops,
        grid,
        outcome,
        cascades,
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
            let result = spin(
                &data,
                &mut rng,
                10,
                SpinMode::FreeSpin {
                    burned: 0,
                    multiplier: 0,
                },
            );
            for reel in 0..result.grid.reel_count() {
                if result.grid.reel_contains(reel, wild) {
                    assert!(
                        (0..result.grid.rows_on(reel)).all(|row| result.grid.at(reel, row) == wild)
                    );
                }
            }
        }
    }
}
