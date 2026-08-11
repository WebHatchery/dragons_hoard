use super::*;

fn entry() -> CelebrationKind {
    CelebrationKind::FreeSpinsEntry {
        spins: 10,
        scatters: 3,
    }
}

fn hatch() -> CelebrationKind {
    CelebrationKind::Hatch {
        credits: 500,
        eggs: 15,
    }
}

#[test]
fn the_first_card_shows_immediately() {
    let mut queue = CelebrationQueue::default();
    assert!(!queue.is_active());

    queue.push(entry());

    assert!(queue.is_active());
    assert_eq!(queue.active().unwrap().kind(), &entry());
    assert_eq!(queue.update(0.0), Some(entry()));
}

#[test]
fn skipping_never_leaves_the_game_briefly_unheld() {
    let mut queue = CelebrationQueue::default();
    queue.push(entry());
    queue.push(hatch());

    queue.skip();

    // The next card must already be up: a single frame with no card showing
    // would let the reels advance underneath the sequence.
    assert!(queue.is_active());
}

#[test]
fn every_card_announces_itself_exactly_once_in_order() {
    let mut queue = CelebrationQueue::default();
    queue.push(entry());
    queue.push(hatch());

    let mut opened = Vec::new();
    for _ in 0..1200 {
        if let Some(kind) = queue.update(1.0 / 60.0) {
            opened.push(kind);
        }
    }

    // The first card is made active by `push`, so its event has to come out
    // of `update` too — otherwise it would never get its particles.
    assert_eq!(opened, vec![entry(), hatch()]);
    assert!(!queue.is_active());
}

#[test]
fn the_queue_empties_once_every_card_has_run() {
    let mut queue = CelebrationQueue::default();
    queue.push(entry());

    for _ in 0..600 {
        queue.update(1.0 / 60.0);
    }

    assert!(!queue.is_active());
}

#[test]
fn skipping_advances_to_the_next_card() {
    let mut queue = CelebrationQueue::default();
    queue.push(entry());
    queue.push(hatch());

    queue.skip();
    assert_eq!(queue.active().unwrap().kind(), &hatch());

    queue.skip();
    assert!(!queue.is_active());
}

#[test]
fn the_backlog_is_bounded_so_a_headless_run_cannot_grow_it() {
    let mut queue = CelebrationQueue::default();
    for _ in 0..10_000 {
        queue.push(hatch());
    }

    assert!(queue.pending.len() <= MAX_QUEUED);
}

#[test]
fn a_card_fades_in_and_out_and_is_solid_in_between() {
    let mut queue = CelebrationQueue::default();
    queue.push(entry());
    // The first update spends itself announcing the card, not ticking it.
    queue.update(0.0);

    assert!(queue.active().unwrap().alpha() < 0.1, "should start faded");

    // Halfway through it should be fully opaque and at rest.
    let half = entry().duration() * 0.5;
    queue.update(half);
    let card = queue.active().unwrap();
    assert_eq!(card.alpha(), 1.0);
    assert_eq!(card.scale(), 1.0);

    // Just before the end it is fading again.
    queue.update(entry().duration() * 0.5 - FADE_TIME * 0.5);
    assert!(queue.active().unwrap().alpha() < 1.0);
}

#[test]
fn a_long_card_still_reaches_full_opacity_quickly() {
    let mut queue = CelebrationQueue::default();
    // The hatch card is the longest one; it must not spend a second of it
    // half-transparent with the reels legible through the middle.
    queue.push(hatch());
    queue.update(0.0);
    queue.update(FADE_TIME);

    assert_eq!(queue.active().unwrap().alpha(), 1.0);
}
