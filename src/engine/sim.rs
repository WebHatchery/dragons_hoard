//! Monte-Carlo RTP harness.
//!
//! RTP is a property of the data JSON — reel strips, paytable, paylines and the
//! feature config — never of the Rust logic. Tuning the game means editing
//! `assets/data/*.json` and re-running this. The sim drives a real
//! [`GameSession`] so free spins, retriggers, expanding wilds and the hoard
//! meter all contribute exactly as they do in play; a base-game-only sim would
//! badly understate the return.

// Only the batch drivers need these; the report shape and its accumulation are
// pure arithmetic and compile into the game for the live profiler (§5.17).
#[cfg(test)]
use crate::data::GameData;
#[cfg(test)]
use crate::state::gamble::Scale;
#[cfg(test)]
use crate::state::GameSession;
use serde::{Deserialize, Serialize};

/// Balance the sim tops up to before each paid spin, so a losing streak can
/// never stall it.
#[cfg(test)]
const SIM_BANKROLL: i64 = 1_000_000_000;

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub struct SimConfig {
    pub spins: u64,
    /// Run with the ante side bet on (§5.75).
    pub ante: bool,
    pub line_bet_index: usize,
    pub seed: u64,
}

#[cfg(test)]
impl Default for SimConfig {
    fn default() -> Self {
        Self {
            spins: 100_000,
            ante: false,
            line_bet_index: 0,
            seed: 0xD2A6_0F1E,
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct SimReport {
    pub paid_spins: u64,
    pub free_spins: u64,
    pub total_wagered: i64,
    pub total_won: i64,
    pub base_won: i64,
    pub free_spin_won: i64,
    pub hatch_won: i64,
    pub jackpot_won: i64,
    pub wrath_won: i64,
    pub scatter_won: i64,
    /// Paid spins that returned anything at all.
    pub hits: u64,
    pub features_triggered: u64,
    pub hatches: u64,
    pub wrath_rounds: u64,
    pub biggest_win: i64,

    /// Return distribution across completed rounds.
    pub stats: RoundStats,
}

/// The statistics of a run: how much came back per round, and how unevenly.
///
/// Split out of [`SimReport`] because the live profiler (§5.17) needs exactly
/// this and none of the batch driver's per-feature breakdown. It is pure
/// arithmetic, so it compiles into the game while the drivers stay test-only.
impl RoundStats {
    /// Record one finished round: everything a single paid spin returned,
    /// including the free spins and features it triggered.
    pub fn record(&mut self, credits: i64, total_bet: i64) {
        if total_bet <= 0 {
            return;
        }
        let ratio = credits as f64 / total_bet as f64;
        self.rounds += 1;
        self.return_sum += ratio;
        self.return_square_sum += ratio * ratio;

        // `position` finds the first bound the ratio does *not* exceed; falling
        // off the end is the top band.
        let band = BANDS
            .iter()
            .position(|bound| ratio <= *bound)
            .unwrap_or(BAND_COUNT - 1);
        self.bands[band] += 1;
    }

    /// Mean return per round, in units of total bet. Equals RTP when every
    /// round was played at the same stake.
    pub fn mean_return(&self) -> f64 {
        if self.rounds == 0 {
            return 0.0;
        }
        self.return_sum / self.rounds as f64
    }

    /// Volatility index: the standard deviation of return per round.
    ///
    /// This is the number that separates the four cabinets in a way RTP cannot.
    /// Two machines can both return 95% while one pays a little constantly and
    /// the other pays nothing for an hour and then everything at once — and it
    /// is the second that empties a balance while the player is waiting.
    pub fn volatility(&self) -> f64 {
        if self.rounds < 2 {
            return 0.0;
        }
        let mean = self.mean_return();
        let variance = (self.return_square_sum / self.rounds as f64) - mean * mean;
        variance.max(0.0).sqrt()
    }

    /// Share of rounds that fell in each band, summing to 1.
    pub fn band_shares(&self) -> [f64; BAND_COUNT] {
        let mut shares = [0.0; BAND_COUNT];
        if self.rounds == 0 {
            return shares;
        }
        for (index, count) in self.bands.iter().enumerate() {
            shares[index] = *count as f64 / self.rounds as f64;
        }
        shares
    }
}

/// Serializable because the Ledger (§5.18) persists a player's own copy of it
/// across sessions — the same shape measuring the machine and measuring them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RoundStats {
    /// Completed rounds: one paid spin plus every free spin and feature it led
    /// to. The unit variance is measured in, because a free spin is part of the
    /// return on the paid spin that bought it, not a spin of its own.
    pub rounds: u64,
    /// Running sums of return-per-round in units of total bet, for the standard
    /// deviation. Kept as sums rather than a list so a million-round profile
    /// costs two floats.
    return_sum: f64,
    return_square_sum: f64,
    /// How many rounds fell in each band of [`BANDS`].
    pub bands: [u64; BAND_COUNT],
}

/// Upper bound of each win band, in multiples of total bet. The last band is
/// everything above the previous one.
///
/// Chosen to say something a player can feel rather than to be evenly spaced:
/// "nothing", "less than the stake back", "a small win", and then the three
/// sizes that are worth telling someone about.
pub const BANDS: [f64; BAND_COUNT - 1] = [0.0, 1.0, 2.0, 5.0, 20.0, 100.0];
pub const BAND_COUNT: usize = 7;

/// Human labels for the bands, in the same order.
pub const BAND_LABELS: [&str; BAND_COUNT] = [
    "nothing", "under 1x", "1-2x", "2-5x", "5-20x", "20-100x", "100x+",
];

#[cfg(test)]
impl SimReport {
    pub fn rtp(&self) -> f64 {
        if self.total_wagered == 0 {
            return 0.0;
        }
        self.total_won as f64 / self.total_wagered as f64
    }

    /// Return with the progressive layer taken out.
    ///
    /// Jackpots are bet-fair by construction but wildly high-variance: a
    /// low-stake run samples each tier a fraction as often as a high-stake one,
    /// so any short comparison across the bet ladder is dominated by whether the
    /// rare tiers happened to land. Excluding them leaves the part that *should*
    /// match spin for spin.
    pub fn rtp_excluding_jackpots(&self) -> f64 {
        if self.total_wagered == 0 {
            return 0.0;
        }
        (self.total_won - self.jackpot_won) as f64 / self.total_wagered as f64
    }

    pub fn hit_frequency(&self) -> f64 {
        if self.paid_spins == 0 {
            return 0.0;
        }
        self.hits as f64 / self.paid_spins as f64
    }

    /// Share of the return coming from each source, as a fraction of turnover.
    pub fn contribution(&self, credits: i64) -> f64 {
        if self.total_wagered == 0 {
            return 0.0;
        }
        credits as f64 / self.total_wagered as f64
    }

    pub fn summary(&self) -> String {
        format!(
            "spins {} (+{} free) | RTP {:.4} | hit {:.3} | base {:.4} free {:.4} hatch {:.4} jackpot {:.4} wrath {:.4} scatter {:.4} | features {} hatches {} wrath {} | max win {}",
            self.paid_spins,
            self.free_spins,
            self.rtp(),
            self.hit_frequency(),
            self.contribution(self.base_won),
            self.contribution(self.free_spin_won),
            self.contribution(self.hatch_won),
            self.contribution(self.jackpot_won),
            self.contribution(self.wrath_won),
            self.contribution(self.scatter_won),
            self.features_triggered,
            self.hatches,
            self.wrath_rounds,
            self.biggest_win,
        )
    }
}

#[cfg(test)]
pub fn run(data: &GameData, config: SimConfig) -> SimReport {
    let mut session = GameSession::new(data, config.seed);
    session.line_bet_index = config.line_bet_index.min(data.config.line_bets.len() - 1);
    session.preferences.ante = config.ante;

    let mut report = SimReport::default();

    for _ in 0..config.spins {
        session.balance = SIM_BANKROLL;

        let Ok(resolution) = session.spin(data) else {
            break;
        };

        // What the spin actually cost, ante included — measuring the return
        // against the base stake while charging the ante one would report a
        // number nobody is playing (§5.75).
        let total_bet = session.staked(data);
        let mut round_credits = resolution.total_credits();

        report.paid_spins += 1;
        accumulate(&mut report, &resolution, false);

        if resolution.outcome().free_spins_awarded > 0 {
            report.features_triggered += 1;
        }

        while session.in_free_spins() {
            let Ok(free) = session.spin(data) else {
                break;
            };
            report.free_spins += 1;
            round_credits += free.total_credits();
            accumulate(&mut report, &free, true);
        }

        report.stats.record(round_credits, total_bet);
    }

    report.total_wagered = session.stats.total_wagered;
    report
}

/// What buying one tier over and over returns per credit spent (§5.13).
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct BuyReport {
    pub buys: u64,
    pub spent: i64,
    pub won: i64,
}

#[cfg(test)]
impl BuyReport {
    pub fn rtp(&self) -> f64 {
        if self.spent == 0 {
            return 0.0;
        }
        self.won as f64 / self.spent as f64
    }
}

/// Buy one tier `rounds` times and measure what it gives back.
///
/// This is the check that keeps the Feature Buy honest. A tier's price is a
/// number in JSON, so nothing stops it drifting away from the value of the
/// feature it buys — except measuring the feature and comparing. Because the
/// buy hands control back to the real session, everything downstream is
/// included: retriggers, expanding wilds, eggs banked into the hoard, a Vault
/// Pick the free spins happened to fill, a Wrath a bought free spin woke.
///
/// The balance is topped up between rounds so a bad run cannot end the sample
/// early; the *stake* is still counted honestly, which is all the ratio needs.
#[cfg(test)]
pub fn simulate_buys(data: &GameData, tier: usize, rounds: u64, seed: u64) -> BuyReport {
    let mut session = GameSession::new(data, seed);
    let mut report = BuyReport::default();

    for _ in 0..rounds {
        session.balance = 1_000_000_000;
        session.celebrations.clear();

        let before = session.balance;
        let Ok(purchase) = session.buy_feature(tier, data) else {
            break;
        };
        report.buys += 1;
        report.spent += purchase.price;

        // Play whatever was bought all the way out, including anything it
        // triggered in turn.
        while session.in_free_spins() {
            if session.spin(data).is_err() {
                break;
            }
        }
        session.auto_play_bonus(data);
        session.auto_play_holdspin(data);

        report.won += session.balance - (before - purchase.price);
    }

    report
}

/// Spin, and push every win through the gamble ladder until it busts or the
/// ladder is spent (§5.16).
///
/// The point of this is the comparison in `gambling_cannot_move_rtp`: an
/// even-money double has expected value equal to its stake, so a player who
/// gambles everything must — over a long enough run — end up in the same place
/// as one who gambles nothing. Anything else means the coin is not fair or the
/// stake accounting is wrong, and neither would show up in a total-RTP band on
/// its own.
#[cfg(test)]
pub fn simulate_gambling_everything(data: &GameData, spins: u64, seed: u64) -> SimReport {
    let mut session = GameSession::new(data, seed);
    let mut report = SimReport::default();

    for _ in 0..spins {
        session.balance = 1_000_000_000;
        session.celebrations.clear();

        let Ok(resolution) = session.spin(data) else {
            break;
        };
        report.paid_spins += 1;
        accumulate(&mut report, &resolution, false);

        while session.in_free_spins() {
            let Ok(free) = session.spin(data) else {
                break;
            };
            report.free_spins += 1;
            accumulate(&mut report, &free, true);
        }

        // Gamble whatever the spin left standing, as far as it will go.
        let before = session.balance;
        if session.begin_gamble(data).is_ok() {
            while session
                .gamble
                .as_ref()
                .is_some_and(|round| round.can_flip())
            {
                let _ = session.flip_gamble(Scale::Ember, false, data);
            }
            session.take_gamble();
            // The delta is what the gamble did to the win: positive if it
            // climbed, negative down to the whole win if it busted.
            report.total_won += session.balance - before;
        }
    }

    report.total_wagered = session.stats.total_wagered;
    report
}

#[cfg(test)]
fn accumulate(report: &mut SimReport, resolution: &crate::state::SpinResolution, free: bool) {
    let credits = resolution.total_credits();
    report.total_won += credits;
    report.biggest_win = report.biggest_win.max(credits);
    report.hatch_won += resolution.hatch_credits;
    report.jackpot_won += resolution.jackpot_credits();
    report.wrath_won += resolution.wrath_credits;
    report.scatter_won += resolution.outcome().scatter_credits;

    if free {
        report.free_spin_won += resolution.spin_credits;
    } else {
        report.base_won += resolution.spin_credits;
        if credits > 0 {
            report.hits += 1;
        }
    }

    if resolution.wrath_credits > 0 {
        report.wrath_rounds += 1;
    }
    if resolution.hatch_credits > 0 {
        report.hatches += 1;
    }
}

#[cfg(test)]
mod tests;
