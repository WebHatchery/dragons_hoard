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

// Tests live in the crate-level integration harness.
