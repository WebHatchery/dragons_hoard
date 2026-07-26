//! Turning a settled [`ActionOutcome`] into everything else the game does about
//! it — sound, cards, particles, panels, persistence.
//!
//! Split out of `game.rs` when it reached the 800-line limit a second time. The
//! seam is the same one `actions.rs` already draws: that module decides *what
//! happened*, working only on the session, and this one decides *what the game
//! does about it*, which is where the sound bank, the notifications and the
//! overlays live. Keeping the two apart is why the rules of the game can be
//! tested without a window.

use super::Game;
use crate::actions::{self, ActionOutcome, SessionRequest};
use crate::audio::Sfx;
use crate::state::autospin::AutospinStop;
use crate::state::featurebuy::BuyBlocked;
use crate::state::gamble::GambleBlocked;
use crate::state::ruin::Lifeline;
use crate::state::{GameSession, SpinBlocked};
use crate::ui::{self, UiAction};
use macroquad_toolkit::rng::random_u64;

impl Game {
    pub(super) fn apply_action(&mut self, action: UiAction) {
        // A bound cap refuses the spin before the session ever sees it, so
        // nothing downstream has to know limits exist (§5.30). Everything else
        // stays available: a player who has stopped playing can still read the
        // ledger, the rules and their own figures.
        if matches!(action, UiAction::Spin | UiAction::ToggleAutospin) {
            if let Some(breach) = self.limits.breach() {
                self.notifications.warning(breach.message());
                return;
            }
        }

        let outcome = actions::apply(
            &self.data,
            &mut self.session,
            &mut self.show_paytable,
            action,
        );

        match outcome {
            ActionOutcome::Ignored => {}
            ActionOutcome::PaytableToggled | ActionOutcome::CelebrationDismissed => {
                self.sound.play(Sfx::Click)
            }
            ActionOutcome::SettingsToggled => {
                self.show_settings = !self.show_settings;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::MachinesToggled => {
                self.show_machines = !self.show_machines;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::BonusPicked => self.sound.play(Sfx::Click),
            ActionOutcome::BonusFinished(credits) => {
                self.sound.play(Sfx::WinBig);
                self.notifications
                    .success(format!("The vault yields {} credits", credits));
                self.autosave();
            }
            ActionOutcome::AchievementsToggled => {
                self.show_achievements = !self.show_achievements;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::GambleOffered => {
                self.note_hint_progress(|counters| counters.gambles += 1);
                self.sound.play(Sfx::Scatter);
                self.show_featurebuy = false;
            }
            ActionOutcome::GambleFlipped(flip) => {
                if flip.won {
                    self.add_trauma(0.35);
                    self.sound.play(Sfx::WinSmall);
                } else {
                    self.sound.play_at(Sfx::ReelStop, 0.5);
                }
            }
            ActionOutcome::GambleTaken(total) => {
                self.sound.play(Sfx::CoinLock);
                self.notifications
                    .success(format!("Gamble taken — {} credits", total));
                self.autosave();
            }
            ActionOutcome::GambleRefused(reason) => {
                self.sound.play_at(Sfx::Click, 0.6);
                self.notifications.warning(gamble_refusal(reason));
            }
            ActionOutcome::VisionToggled => {
                self.show_vision = !self.show_vision;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::WaveformsToggled => {
                self.show_waveforms = !self.show_waveforms;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::HintDismissed => {
                if let Some(id) = self
                    .hints
                    .current(self.achievements.progress(), &self.ledger)
                    .map(|hint| hint.id.clone())
                {
                    self.hints.dismiss(&id);
                    let _ = self.hints.save(&self.data.config);
                }
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::HistoryToggled => {
                self.show_history = !self.show_history;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::LimitsToggled => {
                self.show_limits = !self.show_limits;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::LimitRequested(cap) => {
                self.cycle_limit(cap);
                self.save_limits();
            }
            ActionOutcome::RealityCheckCycled => {
                let choices: Vec<i64> = self
                    .limit_choices
                    .reality_check_minutes
                    .iter()
                    .map(|value| *value as i64)
                    .collect();
                let current = (self.limits.reality_check_minutes != 0)
                    .then_some(self.limits.reality_check_minutes as i64);
                self.limits.reality_check_minutes =
                    ui::limits::next_choice(&choices, current).unwrap_or(0) as u32;
                self.save_limits();
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::RealityCheckAcknowledged => {
                self.limits.clock.acknowledge();
                self.reality_check = false;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::RulesToggled => {
                self.show_rules = !self.show_rules;
                if self.show_rules {
                    self.note_hint_progress(|counters| counters.rules_opened += 1);
                }
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::LedgerToggled => {
                self.show_ledger = !self.show_ledger;
                if self.show_ledger {
                    self.note_hint_progress(|counters| counters.ledger_opened += 1);
                }
                // The panel compares the player against the machine, so the
                // machine has to have been measured. Asking here means opening
                // the ledger starts the profiler if the picker never did.
                if self.show_ledger {
                    self.profiles.request(self.data.machine_id(), &self.data);
                }
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::FeatureBuyToggled => {
                self.show_featurebuy = !self.show_featurebuy;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::FeatureBought(purchase) => {
                self.note_hint_progress(|counters| counters.buys += 1);
                // The menu closes itself: what was bought is about to take over
                // the screen, and leaving the overlay up would hide it.
                self.show_featurebuy = false;
                self.sound.play(Sfx::Scatter);
                self.notifications.success(format!(
                    "{} bought for {} credits",
                    purchase.tier_name, purchase.price
                ));
                self.autosave();
            }
            ActionOutcome::FeatureBuyRefused(reason) => {
                self.sound.play_at(Sfx::Click, 0.6);
                self.notifications.warning(buy_refusal(reason));
            }
            ActionOutcome::MachineSelected(index) => self.switch_machine(index),
            ActionOutcome::PreferenceChanged => {
                // Apply immediately so the change is audible/visible while the
                // panel is still open, then persist it.
                self.sound.set_volume(self.session.preferences.sfx_volume());
                self.music
                    .set_volume(self.session.preferences.music_volume());
                macroquad_toolkit::ui::set_ui_text_scale(self.session.preferences.text_scale());
                self.sound.play(Sfx::Click);
                if !self.session.preferences.particles {
                    self.particles.clear();
                }
                if !self.session.preferences.shared.screen_shake {
                    self.shake.clear();
                }
                let _ = self.session.preferences.save(&self.data.config);
            }
            ActionOutcome::SpinStarted => {
                self.sound.play(Sfx::SpinStart);
                self.floating.clear();
            }
            ActionOutcome::AutospinStarted(spins) => {
                self.notifications
                    .info(format!("Autospin — {} spins", spins));
                self.events.push(UiAction::Spin);
            }
            ActionOutcome::AutospinStopped(reason) => {
                self.notifications.info(reason.message());
            }
            ActionOutcome::SpinBlocked(SpinBlocked::InsufficientBalance) => {
                if let Some(reason) = self.session.stop_autospin(AutospinStop::OutOfCredits) {
                    self.notifications.warning(reason.message());
                } else {
                    // The panel says the rest (§5.53). This used to end
                    // "or start a new game", which was the whole of the game's
                    // answer to the most likely way a session ends.
                    self.notifications
                        .warning("Not enough credits for that bet");
                }
            }
            ActionOutcome::LifelineTaken(lifeline) => {
                self.sound.play(Sfx::WinSmall);
                match lifeline {
                    Lifeline::BreakHoard { credits, eggs, .. } => {
                        self.notifications.info(format!(
                            "Hoard broken for {} — {} lost",
                            crate::ui::naming::credits(credits),
                            crate::ui::naming::eggs(eggs)
                        ));
                    }
                    Lifeline::VaultStake { credits } => {
                        self.notifications.info(format!(
                            "The vault advances {}",
                            crate::ui::naming::credits(credits)
                        ));
                    }
                }
            }
            // Pressing spin again mid-spin is normal input, not an error.
            ActionOutcome::SpinBlocked(SpinBlocked::Busy) => {}
            ActionOutcome::BetChanged(line_bet) => {
                self.sound.play(Sfx::Click);
                self.notifications.info(format!(
                    "Line bet {} — total bet {}",
                    line_bet,
                    self.data.total_bet(line_bet)
                ));
            }
            ActionOutcome::Session(request) => self.apply_session_request(request),
        }
    }

    fn apply_session_request(&mut self, request: SessionRequest) {
        match request {
            SessionRequest::NewGame => {
                // A new game is a new session: the clock restarts, a bound cap
                // is cleared, and any loosening the player filed lands (§5.30).
                self.limits.new_session();
                self.reality_check = false;
                self.history.clear();
                let preferences = self.session.preferences.clone();
                self.session = GameSession::new(&self.data, random_u64());
                self.session.preferences = preferences;
                self.particles.clear();
                self.floating.clear();
                self.session.celebrations.clear();
                self.notifications.info("Fresh stack of credits");
            }
            SessionRequest::Save => self.save_game(),
            SessionRequest::Load => self.load_game(),
            SessionRequest::DeleteSave => self.delete_save(),
        }
    }
}

/// Why a gamble was refused, in the player's terms. `NotOffered` is the one a
/// player will actually hit — pressing G on a losing spin — so it says what is
/// needed rather than what is missing.
fn gamble_refusal(reason: GambleBlocked) -> &'static str {
    match reason {
        GambleBlocked::NotOffered => "Nothing to gamble — win a spin first",
        GambleBlocked::LimitReached => "The gamble ladder is spent",
        GambleBlocked::CannotHalve => "This win cannot be split",
    }
}

/// Why a Feature Buy was refused, in the player's terms rather than the
/// enum's. Every refusal says something — a menu press that produces silence
/// reads as a broken button.
fn buy_refusal(reason: BuyBlocked) -> &'static str {
    match reason {
        BuyBlocked::Busy => "Wait for the reels to settle first",
        BuyBlocked::InsufficientBalance => "Not enough credits for that feature",
        BuyBlocked::FeatureActive => "A feature is already running",
        BuyBlocked::UnknownTier => "That feature is no longer on the menu",
    }
}
