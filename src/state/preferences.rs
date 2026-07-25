//! Player preferences: volume, motion, spin speed, autospin length.
//!
//! The shared half is the toolkit's [`GameSettings`] (volumes, screen shake, UI
//! scale) rather than a private reimplementation — every other game in the
//! workspace already stores those under the same shape. Only the genuinely
//! slot-specific parts live here: how fast the reels turn, and how long an
//! unattended run is.
//!
//! Preferences are **not part of the save slot**. A player who starts a new game
//! keeps their volume; someone who deletes their save keeps it too. They persist
//! under their own key.

use crate::data::GameConfig;
use macroquad_toolkit::persistence::{load_json_key, save_json_key};
use macroquad_toolkit::settings::GameSettings;
use serde::{Deserialize, Serialize};

/// Storage key, kept distinct from the toolkit's own `settings` key so the two
/// blobs can never overwrite each other.
const PREFERENCES_KEY: &str = "preferences";

/// How fast the reels turn. Scales every duration in `state::spin`, so a Turbo
/// spin is the same spin — same stops, same outcome — just revealed sooner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpinSpeed {
    Normal,
    Fast,
    Turbo,
}

impl SpinSpeed {
    /// Multiplier applied to every reel and payout duration. Smaller is faster.
    pub fn time_scale(self) -> f32 {
        match self {
            SpinSpeed::Normal => 1.0,
            SpinSpeed::Fast => 0.6,
            SpinSpeed::Turbo => 0.32,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SpinSpeed::Normal => "Normal",
            SpinSpeed::Fast => "Fast",
            SpinSpeed::Turbo => "Turbo",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SpinSpeed::Normal => SpinSpeed::Fast,
            SpinSpeed::Fast => SpinSpeed::Turbo,
            SpinSpeed::Turbo => SpinSpeed::Normal,
        }
    }
}

/// The text sizes on offer.
///
/// A short list rather than a slider, and every one of them is a size the panels
/// have actually been measured at (§5.37). A continuous control would let a
/// player pick a size nobody ever laid the game out for.
pub const TEXT_SCALES: [f32; 3] = [1.0, 1.15, 1.3];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Volumes, screen shake, UI scale — the toolkit's shared model.
    pub shared: GameSettings,
    pub spin_speed: SpinSpeed,
    /// Index into `GameConfig::autospin_choices`. Stored as an index rather than
    /// a count so editing the choices in JSON cannot strand a saved preference
    /// on a value that is no longer offered.
    pub autospin_choice: usize,
    /// Which cabinet to boot into. Empty means "whatever is first in the
    /// catalog"; an id that no longer exists falls back the same way, so
    /// removing a machine cannot strand a player outside the game.
    pub machine_id: String,
    /// Particle bursts. Paired with `shared.screen_shake` as a "reduced motion"
    /// pair; separate flags because shake is the one that causes trouble for
    /// motion-sensitive players, and some want to keep the sparkle.
    pub particles: bool,
    /// Session caps and the reality-check interval (§5.30). Kept here rather
    /// than in the save so they survive a New Game — a limit that a fresh
    /// bankroll cleared would be no limit at all.
    #[serde(default)]
    pub limits: crate::state::limits::Limits,
    #[serde(default)]
    pub reality_check_minutes: Option<u32>,
    /// Text size, as a percentage of the design size (§5.38). An index into
    /// [`TEXT_SCALES`] rather than a raw float, so a saved value can never be a
    /// size the game was never laid out for.
    #[serde(default)]
    pub text_scale: usize,
}

impl Default for Preferences {
    /// `autospin_choice` starts deliberately out of range: the real default
    /// lives in `game_config.json`, and [`sanitize`](Self::sanitize) is what
    /// resolves it. Build preferences through
    /// [`with_defaults`](Self::with_defaults) rather than this, unless you
    /// genuinely want the unresolved value.
    fn default() -> Self {
        Self {
            shared: GameSettings::default(),
            spin_speed: SpinSpeed::Normal,
            autospin_choice: usize::MAX,
            machine_id: String::new(),
            particles: true,
            limits: crate::state::limits::Limits::default(),
            reality_check_minutes: None,
            text_scale: 0,
        }
    }
}

impl Preferences {
    /// Defaults resolved against the config. This is what a session starts with
    /// when nothing has been saved.
    pub fn with_defaults(config: &GameConfig) -> Self {
        let mut prefs = Self::default();
        prefs.sanitize(config);
        prefs
    }

    /// Load, falling back to defaults, then clamp against the current config.
    pub fn load(config: &GameConfig) -> Self {
        let mut prefs: Self = load_json_key(&config.game_name, PREFERENCES_KEY).unwrap_or_default();
        prefs.sanitize(config);
        prefs
    }

    pub fn save(&self, config: &GameConfig) -> Result<(), String> {
        save_json_key(&config.game_name, PREFERENCES_KEY, self)
    }

    /// Clamp anything a hand-edited file — or an edit to the JSON choices —
    /// could put out of range.
    pub fn sanitize(&mut self, config: &GameConfig) {
        self.shared.sanitize();
        if self.autospin_choice >= config.autospin_choices.len() {
            self.autospin_choice = config
                .default_autospin_choice
                .min(config.autospin_choices.len().saturating_sub(1));
        }
    }

    pub fn sfx_volume(&self) -> f32 {
        self.shared.effective_sfx_volume()
    }

    /// Music level (§5.31). Separate from the effects because the two want
    /// different answers: the music plays constantly and the effects do not, so
    /// a player who wants one quiet rarely wants the other quiet too.
    pub fn text_scale(&self) -> f32 {
        TEXT_SCALES
            .get(self.text_scale)
            .copied()
            .unwrap_or(TEXT_SCALES[0])
    }

    pub fn cycle_text_scale(&mut self) {
        self.text_scale = (self.text_scale + 1) % TEXT_SCALES.len();
    }

    pub fn music_volume(&self) -> f32 {
        self.shared.effective_music_volume()
    }

    pub fn adjust_music_volume(&mut self, delta: f32) {
        self.shared.music_volume = (self.shared.music_volume + delta).clamp(0.0, 1.0);
    }

    pub fn autospin_spins(&self, config: &GameConfig) -> u32 {
        config
            .autospin_choices
            .get(self.autospin_choice)
            .or_else(|| config.autospin_choices.first())
            .copied()
            .unwrap_or(1)
    }

    pub fn time_scale(&self) -> f32 {
        self.spin_speed.time_scale()
    }

    /// Step the master volume by `delta`, snapped to tenths so the readout is
    /// always a round percentage.
    pub fn adjust_volume(&mut self, delta: f32) {
        let stepped = (self.shared.master_volume + delta).clamp(0.0, 1.0);
        self.shared.master_volume = (stepped * 10.0).round() / 10.0;
    }

    pub fn cycle_spin_speed(&mut self) {
        self.spin_speed = self.spin_speed.next();
    }

    pub fn cycle_autospin(&mut self, config: &GameConfig) {
        let count = config.autospin_choices.len().max(1);
        self.autospin_choice = (self.autospin_choice.wrapping_add(1)) % count;
    }

    pub fn toggle_shake(&mut self) {
        self.shared.screen_shake = !self.shared.screen_shake;
    }

    pub fn toggle_particles(&mut self) {
        self.particles = !self.particles;
    }
}

#[cfg(test)]
mod tests {
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
}
#[cfg(test)]
mod text_scale_tests {
    use super::*;

    #[test]
    fn the_smallest_offered_size_is_the_design_size() {
        // Every panel was laid out at 1.0, so it has to be the default and the
        // floor. A player who has never touched the setting must see exactly
        // what the layout was drawn for.
        assert_eq!(TEXT_SCALES[0], 1.0);
        assert_eq!(Preferences::default().text_scale(), 1.0);
    }

    #[test]
    fn the_sizes_only_go_up() {
        // Down is the toolkit's business, not a setting: text smaller than the
        // design size fails the legibility floor §5.25 measures art against.
        for pair in TEXT_SCALES.windows(2) {
            assert!(pair[1] > pair[0], "{:?}", TEXT_SCALES);
        }
        assert!(TEXT_SCALES.iter().all(|scale| *scale >= 1.0));
    }

    #[test]
    fn cycling_reaches_every_size_and_comes_back() {
        let mut prefs = Preferences::default();
        let mut seen = Vec::new();
        for _ in 0..TEXT_SCALES.len() {
            seen.push(prefs.text_scale());
            prefs.cycle_text_scale();
        }
        for scale in TEXT_SCALES {
            assert!(seen.contains(&scale), "{} unreachable", scale);
        }
        assert_eq!(prefs.text_scale(), TEXT_SCALES[0]);
    }

    #[test]
    fn a_saved_index_past_the_end_falls_back_rather_than_panicking() {
        // The list can shrink under a saved preference; an out-of-range index
        // must not take the game down on the frame it loads.
        let prefs = Preferences {
            text_scale: 999,
            ..Preferences::default()
        };
        assert_eq!(prefs.text_scale(), TEXT_SCALES[0]);
    }

    #[test]
    fn no_offered_size_is_larger_than_the_panels_were_measured_at() {
        // The layout audit (§5.37) is run at every value in this list before it
        // ships. Widening the list without re-running it is the mistake this
        // note exists to make loud.
        assert!(TEXT_SCALES.iter().all(|scale| *scale <= 1.3));
    }
}
