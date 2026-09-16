use super::*;

#[test]
fn writing_is_allowed_unless_something_says_otherwise() {
    // The default matters: a real session never calls `set_read_only`, and
    // a guard that defaulted to closed would silently stop saving the game.
    assert!(may_write());

    set_read_only(true);
    assert!(!may_write());
    set_read_only(false);
    assert!(may_write());
}
