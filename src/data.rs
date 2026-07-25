//! Embedded game data: config, symbols, reel strips, paylines, free-spin rules.
//!
//! Everything that defines the game's maths lives in `assets/data/*.json` and is
//! resolved here into index-based lookup tables so the engine can stay allocation
//! free and fast enough for a million-spin Monte-Carlo run.

use macroquad_toolkit::assets::TextureConfig;
use macroquad_toolkit::data_loader::{load_embedded_json, load_embedded_json_labeled};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const GAME_CONFIG_JSON: &str = include_str!("../assets/data/game_config.json");
const SYMBOLS_JSON: &str = include_str!("../assets/data/symbols.json");
const REELS_JSON: &str = include_str!("../assets/data/reels.json");
const PAYLINES_JSON: &str = include_str!("../assets/data/paylines.json");
const FREESPINS_JSON: &str = include_str!("../assets/data/freespins.json");
const JACKPOTS_JSON: &str = include_str!("../assets/data/jackpots.json");
const TEXTURE_MANIFEST_JSON: &str = include_str!("../assets/data/texture_manifest.json");

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
    pub fn award_for(&self, scatter_count: usize) -> u32 {
        self.awards
            .get(&scatter_count.to_string())
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub config: GameConfig,
    pub symbols: Symbols,
    /// One strip per reel, each entry a symbol index.
    pub reels: Vec<Vec<usize>>,
    pub paylines: Vec<Payline>,
    pub freespins: FreeSpinsConfig,
    pub jackpots: Jackpots,
    pub texture_manifest: Vec<TextureConfig>,
}

impl GameData {
    pub fn load() -> Result<Self, String> {
        let config: GameConfig = load_embedded_json_labeled("game_config", GAME_CONFIG_JSON)?;
        let symbol_defs: Vec<SymbolDef> = load_embedded_json_labeled("symbols", SYMBOLS_JSON)?;
        let symbols = Symbols::new(symbol_defs)?;
        let strips: Vec<Vec<String>> = load_embedded_json_labeled("reels", REELS_JSON)?;
        let paylines: Vec<Payline> = load_embedded_json_labeled("paylines", PAYLINES_JSON)?;
        let freespins: FreeSpinsConfig = load_embedded_json_labeled("freespins", FREESPINS_JSON)?;
        let jackpots: Jackpots = load_embedded_json_labeled("jackpots", JACKPOTS_JSON)?;
        let texture_manifest = load_embedded_json(TEXTURE_MANIFEST_JSON)?;

        let reels = resolve_strips(&symbols, &strips)?;
        let data = Self {
            config,
            symbols,
            reels,
            paylines,
            freespins,
            jackpots,
            texture_manifest,
        };
        data.validate()?;
        Ok(data)
    }

    /// Total bet for a line bet: every payline is always active.
    pub fn total_bet(&self, line_bet: i64) -> i64 {
        line_bet * self.paylines.len() as i64
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
        if self.paylines.is_empty() {
            return Err("paylines.json declared no paylines".to_owned());
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
