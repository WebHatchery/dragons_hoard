//! Keyboard navigation for an immediate-mode UI (§5.27).
//!
//! # The soft-lock this exists to fix
//!
//! The game had keyboard shortcuts to *open* every panel and no way to do
//! anything inside one. Mostly that was an inconvenience. On the Vault Pick
//! (§5.10) it was a soft-lock: an open board **holds the game** — the reels do
//! not turn and a spin is refused — and the only way to clear it was to click a
//! chest. Fill the hoard without a mouse and the game stops for good.
//!
//! # How focus works when the controls do not exist between frames
//!
//! An immediate-mode UI has no widget tree to walk. There is only a sequence of
//! draw calls, and it happens to be the *same* sequence every frame as long as
//! the same panels are open. So focus is simply an index into that sequence.
//!
//! Each frame: read the movement and activation keys once, hand every control a
//! [`Hit`] as it draws, then wrap the index against however many there turned
//! out to be. A control does not need to know its own number and nothing has to
//! be registered in advance.
//!
//! The one thing this relies on is that the order is stable. It is, because the
//! panels draw in a fixed order — but the *set* changes when a panel opens, so
//! focus resets whenever the count does. Landing on a different button because
//! something else appeared would be worse than starting again.

use macroquad::prelude::*;
use macroquad_toolkit::ui::Pointer;

/// What a control learned about itself this frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hit {
    /// The keyboard is on this control. Draw it differently.
    pub focused: bool,
    /// It was activated, by mouse or by keyboard.
    pub activated: bool,
}

/// Tracks which control the keyboard is on.
///
/// Lives across frames on the orchestrator, because the index has to persist and
/// the controls do not.
#[derive(Debug, Clone, Default)]
pub struct Nav {
    index: usize,
    /// Controls seen this frame so far.
    seen: usize,
    /// How many there were last frame, so a changed count can reset focus.
    previous: usize,
    step: i32,
    activate: bool,
    /// Set once the player uses the keyboard, so the focus ring does not appear
    /// on a control the mouse merely happens to be near.
    engaged: bool,
    /// Held focus, for the capture harness only.
    pinned: Option<usize>,
}

impl Nav {
    /// Read this frame's input. Call once, before anything draws.
    pub fn begin(&mut self) {
        macroquad_toolkit::ui::begin_target_frame();
        self.seen = 0;
        self.step = 0;
        self.activate = false;

        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        if is_key_pressed(KeyCode::Tab) {
            self.step += if shift { -1 } else { 1 };
        }
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::Right) {
            self.step += 1;
        }
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Left) {
            self.step -= 1;
        }
        // Enter rather than Space: Space already spins, and a key that both
        // spins and presses whatever is focused would be a trap.
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
            self.activate = true;
        }

        if self.step != 0 || self.activate {
            self.engaged = true;
        }

        // Move *now*, against last frame's count, so the ring lands on the new
        // control in the same frame the key was pressed. Applying it after the
        // controls had drawn would show the move a frame late and — worse —
        // activate the one the player had just left.
        if self.step != 0 && self.previous > 0 {
            let count = self.previous as i32;
            self.index = (self.index as i32 + self.step).rem_euclid(count) as usize;
        }
        // Any pointer movement hands control back, so the ring does not linger
        // somewhere the player has stopped looking. A touch does the same, and
        // more emphatically: a finger has arrived somewhere specific.
        if mouse_delta_position() != Vec2::ZERO || !touches().is_empty() {
            self.engaged = false;
        }
    }

    /// Register a control and find out what happened to it.
    ///
    /// `enabled` controls are the only ones focus lands on — stepping onto a
    /// greyed-out button and pressing Enter to no effect reads as a broken key,
    /// not a disabled control.
    pub fn control(&mut self, rect: Rect, enabled: bool, pointer: Pointer) -> Hit {
        if !enabled {
            return Hit::default();
        }
        let slot = self.seen;
        self.seen += 1;

        // Every control in the game passes through here, which is why touch
        // could be added in one place rather than a hundred (§5.45). It is also
        // the only place that knows a control exists, so it is where the
        // hit-target audit measures.
        macroquad_toolkit::ui::note_target(&format!("{}x{}", rect.w, rect.h), rect);
        // And its footprint, so text drawn across it is caught (§5.47).
        macroquad_toolkit::ui::note_control(&format!("control {}x{}", rect.w, rect.h), rect);
        // And for next frame’s growth limits (§5.48).
        macroquad_toolkit::ui::note_neighbour(rect);

        let focused = self.engaged && slot == self.index;
        // Hit-tested against the grown area, drawn at the size it was given
        // (§5.45). A small precise control with a generous invisible margin is
        // the usual answer: visual weight is a design decision and target size
        // is an accessibility one, and they need not be the same number.
        let clicked = pointer.released_on(macroquad_toolkit::ui::touch_area(rect));

        Hit {
            focused,
            activated: clicked || (focused && self.activate),
        }
    }

    /// Record what this frame contained. Call once, after everything has drawn.
    pub fn finish(&mut self) {
        macroquad_toolkit::ui::end_frame_neighbours();
        if self.seen == 0 {
            self.index = 0;
            self.previous = 0;
            return;
        }

        // A changed control count means a panel opened or closed. Starting again
        // beats silently moving focus onto something else.
        if self.seen != self.previous {
            self.index = 0;
        }
        self.previous = self.seen;
        if let Some(index) = self.pinned {
            self.index = index;
        }
        self.index = self.index.min(self.seen - 1);
    }
}

impl Nav {
    /// Hold focus on a known control. For the capture harness (§5.27), which
    /// has no keyboard to press.
    ///
    /// Sticky, because the ordinary reset-on-count-change would wipe it on the
    /// first frame — the panel it is aimed at has only just opened.
    pub fn pin(&mut self, index: usize) {
        self.pinned = Some(index);
        self.index = index;
        self.engaged = true;
    }
}

/// Draw the focus ring around a control.
///
/// Deliberately loud. A focus indicator that has to be looked for is not one.
pub fn focus_ring(rect: Rect) {
    let ring = Rect::new(rect.x - 3.0, rect.y - 3.0, rect.w + 6.0, rect.h + 6.0);
    draw_rectangle_lines(
        ring.x,
        ring.y,
        ring.w,
        ring.h,
        3.0,
        super::palette::gold_bright(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pointer nowhere near anything, so a test measures focus rather than
    /// hit-testing.
    fn away() -> Pointer {
        Pointer {
            position: Vec2::new(-100.0, -100.0),
            ..Pointer::default()
        }
    }

    /// Step the nav without touching the keyboard, which a test has no access
    /// to — `begin` reads real input, so these drive the state directly.
    fn frame(nav: &mut Nav, controls: usize, step: i32, activate: bool) -> Vec<Hit> {
        *nav = Nav {
            seen: 0,
            step,
            activate,
            engaged: true,
            ..nav.clone()
        };
        // What `begin` does with the keys, minus the reading of them.
        if step != 0 && nav.previous > 0 {
            let count = nav.previous as i32;
            nav.index = (nav.index as i32 + step).rem_euclid(count) as usize;
        }

        let hits = (0..controls)
            .map(|i| nav.control(Rect::new(i as f32 * 10.0, 0.0, 5.0, 5.0), true, away()))
            .collect();
        nav.finish();
        hits
    }

    #[test]
    fn focus_starts_on_the_first_control() {
        let mut nav = Nav::default();
        let hits = frame(&mut nav, 4, 0, false);
        assert!(hits[0].focused);
        assert!(hits[1..].iter().all(|hit| !hit.focused));
    }

    #[test]
    fn exactly_one_control_is_focused() {
        let mut nav = Nav::default();
        for step in 0..12 {
            let hits = frame(&mut nav, 5, if step == 0 { 0 } else { 1 }, false);
            assert_eq!(hits.iter().filter(|hit| hit.focused).count(), 1);
        }
    }

    #[test]
    fn focus_wraps_in_both_directions() {
        let mut nav = Nav::default();
        frame(&mut nav, 3, 0, false);
        // Forward past the end.
        for _ in 0..3 {
            frame(&mut nav, 3, 1, false);
        }
        assert!(frame(&mut nav, 3, 0, false)[0].focused);
        // Backward past the start.
        let hits = frame(&mut nav, 3, -1, false);
        assert!(hits[2].focused);
    }

    #[test]
    fn activating_fires_the_focused_control_and_nothing_else() {
        let mut nav = Nav::default();
        frame(&mut nav, 4, 0, false);
        frame(&mut nav, 4, 1, false);

        let hits = frame(&mut nav, 4, 0, true);
        assert!(hits[1].activated);
        assert_eq!(hits.iter().filter(|hit| hit.activated).count(), 1);
    }

    #[test]
    fn a_disabled_control_is_skipped_entirely() {
        // Stepping onto a greyed-out button and pressing Enter to no effect
        // reads as a broken key rather than a disabled control.
        let mut nav = Nav {
            engaged: true,
            activate: true,
            ..Nav::default()
        };

        let away = away();
        let disabled = nav.control(Rect::new(0.0, 0.0, 5.0, 5.0), false, away);
        let enabled = nav.control(Rect::new(10.0, 0.0, 5.0, 5.0), true, away);
        nav.finish();

        assert!(!disabled.focused && !disabled.activated);
        assert!(enabled.focused && enabled.activated);
    }

    #[test]
    fn focus_resets_when_a_panel_opens_or_closes() {
        // The control order is only stable while the same panels are open.
        // Keeping an index across a change would move focus somewhere arbitrary.
        let mut nav = Nav::default();
        frame(&mut nav, 6, 0, false);
        for _ in 0..4 {
            frame(&mut nav, 6, 1, false);
        }

        // A panel opens. The count is only known once everything has drawn, so
        // the reset lands on the frame after — which is fine, because nobody has
        // pressed anything in between.
        frame(&mut nav, 9, 0, false);
        assert!(frame(&mut nav, 9, 0, false)[0].focused);
    }

    #[test]
    fn a_frame_with_no_controls_leaves_focus_somewhere_valid() {
        let mut nav = Nav::default();
        frame(&mut nav, 5, 1, false);
        frame(&mut nav, 0, 0, false);
        // And the next frame with controls starts from the top rather than an
        // index left over from before.
        assert!(frame(&mut nav, 3, 0, false)[0].focused);
    }
}
