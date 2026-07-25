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
/// Strip revolutions a reel travels before landing, plus one more every second
/// reel so later reels visibly spin faster rather than merely longer.
///
/// This **must** be a whole number of revolutions. It was `2.0 + index * 0.5`,
/// which gave reels 2 and 4 two-and-a-half turns — landing them exactly half a
/// strip from their decided stop, so they popped twenty symbols the instant they
/// settled and the resting draw took over. Every landing test happened to use an
/// even reel index, so it went unseen until one looped over reel 1.
const SPIN_REVOLUTIONS: usize = 2;
/// Seconds the win count-up takes.
const PAYOUT_TIME: f32 = 0.75;
/// A reel slower than this many symbols per second is drawn crisply rather than
/// blurred.
const BLUR_SPEED: f32 = 6.0;
/// How much longer an anticipating reel turns for (§5.11). The whole point is
/// that it feels like an age.
const ANTICIPATION_STRETCH: f32 = 2.6;
/// Depth of the landing bounce, in symbols. Small: the reel should settle, not
/// wobble.
const BOUNCE_DEPTH: f32 = 0.16;
/// Fraction of a reel's travel spent bouncing at the end.
const BOUNCE_TAIL: f32 = 0.18;

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
    /// This reel is being held back because the ones already stopped could
    /// still add up to a feature.
    anticipating: bool,
}

impl ReelAnimation {
    /// `time_scale` comes from the player's spin-speed preference. It shortens
    /// the reveal without touching `travel`, so a Turbo spin lands on exactly
    /// the same symbol as a Normal one — the outcome was decided before either
    /// started (§8.2).
    pub fn new(
        strip_len: usize,
        from: usize,
        to: usize,
        index: usize,
        time_scale: f32,
        anticipating: bool,
    ) -> Self {
        let len = strip_len.max(1);
        let from = from % len;
        let to = to % len;
        let delta = (to + len - from) % len;
        let revolutions = SPIN_REVOLUTIONS + index / 2;
        // Never zero: a duration of 0 would land the reels in the frame they
        // start and skip every ReelStopped event.
        let scale = time_scale.clamp(0.05, 4.0);

        let stretch = if anticipating {
            ANTICIPATION_STRETCH
        } else {
            1.0
        };

        Self {
            start: from as f32,
            travel: (revolutions * len + delta) as f32,
            duration: (BASE_SPIN_TIME + index as f32 * REEL_STAGGER) * scale * stretch,
            elapsed: 0.0,
            strip_len: len as f32,
            anticipating,
        }
    }

    pub fn is_anticipating(&self) -> bool {
        self.anticipating
    }

    /// A damped wobble over the last stretch of the travel, in symbols.
    ///
    /// It is exactly zero at `t == 1`, so the reel still comes to rest on the
    /// symbol it was told to — the bounce is presentation, never a change of
    /// mind. A test pins that.
    fn bounce(&self, t: f32) -> f32 {
        if t <= 1.0 - BOUNCE_TAIL {
            return 0.0;
        }
        let u = ((t - (1.0 - BOUNCE_TAIL)) / BOUNCE_TAIL).clamp(0.0, 1.0);
        let decay = 1.0 - u;
        BOUNCE_DEPTH * decay * (u * std::f32::consts::PI * 2.0).sin()
    }

    /// Fractional strip position of the reel's top visible cell.
    pub fn position(&self) -> f32 {
        let t = self.progress();
        let eased = ease_out_cubic(t);
        (self.start + self.travel * eased + self.bounce(t)).rem_euclid(self.strip_len)
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
    /// `anticipating` marks the reels to hold back; see
    /// [`anticipating_reels`].
    pub fn new(
        strip_lengths: &[usize],
        from: &[usize],
        to: &[usize],
        time_scale: f32,
        anticipating: &[bool],
    ) -> Self {
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
                    anticipating.get(index).copied().unwrap_or(false),
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

    /// Symbols this reel covers in a single 60 Hz frame — the distance the
    /// motion blur has to smear over. Clamped so a fast reel streaks rather
    /// than dissolving into a flat band.
    pub fn blur_symbols(&self, reel: usize) -> f32 {
        self.reels
            .get(reel)
            .map_or(0.0, |reel| (reel.speed() / 60.0).min(1.4))
    }

    /// True while this reel is both held back and still turning — what the UI
    /// draws the anticipation frame around.
    pub fn is_anticipating(&self, reel: usize) -> bool {
        self.reels
            .get(reel)
            .is_some_and(|reel| reel.is_anticipating() && !reel.settled())
    }
}

/// Which reels should be held back, given the grid this spin is going to land.
///
/// The rule is the one every physical cabinet uses: once the reels that have
/// already stopped carry `trigger - 1` scatters, the next reel is the one that
/// could complete the feature, so it is made to take its time. Anticipation is
/// therefore never a lie — it only ever fires when the feature is genuinely
/// still live, and the outcome was decided before the reels started (§8.2).
///
/// Every reel still to land is held back while the count is reachable, not just
/// the next one — with two scatters showing and three reels to go, any of the
/// three could be the third scatter, and that is exactly the near-miss a
/// cabinet draws out. `max_anticipating` caps it so a scatter-rich board cannot
/// turn one spin into a slideshow.
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
    /// A respin landed in an open Dragon's Wrath round (§5.12).
    HoldSpinRespun,
    /// The round ended and has been credited.
    HoldSpinFinished(super::holdspin::HoldSpinOutcome),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reel_lands_exactly_on_its_target_stop() {
        // Every reel index, deliberately: this test used to pass only index 2,
        // and the odd-indexed reels were landing half a strip out.
        for index in 0..5 {
            for target in [0usize, 1, 17, 39] {
                let mut reel = ReelAnimation::new(40, 7, target, index, 1.0, false);
                for _ in 0..900 {
                    reel.tick(1.0 / 60.0);
                }

                assert!(reel.settled());
                assert_eq!(
                    reel.position().round() as usize % 40,
                    target,
                    "reel {} missed its stop",
                    index
                );
            }
        }
    }

    #[test]
    fn every_reel_travels_a_whole_number_of_revolutions() {
        // The invariant behind the bug above, stated directly.
        for index in 0..5 {
            let reel = ReelAnimation::new(40, 7, 7, index, 1.0, false);
            assert_eq!(
                reel.travel % 40.0,
                0.0,
                "reel {} travels a fractional strip",
                index
            );
        }
    }

    #[test]
    fn a_reel_lands_on_target_even_from_a_ragged_frame_rate() {
        let mut reel = ReelAnimation::new(40, 3, 22, 0, 1.0, false);
        for dt in [0.004, 0.1, 0.017, 0.05, 0.2, 0.033, 0.4, 0.016] {
            reel.tick(dt);
        }

        assert!(reel.settled());
        assert_eq!(reel.position().round() as usize % 40, 22);
    }

    #[test]
    fn a_reel_that_does_not_move_still_spins_a_full_revolution() {
        let reel = ReelAnimation::new(40, 12, 12, 0, 1.0, false);
        assert!(reel.travel >= 40.0, "travel was {}", reel.travel);
    }

    #[test]
    fn reels_stop_left_to_right_exactly_once_each() {
        let lengths = vec![40; 5];
        let mut spinner =
            ReelSpinner::new(&lengths, &[0; 5], &[10, 20, 30, 5, 15], 1.0, &[false; 5]);

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
        let mut reel = ReelAnimation::new(40, 0, 20, 0, 1.0, false);
        reel.tick(0.01);
        assert!(reel.speed() > BLUR_SPEED);

        for _ in 0..200 {
            reel.tick(1.0 / 60.0);
        }
        assert_eq!(reel.speed(), 0.0);
    }

    #[test]
    fn the_landing_bounce_never_changes_where_a_reel_stops() {
        // The bounce is presentation. If it could shift the resting position by
        // even one symbol it would be quietly rewriting a decided outcome.
        for target in [0usize, 3, 19, 39] {
            let mut reel = ReelAnimation::new(40, 11, target, 1, 1.0, false);
            for _ in 0..900 {
                reel.tick(1.0 / 60.0);
            }
            assert!(reel.settled());
            assert_eq!(reel.position().round() as usize % 40, target);
        }
    }

    #[test]
    fn the_bounce_actually_happens() {
        // Guards the opposite failure: a bounce tuned to nothing is dead code.
        let mut reel = ReelAnimation::new(40, 0, 20, 0, 1.0, false);
        let mut peak: f32 = 0.0;
        for _ in 0..600 {
            reel.tick(1.0 / 60.0);
            peak = peak.max(reel.bounce(reel.progress()).abs());
        }
        assert!(peak > 0.02, "the reel never wobbled (peak {})", peak);
    }

    #[test]
    fn an_anticipating_reel_turns_for_longer() {
        let plain = ReelAnimation::new(40, 0, 10, 2, 1.0, false);
        let held = ReelAnimation::new(40, 0, 10, 2, 1.0, true);

        assert!(held.duration > plain.duration * 2.0);
        assert!(held.is_anticipating());
        assert!(!plain.is_anticipating());
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
