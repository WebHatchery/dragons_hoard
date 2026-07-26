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

use crate::state::wallet::Wallet;
use crate::state::{migrate_save_value, GameSession, SaveData};
use macroquad_toolkit::rng::random_u64;

impl Game {
    /// Whether this process is allowed to write to the player's saves.
    ///
    /// False under the screenshot harness, and it should have been false since
    /// the harness was written (§5.55). Capture scenes deal winning boards,
    /// empty a balance to reach the ruin screen, and walk between cabinets —
    /// and the game loop autosaves when a spin resolves, so all of it was going
    /// straight into the real save slots. `verify.ps1` runs about fifty
    /// captures. Running the verification suite destroyed the player's game,
    /// every time, and nothing said so.
    ///
    /// Muting the audio was already conditional on exactly this (a headless run
    /// has no sound card); nobody asked the same question about the disk.
    fn may_persist(&self) -> bool {
        !macroquad_toolkit::capture::capture_requested("DRAGONS_HOARD")
    }

    /// Autosave once a spin has fully resolved. Mid-feature state is not saved:
    /// a reload lands back in the base game.
    pub(crate) fn autosave(&mut self) {
        if self.session.in_free_spins() || !self.may_persist() {
            return;
        }
        self.store_wallet();
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
        if !self.may_persist() {
            return;
        }
        self.store_wallet();
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
        if !self.may_persist() {
            return;
        }
        match delete_slot(&self.data.config.game_name, &self.data.save_slot()) {
            Ok(()) => {
                self.notifications.info("Save slot cleared");
                self.refresh_save_state();
            }
            Err(err) => self.notifications.danger(format!("Delete failed: {}", err)),
        }
    }

    /// Write the balance where it belongs: to the player, not the cabinet
    /// (§5.55).
    pub(crate) fn store_wallet(&mut self) {
        if !self.may_persist() {
            return;
        }
        let wallet = Wallet {
            balance: self.session.balance,
            staked: self.session.stats.staked,
        };
        let _ = wallet.save(&self.data.config);
    }

    /// Read it back, absorbing the old per-cabinet balances the first time.
    ///
    /// The closure is how the absorb step reaches six saves it cannot see from
    /// inside `state` — the wallet knows the rule, persistence knows the files.
    pub(crate) fn restore_wallet(&mut self) {
        let game_name = self.data.config.game_name.clone();
        let version = self.data.config.version.clone();
        let wallet = Wallet::load(&self.data.config, &|machine_id| {
            let slot = format!("{}_{}", machine_id, self.data.config.save_slot);
            if !slot_exists(&game_name, &slot) {
                return None;
            }
            load_from_slot_with_migration::<SaveData, _>(&game_name, &slot, &version, |v, value| {
                migrate_save_value(v, value, &self.data)
            })
            .ok()
            .map(|save| save.balance)
        });
        self.session.balance = wallet.balance;
        self.session.stats.staked = wallet.staked;
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

        let mut session = match loaded {
            Ok(save) => GameSession::from_save(&self.data, save),
            Err(_) => GameSession::new(&self.data, random_u64()),
        };
        // The hoard, the jackpots and the stats came from the cabinet. The money
        // did not — it is the player's, and it followed them here (§5.55).
        session.balance = self.session.balance;
        session.stats.staked = self.session.stats.staked;
        session
    }
}
