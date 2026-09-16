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

pub mod features;
pub use features::{ShapeProfile, TierProfile};
use features::{ShapeProfiler, TierProfiler};

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
pub const PROFILE_SEED: u64 = 0x5CA1_AB1E_D00D;
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

// Tests live in the crate-level integration harness.

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
