//! Embedded game data: config, symbols, reel strips, paylines, free-spin rules.
//!
//! Everything that defines the game's maths lives in `assets/data/*.json` and is
//! resolved here into index-based lookup tables so the engine can stay allocation
//! free and fast enough for a million-spin Monte-Carlo run.

use macroquad_toolkit::assets::TextureConfig;
use macroquad_toolkit::data_loader::{load_embedded_json, load_embedded_json_labeled};
use serde::{Deserialize, Serialize};

mod machines;

pub use machines::{machine_by_id, MachineDef, MACHINES};
use std::collections::HashMap;

const TEXTURE_MANIFEST_JSON: &str = include_str!("../assets/data/texture_manifest.json");
/// Shared across machines on purpose: the bonus is a presentation layer over
/// the hoard whose expected value is normalised to 1000 permille, so each
/// cabinet's own `hatch_pot_multiplier` is what scales it (§5.10).
const BONUS_JSON: &str = include_str!("../assets/data/bonus.json");
const GAMBLE_JSON: &str = include_str!("../assets/data/gamble.json");

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jackpots {
    /// Fraction of every stake diverted into the pots, in permille.
    pub contribution_permille: i64,
    /// Ordered most frequent to least frequent.
    pub tiers: Vec<JackpotTier>,
}

/// The Vault Pick board (§5.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusConfig {
    pub board_size: usize,
    /// How many empty chests end the round.
    pub blanks: usize,
    /// Prize pool, in permille of the hatch base. Drawn with replacement.
    pub prizes_permille: Vec<i64>,
}

/// The Dragon's Wrath hold-and-spin round (§5.12).
///
/// Shared by both machines like `bonus.json`, because every value here is a
/// multiple of *total bet* rather than a credit figure — the cabinet's own bet
/// ladder already scales it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldSpinConfig {
    /// Eggs on one grid that wake the dragon.
    pub trigger_eggs: usize,
    /// Respins granted, and restored in full by every coin that lands.
    pub respins: usize,
    /// Per-cell chance a coin lands on a respin, in permille.
    pub coin_chance_permille: usize,
    pub coin_values: Vec<CoinValue>,
    /// Paid on top when every cell fills.
    pub full_board_multiple: i64,
}

/// One rung of the coin table: a payout in multiples of total bet, and how
/// often it is drawn relative to the others.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinValue {
    pub multiple: i64,
    pub weight: u32,
}

/// Cascading reels (§5.15). Absent on a cabinet whose reels do not cascade,
/// which is why it is an `Option` on `GameData` rather than a flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeConfig {
    /// Multiplier per step of the chain. The last value repeats, so a ladder
    /// does not have to be as long as `max_steps`.
    pub multipliers: Vec<i64>,
    /// Hard cap on chain length. A strip that refilled into a win every time
    /// would otherwise never terminate.
    pub max_steps: usize,
}

impl CascadeConfig {
    /// Multiplier at a step, holding the top of the ladder once it is reached.
    pub fn multiplier_at(&self, step: usize) -> i64 {
        self.multipliers
            .get(step)
            .or_else(|| self.multipliers.last())
            .copied()
            .unwrap_or(1)
            .max(1)
    }
}

/// The Dragon's Gamble (§5.16).
///
/// Shared by every cabinet. Unlike the hold-and-spin trigger (§5.12) nothing
/// here reads the strips — a fair double is a fair double on any machine — and
/// the ceiling is expressed in total bets, so it scales with the stake by
/// itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GambleConfig {
    /// How many doubles a single win may be pushed through.
    pub max_steps: usize,
    /// Highest stake that may be gambled, in multiples of total bet. What stops
    /// a lucky run compounding without bound.
    pub ceiling_multiple: i64,
    pub allow_half: bool,
}

/// The Feature Buy menu (§5.13).
///
/// Per-machine, and necessarily so: a tier's price is derived from the expected
/// value of the feature it buys, and every cabinet tunes its own features.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBuyConfig {
    /// The return a bought feature is priced to give back, in permille. It is
    /// the machine's own RTP, so buying is neither better nor worse than
    /// spinning — see `state::featurebuy`.
    pub target_rtp_permille: i64,
    pub tiers: Vec<FeatureBuyTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBuyTier {
    pub id: String,
    pub name: String,
    pub description: String,
    pub award: FeatureAward,
    /// Price in multiples of *total* bet.
    pub price_multiple: i64,
}

/// What a tier hands over once it is paid for.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FeatureAward {
    FreeSpins { spins: u32 },
    Wrath { coins: usize },
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
    pub featurebuy: FeatureBuyConfig,
    pub gamble: GambleConfig,
    /// `Some` only on a cascading cabinet (§5.15).
    pub cascade: Option<CascadeConfig>,
    pub texture_manifest: Vec<TextureConfig>,
}

impl GameData {
    /// The machine the game boots into.
    pub fn load() -> Result<Self, String> {
        Self::load_machine(&MACHINES[0])
    }

    pub fn load_machine(machine: &'static MachineDef) -> Result<Self, String> {
        let label = |what: &str| format!("{}/{}", machine.id, what);

        let config: GameConfig = load_embedded_json_labeled(&label("game_config"), machine.config)?;
        let symbol_defs: Vec<SymbolDef> =
            load_embedded_json_labeled(&label("symbols"), machine.symbols)?;
        let symbols = Symbols::new(symbol_defs)?;
        let strips: Vec<Vec<String>> = load_embedded_json_labeled(&label("reels"), machine.reels)?;
        let paylines: Vec<Payline> =
            load_embedded_json_labeled(&label("paylines"), machine.paylines)?;
        let freespins: FreeSpinsConfig =
            load_embedded_json_labeled(&label("freespins"), machine.freespins)?;
        let jackpots: Jackpots = load_embedded_json_labeled(&label("jackpots"), machine.jackpots)?;
        let bonus: BonusConfig = load_embedded_json_labeled("bonus", BONUS_JSON)?;
        let holdspin: HoldSpinConfig =
            load_embedded_json_labeled(&label("holdspin"), machine.holdspin)?;
        let featurebuy: FeatureBuyConfig =
            load_embedded_json_labeled(&label("featurebuy"), machine.featurebuy)?;
        let gamble: GambleConfig = load_embedded_json_labeled("gamble", GAMBLE_JSON)?;
        let cascade: Option<CascadeConfig> = machine
            .cascade
            .map(|raw| load_embedded_json_labeled(&label("cascade"), raw))
            .transpose()?;
        let texture_manifest = load_embedded_json(TEXTURE_MANIFEST_JSON)?;

        let reels = resolve_strips(&symbols, &strips)?;
        let data = Self {
            machine,
            config,
            symbols,
            reels,
            paylines,
            freespins,
            jackpots,
            bonus,
            holdspin,
            featurebuy,
            gamble,
            cascade,
            texture_manifest,
        };
        data.validate()?;
        Ok(data)
    }

    pub fn machine_id(&self) -> &'static str {
        self.machine.id
    }

    /// Each machine gets its own save slot, so switching cabinets never
    /// overwrites the balance and hoard built up on the other one.
    pub fn save_slot(&self) -> String {
        format!("{}_{}", self.machine.id, self.config.save_slot)
    }

    /// Total bet for a line bet: every payline is always active.
    pub fn total_bet(&self, line_bet: i64) -> i64 {
        line_bet * self.bet_units() as i64
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

    fn validate(&self) -> Result<(), String> {
        let reel_count = self.config.reel_count;
        let row_count = self.config.row_count;

        if reel_count == 0 || row_count == 0 {
            return Err("reel_count and row_count must both be positive".to_owned());
        }
        if reel_count > MAX_RUN {
            return Err(format!(
                "reel_count {} exceeds the paytable maximum run of {}",
                reel_count, MAX_RUN
            ));
        }
        if self.reels.len() != reel_count {
            return Err(format!(
                "reels.json has {} strips but reel_count is {}",
                self.reels.len(),
                reel_count
            ));
        }
        for (index, strip) in self.reels.iter().enumerate() {
            if strip.len() < row_count {
                return Err(format!(
                    "reel strip {} has {} symbols, fewer than row_count {}",
                    index,
                    strip.len(),
                    row_count
                ));
            }
        }
        // A ways machine has no paylines by definition; a payline machine with
        // none would silently pay nothing but scatters.
        match self.config.evaluation {
            Evaluation::Lines if self.paylines.is_empty() => {
                return Err("paylines.json declared no paylines".to_owned());
            }
            Evaluation::Ways if !self.paylines.is_empty() => {
                return Err("a ways machine must not declare paylines".to_owned());
            }
            Evaluation::Ways if self.config.bet_units.is_none() => {
                return Err("a ways machine must declare bet_units".to_owned());
            }
            _ => {}
        }
        for line in &self.paylines {
            if line.rows.len() != reel_count {
                return Err(format!(
                    "payline {} covers {} reels but reel_count is {}",
                    line.id,
                    line.rows.len(),
                    reel_count
                ));
            }
            if let Some(row) = line.rows.iter().find(|row| **row >= row_count) {
                return Err(format!(
                    "payline {} references row {} outside row_count {}",
                    line.id, row, row_count
                ));
            }
        }
        if self.symbols.wild().is_none() {
            return Err("no wild symbol declared in symbols.json".to_owned());
        }
        if self.symbols.scatter().is_none() {
            return Err("no scatter symbol declared in symbols.json".to_owned());
        }
        if self.config.line_bets.is_empty() {
            return Err("game_config.json declared no line bets".to_owned());
        }
        if self.config.line_bets.iter().any(|bet| *bet <= 0) {
            return Err("line bets must all be positive".to_owned());
        }
        if self.config.hoard_capacity == 0 {
            return Err("hoard_capacity must be positive".to_owned());
        }
        if self.config.autospin_choices.is_empty() {
            return Err("game_config.json declared no autospin choices".to_owned());
        }
        if self.config.autospin_choices.contains(&0) {
            return Err("autospin choices must all be positive".to_owned());
        }
        crate::state::jackpot::validate(&self.jackpots, &self.config)?;
        if self.bonus.prizes_permille.is_empty() {
            return Err("bonus.json declared no prizes".to_owned());
        }
        if self.bonus.blanks == 0 {
            return Err("a bonus round with no blanks could never end".to_owned());
        }
        if self.bonus.blanks >= self.bonus.board_size {
            return Err("bonus blanks must leave room for at least one prize".to_owned());
        }
        if self.holdspin.coin_values.is_empty() {
            return Err("holdspin.json declared no coin values".to_owned());
        }
        if self
            .holdspin
            .coin_values
            .iter()
            .all(|value| value.weight == 0)
        {
            return Err("holdspin coin values must carry some weight".to_owned());
        }
        if self.holdspin.respins == 0 {
            return Err("a hold-and-spin round with no respins would end at once".to_owned());
        }
        // A trigger the grid cannot hold would make the feature unreachable, and
        // nothing else would notice — the sim would simply measure a game
        // without it.
        let cells = self.config.reel_count * self.config.row_count;
        crate::state::featurebuy::validate(&self.featurebuy, cells)?;
        if let Some(heights) = self.config.reel_heights {
            if heights.min == 0 || heights.min > heights.max {
                return Err("reel_heights must be a range with at least one row".to_owned());
            }
            if heights.max > self.config.row_count {
                return Err(format!(
                    "reel_heights max ({}) exceeds row_count ({}), which is what the \
                     window is sized against",
                    heights.max, self.config.row_count
                ));
            }
            // A payline names a row on every reel. On a cabinet where a reel
            // might only be two rows tall, half of them would point at cells
            // that are not there.
            if self.config.evaluation == Evaluation::Lines {
                return Err("shifting reels need ways evaluation".to_owned());
            }
        }
        if self.gamble.max_steps == 0 {
            return Err("a gamble with no steps could never be taken".to_owned());
        }
        if self.gamble.ceiling_multiple <= 0 {
            return Err("the gamble ceiling must leave something to gamble".to_owned());
        }
        if let Some(cascade) = &self.cascade {
            if cascade.max_steps == 0 {
                return Err("a cascade capped at zero steps would pay nothing".to_owned());
            }
            if cascade.multipliers.is_empty() {
                return Err("cascade.json declared no multipliers".to_owned());
            }
            if cascade.multipliers.iter().any(|value| *value < 1) {
                return Err("a cascade multiplier below 1 would shrink a win".to_owned());
            }
        }
        if self.holdspin.trigger_eggs == 0 || self.holdspin.trigger_eggs > cells {
            return Err(format!(
                "holdspin trigger_eggs ({}) must fit on a {}-cell grid",
                self.holdspin.trigger_eggs, cells
            ));
        }
        Ok(())
    }
}

fn resolve_strips(symbols: &Symbols, strips: &[Vec<String>]) -> Result<Vec<Vec<usize>>, String> {
    strips
        .iter()
        .enumerate()
        .map(|(reel, strip)| {
            strip
                .iter()
                .map(|id| {
                    symbols
                        .index_of(id)
                        .ok_or_else(|| format!("reel {} references unknown symbol '{}'", reel, id))
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_data_loads() {
        let data = GameData::load().unwrap();

        assert_eq!(data.config.game_name, "dragons_hoard");
        assert_eq!(data.reels.len(), data.config.reel_count);
        assert_eq!(data.paylines.len(), 20);
        assert!(data.symbols.wild().is_some());
        assert!(data.symbols.scatter().is_some());
        assert!(data.symbols.hoard().is_some());
    }

    #[test]
    fn paytable_resolves_by_run_length() {
        let data = GameData::load().unwrap();
        let chest = data.symbols.index_of("chest").unwrap();

        // Nothing pays below three, and longer runs always pay more.
        assert_eq!(data.symbols.pay(chest, 2), 0);
        assert!(data.symbols.pay(chest, 3) > 0);
        assert!(data.symbols.pay(chest, 4) > data.symbols.pay(chest, 3));
        assert!(data.symbols.pay(chest, 5) > data.symbols.pay(chest, 4));
        assert_eq!(data.symbols.pay(chest, MAX_RUN + 1), 0);
    }

    #[test]
    fn the_lowest_symbols_only_pay_from_four() {
        let data = GameData::load().unwrap();

        for id in ["copper", "gold"] {
            let symbol = data.symbols.index_of(id).unwrap();
            assert_eq!(data.symbols.pay(symbol, 3), 0, "{} should not pay at 3", id);
            assert!(data.symbols.pay(symbol, 4) > 0);
        }
    }

    #[test]
    fn every_reel_strip_carries_the_same_symbol_pool() {
        let data = GameData::load().unwrap();
        let scatter = data.symbols.scatter().unwrap();

        for (index, strip) in data.reels.iter().enumerate() {
            let scatters = strip.iter().filter(|symbol| **symbol == scatter).count();
            assert_eq!(
                scatters, 1,
                "reel {} should carry exactly one scatter, found {}",
                index, scatters
            );
        }
    }

    #[test]
    fn total_bet_covers_every_payline() {
        let data = GameData::load().unwrap();
        assert_eq!(data.total_bet(10), 200);
    }

    #[test]
    fn free_spin_awards_scale_with_scatters() {
        let data = GameData::load().unwrap();

        assert_eq!(data.freespins.award_for(2), 0);
        assert_eq!(data.freespins.award_for(3), 10);
        assert_eq!(data.freespins.award_for(5), 20);
    }
}
