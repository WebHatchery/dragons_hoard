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
    Click,
}

impl Sfx {
    pub const ALL: [Sfx; 8] = [
        Sfx::SpinStart,
        Sfx::ReelStop,
        Sfx::WinSmall,
        Sfx::WinBig,
        Sfx::Scatter,
        Sfx::Hatch,
        Sfx::CoinLock,
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
            Voice::tone(0.0, 0.12, 880.0, 0.80).wave(Wave::Triangle),
            Voice::tone(0.09, 0.18, 1318.0, 0.80).wave(Wave::Triangle),
        ],
        // A four-note arpeggio, held at the top.
        // Likewise: a big win came in under a reel stop.
        Sfx::WinBig => vec![
            Voice::tone(0.00, 0.13, 659.0, 0.62).wave(Wave::Triangle),
            Voice::tone(0.09, 0.13, 880.0, 0.62).wave(Wave::Triangle),
            Voice::tone(0.18, 0.13, 1046.0, 0.62).wave(Wave::Triangle),
            Voice::tone(0.27, 0.34, 1318.0, 0.70).wave(Wave::Triangle),
            Voice::tone(0.27, 0.34, 1760.0, 0.34),
        ],
        // A long shimmer upward — the scatter is the "something is coming" cue.
        Sfx::Scatter => vec![
            Voice::tone(0.0, 0.55, 440.0, 0.45)
                .wave(Wave::Triangle)
                .glide(1760.0)
                .attack(0.15),
            Voice::tone(0.0, 0.55, 660.0, 0.22)
                .glide(2640.0)
                .attack(0.2),
        ],
        // The biggest sound in the game: a low swell, a chord, and a hiss.
        Sfx::Hatch => vec![
            Voice::tone(0.0, 0.7, 110.0, 0.5).glide(220.0).attack(0.25),
            Voice::tone(0.18, 0.6, 523.0, 0.34).wave(Wave::Triangle),
            Voice::tone(0.18, 0.6, 659.0, 0.30).wave(Wave::Triangle),
            Voice::tone(0.18, 0.6, 784.0, 0.30).wave(Wave::Triangle),
            Voice::tone(0.0, 0.45, 2000.0, 0.16)
                .wave(Wave::Noise)
                .glide(400.0),
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
mod tests {
    use super::*;
    use macroquad_toolkit::synth::render_waveform;

    /// Byte length and checksum of every effect.
    ///
    /// The point of this is that a change to the sound set has to be a decision.
    /// Lifting the renderer into `macroquad_toolkit::synth` was exactly the kind
    /// of refactor that could have quietly altered every sound in the game, and
    /// nothing else here would have noticed — the other tests ask whether the
    /// bytes are well-formed, not whether they are the *same* bytes. The move
    /// left all eight untouched.
    ///
    /// Three then changed on purpose: `WinSmall`, `WinBig` and `CoinLock` were
    /// all quieter than a reel stopping, which the waveform panel (§5.19) made
    /// obvious the moment it existed. This test failed, which is what it is for.
    const BASELINE: [(Sfx, usize, u64); 8] = [
        (Sfx::SpinStart, 8_864, 1_094_828),
        (Sfx::ReelStop, 5_778, 699_881),
        (Sfx::WinSmall, 11_952, 1_488_632),
        (Sfx::WinBig, 26_946, 3_340_559),
        (Sfx::Scatter, 24_300, 3_273_537),
        (Sfx::Hatch, 34_442, 4_209_312),
        (Sfx::CoinLock, 7_100, 852_418),
        (Sfx::Click, 2_250, 276_370),
    ];

    fn checksum(bytes: &[u8]) -> u64 {
        bytes.iter().map(|byte| *byte as u64).sum()
    }

    #[test]
    fn the_sound_set_matches_its_baseline() {
        for (sfx, length, sum) in BASELINE {
            let bytes = render_wav(&voices_for(sfx), &config(), 0xA11CE);
            assert_eq!(bytes.len(), length, "{:?} changed length", sfx);
            assert_eq!(checksum(&bytes), sum, "{:?} changed content", sfx);
        }
    }

    #[test]
    fn the_baseline_covers_every_effect() {
        // A new effect added without a baseline entry would slip past the test
        // above entirely.
        assert_eq!(BASELINE.len(), Sfx::ALL.len());
        for sfx in Sfx::ALL {
            assert!(
                BASELINE.iter().any(|(id, _, _)| *id == sfx),
                "{:?} has no baseline",
                sfx
            );
        }
    }

    #[test]
    fn every_effect_is_audible_and_none_of_them_clip() {
        // Audible: the mix is not silence. Not clipping: it is not crunchy.
        for sfx in Sfx::ALL {
            let wave = render_waveform(&voices_for(sfx), &config(), 0xA11CE);
            let peak = wave.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
            assert!(peak > 0.05, "{:?} is inaudible at {:.3}", sfx, peak);
            assert!(peak < 0.999, "{:?} clips at {:.3}", sfx, peak);
        }
    }

    #[test]
    fn every_effect_decays_to_silence() {
        // One that ended mid-tone would click on every play.
        for sfx in Sfx::ALL {
            let wave = render_waveform(&voices_for(sfx), &config(), 0xA11CE);
            let tail = wave[wave.len().saturating_sub(16)..]
                .iter()
                .fold(0.0f32, |peak, s| peak.max(s.abs()));
            assert!(tail < 0.05, "{:?} ends at {:.3}, not silence", sfx, tail);
        }
    }

    /// The effect that fires up to fifteen times inside a second during a
    /// Dragon's Wrath round (§5.12). Anything with a tail smears into a wash,
    /// and this is the only way to check that without hearing it.
    #[test]
    fn the_coin_lock_is_short_enough_to_repeat() {
        let wave = render_waveform(&voices_for(Sfx::CoinLock), &config(), 0xA11CE);
        let seconds = wave.len() as f32 / config().sample_rate as f32;
        assert!(
            seconds < 0.2,
            "CoinLock runs {:.3}s; fifteen of those overlap",
            seconds
        );
    }
}
