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

mod prose;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MACHINES;

    fn every_machine() -> Vec<GameData> {
        MACHINES
            .iter()
            .map(|machine| GameData::load_machine(machine).unwrap())
            .collect()
    }

    /// The point of the whole module.
    ///
    /// Before §5.29 the rules were prose with nothing to check them against, and
    /// three cabinets shipped mechanics the game never mentioned. This is the
    /// assertion that could not be made then.
    #[test]
    fn every_mechanic_a_cabinet_has_is_explained() {
        for data in every_machine() {
            let described: Vec<Topic> = rules(&data).iter().map(|rule| rule.topic).collect();
            for topic in Topic::present(&data) {
                assert!(
                    described.contains(&topic),
                    "{} has {:?} and never says so",
                    data.machine.id,
                    topic
                );
            }
        }
    }

    /// And the other direction: a rule describing something the cabinet does not
    /// do is worse than no rule, because the player has no way to tell.
    #[test]
    fn nothing_is_explained_that_the_cabinet_does_not_do() {
        for data in every_machine() {
            let present = Topic::present(&data);
            for rule in rules(&data) {
                assert!(
                    present.contains(&rule.topic),
                    "{} explains {:?}, which it does not have",
                    data.machine.id,
                    rule.topic
                );
            }
        }
    }

    #[test]
    fn a_cabinet_is_never_told_it_has_both_win_models() {
        for data in every_machine() {
            let topics = Topic::present(&data);
            let models = [Topic::Lines, Topic::Ways, Topic::Clusters]
                .iter()
                .filter(|topic| topics.contains(topic))
                .count();
            assert_eq!(
                models, 1,
                "{} claims {} win models",
                data.machine.id, models
            );
        }
    }

    #[test]
    fn the_cabinets_that_differ_are_described_differently() {
        // Five cabinets sharing one paragraph is how this went wrong. If two
        // machines produce identical text, one of them is being lied to.
        let rendered: Vec<String> = every_machine()
            .iter()
            .map(|data| {
                rules(data)
                    .iter()
                    .map(|rule| rule.text.clone())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .collect();

        for (i, left) in rendered.iter().enumerate() {
            for right in rendered.iter().skip(i + 1) {
                assert_ne!(left, right);
            }
        }
    }

    #[test]
    fn cascades_are_only_described_where_they_happen() {
        for data in every_machine() {
            let mentions = rules(&data)
                .iter()
                .any(|rule| rule.topic == Topic::Cascades);
            assert_eq!(mentions, data.cascade.is_some(), "{}", data.machine.id);
        }
    }

    #[test]
    fn shifting_reels_quote_the_range_the_engine_deals() {
        for data in every_machine() {
            let Some(heights) = data.config.reel_heights else {
                continue;
            };
            let rule = rules(&data)
                .into_iter()
                .find(|rule| rule.topic == Topic::ShiftingReels)
                .expect("no shifting-reel rule");
            assert!(rule.text.contains(&heights.min.to_string()));
            assert!(rule.text.contains(&heights.max.to_string()));
        }
    }

    #[test]
    fn the_free_spin_trigger_matches_the_award_table() {
        // A rule quoting three scatters at a cabinet that wants four is the
        // exact failure mode data-derived prose exists to remove.
        for data in every_machine() {
            let Some(rule) = prose::free_spins_rule(&data) else {
                continue;
            };
            let trigger = data.freespins.trigger_count();
            assert!(
                rule.text.contains(&format!("{} scatters award", trigger)),
                "{} triggers at {} and the rule does not say so",
                data.machine.id,
                trigger
            );
        }
    }

    #[test]
    fn every_rule_says_something_and_is_readable() {
        for data in every_machine() {
            let rules = rules(&data);
            assert!(
                rules.len() >= 6,
                "{} explains almost nothing",
                data.machine.id
            );
            for rule in rules {
                assert!(!rule.title.trim().is_empty());
                // Long enough to be an explanation, short enough to be read.
                assert!(rule.text.len() > 60, "{:?} is a stub", rule.topic);
                assert!(rule.text.len() < 420, "{:?} is an essay", rule.topic);
                assert!(rule.text.trim_end().ends_with('.'));
            }
        }
    }

    #[test]
    fn no_rule_quotes_an_empty_list() {
        // The refining and cascade rules build their text by joining a config
        // list. An id that no longer resolves would leave a dangling "the ," in
        // the middle of a sentence rather than fail anything.
        for data in every_machine() {
            for rule in rules(&data) {
                assert!(!rule.text.contains("the ,"), "{:?}", rule.topic);
                assert!(!rule.text.contains("  "), "{:?}", rule.topic);
                assert!(!rule.text.contains(" ."), "{:?}", rule.topic);
            }
        }
    }
}

/// Whether a setting in the data can exist without anyone explaining it (§5.66).
///
/// # The gate that did not fire
///
/// This module opens by promising that "add a cabinet with a mechanic and forget
/// to describe it and the build fails naming the topic". §5.64 added a mechanic
/// — the free-spin run can be traded short and sharp — and **nothing failed**.
/// The game shipped a decision it never mentioned.
///
/// The reason is that [`Topic::present`] is a hand-written list of things to
/// look for. It compares the prose against a set of topics; it cannot notice a
/// mechanic nobody added a topic for. The same shape as §5.50's screen registry
/// and §5.53's copy of it in the harness: a list that must be maintained is a
/// list that goes stale, and staleness looks exactly like correctness.
///
/// So the question is turned round. Every key in the feature data is either
/// **a mechanic**, which must be named by a topic that is present when the key
/// is on, or **tuning**, which is excused here in writing. A key that is neither
/// fails the test. Adding a field to `freespins.json` now forces a decision
/// rather than permitting silence — the default is "explain this", and getting
/// out of it means saying why in a place someone reviews.
#[cfg(test)]
mod coverage {
    use super::*;
    use crate::data::{GameData, MACHINES};

    /// Feature settings that are a mechanic, and the topic that must cover them
    /// whenever they are switched on.
    /// Feature settings that are a mechanic, keyed by the block they live in
    /// and the topic that must cover them whenever they are switched on.
    ///
    /// Keyed by **block and name**, not name alone. `tiers` means a jackpot
    /// ladder in one file and a buy menu in another; `max_steps` is a gamble
    /// ladder here and a cascade chain there. The first version of this table
    /// used the bare key and passed — for the wrong reason, because both topics
    /// happen to be present on every cabinet. A check that is right by
    /// coincidence is a check that will be wrong the moment a cabinet drops one
    /// of them.
    const MECHANICS: &[(&str, &str, Topic)] = &[
        ("freespins", "shapes", Topic::FreeSpinShapes),
        ("freespins", "ante", Topic::Ante),
        ("freespins", "refine", Topic::Refining),
        ("freespins", "retrigger", Topic::FreeSpins),
        ("freespins", "multiplier", Topic::FreeSpins),
        ("freespins", "awards", Topic::FreeSpins),
        ("freespins", "expanding_wilds", Topic::Wild),
        // The Vault Pick: how many chests, and how many duds end it.
        ("bonus", "board_size", Topic::Hoard),
        ("bonus", "blanks", Topic::Hoard),
        ("bonus", "prizes_permille", Topic::Hoard),
        // The Dragon's Wrath.
        ("holdspin", "trigger_eggs", Topic::Wrath),
        ("holdspin", "respins", Topic::Wrath),
        ("holdspin", "coin_values", Topic::Wrath),
        ("holdspin", "full_board_multiple", Topic::Wrath),
        // The gamble.
        ("gamble", "max_steps", Topic::Gamble),
        ("gamble", "ceiling_multiple", Topic::Gamble),
        ("gamble", "allow_half", Topic::Gamble),
        // Progressives, and the menu that sells the features.
        ("jackpots", "contribution_permille", Topic::Jackpots),
        ("jackpots", "tiers", Topic::Jackpots),
        ("featurebuy", "tiers", Topic::FeatureBuy),
        // Cascades.
        ("cascade", "multipliers", Topic::Cascades),
        ("cascade", "max_steps", Topic::Cascades),
    ];

    /// Settings that are numbers rather than rules, and why each is excused.
    ///
    /// Every entry is a claim that a player does not need to be told this to
    /// understand the game. They are listed rather than pattern-matched so that
    /// the claim is visible.
    const TUNING: &[(&str, &str, &str)] = &[
        // How often a coin lands on a respin. The player is told the round
        // restores its respins whenever one does, which is the rule; the rate
        // behind it is tuning and quoting it would be noise.
        (
            "holdspin",
            "coin_chance_permille",
            "the rate behind a rule the player is already told",
        ),
        // What the buy menu is priced to return. The Feature Buy rule quotes it
        // as a percentage, which is the readable form of the same number.
        (
            "featurebuy",
            "target_rtp_permille",
            "quoted in the Feature Buy rule as a percentage",
        ),
    ];

    /// Every block of feature data a cabinet carries, and what it is called in
    /// a failure message.
    fn feature_blocks(data: &GameData) -> Vec<(&'static str, serde_json::Value)> {
        let mut blocks = vec![
            ("freespins", serde_json::to_value(&data.freespins).unwrap()),
            ("bonus", serde_json::to_value(&data.bonus).unwrap()),
            ("holdspin", serde_json::to_value(&data.holdspin).unwrap()),
            ("gamble", serde_json::to_value(&data.gamble).unwrap()),
            (
                "featurebuy",
                serde_json::to_value(&data.featurebuy).unwrap(),
            ),
            ("jackpots", serde_json::to_value(&data.jackpots).unwrap()),
        ];
        if let Some(cascade) = data.cascade.as_ref() {
            blocks.push(("cascade", serde_json::to_value(cascade).unwrap()));
        }
        blocks
    }

    #[test]
    fn every_feature_setting_is_a_mechanic_or_excused_tuning() {
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let described: Vec<Topic> = rules(&data).iter().map(|rule| rule.topic).collect();

            for (block, value) in feature_blocks(&data) {
                for (key, setting) in value.as_object().expect("a feature block is an object") {
                    if TUNING
                        .iter()
                        .any(|(where_, name, _)| where_ == &block && name == key)
                    {
                        continue;
                    }
                    let Some((_, _, topic)) = MECHANICS
                        .iter()
                        .find(|(where_, name, _)| where_ == &block && name == key)
                    else {
                        panic!(
                        "{}: {}.json carries '{}' and nothing here says whether it is a mechanic \
                         that must be explained or tuning that need not be. Add it to MECHANICS \
                         or to TUNING with a reason — silence is how §5.64 shipped a choice the \
                         game never mentioned.",
                        machine.id, block, key
                    );
                    };
                    if !is_on(setting) {
                        continue;
                    }
                    assert!(
                        described.contains(topic),
                        "{}: {}.{} is set and {:?} explains nothing",
                        machine.id,
                        block,
                        key,
                        topic
                    );
                }
            }
        }
    }

    /// Is this setting switched on, in the sense of being worth a sentence?
    fn is_on(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Null => false,
            serde_json::Value::Bool(on) => *on,
            serde_json::Value::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
            serde_json::Value::String(s) => !s.is_empty(),
            serde_json::Value::Array(items) => !items.is_empty(),
            serde_json::Value::Object(fields) => !fields.is_empty(),
        }
    }
}
