//! Reel strips → visible grid. Pure functions; all randomness arrives as a
//! borrowed `SeededRng` so outcomes stay reproducible from a saved seed.

use crate::data::GameData;
use macroquad_toolkit::rng::SeededRng;

/// The visible symbol window, stored column-major so a reel's cells are
/// contiguous.
///
/// Reels do **not** all have to be the same height (§5.20). Most cabinets make
/// them so, but a Megaways-style machine rolls a fresh height for every reel on
/// every spin, and the grid is the one place that difference has to live —
/// everything above it asks the grid rather than doing the arithmetic itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    /// Visible rows per reel.
    heights: Vec<usize>,
    /// Where each reel's cells start in `cells`. A prefix sum of `heights`,
    /// cached because the flat index is computed on nearly every draw call and
    /// every evaluation.
    offsets: Vec<usize>,
    cells: Vec<usize>,
}

impl Grid {
    /// Build from column-major columns, e.g. `[[a, b, c], [d, e, f], ...]`.
    /// Columns may differ in length.
    pub fn from_columns(columns: &[Vec<usize>]) -> Self {
        let heights: Vec<usize> = columns.iter().map(Vec::len).collect();
        let mut offsets = Vec::with_capacity(heights.len());
        let mut running = 0usize;
        for height in &heights {
            offsets.push(running);
            running += height;
        }

        Self {
            heights,
            offsets,
            cells: columns.iter().flatten().copied().collect(),
        }
    }

    pub fn reel_count(&self) -> usize {
        self.heights.len()
    }

    /// Rows on one reel.
    pub fn rows_on(&self, reel: usize) -> usize {
        self.heights.get(reel).copied().unwrap_or(0)
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Flat index of a cell. **The only place this arithmetic lives** — it used
    /// to be `reel * rows + row` written out at eight call sites, which is fine
    /// until the reels stop being the same height.
    pub fn index(&self, reel: usize, row: usize) -> usize {
        self.offsets.get(reel).copied().unwrap_or(0) + row
    }

    pub fn at(&self, reel: usize, row: usize) -> usize {
        self.cells[self.index(reel, row)]
    }

    pub fn set(&mut self, reel: usize, row: usize, symbol: usize) {
        let index = self.index(reel, row);
        self.cells[index] = symbol;
    }

    pub fn cells(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        (0..self.reel_count()).flat_map(move |reel| {
            (0..self.rows_on(reel)).map(move |row| (reel, row, self.at(reel, row)))
        })
    }

    /// How many cells across the whole grid hold `symbol`.
    pub fn count_of(&self, symbol: usize) -> usize {
        self.cells.iter().filter(|cell| **cell == symbol).count()
    }

    pub fn reel_contains(&self, reel: usize, symbol: usize) -> bool {
        (0..self.rows_on(reel)).any(|row| self.at(reel, row) == symbol)
    }

    /// Ways through the grid: the product of the reel heights. `3^5 = 243` on a
    /// fixed cabinet, anything up to `7^6` on a shifting one (§5.20).
    pub fn ways(&self) -> usize {
        self.heights.iter().product()
    }
}

/// Pick one stop index per reel. `strip[stop]` becomes the top visible cell.
/// Stops against a cabinet's raw strips. Only the tests reach this now — the
/// engine draws against whichever set is actually spinning (§5.21).
pub fn pick_stops(data: &GameData, rng: &mut SeededRng) -> Vec<usize> {
    pick_stops_on(&data.reels, rng)
}

/// Stops against a given set of strips, which may have been refined (§5.21).
pub fn pick_stops_on(reels: &[Vec<usize>], rng: &mut SeededRng) -> Vec<usize> {
    reels.iter().map(|strip| rng.below(strip.len())).collect()
}

/// Read the visible window out of each strip, wrapping at the end.
pub fn grid_from_stops(data: &GameData, stops: &[usize]) -> Grid {
    grid_from_stops_and_heights(data, stops, &vec![data.config.row_count; data.reels.len()])
}

/// Read the window with a given height per reel (§5.20).
///
/// Heights are picked at commit alongside the stops and travel with them, so a
/// shifting spin is as decided as a fixed one — the animation reveals a board
/// whose *shape* was settled before a reel moved, not just its symbols.
pub fn grid_from_stops_and_heights(data: &GameData, stops: &[usize], heights: &[usize]) -> Grid {
    grid_on(data, &data.reels, stops, heights)
}

/// The window read out of a given set of strips.
pub fn grid_on(data: &GameData, reels: &[Vec<usize>], stops: &[usize], heights: &[usize]) -> Grid {
    let columns: Vec<Vec<usize>> = reels
        .iter()
        .enumerate()
        .map(|(reel, strip)| {
            let stop = stops.get(reel).copied().unwrap_or(0) % strip.len();
            let rows = heights.get(reel).copied().unwrap_or(data.config.row_count);
            (0..rows)
                .map(|row| strip[(stop + row) % strip.len()])
                .collect()
        })
        .collect();
    Grid::from_columns(&columns)
}

/// Roll a height for every reel. Uniform across the configured range: a
/// weighting toward the tall end would be a way to move RTP without touching the
/// paytable, and RTP lives in the data (§4).
pub fn pick_heights(data: &GameData, rng: &mut SeededRng) -> Vec<usize> {
    let Some(range) = data.config.reel_heights else {
        return vec![data.config.row_count; data.reels.len()];
    };
    let span = range.max.saturating_sub(range.min) + 1;
    (0..data.reels.len())
        .map(|_| range.min + rng.below(span))
        .collect()
}

/// A default, deliberately non-winning display grid for a fresh session.
pub fn resting_grid(data: &GameData) -> Grid {
    grid_from_stops(data, &vec![0; data.reels.len()])
}

// Tests live in the crate-level integration harness.
