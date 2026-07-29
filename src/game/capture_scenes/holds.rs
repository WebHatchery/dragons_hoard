//! Holding a moment worth photographing.
//!
//! [`begin_capture_scene`](Game::begin_capture_scene) names the scenes; this is
//! the machinery that gets the game into them. Every one of these searches or
//! forces a fixed-seed session into a state that would otherwise take a person
//! sitting at the cabinet to reach.
//!
//! Split off when the scene file crossed 800 lines (§5.73). The dividing line
//! is a real one rather than an arithmetic one: next door, a `match` on a scene
//! name that reads like a list of pictures; here, the awkward business of making
//! each picture happen.

use super::Game;
use crate::state::GameSession;

impl Game {
    /// Put the **longest** hint on the bar, and make it due.
    ///
    /// Photographing whichever hint happened to be first due measured the wrong
    /// thing twice over (§5.73). The bar is a fixed 30px strip and the longest
    /// sentence is the only one that can wrap into its own border, so that is
    /// the one worth a picture.
    ///
    /// Three things are needed and the order matters. Dismissing the others is
    /// not enough on its own — the longest hint may not have come due, and the
    /// bar then quietly falls back to the shortcut line, which is a capture that
    /// looks perfectly fine and shows nothing. Forcing the counters first is no
    /// good either: the scripted spins run afterwards and write them back, so
    /// this has to be the last thing the scene does. And the hint must be
    /// *un-earned* as well as due — the save on the machine this was written on
    /// had already played two cabinets, which had retired the hint about the
    /// cabinets before the scene began.
    pub(super) fn hold_the_longest_hint(&mut self) {
        let longest = self.hints.longest().clone();
        let others: Vec<String> = self
            .hints
            .all()
            .iter()
            .map(|def| def.id.clone())
            .filter(|id| *id != longest.id)
            .collect();
        for id in others {
            self.hints.dismiss(&id);
        }

        self.set_counter(longest.when, longest.after);
        self.set_counter(longest.earns, longest.until - 1);

        debug_assert!(
            self.hints
                .current(self.achievements.progress(), &self.ledger)
                .is_some_and(|hint| hint.id == longest.id),
            "the hint scene photographed an empty bar"
        );
    }

    /// Put a hint counter at a chosen value, whichever side of the game owns it.
    /// Raises or lowers — a scene has to be able to both reach a threshold and
    /// step back below one.
    pub(super) fn set_counter(&mut self, counter: crate::state::hints::Counter, to: i64) {
        use crate::state::hints::Counter;

        match counter {
            Counter::Spins => self.achievements.progress_mut().spins = to,
            Counter::FreeSpins => self.achievements.progress_mut().free_spins = to,
            Counter::Hatches => self.achievements.progress_mut().hatches = to,
            Counter::MachinesPlayed => {
                let played = &mut self.achievements.progress_mut().machines_played;
                played.truncate(to.max(0) as usize);
                while (played.len() as i64) < to {
                    let next = played.len();
                    played.push(crate::data::MACHINES[next].id.to_string());
                }
            }
            Counter::Gambles => self.hints.progress_mut().gambles = to,
            Counter::Buys => self.hints.progress_mut().buys = to,
            Counter::LedgerOpened => self.hints.progress_mut().ledger_opened = to,
            Counter::RulesOpened => self.hints.progress_mut().rules_opened = to,
            // The ledger counts these, and rebuilding one to order would be a
            // second, worse ledger. No hint watches wins on both sides, and the
            // assertion above catches it the day one does.
            Counter::Wins => {}
        }
    }

    /// Freeze a cascading spin part-way through its chain (§5.15).
    ///
    /// Searches for a chain of at least three grids so the capture shows a
    /// multiplier above x1, then steps to the second collapse.
    pub(super) fn hold_a_cascade(&mut self) {
        for _ in 0..4_000 {
            self.session.balance = 1_000_000;
            // A card holds `update_spin`, and so does an open Vault Pick board
            // (§8.2.1) — clearing only the first left this searching behind a
            // bonus that never closed, so the `cascade` capture photographed a
            // chest board for six iterations rather than a cascade (§5.61).
            self.session.celebrations.clear();
            self.session.bonus = None;
            if self.session.begin_spin(&self.data).is_err() {
                break;
            }
            if self.session.pending_cascade_len() >= 3 {
                // Wait for the chain to be *running* and to have taken a step.
                // Asking `map_or(usize::MAX, ..)` for "not cascading yet" and
                // then testing `>= 1` returned on the very first frame, and the
                // capture photographed a spin that had not landed.
                for _ in 0..2_000 {
                    if self
                        .session
                        .phase
                        .cascade()
                        .is_some_and(|reveal| reveal.step() >= 1)
                    {
                        return;
                    }
                    // A card holds `update_spin` outright (§8.2.1), so the reels
                    // would never advance.
                    self.session.celebrations.clear();
                    self.session.bonus = None;
                    self.session.update_spin(&self.data, 1.0 / 60.0);
                }
                return;
            }
            for _ in 0..3_000 {
                if self.session.phase.is_idle() {
                    break;
                }
                self.session.celebrations.clear();
                self.session.bonus = None;
                self.session.update_spin(&self.data, 1.0 / 60.0);
            }
        }
    }

    /// Stop on an open Dragon's Wrath board, part-way through (§5.12).
    ///
    /// The trigger is roughly one spin in two thousand, so this searches rather
    /// than waits, then takes a few respins so the capture shows a board in play
    /// instead of the five eggs it opened with.
    pub(super) fn hold_a_wrath_round(&mut self) {
        for _ in 0..200_000 {
            self.session.balance = self.data.config.starting_balance;
            self.session.celebrations.clear();
            if self.session.spin_leaving_bonus(&self.data).is_err() {
                break;
            }
            // The hoard can fill on the same grid; resolve it so the respin board
            // is what the capture is actually of.
            self.session.auto_play_bonus(&self.data);

            // A seam opened by the same grid would put its banner over the
            // board this scene is a picture of.
            self.session.auto_play_seam(&self.data);

            if self.session.holdspin.is_some() {
                self.session.celebrations.clear();
                for _ in 0..3 {
                    if self.session.holdspin.is_none() {
                        break;
                    }
                    self.session.update_spin(&self.data, 2.0);
                }
                return;
            }
        }
    }

    /// Stop on an open Seam (§5.80).
    ///
    /// `running` takes a rite and one beat of it; otherwise the board is left
    /// frozen on the choice (§5.81). Two pictures because they are two screens:
    /// the choice has buttons and is what the touch and contrast audits need to
    /// measure, and the running banner is what the feature actually looks like.
    pub(super) fn hold_a_seam(&mut self, running: bool) {
        for _ in 0..200_000 {
            self.session.balance = self.data.config.starting_balance;
            self.session.celebrations.clear();
            if self.session.spin_leaving_bonus(&self.data).is_err() {
                break;
            }
            // The same grid can fill the hoard or wake the dragon; both hold the
            // reel window in front of the seam, so they are resolved first.
            self.session.auto_play_bonus(&self.data);
            self.session.auto_play_holdspin(&self.data);

            if self.session.seam.is_none() {
                continue;
            }
            self.session.celebrations.clear();
            if !running {
                return;
            }
            // The widening, because it is the rite that visibly moves symbols.
            // One beat and only one: a two-step rite would finish and the scene
            // would photograph the base game again.
            self.session.choose_rite(0);
            self.session.update_spin(&self.data, 2.0);
            if self.session.seam.is_some() {
                return;
            }
        }
    }

    /// Find a spin that raises anticipation (§5.11) and freeze it at the moment
    /// the held-back reels are the only ones still turning — the whole point of
    /// the effect, and impossible to photograph any other way, since it lasts
    /// under a second and depends on where the scatters happen to fall.
    ///
    /// Near-misses are common enough that the search terminates quickly, but the
    /// bound is here so a machine tuned without scatters can't hang the harness.
    pub(super) fn hold_a_near_miss(&mut self) {
        for _ in 0..2_000 {
            self.session.balance = self.data.config.starting_balance;
            self.session.celebrations.clear();
            if self.session.begin_spin(&self.data).is_err() {
                break;
            }

            let anticipates = self.session.phase.spinner().is_some_and(|spinner| {
                (0..self.data.config.reel_count).any(|r| spinner.is_held(r))
            });

            if anticipates {
                // Run on until only the stretched reels remain in flight. That
                // is the held breath: most of the board decided, the one that
                // matters still moving.
                for _ in 0..1_200 {
                    let Some(spinner) = self.session.phase.spinner() else {
                        break;
                    };
                    let moving: Vec<usize> = (0..self.data.config.reel_count)
                        .filter(|reel| spinner.is_moving(*reel))
                        .collect();
                    if !moving.is_empty()
                        && moving
                            .iter()
                            .all(|reel| self.session.phase.spinner().unwrap().is_held(*reel))
                    {
                        return;
                    }
                    let _ = self.session.update_spin(&self.data, 1.0 / 60.0);
                }
                return;
            }

            // Not this one — settle it and deal again.
            for _ in 0..1_200 {
                if self.session.phase.is_idle() {
                    break;
                }
                let _ = self.session.update_spin(&self.data, 1.0 / 60.0);
            }
        }
    }

    /// Spin headlessly, topping the balance up, until `reached` holds. Cards
    /// raised by earlier spins are cleared each time, so a scene keyed on a card
    /// always lands on one the *last* spin produced. Bounded so a capture can
    /// never hang on an unreachable state.
    pub(super) fn fast_forward_to(&mut self, reached: impl Fn(&GameSession) -> bool) {
        for _ in 0..20_000 {
            self.session.balance = self.data.config.starting_balance;
            self.session.celebrations.clear();
            // A scene may want to catch a board mid-round, so settle without the
            // headless auto-play and let the predicate look first.
            let Ok(resolution) = self.session.spin_leaving_bonus(&self.data) else {
                break;
            };
            // The headless path skips `report_spin`, so record here too — a
            // capture of the achievements panel should show real progress
            // rather than a column of zeroes.
            self.achievements
                .observe(self.data.machine_id(), &resolution, self.session.balance);

            if reached(&self.session) {
                return;
            }

            // A Dragon's Wrath round is not waiting on anyone, so it is resolved
            // rather than left open — without this a fast-forward stalls the
            // moment a clutch of eggs lands.
            if self.session.auto_play_holdspin(&self.data).is_some() && reached(&self.session) {
                return;
            }

            // And a seam, for the same reason (§5.80): it advances itself, so
            // leaving one open stalls every scene that is looking for something
            // else.
            if self.session.auto_play_seam(&self.data).is_some() && reached(&self.session) {
                return;
            }

            // Nothing wanted the open board, so play it out — and look again,
            // because finishing a board is what raises the Hatch card. Checking
            // only before this is what left the `hatch` scene spinning 20,000
            // times and photographing nothing.
            if self.session.auto_play_bonus(&self.data).is_some() && reached(&self.session) {
                return;
            }
        }
    }
}
