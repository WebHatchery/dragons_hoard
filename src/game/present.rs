//! Handing the game to the view layer.
//!
//! One method, and it is almost entirely a struct literal: everything the UI is
//! allowed to see, gathered in one place. That is the whole architecture stated
//! as code — the view reads a context and returns `UiAction`s, and there is no
//! other channel between them.
//!
//! Split out of `game.rs` when it crossed 800 lines (§5.74). The dividing line
//! is a real one: next door is a game that reacts to things, and this is the
//! frame where it is looked at.

use super::Game;
use crate::ui::{self, palette, UiContext};
use macroquad::prelude::*;
use macroquad_toolkit::notifications::{NotificationAnchor, NotificationRenderConfig};
use macroquad_toolkit::prelude::{begin_virtual_ui_frame, end_virtual_ui_frame};

impl Game {
    pub fn draw(&mut self) {
        clear_background(palette::background());

        // The logical width follows the window's shape (§5.46); the height is
        // fixed, because every panel's vertical layout was written against it.
        let (logical_width, logical_height) =
            ui::frame::logical_size(screen_width(), screen_height());
        let frame = ui::frame::Frame::sized(logical_width, logical_height);
        ui::frame::set_width(logical_width);
        ui::frame::set_height(logical_height);
        let virtual_ui = begin_virtual_ui_frame(logical_width, logical_height);
        let actions = ui::draw_game_ui(
            UiContext {
                data: &self.data,
                session: &self.session,
                save_exists: self.save_exists,
                show_paytable: self.show_paytable,
                show_settings: self.show_settings,
                show_machines: self.show_machines,
                show_achievements: self.show_achievements,
                show_featurebuy: self.show_featurebuy,
                ledger: &self.ledger,
                show_ledger: self.show_ledger,
                show_lines: self.show_lines,
                show_menu: self.show_menu,
                show_sessions: self.show_sessions,
                sessions: &self.sessions,
                proofs: &self.proofs,
                checked: &self.checked,
                show_proofs: self.show_proofs,
                session_over_dismissed: self.session_over_dismissed,
                show_rules: self.show_rules,
                limits: &self.limits,
                limit_choices: &self.limit_choices,
                show_limits: self.show_limits,
                history: &self.history,
                show_history: self.show_history,
                reality_check: self.reality_check,
                show_waveforms: self.show_waveforms,
                music_levels: self.music.levels(),
                music_mood: self.music.mood(),
                music_arrangement: self.music.arrangement(),
                show_vision: self.show_vision,
                // Not while a panel is up (§5.28). A hint offers something to
                // do next, and behind a modal there is nothing to do next — it
                // also drew across the bottom edge of the panel covering it.
                hint: if self.any_overlay_open() {
                    None
                } else {
                    self.hints
                        .current(self.achievements.progress(), &self.ledger)
                },
                profiles: &self.profiles,
                achievements: &self.achievements,
                shake: self.shake.offset(),
                ui_time: self.ui_time,
                ui: &virtual_ui,
                frame,
                overlay_open: self.any_overlay_open(),
            },
            &mut self.nav,
        );

        // Particles and floating text live in logical space, so they belong
        // inside the virtual frame alongside the UI they annotate.
        self.particles.draw();
        self.floating.draw();
        end_virtual_ui_frame();

        // The layout audit (§5.37) runs while the game is genuinely drawing,
        // because measuring text needs the real font. One frame is enough: the
        // panels redraw identically, and the recorder de-duplicates anyway.
        // Touch targets are their own audit and their own scenes (§5.45):
        // sizes want every panel, overlaps want one screen at a time.
        let warm = macroquad_toolkit::ui::neighbours_warm();
        if let Some((width, worst)) =
            macroquad_toolkit::ui::smallest_touchable_width(ui::logical_width()).filter(|_| warm)
        {
            println!(
                "touch targets: need a {:.0}px-wide window; worst is {}",
                width, worst
            );
            for (side, label) in macroquad_toolkit::ui::undersized_targets() {
                println!("touch targets: drawn {}px — {}", side, label);
            }
            for (a, b, area) in macroquad_toolkit::ui::overlapping_targets() {
                println!(
                    "touch targets: {} and {} overlap by {:.0}px² once grown",
                    a, b, area
                );
            }
        }

        if macroquad_toolkit::ui::auditing() {
            let findings = macroquad_toolkit::ui::take_audit();
            if findings.is_empty() {
                println!("layout audit: clean");
            } else {
                for finding in &findings {
                    println!(
                        "layout audit: {} — {:?}",
                        finding.describe(),
                        finding.text()
                    );
                }
                println!("layout audit: {} findings", findings.len());
                // A gate, not a report. A printout nobody reads is the state
                // this replaced — four overflow defects shipped and were found
                // by looking at screenshots (§5.37).
                std::process::exit(1);
            }
        }

        for action in actions {
            self.events.push(action);
        }

        self.notifications
            .draw_with_config(&NotificationRenderConfig {
                anchor: NotificationAnchor::BottomRight,
                ..Default::default()
            });
    }
}
