//! What the Seam actually costs the cabinet (§5.80).
//!
//! The feature adds a genuinely new prize, so it moves RTP — the same bargain
//! the jackpot layer (§5.6) and the respin round (§5.12) made. What is asserted
//! here is the **size** of that addition, per cabinet, measured on more than one
//! stream: a seam that quietly doubles a machine's return would otherwise show
//! up only as a drifting total three tests away, and the total band is wide
//! enough to hide it.

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

/// What each rite is worth, measured on the seams the cabinet really deals.
///
/// The point of offering a choice is that it is a choice of *texture*. If
/// one rite dominated, the panel would be a quiz with a right answer and
/// every player who learned it would be playing a different game from every
/// player who had not — which is the exact fault §5.64 built the equal-value
/// rule to avoid for the free-spin shapes.
///
/// The rites cannot be made equal by construction the way those shapes are:
/// a gilding is worth a multiple of a board that may be worth nothing, and
/// no arithmetic makes that identical to taking four more cells. So it is
/// measured instead, and what is asserted is that the *best* rite is not
/// worth more than three times the worst on any cabinet — a real spread,
/// because the boards differ, but not a right answer.
#[test]
fn no_rite_dominates_the_others_on_any_cabinet() {
    use crate::engine::seam;
    use crate::state::seam::SeamRound;
    use crate::state::GameSession;

    /// How much better the best rite may be than the worst. Not 1.0: the
    /// boards a seam opens on genuinely differ, and a rite that was equal on
    /// every one of them would be the same rite three times.
    const SPREAD: f64 = 2.0;

    let mut lopsided = Vec::new();
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let mut session = GameSession::new(&data, 0x5EA_11FE);
        let mut totals = vec![0i64; data.seam.rites.len()];
        // Seams that paid nothing at all, per rite. A rite comes out empty two
        // ways: a widening walled in by wilds and eggs, or a gilding on a board
        // that was not paying — and the second is the shape of the whole
        // decision (§5.85).
        let mut duds = vec![0u32; data.seam.rites.len()];
        let mut seams = 0u32;

        // Spin until enough real seams have been dealt, then run *every*
        // rite on each one from a copy of the round. Comparing rites on the
        // same boards is the whole method: a rite measured on its own set of
        // grids would be measuring the grids.
        for _ in 0..400_000 {
            if seams >= 600 {
                break;
            }
            session.balance = 1_000_000_000;
            session.celebrations.clear();
            let Ok(resolution) = session.spin_leaving_bonus(&data) else {
                break;
            };
            session.auto_play_bonus(&data);
            session.auto_play_holdspin(&data);
            if session.seam.take().is_none() {
                continue;
            }
            seams += 1;

            let line_bet = session.line_bet(&data);
            let grid = resolution.result.resting_grid().clone();
            let Some(found) = seam::find(&data, &grid, &data.seam) else {
                continue;
            };
            for index in 0..totals.len() {
                let ctx = crate::engine::evaluate::EvalContext::base(&data, line_bet);
                let mut copy = SeamRound::open(&data, &grid, found.clone(), ctx).expect("no round");
                assert!(copy.choose(index));
                // The same stream for every rite on a given seam, so a
                // widening's dice are not being compared against a
                // gilding's.
                let mut rng = macroquad_toolkit::rng::SeededRng::new(0x21FE + seams as u64);
                let paid = crate::state::seam::auto_play(&mut copy, &data, &mut rng).credits;
                totals[index] += paid;
                if paid == 0 {
                    duds[index] += 1;
                }
            }
        }

        assert!(seams > 0, "{} never dealt a seam", machine.id);
        let total_bet = data.total_bet(session.line_bet(&data));
        let means: Vec<f64> = totals
            .iter()
            .map(|sum| *sum as f64 / seams as f64 / total_bet as f64)
            .collect();
        for (index, (rite, mean)) in data.seam.rites.iter().zip(&means).enumerate() {
            println!(
                "{:>9} {:>8}: {:>6.2}x total bet per seam | nothing {:>5.1}% of the time",
                machine.id,
                rite.id,
                mean,
                duds[index] as f64 / seams as f64 * 100.0
            );
        }

        let best = means.iter().copied().fold(0.0f64, f64::max);
        let worst = means.iter().copied().fold(f64::MAX, f64::min);
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
