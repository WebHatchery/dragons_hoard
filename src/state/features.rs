//! What happens when a second-screen feature resolves.
//!
//! The Vault Pick (§5.10) and the Dragon's Wrath (§5.12) both suspend the base
//! game, run to a conclusion of their own, and then hand back credits and a
//! card. They differ in who drives them — the pick waits on the player, the
//! respin round advances on a beat — but the tail is identical, and keeping the
//! two side by side is what makes that obvious.
//!
//! Both are held by `is_settled()`, so while either is open the reels do not
//! turn, the auto-chain does not run, and Save is disabled.

use super::bonus::{self, BonusOutcome};
use super::celebration::CelebrationKind;
use super::holdspin::{self, HoldSpinOutcome};
use super::spin::SpinEvent;
use super::{GameSession, HOLD_SPIN_BEAT};
use crate::data::GameData;
use macroquad_toolkit::timing::Timer;

impl GameSession {
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

    /// Advance an open respin round on its beat, one respin per tick of the
    /// timer. The round is decided by the RNG as it goes, not up front — there
    /// is nothing for the player to do, so there is nothing to hide from them.
    pub(super) fn tick_holdspin(&mut self, data: &GameData, dt: f32) -> Option<SpinEvent> {
        if !self.holdspin_beat.tick(dt) {
            return None;
        }
        self.holdspin_beat = Timer::new(HOLD_SPIN_BEAT * self.preferences.time_scale());

        let round = self.holdspin.as_mut()?;
        match round.respin(&data.holdspin, &mut self.rng) {
            Some(outcome) => {
                self.finish_holdspin(&outcome);
                Some(SpinEvent::HoldSpinFinished(outcome))
            }
            None => Some(SpinEvent::HoldSpinRespun),
        }
    }

    /// Play an open round out at once — the headless spin path, the sim and the
    /// capture harness.
    pub fn auto_play_holdspin(&mut self, data: &GameData) -> Option<HoldSpinOutcome> {
        let round = self.holdspin.as_mut()?;
        let outcome = holdspin::auto_play(round, &data.holdspin, &mut self.rng);
        self.finish_holdspin(&outcome);
        Some(outcome)
    }

    fn finish_holdspin(&mut self, outcome: &HoldSpinOutcome) {
        self.holdspin = None;
        self.balance += outcome.credits;
        self.stats.total_won += outcome.credits;
        self.stats.biggest_win = self.stats.biggest_win.max(outcome.credits);
        self.last_win += outcome.credits;
        self.stats.wrath_rounds += 1;
        self.celebrations.push(CelebrationKind::Wrath {
            credits: outcome.credits,
            coins: outcome.coins,
            full_board: outcome.full_board,
        });
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
}
