//! Embedded game data: config, symbols, reel strips, paylines, free-spin rules.
//!
//! Everything that defines the game's maths lives in `assets/data/*.json` and is
//! resolved here into index-based lookup tables so the engine can stay allocation
//! free and fast enough for a million-spin Monte-Carlo run.

use macroquad_toolkit::assets::TextureConfig;
use macroquad_toolkit::data_loader::{load_embedded_json, load_embedded_json_labeled};
use serde::{Deserialize, Serialize};

mod features;
mod load;
mod machines;
mod presentation;

pub use features::{
    validate_feature_buy, BonusConfig, CascadeConfig, FeatureAward, FeatureBuyConfig,
    FeatureBuyTier, GambleConfig, HoldSpinConfig, RiteDef, RiteKind, SeamConfig,
};
pub use machines::{machine_by_id, symbol_set, MachineDef, MACHINES};
pub use presentation::{
    render as render_text, PresentationConfig, RuleText, ShortcutText, TimingConfig,
};
use std::collections::HashMap;

const TEXTURE_MANIFEST_JSON: &str =
    macroquad_toolkit::include_json_str!("../assets/data/texture_manifest.json");
/// Shared across machines on purpose: the bonus is a presentation layer over
/// the hoard whose expected value is normalised to 1000 permille, so each
/// cabinet's own `hatch_pot_multiplier` is what scales it (§5.10).
const BONUS_JSON: &str = macroquad_toolkit::include_json_str!("../assets/data/bonus.json");
const GAMBLE_JSON: &str = macroquad_toolkit::include_json_str!("../assets/data/gamble.json");
const PRESENTATION_JSON: &str =
    macroquad_toolkit::include_json_str!("../assets/data/presentation.json");

/// Longest run a paytable entry can describe. Index 0..=5, so a 5-reel game
/// indexes `pay_table[symbol][count]` directly.
pub const MAX_RUN: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub game_name: String,
    pub display_name: String,
    pub save_slot: String,
    pub version: String,
    pub starting_balance: i64,
    pub reel_count: usize,
    pub row_count: usize,
    pub line_bets: Vec<i64>,
    pub default_line_bet_index: usize,
    pub hoard_capacity: u32,
    pub hatch_pot_multiplier: i64,
    /// What breaking the hoard early returns, in parts per thousand of the
    /// banked pot (§5.53). The cut is what makes it a decision rather than a
    /// free rescue — a player near a full meter should hold on.
    pub hoard_salvage_permille: u32,
    /// What the vault advances a player with nothing left to break (§5.53).
    /// Must buy several spins at the cheapest stake, or it is not a rescue.
    pub vault_stake: i64,
    pub free_spin_multiplier: i64,
    /// Run lengths the player can pick between in the settings panel.
    pub autospin_choices: Vec<u32>,
    /// Index into `autospin_choices` used until the player picks another.
    pub default_autospin_choice: usize,
    /// A win of this many total bets or more counts as a big win: it raises a
    /// card, shakes the screen, and stops an autospin run.
    pub big_win_multiple: i64,
    /// Master volume for the synthesised effects, 0.0 to 1.0.
    pub sfx_volume: f32,
    /// Which win model this cabinet uses (§5.14). `default` so the two payline
    /// machines need no edit — a key that did not exist yesterday must not
    /// invalidate data that was correct.
    #[serde(default)]
    pub evaluation: Evaluation,
    /// Reels that change height every spin (§5.20). Absent on a fixed cabinet,
    /// which is every one built before it.
    #[serde(default)]
    pub reel_heights: Option<ReelHeights>,
    /// Line-bet units one spin costs. A payline machine buys one unit per line
    /// and leaves this unset; a ways machine has no lines to count, so it says
    /// so outright.
    #[serde(default)]
    pub bet_units: Option<usize>,
    /// Which symbol set this cabinet draws (§5.41).
    pub symbol_set: String,
    /// Which palette the panels and chrome use (§5.43). Defaults to the symbol
    /// set's name, because in practice a cabinet's room matches its symbols —
    /// but the two are separable, and a new theme should not require a new set.
    #[serde(default)]
    pub theme: Option<String>,
}

/// Range of visible rows a reel may take on a shifting cabinet (§5.20).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ReelHeights {
    pub min: usize,
    pub max: usize,
}

/// How a cabinet decides what has won.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evaluation {
    /// Fixed paths across the grid; one win per line at most (§3).
    #[default]
    Lines,
    /// Any path — a symbol pays if it appears on every reel from the first, and
    /// the win is multiplied by how many paths there are (§5.14).
    Ways,
    /// Connected groups, anywhere on the grid, ignoring the reels entirely
    /// (§5.35).
    Cluster,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDef {
    pub id: String,
    pub name: String,
    pub tier: String,
    /// Three-letter code, drawn only when `art` names a shape the renderer does
    /// not know — a visible fallback rather than an empty cell.
    pub short: String,
    /// Which procedural shape `ui::symbols` draws for this symbol.
    pub art: String,
    pub texture: String,
    pub color: [f32; 3],
    #[serde(default)]
    pub is_wild: bool,
    #[serde(default)]
    pub is_scatter: bool,
    /// Contributes to the Dragon's Hoard meter when it lands.
    #[serde(default)]
    pub is_hoard: bool,
    /// Run length ("3"/"4"/"5") to payout multiplier. Line symbols multiply the
    /// line bet; the scatter multiplies the total bet.
    /// Filled from the cabinet's own `paytable.json` at load, not from the
    /// symbol set: what a symbol *is* is shared between cabinets, what it pays
    /// is not (§5.41).
    #[serde(default, skip)]
    pub pays: HashMap<String, i64>,
}

/// Symbol registry resolved into index lookups.
#[derive(Debug, Clone)]
pub struct Symbols {
    defs: Vec<SymbolDef>,
    index_by_id: HashMap<String, usize>,
    pay_table: Vec<[i64; MAX_RUN + 1]>,
    wild: Option<usize>,
    scatter: Option<usize>,
    hoard: Option<usize>,
}

impl Symbols {
    fn new(defs: Vec<SymbolDef>) -> Result<Self, String> {
        if defs.is_empty() {
            return Err("symbols.json contained no symbols".to_owned());
        }

        let mut index_by_id = HashMap::with_capacity(defs.len());
        let mut pay_table = Vec::with_capacity(defs.len());
        let mut wild = None;
        let mut scatter = None;
        let mut hoard = None;

        for (index, def) in defs.iter().enumerate() {
            if index_by_id.insert(def.id.clone(), index).is_some() {
                return Err(format!("duplicate symbol id '{}'", def.id));
            }

            let mut pays = [0i64; MAX_RUN + 1];
            for (key, value) in &def.pays {
                let count: usize = key.parse().map_err(|_| {
                    format!("symbol '{}' has non-numeric pay key '{}'", def.id, key)
                })?;
                if count > MAX_RUN {
                    return Err(format!(
                        "symbol '{}' pays for run {} which exceeds {}",
                        def.id, count, MAX_RUN
                    ));
                }
                pays[count] = *value;
            }
            pay_table.push(pays);

            if def.is_wild {
                if wild.is_some() {
                    return Err("more than one wild symbol declared".to_owned());
                }
                wild = Some(index);
            }
            if def.is_scatter {
                if scatter.is_some() {
                    return Err("more than one scatter symbol declared".to_owned());
                }
                scatter = Some(index);
            }
            if def.is_hoard {
                if hoard.is_some() {
                    return Err("more than one hoard symbol declared".to_owned());
                }
                hoard = Some(index);
            }
        }

        if defs.iter().any(|def| def.is_wild && def.is_scatter) {
            return Err("a symbol cannot be both wild and scatter".to_owned());
        }

        Ok(Self {
            defs,
            index_by_id,
            pay_table,
            wild,
            scatter,
            hoard,
        })
    }

    pub fn get(&self, index: usize) -> &SymbolDef {
        &self.defs[index]
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.index_by_id.get(id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &SymbolDef)> {
        self.defs.iter().enumerate()
    }

    /// Payout multiplier for `count` matching symbols, 0 when the run does not pay.
    pub fn pay(&self, symbol: usize, count: usize) -> i64 {
        if count > MAX_RUN {
            return 0;
        }
        self.pay_table[symbol][count]
    }

    pub fn wild(&self) -> Option<usize> {
        self.wild
    }

    pub fn scatter(&self) -> Option<usize> {
        self.scatter
    }

    pub fn hoard(&self) -> Option<usize> {
        self.hoard
    }

    pub fn is_wild(&self, symbol: usize) -> bool {
        self.wild == Some(symbol)
    }

    pub fn is_scatter(&self, symbol: usize) -> bool {
        self.scatter == Some(symbol)
    }
}

/// One progressive tier. See `state::jackpot` for the maths these drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JackpotTier {
    pub id: String,
    pub name: String,
    /// What the pot resets to after it pays.
    pub seed: i64,
    /// Slice of the pooled contribution, in permille. All tiers must total 1000.
    pub share_permille: i64,
    /// One win per this many credits of turnover — the definition that makes the
    /// trigger bet-fair and its return closed-form.
    pub odds_per_credit: i64,
    /// Whether this pot belongs to the whole floor rather than this cabinet
    /// (§5.57). Its maths is unchanged either way; only where the accrual is
    /// stored moves. `default` so a ladder written before the floor existed
    /// still loads as six independent pots.
    #[serde(default)]
    pub shared: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jackpots {
    /// Fraction of every stake diverted into the pots, in permille.
    pub contribution_permille: i64,
    /// Ordered most frequent to least frequent.
    pub tiers: Vec<JackpotTier>,
}

/// Validate progressive jackpot data before it can become session state.
///
/// The state layer only accrues and rolls already-validated pots. Keeping these
/// invariants beside the schema prevents the loader from depending on runtime
/// behavior merely to decide whether a cabinet is well formed.
pub fn validate_jackpots(jackpots: &Jackpots) -> Result<(), String> {
    if jackpots.tiers.is_empty() {
        return Err("jackpots.json declared no tiers".to_owned());
    }
    if jackpots.contribution_permille < 0 {
        return Err("contribution_permille cannot be negative".to_owned());
    }

    let shares: i64 = jackpots.tiers.iter().map(|tier| tier.share_permille).sum();
    if shares != 1000 {
        return Err(format!(
            "jackpot shares must total 1000 permille, got {}",
            shares
        ));
    }

    for tier in &jackpots.tiers {
        if tier.odds_per_credit <= 0 {
            return Err(format!("jackpot '{}' has non-positive odds", tier.id));
        }
        if tier.seed < 0 {
            return Err(format!("jackpot '{}' has a negative seed", tier.id));
        }
    }

    if jackpots
        .tiers
        .windows(2)
        .any(|pair| pair[1].odds_per_credit <= pair[0].odds_per_credit)
    {
        return Err("jackpot tiers must be ordered from most to least frequent".to_owned());
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payline {
    pub id: u32,
    pub name: String,
    pub rows: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeSpinsConfig {
    /// Scatter count ("3"/"4"/"5") to free spins awarded.
    pub awards: HashMap<String, u32>,
    pub retrigger: bool,
    pub multiplier: i64,
    pub expanding_wilds: bool,
    /// Symbols burned off the strips as the feature runs (§5.21). Absent on a
    /// cabinet whose free spins are simply N of the same spin.
    #[serde(default)]
    pub refine: Option<Refine>,
    /// The side bet that buys a better chance at this feature (§5.75). Absent
    /// on a cabinet that does not offer one.
    #[serde(default)]
    pub ante: Option<Ante>,
    /// The ways this feature can be run (§5.64), all worth the same.
    ///
    /// Empty on a refining cabinet, and that is a decision rather than an
    /// omission: burning symbols off the strips makes a late free spin worth
    /// more than an early one, so trading spins for multiplier there would
    /// change the return. A cabinet that cannot price a choice fairly does not
    /// offer one.
    #[serde(default)]
    pub shapes: Vec<FreeSpinShape>,
}

/// How many times a strip is repeated before the ante's scatters are woven in.
///
/// A strip is a cycle, so repeating it changes nothing on its own — the same
/// symbols in the same proportions. What it buys is **resolution**. Dragon's
/// Hoard's first reel carries one scatter in forty; adding a whole scatter to
/// that strip raises its share by 2.4%, and there is no smaller step available.
/// Against the strip repeated eight times, one scatter is a step of 0.3%, which
/// is the difference between an ante that can be priced and one that cannot.
const ANTE_STRIP_REPEAT: usize = 8;

/// Weave `extra` copies of `symbol` into a strip at even spacing.
///
/// The result is longer than the original, which is the point: adding a symbol
/// without removing one raises that symbol's share. Removing something else to
/// keep the length fixed would change the base game's other odds too, and the
/// ante is meant to change exactly one thing.
fn weave(strip: &[usize], symbol: usize, extra: usize) -> Vec<usize> {
    if extra == 0 || strip.is_empty() {
        return strip.to_vec();
    }
    let mut woven = Vec::with_capacity(strip.len() + extra);
    // Insert after position `i * len / extra` for each i, walking once. Integer
    // arithmetic throughout: a float here would put two scatters in the same
    // slot on some strip lengths and none in the last.
    let mut next = 0usize;
    for (index, entry) in strip.iter().enumerate() {
        woven.push(*entry);
        while next < extra && (next + 1) * strip.len() <= (index + 1) * extra {
            woven.push(symbol);
            next += 1;
        }
    }
    while next < extra {
        woven.push(symbol);
        next += 1;
    }
    woven
}

/// The ante bet (§5.75): pay more per spin for a better chance at the feature.
///
/// Both numbers are per cabinet and both are *stated*, which is the whole
/// difference between this and the thing it is modelled on. A real cabinet sells
/// an ante and does not tell you what it does to the return; this one has a
/// harness that measures both and a panel that prints the answer even when the
/// answer is unflattering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ante {
    /// What the stake is multiplied by, in permille. 1250 is "a quarter more".
    ///
    /// Permille rather than a float because it is money: a stake has to come out
    /// as an exact number of credits on every bet step, and `bet * 5 / 4` does
    /// that while `bet * 1.25` invites a rounding argument nobody wins.
    pub cost_permille: i64,
    /// Extra scatters woven into the **first reel** while the ante is on.
    ///
    /// The first reel only, and that is the whole reason the mechanism is
    /// usable. A feature that needs three scatters triggers on roughly the cube
    /// of the per-reel scatter share, so weaving one extra into every strip does
    /// not raise the trigger rate by a quarter — it raises it by twenty-seven
    /// times. Measured, on Dragon's Hoard: 1,475 features became 45,805 and the
    /// return went from 0.98 to 6.85. Changing one reel is close to linear.
    ///
    /// **Derived, not authored.** Six cabinets would otherwise need six
    /// hand-balanced ante strip sets, which is six chances to get one subtly
    /// wrong and no way to notice. Weaving into the existing strips means a
    /// cabinet's ante is a function of its base game, and a change to the base
    /// strips carries into the ante automatically.
    pub extra_scatters: usize,
}

/// One way of running the feature: fewer spins worth more, or more worth less.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeSpinShape {
    pub id: String,
    pub name: String,
    /// Share of the awarded spins this shape grants, in permille.
    pub spin_permille: i64,
    /// What wins are multiplied by while it runs.
    pub multiplier: i64,
}

impl FreeSpinShape {
    /// Spins × multiplier: what makes two shapes worth the same.
    ///
    /// The whole design rests on this being equal across every shape, so it is
    /// a named thing that a validator and a test can both point at rather than
    /// an arithmetic coincidence in a JSON file.
    pub fn value(&self) -> i64 {
        self.spin_permille * self.multiplier
    }

    /// Spins this shape grants for an award of `spins`.
    ///
    /// At least one: a shape that granted none would take the feature away
    /// entirely, which no amount of multiplier makes up for.
    pub fn spins(&self, awarded: u32) -> u32 {
        ((awarded as i64 * self.spin_permille) / 1000).max(1) as u32
    }
}

/// Progressive symbol removal during free spins (§5.21).
///
/// Each free spin burns the next symbol in `order` off every strip, so the
/// feature escalates instead of repeating: by the last spin the reels hold only
/// what pays well. The order is data rather than "cheapest first" so a designer
/// can burn something out of sequence — and so the sequence is *visible*, which
/// matters when the player is watching it happen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refine {
    pub order: Vec<String>,
}

impl FreeSpinsConfig {
    /// The fewest scatters that award anything — the count the reels are
    /// chasing, and what anticipation (§5.11) is measured against. Derived from
    /// the award table rather than configured separately, so the two can never
    /// disagree.
    pub fn trigger_count(&self) -> usize {
        self.awards
            .iter()
            .filter(|(_, spins)| **spins > 0)
            .filter_map(|(count, _)| count.parse::<usize>().ok())
            .min()
            .unwrap_or(usize::MAX)
    }

    pub fn award_for(&self, scatter_count: usize) -> u32 {
        self.awards
            .get(&scatter_count.to_string())
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct GameData {
    /// Which machine this data came from. `GameData` is always exactly one
    /// machine's worth — switching cabinets rebuilds it rather than indexing
    /// into a collection, which is why no other module needed to change.
    pub machine: &'static MachineDef,
    pub config: GameConfig,
    pub symbols: Symbols,
    /// One strip per reel, each entry a symbol index.
    pub reels: Vec<Vec<usize>>,
    pub paylines: Vec<Payline>,
    pub freespins: FreeSpinsConfig,
    pub jackpots: Jackpots,
    pub bonus: BonusConfig,
    pub holdspin: HoldSpinConfig,
    /// The Seam mini-game (§5.80).
    pub seam: SeamConfig,
    pub featurebuy: FeatureBuyConfig,
    pub gamble: GambleConfig,
    /// `Some` only on a cascading cabinet (§5.15).
    pub cascade: Option<CascadeConfig>,
    pub texture_manifest: Vec<TextureConfig>,
    pub presentation: PresentationConfig,
}

impl GameData {
    pub fn machine_id(&self) -> &'static str {
        self.machine.id
    }

    /// Each machine gets its own save slot, so switching cabinets never
    /// overwrites the balance and hoard built up on the other one.
    pub fn save_slot(&self) -> String {
        format!("{}_{}", self.machine.id, self.config.save_slot)
    }

    /// Total bet for a line bet: every payline is always active.
    /// The palette this cabinet asks for.
    pub fn theme_name(&self) -> &str {
        self.config
            .theme
            .as_deref()
            .unwrap_or(&self.config.symbol_set)
    }

    pub fn total_bet(&self, line_bet: i64) -> i64 {
        line_bet * self.bet_units() as i64
    }

    /// The stake with the ante bet applied (§5.75).
    ///
    /// Integer throughout: `bet * 1250 / 1000` lands on an exact number of
    /// credits at every bet step, and the whole conservation harness rests on
    /// stakes and payouts being whole numbers that add up.
    pub fn staked(&self, line_bet: i64, ante: bool) -> i64 {
        let base = self.total_bet(line_bet);
        match self.freespins.ante.as_ref() {
            Some(def) if ante => base * def.cost_permille / 1_000,
            _ => base,
        }
    }

    /// Does this cabinet sell an ante?
    pub fn ante(&self) -> Option<&Ante> {
        self.freespins.ante.as_ref()
    }

    /// Strips with the first `burned` symbols of the refine order removed
    /// (§5.21). Returns the strips untouched when nothing is burned, which is
    /// every spin on every cabinet without a refine order.
    ///
    /// A strip that lost every symbol would be unspinnable, so a reel that would
    /// empty keeps what it has — validation cannot catch this, because whether
    /// it happens depends on how a designer laid out one particular reel.
    /// The strips as the ante bet turns them (§5.75).
    ///
    /// Extra scatters woven in at even spacing rather than appended, because a
    /// strip is a cycle and a clump of scatters at one end would make the
    /// feature arrive in bursts. Even spacing keeps the ante's trigger rate as
    /// steady as the base game's.
    ///
    /// The first reel carries them all, and the strip is repeated first so the
    /// step is small enough to price — see [`ANTE_STRIP_REPEAT`].
    ///
    /// Returns the base strips unchanged when the cabinet has no ante, so every
    /// caller can ask without checking first.
    pub fn ante_reels(&self) -> Vec<Vec<usize>> {
        let Some(ante) = self.freespins.ante.as_ref() else {
            return self.reels.clone();
        };
        let Some(scatter) = self.symbols.scatter() else {
            return self.reels.clone();
        };
        if ante.extra_scatters == 0 {
            return self.reels.clone();
        }

        self.reels
            .iter()
            .enumerate()
            .map(|(reel, strip)| {
                if reel != 0 {
                    return strip.clone();
                }
                let repeated: Vec<usize> = strip
                    .iter()
                    .copied()
                    .cycle()
                    .take(strip.len() * ANTE_STRIP_REPEAT)
                    .collect();
                weave(&repeated, scatter, ante.extra_scatters)
            })
            .collect()
    }

    pub fn refined_reels(&self, burned: usize) -> Vec<Vec<usize>> {
        let Some(refine) = self.freespins.refine.as_ref() else {
            return self.reels.clone();
        };
        let doomed: Vec<usize> = refine
            .order
            .iter()
            .take(burned)
            .filter_map(|id| self.symbols.index_of(id))
            .collect();
        if doomed.is_empty() {
            return self.reels.clone();
        }

        self.reels
            .iter()
            .map(|strip| {
                let kept: Vec<usize> = strip
                    .iter()
                    .copied()
                    .filter(|symbol| !doomed.contains(symbol))
                    .collect();
                if kept.is_empty() {
                    strip.clone()
                } else {
                    kept
                }
            })
            .collect()
    }

    /// How many symbols the refine order can burn in total.
    pub fn refine_depth(&self) -> usize {
        self.freespins
            .refine
            .as_ref()
            .map_or(0, |refine| refine.order.len())
    }

    /// Line-bet units a spin costs. A payline machine buys its lines; a ways
    /// machine buys all its ways at once for a configured price, because 243
    /// units of line bet would be an absurd stake.
    pub fn bet_units(&self) -> usize {
        self.config.bet_units.unwrap_or(self.paylines.len()).max(1)
    }

    /// How many ways this cabinet pays, for the panel readout.
    ///
    /// `None` on a payline machine, which counts lines instead, and `None` on a
    /// shifting one (§5.20) — there the figure changes every spin, so the panel
    /// reads it off the grid rather than off the config.
    pub fn ways_count(&self) -> Option<usize> {
        if self.config.evaluation != Evaluation::Ways || self.config.reel_heights.is_some() {
            return None;
        }
        Some(self.config.row_count.pow(self.config.reel_count as u32))
    }

    /// Highest ways this cabinet can reach, for the machine picker.
    pub fn max_ways(&self) -> Option<usize> {
        if self.config.evaluation != Evaluation::Ways {
            return None;
        }
        let tallest = self
            .config
            .reel_heights
            .map_or(self.config.row_count, |heights| heights.max);
        Some(tallest.pow(self.config.reel_count as u32))
    }

    pub fn line_bet(&self, index: usize) -> i64 {
        self.config
            .line_bets
            .get(index)
            .copied()
            .unwrap_or_else(|| self.config.line_bets[0])
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod ante_tests;
