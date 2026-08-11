use crate::data::{GameData, SymbolDef, MACHINES};
use crate::ui::symbols;
use macroquad_toolkit::paint::Buffer;

/// Cell height in pixels on the tallest reel the game can produce.
///
/// The reel window is a little under 400 logical pixels tall and a shifting
/// cabinet may divide it six ways. The cells are wider than they are tall,
/// so height is the binding constraint.
const SMALLEST_CELL: usize = 64;
/// How different two symbols must look in monochrome at the smallest cell.
///
/// Calibrated by measuring rather than chosen. Identical art scores 0; the
/// closest pair this game ships sits comfortably above this.
const MIN_DIFFERENCE: f32 = 0.05;

/// Render an art routine on a fixed neutral colour.
///
/// The baseline is about the **routine**, not the palette. Cabinets give the
/// same art different colours — Frost Wyrm's coin is not Dragon's Hoard's —
/// so a fingerprint taken from the shipped colour would be six different
/// numbers for one shape, and pinning one of them would fail the other five.
/// What a colour change should trip is the dichromacy gate, which is a
/// different test asking a different question.
fn render_shape(def: &SymbolDef, size: usize) -> Buffer {
    let neutral = SymbolDef {
        color: [0.62, 0.62, 0.62],
        ..def.clone()
    };
    render(&neutral, size)
}

fn render(def: &SymbolDef, size: usize) -> Buffer {
    let mut buffer = Buffer::new(size, size);
    let bounds = buffer.bounds();
    assert!(
        symbols::paint(def, bounds, 0.0, 1.0, &mut buffer),
        "'{}' names art the renderer does not have",
        def.art
    );
    buffer
}

#[test]
fn every_symbol_actually_draws_something_at_the_smallest_cell() {
    // A symbol whose art vanished at small sizes would leave an empty tile,
    // and nothing before this could have noticed.
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        for (_, def) in data.symbols.iter() {
            let coverage = render(def, SMALLEST_CELL).coverage();
            assert!(
                coverage > 0.10,
                "{}/{} covers {:.3} of its cell",
                machine.id,
                def.id,
                coverage
            );
            assert!(
                coverage < 0.92,
                "{}/{} covers {:.3} — that is a coloured square, not a shape",
                machine.id,
                def.id,
                coverage
            );
        }
    }
}

#[test]
fn no_two_symbols_look_alike_at_the_smallest_cell() {
    // The claim §5.24 rested on and could not check. Colour is ignored
    // entirely here: two symbols must differ in *shape*, because shape is
    // what survives colour blindness, a dim screen and a small cell.
    let mut failures = Vec::new();

    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        let rendered: Vec<(&str, Buffer)> = data
            .symbols
            .iter()
            .map(|(_, def)| (def.id.as_str(), render(def, SMALLEST_CELL)))
            .collect();

        for (i, (first, a)) in rendered.iter().enumerate() {
            for (second, b) in rendered.iter().skip(i + 1) {
                let difference = a.monochrome_difference(b);
                if difference < MIN_DIFFERENCE {
                    failures.push(format!(
                        "{}: '{}' and '{}' differ by only {:.3} in monochrome",
                        machine.id, first, second, difference
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "symbols that look the same in outline:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn the_three_gem_cuts_are_distinct_shapes() {
    // The fix §5.24 made, stated as a property rather than as a screenshot.
    // Machine-specific by nature: only the hoard set has three gem cuts.
    let data = GameData::load().unwrap();
    let cuts: Vec<Buffer> = ["gem", "gem_round", "gem_step"]
        .iter()
        .map(|art| {
            let def = data
                .symbols
                .iter()
                .map(|(_, def)| def)
                .find(|def| def.art == *art)
                .unwrap_or_else(|| panic!("no symbol drawn as '{}'", art));
            render(def, SMALLEST_CELL)
        })
        .collect();

    for (i, a) in cuts.iter().enumerate() {
        for b in cuts.iter().skip(i + 1) {
            let difference = a.silhouette_difference(b);
            // Silhouette, deliberately, not monochrome: the §5.24 claim was
            // that the three cuts differ in *shape*, and colour must not be
            // allowed to prop that up.
            assert!(
                difference > 0.30,
                "two gem cuts overlap all but {:.0}% of their area",
                difference * 100.0
            );
        }
    }
}

/// Fingerprint of every art routine at the smallest cell, recorded before
/// the rasteriser moved into `macroquad_toolkit::paint` (§5.26).
///
/// The art has been changed four times by someone looking at a capture and
/// deciding it was wrong — gems reading as kites, a coin stack as a blob, an
/// egg as a teardrop, a hexagon that was really a circle. Every one of those
/// was a deliberate edit. What nothing could catch was an *accidental* one:
/// a shared helper nudged, a constant tweaked for one shape that four others
/// also use. This is the audio baseline's bargain (§5.19) applied to
/// pixels — a change to the art has to be a decision.
const FINGERPRINTS: [(&str, u64); 25] = [
    ("coin", 3625727106106365457),
    ("coin_stack", 17006485042544377020),
    ("gem", 614872708312737294),
    ("gem_round", 14370928089879261871),
    ("gem_step", 6235144064633444601),
    ("chest", 16303154741136203508),
    ("egg", 824279762175249698),
    ("dragon", 5401410390544798628),
    ("flame", 2771590236847424369),
    ("shell", 14328836527737074386),
    ("pearl", 12576991707644254006),
    ("starfish", 1638469295444639345),
    ("urchin", 16035859890472980341),
    ("anemone", 1468973680529922933),
    ("crab", 9975012197743009831),
    ("coral", 4128180244561694147),
    ("kraken", 6863129112100050749),
    ("wave", 5287333683428903937),
    ("anvil", 8517133283585763627),
    ("snowflake", 14439825930422787029),
    ("icicle", 11894511740481790761),
    ("spire", 978386683581501821),
    ("feather", 18324749348849968957),
    ("boulder", 7664635896251256177),
    ("pine", 557502598297273465),
];

#[test]
fn the_art_matches_its_baseline() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        for (_, def) in data.symbols.iter() {
            let Some((art, expected)) = FINGERPRINTS.iter().find(|(art, _)| *art == def.art) else {
                continue; // Covered by the test below.
            };
            assert_eq!(
                render_shape(def, SMALLEST_CELL).fingerprint(),
                *expected,
                "'{}' has changed",
                art
            );
        }
    }
}

/// The guard that failed to guard.
///
/// This exists so a new shape added without a baseline cannot slip past the
/// test above. It used to read `GameData::load()` — the *first* cabinet — so
/// when §5.36 added nine tidepool routines on the sixth, all nine went
/// unbaselined and this test passed. A gate scoped to one machine is not a
/// gate on a game with six.
#[test]
fn the_baseline_covers_every_art_routine() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        for (_, def) in data.symbols.iter() {
            assert!(
                FINGERPRINTS.iter().any(|(art, _)| *art == def.art),
                "{}: '{}' has no baseline",
                machine.id,
                def.art
            );
        }
    }
}

#[test]
fn art_scales_rather_than_shrinking_into_a_corner() {
    // Every routine works in normalised coordinates, so coverage should be
    // roughly the same at any size. A symbol whose coverage collapsed as the
    // cell shrank would be one drawn in absolute units by mistake.
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        for (_, def) in data.symbols.iter() {
            let small = render(def, SMALLEST_CELL).coverage();
            let large = render(def, SMALLEST_CELL * 3).coverage();
            assert!(
                (small - large).abs() < 0.08,
                "'{}' covers {:.3} small against {:.3} large",
                def.id,
                small,
                large
            );
        }
    }
}
