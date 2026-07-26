//! How the game says things (§5.34).
//!
//! # The paytable and the win line were describing different games
//!
//! Every symbol carries a `short` — a three-letter code the reel renderer draws
//! when it cannot draw the art, and the paytable puts in its swatch. That is
//! what the field is for, and it works.
//!
//! It had also leaked into prose. A win read **"CHS x3 on line 19"** while the
//! paytable two keystrokes away called the same symbol "Treasure Chest", and a
//! refining free spin announced **"burned FRC RIM"** (§5.21). The player is
//! being told what happened in a code they were never given, about symbols the
//! game names perfectly well everywhere else.
//!
//! It survived twenty-eight iterations because nothing was wrong with it
//! locally: each call site had a symbol and reached for the nearest string on
//! it. So the fix is not the two edits, it is having **one place that decides
//! how anything is named** and a test that no short code can reach prose again.
//!
//! # And the numbers
//!
//! The same audit found the other half. A game entirely about quantities was
//! rendering every one of them with `to_string()`: `Balance 1000150`, a peak of
//! `1009419`, a stake of `35800`. Seven digits a player has to count with their
//! eye to know whether they have a million or ten.
//!
//! Every credit figure goes through [`credits`] now, which is the toolkit's
//! digit grouping, and every large count through [`count`]. What stays ungrouped
//! is anything that is an **identifier or a multiplier** rather than a
//! magnitude: `×1,000` is worse than `×1000`, and a grouped payline number would
//! read as money.
//!
//! There was already a private `format_credits` in the reel renderer, used by
//! the jackpot ladder and nothing else. So the game had known separators were
//! needed since the ladder was written, in exactly one place — which is why the
//! Grand read `25,000` while the balance beside it read `1000150`.

use crate::data::GameData;
use crate::engine::evaluate::{SpinOutcome, Win, WinSource};

use macroquad_toolkit::ui::{grouped, signed};

/// A credit figure. Grouped, always — this is the money.
pub fn credits(value: i64) -> String {
    grouped(value)
}

/// Eggs collected toward a hatch, named and counted.
///
/// The one place in the game where a bare number would be wrong: "6" says
/// nothing, and the hoard panel and the ruin screen (§5.53) both need the player
/// to know what is being given up.
pub fn eggs(count: u32) -> String {
    match count {
        1 => "1 egg".to_owned(),
        other => format!("{} eggs", grouped(other as i64)),
    }
}

/// A quantity of things rather than of credits — spins, rounds.
///
/// Grouped for the same reason credits are: it is a magnitude the player reads.
/// Multipliers and line numbers are not, because they are not magnitudes.
pub fn count(value: u64) -> String {
    grouped(value.min(i64::MAX as u64) as i64)
}

/// A credit figure that shows which way it went.
pub fn net(value: i64) -> String {
    signed(value)
}

/// What a symbol is called. The name, never the code.
pub fn symbol(data: &GameData, index: usize) -> &str {
    &data.symbols.get(index).name
}

/// One win, in words.
///
/// The line number is the payline's own id rather than its index, so it matches
/// what the paytable and the reel overlay call it.
pub fn win(data: &GameData, win: &Win) -> String {
    let name = symbol(data, win.symbol);
    match win.source {
        // The line's own name, which has been in `paylines.json` since the game
        // shipped and was never once shown (§5.59). "Bottom" is a thing a
        // player can picture; "line 17" is a thing they have to take on faith,
        // and now that the line is drawn the two have to agree.
        WinSource::Line(index) => format!(
            "{} ×{} on {}",
            name,
            win.count,
            crate::ui::paylines::name(data, index)
        ),
        WinSource::Ways(1) => format!("{} ×{}", name, win.count),
        WinSource::Ways(ways) => format!("{} ×{} across {} ways", name, win.count, ways),
        // The count is the cluster, so repeating it would read as "×8 of 8".
        WinSource::Cluster(size) => format!("{} cluster of {}", name, size),
    }
}

/// The whole win line under the reels.
///
/// Three wins at most and then a count: a grid that pays eleven ways at once
/// would otherwise write a paragraph nobody reads in the second before the next
/// spin.
pub fn wins(data: &GameData, outcome: &SpinOutcome) -> String {
    if outcome.wins.is_empty() && outcome.scatter_credits == 0 {
        return "No win — spin again".to_owned();
    }

    let mut parts: Vec<String> = outcome.wins.iter().take(3).map(|w| win(data, w)).collect();
    if outcome.wins.len() > 3 {
        parts.push(format!("+{} more", outcome.wins.len() - 3));
    }
    if outcome.scatter_credits > 0 {
        let scatter = data
            .symbols
            .scatter()
            .map(|index| symbol(data, index))
            .unwrap_or("Scatter");
        parts.push(format!("{} ×{}", scatter, outcome.scatter_count));
    }
    // A visible separator, not spaces: "cluster of 6 Gold Coins cluster of 6"
    // runs together into one sentence that means nothing (§5.35).
    parts.join("   ·   ")
}

/// The symbols a refining feature has burned off the strips (§5.21).
///
/// Named in full and joined with commas. The escalation is otherwise invisible
/// — the reels simply feel luckier — so the one place it is stated is the one
/// place a code helps least.
pub fn burned(data: &GameData, order: &[String], count: usize) -> String {
    order
        .iter()
        .take(count)
        .filter_map(|id| data.symbols.index_of(id))
        .map(|index| symbol(data, index))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
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
                        !line.split_whitespace().any(|word| {
                            word.trim_matches(|c: char| !c.is_alphanumeric()) == *code
                        }),
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
}
