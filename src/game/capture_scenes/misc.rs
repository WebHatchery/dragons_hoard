//! Capture scenes for panels, limits, hints, and accessibility checks.

use super::super::Game;
use crate::state::celebration::CelebrationKind;

impl Game {
    pub(super) fn capture_feature_card_scene(&mut self) {
        self.fast_forward_to(|session| {
            matches!(
                session.celebrations.active().map(|card| card.kind()),
                Some(CelebrationKind::FreeSpinsEntry { .. })
            )
        });
    }

    pub(super) fn capture_hatch_scene(&mut self) {
        self.fast_forward_to(|session| {
            matches!(
                session.celebrations.active().map(|card| card.kind()),
                Some(CelebrationKind::Hatch { .. })
            )
        });
    }

    pub(super) fn capture_jackpot_scene(&mut self) {
        self.fast_forward_to(|session| {
            matches!(
                session.celebrations.active().map(|card| card.kind()),
                Some(CelebrationKind::Jackpot { .. })
            )
        });
    }

    pub(super) fn capture_ruin_scene(&mut self, vault: bool) {
        self.session.balance = 0;
        if vault {
            self.session.hoard.count = 0;
            self.session.hoard.pot = 0;
        } else {
            self.session.hoard.count = 9;
            self.session.hoard.pot = 1_240;
        }
    }

    pub(super) fn capture_featurebuy_scene(&mut self) {
        self.session.balance = 500_000;
        self.show_featurebuy = true;
    }

    pub(super) fn capture_limits_scene(&mut self) {
        self.limits
            .request(crate::state::limits::Cap::Loss, Some(5_000));
        self.limits
            .request(crate::state::limits::Cap::Spins, Some(100));
        self.limits
            .request(crate::state::limits::Cap::Spins, Some(500));
        self.show_limits = true;
    }

    pub(super) fn capture_reality_scene(&mut self) {
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

    pub(super) fn capture_rules_machine_scene(&mut self, machine: Option<&str>) {
        if let Some(machine) = machine {
            self.use_machine(crate::data::machine_by_id(machine));
        }
        self.show_rules = true;
    }

    pub(super) fn capture_hint_scene(&mut self) {
        self.hints = crate::state::hints::HintBook::load(&self.data.config).unwrap();
        for _ in 0..60 {
            self.session.balance = 1_000_000;
            self.session.celebrations.clear();
            if self.session.spin(&self.data).is_err() {
                break;
            }
            self.drain_finished_rounds();
            let Ok(round) = self.session.spin(&self.data) else {
                break;
            };
            self.achievements
                .observe(self.data.machine_id(), &round, self.session.balance);
        }
        self.session.celebrations.clear();
        self.hold_the_longest_hint();
    }

    pub(super) fn capture_keyboard_scene(&mut self) {
        self.fast_forward_to(|session| session.bonus.is_some());
        self.session.celebrations.clear();
        self.nav.pin(17);
    }
}
