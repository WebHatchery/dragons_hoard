//! Spin presentation: the phase machine that turns an already-decided outcome
//! into a reel animation, a win count-up, and the pause between free spins.
//!
//! Nothing here decides anything. `engine::spin` picks the stops the moment the
//! player commits, and these types only *reveal* that decision over time — the
//! reels are told where to land before they start moving. That is what keeps the
//! maths (and the Monte-Carlo sim, which skips this module entirely) honest.

use macroquad_toolkit::math::{ease_out_cubic, ease_out_quad};
use macroquad_toolkit::timing::Timer;

/// How long reel 1 spins for. Each later reel adds [`REEL_STAGGER`].
const BASE_SPIN_TIME: f32 = 0.62;
/// Extra spin time per reel, which is what produces the left-to-right stop.
const REEL_STAGGER: f32 = 0.26;
/// Whole strip revolutions a reel travels before landing, plus half a
/// revolution per reel so later reels visibly spin faster rather than longer.
const SPIN_REVOLUTIONS: f32 = 2.0;
/// Seconds the win count-up takes.
const PAYOUT_TIME: f32 = 0.75;
/// A reel slower than this many symbols per second is drawn crisply rather than
/// blurred.
const BLUR_SPEED: f32 = 6.0;

/// One reel travelling from its previous stop to its decided stop.
///
/// Position is a closed-form function of elapsed time rather than an integrated
/// velocity, so the reel lands on exactly the right symbol no matter how the
/// frame times fall.
#[derive(Debug, Clone)]
pub struct ReelAnimation {
    start: f32,
    travel: f32,
    duration: f32,
    elapsed: f32,
    strip_len: f32,
}

impl ReelAnimation {
    /// `time_scale` comes from the player's spin-speed preference. It shortens
    /// the reveal without touching `travel`, so a Turbo spin lands on exactly
    /// the same symbol as a Normal one — the outcome was decided before either
    /// started (§8.2).
    pub fn new(strip_len: usize, from: usize, to: usize, index: usize, time_scale: f32) -> Self {
        let len = strip_len.max(1);
        let from = from % len;
        let to = to % len;
        let delta = (to + len - from) % len;
        let revolutions = SPIN_REVOLUTIONS + index as f32 * 0.5;
        // Never zero: a duration of 0 would land the reels in the frame they
        // start and skip every ReelStopped event.
        let scale = time_scale.clamp(0.05, 4.0);

        Self {
            start: from as f32,
            travel: revolutions * len as f32 + delta as f32,
            duration: (BASE_SPIN_TIME + index as f32 * REEL_STAGGER) * scale,
            elapsed: 0.0,
            strip_len: len as f32,
        }
    }

    /// Fractional strip position of the reel's top visible cell.
    pub fn position(&self) -> f32 {
        let eased = ease_out_cubic(self.progress());
        (self.start + self.travel * eased).rem_euclid(self.strip_len)
    }

    /// Symbols per second, used to decide how hard to blur the reel.
    pub fn speed(&self) -> f32 {
        if self.settled() || self.duration <= 0.0 {
            return 0.0;
        }
        // Derivative of the cubic ease-out, scaled by the travel distance.
        let t = self.progress();
        3.0 * (1.0 - t).powi(2) * self.travel / self.duration
    }

    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        }
    }

    pub fn settled(&self) -> bool {
        self.elapsed >= self.duration
    }

    fn tick(&mut self, dt: f32) {
        self.elapsed = (self.elapsed + dt).min(self.duration);
    }
}

/// All five reels mid-flight.
#[derive(Debug, Clone)]
pub struct ReelSpinner {
    reels: Vec<ReelAnimation>,
    announced: usize,
}

impl ReelSpinner {
    pub fn new(strip_lengths: &[usize], from: &[usize], to: &[usize], time_scale: f32) -> Self {
        let reels = strip_lengths
            .iter()
            .enumerate()
            .map(|(index, len)| {
                ReelAnimation::new(
                    *len,
                    from.get(index).copied().unwrap_or(0),
                    to.get(index).copied().unwrap_or(0),
                    index,
                    time_scale,
                )
            })
            .collect();

        Self {
            reels,
            announced: 0,
        }
    }

    /// Advances every reel and returns the indices that came to rest this tick,
    /// in stop order.
    pub fn tick(&mut self, dt: f32) -> Vec<usize> {
        for reel in &mut self.reels {
            reel.tick(dt);
        }

        let mut stopped = Vec::new();
        while self.announced < self.reels.len() && self.reels[self.announced].settled() {
            stopped.push(self.announced);
            self.announced += 1;
        }
        stopped
    }

    pub fn all_settled(&self) -> bool {
        self.reels.iter().all(ReelAnimation::settled)
    }

    pub fn position(&self, reel: usize) -> f32 {
        self.reels
            .get(reel)
            .map(ReelAnimation::position)
            .unwrap_or(0.0)
    }

    /// True while the reel is moving fast enough to warrant motion blur.
    pub fn is_blurred(&self, reel: usize) -> bool {
        self.reels
            .get(reel)
            .is_some_and(|reel| reel.speed() > BLUR_SPEED)
    }

    pub fn is_moving(&self, reel: usize) -> bool {
        self.reels.get(reel).is_some_and(|reel| !reel.settled())
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reel_lands_exactly_on_its_target_stop() {
        for target in [0usize, 1, 17, 39] {
            let mut reel = ReelAnimation::new(40, 7, target, 2, 1.0);
            for _ in 0..600 {
                reel.tick(1.0 / 60.0);
            }

            assert!(reel.settled());
            assert_eq!(reel.position().round() as usize % 40, target);
        }
    }

    #[test]
    fn a_reel_lands_on_target_even_from_a_ragged_frame_rate() {
        let mut reel = ReelAnimation::new(40, 3, 22, 0, 1.0);
        for dt in [0.004, 0.1, 0.017, 0.05, 0.2, 0.033, 0.4, 0.016] {
            reel.tick(dt);
        }

        assert!(reel.settled());
        assert_eq!(reel.position().round() as usize % 40, 22);
    }

    #[test]
    fn a_reel_that_does_not_move_still_spins_a_full_revolution() {
        let reel = ReelAnimation::new(40, 12, 12, 0, 1.0);
        assert!(reel.travel >= 40.0, "travel was {}", reel.travel);
    }

    #[test]
    fn reels_stop_left_to_right_exactly_once_each() {
        let lengths = vec![40; 5];
        let mut spinner = ReelSpinner::new(&lengths, &[0; 5], &[10, 20, 30, 5, 15], 1.0);

        let mut order = Vec::new();
        for _ in 0..400 {
            order.extend(spinner.tick(1.0 / 60.0));
        }

        assert_eq!(order, vec![0, 1, 2, 3, 4]);
        assert!(spinner.all_settled());
        // Already-settled reels are never announced twice.
        assert!(spinner.tick(1.0).is_empty());
    }

    #[test]
    fn a_spinning_reel_is_blurred_early_and_crisp_at_rest() {
        let mut reel = ReelAnimation::new(40, 0, 20, 0, 1.0);
        reel.tick(0.01);
        assert!(reel.speed() > BLUR_SPEED);

        for _ in 0..200 {
            reel.tick(1.0 / 60.0);
        }
        assert_eq!(reel.speed(), 0.0);
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
