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
