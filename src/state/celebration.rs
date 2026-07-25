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
    FreeSpinsEntry { spins: u32, scatters: usize },
    FreeSpinsRetrigger { spins: u32 },
    FreeSpinsSummary { spins: u32, won: i64 },
    Hatch { credits: i64, eggs: u32 },
    Jackpot { name: String, credits: i64 },
    BigWin { credits: i64 },
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
        }
    }

    pub fn title(&self) -> String {
        match self {
            CelebrationKind::FreeSpinsEntry { spins, .. } => format!("{} FREE SPINS", spins),
            CelebrationKind::FreeSpinsRetrigger { spins } => format!("+{} FREE SPINS", spins),
            CelebrationKind::FreeSpinsSummary { won, .. } => format!("{} CREDITS", won),
            CelebrationKind::Hatch { credits, .. } => format!("{} CREDITS", credits),
            CelebrationKind::Jackpot { credits, .. } => format!("{} CREDITS", credits),
            CelebrationKind::BigWin { credits } => format!("{} CREDITS", credits),
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
mod tests {
    use super::*;

    fn entry() -> CelebrationKind {
        CelebrationKind::FreeSpinsEntry {
            spins: 10,
            scatters: 3,
        }
    }

    fn hatch() -> CelebrationKind {
        CelebrationKind::Hatch {
            credits: 500,
            eggs: 15,
        }
    }

    #[test]
    fn the_first_card_shows_immediately() {
        let mut queue = CelebrationQueue::default();
        assert!(!queue.is_active());

        queue.push(entry());

        assert!(queue.is_active());
        assert_eq!(queue.active().unwrap().kind(), &entry());
        assert_eq!(queue.update(0.0), Some(entry()));
    }

    #[test]
    fn skipping_never_leaves_the_game_briefly_unheld() {
        let mut queue = CelebrationQueue::default();
        queue.push(entry());
        queue.push(hatch());

        queue.skip();

        // The next card must already be up: a single frame with no card showing
        // would let the reels advance underneath the sequence.
        assert!(queue.is_active());
    }

    #[test]
    fn every_card_announces_itself_exactly_once_in_order() {
        let mut queue = CelebrationQueue::default();
        queue.push(entry());
        queue.push(hatch());

        let mut opened = Vec::new();
        for _ in 0..1200 {
            if let Some(kind) = queue.update(1.0 / 60.0) {
                opened.push(kind);
            }
        }

        // The first card is made active by `push`, so its event has to come out
        // of `update` too — otherwise it would never get its particles.
        assert_eq!(opened, vec![entry(), hatch()]);
        assert!(!queue.is_active());
    }

    #[test]
    fn the_queue_empties_once_every_card_has_run() {
        let mut queue = CelebrationQueue::default();
        queue.push(entry());

        for _ in 0..600 {
            queue.update(1.0 / 60.0);
        }

        assert!(!queue.is_active());
    }

    #[test]
    fn skipping_advances_to_the_next_card() {
        let mut queue = CelebrationQueue::default();
        queue.push(entry());
        queue.push(hatch());

        queue.skip();
        assert_eq!(queue.active().unwrap().kind(), &hatch());

        queue.skip();
        assert!(!queue.is_active());
    }

    #[test]
    fn the_backlog_is_bounded_so_a_headless_run_cannot_grow_it() {
        let mut queue = CelebrationQueue::default();
        for _ in 0..10_000 {
            queue.push(hatch());
        }

        assert!(queue.pending.len() <= MAX_QUEUED);
    }

    #[test]
    fn a_card_fades_in_and_out_and_is_solid_in_between() {
        let mut queue = CelebrationQueue::default();
        queue.push(entry());
        // The first update spends itself announcing the card, not ticking it.
        queue.update(0.0);

        assert!(queue.active().unwrap().alpha() < 0.1, "should start faded");

        // Halfway through it should be fully opaque and at rest.
        let half = entry().duration() * 0.5;
        queue.update(half);
        let card = queue.active().unwrap();
        assert_eq!(card.alpha(), 1.0);
        assert_eq!(card.scale(), 1.0);

        // Just before the end it is fading again.
        queue.update(entry().duration() * 0.5 - FADE_TIME * 0.5);
        assert!(queue.active().unwrap().alpha() < 1.0);
    }

    #[test]
    fn a_long_card_still_reaches_full_opacity_quickly() {
        let mut queue = CelebrationQueue::default();
        // The hatch card is the longest one; it must not spend a second of it
        // half-transparent with the reels legible through the middle.
        queue.push(hatch());
        queue.update(0.0);
        queue.update(FADE_TIME);

        assert_eq!(queue.active().unwrap().alpha(), 1.0);
    }
}
