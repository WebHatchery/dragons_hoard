//! Runtime session state: the one object the dispatcher mutates.

pub mod achievements;
pub mod autospin;
pub mod bonus;
pub mod celebration;
pub mod featurebuy;
pub mod features;
pub mod gamble;
pub mod hints;
pub mod history;
pub mod hoard;
pub mod holdspin;
pub mod jackpot;
pub mod ledger;
pub mod lifecycle;
pub mod limits;
pub mod preferences;
pub mod profile;
pub mod rules;
pub mod save;
pub mod spin;

use crate::data::GameData;
use crate::engine::{self, Grid, SpinMode, SpinOutcome, SpinResult};
use autospin::{AutospinState, AutospinStop};
use bonus::BonusRound;
use celebration::{CelebrationKind, CelebrationQueue};
use gamble::GambleRound;
use holdspin::HoldSpinRound;
use jackpot::{JackpotState, JackpotWin};
use ledger::OpenRound;
use macroquad_toolkit::rng::SeededRng;
use macroquad_toolkit::timing::Timer;
use preferences::Preferences;
use serde::{Deserialize, Serialize};
use spin::SpinPhase;

pub use hoard::HoardState;
pub use save::{migrate_save_value, SaveData, SessionStats};

/// Beat between automatic spins, free or autospun.
const AUTO_SPIN_PAUSE: f32 = 0.5;
/// Beat between respins in an open Dragon's Wrath round. Slower than a spin's
/// auto-pause on purpose: each respin is its own little reveal, and running them
/// at auto-spin speed would blur the feature into one event.
const HOLD_SPIN_BEAT: f32 = 0.75;
/// A longer pause before the first respin, so the locked eggs are read as coins
/// before anything moves.
const HOLD_SPIN_OPEN_PAUSE: f32 = 1.1;

/// Active free-spin feature. `line_bet` is frozen at the triggering bet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeSpinState {
    pub remaining: u32,
    pub awarded: u32,
    pub line_bet: i64,
    pub total_won: i64,
    /// Symbols burned off the strips so far (§5.21). Rises by one per free spin
    /// and stops at the length of the refine order; a retrigger does **not**
    /// reset it, because taking the reels back to their raw state would make
    /// extra spins a punishment.
    #[serde(default)]
    pub burned: usize,
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
    /// The Dragon's Wrath round this spin opened, once it has been played out
    /// (§5.12). Zero while a round is still on screen, for the same reason
    /// `hatch_credits` is.
    pub wrath_credits: i64,
}

impl SpinResolution {
    pub fn jackpot_credits(&self) -> i64 {
        self.jackpot.as_ref().map_or(0, |win| win.credits)
    }

    pub fn total_credits(&self) -> i64 {
        self.spin_credits + self.hatch_credits + self.jackpot_credits() + self.wrath_credits
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
    /// A clutch of eggs woke the dragon (§5.12).
    opened_holdspin: bool,
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

/// Flat cell indices, row-major over reels, where the hoard symbol landed.
///
/// Row-major by reel matches how the UI walks the grid, so a coin locks in the
/// cell the egg was actually sitting in rather than a transposed one.
pub(super) fn egg_cells(data: &GameData, grid: &Grid) -> Vec<usize> {
    let Some(hoard) = data.symbols.hoard() else {
        return Vec::new();
    };
    (0..grid.reel_count())
        .flat_map(|reel| (0..grid.rows_on(reel)).map(move |row| (reel, row)))
        .filter(|(reel, row)| grid.at(*reel, *row) == hoard)
        .map(|(reel, row)| grid.index(reel, row))
        .collect()
}

/// Scatters landing on each reel of a decided grid.
pub(super) fn scatters_per_reel(data: &GameData, grid: &Grid) -> Vec<usize> {
    let Some(scatter) = data.symbols.scatter() else {
        return vec![0; grid.reel_count()];
    };
    (0..grid.reel_count())
        .map(|reel| {
            (0..grid.rows_on(reel))
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
    holdspin_beat: Timer,
    pub bonus: Option<BonusRound>,
    /// An open Dragon's Wrath round (§5.12). Holds the game like a card does,
    /// but advances on a beat rather than on a pick.
    pub holdspin: Option<HoldSpinRound>,
    /// An open Dragon's Gamble (§5.16). Holds the game while the player decides.
    pub gamble: Option<GambleRound>,
    /// The round being played, for the Ledger (§5.18). One paid spin and
    /// everything it led to, so it stays open across free spins, a bonus
    /// board and a respin round.
    pub open_round: OpenRound,
    /// The round a new stake just ended, waiting to be written to the ledger.
    /// The session cannot write it itself — the ledger spans every cabinet
    /// and belongs to the orchestrator, like the achievements book.
    pub closed_round: Option<OpenRound>,
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
            holdspin_beat: Timer::new(HOLD_SPIN_BEAT),
            bonus: None,
            holdspin: None,
            gamble: None,
            open_round: OpenRound::default(),
            closed_round: None,
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
            holdspin_beat: Timer::new(HOLD_SPIN_BEAT),
            bonus: None,
            holdspin: None,
            gamble: None,
            open_round: OpenRound::default(),
            closed_round: None,
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

    /// The grid to draw right now.
    ///
    /// While a spin is in flight this is the **decided** grid, not the settled
    /// one. `self.grid` is only written when every reel has landed, so a reel
    /// that stops early was drawing the *previous* spin's symbols until the last
    /// one came to rest — and then the whole board snapped. It read as the reels
    /// refusing to lock on their result, and a win hid it only because the
    /// payout count-up holds the board afterwards.
    pub fn display_grid(&self) -> &Grid {
        let Some(pending) = self.pending.as_ref() else {
            return &self.grid;
        };
        // Mid-cascade the board is whichever grid the chain has reached; the
        // landing grid is only the first of them (§5.15).
        match self.phase.cascade() {
            Some(reveal) => pending
                .result
                .cascades
                .get(reveal.step())
                .map_or(&pending.result.grid, |step| &step.grid),
            None => &pending.result.grid,
        }
    }

    /// Length of the pending spin's cascade chain, for tests that need to know
    /// how many grids the reveal owes before it starts.
    pub fn pending_cascade_len(&self) -> usize {
        self.pending
            .as_ref()
            .map_or(0, |pending| pending.result.cascades.len())
    }

    /// The grid the pending chain will come to rest on.
    #[cfg(test)]
    pub fn pending_last_grid(&self) -> Grid {
        self.pending
            .as_ref()
            .map(|pending| pending.result.resting_grid().clone())
            .unwrap_or_else(|| self.grid.clone())
    }

    /// Cascade multiplier currently in force, for the badge over the reels.
    /// `None` when no chain is running or the chain is still at ×1.
    pub fn cascade_multiplier(&self) -> Option<i64> {
        let reveal = self.phase.cascade()?;
        let pending = self.pending.as_ref()?;
        pending
            .result
            .cascades
            .get(reveal.step())
            .map(|step| step.multiplier)
            .filter(|multiplier| *multiplier > 1)
    }

    /// Cells the current cascade step is about to clear, so they can be marked
    /// on the way out.
    pub fn cascade_clearing(&self) -> &[usize] {
        let Some(reveal) = self.phase.cascade() else {
            return &[];
        };
        self.pending
            .as_ref()
            .and_then(|pending| pending.result.cascades.get(reveal.step()))
            .map_or(&[], |step| step.cleared.as_slice())
    }

    pub fn in_free_spins(&self) -> bool {
        self.free_spins.is_some()
    }

    pub fn can_spin(&self, data: &GameData) -> bool {
        self.is_settled() && self.can_afford_spin(data)
    }

    /// Nothing in flight: no reels turning, no payout counting, no card showing,
    /// no bonus board waiting on a pick and no respin round in flight.
    pub fn is_settled(&self) -> bool {
        self.phase.is_idle()
            && !self.celebrations.is_active()
            && self.bonus.is_none()
            && self.holdspin.is_none()
            && self.gamble.is_none()
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
    ///
    /// No longer test-only: the machine profiler (§5.17) runs it at runtime on a
    /// scratch session to measure the cabinet in front of the player.
    pub fn spin(&mut self, data: &GameData) -> Result<SpinResolution, SpinBlocked> {
        let mut resolution = self.spin_leaving_bonus(data)?;
        // A board dealt on this spin is played out immediately, so the headless
        // path stays one call and the sim measures the feature's real EV.
        if let Some(outcome) = self.auto_play_bonus(data) {
            resolution.hatch_credits += outcome.credits;
        }
        // Likewise the respin round — without this the sim would measure a game
        // that triggers the Dragon's Wrath and never pays it (§5.12).
        if let Some(outcome) = self.auto_play_holdspin(data) {
            resolution.wrath_credits += outcome.credits;
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
        } else if highlights.opened_holdspin {
            // The respin round holds the game on its own, but the run has to be
            // torn down too — otherwise it resumes the moment the round ends and
            // the player never gets the board back.
            Some(AutospinStop::WrathWoken)
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
                        burned: 0,
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
