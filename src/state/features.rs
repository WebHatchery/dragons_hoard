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
use super::featurebuy::{self, BuyBlocked, BuyResult};
use super::gamble::{GambleBlocked, GambleFlip, GambleRound, Scale};
use super::holdspin::{self, HoldSpinOutcome, HoldSpinRound};
use super::spin::SpinEvent;
use super::{FreeSpinState, GameSession, HOLD_SPIN_BEAT};
use crate::data::{FeatureAward, GameData};
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

impl GameSession {
    /// Buy a feature outright (§5.13).
    ///
    /// The price is a stake: it leaves the balance, counts toward turnover and
    /// **feeds the progressives**, exactly as a spin's would. What it does not
    /// do is roll for a jackpot — the buy is a purchase, not a spin, and giving
    /// it a jackpot roll would hand a player a second draw per credit that the
    /// bet-fairness argument in §5.6 assumes does not exist.
    ///
    /// Free spins granted this way still cost nothing and still cannot draw a
    /// pot. The buy was the paid event; the spins it bought were not.
    pub fn buy_feature(&mut self, index: usize, data: &GameData) -> Result<BuyResult, BuyBlocked> {
        // A running feature is reported as one, ahead of the general busy
        // check. Both refusals are correct while a feature's card is up, but
        // "the reels are still turning" is the wrong thing to tell someone whose
        // free spins are underway.
        if self.in_free_spins() || self.holdspin.is_some() || self.bonus.is_some() {
            return Err(BuyBlocked::FeatureActive);
        }
        if !self.is_settled() {
            return Err(BuyBlocked::Busy);
        }
        let tier = featurebuy::tier_at(data, index)
            .ok_or(BuyBlocked::UnknownTier)?
            .clone();

        let line_bet = self.line_bet(data);
        let total_bet = data.total_bet(line_bet);
        let price = featurebuy::price(&tier, total_bet);
        if self.balance < price {
            return Err(BuyBlocked::InsufficientBalance);
        }

        self.balance -= price;
        self.stats.total_wagered += price;
        self.stats.features_bought += 1;
        self.jackpots.contribute(&data.jackpots, price);

        match tier.award {
            FeatureAward::FreeSpins { spins } => {
                self.free_spins = Some(FreeSpinState {
                    remaining: spins,
                    awarded: spins,
                    line_bet,
                    total_won: 0,
                });
                self.celebrations
                    .push(CelebrationKind::FreeSpinsEntry { spins, scatters: 0 });
            }
            FeatureAward::Wrath { coins } => {
                let cells = data.config.reel_count * data.config.row_count;
                self.holdspin = Some(HoldSpinRound::new(
                    cells,
                    &featurebuy::opening_cells(coins, cells),
                    total_bet,
                    &data.holdspin,
                    &mut self.rng,
                ));
                self.holdspin_beat =
                    Timer::new(super::HOLD_SPIN_OPEN_PAUSE * self.preferences.time_scale());
            }
        }

        Ok(BuyResult {
            tier_id: tier.id,
            tier_name: tier.name,
            price,
        })
    }

    /// Whether a tier can be bought right now, for greying out the menu.
    pub fn can_buy(&self, index: usize, data: &GameData) -> bool {
        !self.in_free_spins()
            && self.holdspin.is_none()
            && self.bonus.is_none()
            && self.is_settled()
            && featurebuy::tier_at(data, index)
                .is_some_and(|tier| self.balance >= featurebuy::price(tier, self.total_bet(data)))
    }
}

impl GameSession {
    /// Whether a gamble can be offered right now (§5.16).
    ///
    /// Base game only. During free spins the chain spins itself, and a round
    /// that held the reels would either stall the feature or be run straight
    /// over — neither is a decision the player gets to make properly.
    pub fn can_gamble(&self) -> bool {
        self.gamble.is_none()
            && !self.in_free_spins()
            && self.autospin.is_none()
            && self.is_settled()
            && self.last_win > 0
    }

    /// Take the last win back out of the balance and put it at risk.
    pub fn begin_gamble(&mut self, data: &GameData) -> Result<i64, GambleBlocked> {
        if !self.can_gamble() {
            return Err(GambleBlocked::NotOffered);
        }
        let win = self.last_win;
        // The win was credited when the spin settled, so staking it means
        // taking it back out. Anything else would let a player gamble money
        // they had already banked.
        self.balance -= win;
        self.gamble = Some(GambleRound::new(win, self.total_bet(data), &data.gamble));
        Ok(win)
    }

    /// Risk it all, or half of it, on a colour.
    pub fn flip_gamble(
        &mut self,
        picked: Scale,
        half: bool,
        data: &GameData,
    ) -> Result<GambleFlip, GambleBlocked> {
        let round = self.gamble.as_mut().ok_or(GambleBlocked::NotOffered)?;
        let flip = if half {
            round.flip_half(picked, &mut self.rng)
        } else {
            round.flip(picked, &mut self.rng)
        }?;

        // A busted round is closed immediately: there is nothing standing to
        // take, and leaving the panel up would ask the player to press Take on
        // zero.
        if round.standing() == 0 {
            self.gamble = None;
            self.last_win = 0;
            self.stats.gambles_lost += 1;
            self.celebrations.push(CelebrationKind::GambleLost {
                lost: flip.stake.max(0),
                landed: flip.landed.label(),
            });
        }
        let _ = data;
        Ok(flip)
    }

    /// Bank whatever is standing and close the round.
    pub fn take_gamble(&mut self) -> Option<i64> {
        let round = self.gamble.as_mut()?;
        let opening = round.opening();
        let total = round.take();
        self.gamble = None;

        self.balance += total;
        self.last_win = total;
        if total > opening {
            self.stats.gambles_won += 1;
            self.stats.total_won += total - opening;
            self.stats.biggest_win = self.stats.biggest_win.max(total);
        }
        Some(total)
    }
}
