//! Proving the spin was decided before you saw it (§5.74).
//!
//! # The question a slot machine never answers
//!
//! Every other honesty claim in this game is checkable. §5.18 shows what the
//! cabinet has actually paid against what it says it pays. §5.29 generates the
//! rules from the same numbers the engine uses, so the panel cannot describe a
//! machine other than the one running. The RTP harness spins a million times and
//! the conservation soak proves no credit is invented or lost.
//!
//! All of that measures the *machine*. None of it answers the question a player
//! actually has about a *spin*: **was that outcome fixed when I pressed the
//! button, or decided once the reels had started turning?**
//!
//! It is the oldest suspicion about slot machines and it is normally met with a
//! promise. This one answers it with arithmetic.
//!
//! # What makes it answerable
//!
//! [`engine::spin`](crate::engine::spin) is a pure function of exactly four
//! things: the cabinet's data, the generator's state, the line bet, and the
//! mode. Nothing else reaches it — not the balance, not the hoard, not how long
//! you have been playing, not whether you have been winning.
//!
//! And [`SeededRng`] is an xorshift, so its state *is* its future. Record the
//! state before the draw and the outcome is already determined; it just has not
//! been looked at yet.
//!
//! So the game writes down those four things **before** it spins. That record is
//! a commitment: it fixes the answer while the reels are still turning. Handing
//! it back afterwards lets the same spin be run again — by the same engine code,
//! not a re-implementation — and the two are compared symbol for symbol.
//!
//! A verifier that re-implements the thing it verifies proves only that someone
//! wrote the same bug twice. [`verify`] calls `engine::spin`. The one thing it
//! is allowed to do differently is *when*.
//!
//! # What it cannot prove, said plainly
//!
//! This shows a spin was not altered after the fact. It does **not** show the
//! game picked its seed fairly in the first place — a real provable-fairness
//! scheme publishes a hashed server seed before play and reveals it after, and
//! this is a play-money cabinet with no server. The panel says so rather than
//! letting the tick imply more than it has earned.

use crate::data::{GameConfig, GameData};
use crate::engine::{SpinMode, SpinResult};
use crate::state::persist;
use macroquad_toolkit::persistence::{json_key_exists, load_json_key, save_json_key};
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};

const PROOF_KEY: &str = "proofs";

/// How many spins stay checkable. Enough to cover an evening's interesting
/// moments without the save growing without limit.
pub const KEPT: usize = 24;

/// What the game committed to, written down before the reels were drawn.
///
/// Everything `engine::spin` reads, and nothing it does not. If a field here
/// were missing the record would not reproduce the spin; if one were spurious
/// it would suggest the engine reads something it does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commitment {
    /// Which spin of the session this was. The player's handle on it.
    pub seq: u64,
    /// Which cabinet. The strips differ, so the same state on Emberfall and on
    /// Tidepool are different spins. This is the id, and the id is what
    /// [`verify`] loads by.
    pub machine: String,
    /// What that cabinet was called at the time.
    ///
    /// Denormalised deliberately. A log is a record of what happened, so it
    /// freezes the name the cabinet had then — and the alternative is a panel
    /// that re-parses six cabinets' JSON every frame to render a column of
    /// twelve labels. Verification never reads this; it goes by the id.
    pub machine_name: String,
    /// The generator's whole state at the moment the button was pressed.
    pub state: u64,
    pub line_bet: i64,
    /// Base game, or a free spin and what the run had burned and multiplied.
    pub mode: RecordedMode,
    /// What the spin paid, so the row can be read without re-running it.
    pub win: i64,
    /// The resting grid, flattened to symbol ids. Held so a mismatch can say
    /// *what* differs rather than only that something does.
    pub grid: Vec<String>,
}

/// [`SpinMode`] as written to disk.
///
/// A separate type on purpose. `SpinMode` is the engine's business and may grow
/// a variant tomorrow; this is a save format and every change to it has to be
/// deliberate. They convert at the boundary, which is exactly where a new
/// variant should force someone to decide what old saves mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecordedMode {
    /// `ante` defaults to false so records written before the side bet existed
    /// (§5.75) still load, and still verify — they were base spins, which is
    /// exactly what the default says.
    Base {
        #[serde(default)]
        ante: bool,
    },
    FreeSpin {
        burned: usize,
        multiplier: i64,
    },
}

impl From<SpinMode> for RecordedMode {
    fn from(mode: SpinMode) -> Self {
        match mode {
            SpinMode::Base { ante } => RecordedMode::Base { ante },
            SpinMode::FreeSpin { burned, multiplier } => {
                RecordedMode::FreeSpin { burned, multiplier }
            }
        }
    }
}

impl From<RecordedMode> for SpinMode {
    fn from(mode: RecordedMode) -> Self {
        match mode {
            RecordedMode::Base { ante } => SpinMode::Base { ante },
            RecordedMode::FreeSpin { burned, multiplier } => {
                SpinMode::FreeSpin { burned, multiplier }
            }
        }
    }
}

impl Commitment {
    /// Write down a spin at the moment it is drawn.
    ///
    /// Called with the generator state taken **before** the draw, which is the
    /// whole substance of the claim: at the instant this is built the outcome
    /// is already fixed, and nothing between here and the reels coming to rest
    /// can change it. `seq` is filled in by [`ProofLog::push`].
    pub fn record(
        data: &GameData,
        state: u64,
        line_bet: i64,
        mode: SpinMode,
        result: &SpinResult,
    ) -> Self {
        Self {
            seq: 0,
            machine: data.machine_id().to_owned(),
            machine_name: data.config.display_name.clone(),
            state,
            line_bet,
            mode: mode.into(),
            win: result.outcome.total_credits,
            grid: flatten(data, result),
        }
    }

    /// The state as the panel shows it. Hex because the number is a bit
    /// pattern, not a quantity, and because a player copying it out to check by
    /// hand should not have to count digits.
    pub fn state_hex(&self) -> String {
        format!("{:016X}", self.state)
    }
}

/// The result of running a commitment back through the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Same stops, same grid, same payout.
    Matches,
    /// The record is for a cabinet this build does not have. Not a failure —
    /// a save can outlive a machine being renamed — but not a pass either.
    UnknownMachine,
    /// The engine produced something else. The record is what was promised and
    /// the message says how the re-run differed from it.
    Differs(String),
}

impl Verdict {
    pub fn is_match(&self) -> bool {
        *self == Verdict::Matches
    }

    pub fn message(&self) -> &str {
        match self {
            Verdict::Matches => "same spin",
            Verdict::UnknownMachine => "cabinet not in this build",
            Verdict::Differs(how) => how,
        }
    }
}

/// Re-run a committed spin and compare it with what was promised.
///
/// Loads the cabinet by name and calls the engine. Nothing here knows how a
/// reel stops or how a cluster pays, which is the point: a verifier that
/// re-implements the thing it checks proves only that the same mistake was made
/// twice.
pub fn verify(commitment: &Commitment) -> Verdict {
    let Some(machine) = crate::data::MACHINES
        .iter()
        .find(|machine| machine.id == commitment.machine)
    else {
        return Verdict::UnknownMachine;
    };
    let Ok(data) = GameData::load_machine(machine) else {
        return Verdict::UnknownMachine;
    };

    let mut rng = SeededRng::from_state(commitment.state);
    let result = crate::engine::spin(&data, &mut rng, commitment.line_bet, commitment.mode.into());

    compare(commitment, &data, &result)
}

/// Where a re-run and its promise differ, in words a player can act on.
fn compare(commitment: &Commitment, data: &GameData, result: &SpinResult) -> Verdict {
    let won = result.outcome.total_credits;
    if won != commitment.win {
        return Verdict::Differs(format!(
            "paid {} on the re-run, not {}",
            won, commitment.win
        ));
    }

    let grid = flatten(data, result);
    if grid != commitment.grid {
        let differing = grid
            .iter()
            .zip(&commitment.grid)
            .filter(|(now, then)| now != then)
            .count();
        return Verdict::Differs(if grid.len() == commitment.grid.len() {
            format!("{} of {} symbols differ", differing, grid.len())
        } else {
            format!(
                "the re-run landed {} symbols, not {}",
                grid.len(),
                commitment.grid.len()
            )
        });
    }

    Verdict::Matches
}

/// The resting grid as symbol ids, column by column.
///
/// The *resting* grid rather than the landing one: on a cascading cabinet the
/// board the player is looking at when the spin finishes is the last step of the
/// chain, and a proof about a board nobody saw would be a technicality.
/// Symbol **ids**, not the indices the grid holds. An index is a position in
/// whatever symbol list this build happens to ship; adding a symbol would
/// silently rewrite every record on disk into a different board. A name is a
/// name.
pub fn flatten(data: &GameData, result: &SpinResult) -> Vec<String> {
    let grid = result.resting_grid();
    let mut cells = Vec::with_capacity(grid.cell_count());
    for reel in 0..grid.reel_count() {
        for row in 0..grid.rows_on(reel) {
            let symbol = grid.at(reel, row);
            cells.push(data.symbols.get(symbol).id.clone());
        }
    }
    cells
}

/// The spins still open to checking, newest first.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProofLog {
    /// Newest first, so the panel reads in the order a player thinks in and the
    /// oldest falls off the end.
    entries: Vec<Commitment>,
    /// Spins committed to since the log began. Keeps numbering stable as rows
    /// age out, so "spin 412" means one spin forever.
    committed: u64,
}

impl ProofLog {
    pub fn load(config: &GameConfig) -> Self {
        let mut log: Self = match load_json_key(&config.game_name, PROOF_KEY) {
            Ok(log) => log,
            Err(error) => {
                if json_key_exists(&config.game_name, PROOF_KEY) {
                    eprintln!("Dragon's Hoard proof log could not be loaded: {error}");
                }
                Self::default()
            }
        };
        log.entries.truncate(KEPT);
        // A log that has been edited to claim more spins than it holds would
        // otherwise number the next one wrongly. Cheap to repair, and the
        // repair is the honest reading: the count cannot be below what is here.
        log.committed = log.committed.max(log.entries.len() as u64);
        log
    }

    pub fn save(&self, config: &GameConfig) -> Result<(), String> {
        if !persist::may_write() {
            return Ok(());
        }
        save_json_key(&config.game_name, PROOF_KEY, self)
    }

    /// The entries, writable. Used by the capture harness to alter a record
    /// and photograph the panel refusing it (§5.74) — a verifier only ever
    /// seen agreeing is a picture of a tick, not a check.
    pub fn entries_mut(&mut self) -> &mut [Commitment] {
        &mut self.entries
    }

    pub fn entries(&self) -> &[Commitment] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Take a commitment written at the draw and give it a number.
    ///
    /// The record arrives already made — the session builds it where the
    /// generator state is still in hand — and this only files it. Numbering
    /// here rather than there is what keeps "spin 412" meaning one spin even
    /// after the log has rolled over several times.
    pub fn push(&mut self, mut commitment: Commitment) {
        self.committed += 1;
        commitment.seq = self.committed;
        self.entries.insert(0, commitment);
        self.entries.truncate(KEPT);
    }

    /// Check every row that is still here. Returns them paired with a verdict,
    /// in the order the panel shows them.
    pub fn verify_all(&self) -> Vec<(&Commitment, Verdict)> {
        self.entries
            .iter()
            .map(|entry| (entry, verify(entry)))
            .collect()
    }
}

#[cfg(test)]
mod tests;
