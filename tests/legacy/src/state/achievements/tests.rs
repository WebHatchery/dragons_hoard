use super::*;
use crate::data::GameData;
use crate::state::GameSession;

fn book() -> AchievementBook {
    let defs: Vec<AchievementDef> = macroquad_toolkit::data_loader::parse_json_labeled(
        "assets/data/achievements.json",
        ACHIEVEMENTS_JSON,
    )
    .unwrap();
    validate(&defs).unwrap();
    let unlocked = Achievements::from_definitions(
        defs.iter()
            .map(|def| Achievement::new(&def.id, &def.name, &def.description))
            .collect(),
    );
    AchievementBook {
        defs,
        unlocked,
        progress: AchievementProgress::default(),
    }
}

/// A settled spin, built by actually spinning so the shape stays honest.
fn a_spin(session: &mut GameSession, data: &GameData) -> SpinResolution {
    session.balance = 1_000_000;
    session.spin(data).unwrap()
}

#[test]
fn the_shipped_definitions_are_valid() {
    let defs: Vec<AchievementDef> = macroquad_toolkit::data_loader::parse_json_labeled(
        "assets/data/achievements.json",
        ACHIEVEMENTS_JSON,
    )
    .unwrap();
    validate(&defs).unwrap();
    assert!(
        defs.len() >= 8,
        "the set should be worth opening a panel for"
    );
}

#[test]
fn a_threshold_of_zero_is_rejected() {
    // It would unlock before the player did anything.
    let defs = vec![AchievementDef {
        id: "free".to_owned(),
        name: "Free".to_owned(),
        description: "Nothing".to_owned(),
        condition: UnlockCondition {
            kind: ConditionKind::Spins,
            at_least: 0,
        },
    }];
    assert!(validate(&defs).is_err());
}

#[test]
fn duplicate_ids_are_rejected() {
    let def = AchievementDef {
        id: "same".to_owned(),
        name: "Same".to_owned(),
        description: "Same".to_owned(),
        condition: UnlockCondition {
            kind: ConditionKind::Spins,
            at_least: 1,
        },
    };
    assert!(validate(&[def.clone(), def]).is_err());
}

#[test]
fn nothing_is_unlocked_before_playing() {
    let book = book();
    let (unlocked, total) = book.tally();

    assert_eq!(unlocked, 0);
    assert_eq!(total, book.defs().len());
}

#[test]
fn the_first_spin_earns_the_first_achievement() {
    let data = GameData::load().unwrap();
    let mut session = GameSession::new(&data, 7);
    let mut book = book();

    let resolution = a_spin(&mut session, &data);
    let earned = book.observe(data.machine_id(), &resolution, session.balance);

    assert!(
        earned.iter().any(|def| def.id == "first_pull"),
        "expected First Pull, got {:?}",
        earned.iter().map(|d| &d.id).collect::<Vec<_>>()
    );
    assert!(book.is_unlocked("first_pull"));
}

#[test]
fn an_achievement_is_only_ever_earned_once() {
    let data = GameData::load().unwrap();
    let mut session = GameSession::new(&data, 8);
    let mut book = book();

    let mut first_pull_awards = 0;
    for _ in 0..20 {
        let resolution = a_spin(&mut session, &data);
        let earned = book.observe(data.machine_id(), &resolution, session.balance);
        first_pull_awards += earned.iter().filter(|def| def.id == "first_pull").count();
    }

    assert_eq!(first_pull_awards, 1, "the toast would repeat every spin");
}

#[test]
fn progress_counts_free_spins_separately_from_paid_ones() {
    let mut progress = AchievementProgress::default();
    let data = GameData::load().unwrap();
    let mut session = GameSession::new(&data, 9);

    let paid = a_spin(&mut session, &data);
    progress.observe("dragon", &paid, 100);
    assert_eq!(progress.spins, 1);
    assert_eq!(progress.free_spins, 0);

    session.free_spins = Some(crate::state::FreeSpinState {
        remaining: 3,
        awarded: 3,
        line_bet: 1,
        total_won: 0,
        burned: 0,
        multiplier: 0,
    });
    let free = a_spin(&mut session, &data);
    progress.observe("dragon", &free, 100);
    assert_eq!(progress.spins, 1, "a free spin is not a paid one");
    assert_eq!(progress.free_spins, 1);
}

#[test]
fn the_balance_condition_is_a_high_water_mark() {
    // Reaching 25,000 once should earn Hoarder even if it is lost again;
    // otherwise the player is punished for continuing to play.
    let mut progress = AchievementProgress::default();
    let data = GameData::load().unwrap();
    let mut session = GameSession::new(&data, 10);
    let resolution = a_spin(&mut session, &data);

    progress.observe("dragon", &resolution, 30_000);
    progress.observe("dragon", &resolution, 40);

    assert_eq!(progress.best_balance, 30_000);
    assert_eq!(progress.value(ConditionKind::Balance), 30_000);
}

#[test]
fn progress_is_cumulative_across_machines() {
    // The reason this state exists at all: `SessionStats` is per-machine,
    // so counting from it would reset every time the player switched.
    let mut progress = AchievementProgress::default();
    let data = GameData::load().unwrap();
    let mut session = GameSession::new(&data, 11);

    for _ in 0..3 {
        let resolution = a_spin(&mut session, &data);
        progress.observe("dragon", &resolution, 100);
    }
    for _ in 0..2 {
        let resolution = a_spin(&mut session, &data);
        progress.observe("frost", &resolution, 100);
    }

    assert_eq!(progress.spins, 5);
    assert_eq!(progress.machines_played, vec!["dragon", "frost"]);
}

#[test]
fn playing_every_machine_earns_the_floor_walker() {
    let mut book = book();

    assert!(book.note_machine("dragon").is_empty());
    let earned = book.note_machine("frost");

    assert!(
        earned.iter().any(|def| def.id == "floor_walker"),
        "walking to the second cabinet should earn it"
    );
}

#[test]
fn a_machine_is_only_counted_once_however_often_it_is_played() {
    let mut progress = AchievementProgress::default();
    for _ in 0..5 {
        progress.note_machine("dragon");
    }

    assert_eq!(progress.machines_played.len(), 1);
}

#[test]
fn the_save_shape_round_trips() {
    let mut book = book();
    book.note_machine("dragon");
    book.progress.spins = 512;
    book.progress.biggest_win = 9_001;
    book.unlocked.unlock("first_pull");

    let snapshot = AchievementSave {
        unlocked_ids: book
            .unlocked
            .iter()
            .filter(|entry| entry.unlocked)
            .map(|entry| entry.id.clone())
            .collect(),
        progress: book.progress.clone(),
    };
    let json = serde_json::to_string(&snapshot).unwrap();
    let restored: AchievementSave = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.progress, book.progress);
    assert!(restored.unlocked_ids.contains(&"first_pull".to_owned()));
}

#[test]
fn a_save_predating_an_achievement_still_loads() {
    // Definitions are read from JSON every load and only the unlock flags
    // are restored, so adding an achievement cannot invalidate a save.
    let saved: AchievementSave =
        serde_json::from_str(r#"{"unlocked_ids":["first_pull"]}"#).unwrap();

    assert_eq!(saved.progress, AchievementProgress::default());
    assert_eq!(saved.unlocked_ids, vec!["first_pull".to_owned()]);
}
