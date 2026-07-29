//! The Seam as a live session sees it (§5.80, §5.81, §5.83).
//!
//! The engine's own tests next door prove a rite does the right thing to a
//! grid. These are about the round's place in the game: who gets asked, what
//! stops while it waits, and what happens to the things that were running when
//! it opened.

use super::*;

/// Spin until the cabinet deals a seam, leaving it open and unchosen.
///
/// Headless, because reaching one interactively would mean answering every
/// board and card on the way and this is not a test about those. Bounded: a
/// seam is roughly one spin in a hundred here, so twenty thousand is a wide
/// margin and still a failure rather than a hang if the trigger stops firing.
fn deal_a_seam(session: &mut GameSession, data: &GameData) {
    for _ in 0..20_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        if session.spin_leaving_bonus(data).is_err() {
            break;
        }
        session.auto_play_bonus(data);
        session.auto_play_holdspin(data);
        if session.seam.is_some() {
            session.celebrations.clear();
            return;
        }
    }
    panic!("no seam in 20,000 spins");
}

#[test]
fn a_seam_opens_unchosen_and_holds_everything_until_it_is_answered() {
    let data = data();
    let mut session = GameSession::new(&data, 4_820);
    deal_a_seam(&mut session, &data);

    assert_eq!(session.seam_choice().len(), data.seam.rites.len());
    assert!(!session.is_settled(), "a waiting seam must hold the reels");
    assert_eq!(session.begin_spin(&data), Err(SpinBlocked::Busy));

    // Ten seconds of frames change nothing at all. The beat does not run and
    // the timer does not advance, so a player who looks away comes back to the
    // board they left.
    let before = session.seam.as_ref().map(|round| round.steps_left());
    for _ in 0..600 {
        session.update_spin(&data, 1.0 / 60.0);
    }
    assert_eq!(
        session.seam.as_ref().map(|round| round.steps_left()),
        before
    );
    assert!(session.seam.is_some());

    assert!(session.choose_rite(0).is_some());
    assert!(
        session.seam_choice().is_empty(),
        "the deal cannot be re-cut"
    );
    assert!(session.choose_rite(1).is_none());
}

#[test]
fn a_seam_dealt_during_free_spins_asks_the_player_too() {
    // §5.81 excluded free spins by analogy with the gamble (§5.16) and the
    // analogy was wrong: a gamble is offered after a win and would have to
    // interrupt a chain to be taken, while a seam has stopped everything by
    // existing. The Vault Pick has opened mid-feature and waited since §5.10.
    let data = data();
    let mut session = GameSession::new(&data, 9_311);
    let shape = crate::data::FreeSpinShape {
        id: "test".to_owned(),
        name: "Test".to_owned(),
        spin_permille: 1_000,
        multiplier: 1,
    };
    session.grant_free_spins(500, &shape, &data);
    assert!(session.in_free_spins());

    deal_a_seam(&mut session, &data);

    assert!(
        session.in_free_spins(),
        "the run should still be going for this to be the case it is about"
    );
    assert_eq!(session.seam_choice().len(), data.seam.rites.len());

    // And the chain does not run on underneath it.
    let remaining = session.free_spins.as_ref().map(|state| state.remaining);
    for _ in 0..600 {
        session.update_spin(&data, 1.0 / 60.0);
    }
    assert_eq!(
        session.free_spins.as_ref().map(|state| state.remaining),
        remaining,
        "the free-spin chain spun on behind an unanswered seam"
    );
}

#[test]
fn a_seam_that_stops_an_autospin_run_still_asks() {
    // The other half of §5.83: the autospin exclusion was dead code dressed as
    // a policy. A seam tears the run down as it opens, so by the time anything
    // reads the choice there is no run left to exclude — and excluding it would
    // have meant a player whose run had *just stopped for this* being handed a
    // rite they never picked.
    let data = data();
    let mut session = GameSession::new(&data, 7_240);
    assert!(session.start_autospin(500));

    deal_a_seam(&mut session, &data);

    assert!(
        session.autospin.is_none(),
        "a seam is supposed to stop an unattended run"
    );
    assert_eq!(session.seam_choice().len(), data.seam.rites.len());
}

#[test]
fn the_headless_path_still_draws_a_rite_for_itself() {
    // Nobody to ask, so `spin()` must resolve the round rather than leave the
    // sim holding an open board forever. This is the property the whole RTP
    // measurement rests on.
    let data = data();
    let mut session = GameSession::new(&data, 5_150);

    for _ in 0..20_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        let resolution = session.spin(&data).expect("the sim could not spin");
        assert!(session.seam.is_none(), "the headless path left a seam open");
        if resolution.seam_credits > 0 {
            return;
        }
    }
    panic!("no paying seam in 20,000 headless spins");
}

#[test]
fn a_seam_pays_at_the_free_spin_multiplier_it_opened_under() {
    // The round captures the evaluation context of the spin that opened it, so
    // a seam worked during a doubled run is worth double — the same money, a
    // beat later. Measured as a ratio between two identical runs rather than
    // against a figure, because the boards differ.
    let data = data();
    let plain = seam_credits_over(&data, 1);
    let doubled = seam_credits_over(&data, 3);

    assert!(plain > 0 && doubled > 0, "neither run paid anything");
    let ratio = doubled as f64 / plain as f64;
    assert!(
        (2.0..4.0).contains(&ratio),
        "a x3 run returned {:.2} times a x1 one through the seam",
        ratio
    );
}

/// Total seam credits over a fixed run of free spins at a given multiplier.
fn seam_credits_over(data: &GameData, multiplier: i64) -> i64 {
    let mut session = GameSession::new(data, 0x5EA3);
    let shape = crate::data::FreeSpinShape {
        id: "test".to_owned(),
        name: "Test".to_owned(),
        spin_permille: 1_000,
        multiplier,
    };
    session.grant_free_spins(60_000, &shape, data);

    let mut credits = 0;
    for _ in 0..60_000 {
        session.balance = 1_000_000_000;
        session.celebrations.clear();
        let Ok(resolution) = session.spin(data) else {
            break;
        };
        credits += resolution.seam_credits;
    }
    credits
}

/// A seam that pays nothing must not be celebrated as though it did (§5.85).
#[test]
fn a_seam_that_comes_to_nothing_says_so() {
    use crate::state::celebration::CelebrationKind;

    let data = data();
    let mut session = GameSession::new(&data, 3_141);
    let mut dry = 0;
    let mut paid = 0;

    for _ in 0..40_000 {
        session.balance = 1_000_000;
        session.celebrations.clear();
        if session.spin_leaving_bonus(&data).is_err() {
            break;
        }
        session.auto_play_bonus(&data);
        session.auto_play_holdspin(&data);
        if session.seam.is_none() {
            continue;
        }
        session.celebrations.clear();
        let Some(outcome) = session.auto_play_seam(&data) else {
            continue;
        };

        let card = session
            .celebrations
            .active()
            .map(|card| card.kind().clone())
            .expect("a finished seam raises no card at all");
        let CelebrationKind::Seam {
            credits, dry: why, ..
        } = card
        else {
            panic!("a seam raised somebody else's card");
        };

        assert_eq!(credits, outcome.credits);
        assert_eq!(
            why.is_some(),
            outcome.credits == 0,
            "a seam paying {} was described as {}",
            outcome.credits,
            if why.is_some() { "dry" } else { "a win" }
        );
        if why.is_some() {
            dry += 1;
        } else {
            paid += 1;
        }
        if dry >= 3 && paid >= 3 {
            return;
        }
    }
    panic!("only saw {} dry and {} paying seams", dry, paid);
}

/// The counters the awards book keeps, driven the way a player drives them.
///
/// The book is fed one thing: a settled `SpinResolution`, from
/// `SpinEvent::Settled`. Every feature in this game that opens a *round* —
/// the Vault Pick, the Dragon's Wrath, the Seam — credits itself after that
/// event has already gone out, so its figure on the resolution is zero at the
/// moment the book reads it. That is documented behaviour for each of them
/// individually (§5.10, §5.12, §5.80) and nothing had ever put the three facts
/// next to the thing that consumes them.
mod awards {
    use super::*;
    use crate::state::achievements::{AchievementBook, FeatureRound};

    /// Play interactively until the hoard hatches, feeding the book exactly what
    /// the game feeds it.
    #[test]
    fn a_hatch_a_player_watched_reaches_the_awards_book() {
        let data = data();
        let mut session = GameSession::new(&data, 6_006);
        let mut book = AchievementBook::load(&data.config).unwrap();

        for _ in 0..40_000 {
            session.balance = 1_000_000;
            session.celebrations.clear();
            let Ok(resolution) = session.spin_leaving_bonus(&data) else {
                break;
            };
            book.observe(data.machine_id(), &resolution, session.balance);

            // What a player does with an open board: turn chests until it ends.
            // The book is told when the *round* finishes, which is what `Game`
            // does and what §5.84 had to add.
            let had_board = session.bonus.is_some();
            while let Some(index) = session
                .bonus
                .as_ref()
                .and_then(|round| round.next_unrevealed())
            {
                session.pick_bonus(index, &data);
            }
            if had_board {
                book.note_round(FeatureRound::Hatch);
            }
            session.auto_play_holdspin(&data);
            session.auto_play_seam(&data);

            if session.stats.hatches > 0 {
                assert_eq!(
                    book.progress().hatches,
                    session.stats.hatches as i64,
                    "the session counted {} hatches and the awards book {}",
                    session.stats.hatches,
                    book.progress().hatches
                );
                return;
            }
        }
        panic!("no hatch in 40,000 spins");
    }

    /// And the same for the two rounds that had never been counted at all.
    #[test]
    fn every_feature_round_moves_a_counter_of_its_own() {
        let data = data();
        let mut book = AchievementBook::load(&data.config).unwrap();

        for (round, read) in [
            (FeatureRound::Hatch, 0usize),
            (FeatureRound::Wrath, 1),
            (FeatureRound::Seam, 2),
        ] {
            let before = counters(&book);
            book.note_round(round);
            let after = counters(&book);
            for (index, (was, now)) in before.iter().zip(&after).enumerate() {
                if index == read {
                    assert_eq!(*now, was + 1, "{:?} did not move its own counter", round);
                } else {
                    assert_eq!(now, was, "{:?} moved somebody else's counter", round);
                }
            }
        }
    }

    fn counters(book: &AchievementBook) -> [i64; 3] {
        let progress = book.progress();
        [progress.hatches, progress.wraths, progress.seams]
    }
}
