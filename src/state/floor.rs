//! The jackpot that belongs to the floor rather than to a machine (§5.57).
//!
//! # Six Grands nobody could ever fill
//!
//! Every cabinet carried its own four-tier ladder, and the Grand sits behind
//! odds of one win per 12.5 million credits of turnover. At twenty credits a
//! spin that is **625,000 spins** — per cabinet, on a ladder that resets to its
//! seed when it pays. Six of them, each fed only by the play on that one
//! machine, is six pots that will realistically never be seen, and a headline
//! number on the reel window that was decoration.
//!
//! Meanwhile §5.55 had just made the money one bankroll across all six cabinets.
//! The player is one player on a floor of machines; the biggest prize on that
//! floor should be the floor's.
//!
//! # The change is where the pot lives, not what it costs
//!
//! The Grand is not a new tier and nothing about its maths moves: same seed,
//! same share of the contribution, same odds. Only its **accrual** moves out of
//! the per-cabinet save into a store of its own, so every spin on every cabinet
//! feeds one pot and any cabinet can drop it.
//!
//! That matters for §4's numbers: a single-cabinet simulation contributes to the
//! shared pot and wins it back at exactly the rate it always did, so measured
//! RTP is untouched and the million-spin gate is comparing like with like. What
//! changes is the *real* game, where six cabinets fill one pot six times as
//! fast — which is the whole point of a linked jackpot and is why real floors
//! run them.
//!
//! # Keyed by tier id, not by index
//!
//! Cabinets are free to declare different ladders, and one of them will. A
//! shared pot addressed by position would hand Emberfall's third tier the money
//! Tidepool's third tier had banked. The store is a map from tier id to accrued
//! milli-credits, so a cabinet that does not declare `grand` simply never
//! touches it.

use crate::data::{GameConfig, Jackpots};
use crate::state::jackpot::JackpotState;
use macroquad_toolkit::persistence::{load_from_slot, save_to_slot, slot_exists};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Its own slot, deliberately not per-cabinet — §5.56 made that safe to do on
/// the web as well as natively.
const SLOT: &str = "floor";

/// What the floor's shared pots have banked, keyed by tier id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Floor {
    #[serde(default)]
    pub accrued_milli: HashMap<String, i64>,
}

impl Floor {
    pub fn load(config: &GameConfig) -> Self {
        if slot_exists(&config.game_name, SLOT) {
            match load_from_slot::<Self>(&config.game_name, SLOT) {
                Ok(floor) => return floor,
                Err(error) => eprintln!("Dragon's Hoard floor could not be loaded: {error}"),
            }
        }
        Self::default()
    }

    pub fn save(&self, config: &GameConfig) -> Result<(), String> {
        if !crate::state::persist::may_write() {
            return Ok(());
        }
        save_to_slot(&config.game_name, SLOT, self)
    }

    /// Push the floor's banked value into a cabinet's ladder.
    ///
    /// Called after a session is built, so a cabinet the player has never opened
    /// still shows the pot everyone else has been filling.
    pub fn lend_to(&self, jackpots: &Jackpots, state: &mut JackpotState) {
        for (index, tier) in jackpots.tiers.iter().enumerate() {
            if !tier.shared {
                continue;
            }
            if let Some(banked) = self.accrued_milli.get(&tier.id) {
                state.set_accrued_milli(index, *banked);
            }
        }
    }

    /// Take a cabinet's shared tiers back, so the next cabinet sees them.
    ///
    /// Both directions are needed and for the same reason: the pot is one pot.
    /// Only lending would let a win on one cabinet leave the others still
    /// showing the old figure until they were reloaded.
    pub fn take_from(&mut self, jackpots: &Jackpots, state: &JackpotState) {
        for (index, tier) in jackpots.tiers.iter().enumerate() {
            if !tier.shared {
                continue;
            }
            self.accrued_milli
                .insert(tier.id.clone(), state.accrued_milli(index));
        }
    }
}

// Tests live in the crate-level integration harness.
