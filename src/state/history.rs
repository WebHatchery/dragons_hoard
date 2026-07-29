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
mod tests {
    use super::*;

    #[test]
    fn a_fresh_history_has_nothing_to_say() {
        let history = History::default();
        assert!(history.is_empty());
        assert_eq!(history.rounds(), 0);
        assert_eq!(history.extremes(), None);
        assert_eq!(history.opening(), None);
        assert_eq!(history.deepest_fall(), 0);
    }

    #[test]
    fn the_opening_balance_is_the_first_one_seen() {
        let mut history = History::default();
        history.record(1_000);
        history.record(50);
        history.record(9_000);
        assert_eq!(history.opening(), Some(1_000));
    }

    #[test]
    fn the_peak_of_a_long_session_is_exact() {
        // The reason the toolkit series decimates by extremes rather than by
        // averaging: one spike in thousands of rounds must still be the number
        // the panel reports.
        let mut history = History::default();
        for round in 0..20_000 {
            history.record(if round == 4_321 { 250_000 } else { 1_000 });
        }
        let (low, high) = history.extremes().unwrap();
        assert_eq!(high, 250_000);
        assert_eq!(low, 1_000);
    }

    #[test]
    fn the_deepest_fall_is_the_worst_peak_to_trough() {
        let mut history = History::default();
        for balance in [1_000, 5_000, 4_000, 800, 2_000, 1_900] {
            history.record(balance);
        }
        // 5,000 down to 800.
        assert_eq!(history.deepest_fall(), 4_200);
    }

    #[test]
    fn a_session_that_only_climbs_has_no_fall() {
        let mut history = History::default();
        for round in 0..500 {
            history.record(1_000 + round * 10);
        }
        assert_eq!(history.deepest_fall(), 0);
    }

    #[test]
    fn the_fall_survives_decimation_as_a_lower_bound() {
        // Once buckets merge, a peak and the trough after it can share one and
        // their order is lost. It must stay a plausible under-estimate rather
        // than collapsing to zero or inventing a larger one.
        let mut history = History::default();
        history.record(100_000);
        for _ in 0..20_000 {
            history.record(1_000);
        }
        let fall = history.deepest_fall();
        assert!(fall > 0, "the drop vanished entirely");
        assert!(fall <= 99_000, "reported more than ever happened");
    }

    #[test]
    fn memory_stays_bounded_across_a_long_session() {
        let mut history = History::default();
        for round in 0..200_000 {
            history.record(round % 5_000);
            if round % 100 == 0 {
                history.mark(Cause::Feature, round);
            }
        }
        assert!(history.series().len() <= CAPACITY + 2);
        assert!(history.marks().len() <= MAX_MARKS);
        assert_eq!(history.rounds(), 200_000);
    }

    #[test]
    fn a_mark_before_the_first_round_is_ignored() {
        // It would have no line to sit on, and would plot at whatever the graph
        // decided round zero meant.
        let mut history = History::default();
        history.mark(Cause::Jackpot, 5_000);
        assert!(history.marks().is_empty());
    }

    #[test]
    fn marks_remember_when_and_how_much() {
        let mut history = History::default();
        history.record(1_000);
        history.record(900);
        history.mark(Cause::Feature, 900);
        history.record(7_400);
        history.mark(Cause::BigWin, 7_400);

        let marks = history.marks();
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].cause, Cause::Feature);
        assert_eq!(marks[0].round, 2);
        assert_eq!(marks[1].balance, 7_400);
        assert!(marks[1].round > marks[0].round);
    }

    /// The bug the capture found: dropping the oldest left every surviving mark
    /// bunched against the right-hand edge, so the graph had nothing to say
    /// about the session it was describing.
    #[test]
    fn marks_stay_spread_across_the_whole_session() {
        let mut history = History::default();
        for round in 0..4_000u64 {
            history.record(1_000);
            if round % 12 == 0 {
                history.mark(Cause::Hatch, 1_000);
            }
        }
        let marks = history.marks();
        assert!(!marks.is_empty());
        assert!(marks.len() <= MAX_MARKS);

        // Something from the first quarter has to survive, or the graph is
        // describing the last few minutes and calling it the session.
        let total = history.rounds();
        assert!(
            marks.iter().any(|mark| mark.round < total / 4),
            "every mark is from the recent past"
        );
        assert!(marks.iter().any(|mark| mark.round > total * 3 / 4));
    }

    #[test]
    fn a_jackpot_is_never_thinned_away() {
        // The rarest thing the game does, and the one mark a player would look
        // for by name.
        let mut history = History::default();
        history.record(1_000);
        history.mark(Cause::Jackpot, 50_000);
        for _ in 0..MAX_MARKS * 8 {
            history.mark(Cause::Feature, 1_000);
        }
        assert!(history
            .marks()
            .iter()
            .any(|mark| mark.cause == Cause::Jackpot));
    }

    #[test]
    fn thinning_keeps_the_count_bounded() {
        // Including the case the jackpot exemption created: if every mark were
        // exempt, "never dropped" would mean "never bounded".
        for cause in Cause::ALL {
            let mut history = History::default();
            history.record(1_000);
            for _ in 0..20_000 {
                history.mark(cause, 1_000);
                assert!(history.marks().len() <= MAX_MARKS, "{:?}", cause);
            }
        }
    }

    #[test]
    fn marks_stay_in_order() {
        let mut history = History::default();
        history.record(1_000);
        for round in 0..200 {
            history.record(1_000 + round);
            history.mark(Cause::Hatch, 1_000 + round);
        }
        for pair in history.marks().windows(2) {
            assert!(pair[1].round >= pair[0].round);
        }
    }

    #[test]
    fn every_cause_is_named() {
        for cause in Cause::ALL {
            assert!(!cause.label().is_empty());
        }
    }

    #[test]
    fn clearing_starts_the_session_over() {
        let mut history = History::default();
        history.record(1_000);
        history.mark(Cause::Wrath, 1_000);
        history.clear();

        assert!(history.is_empty());
        assert!(history.marks().is_empty());
        assert_eq!(history.opening(), None);
        assert_eq!(history.rounds(), 0);
    }
}
