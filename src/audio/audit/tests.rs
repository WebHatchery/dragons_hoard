use super::*;

/// Print the whole set. Not a gate — the gates are below; this is the thing
/// to read when one of them fails.
#[test]
#[ignore = "a report rather than a check; run with --ignored --nocapture"]
fn print_the_sound_set() {
    println!("\n{}", report());
}

#[test]
fn every_effect_is_well_formed() {
    let mut broken = Vec::new();
    for sfx in Sfx::ALL {
        for fault in faults(sfx) {
            broken.push(format!("{:?}: {}", sfx, fault.describe()));
        }
    }
    assert!(
        broken.is_empty(),
        "the sound set has faults nobody could hear from a waveform plot:\n  {}",
        broken.join("\n  ")
    );
}

/// §5.19's finding, as a gate rather than an eyeball.
///
/// The waveform panel showed a win peaking at half a reel stop, and the
/// gains were raised by hand. Nothing stopped the next edit undoing it.
#[test]
fn no_effect_is_drowned_out_by_another() {
    let peaks: Vec<(Sfx, f32)> = Sfx::ALL
        .iter()
        .map(|&sfx| (sfx, measure_sfx(sfx).peak))
        .collect();
    let loudest = peaks.iter().map(|(_, p)| *p).fold(0.0f32, f32::max);
    for (sfx, peak) in &peaks {
        assert!(
            *peak > loudest * 0.25,
            "{:?} peaks at {:.2} against the set's {:.2} — a quarter of the \
             loudest effect is inaudible next to it",
            sfx,
            peak,
            loudest
        );
    }
}

/// The sound set has an order, and it is a design decision rather than an
/// accident of which gains were typed last.
///
/// §5.19 found a win peaking at half a reel stop, raised the gains by hand
/// and moved on. It was still wrong: a small win peaked at 0.25 against a
/// reel merely stopping at 0.27, with identical RMS. Nothing was watching,
/// because "louder" had never been written down as something to check.
///
/// Quietest to loudest: a button, a seam shifting, a coin locking, the
/// scatter cue, the routine mechanics of a spin, a seam opening, then wins
/// in the order they are worth.
///
/// The two seam sounds land on opposite sides of that list, and the split
/// is the decision worth stating. **Opening** stops the whole game and asks
/// the player a question, so it sits above a reel merely stopping — it is
/// an interruption, not routine. **Moving** happens two or three times while
/// the player is still reading the board, and the board is what they are
/// deciding on, so it sits below even a coin locking. The loud half is the
/// one that wants attention; the quiet half is the one that would steal it
/// (§5.81).
#[test]
fn the_sound_set_is_in_the_order_it_is_meant_to_be_in() {
    let quieter_than = [
        (Sfx::Click, Sfx::SeamMove),
        (Sfx::SeamMove, Sfx::CoinLock),
        (Sfx::CoinLock, Sfx::Scatter),
        (Sfx::Scatter, Sfx::ReelStop),
        (Sfx::ReelStop, Sfx::SeamOpen),
        (Sfx::SeamOpen, Sfx::WinSmall),
        (Sfx::ReelStop, Sfx::WinSmall),
        (Sfx::SpinStart, Sfx::WinSmall),
        (Sfx::WinSmall, Sfx::WinBig),
        (Sfx::WinBig, Sfx::Hatch),
    ];
    for (quiet, loud) in quieter_than {
        let (a, b) = (measure_sfx(quiet).peak, measure_sfx(loud).peak);
        assert!(
            b > a,
            "{:?} peaks at {:.2} and {:?} at {:.2} — the reward is no louder                  than the routine sound it is meant to stand out from",
            loud,
            b,
            quiet,
            a
        );
    }
}

/// The score goes through the same oscillators, so it gets the same gates.
///
/// This is where band-limiting paid for itself twice over: the Arp and the
/// Pluck are squares, and nothing had ever asked what they were doing above
/// Nyquist. Fixing the oscillator fixed the music without the music being
/// touched, because `render_track` is `render_waveform` with a length.
#[test]
fn every_music_stem_is_well_formed() {
    for track in Track::ALL {
        let samples = music::waveform(track);
        let m = measure(&samples, music::config().sample_rate);
        assert!(m.peak > 0.02, "{:?} is inaudible at {:.3}", track, m.peak);
        assert!(m.clipped == 0, "{:?} clips {} samples", track, m.clipped);
        assert!(
            m.dc_offset.abs() < 0.02,
            "{:?} sits {:.3} off zero",
            track,
            m.dc_offset
        );
    }
}

/// A loop that does not meet itself clicks once per repeat — quiet enough to
/// miss on the first pass and impossible to ignore after five minutes.
#[test]
fn every_loop_meets_itself() {
    for track in Track::ALL {
        let seam = macroquad_toolkit::score::seam(&music::waveform(track));
        assert!(
            seam < 0.05,
            "{:?} jumps {:.3} where the loop wraps",
            track,
            seam
        );
    }
}

/// The renderer is deterministic (`synth`'s own promise), so the audit is
/// too — a report that changed between runs could not be acted on.
#[test]
fn measuring_twice_gives_the_same_answer() {
    for sfx in Sfx::ALL {
        assert_eq!(measure_sfx(sfx), measure_sfx(sfx));
    }
}
