//! High-level game loop: owns the session, routes intents, drives feedback.

mod capture_scenes;

use crate::actions::{self, ActionOutcome, SessionRequest};
use crate::audio::{Sfx, SoundBank};
use crate::data::GameData;
use crate::state::achievements::AchievementBook;
use crate::state::autospin::AutospinStop;
use crate::state::celebration::CelebrationKind;
use crate::state::featurebuy::BuyBlocked;
use crate::state::gamble::GambleBlocked;
use crate::state::preferences::Preferences;
use crate::state::spin::SpinEvent;
use crate::state::{migrate_save_value, GameSession, SaveData, SpinBlocked, SpinResolution};
use crate::ui::{self, palette, UiAction, UiContext};
use macroquad::prelude::*;
use macroquad_toolkit::assets::AssetManager;
use macroquad_toolkit::capture;
use macroquad_toolkit::events::EventBus;
use macroquad_toolkit::fx::{BurstConfig, FloatingTextLayer, ParticleSystem, ScreenShake};
use macroquad_toolkit::notifications::{
    NotificationAnchor, NotificationManager, NotificationRenderConfig,
};
use macroquad_toolkit::persistence::{
    delete_slot, load_from_slot_with_migration, save_to_slot_with_version, slot_exists,
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
    /// Monotonic in-game seconds, fed to the UI so pulsing highlights stay
    /// deterministic under the fixed-timestep capture harness.
    ui_time: f32,
    show_paytable: bool,
    show_settings: bool,
    show_machines: bool,
    show_achievements: bool,
    show_featurebuy: bool,
    achievements: AchievementBook,
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

        let mut game = Self {
            data,
            session,
            notifications,
            events: EventBus::new(),
            shake: ScreenShake::new(9.0),
            particles: ParticleSystem::with_capacity(320),
            floating: FloatingTextLayer::new(),
            sound,
            ui_time: 0.0,
            show_paytable: false,
            show_settings: false,
            show_machines: false,
            show_achievements: false,
            show_featurebuy: false,
            achievements,
            save_exists: false,
        };
        game.refresh_save_state();
        game
    }

    pub fn update(&mut self, dt: f32) {
        self.ui_time += dt;
        self.notifications.update(dt);
        self.shake.update(dt);
        self.particles.update(dt);
        self.floating.update(dt);

        let spin_events = self.session.update_spin(&self.data, dt);
        for event in spin_events {
            self.handle_spin_event(event);
        }

        for action in ui::actions_from_keys(self.session.celebrations.is_active()) {
            self.events.push(action);
        }
        if is_key_pressed(KeyCode::Escape) {
            self.show_settings = false;
            self.show_paytable = false;
            self.show_machines = false;
            self.show_achievements = false;
            self.show_featurebuy = false;
        }

        let actions: Vec<UiAction> = self.events.drain().collect();
        for action in actions {
            self.apply_action(action);
        }
    }

    pub fn draw(&mut self) {
        clear_background(palette::BACKGROUND);

        let virtual_ui = begin_virtual_ui_frame(ui::LOGICAL_WIDTH, ui::LOGICAL_HEIGHT);
        let actions = ui::draw_game_ui(UiContext {
            data: &self.data,
            session: &self.session,
            save_exists: self.save_exists,
            show_paytable: self.show_paytable,
            show_settings: self.show_settings,
            show_machines: self.show_machines,
            show_achievements: self.show_achievements,
            show_featurebuy: self.show_featurebuy,
            achievements: &self.achievements,
            shake: self.shake.offset(),
            ui_time: self.ui_time,
            ui: &virtual_ui,
        });

        // Particles and floating text live in logical space, so they belong
        // inside the virtual frame alongside the UI they annotate.
        self.particles.draw();
        self.floating.draw();
        end_virtual_ui_frame();

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
                    colors: vec![palette::GOLD_BRIGHT, palette::GOLD],
                    gravity: 260.0,
                    ..Default::default()
                },
            );
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

        let data = match GameData::load_machine(machine) {
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

    /// The target machine's saved session, or a fresh one if it has never been
    /// played.
    fn load_machine_session(&mut self) -> GameSession {
        let loaded: Result<SaveData, String> = load_from_slot_with_migration(
            &self.data.config.game_name,
            &self.data.save_slot(),
            &self.data.config.version,
            |version, value| migrate_save_value(version, value, &self.data),
        );

        match loaded {
            Ok(save) => GameSession::from_save(&self.data, save),
            Err(_) => GameSession::new(&self.data, random_u64()),
        }
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
                palette::GOLD_BRIGHT,
            );
        }
        self.sound.play(Sfx::WinSmall);
        let _ = self.achievements.save(&self.data.config);
    }

    /// Punch up a card as it opens. The card itself is drawn by the UI; this is
    /// the part you feel rather than read.
    fn celebrate(&mut self, kind: &CelebrationKind) {
        match kind {
            // The rarest event in the game gets the loudest presentation.
            CelebrationKind::Jackpot { .. } => {
                self.add_trauma(1.0);
                self.sound.play(Sfx::Hatch);
                self.spawn_hatch_burst();
                self.spawn_hatch_burst();
            }
            CelebrationKind::Hatch { .. } => {
                self.add_trauma(0.9);
                self.sound.play(Sfx::Hatch);
                self.spawn_hatch_burst();
            }
            // A full board is the top of the feature and is dressed like it.
            CelebrationKind::Wrath { full_board, .. } => {
                self.add_trauma(if *full_board { 1.0 } else { 0.8 });
                self.sound.play(Sfx::Hatch);
                self.spawn_hatch_burst();
                if *full_board {
                    self.spawn_hatch_burst();
                }
            }
            CelebrationKind::FreeSpinsEntry { .. } => {
                self.add_trauma(0.7);
                self.sound.play(Sfx::Scatter);
                self.spawn_hatch_burst();
            }
            // A bust gets shake but no burst: it is the one card in the game
            // that is not good news, and showering it in gold would read wrong.
            CelebrationKind::GambleLost { .. } => {
                self.add_trauma(0.45);
                self.sound.play_at(Sfx::ReelStop, 0.45);
            }
            CelebrationKind::FreeSpinsRetrigger { .. } => {
                self.add_trauma(0.5);
                self.sound.play(Sfx::Scatter);
            }
            CelebrationKind::BigWin { .. } => {
                self.add_trauma(0.5);
                self.sound.play(Sfx::WinBig);
            }
            CelebrationKind::FreeSpinsSummary { .. } => self.sound.play(Sfx::WinSmall),
        }
    }

    /// Ordinary feedback for an ordinary spin. The big moments — a feature
    /// starting or ending, a hatch, a big win — are raised as celebration cards
    /// by the session instead, so nothing is announced twice.
    fn report_spin(&mut self, resolution: &SpinResolution) {
        let credits = resolution.total_credits();
        self.spawn_win_text(resolution);
        self.record_achievements(resolution);

        // A big win gets its sound and its announcement from its celebration
        // card instead, so neither is heard or read twice.
        if credits > 0 && credits < self.session.big_win_threshold(&self.data) {
            self.sound.play(Sfx::WinSmall);
            let message = if resolution.was_free_spin {
                format!("Free spin win {} credits", credits)
            } else {
                format!("Win {} credits", credits)
            };
            self.notifications.info(message);
        }

        // Nothing to count up means the payout phase is skipped, so save here.
        if credits == 0 {
            self.autosave();
        }
    }

    /// One rising number per win, anchored to the last cell that formed it so
    /// the player can see *which* combination paid.
    ///
    /// Reads the win's own cells rather than looking up a payline, so a ways win
    /// — which has no line to look up — lands in the right place too.
    fn spawn_win_text(&mut self, resolution: &SpinResolution) {
        let outcome = resolution.outcome();
        let rows = self.data.config.row_count.max(1);

        for win in outcome.wins.iter().take(6) {
            let Some(cell) = win.cells.last().copied() else {
                continue;
            };
            let (reel, row) = (cell / rows, cell % rows);
            let position = ui::reels::cell_center(&self.data, reel, row);
            self.floating
                .spawn(format!("+{}", win.credits), position, palette::GOLD_BRIGHT);
            if win.credits >= self.session.total_bet(&self.data) {
                self.spawn_win_burst(position);
            }
        }

        if outcome.scatter_credits > 0 {
            self.floating.spawn(
                format!("Scatter +{}", outcome.scatter_credits),
                ui::reels::grid_center(),
                palette::EMBER,
            );
        }
    }

    fn spawn_win_burst(&mut self, position: Vec2) {
        self.burst(
            position,
            18,
            &BurstConfig {
                speed: (60.0, 190.0),
                size: (1.5, 3.5),
                life: (0.35, 0.8),
                colors: vec![palette::GOLD_BRIGHT, palette::GOLD, palette::EMBER],
                gravity: 220.0,
                ..Default::default()
            },
        );
    }

    fn spawn_hatch_burst(&mut self) {
        self.burst(
            ui::celebration::card_center(),
            120,
            &BurstConfig {
                speed: (90.0, 340.0),
                size: (2.0, 5.0),
                life: (0.6, 1.4),
                colors: vec![
                    palette::GOLD_BRIGHT,
                    palette::GOLD,
                    palette::EMBER,
                    Color::new(1.0, 1.0, 0.92, 1.0),
                ],
                gravity: 260.0,
                ..Default::default()
            },
        );
    }

    fn spawn_reel_stop_dust(&mut self, reel: usize) {
        let position = ui::reels::reel_foot(&self.data, reel);
        self.burst(
            position,
            6,
            &BurstConfig {
                speed: (30.0, 90.0),
                size: (1.0, 2.2),
                life: (0.18, 0.4),
                colors: vec![Color::new(0.65, 0.58, 0.48, 0.8)],
                direction: -std::f32::consts::FRAC_PI_2,
                spread: std::f32::consts::PI * 0.8,
                gravity: 180.0,
                ..Default::default()
            },
        );
    }

    fn apply_action(&mut self, action: UiAction) {
        let outcome = actions::apply(
            &self.data,
            &mut self.session,
            &mut self.show_paytable,
            action,
        );

        match outcome {
            ActionOutcome::Ignored => {}
            ActionOutcome::PaytableToggled | ActionOutcome::CelebrationDismissed => {
                self.sound.play(Sfx::Click)
            }
            ActionOutcome::SettingsToggled => {
                self.show_settings = !self.show_settings;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::MachinesToggled => {
                self.show_machines = !self.show_machines;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::BonusPicked => self.sound.play(Sfx::Click),
            ActionOutcome::BonusFinished(credits) => {
                self.sound.play(Sfx::WinBig);
                self.notifications
                    .success(format!("The vault yields {} credits", credits));
                self.autosave();
            }
            ActionOutcome::AchievementsToggled => {
                self.show_achievements = !self.show_achievements;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::GambleOffered => {
                self.sound.play(Sfx::Scatter);
                self.show_featurebuy = false;
            }
            ActionOutcome::GambleFlipped(flip) => {
                if flip.won {
                    self.add_trauma(0.35);
                    self.sound.play(Sfx::WinSmall);
                } else {
                    self.sound.play_at(Sfx::ReelStop, 0.5);
                }
            }
            ActionOutcome::GambleTaken(total) => {
                self.sound.play(Sfx::CoinLock);
                self.notifications
                    .success(format!("Gamble taken — {} credits", total));
                self.autosave();
            }
            ActionOutcome::GambleRefused(reason) => {
                self.sound.play_at(Sfx::Click, 0.6);
                self.notifications.warning(gamble_refusal(reason));
            }
            ActionOutcome::FeatureBuyToggled => {
                self.show_featurebuy = !self.show_featurebuy;
                self.sound.play(Sfx::Click);
            }
            ActionOutcome::FeatureBought(purchase) => {
                // The menu closes itself: what was bought is about to take over
                // the screen, and leaving the overlay up would hide it.
                self.show_featurebuy = false;
                self.sound.play(Sfx::Scatter);
                self.notifications.success(format!(
                    "{} bought for {} credits",
                    purchase.tier_name, purchase.price
                ));
                self.autosave();
            }
            ActionOutcome::FeatureBuyRefused(reason) => {
                self.sound.play_at(Sfx::Click, 0.6);
                self.notifications.warning(buy_refusal(reason));
            }
            ActionOutcome::MachineSelected(index) => self.switch_machine(index),
            ActionOutcome::PreferenceChanged => {
                // Apply immediately so the change is audible/visible while the
                // panel is still open, then persist it.
                self.sound.set_volume(self.session.preferences.sfx_volume());
                self.sound.play(Sfx::Click);
                if !self.session.preferences.particles {
                    self.particles.clear();
                }
                if !self.session.preferences.shared.screen_shake {
                    self.shake.clear();
                }
                let _ = self.session.preferences.save(&self.data.config);
            }
            ActionOutcome::SpinStarted => {
                self.sound.play(Sfx::SpinStart);
                self.floating.clear();
            }
            ActionOutcome::AutospinStarted(spins) => {
                self.notifications
                    .info(format!("Autospin — {} spins", spins));
                self.events.push(UiAction::Spin);
            }
            ActionOutcome::AutospinStopped(reason) => {
                self.notifications.info(reason.message());
            }
            ActionOutcome::SpinBlocked(SpinBlocked::InsufficientBalance) => {
                if let Some(reason) = self.session.stop_autospin(AutospinStop::OutOfCredits) {
                    self.notifications.warning(reason.message());
                } else {
                    self.notifications
                        .warning("Not enough credits — lower the bet or start a new game");
                }
            }
            // Pressing spin again mid-spin is normal input, not an error.
            ActionOutcome::SpinBlocked(SpinBlocked::Busy) => {}
            ActionOutcome::BetChanged(line_bet) => {
                self.sound.play(Sfx::Click);
                self.notifications.info(format!(
                    "Line bet {} — total bet {}",
                    line_bet,
                    self.data.total_bet(line_bet)
                ));
            }
            ActionOutcome::Session(request) => self.apply_session_request(request),
        }
    }

    fn apply_session_request(&mut self, request: SessionRequest) {
        match request {
            SessionRequest::NewGame => {
                let preferences = self.session.preferences.clone();
                self.session = GameSession::new(&self.data, random_u64());
                self.session.preferences = preferences;
                self.particles.clear();
                self.floating.clear();
                self.session.celebrations.clear();
                self.notifications.info("Fresh stack of credits");
            }
            SessionRequest::Save => self.save_game(),
            SessionRequest::Load => self.load_game(),
            SessionRequest::DeleteSave => self.delete_save(),
        }
    }

    /// Autosave once a spin has fully resolved. Mid-feature state is not saved:
    /// a reload lands back in the base game.
    fn autosave(&mut self) {
        if self.session.in_free_spins() {
            return;
        }
        let save = self.session.to_save(&self.data.config.version);
        if save_to_slot_with_version(
            &self.data.config.game_name,
            &self.data.save_slot(),
            &save,
            &self.data.config.version,
        )
        .is_ok()
        {
            self.save_exists = true;
        }
    }

    fn save_game(&mut self) {
        let save = self.session.to_save(&self.data.config.version);
        match save_to_slot_with_version(
            &self.data.config.game_name,
            &self.data.save_slot(),
            &save,
            &self.data.config.version,
        ) {
            Ok(()) => {
                self.notifications.success("Hoard recorded");
                self.refresh_save_state();
            }
            Err(err) => self.notifications.danger(format!("Save failed: {}", err)),
        }
    }

    fn load_game(&mut self) {
        let loaded: Result<SaveData, String> = load_from_slot_with_migration(
            &self.data.config.game_name,
            &self.data.save_slot(),
            &self.data.config.version,
            |version, value| migrate_save_value(version, value, &self.data),
        );

        match loaded {
            Ok(save) => {
                let preferences = self.session.preferences.clone();
                self.session = GameSession::from_save(&self.data, save);
                self.session.preferences = preferences;
                self.particles.clear();
                self.floating.clear();
                self.session.celebrations.clear();
                self.notifications.success("Hoard restored");
                self.refresh_save_state();
            }
            Err(err) => self.notifications.warning(format!("Load failed: {}", err)),
        }
    }

    fn delete_save(&mut self) {
        match delete_slot(&self.data.config.game_name, &self.data.save_slot()) {
            Ok(()) => {
                self.notifications.info("Save slot cleared");
                self.refresh_save_state();
            }
            Err(err) => self.notifications.danger(format!("Delete failed: {}", err)),
        }
    }

    fn refresh_save_state(&mut self) {
        self.save_exists = slot_exists(&self.data.config.game_name, &self.data.save_slot());
    }
}

/// Why a gamble was refused, in the player's terms. `NotOffered` is the one a
/// player will actually hit — pressing G on a losing spin — so it says what is
/// needed rather than what is missing.
fn gamble_refusal(reason: GambleBlocked) -> &'static str {
    match reason {
        GambleBlocked::NotOffered => "Nothing to gamble — win a spin first",
        GambleBlocked::LimitReached => "The gamble ladder is spent",
        GambleBlocked::CannotHalve => "This win cannot be split",
    }
}

/// Why a Feature Buy was refused, in the player's terms rather than the
/// enum's. Every refusal says something — a menu press that produces silence
/// reads as a broken button.
fn buy_refusal(reason: BuyBlocked) -> &'static str {
    match reason {
        BuyBlocked::Busy => "Wait for the reels to settle first",
        BuyBlocked::InsufficientBalance => "Not enough credits for that feature",
        BuyBlocked::FeatureActive => "A feature is already running",
        BuyBlocked::UnknownTier => "That feature is no longer on the menu",
    }
}
