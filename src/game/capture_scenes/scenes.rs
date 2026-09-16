//! Deterministic scene preparation for ordinary capture screens.

use super::super::Game;
use crate::state::gamble::Scale;
use crate::state::GameSession;

impl Game {
    pub(super) fn capture_spin_scene(&mut self) {
        let _ = self.session.begin_spin(&self.data);
    }

    pub(super) fn capture_win_scene(&mut self) {
        self.fast_forward_to(|session| session.last_win > 0);
    }

    pub(super) fn capture_freespins_scene(&mut self) {
        self.fast_forward_to(GameSession::in_free_spins);
        self.session.celebrations.clear();
        let _ = self.session.begin_spin(&self.data);
    }

    pub(super) fn capture_autospin_scene(&mut self) {
        let spins = self.session.preferences.autospin_spins(&self.data.config);
        self.session.start_autospin(spins);
        let _ = self.session.begin_spin(&self.data);
    }

    pub(super) fn capture_bonus_scene(&mut self) {
        self.fast_forward_to(|session| session.bonus.is_some());
        self.session.celebrations.clear();
        // Turn a few over so the capture shows a board in play rather than
        // twelve closed chests.
        for index in [0usize, 1, 2, 5] {
            self.session.pick_bonus(index, &self.data);
        }
    }

    pub(super) fn capture_achievements_scene(&mut self) {
        self.fast_forward_to(|session| session.stats.hatches > 0);
        self.session.auto_play_bonus(&self.data);
        self.session.celebrations.clear();
        self.show_achievements = true;
    }

    pub(super) fn capture_machine_feature_scene(&mut self, machine: &str) {
        self.use_machine(crate::data::machine_by_id(machine));
        self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
        match machine {
            "avalanche" => self.hold_a_cascade(),
            "wyrmspire" | "ways" | "frost" => {
                self.fast_forward_to(|session| session.last_win > 0);
            }
            _ => unreachable!("unsupported capture machine {machine}"),
        }
    }

    pub(super) fn capture_refining_scene(&mut self) {
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

    pub(super) fn capture_wallet_walk_scene(&mut self) {
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
            .position(|machine| machine.id != self.data.machine_id())
            .unwrap_or(0);
        self.switch_machine(target);
        println!(
            "after  walk balance {} eggs {} on {}",
            self.session.balance,
            self.session.hoard.count,
            self.data.machine_id()
        );
        assert_eq!(
            self.session.balance, 4_321,
            "the balance did not survive walking to another cabinet"
        );
        std::process::exit(0);
    }

    pub(super) fn capture_screens_scene(&mut self) {
        for screen in crate::game::screens::Screen::ALL {
            println!("screen {}", screen.id());
        }
        std::process::exit(0);
    }

    pub(super) fn capture_session_over_scene(&mut self) {
        for _ in 0..90 {
            self.session.balance = 1_000_000;
            self.session.celebrations.clear();
            let staked = self.session.total_bet(&self.data);
            let round = match self.session.spin(&self.data) {
                Ok(round) => round,
                Err(_) => break,
            };
            self.limits.clock.record(
                staked,
                round.spin_credits + round.hatch_credits + round.wrath_credits,
            );
        }
        self.limits.clock.elapsed = 25.0 * 60.0;
        let _ = self
            .limits
            .request(crate::state::limits::Cap::Time, Some(20));
        let _ = self.limits.evaluate();
    }

    pub(super) fn capture_proofs_scene(&mut self) {
        for machine in ["dragon", "tidepool"] {
            self.use_machine(crate::data::machine_by_id(machine));
            self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
            for _ in 0..8 {
                self.session.balance = 1_000_000;
                self.session.celebrations.clear();
                if self.session.spin(&self.data).is_err() {
                    break;
                }
                self.drain_finished_rounds();
            }
        }
        self.check_proofs();
        self.show_proofs = true;
    }

    pub(super) fn capture_tampered_proofs_scene(&mut self) {
        self.capture_proofs_scene();
        for (index, entry) in self.proofs.entries_mut().iter_mut().enumerate() {
            match index {
                1 => entry.win += 250,
                3 => entry.grid[0] = "wild".to_owned(),
                5 => entry.machine = "a_cabinet_that_never_shipped".to_owned(),
                _ => {}
            }
        }
        self.check_proofs();
    }

    pub(super) fn capture_sessions_scene(&mut self) {
        for run in 0..6 {
            let mut clock = crate::state::limits::SessionClock::default();
            for spin in 0..(24 + run * 17) {
                self.session.balance = 1_000_000;
                self.session.celebrations.clear();
                let staked = self.session.total_bet(&self.data);
                let Ok(round) = self.session.spin(&self.data) else {
                    break;
                };
                clock.record(
                    staked,
                    round.spin_credits + round.hatch_credits + round.wrath_credits,
                );
                let _ = spin;
            }
            clock.elapsed = 300.0 + run as f32 * 240.0;
            if run == 5 {
                self.sessions
                    .hold(&clock, self.session.stats.biggest_win, 0, None);
            } else {
                self.sessions.record(
                    &clock,
                    self.session.stats.biggest_win,
                    0,
                    (run % 2 == 0).then_some(crate::state::limits::Breach::Time(20)),
                );
            }
        }
        self.show_sessions = true;
    }

    pub(super) fn capture_gamble_scene(&mut self) {
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

    pub(super) fn capture_ledger_scene(&mut self) {
        self.ledger = crate::state::ledger::Ledger::default();
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

    pub(super) fn capture_history_scene(&mut self) {
        self.history.clear();
        self.session.balance = 20_000;
        for _ in 0..600 {
            self.session.celebrations.clear();
            if self.session.balance < 1_000 || self.session.spin(&self.data).is_err() {
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

    pub(super) fn capture_long_history_scene(&mut self) {
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

    pub(super) fn capture_cluster_scene(&mut self) {
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
}
