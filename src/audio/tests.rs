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
const BASELINE: [(Sfx, usize, u64); 10] = [
    (Sfx::SpinStart, 8_864, 1_099_123),
    (Sfx::ReelStop, 5_778, 699_881),
    (Sfx::WinSmall, 11_952, 1_496_569),
    (Sfx::WinBig, 26_946, 3_360_240),
    (Sfx::Scatter, 24_300, 2_997_188),
    (Sfx::Hatch, 34_442, 4_254_136),
    (Sfx::CoinLock, 7_100, 825_784),
    (Sfx::SeamOpen, 18_566, 2_310_889),
    (Sfx::SeamMove, 6_218, 761_015),
    (Sfx::Click, 2_250, 272_193),
];

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().map(|byte| *byte as u64).sum()
}

#[test]
#[ignore = "prints the table for the test below; run after changing a sound"]
fn print_the_baseline() {
    for sfx in Sfx::ALL {
        let bytes = render_wav(&voices_for(sfx), &config(), 0xA11CE);
        println!(
            "        (Sfx::{:?}, {}, {}),",
            sfx,
            bytes.len(),
            checksum(&bytes)
        );
    }
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
