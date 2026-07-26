//! Whether an old save still loads (§5.49).
//!
//! # A rule stated a dozen times and never checked
//!
//! "Adding content must never strand a player" appears throughout this design:
//! §5.9 when achievements gained a field, §5.12 when the Wrath arrived, §5.13
//! when the buy menu did. Every time, the answer was `#[serde(default)]` on the
//! new field and a note in the comment.
//!
//! That is the right mechanism and nothing enforced it. Six structures are
//! written to disk — the save, preferences, achievements, hints, the ledger and
//! the session limits — and between them they have gained a field in most
//! iterations of this game. Whether any of them can still read what an earlier
//! build wrote was never tested; it was inferred from the presence of an
//! attribute.
//!
//! # The invariant that generalises
//!
//! A list of historical shapes goes stale the moment someone forgets to add
//! one. The sharp version needs no list:
//!
//! > **A type that loads from `{}` loads from every earlier version of itself.**
//!
//! Because every earlier version is a subset of the current fields, and a type
//! that tolerates *all* of them missing tolerates any of them missing. One line
//! per structure, and it cannot go stale, because it does not describe history —
//! it describes a property.
//!
//! The converse matters too and is the same argument run forwards: a build must
//! read what a *later* build wrote, or a player who opens a newer version once
//! loses everything by going back. That means **unknown fields are ignored**,
//! which serde does by default and `deny_unknown_fields` would silently undo.
//!
//! # What this cannot check
//!
//! That the loaded value is *sensible*, only that it is loadable. A save whose
//! balance defaults to zero has loaded successfully and lost the player's money.
//! So the fields where a default would be wrong are named individually below,
//! and the save format keeps its explicit migration for exactly that reason.

#[cfg(test)]
mod tests {
    use crate::data::{GameData, MACHINES};
    use crate::state::achievements::AchievementProgress;
    use crate::state::hints::HintProgress;
    use crate::state::ledger::Ledger;
    use crate::state::limits::{LimitState, Limits};
    use crate::state::preferences::Preferences;
    use crate::state::save::migrate_save_value;
    use serde_json::{json, Value};

    fn data() -> GameData {
        GameData::load().unwrap()
    }

    /// The rule, for everything that outlives a session.
    ///
    /// `SaveData` is deliberately absent: it is the one structure where a
    /// defaulted field would be a lie rather than a gap, and it earns its
    /// explicit migration path below.
    #[test]
    fn every_persisted_type_loads_from_nothing() {
        macro_rules! loads_from_empty {
            ($ty:ty) => {
                let parsed = serde_json::from_value::<$ty>(json!({}));
                assert!(
                    parsed.is_ok(),
                    "{} cannot load from an empty object, so a save written \
                     before any of its fields existed will not load: {:?}",
                    stringify!($ty),
                    parsed.err()
                );
            };
        }

        loads_from_empty!(Preferences);
        loads_from_empty!(AchievementProgress);
        loads_from_empty!(HintProgress);
        loads_from_empty!(Ledger);
        loads_from_empty!(LimitState);
        loads_from_empty!(Limits);
    }

    /// The same argument run forwards.
    ///
    /// A player who opens a newer build once and then goes back must not lose
    /// everything. Serde ignores unknown fields by default; `deny_unknown_fields`
    /// would undo this silently and nothing else would notice.
    #[test]
    fn every_persisted_type_ignores_fields_it_has_never_heard_of() {
        let future = json!({
            "a_field_from_2027": 12,
            "nested": { "anything": [1, 2, 3] },
            "shared": { "unknown_setting": true }
        });

        macro_rules! tolerates_extra {
            ($ty:ty) => {
                assert!(
                    serde_json::from_value::<$ty>(future.clone()).is_ok(),
                    "{} refuses a field from a later build",
                    stringify!($ty)
                );
            };
        }

        tolerates_extra!(Preferences);
        tolerates_extra!(AchievementProgress);
        tolerates_extra!(HintProgress);
        tolerates_extra!(Ledger);
        tolerates_extra!(LimitState);
        tolerates_extra!(Limits);
    }

    /// Every shape this game has actually written, oldest first.
    ///
    /// Unlike the property above this *is* a list and *can* go stale, which is
    /// why it is kept small: only the save format, and only the shapes that had
    /// a migration written for them. Everything else is covered by loading from
    /// nothing.
    fn historical_saves() -> Vec<(&'static str, Value)> {
        vec![
            (
                "v0, before the format had a version at all",
                json!({ "balance": 4_200, "line_bet_index": 2, "hoard_count": 7, "hoard_pot": 340 }),
            ),
            (
                "v0 with the old points name for balance",
                json!({ "points": 1_500, "hoard_count": 0, "hoard_pot": 0 }),
            ),
            (
                "wrapped in the envelope the toolkit writes",
                json!({
                    "version": "1.0.0",
                    "data": { "balance": 900, "line_bet_index": 1, "hoard_count": 3, "hoard_pot": 60 }
                }),
            ),
        ]
    }

    #[test]
    fn every_save_this_game_has_written_still_loads() {
        let data = data();
        for (what, value) in historical_saves() {
            let loaded = migrate_save_value(Some("1.0.0".to_owned()), value, &data);
            assert!(loaded.is_ok(), "{}: {:?}", what, loaded.err());
        }
    }

    /// Loading is not enough; the money has to survive.
    ///
    /// A save whose balance quietly defaults to zero has loaded successfully and
    /// taken the player's bankroll with it, which is worse than refusing.
    #[test]
    fn an_old_save_keeps_the_balance_it_recorded() {
        let data = data();
        let loaded = migrate_save_value(
            None,
            json!({ "balance": 4_200, "line_bet_index": 2, "hoard_count": 7, "hoard_pot": 340 }),
            &data,
        )
        .unwrap();
        assert_eq!(loaded.balance, 4_200);
        assert_eq!(loaded.hoard.count, 7);
        assert_eq!(loaded.hoard.pot, 340);
    }

    #[test]
    fn a_save_from_a_cabinet_with_more_bets_lands_on_a_bet_that_exists() {
        // The bet ladder is per-cabinet and can shrink. An index past the end
        // would panic on the first draw.
        let data = data();
        let loaded = migrate_save_value(
            None,
            json!({ "balance": 100, "line_bet_index": 99, "hoard_count": 0, "hoard_pot": 0 }),
            &data,
        )
        .unwrap();
        assert!(loaded.line_bet_index < data.config.line_bets.len());
    }

    /// The exclusion above, checked rather than asserted.
    ///
    /// `SaveData` is the one structure not held to loading-from-nothing, and it
    /// has to earn that: the claim is that its explicit migration covers the
    /// shapes a default would have to invent. So a save carrying only the two
    /// fields every version has ever had must still come back whole.
    #[test]
    fn the_save_format_earns_its_exclusion() {
        let data = data();
        let bare = json!({ "version": "1.0.0", "balance": 777 });
        let loaded = migrate_save_value(Some("1.0.0".to_owned()), bare, &data)
            .expect("a save with only a version and a balance did not load");
        assert_eq!(loaded.balance, 777);
        assert!(loaded.line_bet_index < data.config.line_bets.len());
    }

    #[test]
    fn nonsense_is_refused_rather_than_half_loaded() {
        // The one case where failing is right: a partial load would present
        // invented state as the player's own.
        let data = data();
        for rubbish in [json!("a string"), json!([1, 2, 3]), json!(7)] {
            assert!(migrate_save_value(None, rubbish, &data).is_err());
        }
    }

    /// Preferences carry a saved *index* into lists that live in config, and
    /// those lists change (§5.38 added text sizes, §5.30 the limit choices).
    #[test]
    fn a_preference_index_past_the_end_of_its_list_falls_back() {
        let prefs: Preferences = serde_json::from_value(json!({
            "autospin_choice": 999,
            "text_scale": 999
        }))
        .unwrap();
        assert!(prefs.text_scale() > 0.0);

        let config = data().config;
        let mut sane = prefs.clone();
        sane.sanitize(&config);
        assert!(sane.autospin_spins(&config) > 0);
    }

    /// The ledger and the achievements are keyed by machine and symbol id, and
    /// §5.41 renamed most of them.
    #[test]
    fn a_record_naming_a_cabinet_that_no_longer_exists_is_harmless() {
        let ledger: Ledger = serde_json::from_value(json!({
            "machines": { "a_cabinet_from_2026": { "wagered": 500, "won": 200 } }
        }))
        .unwrap();
        // It loads, it does not appear as any real cabinet, and it does not
        // stop the real ones being recorded.
        for machine in MACHINES {
            assert!(ledger.get(machine.id).is_none());
        }

        let progress: AchievementProgress = serde_json::from_value(json!({
            "machines_played": ["a_cabinet_from_2026", "dragon"]
        }))
        .unwrap();
        assert_eq!(progress.machines_played.len(), 2);
    }

    #[test]
    fn a_limit_saved_before_the_offered_values_changed_still_binds() {
        // §5.30's caps are stored as the value rather than an index, precisely
        // so a changed list cannot strand one — this is that decision, checked.
        let limits: Limits = serde_json::from_value(json!({ "loss": 3_333 })).unwrap();
        assert_eq!(limits.loss, Some(3_333));
        assert_eq!(limits.time_minutes, None);
    }
}

/// Whether what a session banks is what the next one starts with (§5.58).
///
/// # An autosave nobody read
///
/// The game had autosaved after every resolved spin since it shipped, and
/// `Game::new` never once looked at the disk. It built a fresh session and
/// handed it to the player, so every launch reset the hoard meter to zero eggs,
/// the Mini, Minor and Major pots to their seeds, and the session stats to
/// nothing. Three of the four progressives could not grow past a single sitting.
///
/// Nothing caught it because **writing a save nobody reads looks exactly like
/// writing one that is read**. The save file was correct. The autosave test
/// passed. The migration tests passed. §5.49 checked at length that an old save
/// still *loads* — and never asked whether anything loaded it.
///
/// The boot path itself needs a GL context and cannot be tested here, so what is
/// held instead is the property underneath it: everything a player accumulates
/// survives a write and a read. If that ever stops being true, loading at boot
/// would restore a session that had quietly lost the meter anyway.
#[cfg(test)]
mod persistence_of_play {
    use crate::data::GameData;
    use crate::state::{migrate_save_value, GameSession};

    /// Play until there is something worth keeping, then bank it.
    fn a_played_session(data: &GameData) -> GameSession {
        let mut session = GameSession::new(data, 0x0DDBA11);
        for _ in 0..400 {
            session.balance = 1_000_000;
            session.celebrations.clear();
            let _ = session.spin(data);
        }
        session
    }

    #[test]
    fn everything_a_player_accumulates_survives_a_save_and_a_load() {
        let data = GameData::load().unwrap();
        let played = a_played_session(&data);

        // The run has to have produced something, or this passes on nothing —
        // the exact failure mode that let the bug live.
        assert!(played.stats.total_spins > 0);
        assert!(
            played.hoard.count > 0 || played.hoard.pot > 0,
            "400 spins collected no eggs; this test is not checking what it claims"
        );

        let json = serde_json::to_value(played.to_save(&data.config.version)).unwrap();
        let reloaded = migrate_save_value(Some(data.config.version.clone()), json, &data)
            .expect("a save this game just wrote did not load");
        let opened = GameSession::from_save(&data, reloaded);

        assert_eq!(opened.hoard.count, played.hoard.count, "eggs on the meter");
        assert_eq!(opened.hoard.pot, played.hoard.pot, "the banked pot");
        assert_eq!(opened.stats.total_spins, played.stats.total_spins);
        assert_eq!(opened.stats.total_wagered, played.stats.total_wagered);
        assert_eq!(opened.stats.biggest_win, played.stats.biggest_win);
        assert_eq!(opened.stats.hatches, played.stats.hatches);
    }

    /// The pots specifically, because they are the ones that were resetting and
    /// the ones a player would never notice resetting — a progressive that goes
    /// back to its seed looks exactly like a progressive nobody has fed.
    #[test]
    fn the_progressive_pots_survive_a_save_and_a_load() {
        let data = GameData::load().unwrap();
        let played = a_played_session(&data);

        let grown: Vec<usize> = (0..data.jackpots.tiers.len())
            .filter(|tier| played.jackpots.accrued_milli(*tier) > 0)
            .collect();
        assert!(
            !grown.is_empty(),
            "400 spins fed no pot at all, so nothing here is being checked"
        );

        let json = serde_json::to_value(played.to_save(&data.config.version)).unwrap();
        let reloaded = migrate_save_value(Some(data.config.version.clone()), json, &data).unwrap();
        let opened = GameSession::from_save(&data, reloaded);

        for tier in grown {
            assert_eq!(
                opened.jackpots.accrued_milli(tier),
                played.jackpots.accrued_milli(tier),
                "the {} pot went back to its seed",
                data.jackpots.tiers[tier].id
            );
        }
    }
}
