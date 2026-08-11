//! Full-screen celebration cards — the difference between a feature that
//! *happens* and one the player *notices*.
//!
//! A celebration holds the game: while one is showing the reels do not turn, the
//! payout does not count, and free spins do not chain. That is deliberate. The
//! free-spin trigger and the Hatch are the two moments the whole game is built
//! around, and letting the auto-chain run underneath a banner would bury them.

use macroquad_toolkit::timing::Timer;
use std::collections::VecDeque;

/// Cards waiting behind the current one. Bounded because the headless sim
/// settles a million spins and never drains the queue.
const MAX_QUEUED: usize = 3;
/// Seconds a card spends fading in, and again fading out. A fixed time rather
/// than a fraction of the duration: the long cards were reaching the eye still
/// half-transparent, with the reels legible straight through them.
const FADE_TIME: f32 = 0.2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CelebrationKind {
    FreeSpinsEntry {
        spins: u32,
        scatters: usize,
    },
    FreeSpinsRetrigger {
        spins: u32,
    },
    FreeSpinsSummary {
        spins: u32,
        won: i64,
    },
    Hatch {
        credits: i64,
        eggs: u32,
    },
    Jackpot {
        name: String,
        credits: i64,
    },
    BigWin {
        credits: i64,
    },
    /// The Dragon's Wrath respin round paid out (§5.12).
    Wrath {
        credits: i64,
        coins: usize,
        full_board: bool,
    },
    /// A seam finished working the board (§5.80).
    Seam {
        credits: i64,
        /// The rite that ran, in the words the cabinet uses for it.
        rite: String,
        /// Cells the seam held at the end.
        cells: usize,
        /// Why it came to nothing, when it did — `None` when it paid (§5.85).
        ///
        /// A seam can end empty and it is not rare: measured across the
        /// catalog, between 0.8% and 58% of them do, depending on the cabinet
        /// and the rite the player took. Those need the same beat of their own
        /// that a busted gamble does, for the same reason — a card headed
        /// "THE SEAM RUNS" over a total of nothing reads as a bug, and one that
        /// showers gold over it reads as a taunt.
        dry: Option<&'static str>,
    },
    /// A gamble busted (§5.16). Losing needs a beat of its own — without one
    /// the win simply vanishes from the readout and reads as a bug.
    GambleLost {
        lost: i64,
        landed: &'static str,
    },
}

impl CelebrationKind {
    fn duration(&self) -> f32 {
        match self {
            CelebrationKind::FreeSpinsEntry { .. } => 2.4,
            CelebrationKind::FreeSpinsRetrigger { .. } => 1.5,
            CelebrationKind::FreeSpinsSummary { .. } => 2.6,
            CelebrationKind::Hatch { .. } => 2.8,
            CelebrationKind::Jackpot { .. } => 3.4,
            CelebrationKind::BigWin { .. } => 1.9,
            CelebrationKind::GambleLost { .. } => 1.8,
            // A dry seam is over quickly, like a busted gamble. There is
            // nothing to count and nothing to look at.
            CelebrationKind::Seam { dry: Some(_), .. } => 1.8,
            CelebrationKind::Seam { .. } => 2.4,
            CelebrationKind::Wrath { full_board, .. } => {
                if *full_board {
                    3.6
                } else {
                    2.8
                }
            }
        }
    }

    pub fn title(&self) -> String {
        match self {
            CelebrationKind::FreeSpinsEntry { spins, .. } => format!("{} FREE SPINS", spins),
            CelebrationKind::FreeSpinsRetrigger { spins } => format!("+{} FREE SPINS", spins),
            CelebrationKind::FreeSpinsSummary { won, .. } => {
                format!("{} CREDITS", crate::ui::naming::credits(*won))
            }
            CelebrationKind::Hatch { credits, .. } => {
                format!("{} CREDITS", crate::ui::naming::credits(*credits))
            }
            CelebrationKind::Jackpot { credits, .. } => {
                format!("{} CREDITS", crate::ui::naming::credits(*credits))
            }
            CelebrationKind::BigWin { credits } => {
                format!("{} CREDITS", crate::ui::naming::credits(*credits))
            }
            CelebrationKind::GambleLost { .. } => "NOTHING".to_owned(),
            CelebrationKind::Seam { dry: Some(_), .. } => "NOTHING".to_owned(),
            CelebrationKind::Seam { credits, .. } => {
                format!("{} CREDITS", crate::ui::naming::credits(*credits))
            }
            CelebrationKind::Wrath { credits, .. } => {
                format!("{} CREDITS", crate::ui::naming::credits(*credits))
            }
        }
    }

    pub fn heading(&self) -> &'static str {
        match self {
            CelebrationKind::FreeSpinsEntry { .. } => "THE DRAGON STIRS",
            CelebrationKind::FreeSpinsRetrigger { .. } => "RETRIGGER",
            CelebrationKind::FreeSpinsSummary { .. } => "FREE SPINS COMPLETE",
            CelebrationKind::Hatch { .. } => "THE HOARD HATCHES",
            CelebrationKind::Jackpot { .. } => "JACKPOT",
            CelebrationKind::BigWin { .. } => "BIG WIN",
            CelebrationKind::GambleLost { .. } => "THE SCALE TURNS",
            CelebrationKind::Seam { dry: Some(_), .. } => "THE SEAM RUNS DRY",
            CelebrationKind::Seam { .. } => "THE SEAM RUNS",
            CelebrationKind::Wrath { full_board, .. } => {
                if *full_board {
                    "THE HOARD IS YOURS"
                } else {
                    "THE DRAGON'S WRATH"
                }
            }
        }
    }

    pub fn subtitle(&self) -> String {
        match self {
            CelebrationKind::FreeSpinsEntry { scatters, .. } => {
                format!("{} scatters — wilds expand, line wins doubled", scatters)
            }
            CelebrationKind::FreeSpinsRetrigger { .. } => "More scatters, more spins".to_owned(),
            CelebrationKind::FreeSpinsSummary { spins, .. } => {
                format!("won over {} free spins", spins)
            }
            CelebrationKind::Hatch { eggs, .. } => format!("{} dragon eggs cashed in", eggs),
            CelebrationKind::Jackpot { name, .. } => format!("the {} progressive falls", name),
            CelebrationKind::BigWin { .. } => "The vault gives up its gold".to_owned(),
            CelebrationKind::GambleLost { lost, landed } => {
                format!("{} landed — {} credits gone", landed, lost)
            }
            CelebrationKind::Seam {
                rite,
                cells,
                dry: Some(why),
                ..
            } => format!("{} — {}, and {} cells came to nothing", rite, why, cells),
            CelebrationKind::Seam { rite, cells, .. } => {
                format!("{} — {} cells of one treasure", rite, cells)
            }
            CelebrationKind::Wrath {
                coins, full_board, ..
            } => {
                if *full_board {
                    "every cell filled — the full board bonus".to_owned()
                } else {
                    format!("{} coins locked from the dragon's hoard", coins)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Celebration {
    kind: CelebrationKind,
    timer: Timer,
    /// Whether the orchestrator has been told this card opened. Cards are made
    /// active the moment they are pushed, but the "it opened" event has to come
    /// out of `update`, so the flag carries it across.
    announced: bool,
}

impl Celebration {
    fn new(kind: CelebrationKind) -> Self {
        let timer = Timer::new(kind.duration());
        Self {
            kind,
            timer,
            announced: false,
        }
    }

    pub fn kind(&self) -> &CelebrationKind {
        &self.kind
    }

    /// Opacity envelope: fade in, hold, fade out.
    pub fn alpha(&self) -> f32 {
        let elapsed = self.timer.elapsed();
        let remaining = self.timer.duration() - elapsed;
        (elapsed / FADE_TIME)
            .min(remaining / FADE_TIME)
            .clamp(0.0, 1.0)
    }

    /// Card scale, overshooting slightly as it opens.
    pub fn scale(&self) -> f32 {
        let t = (self.timer.elapsed() / FADE_TIME).clamp(0.0, 1.0);
        if t >= 1.0 {
            return 1.0;
        }
        0.86 + 0.14 * macroquad_toolkit::math::ease_out_back(t)
    }

    fn finished(&self) -> bool {
        self.timer.finished()
    }
}

/// One showing card plus a short backlog.
#[derive(Debug, Clone, Default)]
pub struct CelebrationQueue {
    active: Option<Celebration>,
    pending: VecDeque<CelebrationKind>,
}

impl CelebrationQueue {
    pub fn push(&mut self, kind: CelebrationKind) {
        if self.active.is_none() {
            self.active = Some(Celebration::new(kind));
            return;
        }
        if self.pending.len() >= MAX_QUEUED {
            self.pending.pop_front();
        }
        self.pending.push_back(kind);
    }

    pub fn active(&self) -> Option<&Celebration> {
        self.active.as_ref()
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Advance the showing card. Returns the kind of a card that has just
    /// opened, so the orchestrator can fire its particles and shake — including
    /// the very first card, which `push` made active without an event.
    pub fn update(&mut self, dt: f32) -> Option<CelebrationKind> {
        let active = self.active.as_mut()?;

        if !active.announced {
            active.announced = true;
            return Some(active.kind.clone());
        }

        active.timer.tick(dt);
        if active.finished() {
            self.active = None;
            self.promote();
        }
        None
    }

    /// Cut the current card short — the player has seen it and pressed on.
    /// Promotes immediately so the game is never briefly un-held.
    pub fn skip(&mut self) {
        self.active = None;
        self.promote();
    }

    pub fn clear(&mut self) {
        self.active = None;
        self.pending.clear();
    }

    fn promote(&mut self) {
        if let Some(kind) = self.pending.pop_front() {
            self.active = Some(Celebration::new(kind));
        }
    }
}

#[cfg(test)]
mod tests;
