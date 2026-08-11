use super::*;

/// The mechanics moved to `macroquad_toolkit::strip` (§5.23) and are tested
/// there against a default feel. What matters *here* is that this game's own
/// tuning still lands a reel on its stop — a `base_time` or `revolutions` edit
/// is exactly the sort of change that could break it without the toolkit
/// noticing.
#[test]
fn this_cabinets_feel_still_lands_every_reel_on_its_stop() {
    let feel = reel_feel();
    for index in 0..5 {
        for target in [0usize, 1, 17, 39] {
            let mut spinner = ReelSpinner::new(&[40], &[7], &[target], 1.0, &[false], &feel);
            for _ in 0..1_200 {
                spinner.tick(1.0 / 60.0);
            }
            assert!(spinner.all_settled());
            assert_eq!(
                spinner.position(0).round() as usize % 40,
                target,
                "reel {} missed its stop",
                index
            );
        }
    }
}

#[test]
fn anticipation_fires_only_when_the_feature_is_still_live() {
    // Two scatters on the first two reels, needing three: every reel still
    // to land could be the one that completes it, so all three are held
    // back. That is what a physical cabinet does, and it is the reason a
    // near-miss is agonising rather than instant.
    let flags = anticipating_reels(&[1, 1, 0, 0, 0], 3, 4);
    assert_eq!(flags, vec![false, false, true, true, true]);
}

#[test]
fn anticipation_never_fires_when_the_feature_cannot_be_reached() {
    // One scatter and four reels left cannot make three by reel 5 unless
    // more land, so nothing is held back yet.
    assert_eq!(anticipating_reels(&[1, 0, 0, 0, 0], 3, 4), vec![false; 5]);
    // And a board with no scatters at all never anticipates.
    assert_eq!(anticipating_reels(&[0; 5], 3, 4), vec![false; 5]);
}

#[test]
fn anticipation_continues_while_the_count_keeps_climbing() {
    // Scatters on reels 1 and 2 hold reel 3; a third scatter there means
    // the feature has already triggered, and reels 4 and 5 are chasing a
    // bigger award, so they keep anticipating.
    let flags = anticipating_reels(&[1, 1, 1, 0, 0], 3, 4);
    assert_eq!(flags, vec![false, false, true, true, true]);
}

#[test]
fn anticipation_is_capped() {
    // Without a cap a scatter-heavy board would stretch every remaining
    // reel and turn a spin into a slideshow.
    let flags = anticipating_reels(&[1, 1, 1, 1, 1], 3, 2);
    assert_eq!(flags.iter().filter(|held| **held).count(), 2);
}

#[test]
fn the_payout_counter_starts_at_zero_and_ends_on_target() {
    // The mechanism is the toolkit's (§5.63); what is checked here is that
    // *this game's* pacing produces a counter that actually completes
    // within the time a spin allows it.
    let mut counter = payout_counter(1234, 1.0);
    assert_eq!(counter.value(), 0);

    let mut finished = false;
    for _ in 0..120 {
        finished |= counter.tick(1.0 / 60.0);
    }

    assert!(finished);
    assert_eq!(counter.value(), 1234);
}

#[test]
fn the_payout_counter_finishes_exactly_once() {
    let mut counter = payout_counter(10, 1.0);
    let mut finishes = 0;
    for _ in 0..200 {
        if counter.tick(1.0 / 60.0) {
            finishes += 1;
        }
    }

    assert_eq!(finishes, 1);
}
