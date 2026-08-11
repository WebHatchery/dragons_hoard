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
