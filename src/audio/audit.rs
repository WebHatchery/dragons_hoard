//! What the sound set actually sounds like.
//!
//! # The oldest unverified surface in this project
//!
//! The design document has said three times, in three different iterations,
//! that nobody has heard any of this. §5.19 built a waveform panel and the mix
//! was *looked* at, which caught three effects quieter than a reel stop. What a
//! plot cannot show is timbre — whether a triangle at 1568Hz is a pleasant chime
//! or a nasty one — and that was written down as something only ears could
//! settle.
//!
//! It is not. A chime sounds nasty when it carries partials that are not
//! harmonics of the note, and an oscillator built by comparing a phase against a
//! threshold generates exactly that: partials above Nyquist reflect back down to
//! frequencies with no musical relation to anything. `synth::audit` predicts
//! where they land and measures whether they are there.
//!
//! # What is asserted here and what is not
//!
//! This says an effect is well-formed: audible, not clipped, centred, without a
//! click, and without reflections loud enough to hear under the note. It says
//! nothing about whether the sound set is *good*. A tasteful arpeggio and a
//! tasteless one pass identically, and that judgement still needs a listener.

use crate::audio::{config, voices_for, Sfx};
use crate::music::{self, Track};
use macroquad_toolkit::synth::audit::{aliasing, measure, Fault, Measured};
use macroquad_toolkit::synth::render_unclamped;

/// Fixed so a report is reproducible; noise voices draw from it.
const SEED: u64 = 0x5EED;

/// How far under the note a reflection may sit before it stops mattering.
///
/// -32dB is about 2.5% of the note's amplitude. Below that a partial is masked
/// by the note it sits under; above it, it is a separate thing the ear picks out
/// as a buzz or a whistle.
const ALIAS_FLOOR_DB: f32 = -32.0;

/// Measure one effect end to end.
pub fn measure_sfx(sfx: Sfx) -> Measured {
    measure(
        &render_unclamped(&voices_for(sfx), &config(), SEED),
        config().sample_rate,
    )
}

/// Everything wrong with one effect.
pub fn faults(sfx: Sfx) -> Vec<Fault> {
    let config = config();
    let rendered = render_unclamped(&voices_for(sfx), &config, SEED);
    let m = measure(&rendered, config.sample_rate);
    let mut faults = Vec::new();

    if m.peak < 0.01 {
        faults.push(Fault::Silent);
    }
    if m.clipped > 0 {
        faults.push(Fault::Clipped {
            peak: m.peak,
            samples: m.clipped,
        });
    }
    // A little offset is inevitable in a short asymmetric envelope; a lot means
    // the waveform never crosses zero and the effect thumps on and off.
    if m.dc_offset.abs() > 0.02 {
        faults.push(Fault::OffCentre {
            offset: m.dc_offset,
        });
    }
    // A step this big between neighbouring samples is a discontinuity rather
    // than a waveform — at 22kHz even a full-scale square only moves that far
    // twice a cycle, and never faster than its own period.
    if m.worst_step > 0.6 {
        faults.push(Fault::Click {
            step: m.worst_step,
            at_seconds: m.worst_step_at,
        });
    }
    for voice in voices_for(sfx) {
        faults.extend(aliasing(&voice, &config, ALIAS_FLOOR_DB));
    }
    faults
}

/// One music stem, measured the same way as an effect.
///
/// The score goes through the same oscillators as the effects — `render_track`
/// is `render_waveform` with a length — so everything above applies to it, and
/// the Arp and Pluck are squares that had every reason to be aliasing.
pub fn measure_track(track: Track) -> Measured {
    measure(&music::waveform(track), music::config().sample_rate)
}

/// The whole sound set, for a report.
pub fn report() -> String {
    let mut out = String::new();
    for sfx in Sfx::ALL {
        let m = measure_sfx(sfx);
        out.push_str(&format!(
            "{:<10} peak {:.2}  rms {:.3}  dc {:+.3}  step {:.2}  {:.2}s\n",
            format!("{:?}", sfx),
            m.peak,
            m.rms,
            m.dc_offset,
            m.worst_step,
            m.seconds
        ));
        for fault in faults(sfx) {
            out.push_str(&format!("           {}\n", fault.describe()));
        }
    }
    for track in Track::ALL {
        let m = measure_track(track);
        out.push_str(&format!(
            "{:<10} peak {:.2}  rms {:.3}  dc {:+.3}  step {:.2}  {:.2}s  (music)\n",
            format!("{:?}", track),
            m.peak,
            m.rms,
            m.dc_offset,
            m.worst_step,
            m.seconds
        ));
    }
    out
}

#[cfg(test)]
mod tests {
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
}
