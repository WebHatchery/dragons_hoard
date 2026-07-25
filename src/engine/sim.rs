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

/// What buying one tier over and over returns per credit spent (§5.13).
#[derive(Debug, Clone, Default)]
pub struct BuyReport {
    pub buys: u64,
    pub spent: i64,
    pub won: i64,
}

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

#[cfg(test)]
mod buy_tests {
    use super::*;
    use crate::data::MACHINES;
    use crate::state::featurebuy;

    /// A bought feature must give back what the machine gives back — no more,
    /// no less. This is the assertion the whole Feature Buy design rests on
    /// (§5.13): price a tier below its expected value and never spinning beats
    /// spinning; price it above and the menu is a trap.
    ///
    /// Runs in CI at a sample small enough to be quick, which is why the band is
    /// wide. `feature_buy_prices_are_exact` is the tight one.
    #[test]
    fn every_bought_tier_returns_roughly_what_it_cost() {
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let target = data.featurebuy.target_rtp_permille as f64 / 1000.0;

            for (index, tier) in data.featurebuy.tiers.iter().enumerate() {
                let report = simulate_buys(&data, index, 3_000, 0x51E_5EED + index as u64);
                assert!(report.buys > 0, "{} sold nothing", tier.id);

                let rtp = report.rtp();
                assert!(
                    (rtp - target).abs() < 0.35,
                    "{}/{} returns {:.4} against a target of {:.4} — reprice it",
                    machine.id,
                    tier.id,
                    rtp,
                    target
                );
            }
        }
    }

    /// The tight version. High-variance features need a large sample before the
    /// mean settles, so this is `#[ignore]`d and run when tuning:
    /// `cargo test --release -- --ignored feature_buy_prices_are_exact`.
    #[test]
    #[ignore]
    fn feature_buy_prices_are_exact() {
        // Every tier is measured and printed *before* anything is asserted. A
        // run that stopped at the first bad price would make repricing a menu
        // one slow round trip per tier.
        let mut wrong: Vec<String> = Vec::new();

        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let target = data.featurebuy.target_rtp_permille as f64 / 1000.0;

            for (index, tier) in data.featurebuy.tiers.iter().enumerate() {
                let report = simulate_buys(&data, index, 200_000, 0xB0_0B5 + index as u64);
                let rtp = report.rtp();
                // What the price *should* be, given what the feature actually
                // paid: the current price scaled by how far off target it came
                // in. Printed so a failing run hands the designer the answer
                // rather than only the problem.
                let fair = tier.price_multiple as f64 * rtp / target;

                println!(
                    "{:>7}/{:<10} price {:>4}x  rtp {:.4}  (target {:.4})  fair price {:.1}x",
                    machine.id, tier.id, tier.price_multiple, rtp, target, fair
                );
                if (rtp - target).abs() >= 0.02 {
                    wrong.push(format!(
                        "{}/{}: returns {:.4} against {:.4} — price it at {:.0}x, not {}x",
                        machine.id,
                        tier.id,
                        rtp,
                        target,
                        fair.round(),
                        tier.price_multiple
                    ));
                }
            }
        }

        assert!(
            wrong.is_empty(),
            "mispriced tiers:
  {}",
            wrong.join(
                "
  "
            )
        );
    }

    /// Buying must not be a cheaper route to a progressive. The price is a
    /// stake, so it feeds the pots; it is not a spin, so it does not roll.
    #[test]
    fn a_buy_feeds_the_pots_without_drawing_from_them() {
        let data = GameData::load().unwrap();
        let mut session = GameSession::new(&data, 77);
        session.balance = 10_000_000;

        let pots = |session: &GameSession| -> Vec<i64> {
            (0..data.jackpots.tiers.len())
                .map(|tier| session.jackpots.value(&data.jackpots, tier))
                .collect()
        };

        let before = pots(&session);
        let purchase = session.buy_feature(0, &data).unwrap();
        let after = pots(&session);

        assert!(purchase.price > 0);
        assert!(
            after.iter().zip(&before).all(|(now, then)| now > then),
            "a bought feature should feed every tier"
        );
        assert_eq!(
            session.stats.jackpots, 0,
            "the purchase itself must not roll for a pot"
        );
    }

    /// The menu advertises a price; the balance must move by exactly that.
    #[test]
    fn the_price_charged_is_the_price_shown() {
        let data = GameData::load().unwrap();
        for (index, tier) in data.featurebuy.tiers.iter().enumerate() {
            let mut session = GameSession::new(&data, 9_000 + index as u64);
            session.balance = 10_000_000;

            let quoted = featurebuy::price(tier, session.total_bet(&data));
            let before = session.balance;
            let purchase = session.buy_feature(index, &data).unwrap();

            assert_eq!(purchase.price, quoted);
            assert_eq!(session.balance, before - quoted);
        }
    }
}
