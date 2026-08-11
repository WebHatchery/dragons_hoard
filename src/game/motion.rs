//! What is on screen while the reels are turning (§5.52).
//!
//! # The blind spot this project named and left open
//!
//! Two bugs in this game came from a player rather than from the tests, and
//! §15 wrote down why:
//!
//! > everything here is asserted about *state*, and both of these were about
//! > what is on screen at a moment when the state is **mid-flight**. The capture
//! > harness photographs settled frames, so it could not have caught them
//! > either.
//!
//! That was recorded as a lesson and nothing was built for it. Both bugs are
//! frame-level properties of a spin, and both are checkable:
//!
//! - **A reel that had landed kept drawing the previous spin's symbols** until
//!   the last reel settled, and then the whole board snapped. Stated as an
//!   invariant: *once a reel has settled, what it shows must not change again*.
//!   The snap is a settled reel's symbols changing, which is exactly what that
//!   forbids.
//! - **The reels were too fast to read the art in flight.** Stated as a
//!   measurement: *a reel drawing detailed art must not travel more than half a
//!   symbol between frames*. Faster than that and the art is a smear whatever
//!   the artist did, which is why the fix was a speed change rather than a
//!   drawing change.
//!
//! # Why per-frame and not per-state
//!
//! Both properties are about a *sequence* of frames, not a single one. A settled
//! reel changing is only visible by comparing this frame against the one where
//! it settled; travel per frame is a difference between two positions. Nothing
//! that inspects one state can see either, which is why a hundred state tests
//! and a capture harness both missed them.
//!
//! So the audit accumulates across a run, and the run is a real spin stepped at
//! a fixed timestep — the same path the game takes, not a simulation of it.

use crate::state::GameSession;

/// Something wrong with a frame, given the frames before it.
#[derive(Debug, Clone, PartialEq)]
pub enum MotionFault {
    /// A reel that had stopped changed what it was showing.
    SettledReelChanged {
        reel: usize,
        frame: u32,
        was: Vec<usize>,
        now: Vec<usize>,
    },
    /// Detailed art moving faster than it can be read.
    TooFastToRead {
        reel: usize,
        frame: u32,
        symbols_per_frame: f32,
    },
    /// The board went empty, or changed size, while a spin was in flight.
    BoardVanished { frame: u32 },
    /// The run never saw a reel turn, or never saw a spin reach rest.
    ///
    /// Not a fault in the game — a fault in the *run*, and the more dangerous
    /// kind. A 96-frame capture of a spin that takes 130 frames never observes a
    /// reel settling, so the invariant that a settled reel keeps its symbols is
    /// never once evaluated and the audit reports clean. That happened on the
    /// first run of this module and was caught by looking at the filmstrip, not
    /// by reading the verdict.
    NothingObserved { spinning: u32, settled: usize },
}

impl MotionFault {
    pub fn describe(&self) -> String {
        match self {
            MotionFault::SettledReelChanged {
                reel, frame, was, now,
            } => format!(
                "frame {}: reel {} had settled showing {:?} and now shows {:?}",
                frame, reel, was, now
            ),
            MotionFault::TooFastToRead {
                reel,
                frame,
                symbols_per_frame,
            } => format!(
                "frame {}: reel {} moves {:.2} symbols per frame with detailed art — \
                 the symbols cannot be read",
                frame, reel, symbols_per_frame
            ),
            MotionFault::BoardVanished { frame } => {
                format!("frame {}: the board is empty mid-spin", frame)
            }
            MotionFault::NothingObserved { spinning, settled } => format!(
                "the run saw {} frames of movement and {} spins reach rest — too little to have checked anything, so this is not a pass",
                spinning, settled
            ),
        }
    }
}

/// Half a symbol per frame. Above this the art crosses more than its own centre
/// between one drawn frame and the next, and reads as a streak — which is what
/// a player reported before `BASE_SPIN_TIME` went from 0.62 to 0.95.
const READABLE_SYMBOLS_PER_FRAME: f32 = 0.5;

/// Accumulates what it has seen, so a fault can be about a sequence.
#[derive(Debug, Default)]
pub struct MotionAudit {
    frame: u32,
    /// What each reel showed the frame it settled. `None` until it does.
    settled: Vec<Option<Vec<usize>>>,
    positions: Vec<f32>,
    faults: Vec<MotionFault>,
    /// How much the run actually saw, so a clean verdict can be trusted.
    spinning: u32,
    settled_seen: usize,
    was_spinning: bool,
    /// How many spins the run watched all the way to rest.
    resolutions: u32,
}

impl MotionAudit {
    pub fn new(reels: usize) -> Self {
        Self {
            frame: 0,
            settled: vec![None; reels],
            positions: vec![f32::NAN; reels],
            faults: Vec::new(),
            spinning: 0,
            settled_seen: 0,
            was_spinning: false,
            resolutions: 0,
        }
    }

    /// Look at one frame. Call after the game has updated and drawn it.
    pub fn observe(&mut self, session: &GameSession) {
        let grid = session.display_grid();
        // Wyrmspire's reels are different heights each spin (§5.20), so the
        // board's shape is asked per reel rather than assumed from the config.
        if grid.reel_count() < self.settled.len() || grid.cell_count() == 0 {
            self.faults
                .push(MotionFault::BoardVanished { frame: self.frame });
            self.frame += 1;
            return;
        }
        let columns: Vec<Vec<usize>> = (0..self.settled.len())
            .map(|reel| {
                (0..grid.rows_on(reel))
                    .map(|row| grid.at(reel, row))
                    .collect()
            })
            .collect();

        match session.phase.spinner() {
            Some(spinner) => {
                self.spinning += 1;
                self.was_spinning = true;
                for (reel, column) in columns.into_iter().enumerate() {
                    self.check_speed(spinner, reel);
                    // A reel is settled when its strip has stopped. What it
                    // shows at that moment is what it must keep showing.
                    if spinner.get(reel).is_some_and(|strip| strip.settled()) {
                        self.check_stable(reel, column);
                    }
                }
            }
            None if self.was_spinning => {
                // The spin is over, and this is the comparison that matters
                // most. The reported bug was the whole board *snapping* the
                // moment the last reel landed — every earlier reel replaced at
                // once by symbols it had not been showing. That is exactly this
                // check failing, and it cannot be made while the spinner still
                // exists: the last reel settles and the spin resolves on the
                // same frame, so there is never a frame with all reels stopped
                // and a spinner to ask.
                self.was_spinning = false;
                self.resolutions += 1;
                for (reel, column) in columns.into_iter().enumerate() {
                    self.check_stable(reel, column);
                }
                self.settled.iter_mut().for_each(|slot| *slot = None);
            }
            None => {}
        }
        self.frame += 1;
    }

    fn check_speed(&mut self, spinner: &crate::state::spin::ReelSpinner, reel: usize) {
        let Some(strip) = spinner.get(reel) else {
            return;
        };
        let position = strip.position();
        let previous = self.positions[reel];
        self.positions[reel] = position;

        // Blurred art is *meant* to streak — that is the fix for speed, not a
        // symptom of it. The fault is detailed art moving too fast to resolve.
        if spinner.is_blurred(reel) || previous.is_nan() {
            return;
        }
        let travel = (position - previous).abs();
        if travel > READABLE_SYMBOLS_PER_FRAME {
            self.faults.push(MotionFault::TooFastToRead {
                reel,
                frame: self.frame,
                symbols_per_frame: travel,
            });
        }
    }

    fn check_stable(&mut self, reel: usize, column: Vec<usize>) {
        match &self.settled[reel] {
            Some(was) if *was != column => {
                self.faults.push(MotionFault::SettledReelChanged {
                    reel,
                    frame: self.frame,
                    was: was.clone(),
                    now: column,
                });
                // Record the new value, or every remaining frame reports the
                // same change and the report is a wall of one fault.
                self.settled[reel] = None;
            }
            Some(_) => {}
            None => {
                self.settled_seen += 1;
                self.settled[reel] = Some(column);
            }
        }
    }

    pub fn faults(&self) -> &[MotionFault] {
        &self.faults
    }

    /// One line per fault, or a word saying there were none.
    ///
    /// Call once at the end: it decides whether the run saw enough to have
    /// checked anything, which cannot be known until it is over.
    pub fn finish(&mut self) {
        // Every reel has to have been seen coming to rest, or the invariant
        // that matters most was never evaluated for it.
        // A spin watched from turning to rest is the unit of evidence here.
        // Without one, neither invariant was ever evaluated.
        if self.spinning == 0 || self.resolutions == 0 {
            self.faults.push(MotionFault::NothingObserved {
                spinning: self.spinning,
                settled: self.resolutions as usize,
            });
        }
    }

    pub fn report(&self) -> String {
        if self.faults.is_empty() {
            return format!(
                "motion audit: clean over {} frames ({} in flight, {} landings compared, {} to rest)",
                self.frame, self.spinning, self.settled_seen, self.resolutions
            );
        }
        let mut out = String::new();
        for fault in &self.faults {
            out.push_str(&format!("motion audit: {}\n", fault.describe()));
        }
        out.push_str(&format!("motion audit: {} findings", self.faults.len()));
        out
    }
}

#[cfg(test)]
mod tests;
