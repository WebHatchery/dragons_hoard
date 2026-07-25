//! Immediate-mode UI. Pure view layer: it reads state and returns intents.

pub mod achievements;
pub mod bonus;
pub mod celebration;
pub mod featurebuy;
pub mod gamble;
pub mod holdspin;
pub mod ledger;
pub mod machines;
pub mod paytable;
pub mod reels;
pub mod settings;
pub mod symbols;
pub mod waveform;

use crate::data::GameData;
use crate::state::achievements::AchievementBook;
use crate::state::featurebuy::cheapest as cheapest_feature;
use crate::state::GameSession;
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_badge, draw_surface, draw_text_block, draw_text_centered_in_box_ex, draw_text_right,
    draw_ui_text_ex, meter, ButtonStyle, ButtonTone, RectExt, SurfaceStyle, TextStyle, VirtualUi,
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
    /// Open or close the waveform inspector (§5.19).
    ToggleWaveforms,
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
    pub show_waveforms: bool,
    /// Screen-shake displacement, applied to the reels panel only.
    pub shake: Vec2,
    /// Accumulated in-game seconds, used for pulsing highlights. Comes from the
    /// game loop rather than the wall clock so captures stay deterministic.
    pub ui_time: f32,
    pub ui: &'a VirtualUi,
}

pub fn draw_game_ui(ctx: UiContext<'_>) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ctx.ui.mouse_position();

    draw_header(&ctx, mouse, &mut actions);
    reels::draw_reels(ctx.data, ctx.session, ctx.shake, ctx.ui_time);
    draw_control_panel(&ctx, mouse, &mut actions);
    draw_footer(&ctx);

    if ctx.show_paytable {
        paytable::draw(&ctx, mouse, &mut actions);
    }
    if ctx.show_achievements {
        achievements::draw(ctx.achievements, mouse, &mut actions);
    }
    if ctx.show_machines {
        machines::draw(ctx.data, ctx.profiles, mouse, &mut actions);
    }
    if ctx.show_settings {
        settings::draw(
            &ctx.data.config,
            &ctx.session.preferences,
            mouse,
            &mut actions,
        );
    }

    // The respin board takes over the reel window while a round is open.
    if let Some(round) = ctx.session.holdspin.as_ref() {
        holdspin::draw(ctx.data, round, ctx.ui_time);
    }

    // The gamble owns the screen while it is up: it is a decision, and the
    // reels behind it are inert until it is made.
    if let Some(round) = ctx.session.gamble.as_ref() {
        gamble::draw(ctx.data, ctx.session, round, mouse, &mut actions);
    }

    if ctx.show_waveforms {
        waveform::draw(mouse, &mut actions);
    }

    if ctx.show_ledger {
        ledger::draw(ctx.data, ctx.ledger, ctx.profiles, mouse, &mut actions);
    }

    if ctx.show_featurebuy {
        featurebuy::draw(ctx.data, ctx.session, ctx.profiles, mouse, &mut actions);
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
        );
    }

    // The card sits over everything, including the paytable.
    if let Some(card) = ctx.session.celebrations.active() {
        celebration::draw(card);
        if is_mouse_button_released(MouseButton::Left) {
            actions.push(UiAction::DismissCelebration);
        }
    }

    actions
}

fn draw_header(ctx: &UiContext<'_>, mouse: Vec2, actions: &mut Vec<UiAction>) {
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
    ) {
        actions.push(UiAction::ToggleFeatureBuy);
    }
    if virtual_button(
        Rect::new(rect.right() - 828.0, rect.y + 18.0, 108.0, 28.0),
        "Awards",
        true,
        ButtonTone::Secondary,
        mouse,
    ) {
        actions.push(UiAction::ToggleAchievements);
    }
    if virtual_button(
        Rect::new(rect.right() - 710.0, rect.y + 18.0, 108.0, 28.0),
        "Machines",
        true,
        ButtonTone::Secondary,
        mouse,
    ) {
        actions.push(UiAction::ToggleMachines);
    }
    if virtual_button(
        Rect::new(rect.right() - 592.0, rect.y + 18.0, 108.0, 28.0),
        "Settings",
        true,
        ButtonTone::Secondary,
        mouse,
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

fn draw_control_panel(ctx: &UiContext<'_>, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let rect = Rect::new(852.0, 96.0, 410.0, 520.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(palette::STONE)
            .with_border(1.0, palette::GOLD_DIM)
            .with_header(44.0, palette::STONE_HEADER)
            .with_header_divider(1.0, palette::GOLD_DIM),
    );
    draw_ui_text_ex(
        if ctx.session.in_free_spins() {
            "Free Spins"
        } else {
            "Wager"
        },
        rect.x + 18.0,
        rect.y + 30.0,
        TextStyle::new(19.0, palette::GOLD).params(),
    );

    // The wager readout flows from the top and the buttons are anchored to the
    // bottom, so the free-spin banner can claim the space between them without
    // pushing anything off the panel.
    let content = rect.inset(18.0);
    let mut y = content.y + 44.0;
    y = draw_win_readout(ctx, content, y);
    y = draw_bet_controls(ctx, content, y, mouse, actions);
    draw_feature_banner(ctx, content, y);

    let buttons_top = draw_session_buttons(ctx, content, mouse, actions);
    draw_spin_button(ctx, content, buttons_top, mouse, actions);
}

fn draw_win_readout(ctx: &UiContext<'_>, content: Rect, y: f32) -> f32 {
    let rect = Rect::new(content.x, y, content.w, 62.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(Color::new(0.07, 0.06, 0.05, 1.0)).with_border(1.0, palette::GOLD_DIM),
    );
    draw_ui_text_ex(
        "WIN",
        rect.x + 14.0,
        rect.y + 38.0,
        TextStyle::new(18.0, palette::TEXT_DIM).params(),
    );
    let win = ctx.session.displayed_win();
    draw_text_right(
        &win.to_string(),
        rect.right() - 14.0,
        rect.y + 42.0,
        TextStyle::new(
            30.0,
            if win > 0 {
                palette::GOLD_BRIGHT
            } else {
                palette::TEXT_DIM
            },
        ),
    );
    y + 76.0
}

fn draw_bet_controls(
    ctx: &UiContext<'_>,
    content: Rect,
    y: f32,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) -> f32 {
    let line_bet = ctx.session.line_bet(ctx.data);
    let enabled = !ctx.session.bet_locked();

    draw_ui_text_ex(
        "Line Bet",
        content.x,
        y + 24.0,
        TextStyle::new(18.0, palette::TEXT).params(),
    );

    let button = 38.0;
    if virtual_button(
        Rect::new(content.right() - button * 2.0 - 92.0, y, button, button),
        "-",
        enabled,
        ButtonTone::Secondary,
        mouse,
    ) {
        actions.push(UiAction::BetDown);
    }
    draw_text_centered_in_box_ex(
        &line_bet.to_string(),
        content.right() - button - 92.0,
        y,
        92.0,
        button,
        TextStyle::new(22.0, palette::GOLD_BRIGHT),
    );
    if virtual_button(
        Rect::new(content.right() - button, y, button, button),
        "+",
        enabled,
        ButtonTone::Secondary,
        mouse,
    ) {
        actions.push(UiAction::BetUp);
    }

    let y = y + 48.0;
    draw_text_block(
        &format!(
            "{}\nTotal Bet: {}",
            // A ways cabinet has no lines to count, and "Lines: 0" would read as
            // a fault rather than as a different machine.
            match (ctx.data.ways_count(), ctx.data.max_ways()) {
                (Some(ways), _) => format!("{} ways   (all active)", ways),
                // A shifting cabinet (§5.20) has a different number of ways
                // every spin, so it reads the board rather than the config.
                // Quoting the ceiling alone would be advertising a grid the
                // player is almost never looking at.
                (None, Some(ceiling)) => format!(
                    "{} ways this spin   (up to {})",
                    ctx.session.display_grid().ways(),
                    ceiling
                ),
                _ => format!("Lines: {}   (all active)", ctx.data.paylines.len()),
            },
            ctx.data.total_bet(line_bet)
        ),
        content.x,
        y,
        content.w,
        46.0,
        17.0,
        4.0,
        palette::TEXT_DIM,
    );

    y + 54.0
}

/// Free spins take the banner slot; an autospin run gets it when they are not
/// running, so the panel always says what is driving the reels.
fn draw_feature_banner(ctx: &UiContext<'_>, content: Rect, y: f32) {
    let banner = match ctx.session.free_spins.as_ref() {
        Some(free_spins) => Some((
            Color::new(0.24, 0.11, 0.04, 1.0),
            palette::EMBER,
            format!("{} free spins left", free_spins.remaining),
            format!(
                "x{} wilds expand  |  won {}",
                ctx.data.freespins.multiplier, free_spins.total_won
            ),
        )),
        None if ctx.session.autospin_remaining() > 0 => Some((
            Color::new(0.09, 0.14, 0.19, 1.0),
            palette::GOLD,
            format!("Autospin — {} left", ctx.session.autospin_remaining()),
            "Stops on a feature, a hatch or a big win".to_owned(),
        )),
        None => None,
    };

    let Some((fill, accent, title, subtitle)) = banner else {
        return;
    };

    let rect = Rect::new(content.x, y, content.w, 58.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(fill)
            .with_border(2.0, accent)
            .with_left_accent(4.0, palette::GOLD_BRIGHT),
    );
    draw_ui_text_ex(
        &title,
        rect.x + 14.0,
        rect.y + 26.0,
        TextStyle::new(19.0, palette::GOLD_BRIGHT).params(),
    );
    draw_ui_text_ex(
        &subtitle,
        rect.x + 14.0,
        rect.y + 46.0,
        TextStyle::new(15.0, palette::TEXT).params(),
    );
}

/// Spin block, sitting immediately above the bottom-anchored session buttons.
fn draw_spin_button(
    ctx: &UiContext<'_>,
    content: Rect,
    below: f32,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
    let secondary_y = below - 12.0 - 38.0;
    let spin_y = secondary_y - 10.0 - 70.0;

    let label = if ctx.session.phase.is_busy() {
        "SPINNING"
    } else if ctx.session.in_free_spins() {
        "FREE SPIN"
    } else {
        "S P I N"
    };
    if virtual_button(
        Rect::new(content.x, spin_y, content.w, 70.0),
        label,
        ctx.session.can_spin(ctx.data),
        ButtonTone::Positive,
        mouse,
    ) {
        actions.push(UiAction::Spin);
    }

    let third = (content.w - 16.0) / 3.0;
    if virtual_button(
        Rect::new(content.x, secondary_y, third, 38.0),
        "Max Bet",
        !ctx.session.bet_locked(),
        ButtonTone::Primary,
        mouse,
    ) {
        actions.push(UiAction::MaxBet);
    }

    let running = ctx.session.autospin_remaining();
    let (auto_label, auto_tone) = if running > 0 {
        (format!("Stop {}", running), ButtonTone::Danger)
    } else {
        (
            format!(
                "Auto {}",
                ctx.session.preferences.autospin_spins(&ctx.data.config)
            ),
            ButtonTone::Secondary,
        )
    };
    if virtual_button(
        Rect::new(content.x + third + 8.0, secondary_y, third, 38.0),
        &auto_label,
        // Stopping is always allowed; starting needs a settled, affordable game.
        running > 0 || (ctx.session.can_spin(ctx.data) && !ctx.session.in_free_spins()),
        auto_tone,
        mouse,
    ) {
        actions.push(UiAction::ToggleAutospin);
    }

    if virtual_button(
        Rect::new(content.x + (third + 8.0) * 2.0, secondary_y, third, 38.0),
        "Paytable",
        true,
        ButtonTone::Secondary,
        mouse,
    ) {
        actions.push(UiAction::TogglePaytable);
    }
}

/// Save/load/new/delete, anchored to the bottom of the panel. Returns the top
/// of the block so the spin controls can sit on top of it.
fn draw_session_buttons(
    ctx: &UiContext<'_>,
    content: Rect,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) -> f32 {
    let half = (content.w - 10.0) / 2.0;
    let bottom_row = content.bottom() - 34.0;
    let top_row = bottom_row - 42.0;

    // Saving mid-feature would bank a session whose free spins are not
    // persisted, and loading mid-spin would strand a committed stake.
    let storage_ready = ctx.session.is_settled() && !ctx.session.in_free_spins();

    if virtual_button(
        Rect::new(content.x, top_row, half, 34.0),
        "Save",
        storage_ready,
        ButtonTone::Positive,
        mouse,
    ) {
        actions.push(UiAction::Save);
    }
    if virtual_button(
        Rect::new(content.x + half + 10.0, top_row, half, 34.0),
        "Load",
        storage_ready && ctx.save_exists,
        ButtonTone::Primary,
        mouse,
    ) {
        actions.push(UiAction::Load);
    }
    if virtual_button(
        Rect::new(content.x, bottom_row, half, 34.0),
        "New Game",
        ctx.session.phase.is_idle(),
        ButtonTone::Secondary,
        mouse,
    ) {
        actions.push(UiAction::NewGame);
    }
    if virtual_button(
        Rect::new(content.x + half + 10.0, bottom_row, half, 34.0),
        "Delete Save",
        ctx.session.phase.is_idle() && ctx.save_exists,
        ButtonTone::Danger,
        mouse,
    ) {
        actions.push(UiAction::DeleteSave);
    }

    top_row
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
    draw_ui_text_ex(
        "Space spins · Up/Down bet · M max · A autospin · G gamble · B buy · L ledger · P paytable · O settings · C machines · V awards",
        rect.x + 470.0,
        rect.y + 56.0,
        TextStyle::new(15.0, palette::TEXT_DIM).params(),
    );
}

fn virtual_button(rect: Rect, text: &str, enabled: bool, tone: ButtonTone, mouse: Vec2) -> bool {
    let style = ButtonStyle::from_tone(tone);
    let hovered = enabled && rect.contains_point(mouse);
    let pressed = hovered && is_mouse_button_down(MouseButton::Left);
    let activated = hovered && is_mouse_button_released(MouseButton::Left);
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

/// Keyboard shortcuts, mapped to the same intents the buttons produce.
///
/// While a celebration is showing, the spin key dismisses it instead — one key
/// to move the game forward, whatever it is currently waiting on.
pub fn actions_from_keys(celebrating: bool) -> Vec<UiAction> {
    let mut actions = Vec::new();
    if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
        actions.push(if celebrating {
            UiAction::DismissCelebration
        } else {
            UiAction::Spin
        });
    }
    if is_key_pressed(KeyCode::A) {
        actions.push(UiAction::ToggleAutospin);
    }
    if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Equal) {
        actions.push(UiAction::BetUp);
    }
    if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::Minus) {
        actions.push(UiAction::BetDown);
    }
    if is_key_pressed(KeyCode::M) {
        actions.push(UiAction::MaxBet);
    }
    if is_key_pressed(KeyCode::P) {
        actions.push(UiAction::TogglePaytable);
    }
    if is_key_pressed(KeyCode::O) {
        actions.push(UiAction::ToggleSettings);
    }
    if is_key_pressed(KeyCode::C) {
        actions.push(UiAction::ToggleMachines);
    }
    if is_key_pressed(KeyCode::V) {
        actions.push(UiAction::ToggleAchievements);
    }
    if is_key_pressed(KeyCode::B) {
        actions.push(UiAction::ToggleFeatureBuy);
    }
    if is_key_pressed(KeyCode::L) {
        actions.push(UiAction::ToggleLedger);
    }
    if is_key_pressed(KeyCode::G) {
        actions.push(UiAction::OfferGamble);
    }
    if is_key_pressed(KeyCode::S) {
        actions.push(UiAction::Save);
    }
    if is_key_pressed(KeyCode::L) {
        actions.push(UiAction::Load);
    }
    actions
}
