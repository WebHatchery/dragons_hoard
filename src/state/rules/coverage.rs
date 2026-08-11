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
    // The Seam.
    ("seam", "trigger_count", Topic::Seam),
    ("seam", "steps", Topic::Seam),
    ("seam", "rites", Topic::Seam),
    ("seam", "max_multiple", Topic::Seam),
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
        ("seam", serde_json::to_value(&data.seam).unwrap()),
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
