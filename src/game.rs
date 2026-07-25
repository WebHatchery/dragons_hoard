//! High-level game loop: owns the session, routes intents, drives feedback.

use crate::actions::{self, ActionOutcome, SessionRequest};
use crate::audio::{Sfx, SoundBank};
use crate::data::GameData;
use crate::state::achievements::AchievementBook;
use crate::state::autospin::AutospinStop;
use crate::state::celebration::CelebrationKind;
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
            achievements,
            save_exists: false,
        };
        game.refresh_save_state();
        game
    }

    /// Fast-forward into a named state so the screenshot harness can photograph
    /// something other than the boot screen. Uses the headless spin path to skip
    /// ahead, then hands over to the normal loop.
    ///
    /// Scenes: `idle`, `spin` (reels mid-flight), `win`, `freespins`,
    /// `paytable`, `settings`, `feature_card`, `hatch`, `autospin`.
    pub fn begin_capture_scene(&mut self, scene: &str) {
        // A fixed seed keeps every capture reproducible run to run.
        self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
        self.notifications.clear();

        match scene {
            "spin" => {
                let _ = self.session.begin_spin(&self.data);
            }
            "win" => self.fast_forward_to(|session| session.last_win > 0),
            "freespins" => {
                self.fast_forward_to(GameSession::in_free_spins);
                self.session.celebrations.clear();
                let _ = self.session.begin_spin(&self.data);
            }
            "feature_card" => self.fast_forward_to(|session| {
                matches!(
                    session.celebrations.active().map(|card| card.kind()),
                    Some(CelebrationKind::FreeSpinsEntry { .. })
                )
            }),
            "hatch" => self.fast_forward_to(|session| {
                matches!(
                    session.celebrations.active().map(|card| card.kind()),
                    Some(CelebrationKind::Hatch { .. })
                )
            }),
            "autospin" => {
                let spins = self.session.preferences.autospin_spins(&self.data.config);
                self.session.start_autospin(spins);
                let _ = self.session.begin_spin(&self.data);
            }
            "paytable" => self.show_paytable = true,
            "machines" => self.show_machines = true,
            "bonus" => {
                self.fast_forward_to(|session| session.bonus.is_some());
                self.session.celebrations.clear();
                // Turn a few over so the capture shows a board in play rather
                // than twelve closed chests.
                for index in [0usize, 1, 2, 5] {
                    self.session.pick_bonus(index, &self.data);
                }
            }
            "achievements" => {
                self.fast_forward_to(|session| session.stats.hatches > 0);
                // The hatch that got us here raised a card; the panel is the
                // subject of this capture, not the card.
                self.session.celebrations.clear();
                self.show_achievements = true;
            }
            "jackpot" => self.fast_forward_to(|session| {
                matches!(
                    session.celebrations.active().map(|card| card.kind()),
                    Some(CelebrationKind::Jackpot { .. })
                )
            }),
            "frost" => {
                self.data = GameData::load_machine(&crate::data::MACHINES[1]).unwrap();
                self.session = GameSession::new(&self.data, 0xD2A6_0F1E);
                self.fast_forward_to(|session| session.last_win > 0);
            }
            "settings" => self.show_settings = true,
            _ => {}
        }
    }

    /// Spin headlessly, topping the balance up, until `reached` holds. Cards
    /// raised by earlier spins are cleared each time, so a scene keyed on a card
    /// always lands on one the *last* spin produced. Bounded so a capture can
    /// never hang on an unreachable state.
    fn fast_forward_to(&mut self, reached: impl Fn(&GameSession) -> bool) {
        for _ in 0..20_000 {
            self.session.balance = self.data.config.starting_balance;
            self.session.celebrations.clear();
            // A scene may want to catch a board mid-round, so settle without the
            // headless auto-play and let the predicate look first.
            let Ok(resolution) = self.session.spin_leaving_bonus(&self.data) else {
                break;
            };
            // The headless path skips `report_spin`, so record here too — a
            // capture of the achievements panel should show real progress
            // rather than a column of zeroes.
            self.achievements
                .observe(self.data.machine_id(), &resolution, self.session.balance);

            if reached(&self.session) {
                return;
            }

            // Nothing wanted the open board, so play it out — and look again,
            // because finishing a board is what raises the Hatch card. Checking
            // only before this is what left the `hatch` scene spinning 20,000
            // times and photographing nothing.
            if self.session.auto_play_bonus(&self.data).is_some() && reached(&self.session) {
                return;
            }
        }
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
            CelebrationKind::FreeSpinsEntry { .. } => {
                self.add_trauma(0.7);
                self.sound.play(Sfx::Scatter);
                self.spawn_hatch_burst();
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

    /// One rising number per winning line, anchored to the line's last paying
    /// cell so the player can see *which* line paid.
    fn spawn_win_text(&mut self, resolution: &SpinResolution) {
        let outcome = resolution.outcome();
        for win in outcome.line_wins.iter().take(6) {
            let Some(payline) = self.data.paylines.get(win.line) else {
                continue;
            };
            let reel = win.count.saturating_sub(1);
            let Some(row) = payline.rows.get(reel) else {
                continue;
            };

            let position = ui::reels::cell_center(&self.data, reel, *row);
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
