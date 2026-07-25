//! The Feature Buy — paying to skip straight to a feature (§5.13).
//!
//! # Why the price is data but not a free parameter
//!
//! A bought feature is only honest if it costs what it is worth. Price it below
//! the feature's expected value and buying beats spinning, so a player who never
//! touches the reels wins in the long run; price it far above and the menu is
//! decoration. The correct price is
//!
//! ```text
//! price = feature_expected_value / target_rtp
//! ```
//!
//! so a bought round returns the same fraction of what it cost as a paid spin
//! does. That is the whole design: **the buy menu must not be a better or worse
//! game than the reels**, only a faster one.
//!
//! The price lives in JSON rather than being computed, because measuring a
//! feature's EV means playing thousands of them and no one wants that at load
//! time. What keeps it honest is a test: `engine::sim` buys each tier tens of
//! thousands of times and asserts the measured return lands on the machine's
//! target. Edit `freespins.json` and the buy price test fails — which is exactly
//! the reminder a designer needs, and the same trick §5.6 uses to pin the
//! jackpot layer to its closed form.
//!
//! # A bought feature is a stake like any other
//!
//! The price is wagered, so it feeds the progressives (§5.6) and counts toward
//! turnover and achievements. Free spins awarded by a buy still cost nothing and
//! still cannot draw a jackpot — the *buy* was the paid event, not the spins it
//! granted. Getting this backwards would let a player farm the pots by buying
//! features rather than spinning.

use crate::data::{FeatureAward, FeatureBuyConfig, FeatureBuyTier, GameData};
use serde::{Deserialize, Serialize};

/// Why a buy could not go through. Mirrors `SpinBlocked` deliberately: the two
/// are the same class of refusal and the UI treats them alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuyBlocked {
    /// The reels are turning, a card is up, or a feature is already open.
    Busy,
    InsufficientBalance,
    /// A feature is already running — buying a second would either stack
    /// incoherently or silently discard what was paid for.
    FeatureActive,
    /// No tier at that index. Only reachable from a stale UI or a bad save.
    UnknownTier,
}

/// What a completed buy did, for the notification and the stats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyResult {
    pub tier_id: String,
    pub tier_name: String,
    pub price: i64,
}

/// Price of a tier at the given stake, in credits.
///
/// Multiples of *total* bet rather than line bet, so the menu reads the same at
/// every rung of the ladder and the price scales with the feature it buys — the
/// same bet-fairness argument as the jackpot odds (§5.6) and the hoard's
/// per-egg banking (§3).
pub fn price(tier: &FeatureBuyTier, total_bet: i64) -> i64 {
    total_bet * tier.price_multiple
}

/// The cheapest tier, for the button label on the main panel.
pub fn cheapest(config: &FeatureBuyConfig, total_bet: i64) -> Option<i64> {
    config.tiers.iter().map(|tier| price(tier, total_bet)).min()
}

/// Validate the shipped menu. Called from `GameData::validate`, so a machine
/// cannot ship a menu that is unbuyable or free.
pub fn validate(config: &FeatureBuyConfig, data_cells: usize) -> Result<(), String> {
    if config.tiers.is_empty() {
        return Err("featurebuy.json declared no tiers".to_owned());
    }
    if config.target_rtp_permille == 0 {
        return Err("a feature buy priced against a zero target RTP is free".to_owned());
    }

    let mut seen: Vec<&str> = Vec::new();
    for tier in &config.tiers {
        if tier.price_multiple <= 0 {
            return Err(format!("tier '{}' must cost something", tier.id));
        }
        if seen.contains(&tier.id.as_str()) {
            return Err(format!("duplicate feature buy tier '{}'", tier.id));
        }
        seen.push(&tier.id);

        match tier.award {
            FeatureAward::FreeSpins { spins: 0 } => {
                return Err(format!("tier '{}' would award no free spins", tier.id));
            }
            // A Wrath opened with every cell already locked has nothing left to
            // respin, and one opened with none is a different feature entirely.
            FeatureAward::Wrath { coins } if coins == 0 || coins >= data_cells => {
                return Err(format!(
                    "tier '{}' opens {} of {} cells — leaves nothing to respin",
                    tier.id, coins, data_cells
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// The grid cells a bought Wrath round starts on.
///
/// Spread across the board rather than clustered, because a bought round has no
/// spin behind it to say where the eggs fell. Stepping by a stride keeps the
/// opening board looking like something the reels could have produced.
pub fn opening_cells(coins: usize, cells: usize) -> Vec<usize> {
    if cells == 0 {
        return Vec::new();
    }
    let coins = coins.min(cells);
    let stride = cells / coins.max(1);
    (0..coins).map(|index| (index * stride) % cells).collect()
}

/// Tier at an index, or `None` if the menu has changed under a stale UI.
pub fn tier_at(data: &GameData, index: usize) -> Option<&FeatureBuyTier> {
    data.featurebuy.tiers.get(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> FeatureBuyConfig {
        GameData::load().unwrap().featurebuy
    }

    #[test]
    fn the_shipped_menu_validates() {
        let data = GameData::load().unwrap();
        let cells = data.config.reel_count * data.config.row_count;
        assert!(validate(&data.featurebuy, cells).is_ok());
    }

    #[test]
    fn a_free_tier_is_rejected() {
        // The one edit that would break the whole design silently: a tier that
        // costs nothing returns infinite RTP and the reels become pointless.
        let mut config = config();
        config.tiers[0].price_multiple = 0;
        assert!(validate(&config, 15).is_err());
    }

    #[test]
    fn a_wrath_tier_that_fills_the_board_is_rejected() {
        let mut config = config();
        for tier in &mut config.tiers {
            if let FeatureAward::Wrath { coins } = &mut tier.award {
                *coins = 15;
            }
        }
        assert!(validate(&config, 15).is_err());
    }

    #[test]
    fn duplicate_tier_ids_are_rejected() {
        let mut config = config();
        let first = config.tiers[0].clone();
        config.tiers.push(first);
        assert!(validate(&config, 15).is_err());
    }

    #[test]
    fn price_scales_with_the_stake() {
        // Bet-fairness: buying at a high stake must cost proportionally more,
        // or a player could buy cheap and collect at the top of the ladder.
        let config = config();
        let tier = &config.tiers[0];

        assert_eq!(price(tier, 400), price(tier, 200) * 2);
        assert_eq!(price(tier, 20), tier.price_multiple * 20);
    }

    #[test]
    fn the_cheapest_tier_is_the_one_the_button_advertises() {
        let config = config();
        let cheapest = cheapest(&config, 200).unwrap();
        assert!(config.tiers.iter().all(|tier| price(tier, 200) >= cheapest));
    }

    #[test]
    fn an_opening_board_spreads_its_coins_and_never_repeats_a_cell() {
        // Clustered coins would read as a bug, and a repeated index would open
        // the round with fewer coins than were paid for.
        for coins in 1..15 {
            let cells = opening_cells(coins, 15);
            assert_eq!(cells.len(), coins);

            let mut unique = cells.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), coins, "{} coins collided", coins);
            assert!(cells.iter().all(|cell| *cell < 15));
        }
    }

    #[test]
    fn asking_for_more_coins_than_cells_still_deals_a_board() {
        let cells = opening_cells(40, 15);
        assert_eq!(cells.len(), 15);
    }
}
