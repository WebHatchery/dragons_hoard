//! The Feature Buy as it reaches a live session (GDD 5.13).

use super::*;
use crate::data::FeatureAward;
use crate::state::featurebuy::{self, BuyBlocked};

/// Index of the first tier that awards free spins, and of the first Wrath tier.
fn tiers(data: &GameData) -> (usize, usize) {
    let free = data
        .featurebuy
        .tiers
        .iter()
        .position(|tier| matches!(tier.award, FeatureAward::FreeSpins { .. }))
        .expect("no free-spins tier on the menu");
    let wrath = data
        .featurebuy
        .tiers
        .iter()
        .position(|tier| matches!(tier.award, FeatureAward::Wrath { .. }))
        .expect("no wrath tier on the menu");
    (free, wrath)
}

#[test]
fn buying_free_spins_starts_them_at_the_current_stake() {
    let data = data();
    let mut session = GameSession::new(&data, 5100);
    session.balance = 10_000_000;
    let (free, _) = tiers(&data);

    let line_bet = session.line_bet(&data);
    session.buy_feature(free, &data).unwrap();

    let state = session.free_spins.as_ref().expect("no free spins started");
    assert_eq!(
        state.line_bet, line_bet,
        "bought spins must run at the stake that paid for them"
    );
    assert!(state.remaining > 0);
    assert_eq!(state.total_won, 0);
}

#[test]
fn a_bought_wrath_opens_a_board_with_its_coins_already_locked() {
    let data = data();
    let mut session = GameSession::new(&data, 5101);
    session.balance = 10_000_000;
    let (_, wrath) = tiers(&data);

    let expected = match data.featurebuy.tiers[wrath].award {
        FeatureAward::Wrath { coins } => coins,
        FeatureAward::FreeSpins { .. } => unreachable!(),
    };
    session.buy_feature(wrath, &data).unwrap();

    let round = session.holdspin.as_ref().expect("no round opened");
    assert_eq!(round.coins(), expected);
    assert!(round.collected() > 0);
    assert!(!round.is_full(), "a bought board must leave room to respin");
}

#[test]
fn a_buy_with_too_little_credit_takes_nothing() {
    // The refusal that matters most: a partial charge would leave the player
    // poorer with nothing to show for it.
    let data = data();
    let mut session = GameSession::new(&data, 5102);
    let (free, _) = tiers(&data);
    let price = featurebuy::price(&data.featurebuy.tiers[free], session.total_bet(&data));
    session.balance = price - 1;

    assert_eq!(
        session.buy_feature(free, &data),
        Err(BuyBlocked::InsufficientBalance)
    );
    assert_eq!(session.balance, price - 1);
    assert!(session.free_spins.is_none());
}

#[test]
fn a_second_buy_during_a_feature_is_refused_without_charging() {
    let data = data();
    let mut session = GameSession::new(&data, 5103);
    session.balance = 10_000_000;
    let (free, _) = tiers(&data);

    session.buy_feature(free, &data).unwrap();
    let balance = session.balance;
    let remaining = session.free_spins.as_ref().unwrap().remaining;

    assert_eq!(
        session.buy_feature(free, &data),
        Err(BuyBlocked::FeatureActive)
    );
    assert_eq!(session.balance, balance);
    assert_eq!(
        session.free_spins.as_ref().unwrap().remaining,
        remaining,
        "the refused buy must not have added spins"
    );
}

#[test]
fn a_buy_is_refused_while_the_reels_are_turning() {
    let data = data();
    let mut session = GameSession::new(&data, 5104);
    session.balance = 10_000_000;
    let (free, _) = tiers(&data);

    session.begin_spin(&data).unwrap();
    let balance = session.balance;

    assert_eq!(session.buy_feature(free, &data), Err(BuyBlocked::Busy));
    assert_eq!(session.balance, balance);
}

#[test]
fn buying_a_tier_that_is_not_on_the_menu_is_refused() {
    let data = data();
    let mut session = GameSession::new(&data, 5105);
    session.balance = 10_000_000;

    let balance = session.balance;
    assert_eq!(
        session.buy_feature(data.featurebuy.tiers.len(), &data),
        Err(BuyBlocked::UnknownTier)
    );
    assert_eq!(session.balance, balance);
}

#[test]
fn bought_free_spins_still_cost_nothing_to_play() {
    // The buy was the paid event. If the spins it granted also took a stake the
    // player would be charged twice for one feature.
    let data = data();
    let mut session = GameSession::new(&data, 5106);
    session.balance = 10_000_000;
    let (free, _) = tiers(&data);

    let purchase = session.buy_feature(free, &data).unwrap();
    let after_buy = session.balance;
    let wagered = session.stats.total_wagered;

    let mut won = 0i64;
    while session.in_free_spins() {
        session.celebrations.clear();
        won += session.spin(&data).unwrap().total_credits();
    }

    assert_eq!(
        session.stats.total_wagered, wagered,
        "a free spin must not add to turnover"
    );
    assert_eq!(session.balance, after_buy + won);
    assert_eq!(session.stats.total_wagered, wagered);
    assert!(purchase.price > 0);
}

#[test]
fn the_price_counts_as_turnover_and_the_buy_is_counted() {
    let data = data();
    let mut session = GameSession::new(&data, 5107);
    session.balance = 10_000_000;
    let (free, _) = tiers(&data);

    let purchase = session.buy_feature(free, &data).unwrap();

    assert_eq!(session.stats.total_wagered, purchase.price);
    assert_eq!(session.stats.features_bought, 1);
}

#[test]
fn can_buy_agrees_with_what_buying_actually_does() {
    // The menu greys a row out using `can_buy`; if the two disagreed a player
    // could click a live-looking button and be refused, or be blocked from a
    // buy that would have gone through.
    let data = data();
    for (index, _) in data.featurebuy.tiers.iter().enumerate() {
        for balance in [0i64, 1_000, 100_000, 10_000_000] {
            let mut session = GameSession::new(&data, 5108 + index as u64);
            session.balance = balance;

            let predicted = session.can_buy(index, &data);
            let actual = session.buy_feature(index, &data).is_ok();
            assert_eq!(
                predicted, actual,
                "tier {} at balance {}: menu said {}, buy said {}",
                index, balance, predicted, actual
            );
        }
    }
}
