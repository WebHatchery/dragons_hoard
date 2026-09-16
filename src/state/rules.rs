//! What this cabinet actually does, derived from what it is (§5.29).
//!
//! # A paytable that described a game we stopped shipping
//!
//! The rules the paytable showed were four hardcoded paragraphs written when
//! there were two cabinets and both played the same way. Since then Emberfall
//! grew 243 ways, Avalanche grew cascades (§5.15), Wyrmspire grew reels that
//! change height every spin (§5.20), and Frost grew free spins that burn symbols
//! off the strips as they run (§5.21). The paragraphs never changed.
//!
//! So a player on Avalanche watched symbols vanish from the grid with nothing
//! anywhere in the game to say why. Three of five cabinets ran mechanics the
//! game never mentioned, and the gamble and the buy menu were never explained at
//! all — on any of them.
//!
//! # Prose is not the fix; the invariant is
//!
//! Writing five paragraphs would work until the sixth cabinet, which is exactly
//! how this happened the first time. The problem is not that the text was wrong,
//! it is that nothing could *tell* it was wrong.
//!
//! So the config is read twice, by two functions that do not share code:
//!
//! - [`Topic::present`] asks which mechanics a cabinet **has**, from config
//!   presence alone — a `cascade.json` exists, `reel_heights` is set, the award
//!   table is not empty.
//! - [`rules`] produces the prose that **explains** them.
//!
//! A test asserts the two agree, for every machine. Add a cabinet with a
//! mechanic and forget to describe it and the build fails naming the topic —
//! which is the failure that should have fired three iterations ago and could
//! not, because there was nothing to compare the prose against.
//!
//! Deriving the prose from config also settles the numbers. Every figure below
//! is read from the same data the engine pays out of, so a rule cannot quote a
//! trigger of three scatters at a cabinet that wants four.

pub mod prose;
pub use prose::rules;

use crate::data::{Evaluation, GameData};

/// A mechanic a cabinet may have. Closed on purpose: a new variant forces a
/// decision in both [`Topic::present`] and [`rules`], and the test between them
/// makes skipping either one a build failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    /// Fixed paths across the grid (§3).
    Lines,
    /// Any path from reel one (§5.14).
    Ways,
    /// Connected groups anywhere on the grid (§5.35).
    Clusters,
    /// Reels that change height every spin (§5.20).
    ShiftingReels,
    /// Winning symbols leave and the grid refills (§5.15).
    Cascades,
    Wild,
    Scatter,
    FreeSpins,
    /// The free spins run long and shallow or short and sharp (§5.64).
    FreeSpinShapes,
    /// Symbols burned off the strips as the feature runs (§5.21).
    Refining,
    /// The hoard meter and the Vault Pick it opens (§5.10).
    Hoard,
    Jackpots,
    /// The Dragon's Wrath hold-and-spin round (§5.12).
    Wrath,
    /// The Seam mini-game a board full of one treasure opens (§5.80), and the
    /// rite the player picks for it (§5.81).
    ///
    /// One topic and not two, though the choice is the more interesting half.
    /// A second topic would earn a second block in the rules panel, and that
    /// panel is at its limit — four columns and an 11px floor. What holds the
    /// prose to naming the player instead is
    /// `the_seam_rule_says_whose_decision_it_is`, which is a narrower check than
    /// a topic and a more direct one.
    Seam,
    Gamble,
    FeatureBuy,
    /// The side bet that buys a better chance at the feature (§5.75).
    Ante,
}

impl Topic {
    /// Which mechanics this cabinet has, from config presence alone.
    ///
    /// Deliberately dumb, and deliberately written without reference to the
    /// prose below: this is the half of the pair that the descriptions are
    /// checked *against*, so it earns its keep by being the one that cannot
    /// drift. Everything here is a field existing or a collection being
    /// non-empty.
    pub fn present(data: &GameData) -> Vec<Topic> {
        let mut topics = Vec::new();

        topics.push(match data.config.evaluation {
            Evaluation::Lines => Topic::Lines,
            Evaluation::Ways => Topic::Ways,
            Evaluation::Cluster => Topic::Clusters,
        });
        if data.config.reel_heights.is_some() {
            topics.push(Topic::ShiftingReels);
        }
        if data.cascade.is_some() {
            topics.push(Topic::Cascades);
        }
        if data.symbols.wild().is_some() {
            topics.push(Topic::Wild);
        }
        if data.symbols.scatter().is_some() {
            topics.push(Topic::Scatter);
        }
        if data.freespins.awards.values().any(|spins| *spins > 0) {
            topics.push(Topic::FreeSpins);
        }
        if !data.freespins.shapes.is_empty() {
            topics.push(Topic::FreeSpinShapes);
        }
        if data.ante().is_some() {
            topics.push(Topic::Ante);
        }
        if data.freespins.refine.is_some() {
            topics.push(Topic::Refining);
        }
        if data.config.hoard_capacity > 0 {
            topics.push(Topic::Hoard);
        }
        if data.jackpots.contribution_permille > 0 {
            topics.push(Topic::Jackpots);
        }
        if data.holdspin.trigger_eggs > 0 {
            topics.push(Topic::Wrath);
        }
        if data.seam.trigger_count > 0 && !data.seam.rites.is_empty() {
            topics.push(Topic::Seam);
        }
        if data.gamble.max_steps > 0 {
            topics.push(Topic::Gamble);
        }
        if !data.featurebuy.tiers.is_empty() {
            topics.push(Topic::FeatureBuy);
        }
        topics
    }
}

/// Check that this cabinet explains everything it does.
///
/// Runs when a cabinet is loaded rather than only under test, because the
/// machine list is data: a `cascade.json` dropped into a new directory gives
/// that cabinet cascades without a line of Rust changing, and the player would
/// be the one to find out it was never described. Failing at the point the
/// machine is picked up puts it in front of whoever added it.
pub fn validate(data: &GameData) -> Result<(), String> {
    let described: Vec<Topic> = rules(data).iter().map(|rule| rule.topic).collect();
    for topic in Topic::present(data) {
        if !described.contains(&topic) {
            return Err(format!(
                "{} has {:?} and nothing explains it",
                data.machine.id, topic
            ));
        }
    }
    Ok(())
}

/// One explained mechanic.
#[derive(Debug, Clone)]
pub struct Rule {
    pub topic: Topic,
    pub title: String,
    pub text: String,
}

// Tests live in the crate-level integration harness.

// Coverage tests live in the crate-level integration harness.
