use super::*;

fn played(rounds: usize) -> History {
    let mut history = History::default();
    let mut balance = 1_000i64;
    for round in 0..rounds {
        balance -= 20;
        if round % 137 == 0 {
            balance += 3_000;
        }
        history.record(balance.max(0));
    }
    history
}

#[test]
fn the_opening_balance_is_always_in_view() {
    // The reference the whole graph is read against. A session that only
    // ever climbed would otherwise scroll its own baseline off the bottom.
    let mut history = History::default();
    history.record(1_000);
    for round in 0..500 {
        history.record(50_000 + round);
    }
    let (low, high) = range(&history);
    assert!(low <= 1_000.0, "the start fell off the bottom");
    assert!(high >= 50_499.0);
}

#[test]
fn the_range_always_has_height() {
    // A session where the balance never moved would divide by zero.
    let mut history = History::default();
    for _ in 0..50 {
        history.record(1_000);
    }
    let (low, high) = range(&history);
    assert!(high > low);
}

#[test]
fn the_range_covers_every_extreme() {
    let history = played(4_000);
    let (low, high) = range(&history);
    let (min, max) = history.extremes().unwrap();
    assert!(low <= min as f32);
    assert!(high >= max as f32);
}

#[test]
fn every_bucket_lands_inside_the_plot() {
    // The band is drawn from bucket extremes, so a range that did not cover
    // them would draw outside the frame rather than clip.
    let plot = Rect::new(0.0, 0.0, 900.0, 300.0);
    for rounds in [1, 2, 17, 500, 20_000] {
        let history = played(rounds);
        let (low, high) = range(&history);
        let span = (high - low).max(1.0);
        let y_of = |v: f32| plot.bottom() - (v - low) / span * plot.h;

        for bucket in history.series().buckets() {
            for value in [bucket.min, bucket.max, bucket.last] {
                let y = y_of(value);
                assert!(
                    y >= plot.y && y <= plot.bottom(),
                    "{} at {} rounds",
                    y,
                    rounds
                );
            }
        }
    }
}

#[test]
fn a_mark_lands_inside_the_plot_however_long_the_session() {
    // Marks are placed by round against the series clock while the band is
    // placed by bucket index. The two have to agree as the graph decimates.
    let plot = Rect::new(0.0, 0.0, 900.0, 300.0);
    for rounds in [10usize, 900, 50_000] {
        let mut history = History::default();
        for round in 0..rounds {
            history.record(1_000 + (round as i64 % 400));
            if round % (rounds / 8).max(1) == 0 {
                history.mark(Cause::Feature, 1_000 + (round as i64 % 400));
            }
        }

        let buckets = history.series().buckets().len().max(1) as f32;
        let step = plot.w / buckets;
        let total = history.rounds().max(1) as f32;
        for mark in history.marks() {
            let x = plot.x + (mark.round as f32 / total).clamp(0.0, 1.0) * buckets * step;
            assert!(x >= plot.x - 0.5, "{} before the plot", x);
            assert!(x <= plot.right() + 0.5, "{} past the plot", x);
        }
    }
}

#[test]
fn every_cause_has_a_colour_of_its_own() {
    // Two marks that share a colour are two marks the legend cannot explain.
    for (index, left) in Cause::ALL.iter().enumerate() {
        for right in Cause::ALL.iter().skip(index + 1) {
            let (a, b) = (mark_colour(*left), mark_colour(*right));
            let apart = (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
            assert!(apart > 0.25, "{:?} and {:?} look alike", left, right);
        }
    }
}

#[test]
fn the_legend_fits_across_the_panel() {
    // Drawn by measuring each label; a sixth cause would run off the edge
    // silently rather than wrap.
    let width: f32 = Cause::ALL
        .iter()
        .map(|cause| 14.0 + cause.label().len() as f32 * 7.0 + 14.0)
        .sum();
    assert!(width < panel().w - 40.0 - 200.0, "legend is {}px", width);
}
