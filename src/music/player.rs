//! Playing the score: four stems, mixed live, crossfading on mood.
//!
//! # Why this is not in `music`
//!
//! `music.rs` was at 793 lines of an 800-line hard limit (§5.54), and it held
//! two things that only share a subject. Everything left behind is **the
//! composition** — the key, the timing, which notes each track plays, what each
//! one sounds like. It is pure data and pure functions, rendered once and
//! identical every run.
//!
//! This is **the performance**: the loaded stems, the gains that follow the
//! game's mood, the crossfade, the volume the player set. It is stateful, it is
//! touched every frame, and none of it has an opinion about what the notes are.

use super::*;
/// The four loops, playing, with only their volumes moving.
pub struct Music {
    tracks: Vec<(Track, Sound)>,
    /// Where each track's volume is now, so a mood change is a fade.
    levels: [f32; Track::ALL.len()],
    mood: Mood,
    /// Which cabinet's mix is in force (§5.44).
    arrangement: Arrangement,
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
            arrangement: ARRANGEMENTS[0],
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
            arrangement: ARRANGEMENTS[0],
            volume: 0.0,
            enabled: false,
        }
    }

    /// Switch cabinets. The stems keep playing — only the mix moves, so a
    /// machine change is the same crossfade a mood change is (§5.31).
    pub fn set_arrangement(&mut self, arrangement: Arrangement) {
        self.arrangement = arrangement;
    }

    pub fn arrangement(&self) -> Arrangement {
        self.arrangement
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
            let target = self.arrangement.gain(self.mood, *track);
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
