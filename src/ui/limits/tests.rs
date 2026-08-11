use super::*;

#[test]
fn cycling_walks_the_offered_values_and_wraps() {
    let choices = [0, 50, 100, 250];
    assert_eq!(next_choice(&choices, None), Some(50));
    assert_eq!(next_choice(&choices, Some(50)), Some(100));
    assert_eq!(next_choice(&choices, Some(250)), None);
}

#[test]
fn every_offering_is_reachable_by_cycling() {
    // A button that cannot reach a value in the data is a value nobody can
    // pick.
    let choices = [0i64, 15, 30, 60, 120];
    let mut seen = Vec::new();
    let mut current = None;
    for _ in 0..choices.len() {
        current = next_choice(&choices, current);
        seen.push(current.unwrap_or(0));
    }
    for value in choices {
        assert!(seen.contains(&value), "{} unreachable", value);
    }
}

#[test]
fn a_saved_value_no_longer_offered_still_cycles() {
    // The data can change under a saved preference; sticking would leave the
    // button dead.
    let choices = [0, 50, 100];
    // It lands somewhere in the offered set rather than keeping the orphan.
    let first = next_choice(&choices, Some(999));
    assert!(choices.contains(&first.unwrap_or(0)));
    // And pressing again keeps moving, so the button is never dead.
    let second = next_choice(&choices, first);
    assert_ne!(first, second);
}

#[test]
fn off_reads_as_off_rather_than_as_a_number() {
    for cap in [Cap::Time, Cap::Loss, Cap::Spins] {
        assert_eq!(cap_label(cap, None), "Off");
    }
    assert_eq!(minutes_label(0), "Off");
}

#[test]
fn a_set_cap_says_what_it_measures() {
    assert!(cap_label(Cap::Time, Some(30)).contains("minute"));
    assert!(cap_label(Cap::Loss, Some(5_000)).contains("credit"));
    assert!(cap_label(Cap::Spins, Some(100)).contains("spin"));
}
