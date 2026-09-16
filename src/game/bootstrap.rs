//! Load the resources shared by the live loop and the capture harness.

use crate::audio::SoundBank;
use crate::data::GameData;
use crate::state::achievements::AchievementBook;
use crate::state::preferences::Preferences;
use crate::state::GameSession;
use macroquad::prelude::*;
use macroquad_toolkit::assets::AssetManager;
use macroquad_toolkit::capture;
use macroquad_toolkit::notifications::NotificationManager;
use macroquad_toolkit::rng::random_u64;

pub(super) struct BootResources {
    pub(super) data: GameData,
    pub(super) session: GameSession,
    pub(super) notifications: NotificationManager,
    pub(super) sound: SoundBank,
    pub(super) music: crate::music::Music,
    pub(super) limits: crate::state::limits::LimitState,
    pub(super) limit_choices: crate::state::limits::LimitChoices,
    pub(super) hints: crate::state::hints::HintBook,
    pub(super) achievements: AchievementBook,
    pub(super) ledger: crate::state::ledger::Ledger,
    pub(super) proofs: crate::state::proof::ProofLog,
}

impl BootResources {
    pub(super) async fn load() -> Self {
        let (data, preferences, cabinet_warning) = load_catalog();
        let limit_choices = crate::state::limits::LimitChoices::load()
            .unwrap_or_else(|err| panic!("Dragon's Hoard limits failed to load: {}", err));

        let mut assets = AssetManager::new();
        let placeholder = Image::gen_image_color(16, 16, Color::new(0.5, 0.4, 0.2, 1.0));
        assets.set_placeholder_texture_direct(Texture2D::from_image(&placeholder));
        let loaded = assets.load_texture_configs(&data.texture_manifest).await;
        let headless = capture::capture_requested("DRAGONS_HOARD");
        let music = if headless {
            crate::music::Music::silent()
        } else {
            crate::music::Music::load(data.config.sfx_volume).await
        };
        let sound = if headless {
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
        if let Some(warning) = cabinet_warning {
            notifications.warning(warning);
        }

        let mut achievements = AchievementBook::load(&data.config)
            .unwrap_or_else(|err| panic!("achievements.json failed to load: {}", err));
        achievements.note_machine(data.machine_id());
        let mut session = GameSession::new(&data, random_u64());
        session.preferences = preferences;

        let mut sound = sound;
        sound.set_volume(session.preferences.sfx_volume());
        let mut music = music;
        music.set_volume(session.preferences.music_volume());
        music.set_arrangement(crate::music::arrangement(data.theme_name()));

        let ledger = crate::state::ledger::Ledger::load(&data.config);
        let proofs = crate::state::proof::ProofLog::load(&data.config);
        let hints = crate::state::hints::HintBook::load(&data.config)
            .unwrap_or_else(|err| panic!("hints.json failed to load: {}", err));
        let mut limits = crate::state::limits::LimitState::with_defaults(&limit_choices);
        limits.pending = session.preferences.limits;
        if let Some(minutes) = session.preferences.reality_check_minutes {
            limits.reality_check_minutes = minutes;
        }
        limits.new_session();

        Self {
            data,
            session,
            notifications,
            sound,
            music,
            limits,
            limit_choices,
            hints,
            achievements,
            ledger,
            proofs,
        }
    }
}

fn load_catalog() -> (GameData, Preferences, Option<String>) {
    let bootstrap = GameData::load()
        .unwrap_or_else(|err| panic!("Dragon's Hoard embedded data failed to load: {}", err));
    let preferences = Preferences::load(&bootstrap.config);
    if preferences.machine_id.is_empty() || preferences.machine_id == bootstrap.machine_id() {
        return (bootstrap, preferences, None);
    }

    match GameData::load_machine(crate::data::machine_by_id(&preferences.machine_id)) {
        Ok(data) => (data, preferences, None),
        Err(error) => {
            eprintln!(
                "Dragon's Hoard cabinet '{}' could not be loaded; using the default cabinet: {}",
                preferences.machine_id, error
            );
            (
                bootstrap,
                preferences.clone(),
                Some(format!(
                    "Could not load cabinet '{}'; using the default cabinet",
                    preferences.machine_id
                )),
            )
        }
    }
}
