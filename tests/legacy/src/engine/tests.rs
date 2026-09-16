use super::*;

#[test]
fn a_fixed_seed_reproduces_a_whole_spin() {
    let data = GameData::load().unwrap();
    let mut a = SeededRng::new(2024);
    let mut b = SeededRng::new(2024);

    let first = spin(&data, &mut a, 10, SpinMode::Base { ante: false });
    let second = spin(&data, &mut b, 10, SpinMode::Base { ante: false });

    assert_eq!(first.stops, second.stops);
    assert_eq!(first.grid, second.grid);
    assert_eq!(first.outcome, second.outcome);
}

#[test]
fn free_spins_expand_wilds_before_evaluating() {
    let data = GameData::load().unwrap();
    let wild = data.symbols.wild().unwrap();

    let mut rng = SeededRng::new(99);
    for _ in 0..400 {
        let result = spin(
            &data,
            &mut rng,
            10,
            SpinMode::FreeSpin {
                burned: 0,
                multiplier: 0,
            },
        );
        for reel in 0..result.grid.reel_count() {
            if result.grid.reel_contains(reel, wild) {
                assert!((0..result.grid.rows_on(reel)).all(|row| result.grid.at(reel, row) == wild));
            }
        }
    }
}
