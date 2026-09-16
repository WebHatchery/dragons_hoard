//! Playing the game the way a player does, without a player (§5.33).
//!
//! # Two games, one set of numbers
//!
//! Everything the GDD claims about return comes from [`GameSession::spin`] —
//! the headless path the sim and the profiler drive. It settles a spin, then
//! calls `auto_play_bonus` and `auto_play_holdspin` to resolve any feature
//! immediately, because a Monte-Carlo run cannot wait for a beat timer.
//!
//! Nobody plays that game. A player goes through `begin_spin` and then
//! `update_spin` a frame at a time, and their features resolve through
//! `pick_bonus` and `tick_holdspin` instead. **Those are different functions.**
//! Four of them, two per feature, and until this module nothing anywhere
//! asserted the two pairs pay the same.
//!
//! If they ever diverge, every RTP figure in twenty-seven sections describes a
//! game that is not the one being shipped, and no existing test would notice:
//! the sim would keep measuring itself, correctly, forever.
//!
//! # What it checks
//!
//! The driver runs whole rounds frame by frame at a fixed timestep — dismissing
//! cards, picking chests, letting respin rounds beat themselves out — and holds
//! the session to a set of conservation laws while it does:
//!
//! - **The books balance.** `opening + won - wagered == balance`, exactly, after
//!   every round. `total_won` and `total_wagered` are maintained by different
//!   code from `balance`, so this is a real cross-check rather than a tautology:
//!   any path that moves credits without accounting for them breaks it.
//! - **Nothing goes negative**, and no round leaves the machine stuck.
//! - **The hoard and the pots** stay inside their own rules.
//!
//! And then the headline: the return measured through this path is compared
//! against the sim's own, which is the only evidence that the published figures
//! belong to the game people actually play.

use crate::data::GameData;
use crate::state::{GameSession, SpinBlocked};

/// A fixed step, generous enough that a round finishes in a sane number of
/// frames and small enough that nothing is skipped over. Everything the session
/// animates is driven by timers, so a coarse step only makes them finish sooner.
const STEP: f32 = 1.0 / 30.0;

/// Frames one round is allowed before it is called stuck.
///
/// Deliberately finite. The alternative to a budget is a test that hangs, which
/// on CI is indistinguishable from a test that is slow.
const FRAME_BUDGET: usize = 20_000;

/// Why a run stopped early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// The books did not balance after a round.
    Unaccounted {
        round: u64,
        expected: i64,
        actual: i64,
    },
    NegativeBalance {
        round: u64,
        balance: i64,
    },
    /// A round never returned to settled.
    Stuck {
        round: u64,
    },
    HoardOverfull {
        round: u64,
        count: u32,
        capacity: u32,
    },
    /// A progressive pot fell without the round reporting a win.
    PotFellUnwon {
        round: u64,
    },
}

/// What a run measured.
#[derive(Debug, Clone, Default)]
pub struct Tally {
    pub rounds: u64,
    pub wagered: i64,
    pub won: i64,
    /// Rounds that paid anything.
    pub hits: u64,
    /// Rounds that opened a Vault Pick, a Wrath round or free spins.
    pub features: u64,
    /// Frames spent, so a change that doubles round length is visible.
    pub frames: u64,
}

impl Tally {
    pub fn rtp(&self) -> f64 {
        if self.wagered == 0 {
            return 0.0;
        }
        self.won as f64 / self.wagered as f64
    }

    /// Rounds that returned anything, as a share of rounds.
    ///
    /// A **round**, in the sense §5.18 means it: the paid spin and everything
    /// it led to. Compared against the sim's band table rather than its `hits`
    /// counter, which counts only the paid spin's own credits.
    ///
    /// The two turn out to be the same number on every cabinet, and the reason
    /// is worth knowing: the scatter pays from anywhere, so a round that awards
    /// free spins has always already paid something. **There is no such thing
    /// here as a round that pays only inside its feature.** Holding the band
    /// figure anyway keeps the comparison correct if that ever stops being true.
    pub fn hit_frequency(&self) -> f64 {
        if self.rounds == 0 {
            return 0.0;
        }
        self.hits as f64 / self.rounds as f64
    }
}

/// Play `rounds` paid rounds through the interactive path.
///
/// The balance is topped up before every paid spin rather than being allowed to
/// run out. The question is what the machine pays, not how long a bankroll
/// lasts — and a run that went bust at round nine hundred would measure a
/// shorter game than it claimed to.
pub fn play(data: &GameData, seed: u64, rounds: u64) -> Result<Tally, Fault> {
    let mut session = GameSession::new(data, seed);
    let mut tally = Tally::default();

    while tally.rounds < rounds {
        // Top up, and account for it: the books are checked against what was
        // staked and won, so a credit arriving from outside the game has to be
        // outside the sum too.
        let float = 100_000_000;
        session.balance = float;

        let opening_won = session.stats.total_won;
        let opening_wagered = session.stats.total_wagered;
        let pot_before = pot_total(data, &session);

        match session.begin_spin(data) {
            Ok(()) => {}
            // Nothing to measure and nothing wrong: the caller asked for a bet
            // the cabinet cannot take.
            Err(SpinBlocked::InsufficientBalance) => break,
            Err(SpinBlocked::Busy) => {
                return Err(Fault::Stuck {
                    round: tally.rounds,
                })
            }
        }

        let frames = run_round(data, &mut session, tally.rounds)?;
        tally.frames += frames as u64;
        tally.rounds += 1;

        let wagered = session.stats.total_wagered - opening_wagered;
        let won = session.stats.total_won - opening_won;
        tally.wagered += wagered;
        tally.won += won;
        if won > 0 {
            tally.hits += 1;
        }
        // The books, against a balance maintained by entirely different code.
        let expected = float + won - wagered;
        if session.balance != expected {
            return Err(Fault::Unaccounted {
                round: tally.rounds,
                expected,
                actual: session.balance,
            });
        }
        if session.balance < 0 {
            return Err(Fault::NegativeBalance {
                round: tally.rounds,
                balance: session.balance,
            });
        }
        if session.hoard.count > data.config.hoard_capacity {
            return Err(Fault::HoardOverfull {
                round: tally.rounds,
                count: session.hoard.count,
                capacity: data.config.hoard_capacity,
            });
        }
        // A pot only ever falls when it is paid out, and a paid pot is a win.
        if pot_total(data, &session) < pot_before && won <= 0 {
            return Err(Fault::PotFellUnwon {
                round: tally.rounds,
            });
        }
    }

    tally.features = session.stats.hatches as u64
        + session.stats.wrath_rounds as u64
        + session.stats.seams as u64
        + session.stats.free_spins_played;
    Ok(tally)
}

/// Drive one round to completion: reels, cards, boards, respins and every free
/// spin the round bought.
fn run_round(data: &GameData, session: &mut GameSession, round: u64) -> Result<usize, Fault> {
    for frame in 0..FRAME_BUDGET {
        // A card holds the game exactly as it does for a player, and the player
        // is what dismisses it.
        if session.celebrations.is_active() {
            session.celebrations.clear();
        }
        // An open board waits on a pick. The first unopened chest — which is
        // what a player clicking blind does, and the only choice that is always
        // available. Picking a fixed index instead deadlocks the moment that
        // chest has already been turned over, which is how the first version of
        // this driver hung on every round.
        if let Some(index) = session
            .bonus
            .as_ref()
            .and_then(|round| round.next_unrevealed())
        {
            session.pick_bonus(index, data);
            continue;
        }

        // An open seam waits on a rite the same way the board waits on a chest
        // (§5.81). Rotated by round rather than fixed at zero, so a long soak
        // drives every rite the cabinet offers instead of proving one of them
        // conserves credit and assuming the rest do.
        let rites = session.seam_choice().len();
        if rites > 0 {
            session.choose_rite(round as usize % rites);
            continue;
        }

        session.update_spin(data, STEP);

        // A free spin costs nothing and belongs to the round that bought it, so
        // the round is not over until they are all played.
        if session.is_settled() && session.in_free_spins() {
            if session.begin_spin(data).is_err() {
                return Err(Fault::Stuck { round });
            }
            continue;
        }
        if session.is_settled() && !session.in_free_spins() {
            return Ok(frame + 1);
        }
    }
    Err(Fault::Stuck { round })
}

/// Every progressive added up. Only ever falls when one is paid, which is the
/// invariant; it climbs on every staked spin otherwise.
fn pot_total(data: &GameData, session: &GameSession) -> i64 {
    (0..data.jackpots.tiers.len())
        .map(|tier| session.jackpots.value(&data.jackpots, tier))
        .sum()
}

// Tests live in the crate-level integration harness.
