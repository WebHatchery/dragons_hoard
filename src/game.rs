//! High-level game loop: owns the session, routes intents, drives feedback.

mod capture_scenes;
mod feedback;
mod outcomes;
mod persistence;
mod screens;

use crate::audio::{Sfx, SoundBank};
use crate::data::GameData;
use crate::state::achievements::AchievementBook;
use crate::state::preferences::Preferences;
use crate::state::spin::SpinEvent;
use crate::state::{GameSession, SpinResolution};
use crate::ui::{self, palette, UiAction, UiContext};
use macroquad::prelude::*;
use macroquad_toolkit::assets::AssetManager;
use macroquad_toolkit::capture;
use macroquad_toolkit::events::EventBus;
use macroquad_toolkit::fx::{BurstConfig, FloatingTextLayer, ParticleSystem, ScreenShake};
use macroquad_toolkit::notifications::{
    NotificationAnchor, NotificationManager, NotificationRenderConfig,
};
use macroquad_toolkit::prelude::{begin_virtual_ui_frame, end_virtual_ui_frame};
use macroquad_toolkit::rng::random_u64;

pub struct Game {
    data: GameData,
    session: GameSession,
    notifications: NotificationManager,
    events: EventBus<UiAction>,
    shake: ScreenShake,
    particles: ParticleSystem,
    floating: FloatingTextLayer,
    sound: SoundBank,
    /// The four-track loop behind the reels (§5.31).
    music: crate::music::Music,
    /// Monotonic in-game seconds, fed to the UI so pulsing highlights stay
    /// deterministic under the fixed-timestep capture harness.
    ui_time: f32,
    show_paytable: bool,
    show_settings: bool,
    show_machines: bool,
    show_achievements: bool,
    show_featurebuy: bool,
    show_ledger: bool,
    /// The rules panel (§5.29).
    show_rules: bool,
    /// Session limits and the clock behind them (§5.30).
    limits: crate::state::limits::LimitState,
    limit_choices: crate::state::limits::LimitChoices,
    show_limits: bool,
    /// The session graph (§5.32).
    history: crate::state::history::History,
    show_history: bool,
    reality_check: bool,
    show_waveforms: bool,
    show_vision: bool,
    /// Which control the keyboard is on (§5.27). Lives here because the index
    /// has to persist and the controls do not.
    nav: ui::nav::Nav,
    /// What the game has not yet told this player (§5.28).
    hints: crate::state::hints::HintBook,
    /// Measured cabinet profiles (§5.17). Lives here rather than on the session
    /// because it describes the catalog, not one machine's play.
    profiles: crate::state::profile::ProfileBook,
    achievements: AchievementBook,
    /// What this player has actually seen, per cabinet (§5.18).
    ledger: crate::state::ledger::Ledger,
    save_exists: bool,
}

impl Game {
    pub async fn new() -> Self {
        // Load the default machine first purely to find out where preferences
        // live, then honour the cabinet the player last chose.
        let bootstrap = GameData::load()
            .unwrap_or_else(|err| panic!("Dragon's Hoard embedded data failed to load: {}", err));
        let preferences = Preferences::load(&bootstrap.config);
        let data = if preferences.machine_id.is_empty()
            || preferences.machine_id == bootstrap.machine_id()
        {
            bootstrap
        } else {
            GameData::load_machine(crate::data::machine_by_id(&preferences.machine_id))
                .unwrap_or(bootstrap)
        };

        // Fail fast: a limits file that could never bind would let a player set
        // a cap that silently did nothing (§5.30).
        let limit_choices = crate::state::limits::LimitChoices::load()
            .unwrap_or_else(|err| panic!("Dragon's Hoard limits failed to load: {}", err));

        // Symbols are drawn procedurally (see `ui::symbols`), so the manifest is
        // empty by design. It is still loaded so the asset pipeline stays wired
        // up for anything that does need a texture later.
        let mut assets = AssetManager::new();
        let placeholder = Image::gen_image_color(16, 16, Color::new(0.5, 0.4, 0.2, 1.0));
        assets.set_placeholder_texture_direct(Texture2D::from_image(&placeholder));
        let _ = assets.load_asset_pack("assets.zip").await;
        let loaded = assets.load_texture_configs(&data.texture_manifest).await;

        // The screenshot harness runs headless; opening an audio device there
        // buys nothing and can fail on a machine with no sound card.
        let music = if capture::capture_requested("DRAGONS_HOARD") {
            crate::music::Music::silent()
        } else {
            crate::music::Music::load(data.config.sfx_volume).await
        };

        let sound = if capture::capture_requested("DRAGONS_HOARD") {
            SoundBank::muted()
        } else {
            SoundBank::load(data.config.sfx_volume).await
        };

        let mut notifications = NotificationManager::new();
        notifications.info(format!(
            "Dragon's Hoard ready — {} paylines, {} textures, {} sounds",
            data.paylines.len(),
            loaded,
            sound.len()
        ));

        let mut achievements = AchievementBook::load(&data.config)
            .unwrap_or_else(|err| panic!("achievements.json failed to load: {}", err));
        // Booting into a cabinet counts as playing it.
        achievements.note_machine(data.machine_id());

        let mut session = GameSession::new(&data, random_u64());
        session.preferences = preferences;

        let mut sound = sound;
        sound.set_volume(session.preferences.sfx_volume());
        let mut music = music;
        music.set_volume(session.preferences.music_volume());
        music.set_arrangement(crate::music::arrangement(data.theme_name()));
        // Text size (§5.38). Set once at boot and again whenever it changes;
        // the toolkit applies it to drawing and measurement together, so the
        // layout audit measures what the player actually sees.
        // The cabinet's palette (§5.43). Set here and on every machine switch,
        // which are the only two moments it changes.
        ui::theme::set(ui::theme::by_name(data.theme_name()));

        macroquad_toolkit::ui::set_ui_text_scale(session.preferences.text_scale());

        // DRAGONS_HOARD_PSEUDO stress-tests the layout for translation without
        // there being any translation (§5.39). Set here rather than in the
        // audit scene so any capture can be taken under it — the point is as
        // much to *look* at a pseudolocalised panel as to measure one.
        // An *empty* value means off, not on. A harness that clears the knob by
        // setting it to "" turned the pseudolocale on for every later run, and
        // the findings that produced looked like faults on innocent screens
        // (§5.50).
        if std::env::var("DRAGONS_HOARD_PSEUDO").is_ok_and(|value| !value.is_empty()) {
            macroquad_toolkit::ui::pseudo_enable(macroquad_toolkit::ui::Pseudo::default());
        }

        let ledger = crate::state::ledger::Ledger::load(&data.config);
        let hints = crate::state::hints::HintBook::load(&data.config)
            .unwrap_or_else(|err| panic!("hints.json failed to load: {}", err));

        // A fresh run is a fresh session, so the caps the player last chose are
        // the caps in force (§5.30).
        let mut limits = crate::state::limits::LimitState::with_defaults(&limit_choices);
        limits.pending = session.preferences.limits;
        if let Some(minutes) = session.preferences.reality_check_minutes {
            limits.reality_check_minutes = minutes;
        }
        limits.new_session();

        let mut game = Self {
            data,
            session,
            notifications,
            events: EventBus::new(),
            shake: ScreenShake::new(9.0),
            particles: ParticleSystem::with_capacity(320),
            floating: FloatingTextLayer::new(),
            sound,
            music,
            ui_time: 0.0,
            show_paytable: false,
            show_settings: false,
            show_machines: false,
            show_achievements: false,
            show_featurebuy: false,
            show_ledger: false,
            show_rules: false,
            limits,
            limit_choices,
            show_limits: false,
            history: crate::state::history::History::default(),
            show_history: false,
            reality_check: false,
            show_waveforms: false,
            show_vision: false,
            nav: ui::nav::Nav::default(),
            hints,
            profiles: crate::state::profile::ProfileBook::default(),
            achievements,
            ledger,
            save_exists: false,
        };
        game.refresh_save_state();
        game
    }

    pub fn update(&mut self, dt: f32) {
        self.ui_time += dt;
        self.measure_machines();
        self.drain_finished_rounds();
        self.notifications.update(dt);
        self.shake.update(dt);
        self.particles.update(dt);
        self.floating.update(dt);

        // The session clock only runs while the game is playable: a reality
        // check that ticked while the reality check was up would come due again
        // the moment it was dismissed.
        if !self.reality_check && self.limits.breach().is_none() {
            self.limits.clock.tick(dt);
            if let Some(breach) = self.limits.evaluate() {
                self.notifications.warning(breach.message());
                self.session
                    .stop_autospin(crate::state::autospin::AutospinStop::LimitReached);
            } else if self
                .limits
                .clock
                .check_due(self.limits.reality_check_minutes)
            {
                self.reality_check = true;
                self.session
                    .stop_autospin(crate::state::autospin::AutospinStop::RealityCheck);
            }
        }

        // The mood is derived rather than set, so a state the music should
        // react to cannot be added without this line seeing it (§5.31).
        self.music.set_mood(
            if self.session.holdspin.is_some() || self.session.bonus.is_some() {
                crate::music::Mood::Held
            } else if self.session.in_free_spins() {
                crate::music::Mood::Feature
            } else {
                crate::music::Mood::Base
            },
        );
        self.music.update(dt);

        let spin_events = self.session.update_spin(&self.data, dt);
        for event in spin_events {
            self.handle_spin_event(event);
        }

        for action in ui::shortcuts::actions_from_keys(self.session.celebrations.is_active()) {
            self.events.push(action);
        }
        if is_key_pressed(KeyCode::Escape) {
            self.close_screens();
        }

        let actions: Vec<UiAction> = self.events.drain().collect();
        for action in actions {
            self.apply_action(action);
        }
    }

    pub fn draw(&mut self) {
        clear_background(palette::background());

        // The logical width follows the window's shape (§5.46); the height is
        // fixed, because every panel's vertical layout was written against it.
        let logical_width = ui::frame::logical_width(screen_width(), screen_height());
        let frame = ui::frame::Frame::new(logical_width);
        ui::frame::set_width(logical_width);
        let virtual_ui = begin_virtual_ui_frame(logical_width, ui::frame::HEIGHT);
        let actions = ui::draw_game_ui(
            UiContext {
                data: &self.data,
                session: &self.session,
                save_exists: self.save_exists,
                show_paytable: self.show_paytable,
                show_settings: self.show_settings,
                show_machines: self.show_machines,
                show_achievements: self.show_achievements,
                show_featurebuy: self.show_featurebuy,
                ledger: &self.ledger,
                show_ledger: self.show_ledger,
                show_rules: self.show_rules,
                limits: &self.limits,
                limit_choices: &self.limit_choices,
                show_limits: self.show_limits,
                history: &self.history,
                show_history: self.show_history,
                reality_check: self.reality_check,
                show_waveforms: self.show_waveforms,
                music_levels: self.music.levels(),
                music_mood: self.music.mood(),
                music_arrangement: self.music.arrangement(),
                show_vision: self.show_vision,
                // Not while a panel is up (§5.28). A hint offers something to
                // do next, and behind a modal there is nothing to do next — it
                // also drew across the bottom edge of the panel covering it.
                hint: if self.any_overlay_open() {
                    None
                } else {
                    self.hints
                        .current(self.achievements.progress(), &self.ledger)
                },
                profiles: &self.profiles,
                achievements: &self.achievements,
                shake: self.shake.offset(),
                ui_time: self.ui_time,
                ui: &virtual_ui,
                frame,
            },
            &mut self.nav,
        );

        // Particles and floating text live in logical space, so they belong
        // inside the virtual frame alongside the UI they annotate.
        self.particles.draw();
        self.floating.draw();
        end_virtual_ui_frame();

        // The layout audit (§5.37) runs while the game is genuinely drawing,
        // because measuring text needs the real font. One frame is enough: the
        // panels redraw identically, and the recorder de-duplicates anyway.
        // Touch targets are their own audit and their own scenes (§5.45):
        // sizes want every panel, overlaps want one screen at a time.
        let warm = macroquad_toolkit::ui::neighbours_warm();
        if let Some((width, worst)) =
            macroquad_toolkit::ui::smallest_touchable_width(ui::logical_width()).filter(|_| warm)
        {
            println!(
                "touch targets: need a {:.0}px-wide window; worst is {}",
                width, worst
            );
            for (side, label) in macroquad_toolkit::ui::undersized_targets() {
                println!("touch targets: drawn {}px — {}", side, label);
            }
            for (a, b, area) in macroquad_toolkit::ui::overlapping_targets() {
                println!(
                    "touch targets: {} and {} overlap by {:.0}px² once grown",
                    a, b, area
                );
            }
        }

        if macroquad_toolkit::ui::auditing() {
            let findings = macroquad_toolkit::ui::take_audit();
            if findings.is_empty() {
                println!("layout audit: clean");
            } else {
                for finding in &findings {
                    println!(
                        "layout audit: {} — {:?}",
                        finding.describe(),
                        finding.text()
                    );
                }
                println!("layout audit: {} findings", findings.len());
                // A gate, not a report. A printout nobody reads is the state
                // this replaced — four overflow defects shipped and were found
                // by looking at screenshots (§5.37).
                std::process::exit(1);
            }
        }

        for action in actions {
            self.events.push(action);
        }

        self.notifications
            .draw_with_config(&NotificationRenderConfig {
                anchor: NotificationAnchor::BottomRight,
                ..Default::default()
            });
    }

    fn handle_spin_event(&mut self, event: SpinEvent) {
        match event {
            SpinEvent::ReelStopped(reel) => {
                // A small thud per reel, so the left-to-right stop is felt as
                // well as seen. Later reels land a touch louder, which is what
                // makes the stop sequence read as building rather than repeating.
                self.add_trauma(0.12);
                self.sound.play_at(Sfx::ReelStop, 0.7 + 0.08 * reel as f32);
                self.spawn_reel_stop_dust(reel);
            }
            SpinEvent::Settled(resolution) => self.report_spin(&resolution),
            SpinEvent::PayoutFinished => self.autosave(),
            SpinEvent::AutoSpinReady => self.events.push(UiAction::Spin),
            SpinEvent::CelebrationOpened(kind) => self.celebrate(&kind),
            SpinEvent::HoldSpinRespun => self.report_respin(),
            // One collapse. The pitch climbs with the chain, so a long run is
            // heard building rather than repeating.
            SpinEvent::Cascaded => {
                let step = self
                    .session
                    .phase
                    .cascade()
                    .map_or(0, |reveal| reveal.step());
                self.add_trauma(0.10 + 0.04 * step as f32);
                self.sound.play_at(Sfx::ReelStop, 0.75 + 0.10 * step as f32);
                self.spawn_cascade_dust();
            }
            // The round credits itself; what is left is the noise it makes and
            // a line in the log, since the card only shows the headline figure.
            SpinEvent::HoldSpinFinished(outcome) => {
                self.add_trauma(0.6);
                self.sound.play(Sfx::CoinLock);
                self.notifications.info(format!(
                    "The Dragon's Wrath — {} coins over {} respins for {} credits{}",
                    outcome.coins,
                    outcome.respins_used,
                    outcome.credits,
                    if outcome.full_board {
                        ", the full board"
                    } else {
                        ""
                    }
                ));
            }
        }
    }

    /// One respin landed. Any coins that locked get a thud and a puff; a dry
    /// respin gets the quieter reel-stop tick, so the two are told apart by ear.
    fn report_respin(&mut self) {
        let locked = self
            .session
            .holdspin
            .as_ref()
            .map(|round| {
                (0..round.cell_count())
                    .filter(|i| round.just_locked(*i))
                    .count()
            })
            .unwrap_or(0);

        if locked == 0 {
            self.sound.play_at(Sfx::ReelStop, 0.6);
            return;
        }
        self.add_trauma(0.1 + 0.06 * locked as f32);
        self.sound
            .play_at(Sfx::CoinLock, 0.9 + 0.05 * locked as f32);
    }

    /// A puff from each cell a collapse is clearing, so the symbols read as
    /// being knocked out rather than simply replaced.
    fn spawn_cascade_dust(&mut self) {
        let rows = self.data.config.row_count.max(1);
        for cell in self.session.cascade_clearing().to_vec() {
            let position = ui::reels::cell_center(&self.data, cell / rows, cell % rows);
            self.burst(
                position,
                8,
                &BurstConfig {
                    speed: (40.0, 130.0),
                    size: (1.2, 2.8),
                    life: (0.25, 0.55),
                    colors: vec![palette::gold_bright(), palette::gold()],
                    gravity: 260.0,
                    ..Default::default()
                },
            );
        }
    }

    /// Record something a hint (§5.28) was waiting for, and persist it.
    ///
    /// The counters live with the hints rather than in the save slot: a hint
    /// already acted on must not come back because the player started a new
    /// game, any more than an achievement would.
    fn note_hint_progress(&mut self, note: impl Fn(&mut crate::state::hints::HintProgress)) {
        note(self.hints.progress_mut());
        let _ = self.hints.save(&self.data.config);
    }

    /// Write any round the last stake closed into the ledger (§5.18).
    ///
    /// Drained here rather than inside the session because the ledger spans
    /// every cabinet and outlives any one save slot, exactly like the
    /// achievements book.
    pub(super) fn drain_finished_rounds(&mut self) {
        let Some(round) = self.session.closed_round.take() else {
            return;
        };
        self.ledger.record(
            self.data.machine_id(),
            round.wagered,
            round.credits,
            round.feature,
        );
        let _ = self.ledger.save(&self.data.config);

        // The same round the ledger just took, against the session clock
        // (§5.30). Gamble winnings are excluded from both for the same reason.
        self.limits.clock.record(round.wagered, round.credits);
        // And the bankroll itself, for the graph (§5.32). Recorded here rather
        // than every frame so one point is one round — a per-frame sample would
        // make the x axis a measure of how long the player stared at the reels.
        self.history.record(self.session.balance);
        if let Some(breach) = self.limits.evaluate() {
            self.notifications.warning(breach.message());
        }
    }

    /// Keep the machine profiles (§5.17) moving while the picker is open.
    ///
    /// Only while it is open: measuring costs real frame time and nobody is
    /// looking at the answer otherwise. Cabinets are measured one at a time, in
    /// catalog order, so the row the player is reading fills in first.
    fn measure_machines(&mut self) {
        // The buy menu wants its tiers measured (§5.22); the picker and the
        // ledger want the cabinets. Both run a slice per frame, and only while
        // something is looking at the answer.
        if self.show_featurebuy {
            for tier in 0..self.data.featurebuy.tiers.len() {
                if self.profiles.tier(self.data.machine_id(), tier).is_some() {
                    continue;
                }
                self.profiles
                    .request_tier(self.data.machine_id(), tier, &self.data);
                self.profiles.step_tier(self.data.machine_id(), &self.data);
                break;
            }
        }
        if !self.show_machines && !self.show_ledger {
            return;
        }
        for machine in crate::data::MACHINES {
            if self.profiles.get(machine.id).is_some() {
                continue;
            }
            // The profiler needs that cabinet's data, not the one being played.
            let Ok(data) = crate::data::GameData::load_machine(machine) else {
                continue;
            };
            self.profiles.request(machine.id, &data);
            self.profiles.step(machine.id, &data);
            return;
        }
    }

    /// Screen shake, unless the player turned it off. Motion sensitivity is a
    /// real accessibility need, so this is a hard gate rather than a scale.
    fn add_trauma(&mut self, amount: f32) {
        if self.session.preferences.shared.screen_shake {
            self.shake.add_trauma(amount);
        }
    }

    /// Particle bursts, unless the player turned them off.
    fn burst(&mut self, position: Vec2, count: usize, config: &BurstConfig) {
        if self.session.preferences.particles {
            self.particles.spawn_burst(position, count, config);
        }
    }

    /// Move to another cabinet.
    ///
    /// Each machine is a separate maths model with its own balance, hoard and
    /// jackpots, so this banks the current one to its own slot and loads the
    /// target's — it is closer to walking to a different machine than to
    /// changing a theme. Refused mid-spin: the stake on the current machine is
    /// already committed.
    /// Persist the caps. It is the **pending** set that is written, not the
    /// active one: pending is what the player asked for, and the tighten-now
    /// rule reconstructs the rest at the start of the next session.
    fn save_limits(&mut self) {
        self.session.preferences.limits = self.limits.pending;
        self.session.preferences.reality_check_minutes = Some(self.limits.reality_check_minutes);
        let _ = self.session.preferences.save(&self.data.config);
    }

    /// Step a cap and say what happened to it.
    ///
    /// The player is told when a change was filed rather than applied, because
    /// a button that appears not to work is worse than a rule that is explained
    /// (§5.30).
    fn cycle_limit(&mut self, cap: crate::state::limits::Cap) {
        use crate::state::limits::Cap;
        let choices: Vec<i64> = match cap {
            Cap::Time => self
                .limit_choices
                .time_minutes
                .iter()
                .map(|value| *value as i64)
                .collect(),
            Cap::Loss => self.limit_choices.losses.clone(),
            Cap::Spins => self
                .limit_choices
                .spins
                .iter()
                .map(|value| *value as i64)
                .collect(),
        };
        let next = ui::limits::next_choice(&choices, self.limits.requested(cap));
        if self.limits.request(cap, next) {
            // Tightening can bind on something already behind the player.
            if let Some(breach) = self.limits.evaluate() {
                self.notifications.warning(breach.message());
            }
        } else {
            self.notifications.info(format!(
                "{} takes effect from your next game",
                ui::limits::cap_label(cap, next)
            ));
        }
        self.sound.play(Sfx::Click);
    }

    /// Any panel that takes over the screen. Used to hold hints back, and it
    /// lists every overlay on purpose: one added without a line here is one a
    /// hint would draw over.
    fn any_overlay_open(&self) -> bool {
        // Derived from the registry rather than listed again (§5.50). A state
        // that holds the game has to appear there, which is what makes it
        // auditable — the two facts are now the same fact.
        screens::Screen::ALL
            .iter()
            .any(|screen| self.screen_open(*screen))
    }

    fn switch_machine(&mut self, index: usize) {
        let Some(machine) = crate::data::MACHINES.get(index) else {
            return;
        };
        if machine.id == self.data.machine_id() {
            self.show_machines = false;
            return;
        }
        if !self.session.is_settled() {
            self.notifications
                .warning("Finish this spin before switching machines");
            return;
        }

        self.autosave();

        let data = match GameData::load_machine(machine)
            .and_then(|data| crate::state::rules::validate(&data).map(|()| data))
        {
            Ok(data) => data,
            Err(err) => {
                self.notifications
                    .danger(format!("Could not load that machine: {}", err));
                return;
            }
        };

        let mut preferences = self.session.preferences.clone();
        preferences.machine_id = machine.id.to_owned();
        self.data = data;
        ui::theme::set(ui::theme::by_name(self.data.theme_name()));
        self.music
            .set_arrangement(crate::music::arrangement(self.data.theme_name()));
        self.session = self.load_machine_session();
        self.session.preferences = preferences;

        self.particles.clear();
        self.floating.clear();
        self.shake.clear();
        self.show_machines = false;
        self.refresh_save_state();
        let _ = self.session.preferences.save(&self.data.config);

        for def in self.achievements.note_machine(self.data.machine_id()) {
            self.notifications
                .success(format!("Achievement — {}", def.name));
        }
        let _ = self.achievements.save(&self.data.config);

        self.notifications
            .success(format!("Now playing {}", self.data.config.display_name));
    }

    /// Fold a settled spin into the player's lifetime progress and announce
    /// anything it earned. Achievements are saved as soon as one is unlocked
    /// rather than on the autosave beat — losing one to a crash would be worse
    /// than losing a spin's worth of credits.
    fn record_achievements(&mut self, resolution: &SpinResolution) {
        let earned =
            self.achievements
                .observe(self.data.machine_id(), resolution, self.session.balance);
        if earned.is_empty() {
            return;
        }

        for def in &earned {
            self.notifications
                .success(format!("Achievement — {}", def.name));
            self.floating.spawn(
                def.name.clone(),
                ui::celebration::card_center(),
                palette::gold_bright(),
            );
        }
        self.sound.play(Sfx::WinSmall);
        let _ = self.achievements.save(&self.data.config);
    }
}
