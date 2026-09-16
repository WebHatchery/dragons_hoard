//! The sentences themselves, rendered from the shared presentation data.

use super::{Rule, Topic};
use crate::data::{render_text, Evaluation, GameData, RiteKind, RuleText};
use crate::state::{bonus, holdspin, jackpot};

/// Everything this cabinet does, in the order a player meets it.
pub fn rules(data: &GameData) -> Vec<Rule> {
    let text = &data.presentation.rules;
    let mut rules = Vec::new();
    let mut add = |topic: Topic, title_key: &str, body: String| {
        rules.push(Rule {
            topic,
            title: text.template(title_key).to_owned(),
            text: body,
        });
    };

    match data.config.evaluation {
        Evaluation::Lines => add(
            Topic::Lines,
            "title_paylines",
            render_text(
                text.template("paylines"),
                &[
                    ("lines", data.paylines.len().to_string()),
                    (
                        "units",
                        data.config
                            .bet_units
                            .unwrap_or(data.paylines.len())
                            .to_string(),
                    ),
                ],
            ),
        ),
        Evaluation::Cluster => add(
            Topic::Clusters,
            "title_clusters",
            render_text(
                text.template("clusters"),
                &[("minimum", crate::engine::cluster::MIN_CLUSTER.to_string())],
            ),
        ),
        Evaluation::Ways => {
            let key = if data.ways_count().is_some() {
                "ways"
            } else {
                "ways_shifting"
            };
            let ways = data
                .ways_count()
                .map_or(String::new(), |ways| ways.to_string());
            add(
                Topic::Ways,
                "title_ways",
                render_text(text.template(key), &[("ways", ways)]),
            );
        }
    }

    if let Some(heights) = data.config.reel_heights {
        add(
            Topic::ShiftingReels,
            "title_shifting_reels",
            render_text(
                text.template("shifting_reels"),
                &[
                    ("min", heights.min.to_string()),
                    ("max", heights.max.to_string()),
                ],
            ),
        );
    }

    if let Some(cascade) = &data.cascade {
        let ladder = cascade
            .multipliers
            .iter()
            .map(|step| format!("x{step}"))
            .collect::<Vec<_>>()
            .join(", ");
        add(
            Topic::Cascades,
            "title_cascades",
            render_text(
                text.template("cascades"),
                &[("steps", cascade.max_steps.to_string()), ("ladder", ladder)],
            ),
        );
    }

    if let Some(wild) = data.symbols.wild() {
        let expansion_key = if data.freespins.expanding_wilds {
            "wilds_expanded"
        } else {
            "wilds_plain"
        };
        add(
            Topic::Wild,
            "title_wilds",
            render_text(
                text.template("wilds"),
                &[
                    ("wild", data.symbols.get(wild).name.clone()),
                    ("expansion", text.template(expansion_key).to_owned()),
                    (
                        "scatter",
                        data.symbols
                            .scatter()
                            .map(|index| data.symbols.get(index).name.clone())
                            .unwrap_or_else(|| "scatter".to_owned()),
                    ),
                ],
            ),
        );
    }

    if let Some(scatter) = data.symbols.scatter() {
        add(
            Topic::Scatter,
            "title_scatters",
            render_text(
                text.template("scatters"),
                &[("scatter", data.symbols.get(scatter).name.clone())],
            ),
        );
    }

    if let Some(rule) = free_spins_rule(data) {
        add(rule.topic, "title_free_spins", rule.text);
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
            "title_refining",
            render_text(text.template("refining"), &[("burned", burned)]),
        );
    }

    if data.freespins.shapes.len() > 1 {
        let named = data
            .freespins
            .shapes
            .iter()
            .map(|shape| {
                render_text(
                    text.template("shape_name"),
                    &[
                        ("share", (shape.spin_permille / 10).to_string()),
                        ("multiplier", shape.multiplier.to_string()),
                    ],
                )
            })
            .collect::<Vec<_>>()
            .join(", or ");
        add(
            Topic::FreeSpinShapes,
            "title_shapes",
            render_text(text.template("shapes"), &[("shapes", named)]),
        );
    }

    if data.config.hoard_capacity > 0 {
        add(
            Topic::Hoard,
            "title_hoard",
            render_text(
                text.template("hoard"),
                &[
                    ("capacity", data.config.hoard_capacity.to_string()),
                    ("board", data.bonus.board_size.to_string()),
                    ("blanks", data.bonus.blanks.to_string()),
                    (
                        "value",
                        format!("{:.0}", bonus::expected_permille(&data.bonus) / 10.0),
                    ),
                ],
            ),
        );
    }

    if data.jackpots.contribution_permille > 0 {
        let shared = data
            .jackpots
            .tiers
            .iter()
            .filter(|tier| tier.shared)
            .map(|tier| tier.name.as_str())
            .collect::<Vec<_>>();
        let shared_text = if shared.is_empty() {
            String::new()
        } else {
            render_text(
                text.template("jackpots_shared"),
                &[("names", shared.join(" and the "))],
            )
        };
        add(
            Topic::Jackpots,
            "title_jackpots",
            render_text(
                text.template("jackpots"),
                &[
                    (
                        "contribution",
                        (data.jackpots.contribution_permille as f32 / 10.0).to_string(),
                    ),
                    (
                        "rtp",
                        format!("{:.1}", jackpot::expected_rtp(&data.jackpots) * 100.0),
                    ),
                    ("shared", shared_text),
                ],
            ),
        );
    }

    if data.holdspin.trigger_eggs > 0 {
        add(
            Topic::Wrath,
            "title_wrath",
            render_text(
                text.template("wrath"),
                &[
                    ("trigger", data.holdspin.trigger_eggs.to_string()),
                    (
                        "mean",
                        format!("{:.1}", holdspin::mean_coin_multiple(&data.holdspin)),
                    ),
                    ("respins", data.holdspin.respins.to_string()),
                    (
                        "cells",
                        (data.config.reel_count * data.config.row_count).to_string(),
                    ),
                    ("multiple", data.holdspin.full_board_multiple.to_string()),
                ],
            ),
        );
    }

    if data.seam.trigger_count > 0 && !data.seam.rites.is_empty() {
        let named = data
            .seam
            .rites
            .iter()
            .map(|rite| format!("{} {}", rite.name.to_lowercase(), promise(text, rite)))
            .collect::<Vec<_>>()
            .join("; ");
        add(
            Topic::Seam,
            "title_seam",
            render_text(
                text.template("seam"),
                &[
                    ("trigger", data.seam.trigger_count.to_string()),
                    ("steps", data.seam.steps.to_string()),
                    ("rites", named),
                    ("multiple", data.seam.max_multiple.to_string()),
                ],
            ),
        );
    }

    if data.gamble.max_steps > 0 {
        let half = if data.gamble.allow_half {
            text.template("gamble_half").to_owned()
        } else {
            String::new()
        };
        add(
            Topic::Gamble,
            "title_gamble",
            render_text(
                text.template("gamble"),
                &[
                    ("steps", data.gamble.max_steps.to_string()),
                    ("half", half),
                    ("ceiling", data.gamble.ceiling_multiple.to_string()),
                ],
            ),
        );
    }

    if let Some(ante) = data.ante() {
        add(
            Topic::Ante,
            "title_ante",
            render_text(
                text.template("ante"),
                &[("cost", format!("{:.2}", ante.cost_permille as f64 / 1000.0))],
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
            "title_feature_buy",
            render_text(
                text.template("feature_buy"),
                &[
                    ("tiers", data.featurebuy.tiers.len().to_string()),
                    ("cheapest", cheapest.to_string()),
                    (
                        "rtp",
                        format!("{:.1}", data.featurebuy.target_rtp_permille as f64 / 10.0),
                    ),
                ],
            ),
        );
    }

    rules
}

fn promise(text: &RuleText, rite: &crate::data::RiteDef) -> String {
    match rite.kind {
        RiteKind::Widen { .. } => text.template("rite_widen").to_owned(),
        RiteKind::Enrich { rungs } if rungs > 1 => render_text(
            text.template("rite_enrich_many"),
            &[("rungs", rungs.to_string())],
        ),
        RiteKind::Enrich { .. } => text.template("rite_enrich_one").to_owned(),
        RiteKind::Gild { .. } => text.template("rite_gild").to_owned(),
    }
}

/// The free-spin paragraph, which has to read an award table of arbitrary size.
pub(super) fn free_spins_rule(data: &GameData) -> Option<Rule> {
    let text = &data.presentation.rules;
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
        .map(|(count, spins)| {
            render_text(
                text.template("free_spin_award_item"),
                &[("count", count.to_string()), ("spins", spins.to_string())],
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let multiplier = if data.freespins.multiplier > 1 {
        render_text(
            text.template("free_spin_multiplier"),
            &[("multiplier", data.freespins.multiplier.to_string())],
        )
    } else {
        String::new()
    };
    let retrigger = if data.freespins.retrigger {
        text.template("free_spin_retrigger").to_owned()
    } else {
        text.template("free_spin_no_retrigger").to_owned()
    };
    Some(Rule {
        topic: Topic::FreeSpins,
        title: text.template("title_free_spins").to_owned(),
        text: format!(
            "{}{}{}{}",
            render_text(text.template("free_spin_awards"), &[("table", table)]),
            multiplier,
            retrigger,
            text.template("free_spin_close"),
        ),
    })
}
