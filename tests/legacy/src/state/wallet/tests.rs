use super::*;
use crate::data::GameData;

fn config() -> GameConfig {
    GameData::load().unwrap().config
}

/// The migration that must not lose money.
#[test]
fn a_wallet_absorbs_every_cabinet_and_adds_them_up() {
    let config = config();
    let wallet = Wallet::absorb(&config, &|id| match id {
        "dragon" => Some(1_400),
        "frost" => Some(250),
        "tidepool" => Some(90),
        _ => None,
    });
    assert_eq!(wallet.balance, 1_740);
}

/// A player who has never opened another cabinet still gets a stack.
#[test]
fn a_player_with_nothing_saved_starts_fresh() {
    let config = config();
    let wallet = Wallet::absorb(&config, &|_| None);
    assert_eq!(wallet.balance, config.starting_balance);
}

/// What travels with the player and what stays with the machine.
///
/// Stated as a property because the temptation, every time something new is
/// added to a session, is to reach for the wallet — and a hoard or a
/// progressive pot that followed the player between cabinets would be a
/// far worse bug than the one this fixes.
#[test]
fn the_wallet_carries_money_and_nothing_else() {
    let wallet = Wallet {
        balance: 900,
        staked: 200,
    };
    let round_tripped: Wallet =
        serde_json::from_str(&serde_json::to_string(&wallet).unwrap()).unwrap();
    assert_eq!(round_tripped, wallet);

    // The whole shape of it, as a check that nobody has added a field that
    // belongs to a cabinet.
    let value: serde_json::Value = serde_json::to_value(wallet).unwrap();
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["balance", "staked"],
        "the wallet has grown a field; a hoard or a jackpot that followed              the player between cabinets would be worse than the bug this fixed"
    );
}

/// §5.49's rule, for the wallet itself.
#[test]
fn a_wallet_written_before_staking_existed_still_loads() {
    let wallet: Wallet = serde_json::from_value(serde_json::json!({ "balance": 400 })).unwrap();
    assert_eq!(wallet.balance, 400);
    assert_eq!(wallet.staked, 0);
}

/// The specific failure this is written against: an empty save is a real
/// balance of zero, not "no save", and must not be read as a fresh stack.
#[test]
fn a_cabinet_saved_at_zero_is_absorbed_rather_than_replaced() {
    let config = config();
    let wallet = Wallet::absorb(&config, &|id| if id == "dragon" { Some(0) } else { None });
    assert_eq!(
        wallet.balance, 0,
        "a player who went broke and reloaded was handed a new stack"
    );
}
