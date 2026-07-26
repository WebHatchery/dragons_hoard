//! Intent dispatcher: the one place where a `UiAction` becomes a state change.

use crate::data::GameData;
use crate::state::autospin::AutospinStop;
use crate::state::featurebuy::{BuyBlocked, BuyResult};
use crate::state::gamble::{GambleBlocked, GambleFlip};
use crate::state::{GameSession, SpinBlocked};
use crate::ui::UiAction;

/// Requests the dispatcher cannot service itself because they touch the save
/// system, which lives with the game orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRequest {
    NewGame,
    Save,
    Load,
    DeleteSave,
}

#[derive(Debug)]
pub enum ActionOutcome {
    /// The intent was valid but changed nothing (bet already at the cap, etc.).
    Ignored,
    /// The stake is committed and the reels are turning. The outcome lands
    /// later, as a [`SpinEvent::Settled`](crate::state::spin::SpinEvent).
    SpinStarted,
    SpinBlocked(SpinBlocked),
    /// A hoard broken, or a stake advanced, to a player who could not spin
    /// (§5.53).
    LifelineTaken(crate::state::ruin::Lifeline),
    BetChanged(i64),
    PaytableToggled,
    SettingsToggled,
    MachinesToggled,
    AchievementsToggled,
    FeatureBuyToggled,
    LedgerToggled,
    LinesToggled,
    /// A free-spin run reshaped, and the spins it now holds (§5.64).
    FreeSpinShapeChosen(u32),
    RulesToggled,
    LimitsToggled,
    HistoryToggled,
    /// A cap or the reality-check interval was stepped (§5.30).
    LimitRequested(crate::state::limits::Cap),
    RealityCheckCycled,
    RealityCheckAcknowledged,
    HintDismissed,
    WaveformsToggled,
    VisionToggled,
    /// A gamble opened, flipped, or was taken (§5.16).
    GambleOffered,
    GambleFlipped(GambleFlip),
    GambleTaken(i64),
    GambleRefused(GambleBlocked),
    /// A feature was bought: what it was, and what it cost (§5.13).
    FeatureBought(BuyResult),
    /// The buy was refused. Carries why, so the message can say so.
    FeatureBuyRefused(BuyBlocked),
    /// A chest was turned over and the round continues.
    BonusPicked,
    /// The last blank was found; the round paid out.
    BonusFinished(i64),
    /// The player picked a cabinet; the orchestrator owns the swap because it
    /// has to rebuild `GameData` and move save slots.
    MachineSelected(usize),
    /// A preference changed and should be written back to disk.
    PreferenceChanged,
    AutospinStarted(u32),
    AutospinStopped(AutospinStop),
    CelebrationDismissed,
    Session(SessionRequest),
}

pub fn apply(
    data: &GameData,
    session: &mut GameSession,
    show_paytable: &mut bool,
    action: UiAction,
) -> ActionOutcome {
    match action {
        UiAction::Spin => match session.begin_spin(data) {
            Ok(()) => ActionOutcome::SpinStarted,
            Err(blocked) => ActionOutcome::SpinBlocked(blocked),
        },
        UiAction::BetUp => {
            let changed = session.adjust_bet(data, 1);
            bet_outcome(data, session, changed)
        }
        UiAction::BetDown => {
            let changed = session.adjust_bet(data, -1);
            bet_outcome(data, session, changed)
        }
        UiAction::MaxBet => {
            let changed = session.set_max_bet(data);
            bet_outcome(data, session, changed)
        }
        UiAction::TogglePaytable => {
            *show_paytable = !*show_paytable;
            ActionOutcome::PaytableToggled
        }
        UiAction::ToggleSettings => ActionOutcome::SettingsToggled,
        UiAction::ToggleMachines => ActionOutcome::MachinesToggled,
        UiAction::ToggleAchievements => ActionOutcome::AchievementsToggled,
        UiAction::ToggleFeatureBuy => ActionOutcome::FeatureBuyToggled,
        UiAction::ToggleLedger => ActionOutcome::LedgerToggled,
        UiAction::ToggleLines => ActionOutcome::LinesToggled,
        UiAction::ChooseFreeSpinShape(index) => match session.choose_free_spin_shape(index, data) {
            Some(spins) => ActionOutcome::FreeSpinShapeChosen(spins),
            // The run has started since the frame that drew the button.
            None => ActionOutcome::Ignored,
        },
        UiAction::ToggleRules => ActionOutcome::RulesToggled,
        UiAction::ToggleLimits => ActionOutcome::LimitsToggled,
        UiAction::ToggleHistory => ActionOutcome::HistoryToggled,
        UiAction::CycleLimit(cap) => ActionOutcome::LimitRequested(cap),
        UiAction::CycleRealityCheck => ActionOutcome::RealityCheckCycled,
        UiAction::AcknowledgeRealityCheck => ActionOutcome::RealityCheckAcknowledged,
        UiAction::DismissHint => ActionOutcome::HintDismissed,
        UiAction::ToggleWaveforms => ActionOutcome::WaveformsToggled,
        UiAction::ToggleVision => ActionOutcome::VisionToggled,
        UiAction::OfferGamble => match session.begin_gamble(data) {
            Ok(_) => ActionOutcome::GambleOffered,
            Err(reason) => ActionOutcome::GambleRefused(reason),
        },
        UiAction::Gamble(scale) => match session.flip_gamble(scale, false, data) {
            Ok(flip) => ActionOutcome::GambleFlipped(flip),
            Err(reason) => ActionOutcome::GambleRefused(reason),
        },
        UiAction::GambleHalf(scale) => match session.flip_gamble(scale, true, data) {
            Ok(flip) => ActionOutcome::GambleFlipped(flip),
            Err(reason) => ActionOutcome::GambleRefused(reason),
        },
        UiAction::TakeGamble => match session.take_gamble() {
            Some(total) => ActionOutcome::GambleTaken(total),
            None => ActionOutcome::GambleRefused(GambleBlocked::NotOffered),
        },
        UiAction::BuyFeature(index) => match session.buy_feature(index, data) {
            Ok(purchase) => ActionOutcome::FeatureBought(purchase),
            Err(reason) => ActionOutcome::FeatureBuyRefused(reason),
        },
        UiAction::PickBonus(index) => match session.pick_bonus(index, data) {
            Some(outcome) => ActionOutcome::BonusFinished(outcome.credits),
            None => ActionOutcome::BonusPicked,
        },
        UiAction::SelectMachine(index) => ActionOutcome::MachineSelected(index),
        UiAction::VolumeUp => {
            session.preferences.adjust_volume(0.1);
            ActionOutcome::PreferenceChanged
        }
        UiAction::VolumeDown => {
            session.preferences.adjust_volume(-0.1);
            ActionOutcome::PreferenceChanged
        }
        UiAction::MusicVolumeUp => {
            session.preferences.adjust_music_volume(0.1);
            ActionOutcome::PreferenceChanged
        }
        UiAction::MusicVolumeDown => {
            session.preferences.adjust_music_volume(-0.1);
            ActionOutcome::PreferenceChanged
        }
        UiAction::CycleTextScale => {
            session.preferences.cycle_text_scale();
            ActionOutcome::PreferenceChanged
        }
        UiAction::CycleSpinSpeed => {
            session.preferences.cycle_spin_speed();
            ActionOutcome::PreferenceChanged
        }
        UiAction::CycleAutospinLength => {
            session.preferences.cycle_autospin(&data.config);
            ActionOutcome::PreferenceChanged
        }
        UiAction::ToggleShake => {
            session.preferences.toggle_shake();
            ActionOutcome::PreferenceChanged
        }
        UiAction::ToggleParticles => {
            session.preferences.toggle_particles();
            ActionOutcome::PreferenceChanged
        }
        UiAction::ToggleAutospin => toggle_autospin(data, session),
        UiAction::DismissCelebration => {
            if session.celebrations.is_active() {
                session.celebrations.skip();
                ActionOutcome::CelebrationDismissed
            } else {
                ActionOutcome::Ignored
            }
        }
        UiAction::TakeLifeline => match session.take_lifeline(data) {
            Some(lifeline) => ActionOutcome::LifelineTaken(lifeline),
            // Not stuck any more — a press from a frame whose offer has since
            // gone must never mint credits (§5.53).
            None => ActionOutcome::Ignored,
        },
        UiAction::NewGame => ActionOutcome::Session(SessionRequest::NewGame),
        UiAction::Save => ActionOutcome::Session(SessionRequest::Save),
        UiAction::Load => ActionOutcome::Session(SessionRequest::Load),
        UiAction::DeleteSave => ActionOutcome::Session(SessionRequest::DeleteSave),
    }
}

/// One button, two jobs: stop a running autospin, or start a fresh one.
fn toggle_autospin(data: &GameData, session: &mut GameSession) -> ActionOutcome {
    if let Some(reason) = session.stop_autospin(AutospinStop::Cancelled) {
        return ActionOutcome::AutospinStopped(reason);
    }

    // Free spins drive themselves; layering an autospin run on top would fight
    // the feature for control of the reels.
    if session.in_free_spins() {
        return ActionOutcome::Ignored;
    }

    // The player's chosen run length wins over the data default.
    let spins = session.preferences.autospin_spins(&data.config);
    if session.start_autospin(spins) {
        ActionOutcome::AutospinStarted(spins)
    } else {
        ActionOutcome::Ignored
    }
}

fn bet_outcome(data: &GameData, session: &GameSession, changed: bool) -> ActionOutcome {
    if changed {
        ActionOutcome::BetChanged(session.line_bet(data))
    } else {
        ActionOutcome::Ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (GameData, GameSession) {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data, 31337);
        (data, session)
    }

    #[test]
    fn spin_intent_commits_the_stake_and_starts_the_reels() {
        let (data, mut session) = setup();
        let mut paytable = false;
        let start = session.balance;
        let total_bet = session.total_bet(&data);

        let outcome = apply(&data, &mut session, &mut paytable, UiAction::Spin);

        assert!(matches!(outcome, ActionOutcome::SpinStarted));
        assert!(session.phase.is_busy());
        // Paid for up front; nothing is credited until the reels land.
        assert_eq!(session.balance, start - total_bet);
        assert_eq!(session.stats.total_spins, 0);
    }

    #[test]
    fn a_second_spin_intent_mid_spin_is_refused() {
        let (data, mut session) = setup();
        let mut paytable = false;

        apply(&data, &mut session, &mut paytable, UiAction::Spin);
        let balance = session.balance;
        let outcome = apply(&data, &mut session, &mut paytable, UiAction::Spin);

        assert!(matches!(
            outcome,
            ActionOutcome::SpinBlocked(SpinBlocked::Busy)
        ));
        // Critically, the refused spin must not take a second stake.
        assert_eq!(session.balance, balance);
    }

    #[test]
    fn spin_intent_reports_an_empty_balance_instead_of_spinning() {
        let (data, mut session) = setup();
        session.balance = 0;
        let mut paytable = false;

        let outcome = apply(&data, &mut session, &mut paytable, UiAction::Spin);

        assert!(matches!(
            outcome,
            ActionOutcome::SpinBlocked(SpinBlocked::InsufficientBalance)
        ));
        assert_eq!(session.stats.total_spins, 0);
    }

    #[test]
    fn bet_intents_walk_the_ladder_and_stop_at_the_ends() {
        let (data, mut session) = setup();
        session.line_bet_index = 0;
        let mut paytable = false;

        assert!(matches!(
            apply(&data, &mut session, &mut paytable, UiAction::BetUp),
            ActionOutcome::BetChanged(_)
        ));
        assert_eq!(session.line_bet_index, 1);

        assert!(matches!(
            apply(&data, &mut session, &mut paytable, UiAction::MaxBet),
            ActionOutcome::BetChanged(_)
        ));
        assert_eq!(session.line_bet_index, data.config.line_bets.len() - 1);

        assert!(matches!(
            apply(&data, &mut session, &mut paytable, UiAction::BetUp),
            ActionOutcome::Ignored
        ));
    }

    #[test]
    fn the_paytable_intent_toggles_both_ways() {
        let (data, mut session) = setup();
        let mut paytable = false;

        apply(&data, &mut session, &mut paytable, UiAction::TogglePaytable);
        assert!(paytable);
        apply(&data, &mut session, &mut paytable, UiAction::TogglePaytable);
        assert!(!paytable);
    }

    #[test]
    fn save_intents_are_handed_back_to_the_orchestrator() {
        let (data, mut session) = setup();
        let mut paytable = false;

        assert!(matches!(
            apply(&data, &mut session, &mut paytable, UiAction::Save),
            ActionOutcome::Session(SessionRequest::Save)
        ));
        assert!(matches!(
            apply(&data, &mut session, &mut paytable, UiAction::DeleteSave),
            ActionOutcome::Session(SessionRequest::DeleteSave)
        ));
    }
}
