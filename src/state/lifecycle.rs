//! One spin, from commit to settled.
//!
//! The order here is the order it happens in: take the stake and decide the
//! outcome (`begin_spin`/`roll_spin`), advance whatever is revealing it
//! (`update_spin`), then apply it (`settle_landed_spin`/`settle_spin`).
//!
//! Everything in this file obeys §8.2's invariant — the outcome, including a
//! whole cascade chain (§5.15), is fixed at commit and the animation only
//! reveals it. Nothing below `roll_spin` touches the RNG.

use super::holdspin::HoldSpinRound;
use super::seam::SeamRound;
use super::spin::{ReelSpinner, SpinEvent, SpinPhase};
use super::{
    bonus::BonusRound, egg_cells, scatters_per_reel, spin, GameSession, PendingSpin, SpinBlocked,
    SpinHighlights, SpinMode, SpinResolution, AUTO_SPIN_PAUSE, HOLD_SPIN_OPEN_PAUSE,
    SEAM_OPEN_PAUSE,
};
use crate::data::GameData;
use crate::engine;
use crate::engine::evaluate::EvalContext;
use macroquad_toolkit::timing::Timer;

impl GameSession {
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
            &spin::reel_feel(),
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
        // A respin round holds the reels too, but unlike the pick board it is
        // not waiting on the player — it advances itself on a beat.
        if self.holdspin.is_some() {
            if let Some(event) = self.tick_holdspin(data, dt) {
                events.push(event);
            }
            return events;
        }
        // And a seam, which holds them the same way (§5.80).
        if self.seam.is_some() {
            if let Some(event) = self.tick_seam(data, dt) {
                events.push(event);
            }
            return events;
        }

        let mut reels_landed = false;
        let mut payout_done = false;
        let mut auto_ready = false;
        let mut cascaded = false;
        let mut cascade_done = false;

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
            SpinPhase::Cascading(reveal) => {
                if reveal.tick(dt) {
                    cascaded = true;
                }
                cascade_done = reveal.finished();
            }
        }

        if reels_landed {
            // A chain of more than one grid is revealed before anything is
            // settled — the player has to see the collapses that earned the
            // money before the money arrives.
            let chain = self
                .pending
                .as_ref()
                .map_or(1, |pending| pending.result.cascades.len());
            if chain > 1 {
                self.phase = SpinPhase::Cascading(super::spin::cascade_reveal(
                    chain,
                    self.preferences.time_scale(),
                ));
            } else {
                self.settle_landed_spin(data, &mut events);
            }
        }

        if cascaded {
            events.push(SpinEvent::Cascaded);
        }
        if cascade_done {
            self.settle_landed_spin(data, &mut events);
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

    /// Apply the spin the reels have finished revealing, and pick what comes
    /// next. Shared by the plain path and the end of a cascade chain so both
    /// settle through exactly the same code.
    fn settle_landed_spin(&mut self, data: &GameData, events: &mut Vec<SpinEvent>) {
        // A spinning phase always has a pending spin; if it somehow does not,
        // fall back to idle rather than panicking mid-frame.
        let Some(pending) = self.pending.take() else {
            self.phase = SpinPhase::Idle;
            return;
        };

        let resolution = self.settle_spin(data, pending);
        let credits = resolution.total_credits();
        events.push(SpinEvent::Settled(Box::new(resolution)));
        self.phase = if credits > 0 {
            SpinPhase::Payout(super::spin::payout_counter(
                credits,
                self.preferences.time_scale(),
            ))
        } else {
            self.phase_after_spin()
        };
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
    pub(super) fn roll_spin(&mut self, data: &GameData) -> Result<PendingSpin, SpinBlocked> {
        let free_spin = self.free_spins.as_ref().map(|state| state.line_bet);
        let was_free_spin = free_spin.is_some();
        let line_bet = free_spin.unwrap_or_else(|| self.line_bet(data));
        // The ante is a paid-spin thing (§5.75). A free spin costs nothing, so
        // there is nothing to add a quarter to, and its strips are the refined
        // ones rather than the ante ones.
        let ante = !was_free_spin && self.ante(data);
        let total_bet = data.staked(line_bet, ante);

        if was_free_spin {
            if let Some(state) = self.free_spins.as_mut() {
                state.remaining = state.remaining.saturating_sub(1);
            }
            self.stats.free_spins_played += 1;
        } else {
            if self.balance < total_bet {
                return Err(SpinBlocked::InsufficientBalance);
            }
            // Taking a stake is what ends the previous round and begins the
            // next. Everything credited in between — free spins, a bonus
            // board, a respin round — belonged to the round that paid for it.
            if self.open_round.wagered > 0 {
                self.closed_round = Some(std::mem::take(&mut self.open_round));
            }
            self.open_round.wagered = total_bet;
            self.balance -= total_bet;
            self.stats.total_wagered += total_bet;
            // Only paid spins feed the pots — free spins staked nothing.
            self.jackpots.contribute(&data.jackpots, total_bet);
        }

        // The burn deepens *before* the spin it applies to, so the feature's
        // first spin already runs on a refined set (§5.21) — otherwise the
        // escalation would start a beat late and the first free spin would be
        // indistinguishable from a base one.
        let mode = if was_free_spin {
            let depth = data.refine_depth();
            let burned = match self.free_spins.as_mut() {
                Some(state) => {
                    state.burned = (state.burned + 1).min(depth);
                    state.burned
                }
                None => 0,
            };
            // The run's own multiplier, chosen when the feature opened (§5.64).
            let multiplier = self.free_spins.as_ref().map_or(0, |state| state.multiplier);
            SpinMode::FreeSpin { burned, multiplier }
        } else {
            SpinMode::Base { ante }
        };

        // The generator's state *before* the draw — the one number that decides
        // what is about to happen (§5.74). Read here because this is the last
        // instant it exists: `engine::spin` advances the stream, and afterwards
        // there is no way back to it.
        let committed_state = self.rng.state();
        let result = engine::spin(data, &mut self.rng, line_bet, mode);
        self.committed = Some(crate::state::proof::Commitment::record(
            data,
            committed_state,
            line_bet,
            mode,
            &result,
        ));

        Ok(PendingSpin {
            result,
            was_free_spin,
            line_bet,
        })
    }

    /// Apply a decided spin: eggs, hatch, credits, stats, feature awards.
    pub(super) fn settle_spin(&mut self, data: &GameData, pending: PendingSpin) -> SpinResolution {
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

        // A clutch of eggs wakes the dragon (§5.12). Checked against the same
        // egg count that fed the hoard, so one grid can do both — the eggs are
        // banked *and* they open the round.
        if result.outcome.egg_count >= data.holdspin.trigger_eggs {
            let total_bet = data.total_bet(line_bet);
            let seeds = egg_cells(data, &result.grid);
            self.holdspin = Some(HoldSpinRound::new(
                data.config.reel_count * data.config.row_count,
                &seeds,
                total_bet,
                &data.holdspin,
                &mut self.rng,
            ));
            self.holdspin_beat = Timer::new(HOLD_SPIN_OPEN_PAUSE * self.preferences.time_scale());
        }

        // A board that came to rest full of one treasure opens a seam (§5.80).
        //
        // Read off the *resting* grid rather than the landing one, so a
        // cascading cabinet judges the board the player is actually looking at
        // — a seam the chain has already cleared away is not a seam.
        let resting = result.resting_grid();
        if let Some(found) = engine::seam::find(data, resting, &data.seam) {
            // The same context the spin was evaluated under, so a seam opened
            // during free spins pays at the run's multiplier rather than at the
            // base game's. It is the same money, arriving a beat later.
            let ctx = match self.free_spins.as_ref() {
                Some(run) if was_free_spin => {
                    EvalContext::free_spin_at(data, line_bet, run.multiplier(data))
                }
                _ => EvalContext::base(data, line_bet),
            };
            // No rite yet: in the base game the player picks it (§5.81), and
            // nothing here consumes randomness deciding for them.
            self.seam = SeamRound::open(data, resting, found, ctx);
            self.seam_beat = Timer::new(SEAM_OPEN_PAUSE * self.preferences.time_scale());
        }

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
        // The round's return, for the Ledger (§5.18). Progressives are left out
        // for the same reason the machine profile leaves them out (§5.17): one
        // jackpot in a small sample says more about that event than about the
        // cabinet, and the two columns have to be measuring the same thing.
        self.open_round.credits += spin_credits + hatch_credits;
        self.stats.total_spins += 1;
        self.stats.total_won += credited;
        self.stats.biggest_win = self.stats.biggest_win.max(credited);

        let awarded = result.outcome.free_spins_awarded;
        self.open_round.feature |= awarded > 0;
        let (retriggered, finished) = self.apply_free_spin_award(awarded, line_bet, spin_credits);

        self.reel_stops.clone_from(&result.stops);
        // The board comes to rest on the *last* grid of the chain, not the one
        // the reels landed on (§5.15). They are the same thing on a cabinet
        // that does not cascade.
        self.grid = result.resting_grid().clone();
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
            opened_holdspin: self.holdspin.is_some(),
            opened_seam: self.seam.is_some(),
        };
        self.queue_celebrations(data, &highlights);
        self.check_autospin(data, &highlights);

        SpinResolution {
            result,
            was_free_spin,
            spin_credits,
            hatch_credits,
            jackpot,
            wrath_credits: 0,
            seam_credits: 0,
        }
    }
}
