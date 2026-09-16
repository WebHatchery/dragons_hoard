//! Player preferences as they affect a real spin (GDD 5.7).

use super::*;

#[test]
fn spin_speed_changes_how_long_a_spin_takes_but_not_what_it_pays() {
    // The whole promise of Turbo: the same seed produces the same spin, just
    // revealed sooner. If speed touched the outcome it would be a maths change
    // hiding in a settings menu.
    let data = data();
    let mut normal = GameSession::new(&data, 5150);
    let mut turbo = GameSession::new(&data, 5150);
    turbo.preferences.spin_speed = SpinSpeed::Turbo;

    let mut normal_frames = 0;
    let mut turbo_frames = 0;
    for _ in 0..6 {
        normal.begin_spin(&data).unwrap();
        normal_frames += count_frames_to_idle(&mut normal, &data);

        turbo.begin_spin(&data).unwrap();
        turbo_frames += count_frames_to_idle(&mut turbo, &data);
    }

    assert_eq!(normal.grid, turbo.grid, "turbo landed somewhere else");
    assert_eq!(normal.balance, turbo.balance, "turbo paid differently");
    assert_eq!(normal.stats.total_won, turbo.stats.total_won);
    assert!(
        turbo_frames < normal_frames,
        "turbo took {} frames vs normal {}",
        turbo_frames,
        normal_frames
    );
}

/// Run the phase machine to rest, returning how many 60 Hz frames it took.
fn count_frames_to_idle(session: &mut GameSession, data: &GameData) -> u32 {
    let mut frames = 0;
    for _ in 0..2000 {
        if session.is_settled() {
            break;
        }
        session.update_spin(data, 1.0 / 60.0);
        if session.celebrations.is_active() {
            session.celebrations.skip();
        }
        frames += 1;
    }
    frames
}

#[test]
fn every_spin_speed_still_stops_all_five_reels_in_order() {
    // Turbo shortens the durations; it must not compress them so far that two
    // reels land in one frame and an event is lost.
    let data = data();

    for speed in [SpinSpeed::Normal, SpinSpeed::Fast, SpinSpeed::Turbo] {
        let mut session = GameSession::new(&data, 31);
        session.preferences.spin_speed = speed;
        session.begin_spin(&data).unwrap();

        let stopped: Vec<usize> = run_to_idle(&mut session, &data)
            .iter()
            .filter_map(|event| match event {
                SpinEvent::ReelStopped(reel) => Some(*reel),
                _ => None,
            })
            .collect();

        assert_eq!(
            stopped,
            (0..data.config.reel_count).collect::<Vec<_>>(),
            "{:?} lost a reel stop",
            speed
        );
    }
}

#[test]
fn preferences_are_not_carried_in_the_save() {
    // Volume and spin speed belong to the player, not to a save slot.
    let data = data();
    let mut session = GameSession::new(&data, 2);
    session.preferences.spin_speed = SpinSpeed::Turbo;
    session.preferences.shared.master_volume = 0.2;

    let reloaded = GameSession::from_save(&data, session.to_save(&data.config.version));

    assert_eq!(
        reloaded.preferences,
        Preferences::with_defaults(&data.config)
    );
}
