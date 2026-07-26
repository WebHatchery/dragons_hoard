//! Whether this process is allowed to write to the player's saves (§5.55).
//!
//! # The harness was overwriting the game it was verifying
//!
//! The screenshot harness deals winning boards, fast-forwards into features,
//! empties a balance to reach the ruin screen and walks between cabinets. The
//! game loop autosaves whenever a spin resolves. Nobody had ever connected those
//! two facts, so **every capture scene wrote its fabricated state straight into
//! the real save slots** — and `verify.ps1` runs about fifty captures.
//!
//! Running the verification suite destroyed the player's game. Every time. It
//! had been true since the harness was written and nothing said so, because
//! nothing in a capture ever *reads* a save back and notices it is wrong.
//!
//! The tell was there: `Game::new` already asks `capture_requested` before
//! opening an audio device, because a headless run has no sound card. Nobody
//! asked the same question about the disk.
//!
//! # One gate, not six
//!
//! Six things persist — the session slot, the wallet, the ledger, achievements,
//! hints and preferences — through three different toolkit functions. Guarding
//! the two that happened to be noticed would have left the others writing, which
//! is exactly what the first attempt at this did: the save slots went quiet and
//! the ledger kept recording fabricated rounds.
//!
//! So the question is asked once, here, and every writer consults it.

use std::sync::atomic::{AtomicBool, Ordering};

static READ_ONLY: AtomicBool = AtomicBool::new(false);

/// Called once at boot. True under the screenshot harness.
pub fn set_read_only(read_only: bool) {
    READ_ONLY.store(read_only, Ordering::Relaxed);
}

/// Whether a save may be written.
///
/// Writers return `Ok(())` rather than an error when this is false: a capture
/// scene is not failing when it declines to overwrite a real game, and turning
/// that into a visible error would put a red notification in every screenshot.
pub fn may_write() -> bool {
    !READ_ONLY.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
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
}
