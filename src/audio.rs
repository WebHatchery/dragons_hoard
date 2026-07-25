//! Procedurally synthesised sound effects.
//!
//! The game ships no audio files. Each effect is built as 16-bit PCM in memory
//! and handed to `macroquad::audio::load_sound_from_bytes`, the same way the
//! symbols are drawn from primitives rather than sampled from PNGs — one binary,
//! nothing to load over the wire, and the whole sound set is editable as code.
//!
//! This is a candidate for promotion into `macroquad-toolkit`: the toolkit's
//! `SoundManager` can only load from files or asset packs, and every game that
//! wants a placeholder blip currently has to find a `.wav` from somewhere. It is
//! kept project-local for now because changing the shared crate would touch
//! every other game in the workspace.

use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use macroquad_toolkit::rng::SeededRng;
use std::collections::HashMap;

const SAMPLE_RATE: u32 = 22_050;
/// Headroom so summed voices cannot clip into a crackle.
const MASTER_GAIN: f32 = 0.32;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wave {
    Sine,
    Square,
    Triangle,
    Noise,
}

/// One tone in an effect: a pitch glide under an attack/decay envelope.
#[derive(Debug, Clone, Copy)]
struct Voice {
    wave: Wave,
    /// Seconds from the start of the effect.
    start: f32,
    duration: f32,
    freq_from: f32,
    freq_to: f32,
    gain: f32,
    /// Fraction of the voice spent rising to full volume.
    attack: f32,
}

impl Voice {
    fn tone(start: f32, duration: f32, freq: f32, gain: f32) -> Self {
        Self {
            wave: Wave::Sine,
            start,
            duration,
            freq_from: freq,
            freq_to: freq,
            gain,
            attack: 0.04,
        }
    }

    fn glide(mut self, to: f32) -> Self {
        self.freq_to = to;
        self
    }

    fn wave(mut self, wave: Wave) -> Self {
        self.wave = wave;
        self
    }

    fn attack(mut self, attack: f32) -> Self {
        self.attack = attack;
        self
    }

    /// Amplitude envelope: linear attack, then a curved decay to silence.
    fn envelope(&self, t: f32) -> f32 {
        let progress = (t / self.duration).clamp(0.0, 1.0);
        if progress < self.attack {
            progress / self.attack.max(f32::EPSILON)
        } else {
            let fall = (progress - self.attack) / (1.0 - self.attack).max(f32::EPSILON);
            (1.0 - fall).powf(2.2)
        }
    }

    fn sample(&self, t: f32, phase: f32, rng: &mut SeededRng) -> f32 {
        let shape = match self.wave {
            Wave::Sine => (phase * std::f32::consts::TAU).sin(),
            Wave::Square => {
                if phase.fract() < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Wave::Triangle => 4.0 * (phase.fract() - 0.5).abs() - 1.0,
            Wave::Noise => rng.range_f32(-1.0, 1.0),
        };
        shape * self.envelope(t) * self.gain
    }

    /// Pitch at time `t`, glided geometrically so sweeps sound even.
    fn frequency(&self, t: f32) -> f32 {
        let progress = (t / self.duration).clamp(0.0, 1.0);
        self.freq_from * (self.freq_to / self.freq_from).powf(progress)
    }
}

/// Render voices to a mono 16-bit WAV. The noise voices draw from a fixed seed,
/// so a given build always produces byte-identical audio.
fn render(voices: &[Voice], seed: u64) -> Vec<u8> {
    let length = voices
        .iter()
        .map(|voice| voice.start + voice.duration)
        .fold(0.0f32, f32::max);
    let total = ((length * SAMPLE_RATE as f32).ceil() as usize).max(1);

    let mut samples = vec![0.0f32; total];
    let mut rng = SeededRng::new(seed);

    for voice in voices {
        let first = (voice.start * SAMPLE_RATE as f32) as usize;
        let count = (voice.duration * SAMPLE_RATE as f32) as usize;
        let mut phase = 0.0f32;

        for index in 0..count {
            let Some(slot) = samples.get_mut(first + index) else {
                break;
            };
            let t = index as f32 / SAMPLE_RATE as f32;
            *slot += voice.sample(t, phase, &mut rng);
            phase += voice.frequency(t) / SAMPLE_RATE as f32;
        }
    }

    let pcm: Vec<i16> = samples
        .iter()
        .map(|sample| (sample * MASTER_GAIN).clamp(-1.0, 1.0))
        .map(|sample| (sample * i16::MAX as f32) as i16)
        .collect();

    wav_bytes(&pcm)
}

/// A minimal canonical WAV container: RIFF/WAVE, one `fmt ` chunk, one `data`.
fn wav_bytes(pcm: &[i16]) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // format: PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // channels: mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in pcm {
        out.extend_from_slice(&sample.to_le_bytes());
    }

    out
}

/// The score. Every effect is a handful of voices; tuning the game's sound is
/// editing this function.
fn voices_for(sfx: Sfx) -> Vec<Voice> {
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
        Sfx::WinSmall => vec![
            Voice::tone(0.0, 0.12, 880.0, 0.42).wave(Wave::Triangle),
            Voice::tone(0.09, 0.18, 1318.0, 0.42).wave(Wave::Triangle),
        ],
        // A four-note arpeggio, held at the top.
        Sfx::WinBig => vec![
            Voice::tone(0.00, 0.13, 659.0, 0.40).wave(Wave::Triangle),
            Voice::tone(0.09, 0.13, 880.0, 0.40).wave(Wave::Triangle),
            Voice::tone(0.18, 0.13, 1046.0, 0.40).wave(Wave::Triangle),
            Voice::tone(0.27, 0.34, 1318.0, 0.46).wave(Wave::Triangle),
            Voice::tone(0.27, 0.34, 1760.0, 0.22),
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
            Voice::tone(0.0, 0.10, 1568.0, 0.34)
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
            let bytes = render(&voices_for(sfx), 0x51F_0000 + index as u64);
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

    fn read_u32(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    fn read_u16(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    #[test]
    fn the_wav_header_is_well_formed() {
        let bytes = render(&voices_for(Sfx::Click), 1);

        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(read_u32(&bytes, 16), 16, "PCM fmt chunk is 16 bytes");
        assert_eq!(read_u16(&bytes, 20), 1, "format tag is PCM");
        assert_eq!(read_u16(&bytes, 22), 1, "mono");
        assert_eq!(read_u32(&bytes, 24), SAMPLE_RATE);
        assert_eq!(read_u16(&bytes, 34), 16, "16 bits per sample");
        assert_eq!(&bytes[36..40], b"data");
    }

    #[test]
    fn the_declared_sizes_match_the_actual_payload() {
        let bytes = render(&voices_for(Sfx::Hatch), 2);
        let data_len = read_u32(&bytes, 40) as usize;

        assert_eq!(bytes.len(), 44 + data_len);
        assert_eq!(read_u32(&bytes, 4) as usize, bytes.len() - 8);
        assert_eq!(data_len % 2, 0, "16-bit samples come in pairs of bytes");
    }

    #[test]
    fn every_effect_renders_audible_audio() {
        for sfx in Sfx::ALL {
            let bytes = render(&voices_for(sfx), 3);
            assert!(bytes.len() > 44, "{:?} rendered no samples", sfx);

            let peak = bytes[44..]
                .chunks_exact(2)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]]).unsigned_abs())
                .max()
                .unwrap_or(0);
            assert!(peak > 1_000, "{:?} is inaudibly quiet (peak {})", sfx, peak);
        }
    }

    #[test]
    fn nothing_clips() {
        // Summed voices are gain-staged, not merely clamped; if this trips, the
        // mix is hitting the limiter and will sound crunchy.
        for sfx in Sfx::ALL {
            let bytes = render(&voices_for(sfx), 4);
            let clipped = bytes[44..]
                .chunks_exact(2)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
                .filter(|sample| sample.unsigned_abs() >= i16::MAX as u16 - 1)
                .count();
            assert_eq!(clipped, 0, "{:?} clips on {} samples", sfx, clipped);
        }
    }

    #[test]
    fn synthesis_is_deterministic() {
        // Noise voices must come from the seeded generator, not the shared one,
        // or two builds of the same commit would ship different audio.
        assert_eq!(
            render(&voices_for(Sfx::Hatch), 7),
            render(&voices_for(Sfx::Hatch), 7)
        );
    }

    #[test]
    fn an_envelope_opens_and_closes() {
        let voice = Voice::tone(0.0, 1.0, 440.0, 1.0).attack(0.1);

        assert_eq!(voice.envelope(0.0), 0.0);
        assert!((voice.envelope(0.1) - 1.0).abs() < 0.001, "peaks at attack");
        assert!(voice.envelope(0.5) < 1.0);
        assert!(voice.envelope(1.0) < 0.001, "decays to silence");
    }

    #[test]
    fn a_glide_runs_between_its_endpoints() {
        let voice = Voice::tone(0.0, 1.0, 200.0, 1.0).glide(800.0);

        assert!((voice.frequency(0.0) - 200.0).abs() < 0.01);
        assert!((voice.frequency(1.0) - 800.0).abs() < 0.01);
        // Geometric, so the midpoint is the geometric mean, not the average.
        assert!((voice.frequency(0.5) - 400.0).abs() < 0.01);
    }

    #[test]
    fn a_muted_bank_plays_nothing_and_holds_no_sounds() {
        let bank = SoundBank::muted();

        assert_eq!(bank.len(), 0);
        // Must not panic without an audio device.
        bank.play(Sfx::Hatch);
    }
}
