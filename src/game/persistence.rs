//! Reading and writing the save slot.
//!
//! Split out of `game.rs` when it reached the 800-line limit. It is a clean
//! seam: every method here is the same shape — call the toolkit, tell the
//! player what happened, and refresh whether a slot exists — and none of it
//! touches the frame loop.

use super::Game;
use macroquad_toolkit::persistence::{
    delete_slot, load_from_slot_with_migration, save_to_slot_with_version, slot_exists,
};

use crate::state::{migrate_save_value, GameSession, SaveData};
use macroquad_toolkit::rng::random_u64;

impl Game {
    /// Autosave once a spin has fully resolved. Mid-feature state is not saved:
    /// a reload lands back in the base game.
    pub(crate) fn autosave(&mut self) {
        if self.session.in_free_spins() {
            return;
        }
        let save = self.session.to_save(&self.data.config.version);
        if save_to_slot_with_version(
            &self.data.config.game_name,
            &self.data.save_slot(),
            &save,
            &self.data.config.version,
        )
        .is_ok()
        {
            self.save_exists = true;
        }
    }

    pub(crate) fn save_game(&mut self) {
        let save = self.session.to_save(&self.data.config.version);
        match save_to_slot_with_version(
            &self.data.config.game_name,
            &self.data.save_slot(),
            &save,
            &self.data.config.version,
        ) {
            Ok(()) => {
                self.notifications.success("Hoard recorded");
                self.refresh_save_state();
            }
            Err(err) => self.notifications.danger(format!("Save failed: {}", err)),
        }
    }

    pub(crate) fn load_game(&mut self) {
        let loaded: Result<SaveData, String> = load_from_slot_with_migration(
            &self.data.config.game_name,
            &self.data.save_slot(),
            &self.data.config.version,
            |version, value| migrate_save_value(version, value, &self.data),
        );

        match loaded {
            Ok(save) => {
                let preferences = self.session.preferences.clone();
                self.session = GameSession::from_save(&self.data, save);
                self.session.preferences = preferences;
                self.particles.clear();
                self.floating.clear();
                self.session.celebrations.clear();
                self.notifications.success("Hoard restored");
                self.refresh_save_state();
            }
            Err(err) => self.notifications.warning(format!("Load failed: {}", err)),
        }
    }

    pub(crate) fn delete_save(&mut self) {
        match delete_slot(&self.data.config.game_name, &self.data.save_slot()) {
            Ok(()) => {
                self.notifications.info("Save slot cleared");
                self.refresh_save_state();
            }
            Err(err) => self.notifications.danger(format!("Delete failed: {}", err)),
        }
    }

    pub(crate) fn refresh_save_state(&mut self) {
        self.save_exists = slot_exists(&self.data.config.game_name, &self.data.save_slot());
    }

    /// The target machine's saved session, or a fresh one if it has never been
    /// played.
    pub(crate) fn load_machine_session(&mut self) -> GameSession {
        let loaded: Result<SaveData, String> = load_from_slot_with_migration(
            &self.data.config.game_name,
            &self.data.save_slot(),
            &self.data.config.version,
            |version, value| migrate_save_value(version, value, &self.data),
        );

        match loaded {
            Ok(save) => GameSession::from_save(&self.data, save),
            Err(_) => GameSession::new(&self.data, random_u64()),
        }
    }
}
