//! What the Seam actually costs the cabinet (§5.80).
//!
//! The feature adds a genuinely new prize, so it moves RTP — the same bargain
//! the jackpot layer (§5.6) and the respin round (§5.12) made. What is asserted
//! here is the **size** of that addition, per cabinet, measured on more than one
//! stream: a seam that quietly doubles a machine's return would otherwise show
//! up only as a drifting total three tests away, and the total band is wide
//! enough to hide it.
//!
//! Since §5.81 the player picks the rite, and since §5.86 the figure that has to
//! be in band is the one a player who **never picks wrong** would collect.

use super::super::*;
use crate::data::MACHINES;

/// Share of turnover the seam may return. The floor is as important as the
/// ceiling: a feature returning nothing is one whose trigger has quietly
/// stopped firing, and that is the failure mode a strips edit produces.
///
/// The ceiling is where it is because of what it has to fit inside. The
/// catalog sat between 0.955 and 0.963 before this feature existed, against
/// a full-run gate of 0.95 +/- 0.03 — so there was about two points of RTP
/// of room in the whole game, and the seam had to be built to live in it
/// rather than the paytables re-cut to make more.
const BAND: std::ops::Range<f64> = 0.005..0.035;

/// Every rite measured over the same set of real seams (§5.81, §5.86).
#[derive(Debug)]
struct Rites {
    /// Mean payout per seam, in total bets, one per rite in `seam.json` order.
    means: Vec<f64>,
    /// Share of seams each rite paid nothing on.
    duds: Vec<f64>,
    /// Mean payout under the cabinet's own weights — what the sim measures, and
    /// therefore what every published RTP figure in this document is about.
    drawn: f64,
    /// Mean payout taking the best rite **for that board**, every time. No fixed
    /// choice can match it, and it is what a panel that tells the player enough
    /// to choose well hands out.
    perfect: f64,
    /// Seams per paid spin.
    rate: f64,
}

impl Rites {
    /// Turnover a perfect chooser returns over one who lets the weights choose.
    ///
    /// The number a cabinet's stated return is missing whenever the panel is
    /// informative enough for anyone to collect it (§5.86).
    fn edge(&self) -> f64 {
        (self.perfect - self.drawn) * self.rate
    }
}

/// Deal `wanted` real seams and run **every** rite on each one from a copy of
/// the round.
///
/// Comparing rites on the same boards from the same stream is the whole method:
/// a rite measured on its own set of grids would be measuring the grids.
fn measure_rites(data: &GameData, wanted: u32) -> Rites {
    use crate::engine::seam;
    use crate::state::seam::SeamRound;
    use crate::state::GameSession;

    let mut session = GameSession::new(data, 0x5EA1_1FE0);
    let rites = data.seam.rites.len();
    let mut totals = vec![0i64; rites];
    let mut duds = vec![0u32; rites];
    let mut perfect = 0i64;
    let mut drawn = 0i64;
    let mut seams = 0u32;
    let mut spins = 0u64;
    let weight_sum: i64 = data.seam.rites.iter().map(|rite| rite.weight as i64).sum();

    for _ in 0..600_000 {
        if seams >= wanted {
            break;
        }
        session.balance = 1_000_000_000;
        session.celebrations.clear();
        spins += 1;
        let Ok(resolution) = session.spin_leaving_bonus(data) else {
            break;
        };
        session.auto_play_bonus(data);
        session.auto_play_holdspin(data);
        if session.seam.take().is_none() {
            continue;
        }
        seams += 1;

        let line_bet = session.line_bet(data);
        let grid = resolution.result.resting_grid().clone();
        let Some(found) = seam::find(data, &grid, &data.seam) else {
            continue;
        };

        let mut best = 0i64;
        for (index, rite) in data.seam.rites.iter().enumerate() {
            let ctx = crate::engine::evaluate::EvalContext::base(data, line_bet);
            let mut copy = SeamRound::open(data, &grid, found.clone(), ctx).expect("no round");
            assert!(copy.choose(index));
            // One stream per seam, shared by every rite, so a widening's dice
            // are not being compared against a gilding's.
            let mut rng = macroquad_toolkit::rng::SeededRng::new(0x21FE + seams as u64);
            let paid = crate::state::seam::auto_play(&mut copy, data, &mut rng).credits;

            totals[index] += paid;
            if paid == 0 {
                duds[index] += 1;
            }
            best = best.max(paid);
            drawn += paid * rite.weight as i64;
        }
        perfect += best;
    }

    assert!(seams > 0, "{} never dealt a seam", data.machine.id);
    let per_seam = seams as f64 * data.total_bet(session.line_bet(data)) as f64;
    Rites {
        means: totals.iter().map(|sum| *sum as f64 / per_seam).collect(),
        duds: duds
            .iter()
            .map(|count| *count as f64 / seams as f64)
            .collect(),
        drawn: drawn as f64 / weight_sum.max(1) as f64 / per_seam,
        perfect: perfect as f64 / per_seam,
        rate: seams as f64 / spins as f64,
    }
}

/// What each rite is worth, measured on the seams the cabinet really deals.
///
/// The point of offering a choice is that it is a choice of *texture*. If one
/// rite dominated, the panel would be a quiz with a right answer and every
/// player who learned it would be playing a different game from every player
/// who had not — which is the exact fault §5.64 built the equal-value rule to
/// avoid for the free-spin shapes.
///
/// The rites cannot be made equal by construction the way those shapes are: a
/// gilding is worth a multiple of a board that may be worth nothing, and no
/// arithmetic makes that identical to taking four more cells. So it is measured
/// instead, and what is asserted is that the best rite is not worth more than
/// twice the worst — a real spread, because the boards differ, but not a right
/// answer.
#[test]
fn no_rite_dominates_the_others_on_any_cabinet() {
    /// How much better the best rite may be than the worst. Not 1.0: the boards
    /// a seam opens on genuinely differ, and a rite that was equal on every one
    /// of them would be the same rite three times.
    const SPREAD: f64 = 2.0;

    let mut lopsided = Vec::new();
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let measured = measure_rites(&data, 600);

        for (index, rite) in data.seam.rites.iter().enumerate() {
            println!(
                "{:>9} {:>8}: {:>6.2}x per seam | nothing {:>5.1}% of the time",
                machine.id,
                rite.id,
                measured.means[index],
                measured.duds[index] * 100.0
            );
        }
        println!(
            "{:>9} {:>8}: {:>6.2}x drawn, {:>6.2}x perfect | 1 seam in {:.0} | choosing \
             well is worth {:+.4} of turnover",
            machine.id,
            "choice",
            measured.drawn,
            measured.perfect,
            1.0 / measured.rate,
            measured.edge()
        );

        let best = measured.means.iter().copied().fold(0.0f64, f64::max);
        let worst = measured.means.iter().copied().fold(f64::MAX, f64::min);
        if worst <= 0.0 || best > worst * SPREAD {
            lopsided.push(format!(
                "{} spans {:.2}x to {:.2}x",
                machine.id, worst, best
            ));
        }
    }

    assert!(
        lopsided.is_empty(),
        "these cabinets offer a right answer rather than a choice: {}",
        lopsided.join(", ")
    );
}

/// The return has to hold for a player who *always* takes the same rite.
///
/// This is what offering a choice costs. Before §5.81 the rite was drawn by
/// weight, so the weighted mean in `seam.json` was the return and there was
/// nothing else to check. A player picks, and a player who has worked out
/// which rite is best will take it every time — so the figure that has to be
/// in band is not the mean of the three, it is **each of them**.
///
/// Run by rebuilding the cabinet with one rite in it, which is exactly the
/// game that player is playing.
#[test]
fn the_return_holds_whichever_rite_a_player_always_takes() {
    let mut adrift = Vec::new();
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        for rite in &data.seam.rites {
            let mut only = data.clone();
            only.seam.rites = vec![rite.clone()];
            let report = run(
                &only,
                SimConfig {
                    spins: 200_000,
                    ..SimConfig::default()
                },
            );

            let share = report.contribution(report.seam_won);
            println!(
                "{:>9} always {:>6}: RTP {:.4} | seam {:.4}",
                machine.id,
                rite.id,
                report.rtp(),
                share
            );
            if !BAND.contains(&share) {
                adrift.push(format!("{}/{} at {:.4}", machine.id, rite.id, share));
            }
        }
    }

    assert!(
        adrift.is_empty(),
        "a player who always takes one rite is outside the designed {:?}: {}",
        BAND,
        adrift.join(", ")
    );
}

/// **And for a player who never picks wrong.**
///
/// Every RTP figure in this document comes from the sim, and the sim draws its
/// rite by weight — so all of them describe a player who does not think about
/// the choice. §5.81 tightened this once already, from "the weighted mean" to
/// "each rite on its own"; both are still statements about a **fixed** policy.
///
/// A player choosing per board beats every fixed policy, and once the panel says
/// what the board is currently paying that is a policy anyone can run (§5.86).
/// So the figure that has to be in band is the cabinet's return **plus that
/// edge** — the first gate here to be about the best play rather than the
/// average one.
#[test]
#[ignore = "a sim run and a rite sweep per machine; too slow for a debug CI build"]
fn the_return_holds_for_a_player_who_never_picks_wrong() {
    const TARGET: f64 = 0.95;
    const TOLERANCE: f64 = 0.03;

    let mut adrift = Vec::new();
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let report = run(
            &data,
            SimConfig {
                spins: 400_000,
                ..SimConfig::default()
            },
        );
        let edge = measure_rites(&data, 600).edge();
        let perfect = report.rtp() + edge;

        println!(
            "{:>9}: drawn {:.4} | choosing well {:+.4} | perfect {:.4}",
            machine.id,
            report.rtp(),
            edge,
            perfect
        );
        if (perfect - TARGET).abs() >= TOLERANCE {
            adrift.push(format!("{} at {:.4}", machine.id, perfect));
        }
    }

    assert!(
        adrift.is_empty(),
        "a player who never picks wrong is outside {:.2} +/- {:.2}: {}",
        TARGET,
        TOLERANCE,
        adrift.join(", ")
    );
}

#[test]
fn every_cabinet_pays_the_seam_within_the_designed_band() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        for seed in [0xD2A6_0F1E_u64, 0xA11CE, 0x5EED_1234] {
            let report = run(
                &data,
                SimConfig {
                    spins: 20_000,
                    seed,
                    ..SimConfig::default()
                },
            );

            let share = report.contribution(report.seam_won);
            let rate = report.seams as f64 / report.paid_spins.max(1) as f64;
            println!(
                "{:>9} seed {:>10x}: seams {:>4} (1 in {:>5.0}) | {:.4} of turnover",
                machine.id,
                seed,
                report.seams,
                if rate > 0.0 { 1.0 / rate } else { 0.0 },
                share
            );
            assert!(
                BAND.contains(&share),
                "{} returns {:.4} through the seam, outside the designed {:?}",
                machine.id,
                share,
                BAND
            );
        }
    }
}
