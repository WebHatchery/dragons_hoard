//! Capture-scene wiring for the headless screenshot harness.
//!
//! Every scene here fast-forwards a fresh, fixed-seed session into a state worth
//! photographing, then hands back to the normal loop. None of it runs in a real
//! session — it exists so a UI change can be verified without a human sitting at
//! the cabinet pulling the lever until the right thing happens.

use super::Game;
use crate::data::GameData;
use crate::state::celebration::CelebrationKind;
use crate::state::gamble::Scale;
use crate::state::GameSession;

impl Game {
    /// Fast-forward into a named state so the screenshot harness can photograph
    /// something other than the boot screen. Uses the headless spin path to skip
    /// ahead, then hands over to the normal loop.
    ///
    /// Scenes: `idle`, `spin` (reels mid-flight), `win`, `freespins`,
    /// `paytable`, `settings`, `feature_card`, `hatch`, `autospin`, `anticipation`,
    /// `wrath`.
    pub fn begin_capture_scene(&mut self, scene: &str) {
        // A fixed seed keeps every capture reproducible run to run.
        self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
        self.notifications.clear();

        match scene {
            "spin" => {
                let _ = self.session.begin_spin(&self.data);
            }
            "win" => self.fast_forward_to(|session| session.last_win > 0),
            "freespins" => {
                self.fast_forward_to(GameSession::in_free_spins);
                self.session.celebrations.clear();
                let _ = self.session.begin_spin(&self.data);
            }
            "feature_card" => self.fast_forward_to(|session| {
                matches!(
                    session.celebrations.active().map(|card| card.kind()),
                    Some(CelebrationKind::FreeSpinsEntry { .. })
                )
            }),
            "hatch" => self.fast_forward_to(|session| {
                matches!(
                    session.celebrations.active().map(|card| card.kind()),
                    Some(CelebrationKind::Hatch { .. })
                )
            }),
            "autospin" => {
                let spins = self.session.preferences.autospin_spins(&self.data.config);
                self.session.start_autospin(spins);
                let _ = self.session.begin_spin(&self.data);
            }
            "paytable" => self.show_paytable = true,
            "machines" => self.show_machines = true,
            "bonus" => {
                self.fast_forward_to(|session| session.bonus.is_some());
                self.session.celebrations.clear();
                // Turn a few over so the capture shows a board in play rather
                // than twelve closed chests.
                for index in [0usize, 1, 2, 5] {
                    self.session.pick_bonus(index, &self.data);
                }
            }
            "achievements" => {
                self.fast_forward_to(|session| session.stats.hatches > 0);
                // The hatch that got us here raised a card; the panel is the
                // subject of this capture, not the card.
                self.session.celebrations.clear();
                self.show_achievements = true;
            }
            "jackpot" => self.fast_forward_to(|session| {
                matches!(
                    session.celebrations.active().map(|card| card.kind()),
                    Some(CelebrationKind::Jackpot { .. })
                )
            }),
            "cascade" => {
                // Mid-chain, at a step where the multiplier has climbed —
                // a resting Avalanche board looks like any other cabinet.
                self.data = GameData::load_machine(&crate::data::MACHINES[3]).unwrap();
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.hold_a_cascade();
            }
            "shifting" => {
                // The shifting cabinet (§5.20), on a paying spin — a resting
                // board is where the varying reel heights actually read.
                self.data =
                    GameData::load_machine(crate::data::machine_by_id("wyrmspire")).unwrap();
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.fast_forward_to(|session| session.last_win > 0);
            }
            "ways" => {
                // The 243-ways cabinet (§5.14). Fast-forwarded to a win, because
                // a resting board says nothing about how differently it pays.
                self.data = GameData::load_machine(&crate::data::MACHINES[2]).unwrap();
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.fast_forward_to(|session| session.last_win > 0);
            }
            "refining" => {
                // Frost Wyrm mid-feature, a few spins in, so the banner names
                // what has already been burned off the strips (§5.21).
                self.data = GameData::load_machine(crate::data::machine_by_id("frost")).unwrap();
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.fast_forward_to(GameSession::in_free_spins);
                self.session.celebrations.clear();
                for _ in 0..2 {
                    if !self.session.in_free_spins() {
                        break;
                    }
                    self.session.balance = 1_000_000;
                    let _ = self.session.spin(&self.data);
                    self.session.celebrations.clear();
                }
                let _ = self.session.begin_spin(&self.data);
            }
            "frost" => {
                self.data = GameData::load_machine(&crate::data::MACHINES[1]).unwrap();
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.fast_forward_to(|session| session.last_win > 0);
            }
            "wrath" => self.hold_a_wrath_round(),
            "featurebuy" => {
                // Enough credit that every tier reads as affordable — a menu of
                // greyed-out rows would photograph the wallet, not the feature.
                self.session.balance = 500_000;
                self.show_featurebuy = true;
            }
            "gamble" => {
                // A win, staked, and one rung climbed — the panel says more
                // about the decision when there is something on the ladder.
                self.fast_forward_to(|session| session.last_win > 0);
                self.session.celebrations.clear();
                let _ = self.session.begin_gamble(&self.data);
                for _ in 0..8 {
                    if self
                        .session
                        .gamble
                        .as_ref()
                        .is_some_and(|round| round.steps() > 0)
                    {
                        break;
                    }
                    // Keep flipping until one lands; a busted round closes
                    // itself and there would be no panel to photograph.
                    if self
                        .session
                        .flip_gamble(Scale::Ember, false, &self.data)
                        .is_err()
                    {
                        self.fast_forward_to(|session| session.last_win > 0);
                        self.session.celebrations.clear();
                        let _ = self.session.begin_gamble(&self.data);
                    }
                }
            }
            "ledger" => {
                // From empty. The ledger persists to disk like preferences do,
                // so without this each capture run added to the last one and the
                // round count climbed every time — the harness is supposed to be
                // reproducible run to run.
                self.ledger = crate::state::ledger::Ledger::default();
                // A few hundred rounds of real play, so the player's bar has a
                // shape to compare against the machine's.
                for _ in 0..400 {
                    self.session.balance = 1_000_000;
                    self.session.celebrations.clear();
                    if self.session.spin(&self.data).is_err() {
                        break;
                    }
                    self.drain_finished_rounds();
                }
                self.show_ledger = true;
                self.profiles.request(self.data.machine_id(), &self.data);
            }
            "waveforms" => self.show_waveforms = true,
            "settings" => self.show_settings = true,
            "anticipation" => self.hold_a_near_miss(),
            _ => {}
        }
    }

    /// Freeze a cascading spin part-way through its chain (§5.15).
    ///
    /// Searches for a chain of at least three grids so the capture shows a
    /// multiplier above x1, then steps to the second collapse.
    fn hold_a_cascade(&mut self) {
        for _ in 0..4_000 {
            self.session.balance = 1_000_000;
            self.session.celebrations.clear();
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
                    self.session.update_spin(&self.data, 1.0 / 60.0);
                }
                return;
            }
            for _ in 0..3_000 {
                if self.session.phase.is_idle() {
                    break;
                }
                self.session.celebrations.clear();
                self.session.update_spin(&self.data, 1.0 / 60.0);
            }
        }
    }

    /// Stop on an open Dragon's Wrath board, part-way through (§5.12).
    ///
    /// The trigger is roughly one spin in two thousand, so this searches rather
    /// than waits, then takes a few respins so the capture shows a board in play
    /// instead of the five eggs it opened with.
    fn hold_a_wrath_round(&mut self) {
        for _ in 0..200_000 {
            self.session.balance = self.data.config.starting_balance;
            self.session.celebrations.clear();
            if self.session.spin_leaving_bonus(&self.data).is_err() {
                break;
            }
            // The hoard can fill on the same grid; resolve it so the respin board
            // is what the capture is actually of.
            self.session.auto_play_bonus(&self.data);

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

    /// Find a spin that raises anticipation (§5.11) and freeze it at the moment
    /// the held-back reels are the only ones still turning — the whole point of
    /// the effect, and impossible to photograph any other way, since it lasts
    /// under a second and depends on where the scatters happen to fall.
    ///
    /// Near-misses are common enough that the search terminates quickly, but the
    /// bound is here so a machine tuned without scatters can't hang the harness.
    fn hold_a_near_miss(&mut self) {
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
    fn fast_forward_to(&mut self, reached: impl Fn(&GameSession) -> bool) {
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
