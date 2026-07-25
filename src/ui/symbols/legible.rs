//! Is the art actually legible? Measured, not looked at (§5.25).
//!
//! §5.24 proved a claim about *colours*: no two symbols sharing a shape are too
//! close under any dichromacy. It could not prove the claim it rested on — that
//! the shapes really are different — because nothing could see a shape without a
//! GPU and a person. `ui::paint::Buffer` can, so this measures it.
//!
//! The sizes are not arbitrary. A six-row reel on the shifting cabinet (§5.20)
//! divides the same window six ways, which is the smallest a symbol is ever
//! drawn in this game. That is the size the checks run at.

#[cfg(test)]
mod tests {
    use crate::data::{GameData, SymbolDef, MACHINES};
    use crate::ui::paint::Buffer;
    use crate::ui::symbols;

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

    #[test]
    fn art_scales_rather_than_shrinking_into_a_corner() {
        // Every routine works in normalised coordinates, so coverage should be
        // roughly the same at any size. A symbol whose coverage collapsed as the
        // cell shrank would be one drawn in absolute units by mistake.
        let data = GameData::load().unwrap();
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
