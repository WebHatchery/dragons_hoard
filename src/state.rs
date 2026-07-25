//! Runtime session state: the one object the dispatcher mutates.

pub mod achievements;
pub mod autospin;
pub mod bonus;
pub mod celebration;
pub mod hoard;
pub mod jackpot;
pub mod preferences;
pub mod save;
pub mod spin;

use crate::data::GameData;
use crate::engine::{self, Grid, SpinMode, SpinOutcome, SpinResult};
use autospin::{AutospinState, AutospinStop};
use bonus::{BonusOutcome, BonusRound};
use celebration::{CelebrationKind, CelebrationQueue};
use jackpot::{JackpotState, JackpotWin};
use macroquad_toolkit::rng::SeededRng;
use macroquad_toolkit::timing::Timer;
use preferences::Preferences;
use serde::{Deserialize, Serialize};
use spin::{PayoutCounter, ReelSpinner, SpinEvent, SpinPhase};

pub use hoard::HoardState;
pub use save::{migrate_save_value, SaveData, SessionStats};

/// Beat between automatic spins, free or autospun.
const AUTO_SPIN_PAUSE: f32 = 0.5;

/// Active free-spin feature. `line_bet` is frozen at the triggering bet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeSpinState {
    pub remaining: u32,
    pub awarded: u32,
    pub line_bet: i64,
    pub total_won: i64,
}

/// What a single resolved spin did to the session.
#[derive(Debug, Clone)]
pub struct SpinResolution {
    pub result: SpinResult,
    pub was_free_spin: bool,
    /// Line + scatter credits, already multiplied.
    pub spin_credits: i64,
    /// Hatch prize paid this spin. Zero when the hoard opened a Vault Pick
    /// instead and the player has not finished it yet (§5.10).
    pub hatch_credits: i64,
    /// A progressive that landed on this spin. Independent of the reels.
    pub jackpot: Option<JackpotWin>,
}

impl SpinResolution {
    pub fn jackpot_credits(&self) -> i64 {
        self.jackpot.as_ref().map_or(0, |win| win.credits)
    }

    pub fn total_credits(&self) -> i64 {
        self.spin_credits + self.hatch_credits + self.jackpot_credits()
    }

    pub fn outcome(&self) -> &SpinOutcome {
        &self.result.outcome
    }
}

/// Why a spin could not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinBlocked {
    InsufficientBalance,
    /// The reels are still turning or paying out.
    Busy,
}

/// Everything a settled spin produced that might deserve a card or halt an
/// autospin run. Bundled because passing nine positional arguments around was
/// the kind of signature that goes wrong silently.
#[derive(Debug, Clone)]
struct SpinHighlights {
    awarded: u32,
    retriggered: bool,
    scatters: usize,
    hatch_credits: i64,
    credited: i64,
    finished: Option<FreeSpinState>,
    jackpot: Option<JackpotWin>,
    was_free_spin: bool,
    /// The hoard filled and dealt a board; the run must stop for it.
    opened_bonus: bool,
}

/// A spin that has been paid for and decided but not yet revealed.
///
/// It exists only for the length of the reel animation: the stake is already
/// gone, the outcome is already fixed, and nothing the player does can change
/// either. Settling it applies the winnings.
#[derive(Debug, Clone)]
struct PendingSpin {
    result: SpinResult,
    was_free_spin: bool,
    line_bet: i64,
}

/// Scatters landing on each reel of a decided grid.
fn scatters_per_reel(data: &GameData, grid: &Grid) -> Vec<usize> {
    let Some(scatter) = data.symbols.scatter() else {
        return vec![0; grid.reel_count()];
    };
    (0..grid.reel_count())
        .map(|reel| {
            (0..grid.row_count())
                .filter(|row| grid.at(reel, *row) == scatter)
                .count()
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct GameSession {
    pub balance: i64,
    pub line_bet_index: usize,
    pub grid: Grid,
    pub last_outcome: Option<SpinOutcome>,
    pub last_win: i64,
    pub free_spins: Option<FreeSpinState>,
    pub hoard: HoardState,
    pub jackpots: JackpotState,
    pub stats: SessionStats,
    pub rng: SeededRng,
    /// Presentation state. The Monte-Carlo sim never touches this — it calls
    /// [`GameSession::spin`], which rolls and settles in one go.
    pub phase: SpinPhase,
    /// Full-screen cards waiting to be shown. These hold the game while active.
    pub celebrations: CelebrationQueue,
    /// An open Vault Pick. Holds the game exactly as a celebration card does —
    /// it is waiting on the player.
    pub bonus: Option<BonusRound>,
    pub autospin: Option<AutospinState>,
    /// Player preferences. Deliberately *not* part of `SaveData` — volume and
    /// spin speed belong to the player, not to a save slot, so a New Game or a
    /// deleted save leaves them alone.
    pub preferences: Preferences,
    /// Where each reel is currently resting; the animation starts from here.
    reel_stops: Vec<usize>,
    pending: Option<PendingSpin>,
}

impl GameSession {
    pub fn new(data: &GameData, seed: u64) -> Self {
        Self {
            balance: data.config.starting_balance,
            line_bet_index: data
                .config
                .default_line_bet_index
                .min(data.config.line_bets.len() - 1),
            grid: engine::resting_grid(data),
            last_outcome: None,
            last_win: 0,
            free_spins: None,
            hoard: HoardState::default(),
            jackpots: JackpotState::new(&data.jackpots),
            stats: SessionStats::default(),
            rng: SeededRng::new(seed),
            phase: SpinPhase::Idle,
            celebrations: CelebrationQueue::default(),
            bonus: None,
            autospin: None,
            preferences: Preferences::with_defaults(&data.config),
            reel_stops: vec![0; data.reels.len()],
            pending: None,
        }
    }

    pub fn from_save(data: &GameData, save: SaveData) -> Self {
        Self {
            balance: save.balance,
            line_bet_index: save.line_bet_index.min(data.config.line_bets.len() - 1),
            grid: engine::resting_grid(data),
            last_outcome: None,
            last_win: 0,
            free_spins: None,
            hoard: save.hoard,
            jackpots: {
                // An older save may predate a tier; top it up rather than fail.
                let mut jackpots = save.jackpots;
                jackpots.resize_to(&data.jackpots);
                jackpots
            },
            stats: save.stats,
            rng: save.rng,
            phase: SpinPhase::Idle,
            celebrations: CelebrationQueue::default(),
            bonus: None,
            autospin: None,
            preferences: Preferences::with_defaults(&data.config),
            reel_stops: vec![0; data.reels.len()],
            pending: None,
        }
    }

    pub fn to_save(&self, version: &str) -> SaveData {
        SaveData {
            version: version.to_owned(),
            balance: self.balance,
            line_bet_index: self.line_bet_index,
            hoard: self.hoard.clone(),
            jackpots: self.jackpots.clone(),
            stats: self.stats.clone(),
            rng: self.rng,
        }
    }

    pub fn line_bet(&self, data: &GameData) -> i64 {
        data.line_bet(self.line_bet_index)
    }

    pub fn total_bet(&self, data: &GameData) -> i64 {
        data.total_bet(self.line_bet(data))
    }

    pub fn in_free_spins(&self) -> bool {
        self.free_spins.is_some()
    }

    pub fn can_spin(&self, data: &GameData) -> bool {
        self.is_settled() && self.can_afford_spin(data)
    }

    /// Nothing in flight: no reels turning, no payout counting, no card showing,
    /// no bonus board waiting on a pick.
    pub fn is_settled(&self) -> bool {
        self.phase.is_idle() && !self.celebrations.is_active() && self.bonus.is_none()
    }

    /// Turn a chest over. Credits the balance and raises the Hatch card when
    /// the round ends.
    pub fn pick_bonus(&mut self, index: usize, data: &GameData) -> Option<BonusOutcome> {
        let outcome = self.bonus.as_mut()?.pick(index)?;
        self.finish_bonus(&outcome, data);
        Some(outcome)
    }

    /// Play an open board out without a player — the headless spin path, the
    /// sim and the capture harness.
    pub fn auto_play_bonus(&mut self, data: &GameData) -> Option<BonusOutcome> {
        let round = self.bonus.as_mut()?;
        let outcome = bonus::auto_play(round);
        self.finish_bonus(&outcome, data);
        Some(outcome)
    }

    fn finish_bonus(&mut self, outcome: &BonusOutcome, data: &GameData) {
        self.bonus = None;
        self.balance += outcome.credits;
        self.stats.total_won += outcome.credits;
        self.stats.biggest_win = self.stats.biggest_win.max(outcome.credits);
        self.last_win += outcome.credits;
        self.celebrations.push(CelebrationKind::Hatch {
            credits: outcome.credits,
            eggs: data.config.hoard_capacity,
        });
    }

    fn can_afford_spin(&self, data: &GameData) -> bool {
        self.in_free_spins() || self.balance >= self.total_bet(data)
    }

    /// Bet controls are locked while the free-spin feature is running, while a
    /// spin is mid-flight (the stake is already committed), and during an
    /// autospin run (the run was started at a bet the player chose).
    pub fn bet_locked(&self) -> bool {
        self.in_free_spins() || self.phase.is_busy() || self.autospin.is_some()
    }

    pub fn autospin_remaining(&self) -> u32 {
        self.autospin.as_ref().map_or(0, AutospinState::remaining)
    }

    /// Begin an unattended run. Refused mid-spin so the counter cannot be
    /// started against a stake that is already committed.
    pub fn start_autospin(&mut self, spins: u32) -> bool {
        if spins == 0 || self.autospin.is_some() || !self.is_settled() {
            return false;
        }
        self.autospin = Some(AutospinState::new(spins));
        true
    }

    pub fn stop_autospin(&mut self, reason: AutospinStop) -> Option<AutospinStop> {
        self.autospin.take().map(|_| reason)
    }

    /// The win to show right now: the counting value during the payout, the
    /// settled total otherwise.
    pub fn displayed_win(&self) -> i64 {
        match self.phase.payout() {
            Some(counter) => counter.value(),
            None => self.last_win,
        }
    }

    pub fn adjust_bet(&mut self, data: &GameData, delta: i32) -> bool {
        if self.bet_locked() {
            return false;
        }
        let last = data.config.line_bets.len() - 1;
        let next = (self.line_bet_index as i32 + delta).clamp(0, last as i32) as usize;
        if next == self.line_bet_index {
            return false;
        }
        self.line_bet_index = next;
        true
    }

    pub fn set_max_bet(&mut self, data: &GameData) -> bool {
        if self.bet_locked() {
            return false;
        }
        let last = data.config.line_bets.len() - 1;
        if self.line_bet_index == last {
            return false;
        }
        self.line_bet_index = last;
        true
    }

    /// Run one spin end to end with no animation: debit, roll, evaluate, credit,
    /// resolve features.
    ///
    /// The headless path in one call, for the Monte-Carlo sim and the unit
    /// tests. Play goes through [`begin_spin`](Self::begin_spin) so the reels
    /// turn; the capture harness uses
    /// [`spin_leaving_bonus`](Self::spin_leaving_bonus) directly because it
    /// sometimes wants to stop on an open board. All three share `roll_spin` +
    /// `settle_spin`, so the sim exercises the real rules.
    #[cfg(test)]
    pub fn spin(&mut self, data: &GameData) -> Result<SpinResolution, SpinBlocked> {
        let mut resolution = self.spin_leaving_bonus(data)?;
        // A board dealt on this spin is played out immediately, so the headless
        // path stays one call and the sim measures the feature's real EV.
        if let Some(outcome) = self.auto_play_bonus(data) {
            resolution.hatch_credits += outcome.credits;
        }
        Ok(resolution)
    }

    /// Settle a spin but leave any dealt board open, for callers that want to
    /// present the Vault Pick rather than resolve it — the capture harness, and
    /// tests that need to observe the moment a board appears.
    pub fn spin_leaving_bonus(&mut self, data: &GameData) -> Result<SpinResolution, SpinBlocked> {
        let pending = self.roll_spin(data)?;
        Ok(self.settle_spin(data, pending))
    }

    /// Commit the stake, decide the outcome, and set the reels turning. The
    /// winnings are not applied until every reel has landed.
    pub fn begin_spin(&mut self, data: &GameData) -> Result<(), SpinBlocked> {
        if !self.is_settled() {
            return Err(SpinBlocked::Busy);
        }

        let pending = self.roll_spin(data)?;
        let lengths: Vec<usize> = data.reels.iter().map(Vec::len).collect();
        // The grid is already decided, so anticipation can be worked out before
        // a single reel moves — it only ever fires when the feature really is
        // still live (§5.11).
        let anticipating = spin::anticipating_reels(
            &scatters_per_reel(data, &pending.result.grid),
            data.freespins.trigger_count(),
            data.config.reel_count.saturating_sub(1),
        );
        self.phase = SpinPhase::Spinning(ReelSpinner::new(
            &lengths,
            &self.reel_stops,
            &pending.result.stops,
            self.preferences.time_scale(),
            &anticipating,
        ));
        self.pending = Some(pending);
        Ok(())
    }

    /// Advance the reel animation, the payout count-up, and any showing card.
    ///
    /// A celebration holds everything else: while a card is on screen the reels
    /// do not turn, the payout does not count, and the next automatic spin is
    /// not requested. That is what stops a free-spin trigger being buried under
    /// its own auto-chain.
    pub fn update_spin(&mut self, data: &GameData, dt: f32) -> Vec<SpinEvent> {
        let mut events = Vec::new();

        if let Some(opened) = self.celebrations.update(dt) {
            events.push(SpinEvent::CelebrationOpened(opened));
        }
        if self.celebrations.is_active() {
            return events;
        }
        // An open board holds the reels for the same reason a card does: the
        // game is waiting on the player, and the auto-chain must not run on
        // underneath it.
        if self.bonus.is_some() {
            return events;
        }

        let mut reels_landed = false;
        let mut payout_done = false;
        let mut auto_ready = false;

        match &mut self.phase {
            SpinPhase::Idle => {}
            SpinPhase::Spinning(spinner) => {
                for reel in spinner.tick(dt) {
                    events.push(SpinEvent::ReelStopped(reel));
                }
                reels_landed = spinner.all_settled();
            }
            SpinPhase::Payout(counter) => payout_done = counter.tick(dt),
            SpinPhase::AutoPause(timer) => auto_ready = timer.tick(dt),
        }

        if reels_landed {
            // A spinning phase always has a pending spin; if it somehow does not,
            // fall back to idle rather than panicking mid-frame.
            match self.pending.take() {
                Some(pending) => {
                    let resolution = self.settle_spin(data, pending);
                    let credits = resolution.total_credits();
                    events.push(SpinEvent::Settled(Box::new(resolution)));
                    self.phase = if credits > 0 {
                        SpinPhase::Payout(PayoutCounter::new(
                            credits,
                            self.preferences.time_scale(),
                        ))
                    } else {
                        self.phase_after_spin()
                    };
                }
                None => self.phase = SpinPhase::Idle,
            }
        }

        if payout_done {
            events.push(SpinEvent::PayoutFinished);
            self.phase = self.phase_after_spin();
        }

        if auto_ready {
            self.phase = SpinPhase::Idle;
            events.push(SpinEvent::AutoSpinReady);
        }

        events
    }

    /// After a spin resolves: pause briefly if another spin is owed — by the
    /// feature or by an autospin run — otherwise hand control back.
    fn phase_after_spin(&self) -> SpinPhase {
        if self.in_free_spins() || self.autospin.is_some() {
            SpinPhase::AutoPause(Timer::new(AUTO_SPIN_PAUSE * self.preferences.time_scale()))
        } else {
            SpinPhase::Idle
        }
    }

    /// Take the stake and decide the outcome. Nothing is credited here.
    fn roll_spin(&mut self, data: &GameData) -> Result<PendingSpin, SpinBlocked> {
        let free_spin = self.free_spins.as_ref().map(|state| state.line_bet);
        let was_free_spin = free_spin.is_some();
        let line_bet = free_spin.unwrap_or_else(|| self.line_bet(data));
        let total_bet = data.total_bet(line_bet);

        if was_free_spin {
            if let Some(state) = self.free_spins.as_mut() {
                state.remaining = state.remaining.saturating_sub(1);
            }
            self.stats.free_spins_played += 1;
        } else {
            if self.balance < total_bet {
                return Err(SpinBlocked::InsufficientBalance);
            }
            self.balance -= total_bet;
            self.stats.total_wagered += total_bet;
            // Only paid spins feed the pots — free spins staked nothing.
            self.jackpots.contribute(&data.jackpots, total_bet);
        }

        let mode = if was_free_spin {
            SpinMode::FreeSpin
        } else {
            SpinMode::Base
        };

        Ok(PendingSpin {
            result: engine::spin(data, &mut self.rng, line_bet, mode),
            was_free_spin,
            line_bet,
        })
    }

    /// Apply a decided spin: eggs, hatch, credits, stats, feature awards.
    fn settle_spin(&mut self, data: &GameData, pending: PendingSpin) -> SpinResolution {
        let PendingSpin {
            result,
            was_free_spin,
            line_bet,
        } = pending;

        self.hoard.add_eggs(result.outcome.egg_count, line_bet);
        // A full hoard no longer pays out on the spot: it deals a Vault Pick
        // board for the same expected prize (§5.10). `hatch_credits` therefore
        // stays zero here and is credited when the round ends.
        if let Some(base) = self.hoard.take_hatch(&data.config) {
            self.stats.hatches += 1;
            self.bonus = Some(BonusRound::new(base, &data.bonus, &mut self.rng));
        }
        let hatch_credits = 0;

        // Only a paid spin rolls for a progressive: a free spin staked nothing,
        // so it fed nothing into the pots and cannot draw from them.
        let jackpot = if was_free_spin {
            None
        } else {
            let total_bet = data.total_bet(line_bet);
            self.jackpots.roll(&data.jackpots, &mut self.rng, total_bet)
        };
        if jackpot.is_some() {
            self.stats.jackpots += 1;
        }

        let spin_credits = result.outcome.total_credits;
        let jackpot_credits = jackpot.as_ref().map_or(0, |win| win.credits);
        let credited = spin_credits + hatch_credits + jackpot_credits;
        self.balance += credited;
        self.stats.total_spins += 1;
        self.stats.total_won += credited;
        self.stats.biggest_win = self.stats.biggest_win.max(credited);

        let awarded = result.outcome.free_spins_awarded;
        let (retriggered, finished) = self.apply_free_spin_award(awarded, line_bet, spin_credits);

        self.reel_stops.clone_from(&result.stops);
        self.grid = result.grid.clone();
        self.last_win = credited;
        self.last_outcome = Some(result.outcome.clone());

        let highlights = SpinHighlights {
            awarded,
            retriggered,
            scatters: result.outcome.scatter_count,
            hatch_credits,
            credited,
            finished,
            jackpot: jackpot.clone(),
            was_free_spin,
            opened_bonus: self.bonus.is_some(),
        };
        self.queue_celebrations(data, &highlights);
        self.check_autospin(data, &highlights);

        SpinResolution {
            result,
            was_free_spin,
            spin_credits,
            hatch_credits,
            jackpot,
        }
    }

    /// Raise the cards this spin earned, in the order the player should read
    /// them: the feature ending, then the feature starting, then the money —
    /// with the biggest prize last, as the climax.
    fn queue_celebrations(&mut self, data: &GameData, highlights: &SpinHighlights) {
        if let Some(feature) = highlights.finished.as_ref() {
            self.celebrations.push(CelebrationKind::FreeSpinsSummary {
                spins: feature.awarded,
                won: feature.total_won,
            });
        }

        if highlights.awarded > 0 {
            self.celebrations.push(if highlights.retriggered {
                CelebrationKind::FreeSpinsRetrigger {
                    spins: highlights.awarded,
                }
            } else {
                CelebrationKind::FreeSpinsEntry {
                    spins: highlights.awarded,
                    scatters: highlights.scatters,
                }
            });
        }

        // A jackpot outranks a big-win card — stacking both would announce the
        // same money twice, with the smaller headline second.
        if let Some(win) = highlights.jackpot.as_ref() {
            self.celebrations.push(CelebrationKind::Jackpot {
                name: win.name.clone(),
                credits: win.credits,
            });
        } else if highlights.hatch_credits == 0
            && highlights.credited >= self.big_win_threshold(data)
        {
            self.celebrations.push(CelebrationKind::BigWin {
                credits: highlights.credited,
            });
        }
    }

    /// An unattended run must not skate past the moments worth watching.
    fn check_autospin(&mut self, data: &GameData, highlights: &SpinHighlights) {
        if self.autospin.is_none() {
            return;
        }

        let reason = if highlights.jackpot.is_some() {
            Some(AutospinStop::JackpotWon)
        } else if highlights.awarded > 0 {
            Some(AutospinStop::FeatureTriggered)
        } else if highlights.opened_bonus {
            Some(AutospinStop::Hatched)
        } else if highlights.credited >= self.big_win_threshold(data) {
            Some(AutospinStop::BigWin)
        } else {
            None
        };

        if let Some(reason) = reason {
            self.stop_autospin(reason);
            return;
        }

        // Free spins are not part of the run's budget — they cost nothing, so
        // burning an autospin on one would short-change the player.
        if highlights.was_free_spin {
            return;
        }

        if let Some(run) = self.autospin.as_mut() {
            if !run.take() {
                self.stop_autospin(AutospinStop::Completed);
            }
        }
    }

    pub fn big_win_threshold(&self, data: &GameData) -> i64 {
        self.total_bet(data) * data.config.big_win_multiple.max(1)
    }

    /// Start or extend the feature, then retire it when it runs dry. Returns
    /// whether this was a retrigger, and the finished feature if it just ended.
    fn apply_free_spin_award(
        &mut self,
        awarded: u32,
        line_bet: i64,
        spin_credits: i64,
    ) -> (bool, Option<FreeSpinState>) {
        let mut retriggered = false;

        match self.free_spins.as_mut() {
            Some(state) => {
                state.total_won += spin_credits;
                if awarded > 0 {
                    retriggered = true;
                    state.remaining += awarded;
                    state.awarded += awarded;
                }
            }
            None => {
                if awarded > 0 {
                    self.free_spins = Some(FreeSpinState {
                        remaining: awarded,
                        awarded,
                        line_bet,
                        total_won: 0,
                    });
                }
            }
        }

        let spent = self
            .free_spins
            .as_ref()
            .is_some_and(|state| state.remaining == 0);
        let finished = spent.then(|| self.free_spins.take()).flatten();

        (retriggered, finished)
    }
}

#[cfg(test)]
mod tests;
