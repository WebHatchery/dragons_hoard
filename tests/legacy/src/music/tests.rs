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
/// disaster on the fiftieth, and there is no way to hear it coming. Eight
/// stems now rather than four (§5.44), and they must all agree.
#[test]
fn every_stem_is_exactly_one_loop_long() {
    let expected = timing().samples(&config());
    for (track, samples) in all_waveforms() {
        assert_eq!(samples.len(), expected, "{} drifts", track.label());
    }
}

#[test]
fn no_stem_clicks_where_it_wraps() {
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
fn no_single_stem_clips() {
    for (track, samples) in all_waveforms() {
        assert!(peak(&samples) < 1.0, "{} clips alone", track.label());
        assert!(samples.iter().all(|s| s.is_finite()));
    }
}

/// The fault the waveform panel found in §5.31: a stem written so quiet it
/// would never have been heard under the effects.
#[test]
fn no_stem_is_written_too_quiet_to_hear() {
    let peaks: Vec<(Track, f32)> = all_waveforms()
        .iter()
        .map(|(track, samples)| (*track, peak(samples)))
        .collect();
    let loudest = peaks.iter().fold(0.0f32, |worst, (_, p)| worst.max(*p));

    for (track, level) in peaks {
        assert!(
            level > loudest * 0.30,
            "{} peaks at {:.3} against {:.3}, it would vanish in the mix",
            track.label(),
            level,
            loudest
        );
    }
}

/// The failure no audition would find, because it only happens in the game,
/// and now it has to hold for every cabinet rather than one.
#[test]
fn no_cabinet_clips_in_any_mood() {
    let rendered = all_waveforms();
    for arrangement in ARRANGEMENTS {
        for mood in Mood::ALL {
            let mix: Vec<(&[f32], f32)> = rendered
                .iter()
                .map(|(track, samples)| {
                    (samples.as_slice(), arrangement.gain(mood, *track) * MASTER)
                })
                .collect();
            let loudest = mixed_peak(&mix);
            assert!(
                loudest < 1.0,
                "{}/{:?} sums to {}",
                arrangement.id,
                mood,
                loudest
            );
        }
    }
}

#[test]
fn every_cabinet_is_audible_in_every_mood() {
    // A mood with everything at zero presents as "the music stopped", which
    // is indistinguishable from the audio device failing.
    for arrangement in ARRANGEMENTS {
        for mood in Mood::ALL {
            let loudest = Track::ALL
                .iter()
                .map(|track| arrangement.gain(mood, *track))
                .fold(0.0f32, f32::max);
            assert!(loudest > 0.5, "{}/{:?} is silent", arrangement.id, mood);
        }
    }
}

#[test]
fn every_gain_is_a_gain() {
    for arrangement in ARRANGEMENTS {
        for mood in Mood::ALL {
            for track in Track::ALL {
                let gain = arrangement.gain(mood, track);
                assert!(
                    (0.0..=1.0).contains(&gain),
                    "{}/{:?}/{:?} is {}",
                    arrangement.id,
                    mood,
                    track,
                    gain
                );
            }
        }
    }
}

/// The point of §5.44, stated as a property.
#[test]
fn no_two_cabinets_sound_the_same() {
    for (index, a) in ARRANGEMENTS.iter().enumerate() {
        for b in ARRANGEMENTS.iter().skip(index + 1) {
            let apart: f32 = Mood::ALL
                .iter()
                .flat_map(|mood| {
                    Track::ALL
                        .iter()
                        .map(move |track| (a.gain(*mood, *track) - b.gain(*mood, *track)).abs())
                })
                .sum();
            assert!(apart > 0.5, "{} and {} mix alike", a.id, b.id);
        }
    }
}

#[test]
fn every_cabinet_is_an_arrangement_rather_than_a_solo() {
    // Three stems is the fewest that reads as music rather than as a sound.
    for arrangement in ARRANGEMENTS {
        for mood in Mood::ALL {
            let playing = Track::ALL
                .iter()
                .filter(|track| arrangement.gain(mood, **track) > 0.05)
                .count();
            assert!(
                playing >= 3,
                "{}/{:?} plays {} stems",
                arrangement.id,
                mood,
                playing
            );
        }
    }
}

#[test]
fn every_moment_of_the_game_changes_the_mix_on_every_cabinet() {
    // Three moods that mix the same are three names for one piece of music,
    // whichever cabinet is playing it.
    for arrangement in ARRANGEMENTS {
        for (index, left) in Mood::ALL.iter().enumerate() {
            for right in Mood::ALL.iter().skip(index + 1) {
                let same = Track::ALL.iter().all(|track| {
                    arrangement.gain(*left, *track) == arrangement.gain(*right, *track)
                });
                assert!(
                    !same,
                    "{}: {:?} and {:?} are identical",
                    arrangement.id, left, right
                );
            }
        }
    }
}

#[test]
fn the_drum_belongs_to_the_held_rounds_everywhere() {
    // The one cross-cabinet rule: whatever else a room does, the drum
    // arriving is what a held board sounds like.
    for arrangement in ARRANGEMENTS {
        assert!(
            arrangement.gain(Mood::Held, Track::Drum) > arrangement.gain(Mood::Base, Track::Drum),
            "{}",
            arrangement.id
        );
    }
}

#[test]
fn every_cabinet_has_an_arrangement_and_an_unknown_one_falls_back() {
    for (id, _) in crate::ui::theme::THEMES {
        assert_eq!(arrangement(id).id, id, "no arrangement for {}", id);
    }
    assert_eq!(arrangement("no such cabinet").id, ARRANGEMENTS[0].id);
}

#[test]
fn every_pitched_note_is_in_key() {
    for track in [
        Track::Bass,
        Track::Pad,
        Track::Bell,
        Track::Drone,
        Track::Pluck,
    ] {
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
    music.set_arrangement(arrangement("frost"));
    music.set_mood(Mood::Feature);
    for _ in 0..300 {
        music.update(1.0 / 60.0);
    }
    for (index, track) in Track::ALL.iter().enumerate() {
        let target = arrangement("frost").gain(Mood::Feature, *track);
        assert!((music.levels()[index] - target).abs() < 1e-6);
    }
}

#[test]
fn switching_cabinet_is_a_fade_rather_than_a_cut() {
    // The stems keep playing; only the mix moves. A machine change should
    // sound like a mood change, not like the music restarting.
    let mut music = Music::silent();
    for _ in 0..300 {
        music.update(1.0 / 60.0);
    }
    let before = music.levels();

    music.set_arrangement(arrangement("ember"));
    music.update(1.0 / 60.0);
    let after = music.levels();

    for index in 0..Track::ALL.len() {
        assert!(
            (after[index] - before[index]).abs() < 0.5,
            "track {} jumped",
            index
        );
    }
    assert_ne!(before, after);
}

#[test]
fn levels_never_leave_the_rails_however_the_game_moves() {
    let mut music = Music::silent();
    for step in 0..900 {
        music.set_arrangement(ARRANGEMENTS[step % ARRANGEMENTS.len()]);
        music.set_mood(Mood::ALL[step % Mood::ALL.len()]);
        music.update(1.0 / 60.0);
        for level in music.levels() {
            assert!((0.0..=1.0).contains(&level), "{}", level);
        }
    }
}
