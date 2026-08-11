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
