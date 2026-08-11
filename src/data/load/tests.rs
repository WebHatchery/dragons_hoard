use super::*;
use crate::data::MACHINES;

/// Every way a cabinet's data can be wrong that this file claims to catch,
/// applied to real shipped data and required to be refused.
///
/// # A validator nobody has seen reject anything
///
/// `validate` is two hundred lines and thirty-odd rejections standing
/// between a typo in a JSON file and a panic on the first spin. Every test
/// that had ever run it handed it the *shipped* data, which is valid — so
/// as far as this suite was concerned it was indistinguishable from
/// `Ok(())`, and would have stayed that way if someone deleted its body.
///
/// So each case here breaks one thing and requires a specific complaint.
/// The messages are compared because a validator that rejects everything
/// with the same words is only slightly better than one that rejects
/// nothing: the message is what tells whoever edited the file what they did.
/// One way a cabinet's data can be wrong: what it is, the word the
/// complaint must contain, and how to break it.
struct Corruption {
    what: &'static str,
    expected: &'static str,
    break_it: fn(&mut GameData),
}

fn corruption(
    what: &'static str,
    expected: &'static str,
    break_it: fn(&mut GameData),
) -> Corruption {
    Corruption {
        what,
        expected,
        break_it,
    }
}

fn corruptions() -> Vec<Corruption> {
    vec![
        corruption("no reels at all", "positive", |d| d.config.reel_count = 0),
        corruption("no rows", "positive", |d| d.config.row_count = 0),
        corruption("no line bets", "line bets", |d| d.config.line_bets.clear()),
        corruption("a free line bet", "positive", |d| d.config.line_bets[0] = 0),
        corruption("a hoard that never fills", "hoard_capacity", |d| {
            d.config.hoard_capacity = 0
        }),
        corruption("no autospin choices", "autospin", |d| {
            d.config.autospin_choices.clear()
        }),
        corruption("a bonus with no prizes", "prizes", |d| {
            d.bonus.prizes_permille.clear()
        }),
        corruption("a bonus that can never end", "blanks", |d| {
            d.bonus.blanks = 0
        }),
        corruption(
            "a bonus that is all blanks",
            "room for at least one prize",
            |d| d.bonus.blanks = d.bonus.board_size,
        ),
        corruption("a respin round with no coins", "coin values", |d| {
            d.holdspin.coin_values.clear()
        }),
        corruption("a respin round that ends at once", "respins", |d| {
            d.holdspin.respins = 0
        }),
        corruption("a gamble nobody can take", "steps", |d| {
            d.gamble.max_steps = 0
        }),
        corruption("a gamble with nothing to win", "ceiling", |d| {
            d.gamble.ceiling_multiple = 0
        }),
        corruption("a seam nothing can open", "trigger_count", |d| {
            d.seam.trigger_count = 0
        }),
        corruption("a seam bigger than the grid", "trigger_count", |d| {
            d.seam.trigger_count = 999
        }),
        corruption("a seam with no rites", "rites", |d| d.seam.rites.clear()),
        corruption("a seam that never moves", "steps", |d| d.seam.steps = 0),
        corruption("a seam with nothing to win", "ceiling", |d| {
            d.seam.max_multiple = 0
        }),
    ]
}

#[test]
fn the_shipped_data_of_every_cabinet_is_valid() {
    for machine in MACHINES {
        GameData::load_machine(machine).unwrap_or_else(|err| panic!("{}: {}", machine.id, err));
    }
}

#[test]
fn every_corruption_is_refused_and_says_why() {
    let pristine = GameData::load().unwrap();
    for case in corruptions() {
        let mut data = pristine.clone();
        (case.break_it)(&mut data);
        let verdict = data.validate();
        let message = match verdict {
            Ok(()) => panic!("{} was accepted", case.what),
            Err(message) => message,
        };
        assert!(
            message.contains(case.expected),
            "{} was refused, but the message does not say why: {:?} does not \
             mention {:?}",
            case.what,
            message,
            case.expected
        );
    }
}

/// The control. Without it, a `validate` that returned `Err` unconditionally
/// would pass every case above.
#[test]
fn uncorrupted_data_is_not_refused() {
    assert!(GameData::load().unwrap().validate().is_ok());
}
