//! The Feature Buy menu (§5.13).
//!
//! Each row is a tier: what it gives, what it costs, and whether it can be
//! afforded right now. The price shown here is the price charged — a test
//! asserts it (`the_price_charged_is_the_price_shown`), because a menu that
//! quotes one figure and takes another is the worst bug this screen could have.
//!
//! Rows the player cannot afford are drawn dimmed and are not clickable, rather
//! than being hidden. A price you can see but cannot yet meet is a goal; a row
//! that vanishes is a mystery.

use crate::data::GameData;
use crate::engine::sim::BAND_LABELS;
use crate::state::profile::{ProfileBook, TierProfile};
use crate::state::{featurebuy, GameSession};
use crate::ui::naming;
use crate::ui::nav::Nav;
use crate::ui::{palette, virtual_button, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::ui::{
    draw_surface, draw_text_right, draw_ui_text_ex, ButtonTone, RectExt, Region, SurfaceStyle,
    TextStyle,
};

const ROW_HEIGHT: f32 = 128.0;

pub fn draw(
    data: &GameData,
    session: &GameSession,
    profiles: &ProfileBook,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
    nav: &mut Nav,
) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.82),
    );

    let tiers = &data.featurebuy.tiers;
    let height = 152.0 + tiers.len() as f32 * ROW_HEIGHT;
    let panel = Rect::new(260.0, (LOGICAL_HEIGHT - height) * 0.5, 760.0, height);
    // Everything drawn below is measured against this panel (§5.37).
    let _region = Region::new(panel);
    draw_surface(
        panel,
        &SurfaceStyle::new(palette::STONE)
            .with_border(2.0, palette::GOLD)
            .with_header(48.0, palette::STONE_HEADER)
            .with_header_divider(1.0, palette::GOLD_DIM),
    );
    draw_ui_text_ex(
        "Buy a feature",
        panel.x + 20.0,
        panel.y + 32.0,
        TextStyle::new(21.0, palette::GOLD_BRIGHT).params(),
    );

    let total_bet = session.total_bet(data);
    draw_text_right(
        &format!(
            "Total bet {}  ·  Balance {}",
            naming::credits(total_bet),
            naming::credits(session.balance)
        ),
        panel.right() - 150.0,
        panel.y + 32.0,
        TextStyle::new(15.0, palette::TEXT_DIM),
    );

    let mut y = panel.y + 68.0;
    for (index, tier) in tiers.iter().enumerate() {
        let price = featurebuy::price(tier, total_bet);
        let affordable = session.can_buy(index, data);
        let row = Rect::new(panel.x + 20.0, y, panel.w - 40.0, ROW_HEIGHT - 10.0);

        draw_surface(
            row,
            &SurfaceStyle::new(if affordable {
                Color::new(0.10, 0.09, 0.06, 1.0)
            } else {
                Color::new(0.07, 0.06, 0.06, 1.0)
            })
            .with_border(1.0, palette::GOLD_DIM),
        );

        let title = if affordable {
            palette::GOLD_BRIGHT
        } else {
            palette::TEXT_DIM
        };
        draw_ui_text_ex(
            &tier.name,
            row.x + 16.0,
            row.y + 28.0,
            TextStyle::new(19.0, title).params(),
        );
        draw_ui_text_ex(
            &tier.description,
            row.x + 16.0,
            row.y + 52.0,
            TextStyle::new(14.0, palette::TEXT_DIM).params(),
        );

        draw_tier_profile(
            profiles.tier(data.machine_id(), index),
            profiles.tier_progress(data.machine_id(), index),
            Rect::new(row.x + 16.0, row.y + 62.0, row.w - 210.0, 50.0),
        );

        let button = Rect::new(row.right() - 176.0, row.y + 18.0, 160.0, 46.0);
        if affordable {
            if virtual_button(
                button,
                &format!("Buy {}", naming::credits(price)),
                true,
                ButtonTone::Primary,
                mouse,
                nav,
            ) {
                actions.push(UiAction::BuyFeature(index));
            }
        } else {
            // Not a disabled button — a plain plate. A button that looks
            // pressable and does nothing is worse than one that never did.
            draw_surface(
                button,
                &SurfaceStyle::new(Color::new(0.09, 0.08, 0.09, 1.0))
                    .with_border(1.0, Color::new(0.0, 0.0, 0.0, 0.5)),
            );
            draw_ui_text_ex(
                &format!("{} needed", price),
                button.x + 14.0,
                button.y + 29.0,
                TextStyle::new(15.0, palette::TEXT_DIM).params(),
            );
        }

        y += ROW_HEIGHT;
    }

    draw_ui_text_ex(
        "Prices come from what each feature actually pays. The figures below them are measured the same way.",
        panel.x + 20.0,
        panel.bottom() - 46.0,
        TextStyle::new(14.0, palette::TEXT_DIM).params(),
    );

    if virtual_button(
        Rect::new(panel.right() - 130.0, panel.y + 10.0, 110.0, 30.0),
        "Close",
        true,
        ButtonTone::Danger,
        mouse,
        nav,
    ) {
        actions.push(UiAction::ToggleFeatureBuy);
    }

    // Clicking outside the panel closes it, which is what every other overlay
    // in the game does.
    if is_mouse_button_released(MouseButton::Left) && !panel.inset(-4.0).contains(mouse) {
        actions.push(UiAction::ToggleFeatureBuy);
    }
}

/// What a tier actually does when you buy it (§5.22).
///
/// The price says what a feature costs. This says what to expect for it — and
/// the headline is the share of buys that come back under the price, which is
/// the number the decision really turns on and the one no cabinet shows.
fn draw_tier_profile(profile: Option<&TierProfile>, progress: f32, rect: Rect) {
    let Some(profile) = profile else {
        let bar = Rect::new(rect.x, rect.y + 12.0, rect.w * 0.4, 8.0);
        draw_surface(
            bar,
            &SurfaceStyle::new(Color::new(0.07, 0.06, 0.07, 1.0))
                .with_border(1.0, palette::GOLD_DIM),
        );
        draw_rectangle(
            bar.x + 1.0,
            bar.y + 1.0,
            (bar.w - 2.0) * progress,
            bar.h - 2.0,
            palette::EMBER,
        );
        draw_ui_text_ex(
            "measuring this feature...",
            bar.right() + 12.0,
            rect.y + 20.0,
            TextStyle::new(13.0, palette::TEXT_DIM).params(),
        );
        return;
    };

    draw_ui_text_ex(
        &format!(
            "{:.0}% of buys come back under the price   ·   best seen {:.0}x",
            profile.below_cost * 100.0,
            profile.best
        ),
        rect.x,
        rect.y + 14.0,
        TextStyle::new(13.0, palette::TEXT_BRIGHT).params(),
    );

    let bar = Rect::new(rect.x, rect.y + 22.0, rect.w, 12.0);
    let mut x = bar.x;
    for (index, share) in profile.bands.iter().enumerate() {
        let width = bar.w * *share as f32;
        if width < 0.5 {
            continue;
        }
        let heat = index as f32 / (profile.bands.len() as f32 - 1.0);
        draw_rectangle(
            x,
            bar.y,
            width,
            bar.h,
            Color::new(0.10 + 0.72 * heat, 0.09 + 0.30 * heat, 0.10, 1.0),
        );
        x += width;
    }
    draw_surface(
        bar,
        &SurfaceStyle::new(Color::new(0.0, 0.0, 0.0, 0.0)).with_border(1.0, palette::GOLD_DIM),
    );
    draw_ui_text_ex(
        BAND_LABELS[0],
        bar.x,
        bar.bottom() + 13.0,
        TextStyle::new(11.0, palette::TEXT_DIM).params(),
    );
}
