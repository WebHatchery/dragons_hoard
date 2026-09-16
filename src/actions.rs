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
    MenuToggled,
    /// A screen picked from the menu (§5.72).
    ScreenOpened(crate::game::screens::Screen),
    SessionsToggled,
    /// The ante side bet (§5.75).
    AnteToggled,
    /// The spin verifier (§5.74).
    ProofsToggled,
    /// Re-run every recorded spin.
    ProofsChecked,
    SessionOverDismissed,
    /// A free-spin run reshaped, and the spins it now holds (§5.64).
    FreeSpinShapeChosen(u32),
    /// A rite was taken for an open seam, by name (§5.81).
    RiteChosen(String),
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
        UiAction::TogglePaytable
        | UiAction::ToggleSettings
        | UiAction::ToggleMachines
        | UiAction::ToggleAchievements
        | UiAction::ToggleFeatureBuy
        | UiAction::ToggleLedger
        | UiAction::ToggleLines
        | UiAction::ToggleMenu
        | UiAction::OpenScreen(_)
        | UiAction::ToggleSessions
        | UiAction::ToggleAnte
        | UiAction::ToggleProofs
        | UiAction::CheckProofs
        | UiAction::DismissSessionOver
        | UiAction::ToggleRules
        | UiAction::ToggleLimits
        | UiAction::ToggleHistory
        | UiAction::CycleLimit(_)
        | UiAction::CycleRealityCheck
        | UiAction::AcknowledgeRealityCheck
        | UiAction::DismissHint
        | UiAction::ToggleWaveforms
        | UiAction::ToggleVision
        | UiAction::SelectMachine(_) => apply_navigation(show_paytable, action),
        UiAction::VolumeUp
        | UiAction::VolumeDown
        | UiAction::MusicVolumeUp
        | UiAction::MusicVolumeDown
        | UiAction::CycleTextScale
        | UiAction::CycleSpinSpeed
        | UiAction::CycleAutospinLength
        | UiAction::ToggleShake
        | UiAction::ToggleParticles => apply_preferences(data, session, action),
        UiAction::NewGame | UiAction::Save | UiAction::Load | UiAction::DeleteSave => {
            ActionOutcome::Session(match action {
                UiAction::NewGame => SessionRequest::NewGame,
                UiAction::Save => SessionRequest::Save,
                UiAction::Load => SessionRequest::Load,
                UiAction::DeleteSave => SessionRequest::DeleteSave,
                _ => unreachable!(),
            })
        }
        _ => apply_gameplay(data, session, action),
    }
}

fn apply_navigation(show_paytable: &mut bool, action: UiAction) -> ActionOutcome {
    match action {
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
        UiAction::ToggleMenu => ActionOutcome::MenuToggled,
        UiAction::OpenScreen(screen) => ActionOutcome::ScreenOpened(screen),
        UiAction::ToggleSessions => ActionOutcome::SessionsToggled,
        UiAction::ToggleAnte => ActionOutcome::AnteToggled,
        UiAction::ToggleProofs => ActionOutcome::ProofsToggled,
        UiAction::CheckProofs => ActionOutcome::ProofsChecked,
        UiAction::DismissSessionOver => ActionOutcome::SessionOverDismissed,
        UiAction::ToggleRules => ActionOutcome::RulesToggled,
        UiAction::ToggleLimits => ActionOutcome::LimitsToggled,
        UiAction::ToggleHistory => ActionOutcome::HistoryToggled,
        UiAction::CycleLimit(cap) => ActionOutcome::LimitRequested(cap),
        UiAction::CycleRealityCheck => ActionOutcome::RealityCheckCycled,
        UiAction::AcknowledgeRealityCheck => ActionOutcome::RealityCheckAcknowledged,
        UiAction::DismissHint => ActionOutcome::HintDismissed,
        UiAction::ToggleWaveforms => ActionOutcome::WaveformsToggled,
        UiAction::ToggleVision => ActionOutcome::VisionToggled,
        UiAction::SelectMachine(index) => ActionOutcome::MachineSelected(index),
        _ => unreachable!("navigation helper received gameplay action"),
    }
}

fn apply_preferences(
    data: &GameData,
    session: &mut GameSession,
    action: UiAction,
) -> ActionOutcome {
    match action {
        UiAction::VolumeUp => session.preferences.adjust_volume(0.1),
        UiAction::VolumeDown => session.preferences.adjust_volume(-0.1),
        UiAction::MusicVolumeUp => session.preferences.adjust_music_volume(0.1),
        UiAction::MusicVolumeDown => session.preferences.adjust_music_volume(-0.1),
        UiAction::CycleTextScale => session.preferences.cycle_text_scale(),
        UiAction::CycleSpinSpeed => session.preferences.cycle_spin_speed(),
        UiAction::CycleAutospinLength => session.preferences.cycle_autospin(&data.config),
        UiAction::ToggleShake => session.preferences.toggle_shake(),
        UiAction::ToggleParticles => session.preferences.toggle_particles(),
        _ => unreachable!("preference helper received another action"),
    }
    ActionOutcome::PreferenceChanged
}

fn apply_gameplay(data: &GameData, session: &mut GameSession, action: UiAction) -> ActionOutcome {
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
        UiAction::ChooseFreeSpinShape(index) => match session.choose_free_spin_shape(index, data) {
            Some(spins) => ActionOutcome::FreeSpinShapeChosen(spins),
            None => ActionOutcome::Ignored,
        },
        UiAction::ChooseRite(index) => match session.choose_rite(index) {
            Some(name) => ActionOutcome::RiteChosen(name),
            None => ActionOutcome::Ignored,
        },
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
            None => ActionOutcome::Ignored,
        },
        _ => unreachable!("gameplay helper received another action"),
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

// Tests live in the crate-level integration harness.
