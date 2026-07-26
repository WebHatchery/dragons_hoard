//! Capture-scene wiring for the headless screenshot harness.
//!
//! Every scene here fast-forwards a fresh, fixed-seed session into a state worth
//! photographing, then hands back to the normal loop. None of it runs in a real
//! session — it exists so a UI change can be verified without a human sitting at
//! the cabinet pulling the lever until the right thing happens.

use super::Game;
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
    /// Swap the cabinet a scene is shot on.
    ///
    /// Not just an assignment: a cabinet brings its palette with it (§5.43), and
    /// six scenes setting `self.data` directly would each have to remember that.
    /// Always called with `machine_by_id`, never with an index (§5.61).
    ///
    /// The `cascade` scene asked for `MACHINES[3]` and got **Wyrmspire**, which
    /// has no cascades at all — so `hold_a_cascade` searched four thousand
    /// spins for a chain that could never come and the capture photographed a
    /// board doing nothing. Six iterations, with `ui_cascade` listed among this
    /// game's verification captures the whole time. A position in an array is
    /// not a name, and the six cabinets are not in the order anyone assumes.
    fn use_machine(&mut self, machine: &'static crate::data::MachineDef) {
        self.data = crate::data::GameData::load_machine(machine)
            .unwrap_or_else(|err| panic!("{}: {}", machine.id, err));
        crate::ui::theme::set(crate::ui::theme::by_name(self.data.theme_name()));
    }

    pub fn begin_capture_scene(&mut self, scene: &str) {
        // What the game found on the disk when it started (§5.58) — reported
        // before the line below throws it away, which is the whole reason this
        // arm is up here and not with the others. Prints rather than
        // photographs: the fault was that boot never read the save at all, and
        // no picture of a reel window shows that.
        if scene == "boot_report" {
            println!(
                "boot eggs {} pot {} spins {} mini_milli {}",
                self.session.hoard.count,
                self.session.hoard.pot,
                self.session.stats.total_spins,
                self.session.jackpots.accrued_milli(0)
            );
            std::process::exit(0);
        }

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
            "lines" => self.show_lines = true,
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
                self.use_machine(crate::data::machine_by_id("avalanche"));
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.hold_a_cascade();
            }
            "shifting" => {
                // The shifting cabinet (§5.20), on a paying spin — a resting
                // board is where the varying reel heights actually read.
                self.use_machine(crate::data::machine_by_id("wyrmspire"));
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.fast_forward_to(|session| session.last_win > 0);
            }
            "ways" => {
                // The 243-ways cabinet (§5.14). Fast-forwarded to a win, because
                // a resting board says nothing about how differently it pays.
                self.use_machine(crate::data::machine_by_id("ways"));
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.fast_forward_to(|session| session.last_win > 0);
            }
            "refining" => {
                // Frost Wyrm mid-feature, a few spins in, so the banner names
                // what has already been burned off the strips (§5.21).
                self.use_machine(crate::data::machine_by_id("frost"));
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
                self.use_machine(crate::data::machine_by_id("frost"));
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.fast_forward_to(|session| session.last_win > 0);
            }
            "wrath" => self.hold_a_wrath_round(),
            // Out of credits with a hoard worth breaking (§5.53). Both halves
            // matter: an empty hoard shows the vault offer instead, and the two
            // read very differently.
            // Not a scene: the registry, printed, so a harness never has to
            // keep its own copy of the list (§5.53). `verify.ps1` had one and it
            // was already stale — the ruin screen was registered, tested and
            // reachable, and the sweep did not know it existed.
            // Walk from one cabinet to another with money on the clock and
            // print what survived (§5.55). Not a picture — a check that the
            // wallet travels and the hoard does not, run against the real
            // switch path rather than a reconstruction of it.
            "wallet_walk" => {
                self.session.balance = 4_321;
                self.session.hoard.count = 7;
                self.session.hoard.pot = 555;
                println!(
                    "before walk balance {} eggs {} on {}",
                    self.session.balance,
                    self.session.hoard.count,
                    self.data.machine_id()
                );
                let target = crate::data::MACHINES
                    .iter()
                    .position(|m| m.id != self.data.machine_id())
                    .unwrap_or(0);
                self.switch_machine(target);
                println!(
                    "after  walk balance {} eggs {} on {}",
                    self.session.balance,
                    self.session.hoard.count,
                    self.data.machine_id()
                );
                // The claim, checked rather than printed: money is the
                // player's and travels; the hoard is the cabinet's and does
                // not (§5.55).
                assert_eq!(
                    self.session.balance, 4_321,
                    "the balance did not survive walking to another cabinet"
                );
                std::process::exit(0);
            }
            "screens" => {
                for screen in crate::game::screens::Screen::ALL {
                    println!("screen {}", screen.id());
                }
                std::process::exit(0);
            }
            "ruin" => {
                // Set rather than played into: the pot a random session happens
                // to reach is whatever it is, and this capture is about how the
                // offer reads with real money in it.
                self.session.hoard.count = 9;
                self.session.hoard.pot = 1_240;
                self.session.balance = 0;
            }
            // And the other half: nothing left to break.
            "ruin_vault" => {
                self.session.balance = 0;
                self.session.hoard.count = 0;
                self.session.hoard.pot = 0;
            }
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
            // Two cabinets, because the panel's whole claim is that it
            // describes the one in front of the player (§5.29): Avalanche
            // cascades and Frost refines, and neither used to be mentioned
            // anywhere in the game.
            "history" => {
                // A real session: long enough to have a shape, and started from
                // a bankroll the player could plausibly lose, so the graph shows
                // the grind rather than a flat line at a million (§5.32).
                self.history.clear();
                self.session.balance = 20_000;
                for _ in 0..600 {
                    self.session.celebrations.clear();
                    if self.session.balance < 1_000 {
                        break;
                    }
                    if self.session.spin(&self.data).is_err() {
                        break;
                    }
                    self.drain_finished_rounds();
                    // The marks come from the cards the round raised, which is
                    // where the interactive game takes them from too. Drained by
                    // stepping the queue rather than read off it, since that is
                    // the only way a card reports itself as opened.
                    while let Some(card) = self.session.celebrations.update(9.0) {
                        self.celebrate(&card);
                    }
                }
                // The cards fired their bursts on the way through. In the
                // game they would have faded long before the panel opened.
                self.particles.clear();
                self.session.celebrations.clear();
                self.show_history = true;
            }
            // The same panel after enough rounds to decimate, which is the only
            // state where the band between bucket extremes is visible.
            "history_long" => {
                self.history.clear();
                for _ in 0..4_000 {
                    self.session.balance = 1_000_000;
                    self.session.celebrations.clear();
                    if self.session.spin(&self.data).is_err() {
                        break;
                    }
                    self.drain_finished_rounds();
                    while let Some(card) = self.session.celebrations.update(9.0) {
                        self.celebrate(&card);
                    }
                }
                self.particles.clear();
                self.session.celebrations.clear();
                self.show_history = true;
            }
            "cluster" => {
                self.use_machine(crate::data::machine_by_id("tidepool"));
                self.session = self.load_machine_session();
                self.session.balance = 1_000_000;
                for _ in 0..40 {
                    self.session.celebrations.clear();
                    if self.session.spin(&self.data).is_err() {
                        break;
                    }
                    if self
                        .session
                        .last_outcome
                        .as_ref()
                        .is_some_and(|outcome| outcome.wins.len() >= 2)
                    {
                        break;
                    }
                }
                self.particles.clear();
                self.session.celebrations.clear();
            }
            // Open everything at once and let the frame report what did not
            // fit (§5.37). Overlays draw in a fixed order, so one frame with
            // every flag set exercises every panel's text.
            "layout_audit" => {
                self.session.balance = 1_987_654_321;
                self.session.stats.biggest_win = 987_654_321;
                self.session.hoard.pot = 87_654_321;
                self.show_paytable = true;
                self.show_rules = true;
                self.show_settings = true;
                self.show_machines = true;
                self.show_achievements = true;
                self.show_featurebuy = true;
                self.show_ledger = true;
                self.show_limits = true;
                self.show_history = true;
                self.show_waveforms = true;
                self.show_vision = true;
                self.reality_check = true;
                // DRAGONS_HOARD_TEXT_SCALE lets the audit run at every size the
                // settings panel offers (§5.38) without a scene per size.
                if let Ok(scale) = std::env::var("DRAGONS_HOARD_TEXT_SCALE") {
                    if let Ok(scale) = scale.parse::<f32>() {
                        macroquad_toolkit::ui::set_ui_text_scale(scale);
                    }
                }
                // DRAGONS_HOARD_THEME runs the audit under any cabinet's palette
                // without a scene per theme (§5.43). Every colour pairing has to
                // survive every room.
                if let Ok(name) = std::env::var("DRAGONS_HOARD_THEME") {
                    crate::ui::theme::set(crate::ui::theme::by_name(&name));
                }
                macroquad_toolkit::ui::begin_audit();
            }
            // Touch targets are measured on **one** screen at a time. The
            // layout audit opens every overlay at once, which is right for
            // finding every control's size and wrong for finding overlaps: two
            // controls in two panels that are never open together are not
            // ambiguous, they are in different rooms (§5.45).
            "touch_audit" => {
                macroquad_toolkit::ui::begin_target_audit();
                // Collisions are a one-screen question too (§5.47).
                macroquad_toolkit::ui::begin_audit();
                macroquad_toolkit::ui::begin_collision_audit();
            }
            "touch_audit_settings" => {
                self.show_settings = true;
                macroquad_toolkit::ui::begin_target_audit();
                // Collisions are a one-screen question too (§5.47).
                macroquad_toolkit::ui::begin_audit();
                macroquad_toolkit::ui::begin_collision_audit();
            }
            "touch_audit_buy" => {
                self.show_featurebuy = true;
                macroquad_toolkit::ui::begin_target_audit();
                // Collisions are a one-screen question too (§5.47).
                macroquad_toolkit::ui::begin_audit();
                macroquad_toolkit::ui::begin_collision_audit();
            }
            // One audit scene per screen, named from the registry (§5.50). The
            // three that are dealt rather than opened reuse the scenes that
            // already knew how to reach them, which is why they can finally be
            // audited at all.
            // A spin, watched frame by frame rather than photographed at the
            // end (§5.52). `motion:<cabinet>` so every reel behaviour in the
            // catalog gets looked at — cascades, shifting rows and clusters all
            // land differently.
            scene if scene.starts_with("motion:") => {
                let wanted = &scene["motion:".len()..];
                if let Some(machine) = crate::data::MACHINES
                    .iter()
                    .find(|machine| machine.id == wanted)
                {
                    self.use_machine(machine);
                    self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                } else {
                    panic!("no cabinet called '{}'", wanted);
                }
                self.begin_motion_audit();
                let _ = self.session.begin_spin(&self.data);
            }
            scene if scene.starts_with("audit:") => {
                let wanted = &scene["audit:".len()..];
                let Some(screen) = crate::game::screens::Screen::ALL
                    .iter()
                    .find(|s| s.id() == wanted)
                    .copied()
                else {
                    panic!("no screen called '{}'", wanted);
                };
                if !self.open_screen(screen) {
                    // Dealt, not opened: play until the game produces one.
                    self.begin_capture_scene(screen.id());
                }
                // The dispatcher below ends in `_ => {}`, so asking for a scene
                // that does not exist does nothing at all and the audit then
                // reports the *base game* as clean. That is not a hypothetical:
                // it is indistinguishable from the twelve iterations these three
                // screens spent unaudited. So the registry has to prove it
                // arrived, not assume it (§5.50).
                assert!(
                    self.screen_open(screen),
                    "audit:{} did not reach the screen — the capture scene named \
                     '{}' is missing or no longer opens it, and auditing from \
                     here would measure the wrong screen and pass",
                    wanted,
                    screen.id()
                );
                macroquad_toolkit::ui::begin_audit();
                macroquad_toolkit::ui::begin_collision_audit();
                macroquad_toolkit::ui::begin_target_audit();
            }
            "rules" => self.show_rules = true,
            "limits" => {
                // One cap tightened and one loosened, so the panel shows both
                // states it can be in at once (§5.30).
                self.limits
                    .request(crate::state::limits::Cap::Loss, Some(5_000));
                self.limits
                    .request(crate::state::limits::Cap::Spins, Some(100));
                self.limits
                    .request(crate::state::limits::Cap::Spins, Some(500));
                self.show_limits = true;
            }
            "reality" => {
                // A session with something in it to report: down on the day,
                // which is where a real one usually is.
                for _ in 0..180 {
                    self.session.balance = 1_000_000;
                    self.session.celebrations.clear();
                    if self.session.spin(&self.data).is_err() {
                        break;
                    }
                    self.drain_finished_rounds();
                }
                self.limits.clock.tick(23.0 * 60.0 + 40.0);
                self.reality_check = true;
            }
            "rules_avalanche" => {
                self.use_machine(crate::data::machine_by_id("avalanche"));
                self.show_rules = true;
            }
            "rules_frost" => {
                self.use_machine(crate::data::machine_by_id("frost"));
                self.show_rules = true;
            }
            "waveforms" => self.show_waveforms = true,
            "vision" => self.show_vision = true,
            "hint" => {
                // Enough play that the first hint has come due (§5.28). It
                // arrives because the player has won a few times and never
                // gambled — not because the game just loaded.
                self.hints = crate::state::hints::HintBook::load(&self.data.config).unwrap();
                for _ in 0..60 {
                    self.session.balance = 1_000_000;
                    self.session.celebrations.clear();
                    if self.session.spin(&self.data).is_err() {
                        break;
                    }
                    self.drain_finished_rounds();
                    self.achievements.observe(
                        self.data.machine_id(),
                        &self.session.spin(&self.data).unwrap(),
                        self.session.balance,
                    );
                }
                self.session.celebrations.clear();
            }
            "keyboard" => {
                // An open Vault Pick with the keyboard driving. Before §5.27
                // this board could only be cleared with a mouse, and it holds
                // the game — so a keyboard-only player was stuck for good.
                self.fast_forward_to(|session| session.bonus.is_some());
                self.session.celebrations.clear();
                // Past the header and wager controls, onto a chest.
                self.nav.pin(17);
            }
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
