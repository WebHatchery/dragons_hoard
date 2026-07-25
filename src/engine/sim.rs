//! Monte-Carlo RTP harness.
//!
//! RTP is a property of the data JSON — reel strips, paytable, paylines and the
//! feature config — never of the Rust logic. Tuning the game means editing
//! `assets/data/*.json` and re-running this. The sim drives a real
//! [`GameSession`] so free spins, retriggers, expanding wilds and the hoard
//! meter all contribute exactly as they do in play; a base-game-only sim would
//! badly understate the return.

use crate::data::GameData;
use crate::state::GameSession;

/// Balance the sim tops up to before each paid spin, so a losing streak can
/// never stall it.
const SIM_BANKROLL: i64 = 1_000_000_000;

#[derive(Debug, Clone, Copy)]
pub struct SimConfig {
    pub spins: u64,
    pub line_bet_index: usize,
    pub seed: u64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            spins: 100_000,
            line_bet_index: 0,
            seed: 0xD2A6_0F1E,
        }
    }
}

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
}

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

pub fn run(data: &GameData, config: SimConfig) -> SimReport {
    let mut session = GameSession::new(data, config.seed);
    session.line_bet_index = config.line_bet_index.min(data.config.line_bets.len() - 1);

    let mut report = SimReport::default();

    for _ in 0..config.spins {
        session.balance = SIM_BANKROLL;

        let Ok(resolution) = session.spin(data) else {
            break;
        };

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
            accumulate(&mut report, &free, true);
        }
    }

    report.total_wagered = session.stats.total_wagered;
    report
}

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
mod tests {
    use super::*;

    /// Target RTP for Dragon's Hoard. Tune `assets/data/*.json` to move it.
    const TARGET_RTP: f64 = 0.95;
    /// The smoke run is small enough for a debug-build CI pass, so it needs a
    /// wide band; the ignored full run below is the real gate.
    const SMOKE_TOLERANCE: f64 = 0.15;
    const FULL_TOLERANCE: f64 = 0.03;

    /// **Every machine must be in band, not just the one that boots.** A second
    /// cabinet is a whole second maths model; without this it could ship at any
    /// RTP at all and nothing would notice.
    #[test]
    fn every_machine_loads_and_lands_in_band() {
        for machine in crate::data::MACHINES {
            let data = GameData::load_machine(machine)
                .unwrap_or_else(|err| panic!("machine '{}' failed to load: {}", machine.id, err));
            let report = run(
                &data,
                SimConfig {
                    spins: 20_000,
                    ..SimConfig::default()
                },
            );

            println!("{:>8}: {}", machine.id, report.summary());
            assert!(
                (report.rtp() - TARGET_RTP).abs() < SMOKE_TOLERANCE,
                "machine '{}' RTP {:.4} outside {:.2} +/- {:.2}",
                machine.id,
                report.rtp(),
                TARGET_RTP,
                SMOKE_TOLERANCE
            );
        }
    }

    /// Machines should not all play the same. This asserts the catalog actually
    /// offers a choice rather than a reskin.
    #[test]
    fn the_machines_differ_in_volatility() {
        let mut hit_rates = Vec::new();
        for machine in crate::data::MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let report = run(
                &data,
                SimConfig {
                    spins: 20_000,
                    ..SimConfig::default()
                },
            );
            hit_rates.push((machine.id, report.hit_frequency()));
        }

        let lowest = hit_rates
            .iter()
            .map(|(_, rate)| *rate)
            .fold(f64::MAX, f64::min);
        let highest = hit_rates
            .iter()
            .map(|(_, rate)| *rate)
            .fold(0.0f64, f64::max);
        assert!(
            highest - lowest > 0.05,
            "every machine plays the same: {:?}",
            hit_rates
        );
    }

    #[test]
    fn rtp_smoke_lands_in_band() {
        let data = GameData::load().unwrap();
        let report = run(
            &data,
            SimConfig {
                spins: 20_000,
                ..SimConfig::default()
            },
        );

        println!("{}", report.summary());
        assert!(
            (report.rtp() - TARGET_RTP).abs() < SMOKE_TOLERANCE,
            "RTP {:.4} outside {:.2} +/- {:.2}: {}",
            report.rtp(),
            TARGET_RTP,
            SMOKE_TOLERANCE,
            report.summary()
        );
    }

    #[test]
    fn hit_frequency_is_sane() {
        let data = GameData::load().unwrap();
        let report = run(
            &data,
            SimConfig {
                spins: 20_000,
                ..SimConfig::default()
            },
        );

        assert!(
            report.hit_frequency() > 0.15 && report.hit_frequency() < 0.65,
            "hit frequency {:.3} outside the playable band: {}",
            report.hit_frequency(),
            report.summary()
        );
    }

    #[test]
    fn rtp_is_stable_across_the_bet_ladder() {
        let data = GameData::load().unwrap();
        let low = run(
            &data,
            SimConfig {
                spins: 20_000,
                line_bet_index: 0,
                ..SimConfig::default()
            },
        );
        let high = run(
            &data,
            SimConfig {
                spins: 20_000,
                line_bet_index: data.config.line_bets.len() - 1,
                ..SimConfig::default()
            },
        );

        // Same seed, same outcomes — only the stake scales. This is the test
        // that catches a hoard exploit where eggs banked cheap pay out dear.
        //
        // Jackpots are excluded deliberately: they are bet-fair by construction
        // (`state::jackpot::the_trigger_is_bet_fair`) but far too high-variance
        // to compare over 20,000 spins, so including them would make this a
        // flake rather than a guard. Their return is checked against its closed
        // form instead, in `the_jackpot_layer_matches_its_closed_form`.
        assert!(
            (low.rtp_excluding_jackpots() - high.rtp_excluding_jackpots()).abs() < 0.01,
            "RTP drifts with stake: low {:.4} vs high {:.4}",
            low.rtp_excluding_jackpots(),
            high.rtp_excluding_jackpots()
        );
    }

    /// The jackpot layer is the one part of the return with a closed form
    /// (`seed/odds + rate × share`, see `state::jackpot`). Measuring it and
    /// checking it against the formula catches a contribution or trigger bug
    /// that a total-RTP band alone would absorb.
    #[test]
    #[ignore = "the rare tiers need a long run to converge"]
    fn the_jackpot_layer_matches_its_closed_form() {
        let data = GameData::load().unwrap();
        let predicted = crate::state::jackpot::expected_rtp(&data.jackpots);
        let report = run(
            &data,
            SimConfig {
                spins: 4_000_000,
                ..SimConfig::default()
            },
        );
        let measured = report.contribution(report.jackpot_won);

        println!(
            "jackpot RTP predicted {:.4} measured {:.4}",
            predicted, measured
        );
        assert!(
            (measured - predicted).abs() < 0.012,
            "jackpot return {:.4} does not match the predicted {:.4}",
            measured,
            predicted
        );
    }

    #[test]
    fn the_sim_is_deterministic() {
        let data = GameData::load().unwrap();
        let config = SimConfig {
            spins: 2_000,
            ..SimConfig::default()
        };

        assert_eq!(
            run(&data, config).total_won,
            run(&data, config).total_won,
            "same seed produced different turnover"
        );
    }

    /// The long-run gate for the whole catalog. A machine whose features are
    /// rare needs far more spins than the smoke run to converge — Frost Wyrm
    /// triggers its feature roughly half as often as Dragon's Hoard.
    #[test]
    #[ignore = "million-spin run per machine; too slow for a debug CI build"]
    fn every_machine_holds_its_rtp_over_a_long_run() {
        for machine in crate::data::MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let report = run(
                &data,
                SimConfig {
                    spins: 1_000_000,
                    ..SimConfig::default()
                },
            );

            println!("{:>8}: {}", machine.id, report.summary());
            assert!(
                (report.rtp() - TARGET_RTP).abs() < FULL_TOLERANCE,
                "machine '{}' RTP {:.4} outside {:.2} +/- {:.2}",
                machine.id,
                report.rtp(),
                TARGET_RTP,
                FULL_TOLERANCE
            );
        }
    }

    /// The real RTP gate. `cargo test --release -- --ignored --nocapture`.
    #[test]
    #[ignore = "million-spin run; too slow for a debug CI build"]
    fn rtp_full_run_lands_in_band() {
        let data = GameData::load().unwrap();
        let report = run(
            &data,
            SimConfig {
                spins: 1_000_000,
                ..SimConfig::default()
            },
        );

        println!("{}", report.summary());
        assert!(
            (report.rtp() - TARGET_RTP).abs() < FULL_TOLERANCE,
            "RTP {:.4} outside {:.2} +/- {:.2}: {}",
            report.rtp(),
            TARGET_RTP,
            FULL_TOLERANCE,
            report.summary()
        );
    }
}
