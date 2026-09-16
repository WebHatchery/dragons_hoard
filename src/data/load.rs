//! Turning a cabinet's JSON into a [`GameData`], and refusing it when it is
//! wrong.
//!
//! # Why this is not in `data`
//!
//! `data.rs` reached 799 lines of an 800-line hard limit — the rule every
//! project in this workspace states and none of them checked (§5.54). One more
//! field and it would have broken, which is a poor reason to stop adding
//! content.
//!
//! The seam was already there. Everything left behind describes *what a cabinet
//! is*: its config, its symbols, its paylines, its features. Everything here is
//! the one-off act of **building one and proving it makes sense** — reading the
//! embedded JSON, resolving symbol names to strip indices, and the two hundred
//! lines of validation that turn a typo in a data file into a message at boot
//! rather than a panic on the first spin.
//!
//! That distinction matters beyond the line count: the types are read on every
//! frame and this runs exactly once per cabinet switch.

use super::*;
use std::collections::HashMap;

impl GameData {
    /// The machine the game boots into.
    pub fn load() -> Result<Self, String> {
        Self::load_machine(&MACHINES[0])
    }

    pub fn load_machine(machine: &'static MachineDef) -> Result<Self, String> {
        let label = |what: &str| format!("{}/{}", machine.id, what);

        let config: GameConfig = load_embedded_json_labeled(&label("game_config"), machine.config)?;
        // Identity from the shared set, payouts from this cabinet. Four
        // cabinets carried nine duplicated symbol definitions before this
        // split, and retheming one meant editing a copy (§5.41).
        let mut symbol_defs: Vec<SymbolDef> = load_embedded_json_labeled(
            &label("symbol set"),
            symbol_set(&config.symbol_set).ok_or_else(|| {
                format!("{} names no symbol set '{}'", machine.id, config.symbol_set)
            })?,
        )?;
        let paytable: HashMap<String, HashMap<String, i64>> =
            load_embedded_json_labeled(&label("paytable"), machine.paytable)?;
        for def in &mut symbol_defs {
            def.pays = paytable.get(&def.id).cloned().ok_or_else(|| {
                format!("{}: no paytable entry for symbol '{}'", machine.id, def.id)
            })?;
        }
        if let Some(extra) = paytable
            .keys()
            .find(|id| !symbol_defs.iter().any(|def| def.id == **id))
        {
            // A paytable naming a symbol the set does not have is a rename that
            // only got half done, and would silently never pay.
            return Err(format!(
                "{}: paytable names '{}', which is not in the '{}' set",
                machine.id, extra, config.symbol_set
            ));
        }
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
        let seam: SeamConfig = load_embedded_json_labeled(&label("seam"), machine.seam)?;
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
            seam,
            featurebuy,
            gamble,
            cascade,
            texture_manifest,
        };
        data.validate()?;
        Ok(data)
    }

    fn validate(&self) -> Result<(), String> {
        let reel_count = self.config.reel_count;
        let row_count = self.config.row_count;

        if reel_count == 0 || row_count == 0 {
            return Err("reel_count and row_count must both be positive".to_owned());
        }
        // A line or a ways win is a run across the reels, so it cannot be longer
        // than the paytable. A cluster is a connected group and its size has
        // nothing to do with the reel count (§5.35), which is why that cabinet
        // can be six wide.
        if self.config.evaluation != Evaluation::Cluster && reel_count > MAX_RUN {
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
        validate_jackpots(&self.jackpots)?;
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
        validate_feature_buy(&self.featurebuy, cells)?;
        if let Some(refine) = &self.freespins.refine {
            if refine.order.is_empty() {
                return Err("freespins.refine declared no symbols to burn".to_owned());
            }
            for id in &refine.order {
                let Some(index) = self.symbols.index_of(id) else {
                    return Err(format!("refine order names unknown symbol '{}'", id));
                };
                // Burning either of these would break the feature that is
                // running: no scatter means no retrigger, no wild means no
                // substitution, and the hoard symbol feeds the meter.
                if self.symbols.is_wild(index)
                    || self.symbols.is_scatter(index)
                    || Some(index) == self.symbols.hoard()
                {
                    return Err(format!("refine must not burn '{}'", id));
                }
            }
        }
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
        // Every shape of the free-spin feature must be worth the same (§5.64).
        // This is the one rule the whole choice rests on: if two shapes differ
        // in expectation then picking one is picking a better game, and the
        // measured RTP depends on what the player pressed.
        if let Some(first) = self.freespins.shapes.first() {
            for shape in &self.freespins.shapes {
                if shape.value() != first.value() {
                    return Err(format!(
                        "free-spin shape '{}' is worth {} against '{}' at {} — every                          shape must trade spins for multiplier exactly",
                        shape.id,
                        shape.value(),
                        first.id,
                        first.value()
                    ));
                }
            }
            if self.freespins.refine.is_some() {
                return Err(
                    "a refining cabinet cannot offer free-spin shapes: burning symbols makes a                      late spin worth more than an early one, so trading spins for multiplier                      would move the return"
                        .to_owned(),
                );
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
        // A seam trigger the grid cannot hold is the same unreachable-feature
        // fault as the one above, and the sim would measure a game without it
        // rather than complain.
        if self.seam.trigger_count == 0 || self.seam.trigger_count > cells {
            return Err(format!(
                "seam trigger_count ({}) must fit on a {}-cell grid",
                self.seam.trigger_count, cells
            ));
        }
        if self.seam.rites.is_empty() {
            return Err("seam.json declared no rites".to_owned());
        }
        if self.seam.steps == 0 {
            return Err("a seam with no steps would never work the board".to_owned());
        }
        if self.seam.max_multiple <= 0 {
            return Err("the seam ceiling must leave something to win".to_owned());
        }
        // Two rungs is the minimum an enrichment can climb. A cabinet whose
        // paytable left only one payable symbol would make the rite a no-op,
        // and it would look exactly like a feature that never triggered.
        if self.seam_ladder_len() < 2 {
            return Err(
                "a seam needs at least two payable symbols to have a ladder to climb".to_owned(),
            );
        }
        Ok(())
    }
}

impl GameData {
    /// Count the payable, non-special symbols available to a seam. This is a
    /// data invariant: the loader only needs to know that enrichment has at
    /// least two rungs, not how the engine later works a seam.
    fn seam_ladder_len(&self) -> usize {
        self.symbols
            .iter()
            .filter(|(index, _)| {
                !self.symbols.is_wild(*index)
                    && !self.symbols.is_scatter(*index)
                    && Some(*index) != self.symbols.hoard()
                    && self.symbols.pay(*index, MAX_RUN) > 0
            })
            .count()
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
mod tests;
