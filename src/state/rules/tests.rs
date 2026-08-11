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

/// A cabinet that asks the player something has to say so.
///
/// §5.81 made the seam wait on a decision and the rule kept describing it in
/// the passive — "over two moves it takes the cells around it or climbs the
/// paytable", which reads as a thing that happens to you. That is the same
/// fault §5.64 shipped and this module exists to catch, arriving one level
/// down: the *topic* was covered and the *sentence* was wrong.
///
/// A topic of its own would have caught it and would have cost a second
/// block in a panel that has none to spare, so the check is this instead.
#[test]
fn the_seam_rule_says_whose_decision_it_is() {
    for data in every_machine() {
        if data.seam.rites.len() < 2 {
            continue;
        }
        let rule = rules(&data)
            .into_iter()
            .find(|rule| rule.topic == Topic::Seam)
            .unwrap_or_else(|| panic!("{} has a seam and no rule for it", data.machine.id));
        assert!(
            rule.text.contains("You pick") || rule.text.contains("you pick"),
            "{} offers {} rites and its rule never says the player chooses: {:?}",
            data.machine.id,
            data.seam.rites.len(),
            rule.text
        );
        // And every rite has to be named, or the choice is being described
        // as narrower than it is.
        for rite in &data.seam.rites {
            assert!(
                rule.text.to_lowercase().contains(&rite.name.to_lowercase()),
                "{} offers '{}' and the rule never mentions it",
                data.machine.id,
                rite.name
            );
        }
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
