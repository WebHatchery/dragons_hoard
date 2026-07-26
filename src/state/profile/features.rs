//! Measuring the two things a cabinet *sells* rather than the cabinet itself.
//!
//! # Why this is not in `profile`
//!
//! The machine profiler (§5.17) measures a cabinet by playing it. These measure
//! what it offers: a buy tier's distribution (§5.22) and a free-spin shape's
//! (§5.65). Three profilers of the same shape accreted in one file and took it
//! to 832 lines, past the limit — and past it for two iterations, because the
//! gate that should have said so was counting only non-empty lines (§5.69).
//!
//! The seam is what is being measured. A cabinet is measured by paying for
//! spins; a tier by buying it; a shape by being granted a run of it. All three
//! share `RoundStats` and the bands, which stay next door, and share nothing
//! else.

use super::*;

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
