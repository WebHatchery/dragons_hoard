use super::*;
use crate::data::{GameData, MACHINES};

/// A stand-in for the font, wider than the real one on purpose.
///
/// The shipped font averages nearer 0.5em per character; assuming 0.62
/// makes every rule measure taller than it will draw, so a layout that fits
/// here has room to spare in the game. A test that flattered the font would
/// be worse than no test.
fn pessimistic(text: &str, width: f32, size: f32) -> usize {
    let per_line = (width / (size * 0.58)).max(1.0);
    // Wrapping breaks on words, so a line rarely fills completely.
    ((text.len() as f32 / per_line).ceil() as usize).max(1) + 1
}

fn measure<'a>() -> Measure<'a> {
    &pessimistic
}

fn every_machine() -> Vec<GameData> {
    MACHINES
        .iter()
        .map(|machine| GameData::load_machine(machine).unwrap())
        .collect()
}

#[test]
fn every_cabinet_fits_in_the_columns_it_has() {
    let (column_width, available) = geometry();
    for data in every_machine() {
        let rules = rules::rules(&data);
        let body = fitting_size(&rules, column_width, available, measure());
        assert!(
            columns_used(&rules, body, measure()) <= COLUMNS,
            "{} needs more than {} columns even at {}px",
            data.machine.id,
            COLUMNS,
            body
        );
    }
}

#[test]
fn the_type_never_shrinks_below_readable() {
    let (column_width, available) = geometry();
    for data in every_machine() {
        let body = fitting_size(&rules::rules(&data), column_width, available, measure());
        assert!(body >= MIN_BODY, "{} shrank to {}", data.machine.id, body);
        assert!(body <= MAX_BODY);
    }
}

#[test]
fn nothing_is_drawn_outside_the_panel() {
    // The failure this is really guarding against: the old paytable prose
    // grew past its box and drew over the footer behind the overlay.
    let (column_width, available) = geometry();
    let top = panel().y + HEADER;
    for data in every_machine() {
        let rules = rules::rules(&data);
        let body = fitting_size(&rules, column_width, available, measure());

        for (rule, slot) in rules.iter().zip(plan(&rules, body, measure())) {
            let bottom = slot.y + block_height(rule, column_width, body, measure());
            assert!(
                bottom <= panel().bottom(),
                "{} spills {}px past the panel",
                data.machine.id,
                bottom - panel().bottom()
            );
            assert!(slot.y >= top);
            let right = panel().x + PADDING + slot.column as f32 * (column_width + COLUMN_GAP);
            assert!(right + column_width <= panel().right());
        }
    }
}

#[test]
fn a_rule_is_never_split_across_columns() {
    // Every slot is either directly below the previous one or at the top of
    // the next column — never part way down a fresh one.
    let (column_width, available) = geometry();
    let rules = rules::rules(&every_machine()[0]);
    let body = fitting_size(&rules, column_width, available, measure());
    let slots = plan(&rules, body, measure());

    for pair in slots.windows(2) {
        if pair[1].column != pair[0].column {
            assert_eq!(pair[1].y, panel().y + HEADER);
            assert_eq!(pair[1].column, pair[0].column + 1);
        } else {
            assert!(pair[1].y > pair[0].y);
        }
    }
}

#[test]
fn more_rules_never_need_fewer_columns() {
    // The size search walks down assuming the requirement only ever grows.
    let rules = rules::rules(&every_machine()[0]);
    let few = columns_used(&rules, MAX_BODY, measure());

    let mut many_rules = rules.clone();
    for _ in 0..8 {
        many_rules.push(rules[0].clone());
    }
    assert!(columns_used(&many_rules, MAX_BODY, measure()) >= few);
}

#[test]
fn smaller_type_never_needs_more_columns() {
    let rules = rules::rules(&every_machine()[0]);
    let mut previous = usize::MAX;
    let mut size = MAX_BODY;
    while size >= MIN_BODY {
        let used = columns_used(&rules, size, measure());
        assert!(used <= previous || previous == usize::MAX);
        previous = used;
        size -= 1.0;
    }
}

#[test]
fn a_set_too_large_for_any_size_still_returns_one() {
    // Rather than loop forever looking for a fit that does not exist.
    let (column_width, available) = geometry();
    let rule = rules::rules(&every_machine()[0])[0].clone();
    let flood: Vec<Rule> = std::iter::repeat_n(rule, 200).collect();
    assert_eq!(
        fitting_size(&flood, column_width, available, measure()),
        MIN_BODY
    );
}

#[test]
fn a_rule_taller_than_a_column_still_gets_a_slot() {
    // `plan` must not spin looking for a column that could hold it.
    let mut rule = rules::rules(&every_machine()[0])[0].clone();
    rule.text = "word ".repeat(4_000);
    let slots = plan(&[rule], MAX_BODY, measure());
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].column, 0);
}
