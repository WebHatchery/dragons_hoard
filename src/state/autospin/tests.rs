use super::*;

#[test]
fn a_run_counts_down_and_reports_when_it_is_spent() {
    let mut run = AutospinState::new(3);

    assert!(run.take());
    assert_eq!(run.remaining(), 2);
    assert!(run.take());
    assert!(!run.take(), "the third spin is the last one");
    assert_eq!(run.remaining(), 0);
}

#[test]
fn a_spent_run_cannot_go_negative() {
    let mut run = AutospinState::new(1);
    assert!(!run.take());
    assert!(!run.take());
    assert_eq!(run.remaining(), 0);
}
