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
    accrued_milli: HashMap<String, i64>,
}

impl Floor {
    pub fn load(config: &GameConfig) -> Self {
        if slot_exists(&config.game_name, SLOT) {
            if let Ok(floor) = load_from_slot::<Self>(&config.game_name, SLOT) {
                return floor;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{GameData, MACHINES};

    fn data() -> GameData {
        GameData::load().unwrap()
    }

    /// The point of the whole thing: a pot filled on one cabinet is visible on
    /// another.
    #[test]
    fn a_pot_filled_on_one_cabinet_shows_up_on_the_next() {
        let dragon = data();
        let frost = GameData::load_machine(crate::data::machine_by_id("frost")).unwrap();

        let mut playing = JackpotState::new(&dragon.jackpots);
        for _ in 0..500 {
            playing.contribute(&dragon.jackpots, 20);
        }

        let mut floor = Floor::default();
        floor.take_from(&dragon.jackpots, &playing);

        let mut arriving = JackpotState::new(&frost.jackpots);
        let before = arriving.value(&frost.jackpots, grand_index(&frost.jackpots));
        floor.lend_to(&frost.jackpots, &mut arriving);
        let after = arriving.value(&frost.jackpots, grand_index(&frost.jackpots));

        assert!(
            after > before,
            "500 spins on Dragon's Hoard left the Grand on Frost Wyrm at {} \
             (was {}) — the floor pot is not shared",
            after,
            before
        );
    }

    /// And the tiers that are *not* shared stay with their cabinet, or every
    /// pot would be pooled and the ladder would mean nothing.
    #[test]
    fn a_cabinets_own_tiers_do_not_travel() {
        let dragon = data();
        let mut playing = JackpotState::new(&dragon.jackpots);
        for _ in 0..500 {
            playing.contribute(&dragon.jackpots, 20);
        }
        let mut floor = Floor::default();
        floor.take_from(&dragon.jackpots, &playing);

        let frost = GameData::load_machine(crate::data::machine_by_id("frost")).unwrap();
        let mut arriving = JackpotState::new(&frost.jackpots);
        floor.lend_to(&frost.jackpots, &mut arriving);

        for (index, tier) in frost.jackpots.tiers.iter().enumerate() {
            if tier.shared {
                continue;
            }
            assert_eq!(
                arriving.accrued_milli(index),
                0,
                "{} travelled between cabinets and should not have",
                tier.id
            );
        }
    }

    /// Addressed by id, so a cabinet with a different ladder cannot be handed
    /// another cabinet's money by position.
    #[test]
    fn the_floor_is_keyed_by_tier_id() {
        let dragon = data();
        let mut playing = JackpotState::new(&dragon.jackpots);
        playing.contribute(&dragon.jackpots, 20_000);
        let mut floor = Floor::default();
        floor.take_from(&dragon.jackpots, &playing);

        let shared: Vec<&str> = dragon
            .jackpots
            .tiers
            .iter()
            .filter(|t| t.shared)
            .map(|t| t.id.as_str())
            .collect();
        assert!(!shared.is_empty(), "no cabinet declares a shared tier");
        for id in shared {
            assert!(floor.accrued_milli.contains_key(id), "{} not banked", id);
        }
    }

    /// Every cabinet has to agree about which tier is the floor's, or a player
    /// walking between them would see the pot appear and vanish.
    #[test]
    fn every_cabinet_shares_the_same_tier() {
        let mut expected: Option<Vec<String>> = None;
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let shared: Vec<String> = data
                .jackpots
                .tiers
                .iter()
                .filter(|t| t.shared)
                .map(|t| t.id.clone())
                .collect();
            match &expected {
                None => expected = Some(shared),
                Some(first) => assert_eq!(
                    &shared, first,
                    "{} shares {:?} where the others share {:?}",
                    machine.id, shared, first
                ),
            }
        }
        assert_eq!(expected.as_deref(), Some(["grand".to_owned()].as_slice()));
    }

    fn grand_index(jackpots: &Jackpots) -> usize {
        jackpots
            .tiers
            .iter()
            .position(|tier| tier.shared)
            .expect("a shared tier")
    }
}
