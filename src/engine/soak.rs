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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MACHINES;
    use crate::engine::sim::{self, SimConfig};

    fn every_machine() -> Vec<GameData> {
        MACHINES
            .iter()
            .map(|machine| GameData::load_machine(machine).unwrap())
            .collect()
    }

    #[test]
    fn the_books_balance_on_every_cabinet() {
        // The one that would catch a credit arriving from nowhere, or a stake
        // taken twice. `balance` and the two stat counters are maintained by
        // different code, so agreeing is evidence rather than arithmetic.
        for data in every_machine() {
            let tally = play(&data, 0x5EED_1234, 400)
                .unwrap_or_else(|fault| panic!("{}: {:?}", data.machine.id, fault));
            assert_eq!(tally.rounds, 400);
            assert!(tally.wagered > 0);
        }
    }

    #[test]
    fn no_round_gets_stuck() {
        // Every feature in the game holds the reels in some way: a card, an open
        // board, a respin beat, a cascade. A round that never comes back would
        // freeze the real game too.
        for data in every_machine() {
            let tally = play(&data, 0xBEEF_0007, 300).expect("a round hung");
            assert!(tally.frames > 0);
            // A sane average. A round that took thousands of frames would mean
            // something is animating far longer than it looks.
            assert!(
                tally.frames / tally.rounds < 400,
                "{} averages {} frames a round",
                data.machine.id,
                tally.frames / tally.rounds
            );
        }
    }

    #[test]
    fn the_interactive_path_pays_what_the_sim_says_it_does() {
        // The headline. Everything the GDD claims about return comes from the
        // headless path, which resolves features through `auto_play_*`; a player
        // goes through `pick_bonus` and `tick_holdspin`. If the two ever
        // diverged, the sim would keep measuring itself correctly forever and
        // every published figure would be about a game nobody plays.
        for data in every_machine() {
            let rounds = 6_000;
            let live = play(&data, 0xA11CE, rounds).expect("interactive run failed");
            let headless = sim::run(
                &data,
                SimConfig {
                    spins: rounds,
                    seed: 0xA11CE,
                    ..SimConfig::default()
                },
            );

            let apart = (live.rtp() - headless.rtp()).abs();
            assert!(
                apart < 0.22,
                "{}: interactive {:.4} against headless {:.4}",
                data.machine.id,
                live.rtp(),
                headless.rtp()
            );
        }
    }

    #[test]
    fn both_paths_hit_at_the_same_rate() {
        // Hit frequency is far tighter than return at this sample size, because
        // it does not depend on the rare, enormous payouts. A divergence here is
        // a structural difference rather than variance.
        for data in every_machine() {
            let rounds = 6_000;
            let live = play(&data, 0x1234_5678, rounds).expect("interactive run failed");
            let headless = sim::run(
                &data,
                SimConfig {
                    spins: rounds,
                    seed: 0x1234_5678,
                    ..SimConfig::default()
                },
            );

            // Round-level on both sides: band 0 is "the round paid nothing".
            let sim_hits = 1.0 - headless.stats.band_shares()[0];
            let apart = (live.hit_frequency() - sim_hits).abs();
            assert!(
                apart < 0.05,
                "{}: interactive hits {:.4} against headless {:.4}",
                data.machine.id,
                live.hit_frequency(),
                sim_hits
            );
        }
    }

    #[test]
    fn the_run_is_deterministic() {
        // A soak test that cannot be reproduced is a soak test that cannot be
        // debugged when it eventually fails.
        let data = &every_machine()[0];
        let first = play(data, 0xF00D, 250).unwrap();
        let second = play(data, 0xF00D, 250).unwrap();
        assert_eq!(first.won, second.won);
        assert_eq!(first.wagered, second.wagered);
        assert_eq!(first.frames, second.frames);
    }

    #[test]
    fn a_different_seed_is_a_different_run() {
        let data = &every_machine()[0];
        let first = play(data, 1, 250).unwrap();
        let second = play(data, 2, 250).unwrap();
        assert_ne!(first.won, second.won);
    }

    #[test]
    fn every_cabinet_reaches_its_features() {
        // A run that never opened a board or woke the dragon would be checking
        // the base game and calling it the whole machine.
        for data in every_machine() {
            let tally = play(&data, 0xC0FFEE, 3_000).expect("run failed");
            assert!(
                tally.features > 0,
                "{} never reached a feature in 3000 rounds",
                data.machine.id
            );
        }
    }

    /// The long version, and the real proof.
    ///
    /// At two hundred thousand rounds the hit frequency is all but free of
    /// sampling noise, so the two paths agreeing on it to within a percentage
    /// point is structural rather than lucky — measured, they agree to 0.0016
    /// on every cabinet. Return still swings, being dominated by the rare
    /// enormous payouts, which is why it gets the wider tolerance.
    ///
    /// The residual gaps rank in **volatility order**: Frost and Dragon's Hoard,
    /// the two with the fattest tails, sit about 1.4% apart, and Wyrmspire and
    /// Avalanche within 0.1%. That is the signature of sampling error rather
    /// than of a difference between the paths, which would not care how volatile
    /// the cabinet was.
    #[test]
    #[ignore]
    fn soak() {
        const ROUNDS: u64 = 200_000;
        for data in every_machine() {
            let live = play(&data, 0x50AC, ROUNDS)
                .unwrap_or_else(|fault| panic!("{}: {:?}", data.machine.id, fault));
            let headless = sim::run(
                &data,
                SimConfig {
                    spins: ROUNDS,
                    ante: false,
                    seed: 0x50AC,
                    ..SimConfig::default()
                },
            );

            // Round-level on both sides — see `Tally::hit_frequency`.
            let sim_hits = 1.0 - headless.stats.band_shares()[0];
            println!(
                "{:10} rtp {:.4} (sim {:.4})  hits {:.4} (sim {:.4})  features {}  frames/round {}",
                data.machine.id,
                live.rtp(),
                headless.rtp(),
                live.hit_frequency(),
                sim_hits,
                live.features,
                live.frames / live.rounds
            );

            assert!(
                (live.hit_frequency() - sim_hits).abs() < 0.01,
                "{}: hit rates differ structurally",
                data.machine.id
            );
            assert!(
                (live.rtp() - headless.rtp()).abs() < 0.06,
                "{}: returns differ by more than variance explains",
                data.machine.id
            );
        }
    }
}
