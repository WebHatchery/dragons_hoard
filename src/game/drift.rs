//! Ten thousand arbitrary presses (§5.76).
//!
//! # What every audit so far has in common
//!
//! §5.37 checks a screen's layout. §5.50 sweeps every screen, one at a time.
//! §5.52 watches a spin, frame by frame. §5.47 measures one panel's collisions.
//! Every one of them looks at the game **in a state somebody arranged**.
//!
//! Nobody has ever asked what happens when a player does ten thousand arbitrary
//! things in a row. That is the state a real player is in constantly: opening
//! the ledger mid-spin, changing cabinet with a gamble offered, pressing Buy
//! while a card is up, switching the ante on and off between autospins. The
//! game is a state machine with twenty-one screens, fifty-one actions and a
//! reel animation, and the combinations are the part nothing has been near.
//!
//! # Deterministic, or it is not a bug report
//!
//! A fuzzer that finds a fault it cannot reproduce has found nothing. So the
//! presses come from a seeded generator, the frame step is fixed, and a failure
//! prints the seed and the last twenty actions. Re-running with that seed
//! replays the same evening exactly.
//!
//! That also means `Load` is not pressed. Everything else here is a function of
//! the seed; reading whatever save happens to be on the machine is not, and one
//! non-deterministic action would cost the whole property.
//!
//! # What it is actually checking
//!
//! Four things, and the last is the reason it exists.
//!
//! - **The balance never goes negative.** A stake taken twice, or a bet raised
//!   between committing and settling, would show here.
//! - **A payout never exceeds what the cabinet can pay.** A ceiling rather than
//!   a value, because the point is to catch an overflow or a doubled credit,
//!   not to re-measure the maths the RTP harness already measures.
//! - **Every committed spin still verifies** (§5.74). The strongest available
//!   statement that no sequence of presses corrupted a spin in flight: the
//!   proof log is re-run against the engine at the end, and an action that
//!   quietly rewrote a decided grid would break it.
//! - **The game never soft-locks.** After any sequence of presses, closing
//!   every screen and letting the animation settle must return the player to a
//!   cabinet they can spin — or to the ruin screen with a lifeline on offer,
//!   which is the one legitimate way to be unable to spin (§5.53).
//!
//! The soft-lock check is the one worth having. It is the fault §5.27 found by
//! hand on the Vault Pick — an open board holds the game, and without a
//! keyboard there was no way to clear it — generalised into something that
//! cannot be reintroduced quietly.

use super::Game;
use crate::game::screens::Screen;
use crate::state::limits::Cap;
use crate::ui::UiAction;
use macroquad_toolkit::rng::SeededRng;
use std::collections::BTreeSet;

/// Fixed step. Real frame times would make the run depend on how busy the
/// machine was, which is the same objection as reading the disk.
const STEP: f32 = 1.0 / 60.0;

/// How long the harness waits between presses, in frames, drawn per press.
///
/// A single gap is the wrong model in both directions. At seven frames — an
/// eighth of a second — four in five Spin presses land while the reels are
/// still turning and are refused, which tests the refusal path very thoroughly
/// and the game hardly at all. At ninety frames nothing is ever pressed during
/// an animation, which is the half worth testing.
///
/// So it draws: mostly impatient, sometimes long enough for the spin to land.
/// That is also closer to what a person does.
const GAPS: [usize; 6] = [2, 7, 7, 20, 70, 110];

/// How many presses of "close everything" the settle check is allowed before it
/// calls the game stuck. Generous: the deepest legitimate stack is a
/// celebration over a bonus board over an open panel.
const SETTLE_PRESSES: usize = 48;

/// Frames the settle check may advance while waiting for the reels to stop.
const SETTLE_FRAMES: usize = 2_000;

/// Consecutive presses the player may spend unable to spin before the run calls
/// the game locked.
///
/// The end-of-run settle check only ever looks at the *last* state, so a lock
/// that a later random press happened to escape was invisible to it — which is
/// how a deliberately broken gamble round passed a four-thousand-press run
/// untouched. This is the continuous version. Three hundred presses is many
/// seconds of game time and a dozen chances to resolve whatever is holding the
/// reels; a player who has not spun in that long is not choosing to wait.
const LOCKED_AFTER: usize = 300;

pub struct DriftConfig {
    pub steps: usize,
    pub seed: u64,
}

impl DriftConfig {
    /// `DRAGONS_HOARD_DRIFT=<steps>` runs the harness; `DRAGONS_HOARD_DRIFT_SEED`
    /// replays a particular evening.
    pub fn from_env() -> Option<Self> {
        let steps: usize = std::env::var("DRAGONS_HOARD_DRIFT").ok()?.parse().ok()?;
        let seed = std::env::var("DRAGONS_HOARD_DRIFT_SEED")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0x0D21_F7A3_5C18_9E42);
        Some(Self { steps, seed })
    }
}

/// Everything the run remembers, so a failure can describe itself.
pub struct Drift {
    rng: SeededRng,
    seed: u64,
    /// The last few presses, for the report. A whole ten-thousand-press log
    /// would be unreadable and the tail is what matters.
    recent: Vec<&'static str>,
    /// Every distinct action pressed, so the run can say what it never tried.
    pressed: BTreeSet<&'static str>,
    presses: usize,
    /// Spins the run has actually got through.
    ///
    /// Accumulated from positive deltas rather than read off the session at the
    /// end, because `NewGame` is one of the presses and it resets that counter.
    /// The first version reported five spins in ten thousand presses and was
    /// measuring its own resets — a probe that the thing being probed can zero
    /// is not a measurement.
    spins: u64,
    resets: usize,
    last_seen_spins: u64,
    /// Presses since the player could last spin, and the worst run seen.
    unable: usize,
    worst_unable: usize,
    spins_at_last_check: u64,
}

impl Drift {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SeededRng::new(seed),
            seed,
            recent: Vec::new(),
            pressed: BTreeSet::new(),
            presses: 0,
            spins: 0,
            resets: 0,
            last_seen_spins: 0,
            unable: 0,
            worst_unable: 0,
            spins_at_last_check: 0,
        }
    }

    /// A press in the catalogue that never came up, if any.
    ///
    /// A run that never tried an action has not tested it, and the honest
    /// report says so rather than counting it as covered. It is also how a
    /// press that becomes unreachable — a chooser arm that can no longer fire —
    /// stops being silently dropped.
    fn never_pressed(&self) -> Option<String> {
        let missing: Vec<&str> = PANELS
            .iter()
            .chain(WAGER.iter())
            .chain(SETTINGS.iter())
            .chain(DESTRUCTIVE.iter())
            .chain(PARAMETERISED.iter())
            .chain(std::iter::once(&UiAction::Spin))
            .map(|action| action.name())
            .filter(|name| !self.pressed.contains(name))
            .collect();
        if missing.is_empty() {
            None
        } else {
            Some(missing.join(", "))
        }
    }

    /// Fold in what the game has done since the last press.
    fn observe(&mut self, spins: u64) {
        if spins >= self.last_seen_spins {
            self.spins += spins - self.last_seen_spins;
        } else {
            // The counter went backwards, which only happens when something
            // reset the session.
            self.spins += spins;
            self.resets += 1;
        }
        self.last_seen_spins = spins;
    }

    /// Note whether the game is going anywhere, and say so when it has not for
    /// too long.
    ///
    /// "Going anywhere" has to include *a spin in flight*. The first version
    /// asked only whether the player could press Spin right now, which is false
    /// for most of every second the reels are turning — so the streak never
    /// reset and the harness reported a lock three hundred presses into a run
    /// that had just completed sixty-six spins. Busy is not locked.
    fn watch_for_a_lock(&mut self, playable: bool) -> Option<String> {
        let progressed = self.spins > self.spins_at_last_check;
        self.spins_at_last_check = self.spins;
        if playable || progressed {
            self.unable = 0;
            return None;
        }
        self.unable += 1;
        self.worst_unable = self.worst_unable.max(self.unable);
        (self.unable >= LOCKED_AFTER).then(|| {
            format!(
                "the reels have not turned for {} presses and the player is not out of credits",
                self.unable
            )
        })
    }

    fn gap(&mut self) -> usize {
        GAPS[self.rng.below(GAPS.len())]
    }

    fn note(&mut self, action: &UiAction) {
        let name = action.name();
        self.pressed.insert(name);
        self.recent.push(name);
        if self.recent.len() > 20 {
            self.recent.remove(0);
        }
        self.presses += 1;
    }
}

/// The presses this harness knows how to make.
///
/// Deliberately *not* filtered by what is currently sensible. Half the point is
/// pressing Buy during a gamble and Spin with a card up — a chooser that only
/// offered legal moves would test the paths that already work.
///
/// `Load` is absent and that is the one deliberate hole: it reads whatever save
/// is on the machine, which would make the run depend on something other than
/// the seed. Everything else in [`UiAction`] is here, and
/// [`UiAction::name`]'s exhaustive match is what stops a new one being added
/// without somebody standing in front of this list.
///
/// Spin appears four times because weighting is the only thing separating a
/// harness that plays a slot machine from one that opens menus.
const PANELS: &[UiAction] = &[
    UiAction::TogglePaytable,
    UiAction::ToggleSettings,
    UiAction::ToggleMachines,
    UiAction::ToggleAchievements,
    UiAction::ToggleFeatureBuy,
    UiAction::ToggleLedger,
    UiAction::ToggleLines,
    UiAction::ToggleMenu,
    UiAction::ToggleProofs,
    UiAction::CheckProofs,
    UiAction::ToggleSessions,
    UiAction::DismissSessionOver,
    UiAction::ToggleRules,
    UiAction::ToggleHistory,
    UiAction::ToggleLimits,
    UiAction::ToggleWaveforms,
    UiAction::ToggleVision,
    UiAction::AcknowledgeRealityCheck,
    UiAction::DismissHint,
    UiAction::DismissCelebration,
    UiAction::OfferGamble,
    UiAction::TakeGamble,
];

/// The wager controls, and the ante.
const WAGER: &[UiAction] = &[
    UiAction::BetUp,
    UiAction::BetDown,
    UiAction::MaxBet,
    UiAction::ToggleAutospin,
    UiAction::ToggleAnte,
    UiAction::CycleAutospinLength,
];

/// Settings, which a player touches once an evening.
const SETTINGS: &[UiAction] = &[
    UiAction::VolumeUp,
    UiAction::VolumeDown,
    UiAction::MusicVolumeUp,
    UiAction::MusicVolumeDown,
    UiAction::CycleTextScale,
    UiAction::CycleSpinSpeed,
    UiAction::CycleRealityCheck,
    UiAction::ToggleShake,
    UiAction::ToggleParticles,
];

/// The presses that throw the game away.
///
/// Held to one press in a hundred, and that is a measurement rather than
/// taste. Flat across the whole catalogue they came up 154 times in ten
/// thousand presses — a new game every minute — and the run managed 461 spins,
/// which is a harness testing its own reset path.
const DESTRUCTIVE: &[UiAction] = &[
    UiAction::NewGame,
    UiAction::DeleteSave,
    UiAction::Save,
    UiAction::TakeLifeline,
];

/// The presses that carry a payload, named here so the coverage report can ask
/// about them too. The payloads themselves are drawn in [`Drift::choose`].
const PARAMETERISED: &[UiAction] = &[
    UiAction::SelectMachine(0),
    UiAction::BuyFeature(0),
    UiAction::PickBonus(0),
    UiAction::ChooseFreeSpinShape(0),
    UiAction::Gamble(crate::state::gamble::Scale::Ember),
    UiAction::GambleHalf(crate::state::gamble::Scale::Ember),
    UiAction::CycleLimit(Cap::Loss),
    UiAction::OpenScreen(Screen::Paytable),
];

impl Drift {
    /// Draw the next press.
    ///
    /// `board` is how many chests are on the table, and it is the only thing
    /// the chooser knows about the game. A real UI only ever emits presses for
    /// controls it actually drew, so this is less of a concession than it looks.
    ///
    /// Weighted like a player rather than uniformly across the catalogue: two
    /// spins in five, a panel now and then, a setting rarely, and the
    /// destructive presses at one in a hundred. Uniform weighting produced a
    /// run that reset the game 154 times and span 461 reels out of ten thousand
    /// presses, which is not an evening anyone has ever had.
    ///
    /// Every band is still reachable every press. The weighting decides how
    /// much of the run each one gets, not whether it happens.
    fn choose(&mut self, board: usize) -> UiAction {
        // Payload indices are deliberately allowed out of range some of the
        // time: "buy the eighth of three features" is exactly the sort of press
        // a misbehaving UI could produce and a dispatcher should survive.
        // A board on the table changes what a person does next. Facing twelve
        // chests they click chests; they do not wander off into the sound
        // settings. Without this the harness picked about twelve times in three
        // hundred presses, which is nowhere near enough to turn over three
        // blanks among twelve cells, and it reported the game locked when it
        // was only being ignored.
        if board > 0 && self.rng.below(10) < 7 {
            return UiAction::PickBonus(self.rng.below(board));
        }

        match self.rng.below(100) {
            0 => DESTRUCTIVE[self.rng.below(DESTRUCTIVE.len())],
            1..=40 => UiAction::Spin,
            41..=44 => UiAction::SelectMachine(self.rng.below(crate::data::MACHINES.len() + 1)),
            45..=47 => UiAction::BuyFeature(self.rng.below(6)),
            // Mostly a chest that exists, sometimes one that does not. Drawing
            // uniformly from a range wider than any board meant most picks
            // landed on nothing, and a Vault Pick sat open for three hundred
            // presses while the harness reported the game locked. A fuzzer that
            // only ever presses buttons which are not there tests nothing — but
            // one that never presses them tests nothing either.
            48..=51 => UiAction::PickBonus(if board > 0 && self.rng.below(10) > 0 {
                self.rng.below(board)
            } else {
                self.rng.below(14)
            }),
            52..=54 => UiAction::ChooseFreeSpinShape(self.rng.below(4)),
            55..=58 => {
                let scale = if self.rng.below(2) == 0 {
                    crate::state::gamble::Scale::Ember
                } else {
                    crate::state::gamble::Scale::Ash
                };
                if self.rng.below(2) == 0 {
                    UiAction::Gamble(scale)
                } else {
                    UiAction::GambleHalf(scale)
                }
            }
            59..=60 => UiAction::CycleLimit(match self.rng.below(3) {
                0 => Cap::Time,
                1 => Cap::Loss,
                _ => Cap::Spins,
            }),
            61..=63 => {
                let screens: Vec<Screen> = Screen::in_menu().collect();
                UiAction::OpenScreen(screens[self.rng.below(screens.len())])
            }
            64..=71 => WAGER[self.rng.below(WAGER.len())],
            72..=76 => SETTINGS[self.rng.below(SETTINGS.len())],
            _ => PANELS[self.rng.below(PANELS.len())],
        }
    }
}

/// Why the run stopped, when it stopped badly.
struct Broken {
    step: usize,
    what: String,
}

impl Game {
    /// Play the game at random and check it never breaks (§5.76).
    ///
    /// Returns the exit code. Prints a report either way, because a run that
    /// says nothing is indistinguishable from one that did not happen.
    pub fn drift(&mut self, config: &DriftConfig) -> i32 {
        let mut drift = Drift::new(config.seed);
        let ceiling = self.payout_ceiling();
        let mut broken: Option<Broken> = None;

        for step in 0..config.steps {
            let board = self
                .session
                .bonus
                .as_ref()
                .map_or(0, |round| round.board_size());
            let action = drift.choose(board);
            drift.note(&action);
            self.apply_action(action);
            for _ in 0..drift.gap() {
                self.update(STEP);
            }
            drift.observe(self.session.stats.total_spins);

            if let Some(what) = self.violation(ceiling) {
                broken = Some(Broken { step, what });
                break;
            }
            // A lock is the game holding the reels, and nothing else.
            //
            // If the machine is *settled* then a Spin press works, or is
            // refused for a reason the player can undo — lower the bet, take
            // the lifeline, start a new session. The drift presses Spin two
            // times in five, so a settled game that is not spinning is a game
            // being told not to. What cannot be undone by pressing anything is
            // an open board, a gamble or a card that never clears, and those
            // all read as unsettled. That is the fault §5.27 found on the Vault
            // Pick by hand.
            //
            // The first three attempts at this predicate were all wrong in the
            // same direction — too strict — and each reported a lock in a game
            // that was working: once because a spin in flight is not settled,
            // once because a session cap had stopped the reels on purpose, once
            // because the player had been left on max bet.
            let playable = self.session.is_settled();
            if let Some(what) = drift.watch_for_a_lock(playable) {
                broken = Some(Broken { step, what });
                break;
            }
        }

        if broken.is_none() {
            if let Some(what) = self.stuck() {
                broken = Some(Broken {
                    step: config.steps,
                    what,
                });
            }
        }
        if broken.is_none() {
            if let Some(what) = self.proofs_still_hold() {
                broken = Some(Broken {
                    step: config.steps,
                    what,
                });
            }
        }

        // What the run actually did, not just that it finished. A harness that
        // pressed ten thousand buttons and never span would report "nothing
        // broke" while having tested nothing — the clean run is the one that
        // has to justify itself.
        println!(
            "drift: {} presses, {} distinct, seed {} | {} spins, {} resets, \
             longest wait {}, balance {}",
            drift.presses,
            drift.pressed.len(),
            drift.seed,
            drift.spins,
            drift.resets,
            drift.worst_unable,
            self.session.balance,
        );
        // Coverage and the spin floor describe a *complete* run. A run that
        // stopped at press one has not failed to press the other forty-nine
        // buttons; it has found something, and burying that under a list of
        // fifty names is how a report stops being read.
        if broken.is_none() {
            if let Some(missed) = drift.never_pressed() {
                println!("drift: BROKEN — never pressed {}", missed);
                return 1;
            }
            // A twentieth of the presses, at minimum.
            //
            // Not a tenth, and the difference is a measurement rather than a
            // rounding. Most Spin presses are *correctly* refused because the
            // reels are still turning — the harness presses faster than a spin
            // takes, on purpose — so the ratio of spins to presses is capped
            // well below one by the game behaving properly. What this floor
            // catches is the run that stopped playing altogether.
            let floor = (config.steps / 20) as u64;
            if drift.spins < floor {
                println!(
                    "drift: BROKEN — only {} spins in {} presses, under the {} floor",
                    drift.spins, config.steps, floor
                );
                return 1;
            }
        }

        match broken {
            None => {
                println!("drift: nothing broke");
                0
            }
            Some(Broken { step, what }) => {
                println!("drift: BROKEN at press {} — {}", step, what);
                println!("drift: state — {}", self.describe());
                println!("drift: replay with DRAGONS_HOARD_DRIFT_SEED={}", drift.seed);
                println!("drift: last presses — {}", drift.recent.join(", "));
                1
            }
        }
    }

    /// Everything about the current state that could explain a stall.
    ///
    /// A report that says "the reels have not turned" and nothing else sends
    /// the reader back to the seed to find out why. This is the difference
    /// between a harness that finds bugs and one that announces them.
    fn describe(&self) -> String {
        let open: Vec<&str> = Screen::ALL
            .into_iter()
            .filter(|screen| self.screen_open(*screen))
            .map(Screen::id)
            .collect();
        format!(
            "cabinet {} | settled {} | balance {} | staked {} | bet {} | cheapest {} | \
             free {:?} | bonus {} | wrath {} | gamble {} | card {} | autospin {} | \
             breach {:?} | open [{}]",
            self.data.machine_id(),
            self.session.is_settled(),
            self.session.balance,
            self.session.staked(&self.data),
            self.session.line_bet_index,
            self.session.cheapest_spin(&self.data),
            self.session
                .free_spins
                .as_ref()
                .map(|state| state.remaining),
            self.session.bonus.is_some(),
            self.session.holdspin.is_some(),
            self.session.gamble.is_some(),
            self.session.celebrations.is_active(),
            self.session.autospin_remaining(),
            self.limits.breach(),
            open.join(" ")
        )
    }

    /// The most a single round could pay, with room to spare.
    ///
    /// A ceiling rather than a figure: this is looking for a credit invented by
    /// an overflow or a doubled payout, and the RTP harness already measures
    /// what the cabinet really returns.
    fn payout_ceiling(&self) -> i64 {
        self.data.config.starting_balance.saturating_mul(10_000)
    }

    /// Anything wrong with the game as it stands.
    fn violation(&self, ceiling: i64) -> Option<String> {
        if self.session.balance < 0 {
            return Some(format!("the balance is {}", self.session.balance));
        }
        if self.session.last_win > ceiling {
            return Some(format!(
                "a spin paid {}, over the {} ceiling",
                self.session.last_win, ceiling
            ));
        }
        if self.session.balance > ceiling {
            return Some(format!("the balance reached {}", self.session.balance));
        }
        None
    }

    /// Can the player get back to a spinnable cabinet from here?
    ///
    /// Closes everything and lets the animation run out. Being unable to spin is
    /// only legitimate in one place — out of credits, with a lifeline offered
    /// (§5.53) — and anything else is the soft-lock this exists to catch.
    fn stuck(&mut self) -> Option<String> {
        for _ in 0..SETTLE_PRESSES {
            if self.session.is_settled() && !self.any_overlay_open() {
                break;
            }
            self.apply_action(UiAction::DismissCelebration);
            self.apply_action(UiAction::DismissSessionOver);
            // The screens the game *deals* rather than the player opens
            // (§5.50). Closing panels is not enough: a gamble in flight and an
            // open Vault Pick both hold the reels, and each has exactly one
            // press that resolves it.
            self.apply_action(UiAction::TakeGamble);
            if let Some(index) = self
                .session
                .bonus
                .as_ref()
                .and_then(|round| round.next_unrevealed())
            {
                self.apply_action(UiAction::PickBonus(index));
            }
            // `close_screens`, not a loop of `OpenScreen`. `OpenScreen` is the
            // menu's action and it *sets* the flag — the first version of this
            // opened every screen in the registry while believing it was
            // shutting them, and then reported the game stuck with eight panels
            // up that it had raised itself.
            self.close_screens();
            for _ in 0..GAPS[0] {
                self.update(STEP);
            }
        }

        // A spin in flight is not stuck, it is busy. Let it land.
        let mut frames = 0;
        while !self.session.is_settled() && frames < SETTLE_FRAMES {
            self.update(STEP);
            frames += 1;
        }

        // And only then come down off the bet.
        //
        // A player left on max bet with a small balance cannot spin, but they
        // are not stuck — they press the minus button. Leaving this out made
        // the harness report a lock on a player holding 36 credits against a
        // cheapest spin of 20, whose last press had been Max Bet.
        //
        // It has to come *after* the reels stop. The bet controls are locked
        // mid-spin, on purpose, because the stake is already committed — so the
        // first version's sixty-four presses were every one of them refused,
        // and the report said "stuck on a stake of 250 with 220 credits" when
        // one press of the minus button was the whole answer. The order of a
        // two-step recovery is part of the recovery.
        for _ in 0..64 {
            if self.session.can_spin(&self.data) {
                break;
            }
            self.apply_action(UiAction::BetDown);
        }

        if self.session.can_spin(&self.data) {
            return None;
        }
        if self.session.is_ruined(&self.data) {
            return None;
        }
        Some(format!(
            "the player cannot spin and is not out of credits — balance {}, settled {}, \
             overlay {}",
            self.session.balance,
            self.session.is_settled(),
            self.any_overlay_open()
        ))
    }

    /// Every spin the game committed to still re-runs identically (§5.74).
    fn proofs_still_hold(&self) -> Option<String> {
        self.proofs
            .verify_all()
            .into_iter()
            .find(|(_, verdict)| !verdict.is_match())
            .map(|(entry, verdict)| {
                format!(
                    "spin {} no longer verifies: {}",
                    entry.seq,
                    verdict.message()
                )
            })
    }
}
