use super::*;

#[test]
fn short_sessions_are_reported_in_seconds() {
    // "You have been playing for 0 minutes" is worse than saying nothing.
    assert_eq!(duration(42.0, 0), "42 seconds");
}

#[test]
fn minutes_and_hours_read_as_english() {
    assert_eq!(duration(60.0, 1), "1 minute");
    assert_eq!(duration(1_500.0, 25), "25 minutes");
    assert_eq!(duration(3_600.0, 60), "1 hour");
    assert_eq!(duration(7_200.0, 120), "2 hours");
    assert_eq!(duration(5_400.0, 90), "1h 30m");
}

#[test]
fn every_duration_is_something_a_person_would_say() {
    for minutes in 0..400u32 {
        let text = duration(minutes as f32 * 60.0, minutes);
        assert!(!text.is_empty());
        assert!(!text.starts_with("0 minute"), "{}", text);
        assert!(!text.contains("0h"), "{}", text);
    }
}
