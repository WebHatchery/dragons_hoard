//! Procedurally synthesised sound effects.
//!
//! The game ships no audio files. Each effect is built as 16-bit PCM in memory
//! and handed to `macroquad::audio::load_sound_from_bytes`, the same way the
//! symbols are drawn from primitives rather than sampled from PNGs — one binary,
//! nothing to load over the wire, and the whole sound set is editable as code.
//!
//! The synthesis itself now lives in `macroquad_toolkit::synth`, where every
//! game in the workspace can reach it — the toolkit's `SoundManager` can only
//! load from files or asset packs, so a blip used to mean sourcing a `.wav`.
//! What stays here is the part that is actually this game's: which effects exist
//! and what each one is made of.

/// The audit is a gate rather than a feature: nothing in the running game asks
/// what its own sound looks like, the same way `state::compat` is only ever a
/// test. Compiling it into the binary would be dead weight in the WASM build.
#[cfg(test)]
pub mod audit;

use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use macroquad_toolkit::synth::{render_wav, SynthConfig, Voice, Wave};
use std::collections::HashMap;

/// Sample rate and headroom are the toolkit's defaults; nothing about this
/// game's effects wants different ones.
pub fn config() -> SynthConfig {
    SynthConfig::default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sfx {
    SpinStart,
    ReelStop,
    WinSmall,
    WinBig,
    Scatter,
    Hatch,
    /// A coin locking into a Dragon's Wrath cell (§5.12).
    CoinLock,
    /// A seam taking the board — the moment the reels stop for it (§5.80).
    SeamOpen,
    /// One move of a rite: stone shifting (§5.80).
    SeamMove,
    Click,
}

impl Sfx {
    pub const ALL: [Sfx; 10] = [
        Sfx::SpinStart,
        Sfx::ReelStop,
        Sfx::WinSmall,
        Sfx::WinBig,
        Sfx::Scatter,
        Sfx::Hatch,
        Sfx::CoinLock,
        Sfx::SeamOpen,
        Sfx::SeamMove,
        Sfx::Click,
    ];
}

/// The score. Every effect is a handful of voices; tuning the game's sound is
/// editing this function.
pub fn voices_for(sfx: Sfx) -> Vec<Voice> {
    match sfx {
        // A short mechanical wind-up as the reels are released.
        Sfx::SpinStart => vec![
            Voice::tone(0.0, 0.20, 180.0, 0.5)
                .wave(Wave::Square)
                .glide(420.0),
            Voice::tone(0.0, 0.16, 90.0, 0.35).glide(200.0),
        ],
        // A dull thud with a click of noise on the front.
        Sfx::ReelStop => vec![
            Voice::tone(0.0, 0.13, 210.0, 0.7).glide(95.0).attack(0.01),
            Voice::tone(0.0, 0.045, 900.0, 0.22)
                .wave(Wave::Noise)
                .attack(0.01),
        ],
        // Two rising notes.
        //
        // Gains raised from 0.42 after the waveform panel (§5.19) showed this
        // peaking at 0.14 against ReelStop's 0.28 — a win was half the volume of
        // a reel merely stopping. Nothing in the test suite could see that; it
        // only ever asked whether the peak was above zero and below the limiter.
        Sfx::WinSmall => vec![
            Voice::tone(0.0, 0.12, 880.0, 1.06).wave(Wave::Triangle),
            Voice::tone(0.09, 0.18, 1318.0, 1.06).wave(Wave::Triangle),
        ],
        // A four-note arpeggio, held at the top.
        // Likewise: a big win came in under a reel stop.
        Sfx::WinBig => vec![
            Voice::tone(0.00, 0.13, 659.0, 0.80).wave(Wave::Triangle),
            Voice::tone(0.09, 0.13, 880.0, 0.80).wave(Wave::Triangle),
            Voice::tone(0.18, 0.13, 1046.0, 0.80).wave(Wave::Triangle),
            Voice::tone(0.27, 0.34, 1318.0, 0.90).wave(Wave::Triangle),
            Voice::tone(0.27, 0.34, 1760.0, 0.44),
        ],
        // A long shimmer upward — the scatter is the "something is coming" cue.
        Sfx::Scatter => vec![
            Voice::tone(0.0, 0.55, 440.0, 0.62)
                .wave(Wave::Triangle)
                .glide(1760.0)
                .attack(0.15),
            Voice::tone(0.0, 0.55, 660.0, 0.30)
                .glide(2640.0)
                .attack(0.2),
        ],
        // The biggest sound in the game: a low swell, a chord, and a hiss.
        Sfx::Hatch => vec![
            Voice::tone(0.0, 0.7, 110.0, 0.64).glide(220.0).attack(0.25),
            Voice::tone(0.18, 0.6, 523.0, 0.44).wave(Wave::Triangle),
            Voice::tone(0.18, 0.6, 659.0, 0.39).wave(Wave::Triangle),
            Voice::tone(0.18, 0.6, 784.0, 0.39).wave(Wave::Triangle),
            Voice::tone(0.0, 0.45, 2000.0, 0.21)
                .wave(Wave::Noise)
                .glide(400.0),
        ],
        // The seam taking the board: a low swell with a fifth over it, opening
        // slowly. It is a *stop* rather than a reward — the reels have halted
        // and the game is waiting on the player — so it opens rather than
        // strikes, and it sits under the routine mechanics of a spin for the
        // same reason the scatter cue does.
        Sfx::SeamOpen => vec![
            Voice::tone(0.0, 0.42, 196.0, 0.78)
                .wave(Wave::Triangle)
                .glide(294.0)
                .attack(0.3),
            Voice::tone(0.06, 0.36, 392.0, 0.39)
                .wave(Wave::Triangle)
                .attack(0.35),
        ],
        // One move of a rite: stone shifting against stone. Low, brief and
        // deliberately unmusical — it happens two or three times a round while
        // the player is reading the board, and a note would make them look at
        // the sound instead of the symbols.
        Sfx::SeamMove => vec![
            Voice::tone(0.0, 0.14, 150.0, 0.46)
                .glide(105.0)
                .attack(0.04),
            Voice::tone(0.0, 0.09, 620.0, 0.15)
                .wave(Wave::Noise)
                .attack(0.03),
        ],
        // Metal on stone: a bright strike that rings briefly. Deliberately
        // short — in a full round this fires up to fifteen times in a second,
        // so anything with a tail would smear into a wash.
        Sfx::CoinLock => vec![
            Voice::tone(0.0, 0.10, 1568.0, 0.52)
                .wave(Wave::Triangle)
                .attack(0.005),
            Voice::tone(0.0, 0.16, 2349.0, 0.18)
                .wave(Wave::Triangle)
                .attack(0.005),
            Voice::tone(0.0, 0.03, 3200.0, 0.14)
                .wave(Wave::Noise)
                .attack(0.005),
        ],
        // A soft blip for buttons and bet changes.
        Sfx::Click => vec![Voice::tone(0.0, 0.05, 1200.0, 0.35)
            .wave(Wave::Square)
            .attack(0.02)],
    }
}

/// Synthesised sounds, ready to play. Silent when audio could not start — a
/// missing device must never take the game down with it.
pub struct SoundBank {
    sounds: HashMap<Sfx, Sound>,
    volume: f32,
    muted: bool,
}

impl SoundBank {
    /// Silent bank, used by the screenshot harness and as the failure fallback.
    pub fn muted() -> Self {
        Self {
            sounds: HashMap::new(),
            volume: 0.0,
            muted: true,
        }
    }

    pub async fn load(volume: f32) -> Self {
        let mut sounds = HashMap::new();

        for (index, sfx) in Sfx::ALL.into_iter().enumerate() {
            let bytes = render_wav(&voices_for(sfx), &config(), 0x51F_0000 + index as u64);
            match load_sound_from_bytes(&bytes).await {
                Ok(sound) => {
                    sounds.insert(sfx, sound);
                }
                // One effect failing is not worth losing the rest of the set.
                Err(_) => continue,
            }
        }

        Self {
            sounds,
            volume,
            muted: false,
        }
    }

    pub fn len(&self) -> usize {
        self.sounds.len()
    }

    /// Follow the player's volume preference. Zero is true silence, not a
    /// near-inaudible floor.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn play(&self, sfx: Sfx) {
        self.play_at(sfx, 1.0);
    }

    pub fn play_at(&self, sfx: Sfx, gain: f32) {
        if self.muted || self.volume <= 0.0 {
            return;
        }
        let Some(sound) = self.sounds.get(&sfx) else {
            return;
        };
        play_sound(
            sound,
            PlaySoundParams {
                looped: false,
                volume: (self.volume * gain).clamp(0.0, 1.0),
            },
        );
    }
}

#[cfg(test)]
mod tests;
