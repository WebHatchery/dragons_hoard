//! Reel strips → visible grid. Pure functions; all randomness arrives as a
//! borrowed `SeededRng` so outcomes stay reproducible from a saved seed.

use crate::data::GameData;
use macroquad_toolkit::rng::SeededRng;

/// The visible symbol window: `reel_count` columns by `row_count` rows, stored
/// column-major so a reel's cells are contiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    reels: usize,
    rows: usize,
    cells: Vec<usize>,
}

impl Grid {
    /// Build from column-major columns, e.g. `[[a, b, c], [d, e, f], ...]`.
    pub fn from_columns(columns: &[Vec<usize>]) -> Self {
        let rows = columns.first().map(Vec::len).unwrap_or(0);
        debug_assert!(
            columns.iter().all(|column| column.len() == rows),
            "grid columns must all be the same height"
        );
        Self {
            reels: columns.len(),
            rows,
            cells: columns.iter().flatten().copied().collect(),
        }
    }

    pub fn reel_count(&self) -> usize {
        self.reels
    }

    pub fn row_count(&self) -> usize {
        self.rows
    }

    pub fn at(&self, reel: usize, row: usize) -> usize {
        self.cells[reel * self.rows + row]
    }

    pub fn set(&mut self, reel: usize, row: usize, symbol: usize) {
        let index = reel * self.rows + row;
        self.cells[index] = symbol;
    }

    pub fn cells(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        let rows = self.rows;
        self.cells
            .iter()
            .enumerate()
            .map(move |(index, symbol)| (index / rows, index % rows, *symbol))
    }

    /// How many cells across the whole grid hold `symbol`.
    pub fn count_of(&self, symbol: usize) -> usize {
        self.cells.iter().filter(|cell| **cell == symbol).count()
    }

    pub fn reel_contains(&self, reel: usize, symbol: usize) -> bool {
        (0..self.rows).any(|row| self.at(reel, row) == symbol)
    }
}

/// Pick one stop index per reel. `strip[stop]` becomes the top visible cell.
pub fn pick_stops(data: &GameData, rng: &mut SeededRng) -> Vec<usize> {
    data.reels
        .iter()
        .map(|strip| rng.below(strip.len()))
        .collect()
}

/// Read the visible window out of each strip, wrapping at the end.
pub fn grid_from_stops(data: &GameData, stops: &[usize]) -> Grid {
    let rows = data.config.row_count;
    let columns: Vec<Vec<usize>> = data
        .reels
        .iter()
        .enumerate()
        .map(|(reel, strip)| {
            let stop = stops.get(reel).copied().unwrap_or(0) % strip.len();
            (0..rows)
                .map(|row| strip[(stop + row) % strip.len()])
                .collect()
        })
        .collect();
    Grid::from_columns(&columns)
}

/// A default, deliberately non-winning display grid for a fresh session.
pub fn resting_grid(data: &GameData) -> Grid {
    grid_from_stops(data, &vec![0; data.reels.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_read_three_consecutive_strip_cells() {
        let data = GameData::load().unwrap();
        let stops = vec![0, 1, 2, 3, 4];
        let grid = grid_from_stops(&data, &stops);

        for (reel, stop) in stops.iter().enumerate() {
            let strip = &data.reels[reel];
            for row in 0..data.config.row_count {
                assert_eq!(grid.at(reel, row), strip[(stop + row) % strip.len()]);
            }
        }
    }

    #[test]
    fn stops_wrap_around_the_end_of_the_strip() {
        let data = GameData::load().unwrap();
        let last = data.reels[0].len() - 1;
        let grid = grid_from_stops(&data, &[last, 0, 0, 0, 0]);

        assert_eq!(grid.at(0, 0), data.reels[0][last]);
        assert_eq!(grid.at(0, 1), data.reels[0][0]);
        assert_eq!(grid.at(0, 2), data.reels[0][1]);
    }

    #[test]
    fn a_fixed_seed_reproduces_the_same_stops() {
        let data = GameData::load().unwrap();
        let mut a = SeededRng::new(12345);
        let mut b = SeededRng::new(12345);

        assert_eq!(pick_stops(&data, &mut a), pick_stops(&data, &mut b));
    }

    #[test]
    fn stops_stay_inside_their_strip() {
        let data = GameData::load().unwrap();
        let mut rng = SeededRng::new(7);

        for _ in 0..200 {
            for (reel, stop) in pick_stops(&data, &mut rng).iter().enumerate() {
                assert!(*stop < data.reels[reel].len());
            }
        }
    }
}
