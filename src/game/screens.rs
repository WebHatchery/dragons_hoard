//! Every screen that holds the game, and the promise that each can be audited
//! (§5.50).
//!
//! # The oldest open debt in this project
//!
//! §5.37 built the layout audit and ended by naming what it did not cover: "the
//! gamble, the Vault Pick board and the respin round draw from session state
//! rather than a flag and are not yet in the scene." That was twelve iterations
//! ago. Three modal screens a player certainly sees have never been through the
//! layout, contrast, collision or touch checks, because adding a scene was
//! always something to do next time.
//!
//! Three more scenes would close it and would go stale the same way: the debt was
//! never that three screens were missing, it was that **nothing said they were**.
//!
//! # The registry is the source, not a copy of it
//!
//! So the list moves here and `Game::any_overlay_open` is derived from it. A
//! screen that holds the game *must* appear in [`Screen::ALL`], because that is
//! now what holding the game means — and appearing there is what the audit
//! enumerates. A new modal state cannot be added without becoming auditable; it
//! would not hold the reels at all.
//!
//! That is the same move §5.49 made for saves: replace a list that has to be
//! maintained with a property that cannot be forgotten.
//!
//! # Two kinds of screen
//!
//! Most are a flag the player raises and lowers. Three are **session state** —
//! an open Vault Pick, a gamble in flight, a respin round — which is why they
//! were skipped: there is no boolean to set, and reaching them means playing
//! until the game deals one. [`Screen::reachable_by_flag`] is the difference,
//! and it is why the capture harness has to fast-forward for the other three.

use super::Game;

/// Everything that takes over the screen or holds the reels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Paytable,
    Rules,
    Limits,
    History,
    RealityCheck,
    Settings,
    Machines,
    Achievements,
    FeatureBuy,
    Ledger,
    /// The way in to everything else, for anyone without a keyboard (§5.72).
    Menu,
    /// The payline diagrams (§5.60).
    Lines,
    /// The log of sessions played (§5.70).
    Sessions,
    Waveforms,
    Vision,
    /// The Vault Pick board (§5.10). Waits on the player, so it holds the reels.
    Bonus,
    /// A win at risk (§5.16).
    Gamble,
    /// The Dragon's Wrath respin round (§5.12). Advances itself on a beat, but
    /// the reels do not turn while it runs.
    Holdspin,
    /// A cap the player set has ended the session (§5.68).
    SessionOver,
    /// Out of credits (§5.53). Dealt by the balance rather than opened, and it
    /// holds the game in the strongest sense there is — the reels cannot turn
    /// until it is answered.
    Ruin,
}

impl Screen {
    pub const ALL: [Screen; 20] = [
        Screen::Paytable,
        Screen::Rules,
        Screen::Limits,
        Screen::History,
        Screen::RealityCheck,
        Screen::Settings,
        Screen::Machines,
        Screen::Achievements,
        Screen::FeatureBuy,
        Screen::Ledger,
        Screen::Menu,
        Screen::Lines,
        Screen::Sessions,
        Screen::Waveforms,
        Screen::Vision,
        Screen::Bonus,
        Screen::Gamble,
        Screen::Holdspin,
        Screen::Ruin,
        Screen::SessionOver,
    ];

    /// What the capture harness calls it.
    pub fn id(self) -> &'static str {
        match self {
            Screen::Paytable => "paytable",
            Screen::Rules => "rules",
            Screen::Limits => "limits",
            Screen::History => "history",
            Screen::RealityCheck => "reality",
            Screen::Settings => "settings",
            Screen::Machines => "machines",
            Screen::Achievements => "achievements",
            Screen::FeatureBuy => "featurebuy",
            Screen::Ledger => "ledger",
            Screen::Menu => "menu",
            Screen::Lines => "lines",
            Screen::Sessions => "sessions",
            Screen::Waveforms => "waveforms",
            Screen::Vision => "vision",
            Screen::Bonus => "bonus",
            Screen::Gamble => "gamble",
            Screen::Holdspin => "wrath",
            Screen::Ruin => "ruin",
            Screen::SessionOver => "sessionover",
        }
    }

    /// What a menu calls it.
    ///
    /// Separate from [`id`](Self::id), which is what the capture harness asks
    /// for and must never change; this is what a player reads and may. The
    /// dealt screens have one too even though nothing offers them, because the
    /// alternative is an `Option` that every caller has to think about for the
    /// sake of five variants nobody lists.
    pub fn label(self) -> &'static str {
        match self {
            Screen::Menu => "Everything else",
            Screen::Paytable => "Paytable",
            Screen::Rules => "How this cabinet plays",
            Screen::Limits => "Session limits",
            Screen::History => "This session's graph",
            Screen::RealityCheck => "Reality check",
            Screen::Settings => "Settings",
            Screen::Machines => "Machines",
            Screen::Achievements => "Awards",
            Screen::FeatureBuy => "Buy a feature",
            Screen::Ledger => "The Ledger",
            Screen::Lines => "The paylines",
            Screen::Sessions => "Your sessions",
            Screen::Waveforms => "Sound",
            Screen::Vision => "Colour vision",
            Screen::Bonus => "The Vault Pick",
            Screen::Gamble => "The Gamble",
            Screen::Holdspin => "The Dragon's Wrath",
            Screen::Ruin => "Out of credits",
            Screen::SessionOver => "That is the session",
        }
    }

    /// Screens a menu should offer, in the order it should offer them.
    ///
    /// Everything a player can open, minus the menu itself and minus the ones
    /// the game deals rather than the player opens (§5.72). Derived from
    /// [`ALL`](Self::ALL) rather than listed, so a screen added to the registry
    /// is reachable without a keyboard the moment it exists — which four of them
    /// were not, including the colour-vision panel.
    pub fn in_menu() -> impl Iterator<Item = Screen> {
        Self::ALL
            .into_iter()
            .filter(|screen| screen.reachable_by_flag() && *screen != Screen::Menu)
    }

    /// Can it be opened by setting a flag?
    ///
    /// The three that cannot are the three that went unaudited for twelve
    /// iterations, and this is exactly why: reaching them means playing until
    /// the game deals one.
    pub fn reachable_by_flag(self) -> bool {
        !matches!(
            self,
            Screen::Bonus | Screen::Gamble | Screen::Holdspin | Screen::Ruin | Screen::SessionOver
        )
    }
}

impl Game {
    /// Is this screen up?
    pub(super) fn screen_open(&self, screen: Screen) -> bool {
        match screen {
            Screen::Paytable => self.show_paytable,
            Screen::Rules => self.show_rules,
            Screen::Limits => self.show_limits,
            Screen::History => self.show_history,
            Screen::RealityCheck => self.reality_check,
            Screen::Settings => self.show_settings,
            Screen::Machines => self.show_machines,
            Screen::Achievements => self.show_achievements,
            Screen::FeatureBuy => self.show_featurebuy,
            Screen::Ledger => self.show_ledger,
            Screen::Menu => self.show_menu,
            Screen::Lines => self.show_lines,
            Screen::Sessions => self.show_sessions,
            Screen::Waveforms => self.show_waveforms,
            Screen::Vision => self.show_vision,
            Screen::Bonus => self.session.bonus.is_some(),
            Screen::Gamble => self.session.gamble.is_some(),
            Screen::Holdspin => self.session.holdspin.is_some(),
            Screen::Ruin => self.session.is_ruined(&self.data),
            Screen::SessionOver => self.limits.breach().is_some() && !self.session_over_dismissed,
        }
    }

    /// The flag behind a screen, for the twelve that have one.
    ///
    /// [`Screen::reachable_by_flag`] is the gate rather than a description of
    /// one, so the two cannot drift: a screen declared dealt has no flag here
    /// even if someone writes an arm for it, and a screen declared flagged that
    /// has no arm will not compile.
    fn screen_flag(&mut self, screen: Screen) -> Option<&mut bool> {
        if !screen.reachable_by_flag() {
            return None;
        }
        Some(match screen {
            Screen::Paytable => &mut self.show_paytable,
            Screen::Rules => &mut self.show_rules,
            Screen::Limits => &mut self.show_limits,
            Screen::History => &mut self.show_history,
            Screen::RealityCheck => &mut self.reality_check,
            Screen::Settings => &mut self.show_settings,
            Screen::Machines => &mut self.show_machines,
            Screen::Achievements => &mut self.show_achievements,
            Screen::FeatureBuy => &mut self.show_featurebuy,
            Screen::Ledger => &mut self.show_ledger,
            Screen::Menu => &mut self.show_menu,
            Screen::Lines => &mut self.show_lines,
            Screen::Sessions => &mut self.show_sessions,
            Screen::Waveforms => &mut self.show_waveforms,
            Screen::Vision => &mut self.show_vision,
            // Excluded by the gate above; the predicate is the one place that
            // decides, and the compiler holds this arm to it.
            Screen::Bonus
            | Screen::Gamble
            | Screen::Holdspin
            | Screen::Ruin
            | Screen::SessionOver => unreachable!(),
        })
    }

    /// Raise a screen that has a flag. The other three are dealt, not opened.
    pub(super) fn open_screen(&mut self, screen: Screen) -> bool {
        match self.screen_flag(screen) {
            Some(flag) => {
                *flag = true;
                true
            }
            None => false,
        }
    }

    /// Lower every screen that has a flag.
    pub(super) fn close_screens(&mut self) {
        for screen in Screen::ALL {
            if let Some(flag) = self.screen_flag(screen) {
                *flag = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_screen_has_a_name_and_no_two_share_one() {
        // The name is what the capture harness asks for, so a duplicate would
        // silently audit one screen twice and the other never.
        let mut seen: Vec<&str> = Vec::new();
        for screen in Screen::ALL {
            assert!(!screen.id().is_empty());
            assert!(!seen.contains(&screen.id()), "{} twice", screen.id());
            seen.push(screen.id());
        }
        assert_eq!(seen.len(), Screen::ALL.len());
    }

    /// That a screen actually *opened* is checked where it can be: the
    /// `audit:<screen>` capture scene asserts it before measuring anything, and
    /// `verify.ps1` runs all fifteen. `Game` needs a GL context, so there is no
    /// useful unit test here — and the runtime check is the better one anyway,
    /// since it exercises the path the audit really takes.
    ///
    /// The three that went unaudited, named as a property rather than a note.
    #[test]
    fn exactly_the_dealt_screens_have_no_flag() {
        let dealt: Vec<&str> = Screen::ALL
            .iter()
            .filter(|s| !s.reachable_by_flag())
            .map(|s| s.id())
            .collect();
        assert_eq!(
            dealt,
            vec!["bonus", "gamble", "wrath", "ruin", "sessionover"]
        );
    }
}
