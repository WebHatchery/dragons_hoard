//! Machine profiles, measured live (§5.17).
//!
//! # Why this is measured and not written down
//!
//! The catalog offers four cabinets that are genuinely different games — hit
//! frequencies from 0.258 to 0.622, two evaluation models, one that cascades —
//! and until now the player chose between them on one line of blurb.
//!
//! The obvious fix is to bake the figures into JSON. The problem is that they
//! are *derived*: every strip edit, paytable retune or feature change makes them
//! wrong, silently, and nothing in the game would notice. The Feature Buy price
//! (§5.13) gets away with being written down because a price is a design choice
//! that a test can then check. A hit frequency is not a choice; it is an
//! observation.
//!
//! So it is observed. The profiler runs the **real headless spin path** against
//! the cabinet in front of the player and reports what it actually does. There
//! is nothing to drift from, because there is nothing written down.
//!
//! # It runs in chunks, and never on the player's session
//!
//! Twenty thousand spins would visibly hitch a frame, so the profiler does a few
//! hundred per frame and reports progress. More importantly it works on a
//! **scratch session with its own seed** — profiling must not consume a single
//! draw from the player's RNG, or looking at the machine picker would change the
//! spins that came after it. A test asserts exactly that.

use crate::data::GameData;
use crate::engine::sim::{RoundStats, BAND_COUNT};
use crate::state::GameSession;

/// Rounds a full profile is measured over. Enough for hit frequency and the
/// bands to settle; the volatility index is noisier and is reported as a band
/// rather than to three decimals.
pub const PROFILE_ROUNDS: u64 = 20_000;
/// Rounds per frame. Chosen so a profile finishes in about a second at 60fps
/// without any single frame doing enough work to be felt.
const ROUNDS_PER_STEP: u64 = 400;
/// Seed the scratch session runs on. Fixed, so the same cabinet profiles to the
/// same figures every time a player looks at it — a number that wobbled between
/// viewings would read as a fault.
///
/// This was originally the golden-ratio constant, which is the one value
/// `SeededRng::new` xors to a zero state — an xorshift fixed point that returns
/// 0 forever. Every cabinet profiled to the same grid on every round. The
/// toolkit now guards against it, and this is an ordinary number regardless.
const PROFILE_SEED: u64 = 0x5CA1_AB1E_D00D;
/// Balance the scratch session is topped up to, so a losing run cannot stall it.
const SCRATCH_BANKROLL: i64 = 1_000_000_000;

/// What a finished profile says about a cabinet.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineProfile {
    /// Measured return per round, **excluding progressives** — see `round`.
    pub rtp: f64,
    pub hit_frequency: f64,
    pub volatility: f64,
    /// Share of rounds in each band of `sim::BANDS`, summing to 1.
    pub bands: [f64; BAND_COUNT],
    /// Largest single round, in multiples of total bet.
    pub best_round: f64,
    pub rounds: u64,
}

impl MachineProfile {
    /// A word for the volatility index, because the number alone means nothing
    /// to anyone who has not seen another one to compare it against.
    pub fn volatility_label(&self) -> &'static str {
        match self.volatility {
            v if v < 3.0 => "Low",
            v if v < 6.0 => "Medium",
            v if v < 12.0 => "High",
            _ => "Very high",
        }
    }
}

/// Measures one cabinet a few hundred rounds at a time.
pub struct Profiler {
    session: GameSession,
    stats: RoundStats,
    hits: u64,
    best_round: f64,
    target: u64,
}

impl Profiler {
    pub fn new(data: &GameData) -> Self {
        Self {
            session: GameSession::new(data, PROFILE_SEED),
            stats: RoundStats::default(),
            hits: 0,
            best_round: 0.0,
            target: PROFILE_ROUNDS,
        }
    }

    /// 0.0 to 1.0.
    pub fn progress(&self) -> f32 {
        (self.stats.rounds as f32 / self.target as f32).clamp(0.0, 1.0)
    }

    pub fn is_finished(&self) -> bool {
        self.stats.rounds >= self.target
    }

    /// Run one frame's worth. Returns the profile on the step that completes it.
    pub fn step(&mut self, data: &GameData) -> Option<MachineProfile> {
        for _ in 0..ROUNDS_PER_STEP {
            if self.is_finished() {
                break;
            }
            self.round(data);
        }
        self.is_finished().then(|| self.finish())
    }

    /// One paid spin and everything it led to.
    fn round(&mut self, data: &GameData) {
        self.session.balance = SCRATCH_BANKROLL;
        self.session.celebrations.clear();

        let total_bet = self.session.total_bet(data);
        let Ok(resolution) = self.session.spin(data) else {
            // Nothing should be able to refuse a topped-up scratch session, but
            // counting the round anyway keeps `is_finished` reachable rather
            // than spinning forever on a data set that can.
            self.stats.rounds = self.target;
            return;
        };

        // Progressives are left out of the measured return, for the same reason
        // the bet-ladder sim test excludes them (§5.6): a jackpot either lands
        // in a 20,000-round sample or it does not, and either way the figure it
        // produces says more about that one event than about the cabinet. The
        // profile reports what the reels do; the ladder above them advertises
        // itself. Without this, Dragon's Hoard profiled at 90.8% against a true
        // 96.1%, which is worse than saying nothing.
        let mut credits = resolution.total_credits() - resolution.jackpot_credits();
        // Free spins cost nothing, so they are part of the return on the paid
        // spin that bought them rather than rounds of their own.
        while self.session.in_free_spins() {
            let Ok(free) = self.session.spin(data) else {
                break;
            };
            credits += free.total_credits() - free.jackpot_credits();
        }

        if credits > 0 {
            self.hits += 1;
        }
        self.stats.record(credits, total_bet);
        self.best_round = self
            .best_round
            .max(credits as f64 / total_bet.max(1) as f64);
    }

    fn finish(&self) -> MachineProfile {
        let rounds = self.stats.rounds.max(1);
        MachineProfile {
            rtp: self.stats.mean_return(),
            hit_frequency: self.hits as f64 / rounds as f64,
            volatility: self.stats.volatility(),
            bands: self.stats.band_shares(),
            best_round: self.best_round,
            rounds: self.stats.rounds,
        }
    }
}

impl ProfileBook {
    pub fn tier(&self, machine_id: &str, tier: usize) -> Option<&TierProfile> {
        self.tiers
            .iter()
            .find(|((id, index), _)| id == machine_id && *index == tier)
            .map(|(_, profile)| profile)
    }

    pub fn tier_progress(&self, machine_id: &str, tier: usize) -> f32 {
        match &self.tier_running {
            Some(((id, index), profiler)) if id == machine_id && *index == tier => {
                profiler.progress()
            }
            _ => 0.0,
        }
    }

    /// Ask for a tier to be measured. Cheap to call every frame.
    ///
    /// Tier profiling runs on its own slot rather than sharing the machine
    /// profiler's, because the two are looked at on different screens and
    /// queueing one behind the other would leave a panel blank for no reason.
    pub fn request_tier(&mut self, machine_id: &str, tier: usize, data: &GameData) {
        if self.tier(machine_id, tier).is_some() || self.tier_running.is_some() {
            return;
        }
        self.tier_running = Some(((machine_id.to_owned(), tier), TierProfiler::new(data, tier)));
    }

    pub fn shape(&self, machine_id: &str, shape: usize) -> Option<&ShapeProfile> {
        self.shapes
            .iter()
            .find(|((id, index), _)| id == machine_id && *index == shape)
            .map(|(_, profile)| profile)
    }

    /// Ask for a shape to be measured. Cheap to call every frame.
    pub fn request_shape(&mut self, machine_id: &str, shape: usize, data: &GameData) {
        if self.shape(machine_id, shape).is_some() || self.shape_running.is_some() {
            return;
        }
        self.shape_running = Some((
            (machine_id.to_owned(), shape),
            ShapeProfiler::new(data, shape),
        ));
    }

    pub fn step_shape(&mut self, machine_id: &str, data: &GameData) {
        let Some(((id, shape), profiler)) = self.shape_running.as_mut() else {
            return;
        };
        if id != machine_id {
            return;
        }
        if let Some(profile) = profiler.step(data) {
            let key = (id.clone(), *shape);
            self.shapes.push((key, profile));
            self.shape_running = None;
        }
    }

    pub fn step_tier(&mut self, machine_id: &str, data: &GameData) {
        let Some(((id, tier), profiler)) = self.tier_running.as_mut() else {
            return;
        };
        if id != machine_id {
            return;
        }
        if let Some(profile) = profiler.step(data) {
            let key = (id.clone(), *tier);
            self.tiers.push((key, profile));
            self.tier_running = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MACHINES;

    /// Run a profile to completion, as the UI would over many frames.
    fn profile(data: &GameData) -> MachineProfile {
        let mut profiler = Profiler::new(data);
        for _ in 0..10_000 {
            if let Some(profile) = profiler.step(data) {
                return profile;
            }
        }
        panic!("the profiler never finished");
    }

    #[test]
    fn every_cabinet_profiles_to_something_plausible() {
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let profile = profile(&data);

            assert_eq!(profile.rounds, PROFILE_ROUNDS);
            assert!(
                (0.5..1.6).contains(&profile.rtp),
                "{} profiled at RTP {:.3}",
                machine.id,
                profile.rtp
            );
            assert!(
                (0.05..0.95).contains(&profile.hit_frequency),
                "{} profiled at hit {:.3}",
                machine.id,
                profile.hit_frequency
            );
            assert!(profile.volatility > 0.0);
            assert!(profile.best_round > 0.0);
        }
    }

    #[test]
    fn the_bands_account_for_every_round() {
        // A band table that did not sum to one would mean rounds falling
        // through the classifier, and the bar chart would quietly under-report.
        let data = GameData::load().unwrap();
        let profile = profile(&data);

        let total: f64 = profile.bands.iter().sum();
        assert!(
            (total - 1.0).abs() < 1e-9,
            "the bands sum to {:.6}, not 1",
            total
        );
    }

    #[test]
    fn a_profile_agrees_with_the_long_run_sim() {
        // The profiler and the batch sim are two different loops over the same
        // engine. If they disagreed, one of them would be lying to somebody.
        let data = GameData::load().unwrap();
        let profile = profile(&data);
        let batch = crate::engine::sim::run(
            &data,
            crate::engine::sim::SimConfig {
                spins: PROFILE_ROUNDS,
                line_bet_index: 0,
                seed: PROFILE_SEED,
            },
        );

        assert!(
            (profile.hit_frequency - batch.hit_frequency()).abs() < 0.02,
            "profiler {:.3} against batch {:.3}",
            profile.hit_frequency,
            batch.hit_frequency()
        );
        assert!(
            (profile.volatility - batch.stats.volatility()).abs() < 1.5,
            "profiler {:.2} against batch {:.2}",
            profile.volatility,
            batch.stats.volatility()
        );
    }

    #[test]
    fn profiling_does_not_touch_the_players_session() {
        // The whole reason the profiler owns a scratch session. If it drew from
        // the live RNG, opening the machine picker would change the spins that
        // came after it — a save-and-reload divergence the player could see and
        // nobody could explain.
        let data = GameData::load().unwrap();

        let mut untouched = GameSession::new(&data, 4_242);
        let mut watched = GameSession::new(&data, 4_242);

        let mut profiler = Profiler::new(&data);
        for _ in 0..10 {
            profiler.step(&data);
        }

        for _ in 0..50 {
            untouched.balance = 1_000_000;
            watched.balance = 1_000_000;
            untouched.celebrations.clear();
            watched.celebrations.clear();
            let a = untouched.spin(&data).unwrap();
            let b = watched.spin(&data).unwrap();
            assert_eq!(a.result.grid, b.result.grid);
        }
        assert_eq!(untouched.balance, watched.balance);
    }

    #[test]
    fn the_same_cabinet_profiles_the_same_way_twice() {
        // A figure that wobbled between viewings would read as a fault rather
        // than as a measurement.
        let data = GameData::load().unwrap();
        assert_eq!(profile(&data), profile(&data));
    }

    #[test]
    fn the_catalog_spans_more_than_one_volatility() {
        // If every cabinet profiled the same, the picker would be telling the
        // player about a choice that does not exist.
        let mut labels: Vec<&str> = Vec::new();
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            labels.push(profile(&data).volatility_label());
        }
        labels.sort_unstable();
        labels.dedup();
        assert!(labels.len() > 1, "every cabinet profiled as {:?}", labels);
    }
}

/// Every cabinet's profile, measured on demand.
///
/// Owned by the orchestrator rather than the session, because it belongs to the
/// *catalog* rather than to any one machine's play — and because a profile must
/// survive switching cabinets, or walking back and forth would re-measure both
/// every time.
#[derive(Default)]
pub struct ProfileBook {
    finished: Vec<(String, MachineProfile)>,
    /// At most one profiler runs at a time. Measuring four cabinets at once
    /// would quadruple the per-frame cost for no benefit — the player is
    /// looking at one row at a time anyway.
    running: Option<(String, Profiler)>,
    /// Buy tiers already measured, keyed by machine and tier index (§5.22).
    tiers: Vec<((String, usize), TierProfile)>,
    tier_running: Option<((String, usize), TierProfiler)>,
    /// Free-spin shapes already measured, keyed by machine and shape (§5.65).
    shapes: Vec<((String, usize), ShapeProfile)>,
    shape_running: Option<((String, usize), ShapeProfiler)>,
}

impl ProfileBook {
    pub fn get(&self, machine_id: &str) -> Option<&MachineProfile> {
        self.finished
            .iter()
            .find(|(id, _)| id == machine_id)
            .map(|(_, profile)| profile)
    }

    /// How far the profiler has got on a cabinet, 0.0 to 1.0.
    pub fn progress(&self, machine_id: &str) -> f32 {
        match &self.running {
            Some((id, profiler)) if id == machine_id => profiler.progress(),
            _ => 0.0,
        }
    }

    /// Ask for a cabinet to be measured. Cheap to call every frame — an already
    /// measured or already running machine is ignored.
    pub fn request(&mut self, machine_id: &str, data: &GameData) {
        if self.get(machine_id).is_some() || self.running.is_some() {
            return;
        }
        self.running = Some((machine_id.to_owned(), Profiler::new(data)));
    }

    /// Advance the running profiler by one frame's worth.
    ///
    /// `data` must be the machine being profiled, which is why `request` takes
    /// it too: the book cannot load cabinets itself without embedding the
    /// catalog, and the caller already has every `GameData` it needs.
    pub fn step(&mut self, machine_id: &str, data: &GameData) {
        let Some((id, profiler)) = self.running.as_mut() else {
            return;
        };
        if id != machine_id {
            return;
        }
        if let Some(profile) = profiler.step(data) {
            let id = id.clone();
            self.finished.push((id, profile));
            self.running = None;
        }
    }
}

/// Rounds a tier profile is measured over. Fewer than a machine profile because
/// a bought feature is one event rather than a spin, and each one costs several
/// hundred internal spins to play out.
pub const TIER_ROUNDS: u64 = 4_000;
const TIER_BUYS_PER_STEP: u64 = 60;

/// What a finished tier profile says about a purchase (§5.22).
#[derive(Debug, Clone, PartialEq)]
pub struct TierProfile {
    /// Mean return as a share of the price. Sits near the machine's RTP,
    /// because that is exactly what the price was set to make it (§5.13).
    pub rtp: f64,
    /// **Share of buys that hand back less than they cost.**
    ///
    /// The number a purchase actually turns on, and the one no real cabinet
    /// shows. A tier can be priced perfectly fairly and still lose money most
    /// times it is bought, because the distribution is skewed — a few large
    /// returns carry the average while the median sits below the price. Nothing
    /// is wrong with that; it is simply what "fair" means for a bet with a long
    /// tail, and a player deserves to know it before spending 168x.
    pub below_cost: f64,
    pub bands: [f64; BAND_COUNT],
    /// Largest return seen, as a multiple of the price.
    pub best: f64,
    pub buys: u64,
}

/// Measures one Feature Buy tier a few purchases at a time.
pub struct TierProfiler {
    session: GameSession,
    tier: usize,
    stats: RoundStats,
    below_cost: u64,
    best: f64,
    target: u64,
}

impl TierProfiler {
    pub fn new(data: &GameData, tier: usize) -> Self {
        Self {
            // A different seed per tier, so three tiers on one cabinet are not
            // three views of the same stream of luck.
            session: GameSession::new(data, PROFILE_SEED ^ (tier as u64 + 1)),
            tier,
            stats: RoundStats::default(),
            below_cost: 0,
            best: 0.0,
            target: TIER_ROUNDS,
        }
    }

    pub fn progress(&self) -> f32 {
        (self.stats.rounds as f32 / self.target as f32).clamp(0.0, 1.0)
    }

    pub fn is_finished(&self) -> bool {
        self.stats.rounds >= self.target
    }

    pub fn step(&mut self, data: &GameData) -> Option<TierProfile> {
        for _ in 0..TIER_BUYS_PER_STEP {
            if self.is_finished() {
                break;
            }
            self.buy(data);
        }
        self.is_finished().then(|| self.finish())
    }

    /// One purchase, played out to the end.
    fn buy(&mut self, data: &GameData) {
        self.session.balance = SCRATCH_BANKROLL;
        self.session.celebrations.clear();

        let before = self.session.balance;
        let Ok(purchase) = self.session.buy_feature(self.tier, data) else {
            // Nothing should refuse a topped-up scratch session, but ending the
            // measurement beats spinning on a menu that cannot be bought.
            self.stats.rounds = self.target;
            return;
        };

        while self.session.in_free_spins() {
            if self.session.spin(data).is_err() {
                break;
            }
        }
        self.session.auto_play_bonus(data);
        self.session.auto_play_holdspin(data);

        let returned = self.session.balance - (before - purchase.price);
        if returned < purchase.price {
            self.below_cost += 1;
        }
        self.stats.record(returned, purchase.price);
        self.best = self
            .best
            .max(returned as f64 / purchase.price.max(1) as f64);
    }

    fn finish(&self) -> TierProfile {
        let buys = self.stats.rounds.max(1);
        TierProfile {
            rtp: self.stats.mean_return(),
            below_cost: self.below_cost as f64 / buys as f64,
            bands: self.stats.band_shares(),
            best: self.best,
            buys: self.stats.rounds,
        }
    }
}

#[cfg(test)]
mod tier_tests {
    use super::*;
    use crate::data::MACHINES;

    fn profile(data: &GameData, tier: usize) -> TierProfile {
        let mut profiler = TierProfiler::new(data, tier);
        for _ in 0..10_000 {
            if let Some(profile) = profiler.step(data) {
                return profile;
            }
        }
        panic!("the tier profiler never finished");
    }

    #[test]
    fn every_tier_on_every_cabinet_profiles_near_its_price() {
        // The tier profiler and `simulate_buys` are two loops over the same
        // purchase. Both must land on the machine's target, because that is
        // what the price was chosen to make true (§5.13).
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let target = data.featurebuy.target_rtp_permille as f64 / 1000.0;

            for (tier, def) in data.featurebuy.tiers.iter().enumerate() {
                let profile = profile(&data, tier);
                assert_eq!(profile.buys, TIER_ROUNDS);
                assert!(
                    (profile.rtp - target).abs() < 0.30,
                    "{}/{} profiled at {:.3} against {:.3}",
                    machine.id,
                    def.id,
                    profile.rtp,
                    target
                );
            }
        }
    }

    #[test]
    fn a_fairly_priced_buy_still_loses_most_of_the_time() {
        // The point of the whole section. A tier priced at its expected value
        // hands back less than it cost on most purchases, because a few large
        // returns carry the average. If this ever came out near zero the
        // headline figure would be worthless and something would be wrong with
        // either the pricing or the measurement.
        let data = GameData::load().unwrap();
        for (tier, def) in data.featurebuy.tiers.iter().enumerate() {
            let profile = profile(&data, tier);
            assert!(
                (0.2..0.95).contains(&profile.below_cost),
                "{} comes back under the price {:.3} of the time",
                def.id,
                profile.below_cost
            );
        }
    }

    #[test]
    fn the_bands_account_for_every_buy() {
        let data = GameData::load().unwrap();
        let profile = profile(&data, 0);
        assert!((profile.bands.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(profile.best > 0.0);
    }

    #[test]
    fn each_tier_is_measured_on_its_own_stream() {
        // Sharing one seed across three tiers would make them three views of the
        // same run of luck, and the comparison between them meaningless.
        let data = GameData::load().unwrap();
        let first = profile(&data, 0);
        let second = profile(&data, 1);
        assert_ne!(first.bands, second.bands);
    }

    #[test]
    fn profiling_a_tier_does_not_touch_the_players_session() {
        // Same rule as the machine profiler: opening the buy menu must not
        // consume a draw the player's next spin was going to use.
        let data = GameData::load().unwrap();
        let mut untouched = GameSession::new(&data, 7_777);
        let mut watched = GameSession::new(&data, 7_777);

        let mut profiler = TierProfiler::new(&data, 0);
        for _ in 0..3 {
            profiler.step(&data);
        }

        for _ in 0..40 {
            untouched.balance = 1_000_000;
            watched.balance = 1_000_000;
            untouched.celebrations.clear();
            watched.celebrations.clear();
            let a = untouched.spin(&data).unwrap();
            let b = watched.spin(&data).unwrap();
            assert_eq!(a.result.grid, b.result.grid);
        }
        assert_eq!(untouched.balance, watched.balance);
    }
}

/// Runs measured per shape. Fewer than a tier needs: a free-spin run is
/// several internal spins and both shapes are being compared to each other
/// rather than to an absolute claim, so the noise cancels.
pub const SHAPE_RUNS: u64 = 2_000;
const SHAPE_RUNS_PER_STEP: u64 = 40;

/// What running the feature one way actually feels like (§5.65).
///
/// §5.64 offers a choice between shapes that are worth the same, and proves it
/// with arithmetic. Arithmetic is not reassurance: a player looking at "15 spins
/// at ×2" beside "6 spins at ×5" has been told two numbers and asked to trust a
/// third they cannot see. This is the third one, measured.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeProfile {
    /// Mean return of a run, in total-bet multiples.
    pub mean: f64,
    /// Share of runs that came back with nothing at all.
    ///
    /// The number the choice actually turns on. A short sharp run is worth the
    /// same as a long shallow one *on average*, and the average is not what a
    /// player experiences — what they experience is how often it pays nothing,
    /// and the two shapes differ enormously there.
    pub blanks: f64,
    pub bands: [f64; BAND_COUNT],
    /// Best run seen, in total-bet multiples.
    pub best: f64,
    pub runs: u64,
}

pub struct ShapeProfiler {
    session: GameSession,
    shape: usize,
    stats: RoundStats,
    blanks: u64,
    best: f64,
    target: u64,
}

impl ShapeProfiler {
    pub fn new(data: &GameData, shape: usize) -> Self {
        Self {
            // Its own seed per shape, so two shapes are not two views of the
            // same stream of luck — which would make them look more alike than
            // they are, and this measurement exists to show a difference.
            session: GameSession::new(data, PROFILE_SEED ^ 0xF00D ^ (shape as u64 + 1)),
            shape,
            stats: RoundStats::default(),
            blanks: 0,
            best: 0.0,
            target: SHAPE_RUNS,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.stats.rounds >= self.target
    }

    pub fn step(&mut self, data: &GameData) -> Option<ShapeProfile> {
        for _ in 0..SHAPE_RUNS_PER_STEP {
            if self.is_finished() {
                break;
            }
            self.run(data);
        }
        self.is_finished().then(|| self.finish())
    }

    /// One feature, granted directly and played to the end.
    ///
    /// Granted rather than triggered: waiting for scatters would spend a
    /// thousand paid spins per measured run, and what is being compared is the
    /// *feature*, not how often it arrives. Both shapes are handed the same
    /// award for the same reason.
    fn run(&mut self, data: &GameData) {
        self.session.balance = SCRATCH_BANKROLL;
        self.session.celebrations.clear();

        let awarded = data.freespins.award_for(data.freespins.trigger_count());
        let Some(shape) = data.freespins.shapes.get(self.shape) else {
            self.stats.rounds = self.target;
            return;
        };
        let total_bet = self.session.total_bet(data);
        self.session.grant_free_spins(awarded, shape, data);

        let before = self.session.balance;
        while self.session.in_free_spins() {
            if self.session.spin(data).is_err() {
                break;
            }
        }
        self.session.auto_play_bonus(data);
        self.session.auto_play_holdspin(data);

        let won = self.session.balance - before;
        if won <= 0 {
            self.blanks += 1;
        }
        self.stats.record(won, total_bet);
        self.best = self.best.max(won as f64 / total_bet.max(1) as f64);
    }

    fn finish(&self) -> ShapeProfile {
        let runs = self.stats.rounds.max(1);
        ShapeProfile {
            mean: self.stats.mean_return(),
            blanks: self.blanks as f64 / runs as f64,
            bands: self.stats.band_shares(),
            best: self.best,
            runs: self.stats.rounds,
        }
    }
}

#[cfg(test)]
mod shape_tests {
    use super::*;
    use crate::data::MACHINES;

    fn profile(data: &GameData, shape: usize) -> ShapeProfile {
        let mut profiler = ShapeProfiler::new(data, shape);
        loop {
            if let Some(done) = profiler.step(data) {
                return done;
            }
        }
    }

    /// The claim §5.64 makes, checked against play rather than arithmetic.
    ///
    /// The shapes are constructed to be worth the same and a test holds the
    /// construction. This is the other half: actually running two thousand
    /// features each way and seeing the means land together. If they do not,
    /// something about the feature is not linear in the multiplier and the
    /// whole premise is wrong.
    #[test]
    #[ignore = "two thousand features per shape; run with --ignored --release"]
    fn both_shapes_return_the_same_over_two_thousand_runs() {
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            if data.freespins.shapes.len() < 2 {
                continue;
            }
            let measured: Vec<ShapeProfile> = (0..data.freespins.shapes.len())
                .map(|shape| profile(&data, shape))
                .collect();

            let long = measured[0].mean;
            for (index, profile) in measured.iter().enumerate() {
                let drift = (profile.mean - long).abs() / long.max(0.0001);
                println!(
                    "{:>10} {:<6} mean {:7.2}x  blanks {:5.1}%  best {:8.1}x",
                    machine.id,
                    data.freespins.shapes[index].id,
                    profile.mean,
                    profile.blanks * 100.0,
                    profile.best
                );
                assert!(
                    drift < 0.12,
                    "{}: '{}' returns {:.2}x against '{}' at {:.2}x",
                    machine.id,
                    data.freespins.shapes[index].id,
                    profile.mean,
                    data.freespins.shapes[0].id,
                    long
                );
            }
        }
    }
}
