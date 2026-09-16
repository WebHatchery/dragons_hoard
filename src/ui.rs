//! Immediate-mode UI. Pure view layer: it reads state and returns intents.

pub mod achievements;
pub mod bonus;
pub mod celebration;
pub mod chrome;
pub mod featurebuy;
pub mod frame;
pub mod gamble;
pub mod hint;
pub mod history;
pub mod holdspin;
pub mod ledger;
pub mod legibility;
pub mod limits;
pub mod lines;
pub mod machines;
pub mod menu;
pub mod naming;
pub mod nav;
pub mod paylines;
pub mod paytable;
pub mod proof;
pub mod reality;
pub mod reels;
pub mod ruin;
pub mod rules;
pub mod seam;
pub mod sessionover;
pub mod sessions;
pub mod settings;
pub mod shortcuts;
pub mod symbols;
pub mod theme;
pub mod vision;
pub mod wager;
pub mod waveform;

use crate::data::GameData;
use crate::state::achievements::AchievementBook;
use crate::state::featurebuy::cheapest as cheapest_feature;
use crate::state::GameSession;
use crate::ui::nav::Nav;
use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_centered_in_box_ex, ButtonStyle, ButtonTone, Region, SurfaceStyle,
    TextStyle, VirtualUi,
};

/// The width being drawn at. A function since §5.46, because it follows the
/// window's shape rather than being a constant.
pub fn logical_width() -> f32 {
    frame::width()
}
pub const LOGICAL_HEIGHT: f32 = 720.0;

pub mod palette {
    //! The colours, read from whichever theme the cabinet asked for (§5.43).
    //!
    //! Functions rather than constants because the values are now per-cabinet.
    //! Every call site reads the same as it did — `palette::gold()` where it said
    //! `palette::gold()` — which is why nearly three hundred of them could be
    //! converted mechanically and reviewed by the contrast gate instead of by
    //! eye.
    use super::theme;
    use macroquad::prelude::Color;

    pub fn background() -> Color {
        theme::current().background
    }

    pub fn stone() -> Color {
        theme::current().stone
    }

    pub fn stone_header() -> Color {
        theme::current().stone_header
    }

    pub fn gold() -> Color {
        theme::current().gold
    }

    pub fn gold_bright() -> Color {
        theme::current().gold_bright
    }

    pub fn gold_dim() -> Color {
        theme::current().gold_dim
    }

    pub fn ember() -> Color {
        theme::current().ember
    }

    pub fn jade() -> Color {
        theme::current().jade
    }

    pub fn text_bright() -> Color {
        theme::current().text_bright
    }

    pub fn text() -> Color {
        theme::current().text
    }

    pub fn text_dim() -> Color {
        theme::current().text_dim
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    Spin,
    BetUp,
    BetDown,
    MaxBet,
    ToggleAutospin,
    TogglePaytable,
    ToggleSettings,
    ToggleMachines,
    ToggleAchievements,
    /// Open or close the Feature Buy menu (§5.13).
    ToggleFeatureBuy,
    /// Open or close the Ledger panel (§5.18).
    ToggleLedger,
    /// The payline diagrams (§5.60).
    ToggleLines,
    /// The way in to everything else (§5.72).
    ToggleMenu,
    /// The ante side bet (§5.75).
    ToggleAnte,
    /// The spin verifier (§5.74).
    ToggleProofs,
    /// Re-run every recorded spin through the engine.
    CheckProofs,
    /// Open a screen by name, from the menu.
    OpenScreen(crate::game::screens::Screen),
    /// The log of sessions played (§5.70).
    ToggleSessions,
    /// Put the closing summary away (§5.68).
    DismissSessionOver,
    /// Run the free spins the chosen way (§5.64).
    ChooseFreeSpinShape(usize),
    /// Pick which rite an open seam runs (§5.81).
    ChooseRite(usize),
    /// Open or close the rules panel (§5.29).
    ToggleRules,
    MusicVolumeUp,
    MusicVolumeDown,
    /// Open or close the session graph (§5.32).
    ToggleHistory,
    /// Open or close the session limits panel (§5.30).
    ToggleLimits,
    /// Step a cap to the next offering (§5.30).
    CycleLimit(crate::state::limits::Cap),
    CycleRealityCheck,
    /// The player has read the reality check (§5.30).
    AcknowledgeRealityCheck,
    /// Put the current hint away for good (§5.28).
    DismissHint,
    /// Open or close the waveform inspector (§5.19).
    ToggleWaveforms,
    /// Open or close the colour-vision panel (§5.24).
    ToggleVision,
    /// Put the last win at risk (§5.16).
    OfferGamble,
    Gamble(crate::state::gamble::Scale),
    GambleHalf(crate::state::gamble::Scale),
    TakeGamble,
    /// Buy the tier at this index of `featurebuy.json`.
    BuyFeature(usize),
    /// Index into `data::MACHINES`.
    SelectMachine(usize),
    VolumeUp,
    VolumeDown,
    CycleTextScale,
    CycleSpinSpeed,
    CycleAutospinLength,
    ToggleShake,
    ToggleParticles,
    /// Turn over a chest in the Vault Pick (§5.10).
    PickBonus(usize),
    /// Cut the showing celebration card short.
    DismissCelebration,
    NewGame,
    /// Break the hoard, or take the vault's stake (§5.53).
    TakeLifeline,
    Save,
    Load,
    DeleteSave,
}
impl UiAction {
    /// A stable name for this press.
    ///
    /// The match is exhaustive on purpose (§5.76). A new action cannot be added
    /// without the build stopping here, which is the moment to decide whether
    /// the random-play harness should be pressing it — the alternative is a
    /// button nothing has ever tried.
    pub fn name(&self) -> &'static str {
        match self {
            UiAction::Spin => "spin",
            UiAction::BetUp => "bet_up",
            UiAction::BetDown => "bet_down",
            UiAction::MaxBet => "max_bet",
            UiAction::ToggleAutospin => "toggle_autospin",
            UiAction::TogglePaytable => "toggle_paytable",
            UiAction::ToggleSettings => "toggle_settings",
            UiAction::ToggleMachines => "toggle_machines",
            UiAction::ToggleAchievements => "toggle_achievements",
            UiAction::ToggleFeatureBuy => "toggle_feature_buy",
            UiAction::ToggleLedger => "toggle_ledger",
            UiAction::ToggleLines => "toggle_lines",
            UiAction::ToggleMenu => "toggle_menu",
            UiAction::ToggleAnte => "toggle_ante",
            UiAction::ToggleProofs => "toggle_proofs",
            UiAction::CheckProofs => "check_proofs",
            UiAction::OpenScreen(..) => "open_screen",
            UiAction::ToggleSessions => "toggle_sessions",
            UiAction::DismissSessionOver => "dismiss_session_over",
            UiAction::ChooseFreeSpinShape(..) => "choose_free_spin_shape",
            UiAction::ChooseRite(..) => "choose_rite",
            UiAction::ToggleRules => "toggle_rules",
            UiAction::MusicVolumeUp => "music_volume_up",
            UiAction::MusicVolumeDown => "music_volume_down",
            UiAction::ToggleHistory => "toggle_history",
            UiAction::ToggleLimits => "toggle_limits",
            UiAction::CycleLimit(..) => "cycle_limit",
            UiAction::CycleRealityCheck => "cycle_reality_check",
            UiAction::AcknowledgeRealityCheck => "acknowledge_reality_check",
            UiAction::DismissHint => "dismiss_hint",
            UiAction::ToggleWaveforms => "toggle_waveforms",
            UiAction::ToggleVision => "toggle_vision",
            UiAction::OfferGamble => "offer_gamble",
            UiAction::Gamble(..) => "gamble",
            UiAction::GambleHalf(..) => "gamble_half",
            UiAction::TakeGamble => "take_gamble",
            UiAction::BuyFeature(..) => "buy_feature",
            UiAction::SelectMachine(..) => "select_machine",
            UiAction::VolumeUp => "volume_up",
            UiAction::VolumeDown => "volume_down",
            UiAction::CycleTextScale => "cycle_text_scale",
            UiAction::CycleSpinSpeed => "cycle_spin_speed",
            UiAction::CycleAutospinLength => "cycle_autospin_length",
            UiAction::ToggleShake => "toggle_shake",
            UiAction::ToggleParticles => "toggle_particles",
            UiAction::PickBonus(..) => "pick_bonus",
            UiAction::DismissCelebration => "dismiss_celebration",
            UiAction::NewGame => "new_game",
            UiAction::TakeLifeline => "take_lifeline",
            UiAction::Save => "save",
            UiAction::Load => "load",
            UiAction::DeleteSave => "delete_save",
        }
    }
}

pub struct UiContext<'a> {
    pub data: &'a GameData,
    pub session: &'a GameSession,
    pub achievements: &'a AchievementBook,
    pub save_exists: bool,
    pub show_paytable: bool,
    pub show_settings: bool,
    pub show_machines: bool,
    pub show_achievements: bool,
    pub show_featurebuy: bool,
    pub profiles: &'a crate::state::profile::ProfileBook,
    pub sessions: &'a crate::state::sessions::SessionLog,
    /// Spins the game committed to before drawing them (§5.74).
    pub proofs: &'a crate::state::proof::ProofLog,
    /// What the last check found, by spin number.
    pub checked: &'a [(u64, crate::state::proof::Verdict)],
    pub show_proofs: bool,
    pub ledger: &'a crate::state::ledger::Ledger,
    pub show_ledger: bool,
    pub show_lines: bool,
    pub show_menu: bool,
    pub show_sessions: bool,
    pub session_over_dismissed: bool,
    pub show_rules: bool,
    pub limits: &'a crate::state::limits::LimitState,
    pub limit_choices: &'a crate::state::limits::LimitChoices,
    pub show_limits: bool,
    pub history: &'a crate::state::history::History,
    pub show_history: bool,
    /// A reality check is waiting to be read (§5.30). Holds the game.
    pub reality_check: bool,
    pub show_waveforms: bool,
    /// Live mix, for the waveform inspector (§5.31).
    pub music_levels: [f32; crate::music::Track::ALL.len()],
    pub music_mood: crate::music::Mood,
    pub music_arrangement: crate::music::Arrangement,
    pub show_vision: bool,
    /// The hint on offer, if any (§5.28).
    pub hint: Option<&'a crate::state::hints::HintDef>,
    /// Screen-shake displacement, applied to the reels panel only.
    pub shake: Vec2,
    /// Accumulated in-game seconds, used for pulsing highlights. Comes from the
    /// game loop rather than the wall clock so captures stay deterministic.
    pub ui_time: f32,
    pub ui: &'a VirtualUi,
    /// Where the big pieces go at this window's shape (§5.46).
    pub frame: frame::Frame,
    /// Is a panel covering the game (§5.78)? The controls underneath one still
    /// draw, and must not still answer a tap.
    pub overlay_open: bool,
}

pub fn draw_game_ui(ctx: UiContext<'_>, nav: &mut Nav) -> Vec<UiAction> {
    nav.begin();
    let mut actions = Vec::new();
    // One read per frame, mouse or finger (§5.45).
    let pointer = Pointer::read(|at| ctx.ui.screen_to_ui(at));

    // A panel on top takes every control under it out of play (§5.78).
    //
    // They keep drawing, because the game behind a modal is still worth
    // looking at. They stop answering, because a tap landing on both a panel
    // row and the button behind it fired both. Nothing had ever said so: the
    // report that would have said it was suppressed for eight sections
    // (§5.77), and the moment it could speak it named 3,780 square pixels of
    // overlap between the session-limit rows and the rules button underneath.
    nav.set_inert(ctx.overlay_open || ctx.session.celebrations.is_active());
    chrome::draw_header(&ctx, pointer, &mut actions, nav);
    reels::draw_reels(ctx.data, ctx.session, ctx.shake, ctx.ui_time);
    wager::draw_control_panel(&ctx, pointer, &mut actions, nav);
    chrome::draw_footer(&ctx);
    nav.set_inert(ctx.session.celebrations.is_active());

    if ctx.show_paytable {
        paytable::draw(&ctx, pointer, &mut actions, nav);
    }
    if ctx.show_rules {
        rules::draw(&ctx, pointer, &mut actions, nav);
    }
    if ctx.show_limits {
        limits::draw(ctx.limits, ctx.limit_choices, pointer, &mut actions, nav);
    }
    if ctx.show_history {
        history::draw(ctx.history, ctx.ledger, pointer, &mut actions, nav);
    }
    if ctx.show_achievements {
        achievements::draw(ctx.achievements, pointer, &mut actions, nav);
    }
    if ctx.show_machines {
        machines::draw(ctx.data, ctx.profiles, pointer, &mut actions, nav);
    }
    if ctx.show_settings {
        settings::draw(
            &ctx.data.config,
            &ctx.session.preferences,
            pointer,
            &mut actions,
            nav,
        );
    }

    // The respin board takes over the reel window while a round is open.
    if let Some(round) = ctx.session.holdspin.as_ref() {
        holdspin::draw(ctx.data, round, ctx.ui_time);
    }

    // A seam does not take the window over — it marks up the board already
    // drawn there and puts a banner on top of it (§5.80).
    if let Some(round) = ctx.session.seam.as_ref() {
        seam::draw(
            ctx.data,
            round,
            ctx.session.seam_choice(),
            ctx.frame.wager,
            pointer,
            ctx.ui_time,
            &mut actions,
            nav,
        );
    }

    // The gamble owns the screen while it is up: it is a decision, and the
    // reels behind it are inert until it is made.
    if let Some(round) = ctx.session.gamble.as_ref() {
        gamble::draw(ctx.data, ctx.session, round, pointer, &mut actions, nav);
    }

    if ctx.show_vision {
        vision::draw(ctx.data, pointer, &mut actions, nav);
    }

    if ctx.show_waveforms {
        waveform::draw(
            ctx.music_levels,
            ctx.music_mood,
            ctx.music_arrangement,
            pointer,
            &mut actions,
            nav,
        );
    }

    if ctx.show_menu {
        menu::draw(pointer, &mut actions, nav);
    }

    if ctx.show_lines {
        lines::draw(ctx.data, pointer, &mut actions, nav);
    }

    if ctx.show_proofs {
        proof::draw(ctx.proofs, ctx.checked, pointer, &mut actions, nav);
    }
    if ctx.show_sessions {
        sessions::draw(ctx.sessions, pointer, &mut actions, nav);
    }

    if ctx.show_ledger {
        ledger::draw(
            ctx.data,
            ctx.ledger,
            ctx.profiles,
            pointer,
            &mut actions,
            nav,
        );
    }

    if ctx.show_featurebuy {
        featurebuy::draw(
            ctx.data,
            ctx.session,
            ctx.profiles,
            pointer,
            &mut actions,
            nav,
        );
    }

    // The bonus board sits over the game but under a card, so the Hatch card
    // that closes the round still reads as the last word.
    if let Some(round) = ctx.session.bonus.as_ref() {
        bonus::draw(
            round,
            ctx.data
                .symbols
                .iter()
                .find(|(_, def)| def.art == "chest")
                .map(|(_, def)| def),
            pointer,
            ctx.ui_time,
            &mut actions,
            nav,
        );
    }

    // Over everything except the ruin panel: a cap the player set has stopped
    // play and the account of it is the last word (§5.68).
    if let Some(breach) = ctx.limits.breach() {
        if !ctx.session_over_dismissed {
            sessionover::draw(
                breach,
                &ctx.limits.clock,
                ctx.session,
                pointer,
                &mut actions,
                nav,
            );
        }
    }

    // Last of the overlays and over all of them: a player who cannot spin needs
    // this more than they need whatever they had open (§5.53). It is dealt
    // rather than opened, so there is no flag and no close button — it is up
    // exactly while the reels cannot turn.
    if let Some(lifeline) = ctx.session.lifeline(ctx.data) {
        ruin::draw(
            lifeline,
            ctx.session.balance,
            ctx.session.cheapest_spin(ctx.data),
            pointer,
            &mut actions,
            nav,
        );
    }

    // The card sits over everything, including the paytable.
    if let Some(card) = ctx.session.celebrations.active() {
        nav.set_inert(false);
        if celebration::draw(card, &ctx.data.presentation, pointer, nav) {
            actions.push(UiAction::DismissCelebration);
        }
        nav.set_inert(true);
    }

    // Over everything, card included. This one is meant to interrupt (§5.30),
    // and a celebration raised on the spin that tripped it would otherwise sit
    // on top of the thing asking the player to stop and look.
    if ctx.reality_check {
        reality::draw(&ctx.limits.clock, pointer, &mut actions, nav);
    }

    // Under the overlays but over the footer: an offer, not an interruption.
    if let Some(hint) = ctx.hint {
        hint::draw(hint, pointer, &mut actions, nav);
    }

    // Every control has registered by now, so focus can be moved.
    nav.finish();

    actions
}

/// Where a panel's Close button goes.
///
/// One shape, in one place. The same rectangle was written out longhand in ten
/// separate files, which is why it was thirty pixels tall in all ten — a
/// repeated literal is a decision nobody ever revisits. At 44 it meets the
/// touch standard (§5.78), and a panel header is 48 tall, so it still sits
/// inside one.
pub fn close_button(panel: Rect) -> Rect {
    Rect::new(panel.right() - 128.0, panel.y + 2.0, 108.0, 44.0)
}

pub(crate) fn virtual_button(
    rect: Rect,
    text: &str,
    enabled: bool,
    tone: ButtonTone,
    pointer: Pointer,
    nav: &mut Nav,
) -> bool {
    // Registering here means every button in the game answers to the keyboard
    // (§5.27) without a single call site having to think about it.
    let hit = nav.control(rect, enabled, pointer);
    let style = ButtonStyle::from_tone(tone);
    let hovered = enabled && pointer.hovering_over(rect) || pointer.pressing(rect);
    let pressed = hovered && is_mouse_button_down(MouseButton::Left);
    let activated = hit.activated;
    let fill = if !enabled {
        style.disabled
    } else if pressed {
        style.pressed
    } else if hovered {
        style.hovered
    } else {
        style.normal
    };

    // Darkened only as far as its label needs (§5.40). The audit found white on
    // the bright tones at 2.1:1 — every button in the game was below the
    // standard, and the fills had been chosen by eye against a dark panel rather
    // than against the text on top of them.
    let fill = macroquad_toolkit::ui::darken_until(
        fill,
        if enabled {
            style.text_color
        } else {
            palette::text_dim()
        },
        17.0,
    );
    draw_surface(
        rect,
        &SurfaceStyle::new(fill).with_border(1.0, style.border),
    );
    if hit.focused {
        nav::focus_ring(rect);
    }
    // A button's label sits on the button, not on the panel behind it — and the
    // fill changes with hover and disabled state, which is exactly where a
    // label goes quietly unreadable (§5.40).
    let _label_region = Region::on(rect, fill);
    draw_text_centered_in_box_ex(
        text,
        rect.x + 8.0,
        rect.y + if pressed { 2.0 } else { 0.0 },
        rect.w - 16.0,
        rect.h,
        TextStyle::new(
            17.0,
            if enabled {
                style.text_color
            } else {
                palette::text_dim()
            },
        ),
    );

    activated
}
