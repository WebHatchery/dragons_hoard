use super::*;

fn audit() -> MotionAudit {
    MotionAudit::new(2)
}

/// The disproof, on the fault this whole module exists for: a settled reel
/// whose symbols change has to be reported, or the clean runs mean nothing.
#[test]
fn a_settled_reel_that_changes_is_caught() {
    let mut audit = audit();
    audit.check_stable(0, vec![1, 2, 3]);
    assert!(
        audit.faults().is_empty(),
        "the first sighting is the record"
    );

    audit.frame = 7;
    audit.check_stable(0, vec![1, 2, 3]);
    assert!(audit.faults().is_empty(), "unchanged is not a fault");

    audit.check_stable(0, vec![4, 5, 6]);
    assert_eq!(audit.faults().len(), 1);
    assert!(matches!(
        audit.faults()[0],
        MotionFault::SettledReelChanged {
            reel: 0,
            frame: 7,
            ..
        }
    ));
}

/// And it reports once rather than once per remaining frame.
#[test]
fn a_change_is_reported_once_not_every_frame_after() {
    let mut audit = audit();
    audit.check_stable(0, vec![1, 2, 3]);
    audit.check_stable(0, vec![4, 5, 6]);
    for _ in 0..20 {
        audit.check_stable(0, vec![4, 5, 6]);
    }
    assert_eq!(audit.faults().len(), 1);
}

/// Reels are judged apart: one settling does not silence another.
#[test]
fn each_reel_is_tracked_on_its_own() {
    let mut audit = audit();
    audit.check_stable(0, vec![1, 1, 1]);
    audit.check_stable(1, vec![2, 2, 2]);
    audit.check_stable(0, vec![9, 9, 9]);
    assert_eq!(audit.faults().len(), 1);
    audit.check_stable(1, vec![8, 8, 8]);
    assert_eq!(audit.faults().len(), 2);
}

#[test]
fn the_readable_bound_is_under_a_whole_symbol() {
    // A symbol travelling its own full height between frames has passed the
    // eye entirely; the bound has to be tighter than that to mean anything.
    const { assert!(READABLE_SYMBOLS_PER_FRAME < 1.0) };
    const { assert!(READABLE_SYMBOLS_PER_FRAME > 0.0) };
}
