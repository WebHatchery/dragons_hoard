//! Spin presentation: the phase machine that turns an already-decided outcome
//! into a reel animation, a win count-up, and the pause between free spins.
//!
//! Nothing here decides anything. `engine::spin` picks the stops the moment the
//! player commits, and these types only *reveal* that decision over time — the
//! reels are told where to land before they start moving. That is what keeps the
//! maths (and the Monte-Carlo sim, which skips this module entirely) honest.

use crate::data::TimingConfig;
use macroquad_toolkit::strip::{StripFeel, StripSpinner};
use macroquad_toolkit::timing::Timer;

/// How this cabinet's reels move. The mechanics live in
/// `macroquad_toolkit::strip` (§5.23); what stays here is the tuning, because
/// how long a reel should turn for is a judgement about *this* game.
///
/// Reels were slowed from 0.62s after a player reported the symbols were
/// unreadable in flight, and the blur cap came down with them.
pub fn reel_feel(timing: &TimingConfig) -> StripFeel {
    StripFeel {
        base_time: timing.reel_base_seconds,
        stagger: timing.reel_stagger_seconds,
        blur_cap: timing.reel_blur_cap,
        ..StripFeel::default()
    }
}

/// Steps through a decided cascade chain so the player sees each collapse
/// rather than the last grid appearing all at once (§5.63).
///
/// The toolkit's [`Stepper`](macroquad_toolkit::reveal::Stepper) with this
/// game's beat baked in. It holds no symbols of its own — the chain lives on
/// the pending spin and this is only a cursor into it, so nothing here can
/// change what was decided (§8.2).
pub fn cascade_reveal(timing: &TimingConfig, steps: usize, scale: f32) -> CascadeReveal {
    CascadeReveal::new(steps, timing.cascade_beat_seconds, scale)
}

/// Counts a win up rather than snapping it on. The toolkit's
/// [`Countup`](macroquad_toolkit::reveal::Countup) at this game's pace.
pub fn payout_counter(timing: &TimingConfig, target: i64, scale: f32) -> PayoutCounter {
    PayoutCounter::new(target, timing.payout_seconds, scale)
}

pub type CascadeReveal = macroquad_toolkit::reveal::Stepper;
pub type PayoutCounter = macroquad_toolkit::reveal::Countup;

/// Reels mid-flight. The slot's name for the toolkit's spinner.
pub type ReelSpinner = StripSpinner;

pub fn anticipating_reels(
    scatters_per_reel: &[usize],
    trigger_count: usize,
    max_anticipating: usize,
) -> Vec<bool> {
    let mut flags = vec![false; scatters_per_reel.len()];
    if trigger_count == 0 || scatters_per_reel.is_empty() {
        return flags;
    }

    let mut running = 0usize;
    let mut stretched = 0usize;
    for (index, scatters) in scatters_per_reel.iter().enumerate() {
        // Decide *before* folding this reel in: it is the one still to land.
        let still_reachable = running + (scatters_per_reel.len() - index) >= trigger_count;
        if running + 1 >= trigger_count && still_reachable && stretched < max_anticipating {
            flags[index] = true;
            stretched += 1;
        }
        running += scatters;
    }
    flags
}

/// Where the machine is in the spin cycle. Only the dispatcher advances it; the
/// UI reads it to decide what to draw.
#[derive(Debug, Clone)]
pub enum SpinPhase {
    Idle,
    Spinning(ReelSpinner),
    /// Walking a decided cascade chain, one grid per beat (§5.15).
    Cascading(CascadeReveal),
    Payout(PayoutCounter),
    /// Brief beat before the next automatic free spin.
    AutoPause(Timer),
}

impl SpinPhase {
    pub fn is_idle(&self) -> bool {
        matches!(self, SpinPhase::Idle)
    }

    /// True whenever a spin is in flight — the point at which a second Spin
    /// intent must be ignored rather than queued.
    pub fn is_busy(&self) -> bool {
        !self.is_idle()
    }

    /// The cascade cursor, while a chain is being revealed.
    pub fn cascade(&self) -> Option<&CascadeReveal> {
        match self {
            SpinPhase::Cascading(reveal) => Some(reveal),
            _ => None,
        }
    }

    pub fn spinner(&self) -> Option<&ReelSpinner> {
        match self {
            SpinPhase::Spinning(spinner) => Some(spinner),
            _ => None,
        }
    }

    pub fn payout(&self) -> Option<&PayoutCounter> {
        match self {
            SpinPhase::Payout(counter) => Some(counter),
            _ => None,
        }
    }
}

/// What the machine wants the orchestrator to react to — sound, particles,
/// notifications. Emitted in the order things happened.
#[derive(Debug)]
pub enum SpinEvent {
    ReelStopped(usize),
    /// Every reel has landed and the outcome has been applied to the session.
    Settled(Box<super::SpinResolution>),
    PayoutFinished,
    /// A free or autospun spin is due; the orchestrator should raise a Spin
    /// intent so every spin still enters through the dispatcher.
    AutoSpinReady,
    /// A full-screen card just came up — time for its particles and shake.
    CelebrationOpened(super::celebration::CelebrationKind),
    /// A respin landed in an open Dragon's Wrath round (§5.12).
    HoldSpinRespun,
    /// The round ended and has been credited.
    HoldSpinFinished(super::holdspin::HoldSpinOutcome),
    /// A rite took another bite of the board in an open seam (§5.80).
    SeamMoved,
    /// The seam finished and has been credited. Boxed for the same reason
    /// `Settled` is: the outcome carries the rite's name, and a fat variant
    /// makes every other event as big as this one.
    SeamFinished(Box<super::seam::SeamOutcome>),
    /// A cascade collapsed into the next grid (§5.15).
    Cascaded,
}

// Tests live in the crate-level integration harness.
