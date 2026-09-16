//! Capture scenes that arm layout, touch, and motion diagnostics.

use super::super::Game;
use crate::state::GameSession;

impl Game {
    pub(super) fn capture_layout_audit(&mut self) {
        self.session.balance = 1_987_654_321;
        self.session.stats.biggest_win = 987_654_321;
        self.session.hoard.pot = 87_654_321;
        self.show_paytable = true;
        self.show_rules = true;
        self.show_settings = true;
        self.show_machines = true;
        self.show_achievements = true;
        self.show_featurebuy = true;
        self.show_ledger = true;
        self.show_limits = true;
        self.show_history = true;
        self.show_waveforms = true;
        self.show_vision = true;
        self.reality_check = true;
        if let Ok(scale) = std::env::var("DRAGONS_HOARD_TEXT_SCALE") {
            if let Ok(scale) = scale.parse::<f32>() {
                macroquad_toolkit::ui::set_ui_text_scale(scale);
            }
        }
        if let Ok(name) = std::env::var("DRAGONS_HOARD_THEME") {
            crate::ui::theme::set(crate::ui::theme::by_name(&name));
        }
        macroquad_toolkit::ui::begin_audit();
    }

    pub(super) fn capture_touch_audit(&mut self, panel: Option<&str>) {
        match panel {
            Some("settings") => self.show_settings = true,
            Some("buy") => self.show_featurebuy = true,
            None | Some(_) => {}
        }
        macroquad_toolkit::ui::begin_target_audit();
        macroquad_toolkit::ui::begin_audit();
        macroquad_toolkit::ui::begin_collision_audit();
    }

    pub(super) fn capture_motion_scene(&mut self, scene: &str) {
        let wanted = &scene["motion:".len()..];
        let Some(machine) = crate::data::MACHINES
            .iter()
            .find(|machine| machine.id == wanted)
        else {
            panic!("no cabinet called '{}'", wanted);
        };
        self.use_machine(machine);
        self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
        self.begin_motion_audit();
        let _ = self.session.begin_spin(&self.data);
    }

    pub(super) fn capture_screen_audit(&mut self, scene: &str) {
        // Pin the cabinet before anything is measured (§5.76). A stable
        // environment variable makes the sweep independent of player saves.
        macroquad_toolkit::ui::begin_target_audit();
        let machine = std::env::var("DRAGONS_HOARD_MACHINE")
            .ok()
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| crate::data::MACHINES[0].id.to_owned());
        self.use_machine(crate::data::machine_by_id(&machine));
        self.session = GameSession::new(&self.data, 0xD2A6_0F1E);

        let wanted = &scene["audit:".len()..];
        let Some(screen) = crate::game::screens::Screen::ALL
            .iter()
            .find(|screen| screen.id() == wanted)
            .copied()
        else {
            panic!("no screen called '{}'", wanted);
        };
        if !self.open_screen(screen) {
            // Dealt, not opened: use the scene that knows how to reach it.
            self.begin_capture_scene(screen.id());
        }
        assert!(
            self.screen_open(screen),
            "audit:{} did not reach the screen — the capture scene named \
             '{}' is missing or no longer opens it",
            wanted,
            screen.id()
        );
        macroquad_toolkit::ui::begin_audit();
        macroquad_toolkit::ui::begin_collision_audit();
        macroquad_toolkit::ui::begin_target_audit();
    }
}
