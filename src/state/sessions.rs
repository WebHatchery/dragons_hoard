//! The sessions you have played (§5.70).
//!
//! # A record the cabinet has never kept
//!
//! §5.68 gives an account of a session at the moment a cap ends it — staked,
//! returned, net, the best moment — and then throws it away. `new_session`
//! replaces the clock with a fresh one and the previous evening is gone.
//!
//! What survives is the Ledger (§5.18), and the Ledger answers a different
//! question. It is per-cabinet and lifetime: *how has Frost Wyrm treated me,
//! ever*. That is the machine's story. It cannot tell you that last night was
//! twenty minutes and you finished up, or that the four before it were an hour
//! each and every one of them was down.
//!
//! This is the player's story, and it is the one a real cabinet never offers —
//! not because it is hard, but because a machine has no reason to help you
//! notice a pattern in your own play.
//!
//! # What is kept, and what is deliberately not
//!
//! Six figures per session, all of which the clock already recorded, plus which
//! cap ended it if one did. No wall-clock date: the game measures time with the
//! frame loop so the capture harness stays reproducible (§5.30), and inventing a
//! calendar here would mean a second notion of when things happened that could
//! disagree with the first.
//!
//! The last twenty, oldest dropped. A log that grew forever would be a file that
//! grew forever, and twenty evenings is already more than anyone needs to see a
//! pattern.

use crate::data::GameConfig;
use crate::state::limits::{Breach, SessionClock};
use macroquad_toolkit::persistence::{load_from_slot, save_to_slot, slot_exists};
use serde::{Deserialize, Serialize};

const SLOT: &str = "sessions";
/// Kept sessions. Past this the oldest goes.
pub const KEPT: usize = 20;
/// Below this a session is not worth recording — an app opened and closed, or a
/// cabinet glanced at. Recording those would bury the real evenings.
const WORTH_KEEPING: u32 = 5;

/// One session, after the fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub seconds: u32,
    pub spins: u32,
    pub staked: i64,
    pub returned: i64,
    /// Best single win in it, from the session stats rather than the clock.
    #[serde(default)]
    pub best: i64,
    /// Credits the vault advanced (§5.53). Worth keeping apart: a session that
    /// finished level on borrowed money did not finish level.
    #[serde(default)]
    pub staked_by_vault: i64,
    /// Which cap ended it, if one did. `None` is a session the player left.
    #[serde(default)]
    pub ended_by: Option<EndedBy>,
}

/// Why a session stopped, in a form that outlives the `Breach` it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndedBy {
    Time,
    Loss,
    Spins,
}

impl From<Breach> for EndedBy {
    fn from(breach: Breach) -> Self {
        match breach {
            Breach::Time(_) => EndedBy::Time,
            Breach::Loss(_) => EndedBy::Loss,
            Breach::Spins(_) => EndedBy::Spins,
        }
    }
}

impl Session {
    pub fn net(&self) -> i64 {
        self.returned - self.staked
    }

    pub fn minutes(&self) -> u32 {
        self.seconds / 60
    }
}

/// Every session kept, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLog {
    #[serde(default)]
    sessions: Vec<Session>,
}

impl SessionLog {
    pub fn load(config: &GameConfig) -> Self {
        if slot_exists(&config.game_name, SLOT) {
            if let Ok(log) = load_from_slot::<Self>(&config.game_name, SLOT) {
                return log;
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

    /// Newest first, which is the order anyone reads a log in.
    pub fn recent(&self) -> impl Iterator<Item = &Session> {
        self.sessions.iter().rev()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Close a session and keep it, if there was anything to it.
    ///
    /// Returns whether it was kept. A session of two spins is not an evening,
    /// and a log full of them would hide the ones that were.
    pub fn record(
        &mut self,
        clock: &SessionClock,
        best: i64,
        staked_by_vault: i64,
        ended_by: Option<Breach>,
    ) -> bool {
        if clock.spins < WORTH_KEEPING {
            return false;
        }
        self.sessions.push(Session {
            seconds: clock.elapsed as u32,
            spins: clock.spins,
            staked: clock.staked,
            returned: clock.returned,
            best,
            staked_by_vault,
            ended_by: ended_by.map(EndedBy::from),
        });
        // Oldest first out. `drain` rather than `remove(0)` so trimming a log
        // that somehow grew past the cap costs one pass rather than many.
        if self.sessions.len() > KEPT {
            let excess = self.sessions.len() - KEPT;
            self.sessions.drain(0..excess);
        }
        true
    }

    /// What the kept sessions add up to.
    ///
    /// Stated as totals rather than an average, because an average session is
    /// not a thing anyone had.
    pub fn totals(&self) -> (u32, i64, i64) {
        self.sessions
            .iter()
            .fold((0, 0, 0), |(spins, staked, returned), s| {
                (spins + s.spins, staked + s.staked, returned + s.returned)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(spins: u32, staked: i64, returned: i64) -> SessionClock {
        let mut clock = SessionClock::default();
        for _ in 0..spins {
            clock.record(staked / spins.max(1) as i64, returned / spins.max(1) as i64);
        }
        clock.elapsed = spins as f32 * 12.0;
        clock
    }

    #[test]
    fn a_session_worth_keeping_is_kept_and_reads_back() {
        let mut log = SessionLog::default();
        assert!(log.record(&clock(120, 2_400, 1_800), 900, 0, Some(Breach::Loss(600))));

        let session = *log.recent().next().expect("one session");
        assert_eq!(session.spins, 120);
        assert_eq!(session.staked, 2_400);
        assert_eq!(session.returned, 1_800);
        assert_eq!(session.net(), -600);
        assert_eq!(session.best, 900);
        assert_eq!(session.ended_by, Some(EndedBy::Loss));
    }

    /// Opening the game and closing it is not an evening.
    #[test]
    fn a_session_of_almost_nothing_is_not_recorded() {
        let mut log = SessionLog::default();
        assert!(!log.record(&clock(2, 40, 0), 0, 0, None));
        assert!(log.is_empty());
    }

    /// The log has a ceiling, and it drops the oldest rather than the newest —
    /// the failure here would be a log that fills up and then stops recording.
    #[test]
    fn the_oldest_session_goes_when_the_log_is_full() {
        let mut log = SessionLog::default();
        for spins in 0..(KEPT as u32 + 5) {
            log.record(&clock(10 + spins, 100, 50), 0, 0, None);
        }
        assert_eq!(log.len(), KEPT);

        let newest = log.recent().next().unwrap().spins;
        let oldest = log.recent().last().unwrap().spins;
        assert_eq!(newest, 10 + KEPT as u32 + 4, "the newest was dropped");
        assert!(oldest > 10, "the oldest survived a full log");
    }

    /// Newest first: a log read oldest-first would put the session someone just
    /// finished at the bottom of the screen.
    #[test]
    fn the_log_reads_newest_first() {
        let mut log = SessionLog::default();
        log.record(&clock(10, 100, 50), 0, 0, None);
        log.record(&clock(99, 100, 50), 0, 0, None);
        assert_eq!(log.recent().next().unwrap().spins, 99);
    }

    /// §5.49's rule: a log written before a field existed still loads.
    #[test]
    fn a_log_written_before_the_vault_was_counted_still_loads() {
        let log: SessionLog = serde_json::from_value(serde_json::json!({
            "sessions": [{ "seconds": 600, "spins": 50, "staked": 1000, "returned": 900 }]
        }))
        .unwrap();
        let session = *log.recent().next().unwrap();
        assert_eq!(session.staked_by_vault, 0);
        assert_eq!(session.ended_by, None);
        assert_eq!(session.net(), -100);
    }

    #[test]
    fn the_totals_are_the_sessions_added_up() {
        let mut log = SessionLog::default();
        log.record(&clock(10, 200, 100), 0, 0, None);
        log.record(&clock(20, 400, 500), 0, 0, None);
        assert_eq!(log.totals(), (30, 600, 600));
    }
}
