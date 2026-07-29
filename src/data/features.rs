//! Feature configuration: one type per system, all of it JSON.
//!
//! Split out of `data.rs` on size. There is no shared behaviour here — each of
//! these is the shape of one file under `assets/data/` — but keeping them
//! together makes it plain how much of this game is data rather than code.

use serde::{Deserialize, Serialize};

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

/// The Seam — the mini-game a board taken over by one treasure opens (§5.80).
///
/// Per-cabinet, and necessarily so: `trigger_count` is a statement about that
/// cabinet's strips and grid. Six of one symbol is a once-in-a-hundred board on
/// a 5x3 payline machine and a near-certainty on a 6x5 cluster one, so a shared
/// number would mean a feature that never fires on one cabinet and never stops
/// on another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeamConfig {
    /// Cells of one paying symbol on the resting grid that open a seam.
    pub trigger_count: usize,
    /// Beats the seam works itself out over.
    pub steps: usize,
    /// Most a single seam may pay, in multiples of total bet. The rites can
    /// take a whole grid in the tail of their distribution, and a payline
    /// cabinet pays every line it makes — without a ceiling the feature's
    /// variance would swamp the cabinet it is bolted to.
    pub max_multiple: i64,
    /// The rites, drawn by weight when a seam opens.
    pub rites: Vec<RiteDef>,
}

/// One thing a seam can do, and how often it is the thing that happens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiteDef {
    pub id: String,
    /// What the banner calls it while it runs.
    pub name: String,
    pub weight: u32,
    #[serde(flatten)]
    pub kind: RiteKind,
}

/// What a rite does to the board on each of its beats.
///
/// The two the feature was built around: a seam either takes more of the grid,
/// or it becomes worth more where it already is.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "rite", rename_all = "snake_case")]
pub enum RiteKind {
    /// The seam widens: each cell touching it converts with this chance, in
    /// permille, and the new cells can spread again on the next beat.
    Widen { spread_permille: usize },
    /// The seam deepens: every cell of it climbs this many rungs of the pay
    /// ladder at once, so what is already there is worth more.
    Enrich { rungs: usize },
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
