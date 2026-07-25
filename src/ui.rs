//! Immediate-mode UI. Pure view layer: it reads state and returns intents.

pub mod achievements;
pub mod bonus;
pub mod celebration;
pub mod featurebuy;
pub mod gamble;
pub mod hint;
pub mod holdspin;
pub mod ledger;
pub mod legibility;
pub mod limits;
pub mod machines;
pub mod nav;
pub mod paytable;
pub mod reality;
pub mod reels;
pub mod rules;
pub mod settings;
pub mod shortcuts;
pub mod symbols;
pub mod vision;
pub mod wager;
pub mod waveform;

use crate::data::GameData;
use crate::state::achievements::AchievementBook;
use crate::state::featurebuy::cheapest as cheapest_feature;
use crate::state::GameSession;
use crate::ui::nav::Nav;
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_badge, draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_ui_text_ex,
    meter, ButtonStyle, ButtonTone, RectExt, SurfaceStyle, TextStyle, VirtualUi,
};

pub const LOGICAL_WIDTH: f32 = 1280.0;
pub const LOGICAL_HEIGHT: f32 = 720.0;

/// Gold-on-stone theme.
pub mod palette {
    use macroquad::prelude::Color;

    pub const BACKGROUND: Color = Color::new(0.045, 0.038, 0.052, 1.0);
    pub const STONE: Color = Color::new(0.098, 0.086, 0.098, 0.97);
    pub const STONE_HEADER: Color = Color::new(0.14, 0.118, 0.125, 1.0);
    pub const GOLD: Color = Color::new(0.90, 0.74, 0.36, 1.0);
    pub const GOLD_BRIGHT: Color = Color::new(1.0, 0.88, 0.52, 1.0);
    pub const GOLD_DIM: Color = Color::new(0.52, 0.41, 0.20, 0.85);
    pub const EMBER: Color = Color::new(0.93, 0.45, 0.18, 1.0);
    /// The other direction. Used only where a figure is up rather than down.
    pub const JADE: Color = Color::new(0.42, 0.82, 0.52, 1.0);
    pub const TEXT_BRIGHT: Color = Color::new(0.96, 0.93, 0.88, 1.0);
    pub const TEXT: Color = Color::new(0.84, 0.80, 0.74, 1.0);
    pub const TEXT_DIM: Color = Color::new(0.62, 0.57, 0.52, 1.0);
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
    /// Open or close the rules panel (§5.29).
    ToggleRules,
    MusicVolumeUp,
    MusicVolumeDown,
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
    CycleSpinSpeed,
    CycleAutospinLength,
    ToggleShake,
    ToggleParticles,
    /// Turn over a chest in the Vault Pick (§5.10).
    PickBonus(usize),
    /// Cut the showing celebration card short.
    DismissCelebration,
    NewGame,
    Save,
    Load,
    DeleteSave,
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
    pub ledger: &'a crate::state::ledger::Ledger,
    pub show_ledger: bool,
    pub show_rules: bool,
    pub limits: &'a crate::state::limits::LimitState,
    pub limit_choices: &'a crate::state::limits::LimitChoices,
    pub show_limits: bool,
    /// A reality check is waiting to be read (§5.30). Holds the game.
    pub reality_check: bool,
    pub show_waveforms: bool,
    /// Live mix, for the waveform inspector (§5.31).
    pub music_levels: [f32; crate::music::Track::ALL.len()],
    pub music_mood: crate::music::Mood,
    pub show_vision: bool,
    /// The hint on offer, if any (§5.28).
    pub hint: Option<&'a crate::state::hints::HintDef>,
    /// Screen-shake displacement, applied to the reels panel only.
    pub shake: Vec2,
    /// Accumulated in-game seconds, used for pulsing highlights. Comes from the
    /// game loop rather than the wall clock so captures stay deterministic.
    pub ui_time: f32,
    pub ui: &'a VirtualUi,
}

pub fn draw_game_ui(ctx: UiContext<'_>, nav: &mut Nav) -> Vec<UiAction> {
    nav.begin();
    let mut actions = Vec::new();
    let mouse = ctx.ui.mouse_position();

    draw_header(&ctx, mouse, &mut actions, nav);
    reels::draw_reels(ctx.data, ctx.session, ctx.shake, ctx.ui_time);
    wager::draw_control_panel(&ctx, mouse, &mut actions, nav);
    draw_footer(&ctx);

    if ctx.show_paytable {
        paytable::draw(&ctx, mouse, &mut actions, nav);
    }
    if ctx.show_rules {
        rules::draw(&ctx, mouse, &mut actions, nav);
    }
    if ctx.show_limits {
        limits::draw(ctx.limits, ctx.limit_choices, mouse, &mut actions, nav);
    }
    if ctx.show_achievements {
        achievements::draw(ctx.achievements, mouse, &mut actions, nav);
    }
    if ctx.show_machines {
        machines::draw(ctx.data, ctx.profiles, mouse, &mut actions, nav);
    }
    if ctx.show_settings {
        settings::draw(
            &ctx.data.config,
            &ctx.session.preferences,
            mouse,
            &mut actions,
            nav,
        );
    }

    // The respin board takes over the reel window while a round is open.
    if let Some(round) = ctx.session.holdspin.as_ref() {
        holdspin::draw(ctx.data, round, ctx.ui_time);
    }

    // The gamble owns the screen while it is up: it is a decision, and the
    // reels behind it are inert until it is made.
    if let Some(round) = ctx.session.gamble.as_ref() {
        gamble::draw(ctx.data, ctx.session, round, mouse, &mut actions, nav);
    }

    if ctx.show_vision {
        vision::draw(ctx.data, mouse, &mut actions, nav);
    }

    if ctx.show_waveforms {
        waveform::draw(ctx.music_levels, ctx.music_mood, mouse, &mut actions, nav);
    }

    if ctx.show_ledger {
        ledger::draw(ctx.data, ctx.ledger, ctx.profiles, mouse, &mut actions, nav);
    }

    if ctx.show_featurebuy {
        featurebuy::draw(
            ctx.data,
            ctx.session,
            ctx.profiles,
            mouse,
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
            mouse,
            ctx.ui_time,
            &mut actions,
            nav,
        );
    }

    // The card sits over everything, including the paytable.
    if let Some(card) = ctx.session.celebrations.active() {
        celebration::draw(card);
        if is_mouse_button_released(MouseButton::Left) {
            actions.push(UiAction::DismissCelebration);
        }
    }

    // Over everything, card included. This one is meant to interrupt (§5.30),
    // and a celebration raised on the spin that tripped it would otherwise sit
    // on top of the thing asking the player to stop and look.
    if ctx.reality_check {
        reality::draw(&ctx.limits.clock, mouse, &mut actions, nav);
    }

    // Under the overlays but over the footer: an offer, not an interruption.
    if let Some(hint) = ctx.hint {
        hint::draw(hint, mouse, &mut actions, nav);
    }

    // Every control has registered by now, so focus can be moved.
    nav.finish();

    actions
}

fn draw_header(ctx: &UiContext<'_>, mouse: Vec2, actions: &mut Vec<UiAction>, nav: &mut Nav) {
    let rect = Rect::new(18.0, 16.0, LOGICAL_WIDTH - 36.0, 64.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(palette::STONE_HEADER)
            .with_border(1.0, palette::GOLD_DIM)
            .with_top_highlight(2.0, palette::GOLD),
    );

    draw_ui_text_ex(
        &ctx.data.config.display_name,
        rect.x + 18.0,
        rect.y + 41.0,
        TextStyle::new(31.0, palette::GOLD_BRIGHT).params(),
    );

    // The header has the only spare width on screen, and these should be
    // reachable from anywhere rather than buried in the wager panel.
    // The button carries the entry price, so the cost of the cheapest feature
    // is visible without opening anything — and it moves with the bet ladder,
    // which is the quickest way to see that the menu is priced per stake.
    let from = cheapest_feature(&ctx.data.featurebuy, ctx.session.total_bet(ctx.data));
    if virtual_button(
        Rect::new(rect.right() - 946.0, rect.y + 18.0, 108.0, 28.0),
        &match from {
            Some(price) => format!("Buy {}", price),
            None => "Buy".to_owned(),
        },
        true,
        ButtonTone::Secondary,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleFeatureBuy);
    }
    if virtual_button(
        Rect::new(rect.right() - 828.0, rect.y + 18.0, 108.0, 28.0),
        "Awards",
        true,
        ButtonTone::Secondary,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleAchievements);
    }
    if virtual_button(
        Rect::new(rect.right() - 710.0, rect.y + 18.0, 108.0, 28.0),
        "Machines",
        true,
        ButtonTone::Secondary,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleMachines);
    }
    if virtual_button(
        Rect::new(rect.right() - 592.0, rect.y + 18.0, 108.0, 28.0),
        "Settings",
        true,
        ButtonTone::Secondary,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleSettings);
    }

    let hoard = &ctx.session.hoard;
    draw_badge(
        Rect::new(rect.right() - 470.0, rect.y + 18.0, 200.0, 28.0),
        &format!(
            "Hoard {}/{}  pot {}",
            hoard.count, ctx.data.config.hoard_capacity, hoard.pot
        ),
        Color::new(0.22, 0.16, 0.10, 1.0),
        palette::TEXT,
    );
    draw_badge(
        Rect::new(rect.right() - 258.0, rect.y + 18.0, 152.0, 28.0),
        &format!("Balance {}", ctx.session.balance),
        Color::new(0.16, 0.20, 0.13, 1.0),
        palette::TEXT_BRIGHT,
    );
    draw_badge(
        Rect::new(rect.right() - 96.0, rect.y + 18.0, 78.0, 28.0),
        &format!("v{}", ctx.data.config.version),
        Color::new(0.18, 0.15, 0.22, 1.0),
        palette::TEXT_DIM,
    );
}

/// Bottom strip: hoard progress and session stats. The far right is left clear
/// for the notification stack, which anchors bottom-right.
fn draw_footer(ctx: &UiContext<'_>) {
    let rect = Rect::new(18.0, 632.0, LOGICAL_WIDTH - 36.0, 70.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(Color::new(0.07, 0.06, 0.07, 0.96)).with_border(1.0, palette::GOLD_DIM),
    );

    let hoard = &ctx.session.hoard;
    meter(
        Rect::new(rect.x + 18.0, rect.y + 14.0, 420.0, 22.0),
        hoard.count as f32,
        ctx.data.config.hoard_capacity as f32,
        palette::EMBER,
        // Machine-agnostic: the Frost cabinet has a hoard too.
        Some(&format!(
            "Hoard {}/{}",
            hoard.count, ctx.data.config.hoard_capacity
        )),
    );
    draw_ui_text_ex(
        &format!(
            "Fill the hoard to hatch a prize worth {}x the banked pot ({}).",
            ctx.data.config.hatch_pot_multiplier, hoard.pot
        ),
        rect.x + 18.0,
        rect.y + 56.0,
        TextStyle::new(15.0, palette::TEXT_DIM).params(),
    );

    let stats = &ctx.session.stats;
    draw_ui_text_ex(
        &format!(
            "Spins {}   Best win {}   Free spins played {}   Hatches {}",
            stats.total_spins, stats.biggest_win, stats.free_spins_played, stats.hatches
        ),
        rect.x + 470.0,
        rect.y + 30.0,
        TextStyle::new(16.0, palette::TEXT).params(),
    );
    // A hint (§5.28) takes this line while it is showing. The two say the same
    // sort of thing and only one of them gets read.
    if ctx.hint.is_none() {
        // Sized to fit rather than set at 15: the line is generated from the
        // shortcut table now (§5.29), so adding a binding lengthens it and a
        // fixed size would quietly clip the last one off the right edge.
        let left = rect.x + 470.0;
        draw_text_block(
            &shortcuts::footer_line(),
            left,
            rect.y + 42.0,
            rect.right() - left - 8.0,
            18.0,
            15.0,
            0.0,
            palette::TEXT_DIM,
        );
    }
}

fn virtual_button(
    rect: Rect,
    text: &str,
    enabled: bool,
    tone: ButtonTone,
    mouse: Vec2,
    nav: &mut Nav,
) -> bool {
    // Registering here means every button in the game answers to the keyboard
    // (§5.27) without a single call site having to think about it.
    let hit = nav.control(rect, enabled, mouse);
    let style = ButtonStyle::from_tone(tone);
    let hovered = enabled && rect.contains_point(mouse);
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

    draw_surface(
        rect,
        &SurfaceStyle::new(fill).with_border(1.0, style.border),
    );
    if hit.focused {
        nav::focus_ring(rect);
    }
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
                palette::TEXT_DIM
            },
        ),
    );

    activated
}
