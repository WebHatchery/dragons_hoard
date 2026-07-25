//! The score, and the mix that follows the game (§5.31).
//!
//! # Vertical remixing
//!
//! Four tracks, all the same length, all started together and **left running for
//! the rest of the session**. Nothing is ever started or stopped in response to
//! gameplay; only the four volumes move. That is the whole design, and it buys
//! two things that a start/stop approach cannot:
//!
//! - **It cannot glitch.** A track begun when the free spins trigger would enter
//!   wherever the bar happened to be, and would need scheduling against the beat
//!   to avoid sounding like a mistake. Tracks that never stop are always in time
//!   with each other because they have been since the first frame.
//! - **A transition is a fade,** so the arrangement thickens and thins rather
//!   than cutting. The base game is bass and pad; free spins bring in the
//!   arpeggio; the Dragon's Wrath adds the drum and pushes everything else down.
//!
//! The cost is four sounds playing at all times, three of them usually silent.
//! At 22 kHz mono that is cheap, and it is the same cost every frame, which
//! matters more than the average.
//!
//! # Written blind
//!
//! Like the effects (§7.1, §5.19), this was composed by someone who has never
//! heard it. That rules out mixing by ear and puts the weight on things that can
//! be measured, so the tests check the three faults that are inaudible until
//! they are not: **tracks that drift out of phase**, **a loop that does not meet
//! itself** and clicks once per repeat, and **four tracks that are each fine
//! alone and clip when summed**.
//!
//! What no test can check is whether it is any good.

use macroquad::audio::{
    load_sound_from_bytes, play_sound, set_sound_volume, PlaySoundParams, Sound,
};
use macroquad_toolkit::score::{
    lay, render_track, Note, Scale, Timbre, Timing, MINOR, PENTATONIC_MINOR,
};
use macroquad_toolkit::synth::{wav_bytes, SynthConfig, Wave};

/// Quiet, and quieter than the effects on purpose: this plays constantly and
/// they do not. Music that competes with the reel stops is music that gets
/// turned off.
const MASTER: f32 = 0.20;

/// Four bars at a walking tempo. Long enough not to feel like a jingle, short
/// enough that all four tracks fit in memory without thought.
pub fn timing() -> Timing {
    Timing {
        bpm: 84.0,
        beats_per_bar: 4,
        bars: 4,
    }
}

/// D minor, two octaves below the reference. Low, because everything else in
/// the game — every effect, every voice in them — sits above it.
pub fn key() -> Scale {
    Scale::new(-19, MINOR)
}

/// The pentatonic over the same tonic, for the arpeggio. Five degrees that
/// cannot clash with each other, so a run over a moving bass stays consonant
/// without anyone having to think about which chord is underneath.
pub fn arp_key() -> Scale {
    Scale::new(-19, PENTATONIC_MINOR)
}

/// A layer of the arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Track {
    /// Root movement. The only track audible on its own.
    Bass,
    /// Sustained thirds and fifths. The bed everything else sits on.
    Pad,
    /// Sixteenths over the pentatonic. Free spins and nothing else.
    Arp,
    /// The pulse. Only when the dragon is awake.
    Drum,
}

impl Track {
    pub const ALL: [Track; 4] = [Track::Bass, Track::Pad, Track::Arp, Track::Drum];

    pub fn label(self) -> &'static str {
        match self {
            Track::Bass => "Bass",
            Track::Pad => "Pad",
            Track::Arp => "Arp",
            Track::Drum => "Drum",
        }
    }
}

/// What the game is doing, in the only terms the music cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mood {
    #[default]
    Base,
    /// Free spins, of any kind (§5.4, §5.21).
    Feature,
    /// The Dragon's Wrath respin round (§5.12) or a Vault Pick board (§5.10) —
    /// both hold the reels and both want the room to feel different.
    Held,
}

impl Mood {
    pub const ALL: [Mood; 3] = [Mood::Base, Mood::Feature, Mood::Held];

    /// Where each track sits in this mood.
    ///
    /// Every mood names every track deliberately: a `_ => 0.0` fallback is how a
    /// layer added later ends up silent everywhere and nobody notices.
    pub fn gain(self, track: Track) -> f32 {
        match (self, track) {
            (Mood::Base, Track::Bass) => 1.0,
            (Mood::Base, Track::Pad) => 0.7,
            (Mood::Base, Track::Arp) => 0.0,
            (Mood::Base, Track::Drum) => 0.0,

            (Mood::Feature, Track::Bass) => 0.9,
            (Mood::Feature, Track::Pad) => 0.8,
            (Mood::Feature, Track::Arp) => 1.0,
            (Mood::Feature, Track::Drum) => 0.0,

            // The bass drops back so the drum has somewhere to sit; the arp
            // stays out of it, because the Wrath is a held board rather than a
            // run of spins and a busy top end would fight the coin locks.
            (Mood::Held, Track::Bass) => 0.6,
            (Mood::Held, Track::Pad) => 0.5,
            (Mood::Held, Track::Arp) => 0.0,
            (Mood::Held, Track::Drum) => 1.0,
        }
    }
}

/// How a track is voiced.
fn timbre(track: Track) -> Timbre {
    match track {
        // Short and round, so the root reads as a pulse rather than a drone.
        Track::Bass => Timbre::new(Wave::Sine, 0.85).sustain(0.55).attack(0.03),
        // Long attack, full sustain: the two things that make a pad.
        Track::Pad => Timbre::new(Wave::Triangle, 0.30).sustain(1.0).attack(0.35),
        // These two carry the transitions, so they are levelled against the
        // bass rather than written quiet and hoped for. The waveform panel had
        // them at 0.04 and 0.07 peak against the bass's 0.19 — the arpeggio
        // that is supposed to announce free spins would not have been audible
        // under the effects at all (§5.19, §5.31).
        Track::Arp => Timbre::new(Wave::Square, 0.62).sustain(0.45).attack(0.02),
        // Noise with an instant attack and no tail is a drum, near enough.
        Track::Drum => Timbre::new(Wave::Noise, 0.95).sustain(0.16).attack(0.01),
    }
}

/// The notes.
///
/// Four bars of i — VI — III — VII, which is the progression every minor-key
/// game loop has been for thirty years, because it never resolves and so never
/// asks to end.
fn notes(track: Track) -> Vec<Note> {
    // Root of each bar, as a scale degree.
    const ROOTS: [i32; 4] = [0, 5, 2, 6];

    match track {
        Track::Bass => ROOTS
            .iter()
            .enumerate()
            .flat_map(|(bar, root)| {
                let beat = bar as f32 * 4.0;
                [
                    Note::new(beat, *root, 1.0),
                    Note::new(beat + 1.5, *root, 0.5).gain(0.6),
                    Note::new(beat + 2.0, *root, 1.0).gain(0.85),
                    // The fifth on the last beat, walking into the next bar.
                    Note::new(beat + 3.0, root + 4, 1.0).gain(0.7),
                ]
            })
            .collect(),

        // One chord per bar, held its whole length: root, third, fifth.
        Track::Pad => ROOTS
            .iter()
            .enumerate()
            .flat_map(|(bar, root)| {
                let beat = bar as f32 * 4.0;
                [
                    Note::new(beat, *root, 4.0).octave(1),
                    Note::new(beat, root + 2, 4.0).octave(1).gain(0.8),
                    Note::new(beat, root + 4, 4.0).octave(1).gain(0.7),
                ]
            })
            .collect(),

        // Eighths climbing and falling through the pentatonic. The degrees are
        // pentatonic, so they are consonant over every root above.
        Track::Arp => (0..32)
            .map(|step| {
                let shape = [0, 1, 2, 3, 4, 3, 2, 1][step % 8];
                Note::new(step as f32 * 0.5, shape, 0.5)
                    .octave(2)
                    .gain(if step % 4 == 0 { 1.0 } else { 0.65 })
            })
            .collect(),

        // Four to the floor with an offbeat lift. Pitch is irrelevant to noise,
        // so the degree is a formality; the gain is what carries the pattern.
        Track::Drum => (0..16)
            .flat_map(|beat| {
                [
                    Note::new(beat as f32, 0, 0.25).gain(if beat % 4 == 0 { 1.0 } else { 0.55 }),
                    Note::new(beat as f32 + 0.5, 0, 0.25).gain(0.28),
                ]
            })
            .collect(),
    }
}

/// One track's waveform, at the length the timing demands.
pub fn waveform(track: Track) -> Vec<f32> {
    let timing = timing();
    let scale = if track == Track::Arp {
        arp_key()
    } else {
        key()
    };
    let voices = lay(&notes(track), &scale, &timing, &timbre(track));
    // Fixed seeds so the drum's noise is the same loop every run — a drum that
    // re-randomised would be a different track each session.
    render_track(&voices, &config(), &timing, 0xD0_0D + track as u64)
}

/// Quieter than the effects share, and with the headroom four tracks need.
fn config() -> SynthConfig {
    SynthConfig {
        master_gain: 0.22,
        ..SynthConfig::default()
    }
}

/// The four loops, playing, with only their volumes moving.
pub struct Music {
    tracks: Vec<(Track, Sound)>,
    /// Where each track's volume is now, so a mood change is a fade.
    levels: [f32; Track::ALL.len()],
    mood: Mood,
    volume: f32,
    enabled: bool,
}

impl Music {
    /// Render, load and start every track at silence.
    ///
    /// Starting them all immediately — before the player has done anything — is
    /// what keeps them in phase for the rest of the session.
    pub async fn load(volume: f32) -> Self {
        let mut tracks = Vec::new();
        for track in Track::ALL {
            let pcm: Vec<i16> = waveform(track)
                .iter()
                .map(|sample| (sample * i16::MAX as f32) as i16)
                .collect();
            let Ok(sound) = load_sound_from_bytes(&wav_bytes(&pcm, &config())).await else {
                continue;
            };
            play_sound(
                &sound,
                PlaySoundParams {
                    looped: true,
                    volume: 0.0,
                },
            );
            tracks.push((track, sound));
        }

        Self {
            tracks,
            levels: [0.0; Track::ALL.len()],
            mood: Mood::Base,
            volume,
            enabled: true,
        }
    }

    /// A silent stand-in for the capture harness, which has no audio device.
    pub fn silent() -> Self {
        Self {
            tracks: Vec::new(),
            levels: [0.0; Track::ALL.len()],
            mood: Mood::Base,
            volume: 0.0,
            enabled: false,
        }
    }

    pub fn set_mood(&mut self, mood: Mood) {
        self.mood = mood;
    }

    pub fn mood(&self) -> Mood {
        self.mood
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn levels(&self) -> [f32; Track::ALL.len()] {
        self.levels
    }

    /// Move every level toward its target and push the result to the mixer.
    ///
    /// A fixed rate rather than a fixed duration, so a small change is quick and
    /// a large one is not instant — the arpeggio arriving takes about a second
    /// whichever mood it came from.
    pub fn update(&mut self, dt: f32) {
        const RATE: f32 = 1.2;

        for (index, track) in Track::ALL.iter().enumerate() {
            let target = self.mood.gain(*track);
            let level = &mut self.levels[index];
            let step = RATE * dt;
            *level = if (*level - target).abs() <= step {
                target
            } else if *level < target {
                *level + step
            } else {
                *level - step
            };
        }

        if !self.enabled {
            return;
        }
        for (track, sound) in &self.tracks {
            let index = Track::ALL.iter().position(|t| t == track).unwrap_or(0);
            set_sound_volume(sound, self.levels[index] * self.volume * MASTER);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad_toolkit::score::{mixed_peak, peak, seam};

    fn all_waveforms() -> Vec<(Track, Vec<f32>)> {
        Track::ALL
            .iter()
            .map(|track| (*track, waveform(*track)))
            .collect()
    }

    /// The invariant vertical remixing rests on.
    ///
    /// One sample of drift per loop is inaudible on the first pass and a
    /// disaster on the fiftieth, and there is no way to hear it coming.
    #[test]
    fn every_track_is_exactly_one_loop_long() {
        let expected = timing().samples(&config());
        for (track, samples) in all_waveforms() {
            assert_eq!(samples.len(), expected, "{} drifts", track.label());
        }
    }

    #[test]
    fn no_track_clicks_where_it_wraps() {
        // A loop that does not meet itself steps discontinuously once per
        // repeat. Quiet enough to miss once, impossible to ignore after five
        // minutes.
        for (track, samples) in all_waveforms() {
            assert!(
                seam(&samples) < 0.05,
                "{} jumps {} at the seam",
                track.label(),
                seam(&samples)
            );
        }
    }

    #[test]
    fn no_single_track_clips() {
        for (track, samples) in all_waveforms() {
            assert!(peak(&samples) < 1.0, "{} clips alone", track.label());
            assert!(samples.iter().all(|s| s.is_finite()));
        }
    }

    /// The failure no audition would find, because it only happens in the game.
    #[test]
    fn no_mood_clips_when_its_tracks_play_together() {
        let rendered = all_waveforms();
        for mood in Mood::ALL {
            let mix: Vec<(&[f32], f32)> = rendered
                .iter()
                .map(|(track, samples)| (samples.as_slice(), mood.gain(*track) * MASTER))
                .collect();
            let loudest = mixed_peak(&mix);
            assert!(loudest < 1.0, "{:?} sums to {}", mood, loudest);
        }
    }

    /// The fault the waveform panel found: two tracks written so quiet they
    /// would never have been heard under the effects.
    #[test]
    fn no_track_is_written_too_quiet_to_hear() {
        let peaks: Vec<(Track, f32)> = all_waveforms()
            .iter()
            .map(|(track, samples)| (*track, peak(samples)))
            .collect();
        let loudest = peaks.iter().fold(0.0f32, |worst, (_, p)| worst.max(*p));

        for (track, level) in peaks {
            assert!(
                level > loudest * 0.35,
                "{} peaks at {:.3} against {:.3} — it would vanish in the mix",
                track.label(),
                level,
                loudest
            );
        }
    }

    #[test]
    fn every_mood_is_audible() {
        // A mood with everything at zero is a bug that presents as "the music
        // stopped", which is indistinguishable from the audio device failing.
        for mood in Mood::ALL {
            let loudest = Track::ALL
                .iter()
                .map(|track| mood.gain(*track))
                .fold(0.0f32, f32::max);
            assert!(loudest > 0.5, "{:?} is silent", mood);
        }
    }

    #[test]
    fn every_mood_names_every_track() {
        // Guarding the `match` above: a gain outside 0..=1 is the shape a
        // forgotten arm takes once someone adds a catch-all.
        for mood in Mood::ALL {
            for track in Track::ALL {
                let gain = mood.gain(track);
                assert!((0.0..=1.0).contains(&gain), "{:?}/{:?}", mood, track);
            }
        }
    }

    #[test]
    fn the_moods_are_actually_different_arrangements() {
        // Three moods that mix the same are three names for one piece of music.
        for (index, left) in Mood::ALL.iter().enumerate() {
            for right in Mood::ALL.iter().skip(index + 1) {
                let same = Track::ALL
                    .iter()
                    .all(|track| left.gain(*track) == right.gain(*track));
                assert!(!same, "{:?} and {:?} sound identical", left, right);
            }
        }
    }

    #[test]
    fn the_arpeggio_belongs_to_the_feature() {
        // The one track whose presence is the transition. If it played in the
        // base game there would be nothing left for free spins to add.
        assert_eq!(Mood::Base.gain(Track::Arp), 0.0);
        assert!(Mood::Feature.gain(Track::Arp) > 0.5);
        assert_eq!(Mood::Held.gain(Track::Arp), 0.0);
    }

    #[test]
    fn every_pitched_note_is_in_key() {
        // A written wrong note is indistinguishable from a typo in a degree, and
        // neither can be heard by its author.
        for track in [Track::Bass, Track::Pad] {
            for note in notes(track) {
                let semitone = key().semitones(note.degree);
                assert!(
                    key().contains_semitone(semitone),
                    "{} plays degree {} out of key",
                    track.label(),
                    note.degree
                );
            }
        }
        for note in notes(Track::Arp) {
            let semitone = arp_key().semitones(note.degree);
            assert!(arp_key().contains_semitone(semitone));
            // And the pentatonic has to be a subset of the key it sits over, or
            // the arpeggio would clash with the pad rather than colour it.
            assert!(key().contains_semitone(semitone), "arp leaves the key");
        }
    }

    #[test]
    fn every_note_lands_inside_the_loop() {
        let beats = timing().beats();
        for track in Track::ALL {
            for note in notes(track) {
                assert!(note.beat >= 0.0, "{} starts early", track.label());
                assert!(note.beat < beats, "{} starts past the end", track.label());
                assert!(note.beats > 0.0);
                assert!((0.0..=1.0).contains(&note.gain));
            }
        }
    }

    #[test]
    fn a_fade_reaches_its_target_and_stops_there() {
        let mut music = Music::silent();
        music.set_mood(Mood::Feature);
        for _ in 0..200 {
            music.update(1.0 / 60.0);
        }
        for (index, track) in Track::ALL.iter().enumerate() {
            assert!((music.levels()[index] - Mood::Feature.gain(*track)).abs() < 1e-6);
        }
    }

    #[test]
    fn a_mood_change_is_a_fade_rather_than_a_cut() {
        // A cut is what start/stop scheduling was avoided to prevent.
        let mut music = Music::silent();
        for _ in 0..200 {
            music.update(1.0 / 60.0);
        }
        let before = music.levels();

        music.set_mood(Mood::Held);
        music.update(1.0 / 60.0);
        let after = music.levels();

        for index in 0..Track::ALL.len() {
            let moved = (after[index] - before[index]).abs();
            assert!(moved < 0.5, "track {} jumped {}", index, moved);
        }
        assert_ne!(before, after);
    }

    #[test]
    fn levels_never_leave_the_rails_however_the_mood_moves() {
        let mut music = Music::silent();
        for step in 0..600 {
            music.set_mood(Mood::ALL[step % Mood::ALL.len()]);
            music.update(1.0 / 60.0);
            for level in music.levels() {
                assert!((0.0..=1.0).contains(&level), "{}", level);
            }
        }
    }
}
