use super::*;
use crate::data::MACHINES;
use crate::engine::evaluate::EvalContext;
use crate::engine::reels::Grid;
use crate::engine::seam;
use crate::state::seam::SeamRound;

/// A board of one symbol end to end, which every cabinet's trigger clears.
fn flooded(data: &GameData, symbol: usize) -> Grid {
    let columns: Vec<Vec<usize>> = (0..data.config.reel_count)
        .map(|_| vec![symbol; data.config.row_count])
        .collect();
    Grid::from_columns(&columns)
}

fn round_on(data: &GameData, grid: &Grid) -> SeamRound {
    let found = seam::find(data, grid, &data.seam).expect("no seam on this board");
    SeamRound::open(data, grid, found, EvalContext::base(data, 10)).expect("no round")
}

/// Every caption is a fact, and every fact is the engine's (§5.87).
///
/// The panel is the only place in the game that tells a player what a rite
/// will do to *this* board, and it is read at the moment they commit. A
/// caption that drifted from the engine would be a lie told at the worst
/// possible time, so each one is compared against the function the rite
/// itself uses rather than against a second copy of the arithmetic.
#[test]
fn every_caption_agrees_with_what_the_rite_would_do() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let ladder = seam::ladder(&data);
        let grid = flooded(&data, ladder[0]);
        let round = round_on(&data, &grid);

        for rite in &data.seam.rites {
            let caption = promise(&data, &round, rite);
            match rite.kind {
                RiteKind::Widen { .. } => {
                    let offered = round.frontier(&data);
                    assert!(
                        caption.contains(&offered.to_string()) || offered == 0,
                        "{}: {} cells are offered and the caption says {:?}",
                        machine.id,
                        offered,
                        caption
                    );
                }
                RiteKind::Enrich { .. } => {
                    let next = data.symbols.get(ladder[1]).name.clone();
                    assert!(
                        caption.contains(&next),
                        "{}: a flooded board climbs to {} and the caption says {:?}",
                        machine.id,
                        next,
                        caption
                    );
                }
                RiteKind::Gild { .. } => {
                    // A flooded board pays a great deal, so the gilding has
                    // something to multiply and must quote a figure.
                    assert!(
                        !caption.contains("nothing"),
                        "{}: a flooded board pays and the caption says {:?}",
                        machine.id,
                        caption
                    );
                }
            }
        }
    }
}

/// The two cases the panel exists to make visible: a rite that will do
/// nothing at all has to say so, or it looks exactly like one that will.
#[test]
fn a_rite_with_nowhere_to_go_says_so() {
    let data = GameData::load().unwrap();
    let ladder = seam::ladder(&data);

    // The top rung: an enrichment has nothing above it to climb to.
    let grid = flooded(&data, *ladder.last().unwrap());
    let round = round_on(&data, &grid);
    assert!(round.next_rung(&data).is_none());
    for rite in &data.seam.rites {
        if matches!(rite.kind, RiteKind::Enrich { .. }) {
            assert!(promise(&data, &round, rite).contains("richest"));
        }
    }

    // A board with no frontier: the seam already holds every cell a rite is
    // allowed to take, so a widening is walled in.
    assert_eq!(round.frontier(&data), 0);
    for rite in &data.seam.rites {
        if matches!(rite.kind, RiteKind::Widen { .. }) {
            assert!(promise(&data, &round, rite).contains("walled in"));
        }
    }
}
