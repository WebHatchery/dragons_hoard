//! The shape of the session, drawn (§5.32).
//!
//! # Three views of variance, and none of them was the obvious one
//!
//! The profiler (§5.17) simulates twenty thousand rounds and reports a
//! distribution. The ledger (§5.18) records what the player has actually seen
//! and sets it beside that distribution. The reality check (§5.30) states the
//! session totals. All three are **summaries**, and every one of them answers
//! the question "how much" while carefully avoiding "what did it feel like".
//!
//! Which is the question a player is really asking. A hit frequency of 0.41 and
//! a return of 95% describe a session perfectly and convey nothing about the
//! forty spins that returned nothing followed by one that paid two hundred
//! times. **That is the shape of the thing, and it is only visible over time.**
//!
//! So this records the bankroll after every round and draws it. The graph makes
//! an argument no table can: the long grinding decline is the normal state, the
//! spikes are where the money comes back, and the two are the same machine.
//!
//! # Marks
//!
//! A line alone would be a squiggle. Each feature, jackpot and big win is
//! recorded at the point it happened, so the spikes have causes attached —
//! which is what turns "I went up" into "I went up because the free spins came
//! in", and, more usefully, shows how much of the decline happened between them.
//!
//! # Why it does not persist
//!
//! The ledger is a lifetime record and survives everything. This deliberately
//! does not: it is a session, in the same sense §5.30 means it, and a graph
//! spanning six sittings would be a different and much less interesting picture.

use macroquad_toolkit::series::Series;

/// How many buckets the bankroll track keeps.
///
/// Wider than any plot will draw, so decimation is driven by memory rather than
/// by pixels and the graph stays honest if the panel ever grows.
const CAPACITY: usize = 512;

/// The most marks kept.
///
/// Thinned by **halving**, the same principle the bankroll series uses: when the
/// list fills, every second mark goes, which keeps coverage spread across the
/// whole session at declining density.
///
/// The obvious alternative — drop the oldest — was tried first and looks broken.
/// Three hundred hatches over four thousand rounds meant the surviving sixty-four
/// were all from the last few minutes, so the graph drew a wall of marks against
/// the right-hand edge and nothing at all across the session it was supposed to
/// be describing.
const MAX_MARKS: usize = 64;

/// Something worth pointing at on the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// Free spins began (§5.4, §5.21).
    Feature,
    /// The hoard hatched into a Vault Pick (§5.10).
    Hatch,
    /// The Dragon's Wrath woke (§5.12).
    Wrath,
    /// A seam ran through the board (§5.80).
    Seam,
    /// A progressive paid (§5.7).
    Jackpot,
    /// A round paid at or above the cabinet's big-win threshold.
    BigWin,
}

impl Cause {
    pub const ALL: [Cause; 6] = [
        Cause::Feature,
        Cause::Hatch,
        Cause::Wrath,
        Cause::Seam,
        Cause::Jackpot,
        Cause::BigWin,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Cause::Feature => "Free spins",
            Cause::Hatch => "Hatch",
            Cause::Wrath => "Wrath",
            Cause::Seam => "Seam",
            Cause::Jackpot => "Jackpot",
            Cause::BigWin => "Big win",
        }
    }
}

/// One marked moment: what happened, and how far into the session.
#[derive(Debug, Clone, Copy)]
pub struct Mark {
    pub cause: Cause,
    /// Rounds elapsed when it happened, matching the series' own clock.
    pub round: u64,
    pub balance: i64,
}

/// The session's bankroll, and what happened along it.
#[derive(Debug, Clone)]
pub struct History {
    balance: Series,
    marks: Vec<Mark>,
    /// Where the session started, so the graph has a baseline to be above or
    /// below rather than only a range.
    opening: Option<i64>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            balance: Series::new(CAPACITY),
            marks: Vec::new(),
            opening: None,
        }
    }
}

impl History {
    /// Record the bankroll at the end of a round.
    pub fn record(&mut self, balance: i64) {
        if self.opening.is_none() {
            self.opening = Some(balance);
        }
        self.balance.push(balance as f32);
    }

    /// Note something worth pointing at. Ignored before the first round, since
    /// a mark with no line under it has nowhere to sit.
    pub fn mark(&mut self, cause: Cause, balance: i64) {
        if self.balance.is_empty() {
            return;
        }
        if self.marks.len() >= MAX_MARKS {
            self.thin();
        }
        self.marks.push(Mark {
            cause,
            round: self.rounds(),
            balance,
        });
    }

    /// Halve the marks, keeping every second one.
    ///
    /// A jackpot is never dropped. It is the rarest thing the game does and the
    /// one mark a player would look for by name, and there are few enough of
    /// them that keeping every one cannot overrun the budget on its own.
    fn thin(&mut self) {
        let mut kept: Vec<Mark> = self
            .marks
            .iter()
            .enumerate()
            .filter(|(index, mark)| index % 2 == 0 || mark.cause == Cause::Jackpot)
            .map(|(_, mark)| *mark)
            .collect();

        // The exemption yields to the budget rather than the other way round: a
        // session of nothing but jackpots would otherwise keep every mark and
        // grow without bound, which is a promise this cannot afford to keep.
        while kept.len() >= MAX_MARKS {
            kept = kept.iter().step_by(2).copied().collect();
        }
        self.marks = kept;
    }

    pub fn clear(&mut self) {
        self.balance.clear();
        self.marks.clear();
        self.opening = None;
    }

    pub fn is_empty(&self) -> bool {
        self.balance.is_empty()
    }

    pub fn rounds(&self) -> u64 {
        self.balance.count()
    }

    pub fn series(&self) -> &Series {
        &self.balance
    }

    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    pub fn opening(&self) -> Option<i64> {
        self.opening
    }

    /// Highest and lowest the bankroll has been, including inside a bucket that
    /// has since been decimated — so the peak of a session is exact however
    /// long it ran.
    pub fn extremes(&self) -> Option<(i64, i64)> {
        Some((self.balance.min()? as i64, self.balance.max()? as i64))
    }

    /// The largest peak-to-trough fall, in credits.
    ///
    /// **Measured over the buckets, which means it is a lower bound** once the
    /// series has been decimated: a peak and the trough after it can end up in
    /// the same bucket, and their order within it is no longer known. Reported
    /// as "at least" for that reason rather than presented as exact.
    ///
    /// It is the number a player recognises. The ledger's bands say how often a
    /// round paid nothing; this says how far down that actually took them.
    pub fn deepest_fall(&self) -> i64 {
        let mut peak = f32::MIN;
        let mut worst = 0.0f32;
        for bucket in self.balance.buckets() {
            peak = peak.max(bucket.max);
            worst = worst.max(peak - bucket.min);
        }
        worst.max(0.0) as i64
    }
}

#[cfg(test)]
mod tests;
