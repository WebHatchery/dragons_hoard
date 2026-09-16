//! One bankroll, six cabinets (§5.55).
//!
//! # A hole under the floor of §5.53
//!
//! Every cabinet had its own save slot, and a balance inside it. Walk from
//! Dragon's Hoard to Frost Wyrm and you arrived at a **fresh thousand credits**.
//! Six cabinets, six starting stacks, and a New Game button under each of them.
//!
//! That quietly made the previous iteration meaningless. §5.53 offers a player
//! who has run out a real decision — break the hoard at half its value and lose
//! every egg on the meter, or take a stake the game will count against you — and
//! both cost something. Neither costs anything at all if a full stack is one
//! click away in the machine picker. The careful thing was sitting on top of a
//! trapdoor.
//!
//! It also made the ledger's per-cabinet returns describe six unrelated
//! economies rather than one player's session.
//!
//! # Money belongs to the player, not the machine
//!
//! Which is how a real floor works: you carry your money between machines and
//! the machines keep their own jackpots. So the balance moves out of the
//! per-cabinet save into a wallet of its own, and switching cabinets carries it
//! across untouched.
//!
//! Everything else stays where it was, because everything else genuinely does
//! belong to the machine. A hoard meter is *that cabinet's* eggs, at that
//! cabinet's line bet. A progressive pot is funded by play on that cabinet and
//! paying it out anywhere else would be theft from the people who fed it. The
//! ledger is per-cabinet *because* the whole point is comparing them.
//!
//! The cabinet stops being a separate game you start over, and becomes a
//! different table to take your money to — which is what the machine picker
//! always claimed it was.
//!
//! # What happens to money already saved
//!
//! §5.49's rule: adding content must never strand a player, and a save written
//! before today has a balance in it. Six of them might.
//!
//! So the first time the wallet is opened it **absorbs** every cabinet's saved
//! balance and adds them together. Not the largest, not the current one — all of
//! them. Whatever the shape of the economy the player was in, that money was
//! theirs, and a migration that quietly deleted five sixths of it would be the
//! worst bug this game could ship.

use crate::data::{GameConfig, MACHINES};
use macroquad_toolkit::persistence::{load_from_slot, save_to_slot, slot_exists};
use serde::{Deserialize, Serialize};

/// The slot the wallet lives in. Deliberately not per-cabinet — that is the
/// entire point.
const SLOT: &str = "wallet";

/// The player's money, and what has been advanced to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wallet {
    pub balance: i64,
    /// Credits the vault has advanced across every cabinet (§5.53).
    ///
    /// Session stats reset when a session does; this does not, because a player
    /// who has been staked eleven times should not be able to make that go away
    /// by switching machines.
    #[serde(default)]
    pub staked: i64,
}

impl Wallet {
    /// A new player's stack, from the cabinet they booted into.
    pub fn new(config: &GameConfig) -> Self {
        Self {
            balance: config.starting_balance,
            staked: 0,
        }
    }

    /// Read the wallet, creating it from whatever the old per-cabinet saves held
    /// the first time it is asked for.
    pub fn load(config: &GameConfig, per_cabinet: &dyn Fn(&str) -> Option<i64>) -> Self {
        if slot_exists(&config.game_name, SLOT) {
            match load_from_slot::<Self>(&config.game_name, SLOT) {
                Ok(wallet) => return wallet,
                Err(error) => eprintln!("Dragon's Hoard wallet could not be loaded: {error}"),
            }
        }
        Self::absorb(config, per_cabinet)
    }

    /// Gather every cabinet's saved balance into one.
    ///
    /// Adding rather than choosing: all of it was the player's, and the
    /// alternative is a migration that deletes money.
    fn absorb(config: &GameConfig, per_cabinet: &dyn Fn(&str) -> Option<i64>) -> Self {
        let mut found = false;
        let mut balance = 0;
        for machine in MACHINES {
            if let Some(saved) = per_cabinet(machine.id) {
                balance += saved;
                found = true;
            }
        }
        if !found {
            return Self::new(config);
        }
        Self { balance, staked: 0 }
    }

    pub fn save(&self, config: &GameConfig) -> Result<(), String> {
        if !crate::state::persist::may_write() {
            return Ok(());
        }
        save_to_slot(&config.game_name, SLOT, self)
    }
}

#[cfg(test)]
mod tests;
