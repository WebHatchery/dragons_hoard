use super::*;
use crate::data::MACHINES;

fn every_machine() -> Vec<GameData> {
    MACHINES
        .iter()
        .map(|machine| GameData::load_machine(machine).unwrap())
        .collect()
}

/// The audit that keeps §5.34 fixed.
///
/// A short code is a rendering fallback, not a word. If one turns up in
/// anything the player reads as a sentence, it is because a call site
/// reached for the nearest string on a symbol again.
#[test]
fn no_short_code_reaches_prose() {
    for data in every_machine() {
        let codes: Vec<&str> = data
            .symbols
            .iter()
            .map(|(_, def)| def.short.as_str())
            .collect();

        let mut prose = Vec::new();
        for (index, _) in data.symbols.iter() {
            prose.push(symbol(&data, index).to_owned());
            for count in 3..=5 {
                prose.push(win(
                    &data,
                    &Win {
                        symbol: index,
                        count,
                        credits: 100,
                        source: WinSource::Line(0),
                        cells: Vec::new(),
                    },
                ));
                prose.push(win(
                    &data,
                    &Win {
                        symbol: index,
                        count,
                        credits: 100,
                        source: WinSource::Ways(12),
                        cells: Vec::new(),
                    },
                ));
            }
        }
        if let Some(refine) = data.freespins.refine.as_ref() {
            prose.push(burned(&data, &refine.order, refine.order.len()));
        }

        for line in prose {
            for code in &codes {
                assert!(
                    !line
                        .split_whitespace()
                        .any(|word| { word.trim_matches(|c: char| !c.is_alphanumeric()) == *code }),
                    "{} says {:?} in {:?}",
                    data.machine.id,
                    code,
                    line
                );
            }
        }
    }
}

#[test]
fn a_win_names_the_symbol_the_paytable_names() {
    // The defect exactly: two screens, two keystrokes apart, calling the
    // same symbol different things.
    for data in every_machine() {
        for (index, def) in data.symbols.iter() {
            let text = win(
                &data,
                &Win {
                    symbol: index,
                    count: 3,
                    credits: 100,
                    source: WinSource::Line(0),
                    cells: Vec::new(),
                },
            );
            assert!(text.contains(&def.name), "{:?} lacks {}", text, def.name);
        }
    }
}

#[test]
fn a_line_win_names_the_line_the_overlay_draws() {
    // This used to require the payline's own id, because the index would
    // have been off by one against the paytable. The line is drawn on the
    // grid now (§5.59), so the readout names it instead — and the two have
    // to agree, or the sentence would describe a different line from the
    // one lit up beside it.
    let data = &every_machine()[0];
    for (index, line) in data.paylines.iter().enumerate() {
        let text = win(
            data,
            &Win {
                symbol: 0,
                count: 3,
                credits: 100,
                source: WinSource::Line(index),
                cells: Vec::new(),
            },
        );
        assert!(
            text.ends_with(&line.name),
            "{} does not name {}",
            text,
            line.name
        );
        assert_eq!(
            text,
            format!(
                "{} on {}",
                text.trim_end_matches(&format!(" on {}", line.name)),
                crate::ui::paylines::name(data, index)
            )
        );
    }
}

#[test]
fn a_ways_win_says_how_many_ways_unless_there_is_only_one() {
    let data = &every_machine()[0];
    let single = win(
        data,
        &Win {
            symbol: 0,
            count: 3,
            credits: 100,
            source: WinSource::Ways(1),
            cells: Vec::new(),
        },
    );
    assert!(!single.contains("ways"), "{}", single);

    let many = win(
        data,
        &Win {
            symbol: 0,
            count: 3,
            credits: 100,
            source: WinSource::Ways(24),
            cells: Vec::new(),
        },
    );
    assert!(many.contains("24 ways"), "{}", many);
}

#[test]
fn a_losing_spin_says_so_rather_than_saying_nothing() {
    let data = &every_machine()[0];
    let text = wins(data, &SpinOutcome::default());
    assert!(text.contains("No win"));
}

#[test]
fn a_crowded_grid_is_summarised_rather_than_listed() {
    // Eleven simultaneous ways wins would otherwise write a paragraph in the
    // second before the next spin.
    let data = &every_machine()[0];
    let outcome = SpinOutcome {
        wins: (0..11)
            .map(|i| Win {
                symbol: i % 4,
                count: 3,
                credits: 100,
                source: WinSource::Ways(2),
                cells: Vec::new(),
            })
            .collect(),
        ..SpinOutcome::default()
    };
    let text = wins(data, &outcome);
    assert!(text.contains("+8 more"), "{}", text);
    assert!(text.len() < 160, "{} characters", text.len());
}

#[test]
fn burned_symbols_are_named_in_the_order_they_go() {
    for data in every_machine() {
        let Some(refine) = data.freespins.refine.as_ref() else {
            continue;
        };
        let text = burned(&data, &refine.order, refine.order.len());
        for id in &refine.order {
            let index = data.symbols.index_of(id).unwrap();
            assert!(text.contains(symbol(&data, index)), "{:?}", text);
        }
        // In order, so the player can see which is next.
        let first = data.symbols.index_of(&refine.order[0]).unwrap();
        assert!(text.starts_with(symbol(&data, first)));
    }
}

#[test]
fn burning_nothing_says_nothing() {
    let data = &every_machine()[0];
    assert!(burned(data, &["dragon".to_owned()], 0).is_empty());
}

#[test]
fn credits_are_grouped_and_multipliers_are_not() {
    // The rule that keeps the two apart: `x1,000` is worse than `x1000`, and
    // a grouped payline number would look like money.
    assert_eq!(credits(1_009_419), "1,009,419");
    assert_eq!(net(-15_083), "-15,083");
    assert_eq!(net(140), "+140");

    let data = &every_machine()[0];
    let text = win(
        data,
        &Win {
            symbol: 0,
            count: 5,
            credits: 100,
            source: WinSource::Ways(1_024),
            cells: Vec::new(),
        },
    );
    assert!(text.contains("1024 ways"), "{}", text);
}
