//! The Dragon's Hoard meter — the long-horizon egg-collection meta.

use crate::data::GameConfig;
use serde::{Deserialize, Serialize};

/// Eggs collected, plus the pot each egg banked.
///
/// The pot exists so the meter cannot be farmed at the minimum bet and cashed
/// out at the maximum — an egg is always worth the line bet it landed on.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HoardState {
    pub count: u32,
    pub pot: i64,
}

impl HoardState {
    pub fn add_eggs(&mut self, eggs: usize, line_bet: i64) {
        self.count += eggs as u32;
        self.pot += eggs as i64 * line_bet;
    }

    /// Pay out and reset when the meter is full. Eggs beyond capacity carry over
    /// with their share of the pot so nothing is silently swallowed.
    pub fn take_hatch(&mut self, config: &GameConfig) -> Option<i64> {
        if self.count < config.hoard_capacity {
            return None;
        }

        let carry_count = self.count - config.hoard_capacity;
        let carry_pot = if self.count > 0 {
            self.pot * carry_count as i64 / self.count as i64
        } else {
            0
        };
        let hatched_pot = self.pot - carry_pot;

        self.count = carry_count;
        self.pot = carry_pot;

        Some(hatched_pot * config.hatch_pot_multiplier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    #[test]
    fn the_hoard_pays_its_pot_and_carries_the_overflow() {
        let data = GameData::load().unwrap();
        let mut hoard = HoardState::default();
        let capacity = data.config.hoard_capacity as usize;

        hoard.add_eggs(capacity + 1, 10);
        let prize = hoard.take_hatch(&data.config).unwrap();

        // 16 eggs at 10 banked 160; one egg's worth carries over.
        assert_eq!(hoard.count, 1);
        assert_eq!(hoard.pot, 10);
        assert_eq!(prize, 150 * data.config.hatch_pot_multiplier);
    }

    #[test]
    fn the_hoard_does_not_hatch_below_capacity() {
        let data = GameData::load().unwrap();
        let mut hoard = HoardState::default();
        hoard.add_eggs(data.config.hoard_capacity as usize - 1, 5);

        assert!(hoard.take_hatch(&data.config).is_none());
    }
}
