//! The sentences themselves.
//!
//! # Why this is not in `rules`
//!
//! Next door is the *machinery*: which mechanics a cabinet has, the check that
//! every one of them is described, and the coverage table that makes a new
//! setting fail the build until someone declares it (§5.66, §5.67). This is the
//! prose those checks are run against — three hundred lines of it, and growing
//! by a paragraph every time the game learns to do something.
//!
//! Splitting them keeps the file under the limit (§5.69) and puts the two halves
//! where they belong: the rules about rules on one side, the writing on the
//! other. Every figure here is still read from the same data the engine pays out
//! of, which is the point §5.29 was built on — a rule cannot quote a trigger of
//! three scatters at a cabinet that wants four.

use super::{Rule, Topic};
use crate::data::{Evaluation, GameData};
use crate::state::{bonus, holdspin, jackpot};

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

    if let Some(ante) = data.ante() {
        add(
            Topic::Ante,
            "The Ante",
            format!(
                "The ante stakes {:.2}x the usual bet and weaves extra scatters into the \
                 first reel, so the free spins arrive about twice as often. Its price was \
                 measured from what those features are worth, so the return is unchanged. \
                 It buys a shorter wait, not an edge.",
                ante.cost_permille as f64 / 1000.0,
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
pub(super) fn free_spins_rule(data: &GameData) -> Option<Rule> {
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
