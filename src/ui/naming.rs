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
mod tests;
