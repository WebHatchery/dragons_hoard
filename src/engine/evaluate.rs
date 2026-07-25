//! Payline / scatter evaluation. Pure: grid in, `SpinOutcome` out.

use crate::data::{GameData, MAX_RUN};
use crate::engine::reels::Grid;

/// One paying payline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineWin {
    /// Index into `GameData::paylines`.
    pub line: usize,
    /// The symbol the line was paid as (may differ from the leading cell when
    /// wilds lead the run).
    pub symbol: usize,
    pub count: usize,
    pub credits: i64,
}

/// Everything one spin produced, before any of it is applied to the session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpinOutcome {
    pub line_wins: Vec<LineWin>,
    pub line_credits: i64,
    pub scatter_count: usize,
    pub scatter_credits: i64,
    /// Hoard symbols on the grid; each one banks a line bet into the hatch pot.
    pub egg_count: usize,
    pub free_spins_awarded: u32,
    pub total_credits: i64,
}

/// Bet and feature modifiers in force for a single evaluation.
#[derive(Debug, Clone, Copy)]
pub struct EvalContext {
    pub line_bet: i64,
    pub total_bet: i64,
    /// Applied to line wins only — scatter pays already scale with total bet.
    pub win_multiplier: i64,
}

impl EvalContext {
    pub fn base(data: &GameData, line_bet: i64) -> Self {
        Self {
            line_bet,
            total_bet: data.total_bet(line_bet),
            win_multiplier: 1,
        }
    }

    pub fn free_spin(data: &GameData, line_bet: i64) -> Self {
        Self {
            win_multiplier: data.freespins.multiplier.max(1),
            ..Self::base(data, line_bet)
        }
    }
}

/// Wilds swallow their whole column. Used during free spins.
pub fn expand_wilds(data: &GameData, grid: &Grid) -> Grid {
    let Some(wild) = data.symbols.wild() else {
        return grid.clone();
    };

    let mut expanded = grid.clone();
    for reel in 0..grid.reel_count() {
        if grid.reel_contains(reel, wild) {
            for row in 0..grid.row_count() {
                expanded.set(reel, row, wild);
            }
        }
    }
    expanded
}

pub fn evaluate(data: &GameData, grid: &Grid, ctx: &EvalContext) -> SpinOutcome {
    let mut outcome = SpinOutcome::default();

    let mut line_cells = Vec::with_capacity(grid.reel_count());
    for (index, payline) in data.paylines.iter().enumerate() {
        line_cells.clear();
        line_cells.extend(
            payline
                .rows
                .iter()
                .enumerate()
                .map(|(reel, row)| grid.at(reel, *row)),
        );

        if let Some((symbol, count, multiplier)) = best_line_result(data, &line_cells) {
            let credits = multiplier * ctx.line_bet * ctx.win_multiplier;
            outcome.line_credits += credits;
            outcome.line_wins.push(LineWin {
                line: index,
                symbol,
                count,
                credits,
            });
        }
    }

    if let Some(scatter) = data.symbols.scatter() {
        let count = grid.count_of(scatter);
        outcome.scatter_count = count;
        let paid_count = count.min(MAX_RUN);
        outcome.scatter_credits = data.symbols.pay(scatter, paid_count) * ctx.total_bet;
        outcome.free_spins_awarded = data.freespins.award_for(paid_count);
    }

    if let Some(hoard) = data.symbols.hoard() {
        outcome.egg_count = grid.count_of(hoard);
    }

    outcome.total_credits = outcome.line_credits + outcome.scatter_credits;
    outcome
}

/// Best-paying interpretation of one payline, left to right.
///
/// A wild-led run can be read as several different symbols (`W W W C C` is both
/// a 3-of-a-kind wild and a 5-of-a-kind chest); every payable symbol is tried
/// and the highest payout wins. Returns `(symbol, run length, multiplier)`.
fn best_line_result(data: &GameData, cells: &[usize]) -> Option<(usize, usize, i64)> {
    let mut best: Option<(usize, usize, i64)> = None;

    for (candidate, _) in data.symbols.iter() {
        if data.symbols.is_scatter(candidate) {
            continue;
        }

        let run = leading_run(data, cells, candidate);
        if run < 3 {
            continue;
        }

        let multiplier = data.symbols.pay(candidate, run);
        if multiplier <= 0 {
            continue;
        }

        let better = match best {
            None => true,
            Some((_, _, best_multiplier)) => multiplier > best_multiplier,
        };
        if better {
            best = Some((candidate, run, multiplier));
        }
    }

    best
}

/// Leading cells that read as `candidate`, counting wilds as substitutes.
fn leading_run(data: &GameData, cells: &[usize], candidate: usize) -> usize {
    cells
        .iter()
        .take_while(|cell| **cell == candidate || data.symbols.is_wild(**cell))
        .count()
}

#[cfg(test)]
mod tests;
