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
use macroquad_toolkit::persistence::{json_key_exists, load_json_key, save_json_key};
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
    /// The ante side bet (§5.75). A preference rather than save state, like
    /// the bet step it modifies — it is how the player likes to play, not
    /// something a bankroll owns.
    #[serde(default)]
    pub ante: bool,
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
            // Off. A side bet the player did not ask for is a side bet
            // charged without consent (§5.75).
            ante: false,
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
        let mut prefs: Self = match load_json_key(&config.game_name, PREFERENCES_KEY) {
            Ok(prefs) => prefs,
            Err(error) => {
                if json_key_exists(&config.game_name, PREFERENCES_KEY) {
                    eprintln!("Dragon's Hoard preferences could not be loaded: {error}");
                }
                Self::default()
            }
        };
        prefs.sanitize(config);
        prefs
    }

    pub fn save(&self, config: &GameConfig) -> Result<(), String> {
        if !crate::state::persist::may_write() {
            return Ok(());
        }
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

// Tests live in the crate-level integration harness.
