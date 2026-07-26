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

use crate::data::{Evaluation, GameData};
use crate::state::{bonus, holdspin, jackpot};

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

/// Everything this cabinet does, in the order a player meets it.
///
/// How a win is read first, because nothing else means anything without it;
/// then what is on the reels, then what the reels can open, then the two things
/// the player chooses to do rather than has done to them.
pub fn rules(data: &GameData) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut add = |topic: Topic, title: &str, text: String| {
        rules.push(Rule {
            topic,
            title: title.to_owned(),
            text,
        })
    };

    match data.config.evaluation {
        Evaluation::Lines => add(
            Topic::Lines,
            "Paylines",
            format!(
                "Wins pay left to right from reel 1 along {} fixed lines. Each line pays its best \
                 reading once, and every line is bet on every spin — the total stake is {} times \
                 the line bet.",
                data.paylines.len(),
                data.config.bet_units.unwrap_or(data.paylines.len()),
            ),
        ),
        Evaluation::Cluster => add(
            Topic::Clusters,
            "Clusters",
            format!(
                concat!(
                    "There are no lines, and no reading across the reels. {} or more of the ",
                    "same symbol touching each other — up, down, left or right, never ",
                    "diagonally — pay as one group, anywhere on the grid. One big ",
                    "group is worth far more than two small ones."
                ),
                crate::engine::cluster::MIN_CLUSTER,
            ),
        ),
        Evaluation::Ways => add(
            Topic::Ways,
            "Ways",
            match data.ways_count() {
                Some(ways) => format!(
                    "There are no lines. A symbol pays if it lands anywhere on each reel starting \
                     from reel 1, and every path through those positions is paid — {} of them on a \
                     full grid. Several symbols can pay at once.",
                    ways
                ),
                // A shifting cabinet has no fixed count to quote, which is the
                // whole point of it; §5.20 says why.
                None => "There are no lines. A symbol pays if it lands anywhere on each reel \
                         starting from reel 1, and every path through those positions is paid, so \
                         a taller grid is worth more. Several symbols can pay at once."
                    .to_owned(),
            },
        ),
    }

    if let Some(heights) = data.config.reel_heights {
        add(
            Topic::ShiftingReels,
            "Shifting reels",
            format!(
                "Every reel is dealt a fresh height of {} to {} rows each spin. Taller reels carry \
                 more symbols and multiply the paths through them, so the size of the win is \
                 settled before the reels stop.",
                heights.min, heights.max,
            ),
        );
    }

    if let Some(cascade) = &data.cascade {
        let ladder = cascade
            .multipliers
            .iter()
            .map(|step| format!("x{}", step))
            .collect::<Vec<_>>()
            .join(", ");
        add(
            Topic::Cascades,
            "Cascades",
            format!(
                "Winning symbols are removed and the ones above fall into the gaps, refilling from \
                 the top. If the new grid wins, it happens again — up to {} times in one spin. The \
                 chain multiplies as it runs: {}. Everything the chain pays belongs to the one \
                 spin that started it.",
                cascade.max_steps, ladder,
            ),
        );
    }

    if let Some(wild) = data.symbols.wild() {
        add(
            Topic::Wild,
            "Wilds",
            format!(
                "The {} substitutes for any paying symbol{}. It never stands in for the {}, which \
                 has to land on its own.",
                data.symbols.get(wild).name,
                if data.freespins.expanding_wilds {
                    " and fills its whole reel during free spins"
                } else {
                    ""
                },
                data.symbols
                    .scatter()
                    .map(|index| data.symbols.get(index).name.clone())
                    .unwrap_or_else(|| "scatter".to_owned()),
            ),
        );
    }

    if let Some(scatter) = data.symbols.scatter() {
        add(
            Topic::Scatter,
            "Scatters",
            format!(
                "The {} pays from anywhere on the grid — it does not have to line up, and it is \
                 what opens the free spins.",
                data.symbols.get(scatter).name,
            ),
        );
    }

    if let Some(rule) = free_spins_rule(data) {
        add(rule.topic, &rule.title, rule.text);
    }

    if let Some(refine) = &data.freespins.refine {
        let burned = refine
            .order
            .iter()
            .filter_map(|id| data.symbols.index_of(id))
            .map(|index| data.symbols.get(index).name.clone())
            .collect::<Vec<_>>()
            .join(", then the ");
        add(
            Topic::Refining,
            "Refining",
            format!(
                "The free spins get better as they run. Each one burns the next symbol off every \
                 reel for good — the {} — so by the last spin the strips hold only what pays well.",
                burned,
            ),
        );
    }

    if data.freespins.shapes.len() > 1 {
        let named: Vec<String> = data
            .freespins
            .shapes
            .iter()
            .map(|shape| {
                format!(
                    "{}% of them at x{}",
                    shape.spin_permille / 10,
                    shape.multiplier
                )
            })
            .collect();
        add(
            Topic::FreeSpinShapes,
            "Long or short",
            format!(
                "You choose how the run goes before the first free spin: {}. Both are worth the \
                 same to the credit, so the choice is how the return arrives rather than how much \
                 of it there is — the short one pays nothing more often, and far more when it does.",
                named.join(", or "),
            ),
        );
    }

    if data.config.hoard_capacity > 0 {
        add(
            Topic::Hoard,
            "The Hoard",
            format!(
                "Hoard symbols fill the meter beside the reels. At {} it hatches and deals a board \
                 of {} chests: keep picking until {} come up empty, and every prize is a share of \
                 the hoard. A full board is worth about {:.0}% of what the meter holds.",
                data.config.hoard_capacity,
                data.bonus.board_size,
                data.bonus.blanks,
                bonus::expected_permille(&data.bonus) / 10.0,
            ),
        );
    }

    if data.jackpots.contribution_permille > 0 {
        // A pot fed by machines the player is not sitting at is a genuinely
        // surprising rule, so it is said outright (§5.57) — but as a sentence
        // rather than a heading of its own. The panel is two columns of a fixed
        // height and a fifth topic pushed Frost Wyrm into a third, which is a
        // gate §5.29 put there for exactly this.
        let shared: Vec<&str> = data
            .jackpots
            .tiers
            .iter()
            .filter(|tier| tier.shared)
            .map(|tier| tier.name.as_str())
            .collect();
        let floor = if shared.is_empty() {
            String::new()
        } else {
            format!(
                " The {} is shared by every cabinet.",
                shared.join(" and the ")
            )
        };
        add(
            Topic::Jackpots,
            "Progressives",
            format!(
                "{}% of every stake feeds the four pots, which can pay at random on any paid spin. \
                 A bigger stake wins them proportionally more often, so the return per credit is \
                 the same at every bet — worth {:.1}% of all play.{}",
                data.jackpots.contribution_permille as f32 / 10.0,
                jackpot::expected_rtp(&data.jackpots) * 100.0,
                floor,
            ),
        );
    }

    if data.holdspin.trigger_eggs > 0 {
        add(
            Topic::Wrath,
            "The Dragon's Wrath",
            format!(
                "{} dragon eggs on one grid wake the dragon. The eggs lock as coins worth {:.1}x \
                 the total bet on average and you get {} respins; every coin that lands restores \
                 them in full, so the round only ends when nothing does. Fill all {} cells for {}x \
                 the total bet on top.",
                data.holdspin.trigger_eggs,
                holdspin::mean_coin_multiple(&data.holdspin),
                data.holdspin.respins,
                data.config.reel_count * data.config.row_count,
                data.holdspin.full_board_multiple,
            ),
        );
    }

    if data.gamble.max_steps > 0 {
        add(
            Topic::Gamble,
            "The Gamble",
            format!(
                "Any win can be staked on a coin: double it or lose it, up to {} times in a row{}. \
                 The odds are exactly even, so gambling changes nothing about what the machine \
                 returns over time — only how far it swings. Wins above {}x the total bet cannot \
                 be gambled.",
                data.gamble.max_steps,
                if data.gamble.allow_half {
                    ", or risk half and keep the rest"
                } else {
                    ""
                },
                data.gamble.ceiling_multiple,
            ),
        );
    }

    if !data.featurebuy.tiers.is_empty() {
        let cheapest = data
            .featurebuy
            .tiers
            .iter()
            .map(|tier| tier.price_multiple)
            .min()
            .unwrap_or(0);
        add(
            Topic::FeatureBuy,
            "Feature Buy",
            format!(
                "You can pay for a feature outright rather than wait for it — {} of them, from {}x \
                 the total bet. Each price is set from what the feature is actually worth, at the \
                 same {:.1}% return the cabinet pays anyway, so buying is neither a shortcut nor a \
                 tax. It only trades waiting for volatility.",
                data.featurebuy.tiers.len(),
                cheapest,
                data.featurebuy.target_rtp_permille as f64 / 10.0,
            ),
        );
    }

    rules
}

/// The free spins paragraph, which has to read an award table of arbitrary size.
fn free_spins_rule(data: &GameData) -> Option<Rule> {
    let mut awards: Vec<(usize, u32)> = data
        .freespins
        .awards
        .iter()
        .filter(|(_, spins)| **spins > 0)
        .filter_map(|(count, spins)| Some((count.parse::<usize>().ok()?, *spins)))
        .collect();
    if awards.is_empty() {
        return None;
    }
    awards.sort_unstable();

    let table = awards
        .iter()
        // Which number is which has to be unmistakable: "5 for 6, 6 for 10"
        // reads as either, and both readings are plausible.
        .map(|(count, spins)| format!("{} scatters award {}", count, spins))
        .collect::<Vec<_>>()
        .join(", ");

    let multiplier = if data.freespins.multiplier > 1 {
        format!(
            " Everything they pay is multiplied by {}.",
            data.freespins.multiplier
        )
    } else {
        String::new()
    };
    let retrigger = if data.freespins.retrigger {
        " Landing the scatters again during the feature adds more."
    } else {
        " They cannot be retriggered."
    };

    Some(Rule {
        topic: Topic::FreeSpins,
        title: "Free Spins".to_owned(),
        text: format!(
            "Free spins are awarded by the scatters: {}.{}{} A free spin costs \
             nothing and is part of the round that bought it, at the same bet.",
            table, multiplier, retrigger,
        ),
    })
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
            let Some(rule) = free_spins_rule(&data) else {
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
    const MECHANICS: &[(&str, Topic)] = &[
        ("shapes", Topic::FreeSpinShapes),
        ("refine", Topic::Refining),
        ("retrigger", Topic::FreeSpins),
        ("multiplier", Topic::FreeSpins),
        ("awards", Topic::FreeSpins),
        ("expanding_wilds", Topic::Wild),
    ];

    /// Settings that are numbers rather than rules, and why each is excused.
    ///
    /// Every entry is a claim that a player does not need to be told this to
    /// understand the game. They are listed rather than pattern-matched so that
    /// the claim is visible.
    const TUNING: &[(&str, &str)] = &[];

    #[test]
    fn every_free_spin_setting_is_a_mechanic_or_excused_tuning() {
        for machine in MACHINES {
            let data = GameData::load_machine(machine).unwrap();
            let value = serde_json::to_value(&data.freespins).unwrap();
            let described: Vec<Topic> = rules(&data).iter().map(|rule| rule.topic).collect();

            for (key, setting) in value.as_object().expect("freespins is an object") {
                if TUNING.iter().any(|(name, _)| name == key) {
                    continue;
                }
                let Some((_, topic)) = MECHANICS.iter().find(|(name, _)| name == key) else {
                    panic!(
                        "{}: freespins.json carries '{}' and nothing here says whether it is a \
                         mechanic that must be explained or tuning that need not be. Add it to \
                         MECHANICS or to TUNING with a reason — silence is how §5.64 shipped a \
                         choice the game never mentioned.",
                        machine.id, key
                    );
                };
                if !is_on(setting) {
                    continue;
                }
                assert!(
                    described.contains(topic),
                    "{}: '{}' is set and {:?} explains nothing",
                    machine.id,
                    key,
                    topic
                );
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
