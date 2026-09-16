//! Shared player-facing copy and presentation timing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const REQUIRED_SHORTCUTS: &[&str] = &[
    "bet", "max_bet", "autospin", "gamble", "buy", "rules", "paytable", "ledger", "machines",
    "awards", "settings", "session", "limits",
];
const REQUIRED_CELEBRATION_HEADINGS: &[&str] = &[
    "free_spins_entry",
    "free_spins_retrigger",
    "free_spins_summary",
    "hatch",
    "jackpot",
    "big_win",
    "gamble_lost",
    "seam_dry",
    "seam",
    "wrath",
    "wrath_full_board",
];
const REQUIRED_CELEBRATION_TITLES: &[&str] = &[
    "free_spins_entry",
    "free_spins_retrigger",
    "credits",
    "nothing",
];
const REQUIRED_CELEBRATION_SUBTITLES: &[&str] = &[
    "free_spins_entry",
    "free_spins_retrigger",
    "free_spins_summary",
    "hatch",
    "jackpot",
    "big_win",
    "gamble_lost",
    "seam_dry",
    "seam",
    "wrath",
    "wrath_full_board",
];
const REQUIRED_RULES: &[&str] = &[
    "title_paylines",
    "title_clusters",
    "title_ways",
    "title_shifting_reels",
    "title_cascades",
    "title_wilds",
    "title_scatters",
    "title_free_spins",
    "title_refining",
    "title_shapes",
    "title_hoard",
    "title_jackpots",
    "title_wrath",
    "title_seam",
    "title_gamble",
    "title_ante",
    "title_feature_buy",
    "paylines",
    "clusters",
    "ways",
    "ways_shifting",
    "shifting_reels",
    "cascades",
    "wilds",
    "wilds_expanded",
    "scatters",
    "refining",
    "shape_name",
    "shapes",
    "hoard",
    "jackpots",
    "jackpots_shared",
    "wrath",
    "seam",
    "gamble",
    "gamble_half",
    "ante",
    "feature_buy",
    "rite_widen",
    "rite_enrich_one",
    "rite_enrich_many",
    "rite_gild",
    "free_spin_awards",
    "free_spin_award_item",
    "free_spin_multiplier",
    "free_spin_retrigger",
    "free_spin_no_retrigger",
    "free_spin_close",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresentationConfig {
    pub timing: TimingConfig,
    pub shortcuts: ShortcutText,
    pub celebration: CelebrationText,
    pub rules: RuleText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    pub reel_base_seconds: f32,
    pub reel_stagger_seconds: f32,
    pub reel_blur_cap: f32,
    pub payout_seconds: f32,
    pub cascade_beat_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutText {
    pub spin: String,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CelebrationText {
    pub continue_prompt: String,
    pub headings: HashMap<String, String>,
    pub titles: HashMap<String, String>,
    pub subtitles: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleText {
    pub templates: HashMap<String, String>,
}

impl PresentationConfig {
    /// Reject missing copy and unsafe animation timings while the cabinet data
    /// is loaded, before a player can reach a half-rendered screen.
    pub fn validate(&self) -> Result<(), String> {
        validate_timing(&self.timing)?;
        require_keys("shortcut label", &self.shortcuts.labels, REQUIRED_SHORTCUTS)?;
        require_keys(
            "celebration heading",
            &self.celebration.headings,
            REQUIRED_CELEBRATION_HEADINGS,
        )?;
        require_keys(
            "celebration title",
            &self.celebration.titles,
            REQUIRED_CELEBRATION_TITLES,
        )?;
        require_keys(
            "celebration subtitle",
            &self.celebration.subtitles,
            REQUIRED_CELEBRATION_SUBTITLES,
        )?;
        require_keys("rule template", &self.rules.templates, REQUIRED_RULES)
    }
}

impl RuleText {
    pub fn template(&self, key: &str) -> &str {
        self.templates.get(key).map(String::as_str).unwrap_or("")
    }
}

fn validate_timing(timing: &TimingConfig) -> Result<(), String> {
    let values = [
        ("reel_base_seconds", timing.reel_base_seconds),
        ("reel_stagger_seconds", timing.reel_stagger_seconds),
        ("reel_blur_cap", timing.reel_blur_cap),
        ("payout_seconds", timing.payout_seconds),
        ("cascade_beat_seconds", timing.cascade_beat_seconds),
    ];
    if let Some((name, _)) = values.iter().find(|(_, value)| !value.is_finite()) {
        return Err(format!("presentation timing '{}' is not finite", name));
    }
    if timing.reel_base_seconds <= 0.0 || timing.reel_stagger_seconds < 0.0 {
        return Err("reel timing needs a positive base and non-negative stagger".to_owned());
    }
    if !(0.0..=1.0).contains(&timing.reel_blur_cap) {
        return Err("reel_blur_cap must be between 0 and 1".to_owned());
    }
    if timing.payout_seconds <= 0.0 || timing.cascade_beat_seconds <= 0.0 {
        return Err("payout and cascade timings must be positive".to_owned());
    }
    Ok(())
}

fn require_keys(
    kind: &str,
    values: &HashMap<String, String>,
    required: &[&str],
) -> Result<(), String> {
    for key in required {
        if values.get(*key).is_none_or(|value| value.trim().is_empty()) {
            return Err(format!("presentation.json is missing {} '{}',", kind, key));
        }
    }
    Ok(())
}

/// Substitute the small `{name}` placeholders used by the authored copy.
pub fn render(template: &str, values: &[(&str, String)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (key, value)| {
            text.replace(&format!("{{{key}}}"), value)
        })
}
