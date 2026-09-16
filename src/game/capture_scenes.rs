//! Capture-scene wiring for the headless screenshot harness.
//!
//! Every scene here fast-forwards a fresh, fixed-seed session into a state worth
//! photographing, then hands back to the normal loop. None of it runs in a real
//! session — it exists so a UI change can be verified without a human sitting at
//! the cabinet pulling the lever until the right thing happens.

use super::Game;
use crate::state::GameSession;

mod audit;
mod holds;
mod misc;
mod scenes;

impl Game {
    /// Fast-forward into a named state so the screenshot harness can photograph
    /// something other than the boot screen. Uses the headless spin path to skip
    /// ahead, then hands over to the normal loop.
    ///
    /// Scenes: `idle`, `spin` (reels mid-flight), `win`, `freespins`,
    /// `paytable`, `settings`, `feature_card`, `hatch`, `autospin`, `anticipation`,
    /// `wrath`.
    /// Swap the cabinet a scene is shot on.
    ///
    /// Not just an assignment: a cabinet brings its palette with it (§5.43), and
    /// six scenes setting `self.data` directly would each have to remember that.
    /// Always called with `machine_by_id`, never with an index (§5.61).
    ///
    /// The `cascade` scene asked for `MACHINES[3]` and got **Wyrmspire**, which
    /// has no cascades at all — so `hold_a_cascade` searched four thousand
    /// spins for a chain that could never come and the capture photographed a
    /// board doing nothing. Six iterations, with `ui_cascade` listed among this
    /// game's verification captures the whole time. A position in an array is
    /// not a name, and the six cabinets are not in the order anyone assumes.
    fn use_machine(&mut self, machine: &'static crate::data::MachineDef) {
        self.data = crate::data::GameData::load_machine(machine)
            .unwrap_or_else(|err| panic!("{}: {}", machine.id, err));
        crate::ui::theme::set(crate::ui::theme::by_name(self.data.theme_name()));
    }

    pub fn begin_capture_scene(&mut self, scene: &str) {
        // What the game found on the disk when it started (§5.58) — reported
        // before the line below throws it away, which is the whole reason this
        // arm is up here and not with the others. Prints rather than
        // photographs: the fault was that boot never read the save at all, and
        // no picture of a reel window shows that.
        if scene == "boot_report" {
            println!(
                "boot eggs {} pot {} spins {} mini_milli {}",
                self.session.hoard.count,
                self.session.hoard.pot,
                self.session.stats.total_spins,
                self.session.jackpots.accrued_milli(0)
            );
            std::process::exit(0);
        }

        // A fixed seed keeps every capture reproducible run to run.
        self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
        self.notifications.clear();

        match scene {
            "spin" => self.capture_spin_scene(),
            "win" => self.capture_win_scene(),
            "freespins" => self.capture_freespins_scene(),
            "feature_card" => self.capture_feature_card_scene(),
            "hatch" => self.capture_hatch_scene(),
            "autospin" => self.capture_autospin_scene(),
            "paytable" => self.show_paytable = true,
            "menu" => self.show_menu = true,
            "lines" => self.show_lines = true,
            "machines" => self.show_machines = true,
            "bonus" => self.capture_bonus_scene(),
            "achievements" => self.capture_achievements_scene(),
            "jackpot" => self.capture_jackpot_scene(),
            "cascade" => self.capture_machine_feature_scene("avalanche"),
            "shifting" => self.capture_machine_feature_scene("wyrmspire"),
            "ways" => self.capture_machine_feature_scene("ways"),
            "refining" => self.capture_refining_scene(),
            "frost" => self.capture_machine_feature_scene("frost"),
            "wrath" => self.hold_a_wrath_round(),
            "seam" => self.hold_a_seam(false),
            "seam_running" => self.hold_a_seam(true),
            "wallet_walk" => self.capture_wallet_walk_scene(),
            "screens" => self.capture_screens_scene(),
            "sessionover" => self.capture_session_over_scene(),
            "ante" => self.session.preferences.ante = true,
            "proofs" => self.capture_proofs_scene(),
            "proofs_tampered" => self.capture_tampered_proofs_scene(),
            "sessions" => self.capture_sessions_scene(),
            "ruin" => self.capture_ruin_scene(false),
            "ruin_vault" => self.capture_ruin_scene(true),
            "featurebuy" => self.capture_featurebuy_scene(),
            "gamble" => self.capture_gamble_scene(),
            "ledger" => self.capture_ledger_scene(),
            "history" => self.capture_history_scene(),
            "history_long" => self.capture_long_history_scene(),
            "cluster" => self.capture_cluster_scene(),
            "layout_audit" => self.capture_layout_audit(),
            "touch_audit" => self.capture_touch_audit(None),
            "touch_audit_settings" => self.capture_touch_audit(Some("settings")),
            "touch_audit_buy" => self.capture_touch_audit(Some("buy")),
            scene if scene.starts_with("motion:") => self.capture_motion_scene(scene),
            scene if scene.starts_with("audit:") => self.capture_screen_audit(scene),
            "rules" => self.show_rules = true,
            "limits" => self.capture_limits_scene(),
            "reality" => self.capture_reality_scene(),
            "rules_avalanche" => self.capture_rules_machine_scene(Some("avalanche")),
            "rules_frost" => self.capture_rules_machine_scene(Some("frost")),
            "waveforms" => self.show_waveforms = true,
            "vision" => self.show_vision = true,
            "hint" => self.capture_hint_scene(),
            "keyboard" => self.capture_keyboard_scene(),
            "settings" => self.show_settings = true,
            "anticipation" => self.hold_a_near_miss(),
            _ => {}
        }
    }
}
