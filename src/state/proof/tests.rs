use super::*;
use crate::state::GameSession;

/// Play a cabinet and keep **every** commitment, not the log's window.
///
/// The first version of this returned `log.entries()`, which is the last
/// `KEPT` — so a test that played four thousand spins looking for a free
/// one examined the final twenty-four and found none. The log rolling over
/// is a feature of the log, not of the run.
fn play(machine: &str, spins: usize) -> (GameData, Vec<Commitment>) {
    let data = GameData::load_machine(crate::data::machine_by_id(machine)).unwrap();
    let mut session = GameSession::new(&data, 0x51E4_7B03);
    let mut log = ProofLog::default();
    let mut every = Vec::new();
    for _ in 0..spins {
        session.balance = 1_000_000;
        session.celebrations.clear();
        if session.spin(&data).is_err() {
            break;
        }
        if let Some(commitment) = session.committed.take() {
            log.push(commitment);
            every.push(log.entries()[0].clone());
        }
    }
    (data, every)
}

/// The claim, stated as a test: a spin the game committed to runs again to
/// the same board and the same payout.
#[test]
fn every_committed_spin_reruns_identically() {
    for machine in crate::data::MACHINES {
        let (_, entries) = play(machine.id, 24);
        assert!(!entries.is_empty(), "{} recorded nothing", machine.id);
        for entry in &entries {
            assert_eq!(
                verify(entry),
                Verdict::Matches,
                "{} spin {} did not re-run identically: {}",
                machine.id,
                entry.seq,
                verify(entry).message()
            );
        }
    }
}

/// And the other half, which is the one that makes it worth anything: a
/// record that has been altered is caught. A verifier that only ever says
/// yes is a picture of a tick.
#[test]
fn a_payout_edited_after_the_fact_is_caught() {
    let (_, entries) = play("dragon", 12);
    let mut tampered = entries[0].clone();
    tampered.win += 500;
    match verify(&tampered) {
        Verdict::Differs(how) => {
            assert!(how.contains("500") || how.contains("paid"), "{}", how)
        }
        other => panic!("an edited payout passed: {:?}", other),
    }
}

/// A board swapped for a better one, with the payout left alone.
#[test]
fn a_grid_edited_after_the_fact_is_caught() {
    let (_, entries) = play("dragon", 12);
    let mut tampered = entries[0].clone();
    tampered.grid[0] = "not_a_symbol".to_owned();
    match verify(&tampered) {
        Verdict::Differs(how) => assert!(how.contains("differ"), "{}", how),
        other => panic!("an edited board passed: {:?}", other),
    }
}

/// The state is the thing that decides. Move it by one and a different
/// spin comes out — which is also why it is worth showing the player.
#[test]
fn a_different_state_is_a_different_spin() {
    let (_, entries) = play("dragon", 12);
    let mut moved = entries[0].clone();
    moved.state = moved.state.wrapping_add(1);
    assert_ne!(
        verify(&moved),
        Verdict::Matches,
        "shifting the deciding number changed nothing, which would mean it does not decide"
    );
}

/// The same number on two cabinets is two different spins, so the record
/// has to name the cabinet as well.
#[test]
fn the_cabinet_is_part_of_the_record() {
    let (_, entries) = play("dragon", 8);
    let mut moved = entries[0].clone();
    moved.machine = "tidepool".to_owned();
    assert_ne!(
        verify(&moved),
        Verdict::Matches,
        "the same state verified on a different cabinet"
    );
}

/// An unknown cabinet is neither a pass nor a failure, and must not be
/// reported as either.
#[test]
fn a_cabinet_this_build_does_not_have_is_said_so() {
    let (_, entries) = play("dragon", 4);
    let mut gone = entries[0].clone();
    gone.machine = "a_cabinet_that_never_shipped".to_owned();
    assert_eq!(verify(&gone), Verdict::UnknownMachine);
    assert!(!verify(&gone).is_match());
}

/// Free spins run on refined strips (§5.21) and a multiplier chosen by the
/// player (§5.64), so the mode has to be part of the record too.
#[test]
fn the_mode_is_part_of_the_record() {
    let (data, entries) = play("frost", 4_000);
    assert!(
        entries
            .iter()
            .any(|entry| !matches!(entry.mode, RecordedMode::Base { .. })),
        "four thousand spins on {} never reached a free spin",
        data.machine_id()
    );
    let free = entries
        .iter()
        .find(|entry| !matches!(entry.mode, RecordedMode::Base { .. }))
        .unwrap();
    let mut as_base = free.clone();
    as_base.mode = RecordedMode::Base { ante: false };
    assert_ne!(
        verify(&as_base),
        Verdict::Matches,
        "a free spin verified as a base spin, so the refined strips are not being used"
    );
}

/// The log holds the last `KEPT` and numbers them for good.
#[test]
fn the_log_rolls_over_without_renumbering() {
    let (_, every) = play("dragon", KEPT * 2 + 5);
    assert_eq!(every.len(), KEPT * 2 + 5);

    let mut log = ProofLog::default();
    for commitment in &every {
        let mut fresh = commitment.clone();
        fresh.seq = 0;
        log.push(fresh);
    }
    let entries = log.entries();
    assert_eq!(entries.len(), KEPT);
    assert_eq!(entries[0].seq, (KEPT * 2 + 5) as u64);
    for pair in entries.windows(2) {
        assert_eq!(
            pair[0].seq,
            pair[1].seq + 1,
            "the log is not newest-first and contiguous"
        );
    }
}

/// Verification must not disturb the generator that is still being played.
/// Re-running a spin restores a *copy* of the state; if it reached into the
/// live one, checking a spin would change the next one.
#[test]
fn checking_a_spin_does_not_change_the_next_one() {
    let data = GameData::load_machine(crate::data::machine_by_id("dragon")).unwrap();
    let mut session = GameSession::new(&data, 0x51E4_7B03);
    let mut log = ProofLog::default();
    for _ in 0..12 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        session.spin(&data).unwrap();
        if let Some(commitment) = session.committed.take() {
            log.push(commitment);
        }
    }

    let untouched = session.rng.state();
    let all = log.verify_all();
    assert!(all.iter().all(|(_, verdict)| verdict.is_match()));
    assert_eq!(
        session.rng.state(),
        untouched,
        "verifying reached into the live generator"
    );
}
