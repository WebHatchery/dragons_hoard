use super::*;
use crate::data::GameData;

fn config() -> GameConfig {
    GameData::load().unwrap().config
}

fn prefs(config: &GameConfig) -> Preferences {
    Preferences::with_defaults(config)
}

#[test]
fn defaults_are_playable() {
    let config = config();
    let prefs = prefs(&config);

    assert_eq!(prefs.spin_speed, SpinSpeed::Normal);
    assert_eq!(prefs.time_scale(), 1.0);
    assert_eq!(
        prefs.autospin_spins(&config),
        config.autospin_choices[config.default_autospin_choice]
    );
    assert!(prefs.particles);
    assert!(prefs.shared.screen_shake);
    assert!(prefs.sfx_volume() > 0.0);
}

#[test]
fn spin_speed_cycles_through_every_setting_and_wraps() {
    let mut prefs = Preferences::default();
    let mut seen = vec![prefs.spin_speed];

    for _ in 0..2 {
        prefs.cycle_spin_speed();
        seen.push(prefs.spin_speed);
    }

    assert_eq!(
        seen,
        vec![SpinSpeed::Normal, SpinSpeed::Fast, SpinSpeed::Turbo]
    );
    // Faster settings must actually be faster...
    assert!(SpinSpeed::Fast.time_scale() < SpinSpeed::Normal.time_scale());
    assert!(SpinSpeed::Turbo.time_scale() < SpinSpeed::Fast.time_scale());
    // ...and never zero, or the reels would land in the frame they start
    // and skip every ReelStopped event.
    assert!(SpinSpeed::Turbo.time_scale() > 0.0);

    prefs.cycle_spin_speed();
    assert_eq!(prefs.spin_speed, SpinSpeed::Normal, "the cycle wraps");
}

#[test]
fn autospin_cycles_through_every_choice_and_wraps() {
    let config = config();
    let mut prefs = prefs(&config);
    let started_on = prefs.autospin_spins(&config);
    let mut seen = Vec::new();

    for _ in 0..config.autospin_choices.len() {
        seen.push(prefs.autospin_spins(&config));
        prefs.cycle_autospin(&config);
    }
    seen.sort_unstable();

    let mut expected = config.autospin_choices.clone();
    expected.sort_unstable();
    assert_eq!(seen, expected, "the cycle must reach every choice");
    assert_eq!(
        prefs.autospin_spins(&config),
        started_on,
        "back where it started"
    );
}

#[test]
fn volume_steps_in_tenths_and_clamps() {
    let mut prefs = Preferences::default();
    prefs.shared.master_volume = 0.5;

    prefs.adjust_volume(0.1);
    assert!((prefs.shared.master_volume - 0.6).abs() < 1e-6);

    for _ in 0..20 {
        prefs.adjust_volume(0.1);
    }
    assert_eq!(prefs.shared.master_volume, 1.0, "clamped at the top");

    for _ in 0..20 {
        prefs.adjust_volume(-0.1);
    }
    assert_eq!(prefs.shared.master_volume, 0.0, "clamped at the bottom");
}

#[test]
fn silence_is_reachable() {
    // There must be a way to turn the sound off outright — the effects are
    // synthesised and have never been listened to.
    let mut prefs = Preferences::default();
    for _ in 0..20 {
        prefs.adjust_volume(-0.1);
    }

    assert_eq!(prefs.sfx_volume(), 0.0);
}

#[test]
fn motion_toggles_are_independent() {
    let mut prefs = Preferences::default();

    prefs.toggle_shake();
    assert!(!prefs.shared.screen_shake);
    assert!(prefs.particles, "turning shake off kept the sparkle");

    prefs.toggle_particles();
    assert!(!prefs.particles);
}

#[test]
fn a_raw_default_is_not_yet_resolved() {
    // Guards the bug this constructor exists to prevent: a session built
    // with `Preferences::default()` reported the *first* autospin choice
    // rather than the configured default, so the settings panel and the
    // Auto button both showed 10 instead of 25.
    let config = config();

    assert_ne!(
        Preferences::default().autospin_choice,
        Preferences::with_defaults(&config).autospin_choice
    );
    assert_eq!(
        Preferences::with_defaults(&config).autospin_spins(&config),
        config.autospin_choices[config.default_autospin_choice]
    );
}

#[test]
fn a_choice_that_no_longer_exists_falls_back_to_the_configured_default() {
    // Trimming `autospin_choices` in the JSON must not strand a saved
    // preference on an index that is gone.
    let config = config();
    let mut prefs = Preferences {
        autospin_choice: 99,
        ..Default::default()
    };
    prefs.shared.master_volume = 12.0;
    prefs.sanitize(&config);

    assert_eq!(prefs.shared.master_volume, 1.0);
    assert!(prefs.autospin_choice < config.autospin_choices.len());
    assert!(config
        .autospin_choices
        .contains(&prefs.autospin_spins(&config)));
}

#[test]
fn preferences_round_trip_through_json() {
    let config = config();
    let mut prefs = prefs(&config);
    prefs.spin_speed = SpinSpeed::Turbo;
    prefs.autospin_choice = config.autospin_choices.len() - 1;
    prefs.particles = false;
    prefs.shared.master_volume = 0.3;

    let json = serde_json::to_string(&prefs).unwrap();
    let restored: Preferences = serde_json::from_str(&json).unwrap();

    assert_eq!(restored, prefs);
}

#[test]
fn a_partial_file_fills_in_the_rest() {
    // `serde(default)` throughout means a preferences file written by an
    // older build — or hand-trimmed — still loads.
    let config = config();
    let mut restored: Preferences = serde_json::from_str(r#"{"spin_speed":"Fast"}"#).unwrap();
    restored.sanitize(&config);

    assert_eq!(restored.spin_speed, SpinSpeed::Fast);
    assert_eq!(
        restored.autospin_spins(&config),
        config.autospin_choices[config.default_autospin_choice]
    );
    assert!(restored.particles);
}
