//! The part you feel rather than read.
//!
//! Shake, particles, floating numbers and the punch a card lands with. All of it
//! is downstream of a `SpinEvent` or a `CelebrationKind` — nothing here decides
//! anything, it only dresses a decision already made.
//!
//! Both gates live in `add_trauma` and `burst`, so no routine in this file can
//! bypass a player who has turned motion or particles off (§5.7).

use super::Game;
use crate::audio::Sfx;
use crate::state::celebration::CelebrationKind;
use crate::state::SpinResolution;
use crate::ui::{self, palette};
use macroquad::prelude::*;
use macroquad_toolkit::fx::BurstConfig;

impl Game {
    /// Punch up a card as it opens. The card itself is drawn by the UI; this is
    /// the part you feel rather than read.
    pub(super) fn celebrate(&mut self, kind: &CelebrationKind) {
        match kind {
            // The rarest event in the game gets the loudest presentation.
            CelebrationKind::Jackpot { .. } => {
                self.add_trauma(1.0);
                self.sound.play(Sfx::Hatch);
                self.spawn_hatch_burst();
                self.spawn_hatch_burst();
            }
            CelebrationKind::Hatch { .. } => {
                self.add_trauma(0.9);
                self.sound.play(Sfx::Hatch);
                self.spawn_hatch_burst();
            }
            // A full board is the top of the feature and is dressed like it.
            CelebrationKind::Wrath { full_board, .. } => {
                self.add_trauma(if *full_board { 1.0 } else { 0.8 });
                self.sound.play(Sfx::Hatch);
                self.spawn_hatch_burst();
                if *full_board {
                    self.spawn_hatch_burst();
                }
            }
            CelebrationKind::FreeSpinsEntry { .. } => {
                self.add_trauma(0.7);
                self.sound.play(Sfx::Scatter);
                self.spawn_hatch_burst();
            }
            // A bust gets shake but no burst: it is the one card in the game
            // that is not good news, and showering it in gold would read wrong.
            CelebrationKind::GambleLost { .. } => {
                self.add_trauma(0.45);
                self.sound.play_at(Sfx::ReelStop, 0.45);
            }
            CelebrationKind::FreeSpinsRetrigger { .. } => {
                self.add_trauma(0.5);
                self.sound.play(Sfx::Scatter);
            }
            CelebrationKind::BigWin { .. } => {
                self.add_trauma(0.5);
                self.sound.play(Sfx::WinBig);
            }
            CelebrationKind::FreeSpinsSummary { .. } => self.sound.play(Sfx::WinSmall),
        }
    }

    /// Ordinary feedback for an ordinary spin. The big moments — a feature
    /// starting or ending, a hatch, a big win — are raised as celebration cards
    /// by the session instead, so nothing is announced twice.
    pub(super) fn report_spin(&mut self, resolution: &SpinResolution) {
        let credits = resolution.total_credits();
        self.spawn_win_text(resolution);
        self.record_achievements(resolution);

        // A big win gets its sound and its announcement from its celebration
        // card instead, so neither is heard or read twice.
        if credits > 0 && credits < self.session.big_win_threshold(&self.data) {
            self.sound.play(Sfx::WinSmall);
            let message = if resolution.was_free_spin {
                format!("Free spin win {} credits", credits)
            } else {
                format!("Win {} credits", credits)
            };
            self.notifications.info(message);
        }

        // Nothing to count up means the payout phase is skipped, so save here.
        if credits == 0 {
            self.autosave();
        }
    }

    /// One rising number per win, anchored to the last cell that formed it so
    /// the player can see *which* combination paid.
    ///
    /// Reads the win's own cells rather than looking up a payline, so a ways win
    /// — which has no line to look up — lands in the right place too.
    pub(super) fn spawn_win_text(&mut self, resolution: &SpinResolution) {
        let outcome = resolution.outcome();
        let rows = self.data.config.row_count.max(1);

        for win in outcome.wins.iter().take(6) {
            let Some(cell) = win.cells.last().copied() else {
                continue;
            };
            let (reel, row) = (cell / rows, cell % rows);
            let position = ui::reels::cell_center(&self.data, reel, row);
            self.floating
                .spawn(format!("+{}", win.credits), position, palette::GOLD_BRIGHT);
            if win.credits >= self.session.total_bet(&self.data) {
                self.spawn_win_burst(position);
            }
        }

        if outcome.scatter_credits > 0 {
            self.floating.spawn(
                format!("Scatter +{}", outcome.scatter_credits),
                ui::reels::grid_center(),
                palette::EMBER,
            );
        }
    }

    pub(super) fn spawn_win_burst(&mut self, position: Vec2) {
        self.burst(
            position,
            18,
            &BurstConfig {
                speed: (60.0, 190.0),
                size: (1.5, 3.5),
                life: (0.35, 0.8),
                colors: vec![palette::GOLD_BRIGHT, palette::GOLD, palette::EMBER],
                gravity: 220.0,
                ..Default::default()
            },
        );
    }

    pub(super) fn spawn_hatch_burst(&mut self) {
        self.burst(
            ui::celebration::card_center(),
            120,
            &BurstConfig {
                speed: (90.0, 340.0),
                size: (2.0, 5.0),
                life: (0.6, 1.4),
                colors: vec![
                    palette::GOLD_BRIGHT,
                    palette::GOLD,
                    palette::EMBER,
                    Color::new(1.0, 1.0, 0.92, 1.0),
                ],
                gravity: 260.0,
                ..Default::default()
            },
        );
    }

    pub(super) fn spawn_reel_stop_dust(&mut self, reel: usize) {
        let position = ui::reels::reel_foot(&self.data, reel);
        self.burst(
            position,
            6,
            &BurstConfig {
                speed: (30.0, 90.0),
                size: (1.0, 2.2),
                life: (0.18, 0.4),
                colors: vec![Color::new(0.65, 0.58, 0.48, 0.8)],
                direction: -std::f32::consts::FRAC_PI_2,
                spread: std::f32::consts::PI * 0.8,
                gravity: 180.0,
                ..Default::default()
            },
        );
    }
}
