//! Integration tests for the session: the spin lifecycle, features, autospin,
//! and the celebration cards they raise.

use super::*;
use crate::state::preferences::{Preferences, SpinSpeed};

fn data() -> GameData {
    GameData::load().unwrap()
}

/// Drive the phase machine at a fixed 60 Hz until it goes idle again,
/// collecting everything it emitted on the way. Cards are dismissed as they
/// appear, standing in for a player pressing on — without that the loop
/// would stall, because a showing card deliberately holds the reels.
fn run_to_idle(session: &mut GameSession, data: &GameData) -> Vec<SpinEvent> {
    let mut events = Vec::new();
    for _ in 0..2000 {
        if session.is_settled() {
            break;
        }
        events.extend(session.update_spin(data, 1.0 / 60.0));
        if session.celebrations.is_active() {
            session.celebrations.skip();
        }
    }
    assert!(session.is_settled(), "the spin never came to rest");
    events
}

fn settled(events: &[SpinEvent]) -> &SpinResolution {
    events
        .iter()
        .find_map(|event| match event {
            SpinEvent::Settled(resolution) => Some(resolution.as_ref()),
            _ => None,
        })
        .expect("the spin never settled")
}

#[test]
fn nothing_is_credited_until_the_reels_land() {
    let data = data();
    let mut session = GameSession::new(&data, 90210);
    let start = session.balance;
    let total_bet = session.total_bet(&data);

    session.begin_spin(&data).unwrap();

    // Mid-flight: the stake is gone, the win is not in yet.
    assert_eq!(session.balance, start - total_bet);
    assert_eq!(session.stats.total_spins, 0);
    assert_eq!(session.last_win, 0);

    let events = run_to_idle(&mut session, &data);
    let resolution = settled(&events);

    assert_eq!(session.stats.total_spins, 1);
    assert_eq!(
        session.balance,
        start - total_bet + resolution.total_credits()
    );
}

#[test]
fn the_animated_spin_produces_the_same_outcome_as_the_headless_one() {
    let data = data();
    let mut animated = GameSession::new(&data, 555);
    let mut headless = GameSession::new(&data, 555);

    for _ in 0..12 {
        if headless.spin(&data).is_err() {
            break;
        }
        animated.begin_spin(&data).unwrap();
        run_to_idle(&mut animated, &data);
    }

    // The animation must not consume a single RNG draw of its own.
    assert_eq!(animated.grid, headless.grid);
    assert_eq!(animated.balance, headless.balance);
    assert_eq!(animated.hoard.count, headless.hoard.count);
    assert_eq!(animated.stats.total_spins, headless.stats.total_spins);
}

#[test]
fn reels_report_stopping_left_to_right_before_settling() {
    let data = data();
    let mut session = GameSession::new(&data, 8);
    session.begin_spin(&data).unwrap();

    let events = run_to_idle(&mut session, &data);
    let mut stopped = Vec::new();
    let mut last_stop_at = 0;
    let mut settled_at = None;
    for (index, event) in events.iter().enumerate() {
        match event {
            SpinEvent::ReelStopped(reel) => {
                stopped.push(*reel);
                last_stop_at = index;
            }
            SpinEvent::Settled(_) => settled_at = Some(index),
            _ => {}
        }
    }

    assert_eq!(stopped, (0..data.config.reel_count).collect::<Vec<_>>());
    assert!(
        settled_at.expect("the spin never settled") > last_stop_at,
        "the outcome was applied before the last reel stopped"
    );
}

#[test]
fn the_reels_come_to_rest_on_the_grid_that_was_evaluated() {
    let data = data();
    let mut session = GameSession::new(&data, 4711);
    session.begin_spin(&data).unwrap();
    let events = run_to_idle(&mut session, &data);

    let stops = settled(&events).result.stops.clone();
    assert_eq!(session.reel_stops, stops);
    assert_eq!(session.grid, engine::grid_from_stops(&data, &stops));
}

#[test]
fn free_spins_chain_themselves_without_another_stake() {
    let data = data();
    let mut session = GameSession::new(&data, 3);
    session.free_spins = Some(FreeSpinState {
        remaining: 3,
        awarded: 3,
        line_bet: 1,
        total_won: 0,
    });
    let wagered = session.stats.total_wagered;

    session.begin_spin(&data).unwrap();
    let events = run_to_idle(&mut session, &data);

    assert!(
        events
            .iter()
            .any(|event| matches!(event, SpinEvent::AutoSpinReady)),
        "the feature owed more spins but never asked for one"
    );
    assert_eq!(session.stats.total_wagered, wagered);
}

#[test]
fn the_last_free_spin_does_not_ask_for_another() {
    let data = data();
    let mut session = GameSession::new(&data, 21);

    // Spin the feature down to nothing, however long retriggers make it.
    session.free_spins = Some(FreeSpinState {
        remaining: 1,
        awarded: 1,
        line_bet: 1,
        total_won: 0,
    });
    let mut events = Vec::new();
    while session.in_free_spins() {
        session.begin_spin(&data).unwrap();
        events = run_to_idle(&mut session, &data);
    }

    assert!(!events
        .iter()
        .any(|event| matches!(event, SpinEvent::AutoSpinReady)));
    assert!(session.phase.is_idle());
}

#[test]
fn a_showing_card_holds_the_reels() {
    let data = data();
    let mut session = GameSession::new(&data, 17);
    session.begin_spin(&data).unwrap();
    session
        .celebrations
        .push(CelebrationKind::BigWin { credits: 500 });

    // One second: longer than reel 1 takes to land, shorter than the card.
    for _ in 0..60 {
        session.update_spin(&data, 1.0 / 60.0);
    }

    assert!(
        session.celebrations.is_active(),
        "the card should still be up"
    );
    assert!(session.phase.is_busy(), "the reels advanced under the card");
    assert_eq!(session.stats.total_spins, 0);

    session.celebrations.skip();
    run_to_idle(&mut session, &data);
    assert_eq!(session.stats.total_spins, 1);
}

#[test]
fn a_spin_is_refused_while_a_card_is_showing() {
    let data = data();
    let mut session = GameSession::new(&data, 17);
    session
        .celebrations
        .push(CelebrationKind::BigWin { credits: 500 });
    let balance = session.balance;

    assert!(!session.can_spin(&data));
    assert_eq!(session.begin_spin(&data).err(), Some(SpinBlocked::Busy));
    assert_eq!(session.balance, balance);
}

#[test]
fn ending_the_feature_raises_a_summary_card_with_its_total() {
    let data = data();
    let mut session = GameSession::new(&data, 91);
    session.free_spins = Some(FreeSpinState {
        remaining: 1,
        awarded: 4,
        line_bet: 1,
        total_won: 0,
    });

    while session.in_free_spins() {
        session.celebrations.clear();
        session.spin(&data).unwrap();
    }

    match session.celebrations.active().map(|card| card.kind()) {
        Some(CelebrationKind::FreeSpinsSummary { spins, won }) => {
            assert!(*spins >= 4, "the card should report every spin awarded");
            assert!(*won >= 0);
        }
        other => panic!("expected a free-spins summary card, got {:?}", other),
    }
}

#[test]
fn autospin_never_survives_a_moment_worth_watching() {
    let data = data();
    let mut session = GameSession::new(&data, 606);
    let mut features = 0;

    for _ in 0..3000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        if session.autospin.is_none() && !session.in_free_spins() {
            session.start_autospin(50);
        }

        let resolution = session.spin(&data).unwrap();
        let notable = resolution.outcome().free_spins_awarded > 0
            || resolution.hatch_credits > 0
            || resolution.total_credits() >= session.big_win_threshold(&data);

        if notable && !resolution.was_free_spin {
            features += 1;
            assert!(
                session.autospin.is_none(),
                "autospin ran straight past something the player wanted to see"
            );
        }
    }

    assert!(features > 0, "the sample never hit a notable spin");
}

#[test]
fn autospin_spends_exactly_the_spins_it_was_given() {
    let data = data();
    let mut session = GameSession::new(&data, 2024);
    assert!(session.start_autospin(6));

    let mut spins = 0;
    while session.autospin.is_some() {
        session.balance = 1_000_000;
        session.celebrations.clear();
        session.spin(&data).unwrap();
        spins += 1;
        assert!(spins <= 6, "the run outlived its budget");
    }

    // It either ran the full six or stopped early on something notable.
    assert!(spins <= 6);
}

#[test]
fn autospin_cannot_start_on_top_of_a_committed_stake() {
    let data = data();
    let mut session = GameSession::new(&data, 5);
    session.begin_spin(&data).unwrap();

    assert!(!session.start_autospin(10));
    assert_eq!(session.autospin_remaining(), 0);
}

#[test]
fn a_free_spin_does_not_cost_the_autospin_run_a_spin() {
    let data = data();
    let mut session = GameSession::new(&data, 44);
    session.start_autospin(10);
    session.free_spins = Some(FreeSpinState {
        remaining: 3,
        awarded: 3,
        line_bet: 1,
        total_won: 0,
    });
    let before = session.autospin_remaining();

    session.spin(&data).unwrap();

    assert_eq!(
        session.autospin_remaining(),
        before,
        "a free spin was billed to the autospin budget"
    );
}

#[test]
fn a_spin_debits_the_total_bet() {
    let data = data();
    let mut session = GameSession::new(&data, 1);
    let start = session.balance;
    let total_bet = session.total_bet(&data);

    let resolution = session.spin(&data).unwrap();

    assert!(!resolution.was_free_spin);
    assert_eq!(session.stats.total_wagered, total_bet);
    assert_eq!(
        session.balance,
        start - total_bet + resolution.total_credits()
    );
}

#[test]
fn a_spin_is_refused_when_the_balance_is_short() {
    let data = data();
    let mut session = GameSession::new(&data, 1);
    session.balance = session.total_bet(&data) - 1;

    assert!(!session.can_spin(&data));
    assert_eq!(
        session.spin(&data).err(),
        Some(SpinBlocked::InsufficientBalance)
    );
    assert_eq!(session.stats.total_spins, 0);
}

#[test]
fn free_spins_cost_nothing_and_count_down() {
    let data = data();
    let mut session = GameSession::new(&data, 1);
    session.free_spins = Some(FreeSpinState {
        remaining: 2,
        awarded: 2,
        line_bet: 10,
        total_won: 0,
    });
    let start = session.balance;

    let resolution = session.spin(&data).unwrap();

    assert!(resolution.was_free_spin);
    assert_eq!(session.balance, start + resolution.total_credits());
    assert_eq!(session.stats.total_wagered, 0);
}

#[test]
fn the_feature_retires_once_its_last_spin_resolves() {
    let data = data();
    let mut session = GameSession::new(&data, 4);
    session.free_spins = Some(FreeSpinState {
        remaining: 1,
        awarded: 1,
        line_bet: 1,
        total_won: 0,
    });

    // A retrigger on the final spin would keep it alive, so spin until the
    // feature actually runs dry.
    while session.in_free_spins() {
        session.spin(&data).unwrap();
    }

    assert!(!session.in_free_spins());
    // Ending the feature raises its summary card.
    assert!(session.celebrations.is_active());
}

#[test]
fn bets_are_locked_during_free_spins() {
    let data = data();
    let mut session = GameSession::new(&data, 1);
    session.line_bet_index = 0;
    session.free_spins = Some(FreeSpinState {
        remaining: 3,
        awarded: 3,
        line_bet: 1,
        total_won: 0,
    });

    assert!(!session.adjust_bet(&data, 1));
    assert!(!session.set_max_bet(&data));
    assert_eq!(session.line_bet_index, 0);
}

#[test]
fn bets_clamp_to_the_configured_ladder() {
    let data = data();
    let mut session = GameSession::new(&data, 1);
    let last = data.config.line_bets.len() - 1;

    session.line_bet_index = 0;
    assert!(!session.adjust_bet(&data, -1));
    assert_eq!(session.line_bet_index, 0);

    assert!(session.set_max_bet(&data));
    assert_eq!(session.line_bet_index, last);
    assert!(!session.adjust_bet(&data, 1));
}

#[test]
fn a_save_round_trips_the_session() {
    let data = data();
    let mut session = GameSession::new(&data, 77);
    for _ in 0..25 {
        if session.spin(&data).is_err() {
            break;
        }
    }

    let save = session.to_save(&data.config.version);
    let json = serde_json::to_value(&save).unwrap();
    let restored = migrate_save_value(Some("1.0.0".to_owned()), json, &data).unwrap();
    let reloaded = GameSession::from_save(&data, restored);

    assert_eq!(reloaded.balance, session.balance);
    assert_eq!(reloaded.line_bet_index, session.line_bet_index);
    assert_eq!(reloaded.hoard.count, session.hoard.count);
    assert_eq!(reloaded.hoard.pot, session.hoard.pot);
    assert_eq!(reloaded.stats.total_spins, session.stats.total_spins);
}

#[test]
fn a_reloaded_session_keeps_producing_the_same_spins() {
    let data = data();
    let mut session = GameSession::new(&data, 4242);
    session.spin(&data).unwrap();

    let save = session.to_save(&data.config.version);
    let mut reloaded = GameSession::from_save(&data, save);

    let a = session.spin(&data).unwrap();
    let b = reloaded.spin(&data).unwrap();
    assert_eq!(a.result.grid, b.result.grid);
}

#[test]
fn a_legacy_save_migrates_to_the_current_shape() {
    let data = data();
    let value = serde_json::json!({
        "points": 640,
        "line_bet_index": 99,
        "hoard_count": 6,
        "hoard_pot": 60
    });

    let migrated = migrate_save_value(Some("0.1.0".to_owned()), value, &data).unwrap();

    assert_eq!(migrated.version, "1.0.0");
    assert_eq!(migrated.balance, 640);
    assert_eq!(migrated.line_bet_index, data.config.line_bets.len() - 1);
    assert_eq!(migrated.hoard.count, 6);
}

// --- Progressive jackpots -------------------------------------------------

/// Drive a session until a jackpot lands, topping the balance up so a losing
/// streak cannot stall it. Returns the winning resolution.
fn spin_until_jackpot(session: &mut GameSession, data: &GameData) -> SpinResolution {
    for _ in 0..200_000 {
        session.balance = 1_000_000_000;
        session.celebrations.clear();
        let resolution = session.spin(data).unwrap();
        if resolution.jackpot.is_some() {
            return resolution;
        }
    }
    panic!("no jackpot landed in 200,000 spins");
}

#[test]
fn every_paid_spin_grows_the_pots() {
    let data = data();
    let mut session = GameSession::new(&data, 11);
    let before = session.jackpots.value(&data.jackpots, 0);

    for _ in 0..2_000 {
        session.balance = 1_000_000;
        session.spin(&data).unwrap();
    }

    assert!(session.jackpots.value(&data.jackpots, 0) > before);
}

#[test]
fn a_free_spin_neither_feeds_nor_draws_a_jackpot() {
    // A free spin staked nothing, so it must not top the pots up and must not
    // be allowed to win them.
    let data = data();
    let mut session = GameSession::new(&data, 12);
    session.free_spins = Some(FreeSpinState {
        remaining: 20,
        awarded: 20,
        line_bet: 25,
        total_won: 0,
    });
    let before = session.jackpots.value(&data.jackpots, 0);

    while session.in_free_spins() {
        let resolution = session.spin(&data).unwrap();
        assert!(resolution.jackpot.is_none(), "a free spin won a jackpot");
    }

    assert_eq!(session.jackpots.value(&data.jackpots, 0), before);
}

#[test]
fn winning_a_jackpot_credits_it_and_resets_that_tier() {
    let data = data();
    let mut session = GameSession::new(&data, 909);
    let resolution = spin_until_jackpot(&mut session, &data);
    let win = resolution.jackpot.clone().unwrap();

    assert!(win.credits >= data.jackpots.tiers[win.tier].seed);
    assert_eq!(resolution.jackpot_credits(), win.credits);
    assert!(resolution.total_credits() >= win.credits);
    assert_eq!(
        session.jackpots.value(&data.jackpots, win.tier),
        data.jackpots.tiers[win.tier].seed,
        "the won tier should be back at its seed"
    );
    assert_eq!(session.stats.jackpots, 1);
}

#[test]
fn a_jackpot_raises_its_own_card_and_suppresses_the_big_win_one() {
    let data = data();
    let mut session = GameSession::new(&data, 707);
    spin_until_jackpot(&mut session, &data);

    let mut kinds = Vec::new();
    while let Some(kind) = session
        .celebrations
        .active()
        .map(|card| card.kind().clone())
    {
        kinds.push(kind);
        session.celebrations.skip();
    }

    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, CelebrationKind::Jackpot { .. })),
        "no jackpot card was raised, saw {:?}",
        kinds
    );
    assert!(
        !kinds
            .iter()
            .any(|kind| matches!(kind, CelebrationKind::BigWin { .. })),
        "a big-win card doubled up on the jackpot"
    );
}

#[test]
fn autospin_stops_on_a_jackpot() {
    let data = data();
    let mut session = GameSession::new(&data, 4321);

    for _ in 0..200_000 {
        session.balance = 1_000_000_000;
        session.celebrations.clear();
        if session.autospin.is_none() && !session.in_free_spins() {
            session.start_autospin(100_000);
        }
        if session.spin(&data).unwrap().jackpot.is_some() {
            assert!(
                session.autospin.is_none(),
                "autospin ran straight past a jackpot"
            );
            return;
        }
    }
    panic!("no jackpot landed in 200,000 spins");
}

#[test]
fn the_pots_survive_a_save_and_reload() {
    let data = data();
    let mut session = GameSession::new(&data, 88);
    for _ in 0..5_000 {
        session.balance = 1_000_000;
        session.spin(&data).unwrap();
    }
    let banked: Vec<i64> = (0..data.jackpots.tiers.len())
        .map(|i| session.jackpots.value(&data.jackpots, i))
        .collect();

    let reloaded = GameSession::from_save(&data, session.to_save(&data.config.version));

    for (index, expected) in banked.iter().enumerate() {
        assert_eq!(
            reloaded.jackpots.value(&data.jackpots, index),
            *expected,
            "tier {} lost its accrual across a reload",
            index
        );
    }
}

// --- Preferences ----------------------------------------------------------

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
