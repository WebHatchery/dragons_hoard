//! Spin presentation: the phase machine that turns an already-decided outcome
//! into a reel animation, a win count-up, and the pause between free spins.
//!
//! Nothing here decides anything. `engine::spin` picks the stops the moment the
//! player commits, and these types only *reveal* that decision over time — the
//! reels are told where to land before they start moving. That is what keeps the
//! maths (and the Monte-Carlo sim, which skips this module entirely) honest.

use macroquad_toolkit::math::ease_out_quad;
use macroquad_toolkit::strip::{StripFeel, StripSpinner};
use macroquad_toolkit::timing::Timer;

/// How this cabinet's reels move. The mechanics live in
/// `macroquad_toolkit::strip` (§5.23); what stays here is the tuning, because
/// how long a reel should turn for is a judgement about *this* game.
///
/// Reels were slowed from 0.62s after a player reported the symbols were
/// unreadable in flight, and the blur cap came down with them.
pub fn reel_feel() -> StripFeel {
    StripFeel {
        base_time: 0.95,
        stagger: 0.30,
        blur_cap: 0.85,
        ..StripFeel::default()
    }
}

/// Reels mid-flight. The slot's name for the toolkit's spinner.
pub type ReelSpinner = StripSpinner;

/// Seconds the win count-up takes.
const PAYOUT_TIME: f32 = 0.75;
/// Beat between cascade collapses. Slower than a reel stop on purpose — each
/// collapse is its own small reveal, and running them faster turns a chain into
/// a flicker.
const CASCADE_BEAT: f32 = 0.55;
/// Steps through a decided cascade chain so the player sees each collapse
/// rather than the last grid appearing all at once.
///
/// Holds no symbols of its own — the chain lives on the pending spin, and this
/// is only a cursor into it. Nothing here can change what was decided (§8.2).
#[derive(Debug, Clone)]
pub struct CascadeReveal {
    step: usize,
    steps: usize,
    /// Set when the *last* grid has had its beat, not when it is reached.
    /// Without this the final grid of a chain settled the instant the reveal
    /// arrived at it and was never shown as part of the chain at all.
    done: bool,
    beat: Timer,
    scale: f32,
}

impl CascadeReveal {
    pub fn new(steps: usize, scale: f32) -> Self {
        Self {
            step: 0,
            steps,
            done: false,
            beat: Timer::new(CASCADE_BEAT * scale),
            scale,
        }
    }

    pub fn step(&self) -> usize {
        self.step
    }

    /// Advance on the beat. Returns `true` on the tick that moves to a new grid,
    /// so the orchestrator can make a noise exactly once per collapse.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.done || !self.beat.tick(dt) {
            return false;
        }
        if self.step + 1 >= self.steps {
            // The last grid has now had its beat; the chain is over.
            self.done = true;
            return false;
        }
        self.step += 1;
        self.beat = Timer::new(CASCADE_BEAT * self.scale);
        true
    }

    pub fn finished(&self) -> bool {
        self.done
    }
}

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

/// Counts a win up rather than snapping it on.
#[derive(Debug, Clone)]
pub struct PayoutCounter {
    target: i64,
    timer: Timer,
}

impl PayoutCounter {
    pub fn new(target: i64, time_scale: f32) -> Self {
        Self {
            target,
            timer: Timer::new(PAYOUT_TIME * time_scale.clamp(0.05, 4.0)),
        }
    }

    /// Returns true on the tick that finishes the count-up.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.timer.tick(dt)
    }

    pub fn value(&self) -> i64 {
        (self.target as f32 * ease_out_quad(self.timer.progress())) as i64
    }
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
    /// A cascade collapsed into the next grid (§5.15).
    Cascaded,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mechanics moved to `macroquad_toolkit::strip` (§5.23) and are tested
    /// there against a default feel. What matters *here* is that this game's own
    /// tuning still lands a reel on its stop — a `base_time` or `revolutions` edit
    /// is exactly the sort of change that could break it without the toolkit
    /// noticing.
    #[test]
    fn this_cabinets_feel_still_lands_every_reel_on_its_stop() {
        let feel = reel_feel();
        for index in 0..5 {
            for target in [0usize, 1, 17, 39] {
                let mut spinner = ReelSpinner::new(&[40], &[7], &[target], 1.0, &[false], &feel);
                for _ in 0..1_200 {
                    spinner.tick(1.0 / 60.0);
                }
                assert!(spinner.all_settled());
                assert_eq!(
                    spinner.position(0).round() as usize % 40,
                    target,
                    "reel {} missed its stop",
                    index
                );
            }
        }
    }

    #[test]
    fn anticipation_fires_only_when_the_feature_is_still_live() {
        // Two scatters on the first two reels, needing three: every reel still
        // to land could be the one that completes it, so all three are held
        // back. That is what a physical cabinet does, and it is the reason a
        // near-miss is agonising rather than instant.
        let flags = anticipating_reels(&[1, 1, 0, 0, 0], 3, 4);
        assert_eq!(flags, vec![false, false, true, true, true]);
    }

    #[test]
    fn anticipation_never_fires_when_the_feature_cannot_be_reached() {
        // One scatter and four reels left cannot make three by reel 5 unless
        // more land, so nothing is held back yet.
        assert_eq!(anticipating_reels(&[1, 0, 0, 0, 0], 3, 4), vec![false; 5]);
        // And a board with no scatters at all never anticipates.
        assert_eq!(anticipating_reels(&[0; 5], 3, 4), vec![false; 5]);
    }

    #[test]
    fn anticipation_continues_while_the_count_keeps_climbing() {
        // Scatters on reels 1 and 2 hold reel 3; a third scatter there means
        // the feature has already triggered, and reels 4 and 5 are chasing a
        // bigger award, so they keep anticipating.
        let flags = anticipating_reels(&[1, 1, 1, 0, 0], 3, 4);
        assert_eq!(flags, vec![false, false, true, true, true]);
    }

    #[test]
    fn anticipation_is_capped() {
        // Without a cap a scatter-heavy board would stretch every remaining
        // reel and turn a spin into a slideshow.
        let flags = anticipating_reels(&[1, 1, 1, 1, 1], 3, 2);
        assert_eq!(flags.iter().filter(|held| **held).count(), 2);
    }

    #[test]
    fn the_payout_counter_starts_at_zero_and_ends_on_target() {
        let mut counter = PayoutCounter::new(1234, 1.0);
        assert_eq!(counter.value(), 0);

        let mut finished = false;
        for _ in 0..120 {
            finished |= counter.tick(1.0 / 60.0);
        }

        assert!(finished);
        assert_eq!(counter.value(), 1234);
    }

    #[test]
    fn the_payout_counter_finishes_exactly_once() {
        let mut counter = PayoutCounter::new(10, 1.0);
        let mut finishes = 0;
        for _ in 0..200 {
            if counter.tick(1.0 / 60.0) {
                finishes += 1;
            }
        }

        assert_eq!(finishes, 1);
    }
}
