use super::*;

#[test]
fn embedded_data_loads() {
    let data = GameData::load().unwrap();

    assert_eq!(data.config.game_name, "dragons_hoard");
    assert_eq!(data.reels.len(), data.config.reel_count);
    assert_eq!(data.paylines.len(), 20);
    assert!(data.symbols.wild().is_some());
    assert!(data.symbols.scatter().is_some());
    assert!(data.symbols.hoard().is_some());
}

#[test]
fn paytable_resolves_by_run_length() {
    let data = GameData::load().unwrap();
    let chest = data.symbols.index_of("chest").unwrap();

    // Nothing pays below three, and longer runs always pay more.
    assert_eq!(data.symbols.pay(chest, 2), 0);
    assert!(data.symbols.pay(chest, 3) > 0);
    assert!(data.symbols.pay(chest, 4) > data.symbols.pay(chest, 3));
    assert!(data.symbols.pay(chest, 5) > data.symbols.pay(chest, 4));
    assert_eq!(data.symbols.pay(chest, MAX_RUN + 1), 0);
}

#[test]
fn the_lowest_symbols_only_pay_from_four() {
    let data = GameData::load().unwrap();

    for id in ["copper", "gold"] {
        let symbol = data.symbols.index_of(id).unwrap();
        assert_eq!(data.symbols.pay(symbol, 3), 0, "{} should not pay at 3", id);
        assert!(data.symbols.pay(symbol, 4) > 0);
    }
}

#[test]
fn every_reel_strip_carries_the_same_symbol_pool() {
    let data = GameData::load().unwrap();
    let scatter = data.symbols.scatter().unwrap();

    for (index, strip) in data.reels.iter().enumerate() {
        let scatters = strip.iter().filter(|symbol| **symbol == scatter).count();
        assert_eq!(
            scatters, 1,
            "reel {} should carry exactly one scatter, found {}",
            index, scatters
        );
    }
}

#[test]
fn total_bet_covers_every_payline() {
    let data = GameData::load().unwrap();
    assert_eq!(data.total_bet(10), 200);
}

#[test]
fn free_spin_awards_scale_with_scatters() {
    let data = GameData::load().unwrap();

    assert_eq!(data.freespins.award_for(2), 0);
    assert_eq!(data.freespins.award_for(3), 10);
    assert_eq!(data.freespins.award_for(5), 20);
}
