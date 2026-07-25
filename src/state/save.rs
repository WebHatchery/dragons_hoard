//! Save payload, lifetime stats, and migration from older save shapes.

use super::jackpot::JackpotState;
use super::{GameSession, HoardState};
use crate::data::GameData;
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStats {
    pub total_spins: u64,
    pub total_wagered: i64,
    pub total_won: i64,
    pub biggest_win: i64,
    pub hatches: u32,
    pub free_spins_played: u64,
    #[serde(default)]
    pub jackpots: u32,
    /// Dragon's Wrath rounds played. `default` so a save written before the
    /// feature existed still loads (§5.9's rule: adding content must never
    /// strand a player).
    #[serde(default)]
    pub wrath_rounds: u32,
    /// Features bought outright (§5.13). `default` so a save written before the
    /// menu existed still loads.
    #[serde(default)]
    pub features_bought: u32,
    /// Gambles that ended above what they staked, and gambles that busted
    /// (§5.16). `default` so a save from before the feature still loads.
    #[serde(default)]
    pub gambles_won: u32,
    #[serde(default)]
    pub gambles_lost: u32,
}

/// What lands on disk. Deliberately excludes the grid and any in-flight spin or
/// feature: a reload always resumes in the base game with a resting display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveData {
    pub version: String,
    pub balance: i64,
    pub line_bet_index: usize,
    pub hoard: HoardState,
    /// Progressive pots. `default` so a save written before jackpots existed
    /// still loads — it simply starts every tier at its seed.
    #[serde(default)]
    pub jackpots: JackpotState,
    pub stats: SessionStats,
    pub rng: SeededRng,
}

#[derive(Debug, Deserialize)]
struct LegacySave {
    balance: Option<i64>,
    points: Option<i64>,
    line_bet_index: Option<usize>,
    hoard_count: Option<u32>,
    hoard_pot: Option<i64>,
}

pub fn migrate_save_value(
    detected_version: Option<String>,
    value: Value,
    data: &GameData,
) -> Result<SaveData, String> {
    let payload = value.get("data").cloned().unwrap_or(value);

    if let Ok(mut current) = serde_json::from_value::<SaveData>(payload.clone()) {
        current.version = data.config.version.clone();
        current.line_bet_index = current.line_bet_index.min(data.config.line_bets.len() - 1);
        return Ok(current);
    }

    let legacy: LegacySave = serde_json::from_value(payload)
        .map_err(|err| format!("Unsupported save format {:?}: {}", detected_version, err))?;

    let mut session = GameSession::new(data, 0x5EED_1234);
    if let Some(balance) = legacy.balance.or(legacy.points) {
        session.balance = balance.max(0);
    }
    if let Some(index) = legacy.line_bet_index {
        session.line_bet_index = index.min(data.config.line_bets.len() - 1);
    }
    session.hoard.count = legacy.hoard_count.unwrap_or(0);
    session.hoard.pot = legacy.hoard_pot.unwrap_or(0);

    Ok(session.to_save(&data.config.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> GameData {
        GameData::load().unwrap()
    }

    #[test]
    fn a_save_round_trips_the_session() {
        let data = data();
        let mut session = GameSession::new(&data, 77);
        for _ in 0..25 {
            if session.spin(&data).is_err() {
                break;
            }
        }

        let save = session.to_save(&data.config.version);
        let json = serde_json::to_value(&save).unwrap();
        let restored = migrate_save_value(Some("1.0.0".to_owned()), json, &data).unwrap();
        let reloaded = GameSession::from_save(&data, restored);

        assert_eq!(reloaded.balance, session.balance);
        assert_eq!(reloaded.line_bet_index, session.line_bet_index);
        assert_eq!(reloaded.hoard.count, session.hoard.count);
        assert_eq!(reloaded.hoard.pot, session.hoard.pot);
        assert_eq!(reloaded.stats.total_spins, session.stats.total_spins);
    }

    #[test]
    fn a_reloaded_session_keeps_producing_the_same_spins() {
        let data = data();
        let mut session = GameSession::new(&data, 4242);
        session.spin(&data).unwrap();

        let save = session.to_save(&data.config.version);
        let mut reloaded = GameSession::from_save(&data, save);

        let a = session.spin(&data).unwrap();
        let b = reloaded.spin(&data).unwrap();
        assert_eq!(a.result.grid, b.result.grid);
    }

    #[test]
    fn a_legacy_save_migrates_to_the_current_shape() {
        let data = data();
        let value = serde_json::json!({
            "points": 640,
            "line_bet_index": 99,
            "hoard_count": 6,
            "hoard_pot": 60
        });

        let migrated = migrate_save_value(Some("0.1.0".to_owned()), value, &data).unwrap();

        assert_eq!(migrated.version, "1.0.0");
        assert_eq!(migrated.balance, 640);
        assert_eq!(migrated.line_bet_index, data.config.line_bets.len() - 1);
        assert_eq!(migrated.hoard.count, 6);
    }

    #[test]
    fn a_save_never_carries_an_in_flight_feature() {
        let data = data();
        let mut session = GameSession::new(&data, 12);
        session.free_spins = Some(super::super::FreeSpinState {
            remaining: 7,
            awarded: 10,
            line_bet: 5,
            total_won: 300,
            burned: 0,
        });

        let reloaded = GameSession::from_save(&data, session.to_save(&data.config.version));

        assert!(!reloaded.in_free_spins());
        assert!(reloaded.phase.is_idle());
    }
}
