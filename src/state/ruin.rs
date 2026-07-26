//! Running out, and what the cabinet does about it (§5.53).
//!
//! # The most likely thing that can happen to a player, and the game had no
//! # answer for it
//!
//! Twenty paylines at the cheapest stake is twenty credits a spin against a
//! thousand to start with: fifty spins, at a measured 96% return with a slot
//! machine's variance behind it. Going broke is not an edge case here. It is the
//! ordinary end of a session, and until now the whole of the game's response was
//! a notification reading *"Not enough credits — lower the bet or start a new
//! game"*.
//!
//! Lowering the bet buys a few more spins. Starting a new game throws away the
//! session, the hoard meter, the graph and everything on the way to a hatch.
//! Fifty-two systems, and the single most common way to reach the end of play
//! was a dead end with a reset button at the bottom of it.
//!
//! # What a broke player still owns
//!
//! The hoard. Eggs collected toward a hatch, and a pot banked behind them, which
//! is real value the player earned and which is worth nothing while they cannot
//! spin. So the first lifeline is **breaking the hoard**: take the banked pot
//! now, at a cut, and lose the eggs.
//!
//! That is a genuine decision rather than a free rescue. The pot is what the
//! hatch would have paid; taking it early costs the salvage rate and every egg
//! collected toward the meter. A player near a full hoard should hold on, and a
//! player who has just hatched has nothing to break.
//!
//! # And when there is nothing left to break
//!
//! The vault stakes them. This is play money and refusing to let someone keep
//! playing is worse than the alternative — but a stipend that appears from
//! nowhere and is never mentioned again would quietly make every figure in the
//! Ledger a lie. So a stake is **counted**, and the count is shown.
//!
//! # Where the money comes from, for the books
//!
//! §5.33's harness asserts `opening + won - wagered == balance` after every
//! round, and both lifelines move the balance without a spin happening.
//!
//! Breaking the hoard is not new money: the pot is funded by eggs at the line
//! bet and would have been paid out at the hatch as winnings, so a salvage is a
//! win taken early and small. It is counted as one, and the books balance
//! without a new term.
//!
//! A vault stake genuinely is new money, so it gets its own counter and its own
//! term in the identity. It is the only credit in this game that was not won.

use super::GameSession;
use crate::data::GameData;

/// A way out for a player who cannot afford a spin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifeline {
    /// Take the banked pot now, at a cut, and lose the eggs behind it.
    BreakHoard {
        /// What the player receives.
        credits: i64,
        /// What it would have been worth left alone.
        pot: i64,
        /// Eggs given up.
        eggs: u32,
    },
    /// The vault advances a stake, because there is nothing left to break.
    VaultStake { credits: i64 },
}

impl Lifeline {
    pub fn credits(self) -> i64 {
        match self {
            Lifeline::BreakHoard { credits, .. } | Lifeline::VaultStake { credits } => credits,
        }
    }
}

impl GameSession {
    /// The cheapest spin this cabinet sells.
    ///
    /// Not the current bet: a player at the top of the ladder is not out of
    /// credits while they can still drop to the bottom of it, and offering a
    /// rescue to someone with twenty playable spins left would be absurd.
    /// The least a spin can cost *as the player has the machine set up*.
    ///
    /// The ante is part of it (§5.75). This asked `total_bet` and so ignored the
    /// side bet entirely, which put the player in a state the game had no name
    /// for: on Tidepool with the ante on, twenty credits and a minimum stake of
    /// twenty-four, `can_spin` was false and `is_ruined` was false. No ruin
    /// screen, no lifeline, no explanation — just a Spin button that did
    /// nothing, and one way out that the game never mentioned. Found by ten
    /// thousand random presses (§5.76) on the fourth seed.
    pub fn cheapest_spin(&self, data: &GameData) -> i64 {
        let cheapest = data.config.line_bets.iter().copied().min().unwrap_or(1);
        data.staked(cheapest, self.ante(data))
    }

    /// Is the player actually stuck?
    ///
    /// Deliberately narrow. Free spins cost nothing, so a player mid-feature
    /// with an empty balance is not stuck; nor is one with a bonus board open,
    /// a respin round running or a gamble in flight, because all three are
    /// about to pay. Offering a rescue to any of them would be the game
    /// panicking on the player's behalf.
    pub fn is_ruined(&self, data: &GameData) -> bool {
        self.balance < self.cheapest_spin(data)
            && self.free_spins.is_none()
            && self.bonus.is_none()
            && self.holdspin.is_none()
            && self.gamble.is_none()
            && self.is_settled()
    }

    /// What the cabinet will offer, given what the player has left.
    ///
    /// `None` when they are not stuck. The hoard comes first because it is the
    /// player's own money and spending it is their decision; the vault only
    /// appears when there is nothing of theirs left to spend.
    pub fn lifeline(&self, data: &GameData) -> Option<Lifeline> {
        if !self.is_ruined(data) {
            return None;
        }
        // Offered only when it actually buys a spin. A pot of forty at half
        // salvage is twenty credits against a twenty-credit spin — take it and
        // the eggs are gone and the player is stuck again one press later,
        // which is a worse outcome than never being offered it.
        let salvage = self.hoard_salvage(data);
        if salvage >= self.cheapest_spin(data) {
            return Some(Lifeline::BreakHoard {
                credits: salvage,
                pot: self.hoard.pot,
                eggs: self.hoard.count,
            });
        }
        Some(Lifeline::VaultStake {
            credits: data.config.vault_stake,
        })
    }

    /// What breaking the hoard would pay, after the vault's cut.
    ///
    /// Whether that is *enough* is [`lifeline`](Self::lifeline)'s question:
    /// a salvage smaller than a spin is not a rescue.
    pub fn hoard_salvage(&self, data: &GameData) -> i64 {
        self.hoard.pot * data.config.hoard_salvage_permille as i64 / 1000
    }

    /// Take whichever lifeline is on offer.
    ///
    /// Returns what was taken, or `None` if the player was not stuck — so a
    /// stale button press from a frame where the balance had already changed
    /// cannot mint credits.
    pub fn take_lifeline(&mut self, data: &GameData) -> Option<Lifeline> {
        let lifeline = self.lifeline(data)?;
        match lifeline {
            Lifeline::BreakHoard { credits, .. } => {
                self.hoard.count = 0;
                self.hoard.pot = 0;
                self.balance += credits;
                // Counted as winnings, because that is what it is: the pot
                // would have been paid at the hatch, and the books
                // (§5.33) balance on `opening + won - wagered`.
                self.stats.total_won += credits;
                self.stats.hoards_broken += 1;
            }
            Lifeline::VaultStake { credits } => {
                self.balance += credits;
                // *Not* winnings. The only credit in this game that nobody won,
                // so it gets its own term or every figure downstream is wrong.
                self.stats.vault_stakes += 1;
                self.stats.staked += credits;
            }
        }
        Some(lifeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameSession;

    fn data() -> GameData {
        GameData::load().unwrap()
    }

    fn broke(data: &GameData) -> GameSession {
        let mut session = GameSession::new(data, 0xB0_1E);
        session.balance = 0;
        session
    }

    #[test]
    fn a_player_who_can_still_afford_the_cheapest_spin_is_not_stuck() {
        let data = data();
        let mut session = broke(&data);
        session.balance = session.cheapest_spin(&data);
        assert!(!session.is_ruined(&data));
        assert_eq!(session.lifeline(&data), None);

        // One credit short of it, and they are.
        session.balance -= 1;
        assert!(session.is_ruined(&data));
    }

    /// The narrowness is the point: every one of these is about to pay, and
    /// rescuing them would be the game panicking on the player's behalf.
    #[test]
    fn a_player_mid_feature_is_never_stuck() {
        let data = data();
        let mut session = broke(&data);
        assert!(session.is_ruined(&data), "the empty case, for contrast");

        session.free_spins = Some(crate::state::FreeSpinState {
            remaining: 3,
            awarded: 8,
            line_bet: 1,
            total_won: 0,
            burned: 0,
            multiplier: 0,
        });
        assert!(
            !session.is_ruined(&data),
            "free spins cost nothing, so an empty balance is not stuck"
        );
    }

    #[test]
    fn the_hoard_is_offered_before_the_vault() {
        let data = data();
        let mut session = broke(&data);
        session.hoard.count = 6;
        session.hoard.pot = 400;

        match session.lifeline(&data) {
            Some(Lifeline::BreakHoard { credits, pot, eggs }) => {
                assert_eq!(pot, 400);
                assert_eq!(eggs, 6);
                assert!(credits > 0 && credits < pot, "the cut has to bite");
            }
            other => panic!("expected the hoard to be offered, got {:?}", other),
        }
    }

    #[test]
    fn an_empty_hoard_falls_through_to_the_vault() {
        let data = data();
        let session = broke(&data);
        assert!(matches!(
            session.lifeline(&data),
            Some(Lifeline::VaultStake { .. })
        ));
    }

    /// The books, for the lifeline that is a win.
    #[test]
    fn breaking_the_hoard_is_counted_as_winnings() {
        let data = data();
        let mut session = broke(&data);
        session.hoard.count = 9;
        session.hoard.pot = 1_000;
        let before = session.stats.total_won;

        let taken = session.take_lifeline(&data).expect("stuck, so offered");
        assert_eq!(session.balance, taken.credits());
        assert_eq!(session.stats.total_won, before + taken.credits());
        assert_eq!(session.hoard.count, 0);
        assert_eq!(session.hoard.pot, 0);
        assert_eq!(session.stats.staked, 0, "nothing was minted");
    }

    /// And for the one that is not.
    #[test]
    fn a_vault_stake_is_never_counted_as_winnings() {
        let data = data();
        let mut session = broke(&data);
        let before = session.stats.total_won;

        let taken = session.take_lifeline(&data).expect("stuck, so offered");
        assert_eq!(session.stats.total_won, before, "nobody won this");
        assert_eq!(session.stats.staked, taken.credits());
        assert_eq!(session.stats.vault_stakes, 1);
        assert!(
            session.balance >= session.cheapest_spin(&data),
            "a stake that does not buy a spin has not rescued anyone"
        );
    }

    /// A stale press from a frame where the balance had already changed must
    /// not mint credits.
    #[test]
    fn taking_a_lifeline_that_is_not_on_offer_does_nothing() {
        let data = data();
        let mut session = GameSession::new(&data, 0xB0_1E);
        let before = session.balance;
        assert_eq!(session.take_lifeline(&data), None);
        assert_eq!(session.balance, before);
    }

    /// The books, over a session that actually goes broke.
    ///
    /// §5.33's harness floats the balance before every spin so it never runs
    /// out, which means it has never once exercised a lifeline. The identity it
    /// checks is `opening + won - wagered == balance`; a vault stake is the one
    /// credit in this game that is neither, so the identity gains a term and
    /// this is the only place it is checked.
    #[test]
    fn the_books_balance_across_being_rescued() {
        let data = data();
        let mut session = GameSession::new(&data, 0x_B00C);
        let opening = session.balance;
        let mut rescued = 0;

        for _ in 0..3_000 {
            // A card holds the session unsettled, and `is_ruined` deliberately
            // waits for it — a rescue offered over the top of a Hatch card
            // would be the game interrupting its own good news. The player
            // dismisses them; here the loop does.
            session.celebrations.clear();
            if session.take_lifeline(&data).is_some() {
                rescued += 1;
            }
            if session.spin(&data).is_err() {
                // Not stuck and not spinnable means the bet is above the
                // balance; drop to the floor and carry on.
                session.line_bet_index = 0;
                continue;
            }
        }

        assert!(
            rescued > 0,
            "3,000 spins from {} credits without going broke once — this test is              not exercising what it claims to",
            opening
        );
        assert_eq!(
            session.balance,
            opening + session.stats.total_won + session.stats.staked - session.stats.total_wagered,
            "opening {} + won {} + staked {} - wagered {} against a balance of {}",
            opening,
            session.stats.total_won,
            session.stats.staked,
            session.stats.total_wagered,
            session.balance
        );
        assert!(session.balance >= 0, "a rescued player still went negative");
    }

    /// Whatever else it does, it has to leave them able to press spin.
    #[test]
    fn every_lifeline_buys_at_least_one_spin() {
        let data = data();
        for pot in [0, 40, 400, 4_000] {
            let mut session = broke(&data);
            session.hoard.count = 3;
            session.hoard.pot = pot;
            session.take_lifeline(&data).expect("stuck, so offered");
            assert!(
                session.balance >= session.cheapest_spin(&data),
                "a pot of {} left the player on {} against a {} spin",
                pot,
                session.balance,
                session.cheapest_spin(&data)
            );
        }
    }
}
