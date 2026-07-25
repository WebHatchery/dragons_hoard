//! Intent dispatcher: the one place where a `UiAction` becomes a state change.

use crate::data::GameData;
use crate::state::autospin::AutospinStop;
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
    BetChanged(i64),
    PaytableToggled,
    SettingsToggled,
    MachinesToggled,
    AchievementsToggled,
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
        UiAction::SelectMachine(index) => ActionOutcome::MachineSelected(index),
        UiAction::VolumeUp => {
            session.preferences.adjust_volume(0.1);
            ActionOutcome::PreferenceChanged
        }
        UiAction::VolumeDown => {
            session.preferences.adjust_volume(-0.1);
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
