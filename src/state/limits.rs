//! Session limits and the reality check (§5.30).
//!
//! # The one thing the game never told you
//!
//! Twenty-four systems have gone into being honest about the maths. The
//! jackpots are solved in closed form (§5.7), the buy tiers are priced at the
//! machine's own return (§5.13), the gamble is proved neutral (§5.16), the
//! ledger sets what the player has seen against what the cabinet does (§5.18),
//! and the rules are generated from the config so they cannot describe a
//! different game (§5.29).
//!
//! All of it describes **the machine**. None of it describes **the session** —
//! how long this has been going on, what has gone in, and what has come back.
//! Those are the numbers a player actually loses track of, and a slot machine is
//! specifically good at making them hard to hold on to: the balance is one
//! number that moves in both directions, and it is the only one on screen.
//!
//! So: a clock the player did not have to start, a plain statement of the three
//! figures at an interval they choose, and limits they can bind themselves to in
//! advance.
//!
//! # Tighten now, loosen later
//!
//! A limit you can lift the moment it binds is a suggestion. A limit you can
//! never lift is a trap, and this is play money.
//!
//! So the change is asymmetric. **Tightening takes effect immediately** —
//! deciding you have had enough should never involve waiting. **Loosening takes
//! effect at the next session**, which is the only rule here that does any real
//! work: it moves the decision to raise a limit out of the moment that made you
//! want to raise it.
//!
//! That asymmetry is the entire system. Everything else is arithmetic.
//!
//! # It is play money, and it says so
//!
//! The reality check states it outright. A game that borrows the shape of a slot
//! machine this closely should be unambiguous about the one way it differs, and
//! burying that in an about box would be the dishonest choice.

use macroquad_toolkit::data_loader::load_embedded_json_labeled;
use serde::{Deserialize, Serialize};

const LIMITS_JSON: &str = include_str!("../../assets/data/limits.json");

/// What the settings panel offers. Data, so the shape of the choices is a
/// design decision rather than a code one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitChoices {
    /// How often the reality check appears, in minutes. `0` means never.
    pub reality_check_minutes: Vec<u32>,
    pub default_reality_check: usize,
    /// Session length caps, in minutes. `0` means no cap.
    pub time_minutes: Vec<u32>,
    /// Net-loss caps, in credits. `0` means no cap.
    pub losses: Vec<i64>,
    /// Spin-count caps. `0` means no cap.
    pub spins: Vec<u32>,
}

impl LimitChoices {
    pub fn load() -> Result<Self, String> {
        let choices: Self = load_embedded_json_labeled("limits", LIMITS_JSON)?;
        choices.validate()?;
        Ok(choices)
    }

    fn validate(&self) -> Result<(), String> {
        for (name, list) in [
            ("reality_check_minutes", &self.reality_check_minutes),
            ("time_minutes", &self.time_minutes),
        ] {
            if list.is_empty() {
                return Err(format!("limits.json: {} offered nothing", name));
            }
            // "Off" has to be reachable, or a limit could never be cleared and
            // the asymmetry below would only ever ratchet one way.
            if !list.contains(&0) {
                return Err(format!("limits.json: {} cannot be turned off", name));
            }
        }
        if !self.losses.contains(&0) || !self.spins.contains(&0) {
            return Err("limits.json: every limit must offer an off switch".to_owned());
        }
        if self.default_reality_check >= self.reality_check_minutes.len() {
            return Err("limits.json: default_reality_check is out of range".to_owned());
        }
        Ok(())
    }
}

/// One player's caps. `None` everywhere is the default: this is opt-in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub time_minutes: Option<u32>,
    pub loss: Option<i64>,
    pub spins: Option<u32>,
}

/// Which cap is being set. Used to keep the tighten/loosen rule in one place
/// rather than repeated three times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    Time,
    Loss,
    Spins,
}

impl Limits {
    fn get(&self, cap: Cap) -> Option<i64> {
        match cap {
            Cap::Time => self.time_minutes.map(i64::from),
            Cap::Loss => self.loss,
            Cap::Spins => self.spins.map(i64::from),
        }
    }

    fn set(&mut self, cap: Cap, value: Option<i64>) {
        match cap {
            Cap::Time => self.time_minutes = value.map(|v| v as u32),
            Cap::Loss => self.loss = value,
            Cap::Spins => self.spins = value.map(|v| v as u32),
        }
    }
}

/// Is `next` a stricter cap than `current`?
///
/// `None` is "no limit", which is the loosest thing there is — so any cap is
/// tighter than none, and none is never tighter than a cap.
fn is_tighter(current: Option<i64>, next: Option<i64>) -> bool {
    match (current, next) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(now), Some(then)) => then < now,
    }
}

/// Why play stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Breach {
    Time(u32),
    Loss(i64),
    Spins(u32),
}

impl Breach {
    /// What the player is told. Says which cap and what to do about it, because
    /// a refusal that only says "no" reads as a bug.
    pub fn message(&self) -> String {
        match self {
            Breach::Time(minutes) => format!(
                "Your {}-minute session limit is up. Start a new game to play on.",
                minutes
            ),
            Breach::Loss(credits) => format!(
                "You have reached your {} credit loss limit. Start a new game to play on.",
                credits
            ),
            Breach::Spins(spins) => format!(
                "You have reached your {}-spin session limit. Start a new game to play on.",
                spins
            ),
        }
    }
}

/// What this session has actually done.
///
/// Separate from the ledger (§5.18), which is a lifetime record per cabinet.
/// This one spans every machine and resets with the session, because "how long
/// have I been at this" is not a question about a cabinet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionClock {
    /// Seconds of play. Fed from the frame loop's own clock rather than the wall
    /// clock, so the capture harness stays reproducible.
    pub elapsed: f32,
    pub spins: u32,
    pub staked: i64,
    pub returned: i64,
    /// Seconds at the last reality check, so the next one is an interval rather
    /// than a total.
    acknowledged_at: f32,
}

impl SessionClock {
    /// Net position. Negative is down, which is where it will usually be.
    pub fn net(&self) -> i64 {
        self.returned - self.staked
    }

    pub fn minutes(&self) -> u32 {
        (self.elapsed / 60.0) as u32
    }

    /// Measured return this session. Nearly meaningless at this sample size, and
    /// shown anyway with that said — same lesson as the ledger.
    pub fn rtp(&self) -> f64 {
        if self.staked == 0 {
            return 0.0;
        }
        self.returned as f64 / self.staked as f64
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed += dt;
    }

    pub fn record(&mut self, staked: i64, returned: i64) {
        if staked <= 0 {
            return;
        }
        self.spins += 1;
        self.staked += staked;
        self.returned += returned;
    }

    /// Has a reality check come due? `interval` of zero means never.
    pub fn check_due(&self, interval_minutes: u32) -> bool {
        if interval_minutes == 0 {
            return false;
        }
        let interval = interval_minutes as f32 * 60.0;
        self.elapsed - self.acknowledged_at >= interval
    }

    /// Note that the player has read it. Records the elapsed time rather than
    /// zeroing anything, so the session totals keep accumulating across checks.
    pub fn acknowledge(&mut self) {
        self.acknowledged_at = self.elapsed;
    }
}

/// The caps in force, the ones waiting for the next session, and the clock.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitState {
    pub active: Limits,
    /// Loosened caps waiting for a new session. Empty of anything the active set
    /// does not already hold looser.
    pub pending: Limits,
    pub reality_check_minutes: u32,
    pub clock: SessionClock,
    /// Set once a cap has bound, so the refusal survives the frame it happened
    /// on and play stays stopped.
    breached: Option<Breach>,
}

impl LimitState {
    pub fn with_defaults(choices: &LimitChoices) -> Self {
        Self {
            reality_check_minutes: choices.reality_check_minutes[choices.default_reality_check],
            ..Self::default()
        }
    }

    /// Set a cap.
    ///
    /// Returns `true` if it took effect now, `false` if it was filed for the
    /// next session. The asymmetry described at the top of this module lives
    /// here and nowhere else.
    pub fn request(&mut self, cap: Cap, value: Option<i64>) -> bool {
        if is_tighter(self.active.get(cap), value) {
            self.active.set(cap, value);
            self.pending.set(cap, value);
            // A cap that just got stricter may already be behind us.
            true
        } else {
            self.pending.set(cap, value);
            false
        }
    }

    /// What the settings panel shows for a cap: what will be in force after the
    /// next new game, which is the value the player just chose.
    pub fn requested(&self, cap: Cap) -> Option<i64> {
        self.pending.get(cap)
    }

    pub fn in_force(&self, cap: Cap) -> Option<i64> {
        self.active.get(cap)
    }

    /// Is this cap waiting on a new session to take effect?
    pub fn deferred(&self, cap: Cap) -> bool {
        self.pending.get(cap) != self.active.get(cap)
    }

    /// Begin a session: the clock restarts and every filed change lands.
    pub fn new_session(&mut self) {
        self.active = self.pending;
        self.clock = SessionClock::default();
        self.breached = None;
    }

    /// Whether play is stopped, and why. Sticky once it happens.
    pub fn breach(&self) -> Option<Breach> {
        self.breached
    }

    /// Re-check the caps against the clock. Call after time passes or a round
    /// settles; cheap, and idempotent once something has bound.
    pub fn evaluate(&mut self) -> Option<Breach> {
        if self.breached.is_some() {
            return self.breached;
        }
        if let Some(minutes) = self.active.time_minutes {
            if self.clock.minutes() >= minutes {
                self.breached = Some(Breach::Time(minutes));
            }
        }
        if let Some(loss) = self.active.loss {
            if -self.clock.net() >= loss {
                self.breached = Some(Breach::Loss(loss));
            }
        }
        if let Some(spins) = self.active.spins {
            if self.clock.spins >= spins {
                self.breached = Some(Breach::Spins(spins));
            }
        }
        self.breached
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices() -> LimitChoices {
        LimitChoices::load().unwrap()
    }

    fn state() -> LimitState {
        LimitState::with_defaults(&choices())
    }

    #[test]
    fn the_shipped_choices_load_and_can_all_be_turned_off() {
        let choices = choices();
        assert!(choices.validate().is_ok());
        assert!(choices.time_minutes.contains(&0));
        assert!(choices.losses.contains(&0));
        assert!(choices.spins.contains(&0));
        assert!(choices.reality_check_minutes.contains(&0));
    }

    #[test]
    fn a_choice_set_with_no_off_switch_is_rejected() {
        let mut choices = choices();
        choices.losses.retain(|value| *value != 0);
        assert!(choices.validate().is_err());
    }

    #[test]
    fn limits_start_off() {
        // Opt-in. A game that capped play by default would be making a decision
        // that is not its to make.
        let state = state();
        assert_eq!(state.in_force(Cap::Time), None);
        assert_eq!(state.in_force(Cap::Loss), None);
        assert_eq!(state.in_force(Cap::Spins), None);
        assert!(state.breach().is_none());
    }

    #[test]
    fn tightening_takes_effect_immediately() {
        // Deciding you have had enough should never involve waiting.
        let mut state = state();
        assert!(state.request(Cap::Loss, Some(5_000)));
        assert_eq!(state.in_force(Cap::Loss), Some(5_000));

        assert!(state.request(Cap::Loss, Some(1_000)));
        assert_eq!(state.in_force(Cap::Loss), Some(1_000));
        assert!(!state.deferred(Cap::Loss));
    }

    #[test]
    fn loosening_waits_for_the_next_session() {
        // The only rule here that does real work: it moves the decision to raise
        // a limit out of the moment that made you want to raise it.
        let mut state = state();
        state.request(Cap::Loss, Some(1_000));

        assert!(!state.request(Cap::Loss, Some(9_000)));
        assert_eq!(state.in_force(Cap::Loss), Some(1_000));
        assert_eq!(state.requested(Cap::Loss), Some(9_000));
        assert!(state.deferred(Cap::Loss));

        state.new_session();
        assert_eq!(state.in_force(Cap::Loss), Some(9_000));
        assert!(!state.deferred(Cap::Loss));
    }

    #[test]
    fn turning_a_limit_off_is_the_loosest_move_there_is() {
        let mut state = state();
        state.request(Cap::Spins, Some(50));
        assert!(!state.request(Cap::Spins, None));
        assert_eq!(state.in_force(Cap::Spins), Some(50));

        state.new_session();
        assert_eq!(state.in_force(Cap::Spins), None);
    }

    #[test]
    fn a_first_cap_is_always_a_tightening() {
        let mut state = state();
        assert!(state.request(Cap::Time, Some(30)));
        assert_eq!(state.in_force(Cap::Time), Some(30));
    }

    #[test]
    fn the_three_caps_are_independent() {
        let mut state = state();
        state.request(Cap::Loss, Some(1_000));
        state.request(Cap::Spins, Some(100));
        assert_eq!(state.in_force(Cap::Loss), Some(1_000));
        assert_eq!(state.in_force(Cap::Spins), Some(100));
        assert_eq!(state.in_force(Cap::Time), None);
    }

    #[test]
    fn a_loss_limit_binds_on_net_position() {
        // Net, not turnover: a player who has staked a fortune and got it back
        // has not lost anything.
        let mut state = state();
        state.request(Cap::Loss, Some(1_000));

        for _ in 0..10 {
            state.clock.record(500, 500);
        }
        assert!(state.evaluate().is_none(), "even money is not a loss");

        state.clock.record(1_000, 0);
        assert_eq!(state.evaluate(), Some(Breach::Loss(1_000)));
    }

    #[test]
    fn winning_back_does_not_lift_a_breach() {
        // Sticky on purpose. "Play until you are even" is the exact thought the
        // limit was set to interrupt.
        let mut state = state();
        state.request(Cap::Loss, Some(1_000));
        state.clock.record(2_000, 0);
        assert!(state.evaluate().is_some());

        state.clock.record(0, 50_000);
        assert_eq!(state.evaluate(), Some(Breach::Loss(1_000)));
    }

    #[test]
    fn a_time_limit_binds_on_the_clock() {
        let mut state = state();
        state.request(Cap::Time, Some(2));
        state.clock.tick(119.0);
        assert!(state.evaluate().is_none());
        state.clock.tick(2.0);
        assert_eq!(state.evaluate(), Some(Breach::Time(2)));
    }

    #[test]
    fn a_spin_limit_binds_on_paid_spins() {
        let mut state = state();
        state.request(Cap::Spins, Some(3));
        for _ in 0..2 {
            state.clock.record(100, 0);
        }
        assert!(state.evaluate().is_none());
        // A free spin costs nothing and is part of the round that bought it, so
        // it is not a spin against the cap (§5.18).
        state.clock.record(0, 5_000);
        assert!(state.evaluate().is_none());

        state.clock.record(100, 0);
        assert_eq!(state.evaluate(), Some(Breach::Spins(3)));
    }

    #[test]
    fn a_new_game_clears_a_breach() {
        let mut state = state();
        state.request(Cap::Spins, Some(1));
        state.clock.record(100, 0);
        assert!(state.evaluate().is_some());

        state.new_session();
        assert!(state.breach().is_none());
        assert_eq!(state.clock.spins, 0);
    }

    #[test]
    fn tightening_below_where_you_already_are_binds_at_once() {
        // Setting a 10-spin cap after 40 spins should stop play, not wait for
        // spin 50.
        let mut state = state();
        for _ in 0..40 {
            state.clock.record(100, 0);
        }
        state.request(Cap::Spins, Some(10));
        assert_eq!(state.evaluate(), Some(Breach::Spins(10)));
    }

    #[test]
    fn the_reality_check_repeats_on_an_interval() {
        let mut state = state();
        state.reality_check_minutes = 5;
        assert!(!state.clock.check_due(5));

        state.clock.tick(5.0 * 60.0);
        assert!(state.clock.check_due(5));

        state.clock.acknowledge();
        assert!(!state.clock.check_due(5));

        // And comes back an interval later rather than at the next total.
        state.clock.tick(4.0 * 60.0);
        assert!(!state.clock.check_due(5));
        state.clock.tick(60.0);
        assert!(state.clock.check_due(5));
    }

    #[test]
    fn a_reality_check_of_zero_never_fires() {
        let mut state = state();
        state.clock.tick(10_000.0);
        assert!(!state.clock.check_due(0));
    }

    #[test]
    fn acknowledging_does_not_reset_the_session_totals() {
        // The check reports the session, not the interval — otherwise every
        // check would say the player had just arrived.
        let mut state = state();
        state.clock.record(1_000, 200);
        state.clock.tick(600.0);
        state.clock.acknowledge();
        assert_eq!(state.clock.staked, 1_000);
        assert_eq!(state.clock.net(), -800);
    }

    #[test]
    fn the_clock_reports_what_it_measured() {
        let mut state = state();
        state.clock.record(1_000, 400);
        state.clock.record(1_000, 1_600);
        assert_eq!(state.clock.spins, 2);
        assert_eq!(state.clock.staked, 2_000);
        assert_eq!(state.clock.returned, 2_000);
        assert_eq!(state.clock.net(), 0);
        assert!((state.clock.rtp() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn an_empty_session_reports_no_return_rather_than_dividing_by_zero() {
        assert_eq!(SessionClock::default().rtp(), 0.0);
        assert_eq!(SessionClock::default().net(), 0);
    }

    #[test]
    fn every_breach_says_which_cap_and_what_to_do() {
        for breach in [Breach::Time(30), Breach::Loss(5_000), Breach::Spins(200)] {
            let message = breach.message();
            assert!(message.contains("new game"), "{}", message);
            assert!(message.len() > 30);
        }
    }

    #[test]
    fn tightness_is_ordered_the_way_a_player_would_expect() {
        assert!(is_tighter(None, Some(10)));
        assert!(is_tighter(Some(10), Some(5)));
        assert!(!is_tighter(Some(5), Some(10)));
        assert!(!is_tighter(Some(5), None));
        assert!(!is_tighter(None, None));
        // Equal is not tighter, so re-picking the current value is a no-op
        // rather than a deferral that would look like a pending change.
        assert!(!is_tighter(Some(5), Some(5)));
    }

    #[test]
    fn re_picking_the_same_value_leaves_nothing_pending() {
        let mut state = state();
        state.request(Cap::Loss, Some(1_000));
        state.request(Cap::Loss, Some(1_000));
        assert!(!state.deferred(Cap::Loss));
    }
}
