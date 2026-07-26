//! Batch simulations: the long-run measurements that hold the maths in place.
//!
//! Kept out of `sim.rs` so the file that ships in the game stays the report
//! shape and the drivers, not several hundred lines of assertions.

mod batch {
    use super::super::*;

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

mod buy_tests {
    use super::super::*;
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

mod gamble_tests {
    use super::super::*;
    use crate::data::MACHINES;

    /// The claim §5.16 rests on: an even-money double moves variance and
    /// nothing else. A player who gambles every win to the ladder's end must
    /// measure the same RTP as one who never gambles.
    ///
    /// This is the only test in the suite that compares two *whole* simulations
    /// against each other rather than against a target, because "unchanged" is
    /// the assertion — there is no number to aim at.
    #[test]
    fn gambling_cannot_move_rtp() {
        let data = GameData::load().unwrap();
        let plain = run(
            &data,
            SimConfig {
                spins: 120_000,
                line_bet_index: 0,
                seed: 0x6A_6B1E,
                ante: false,
            },
        );
        let gambled = simulate_gambling_everything(&data, 120_000, 0x6A_6B1E);

        println!(
            "plain {:.4} | gambling everything {:.4}",
            plain.rtp(),
            gambled.rtp()
        );
        assert!(
            (plain.rtp() - gambled.rtp()).abs() < 0.06,
            "gambling moved RTP from {:.4} to {:.4}",
            plain.rtp(),
            gambled.rtp()
        );
    }

    /// A gamble must be offered on exactly the machines that can pay one, which
    /// is all of them — the config is shared and reads nothing from the strips.
    #[test]
    fn every_machine_offers_the_gamble() {
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            assert!(data.gamble.max_steps > 0, "{} has no ladder", machine.id);
            assert!(data.gamble.ceiling_multiple > 0);
        }
    }
}

#[cfg(test)]
mod ante {
    use super::super::{run, SimConfig};
    use crate::data::GameData;

    /// What each ante's price *should* be, given what it actually returns.
    ///
    /// The price is a fixed point: charge it, measure the return, and the ratio
    /// to the base return is the correction. Two or three rounds converge. Run
    /// with `--ignored --nocapture` and paste the last column into the JSON.
    #[test]
    #[ignore = "calibration, not a gate"]
    fn calibrate_the_ante() {
        const SPINS: u64 = 600_000;
        println!(
            "{:<11} {:>8} {:>8} {:>6} {:>9}",
            "cabinet", "base", "ante", "now", "should be"
        );
        for machine in crate::data::MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let Some(ante) = data.ante() else { continue };
            let base = run(
                &data,
                SimConfig {
                    spins: SPINS,
                    ..SimConfig::default()
                },
            );
            let anted = run(
                &data,
                SimConfig {
                    spins: SPINS,
                    ante: true,
                    ..SimConfig::default()
                },
            );
            let corrected = (ante.cost_permille as f64 * anted.rtp() / base.rtp()).round() as i64;
            println!(
                "{:<11} {:>8.4} {:>8.4} {:>6} {:>9}",
                machine.id,
                base.rtp(),
                anted.rtp(),
                ante.cost_permille,
                corrected
            );
        }
    }

    /// The claim, measured: an ante does not move the return.
    ///
    /// It only trades a bigger stake for a feature that arrives more often —
    /// the same trade the Feature Buy makes (§5.13) and the same promise. A
    /// cabinet whose ante fails this has no business selling one.
    #[test]
    #[ignore = "a few minutes; run before shipping a change to the ante"]
    fn the_ante_does_not_move_the_return() {
        const SPINS: u64 = 400_000;
        // Wider than the smoke tolerance on purpose. The price is an integer
        // permille derived from a finite sample, so it cannot land exactly, and
        // pretending otherwise would make this gate fail on noise.
        const TOLERANCE: f64 = 0.035;

        for machine in crate::data::MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let Some(ante) = data.ante() else {
                println!("{:<11} sells no ante", machine.id);
                continue;
            };
            let base = run(
                &data,
                SimConfig {
                    spins: SPINS,
                    ..SimConfig::default()
                },
            );
            let anted = run(
                &data,
                SimConfig {
                    spins: SPINS,
                    ante: true,
                    ..SimConfig::default()
                },
            );
            println!(
                "{:<11} base {:.4}  ante {:.4}  cost {:.3}  features {} -> {}",
                machine.id,
                base.rtp(),
                anted.rtp(),
                ante.cost_permille as f64 / 1000.0,
                base.features_triggered,
                anted.features_triggered,
            );
            assert!(
                (anted.rtp() - base.rtp()).abs() < TOLERANCE,
                "{}: the ante returns {:.4} against a base of {:.4}, which is a {:.1}%                  difference — it is priced wrong",
                machine.id,
                anted.rtp(),
                base.rtp(),
                (anted.rtp() / base.rtp() - 1.0) * 100.0
            );
            assert!(
                anted.features_triggered > base.features_triggered,
                "{}: the ante costs more and triggers the feature no more often",
                machine.id
            );
        }
    }

    /// What the ante actually does, swept across the one knob it has.
    ///
    /// Run with `--ignored --nocapture`. The cost is forced to 1.0 so the
    /// column reads as *value gained*, which is what a price has to be derived
    /// from (§5.75). Writing the panel from what the ante was meant to do
    /// rather than from this table is how a cabinet ends up selling a bet that
    /// takes 17% off the return, which is exactly what the first attempt did.
    #[test]
    #[ignore = "measurement, not a gate"]
    fn measure_the_ante() {
        const SPINS: u64 = 300_000;
        println!(
            "{:<11} {:>8} {:>7} {:>9} {:>8} {:>9} {:>9}",
            "cabinet", "base rtp", "extra", "gross rtp", "uplift", "fair cost", "features"
        );
        for machine in crate::data::MACHINES {
            let mut data = GameData::load_machine(machine).unwrap();
            let base = run(
                &data,
                SimConfig {
                    spins: SPINS,
                    ..SimConfig::default()
                },
            );
            println!(
                "{:<11} {:>8.4} {:>7} {:>9} {:>8} {:>9} {:>9}",
                machine.id,
                base.rtp(),
                "-",
                "-",
                "-",
                "-",
                base.features_triggered
            );

            for extra in [4usize, 8, 16, 32] {
                data.freespins.ante = Some(crate::data::Ante {
                    cost_permille: 1_000,
                    extra_scatters: extra,
                });
                let anted = run(
                    &data,
                    SimConfig {
                        spins: SPINS,
                        ante: true,
                        ..SimConfig::default()
                    },
                );
                // At cost 1.0 the RTP *is* the gross return per base stake, so
                // the price that leaves the return unchanged is simply the
                // ratio of the two.
                let fair = anted.rtp() / base.rtp();
                println!(
                    "{:<11} {:>8} {:>7} {:>9.4} {:>+8.4} {:>9.3} {:>9}",
                    "",
                    "",
                    extra,
                    anted.rtp(),
                    anted.rtp() - base.rtp(),
                    fair,
                    anted.features_triggered
                );
            }
        }
    }
}
