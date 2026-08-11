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
mod tests;
