use super::*;
use crate::data::MACHINES;
use crate::engine::seam;

/// The note has to agree with the ladder the rite actually climbs.
///
/// A paytable that named the wrong next symbol would be worse than one that
/// named none: the player would make the deepening decision on it. So the
/// claim is derived from `seam::ladder` and checked against it here for
/// every symbol on every cabinet, rather than trusted because both happen to
/// read the same JSON today.
#[test]
fn every_seam_note_names_the_rung_the_rite_would_climb_to() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let ladder = seam::ladder(&data);

        for (index, def) in data.symbols.iter() {
            if !seam::seamable(&data, index) {
                continue;
            }
            let at = ladder
                .iter()
                .position(|rung| *rung == index)
                .unwrap_or_else(|| panic!("{} is seamable and off the ladder", def.id));
            let note = seam_note(&data, index);

            assert!(
                note.contains(&data.seam.trigger_count.to_string()),
                "{}/{} never quotes the trigger: {:?}",
                machine.id,
                def.id,
                note
            );
            match ladder.get(at + 1) {
                Some(next) => assert!(
                    note.contains(&data.symbols.get(*next).name),
                    "{}/{} deepens to {} and the note says {:?}",
                    machine.id,
                    def.id,
                    data.symbols.get(*next).name,
                    note
                ),
                None => assert!(
                    note.contains("top rung"),
                    "{}/{} is the richest rung and the note does not say so: {:?}",
                    machine.id,
                    def.id,
                    note
                ),
            }
        }
    }
}

/// The three special symbols keep their own notes: a seam can neither be
/// made of them nor turn a cell into one (§5.80), so a seam figure on their
/// row would be a promise the feature cannot keep.
#[test]
fn the_specials_say_nothing_about_seams() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        for special in [
            data.symbols.wild(),
            data.symbols.scatter(),
            data.symbols.hoard(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                !seam_note(&data, special).contains("seam"),
                "{}/{} offers a seam it cannot open",
                machine.id,
                data.symbols.get(special).id
            );
        }
    }
}
