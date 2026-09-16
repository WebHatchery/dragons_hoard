//! The room each cabinet sits in (§5.43).
//!
//! # Six themes for the symbols, one for everything around them
//!
//! §5.41 and §5.42 gave every cabinet its own symbols. The panels, the reel
//! frame, the header and the footer stayed the same stone and gold on all six,
//! so Tidepool's shells sat in a dragon's vault and Frost Wyrm's ice sat in a
//! warm gold surround. The symbols were themed; the room was not.
//!
//! # Why this could not be done earlier
//!
//! The palette was eleven constants used in nearly three hundred places, and
//! changing colours across a whole UI by hand is exactly the kind of edit whose
//! failures are invisible: a label that was legible on stone is not legible on
//! ice, and nothing says so. **§5.40's contrast gate is what makes this safe.**
//! Each theme is run through the layout audit, and a colour pairing that cannot
//! be read fails the build rather than shipping.
//!
//! That is the whole reason the accessibility work came first. A gate is not
//! only a check on what exists — it is permission to change it.
//!
//! # One theme at a time
//!
//! Held in a thread-local rather than threaded through every draw call, because
//! the alternative is a parameter on three hundred call sites that would say the
//! same thing every time. The theme changes when a cabinet is loaded and at no
//! other moment, so there is exactly one writer and it runs between frames.

use macroquad::prelude::Color;
use std::cell::RefCell;

/// Every colour the interface is built from.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub background: Color,
    /// Panel fill.
    pub stone: Color,
    /// The strip along the top of a panel.
    pub stone_header: Color,
    pub gold: Color,
    pub gold_bright: Color,
    pub gold_dim: Color,
    /// A figure going the wrong way, and the hoard meter.
    pub ember: Color,
    /// A figure going the right way. Used sparingly and on purpose.
    pub jade: Color,
    pub text_bright: Color,
    pub text: Color,
    pub text_dim: Color,
}

/// The original. Every other theme is a departure from this one, and it is what
/// the layout was drawn and measured against.
pub const HOARD: Theme = Theme {
    background: Color::new(0.045, 0.038, 0.052, 1.0),
    stone: Color::new(0.098, 0.086, 0.098, 1.0),
    stone_header: Color::new(0.14, 0.118, 0.125, 1.0),
    gold: Color::new(0.90, 0.74, 0.36, 1.0),
    gold_bright: Color::new(1.0, 0.88, 0.52, 1.0),
    gold_dim: Color::new(0.52, 0.41, 0.20, 0.85),
    ember: Color::new(0.93, 0.45, 0.18, 1.0),
    jade: Color::new(0.42, 0.82, 0.52, 1.0),
    text_bright: Color::new(0.96, 0.93, 0.88, 1.0),
    text: Color::new(0.84, 0.80, 0.74, 1.0),
    text_dim: Color::new(0.62, 0.57, 0.52, 1.0),
};

/// Frost Wyrm. Blue-black stone under a pale, cold accent.
pub const FROST: Theme = Theme {
    background: Color::new(0.030, 0.042, 0.060, 1.0),
    stone: Color::new(0.070, 0.090, 0.115, 1.0),
    stone_header: Color::new(0.100, 0.130, 0.165, 1.0),
    gold: Color::new(0.62, 0.82, 0.94, 1.0),
    gold_bright: Color::new(0.82, 0.94, 1.0, 1.0),
    gold_dim: Color::new(0.30, 0.44, 0.56, 0.85),
    // Warm, because the accent has to leave the blue axis a deuteranope loses.
    ember: Color::new(0.95, 0.58, 0.30, 1.0),
    jade: Color::new(0.46, 0.86, 0.70, 1.0),
    ..HOARD
};

/// Emberfall. Forge dark, with heat in the accent.
pub const EMBER: Theme = Theme {
    background: Color::new(0.052, 0.030, 0.026, 1.0),
    stone: Color::new(0.105, 0.068, 0.058, 1.0),
    stone_header: Color::new(0.150, 0.092, 0.072, 1.0),
    gold: Color::new(0.96, 0.68, 0.30, 1.0),
    gold_bright: Color::new(1.0, 0.84, 0.46, 1.0),
    gold_dim: Color::new(0.56, 0.34, 0.16, 0.85),
    ember: Color::new(0.98, 0.42, 0.24, 1.0),
    jade: Color::new(0.50, 0.84, 0.56, 1.0),
    ..HOARD
};

/// Wyrmspire. Bare stone and open sky.
pub const SPIRE: Theme = Theme {
    background: Color::new(0.040, 0.044, 0.052, 1.0),
    stone: Color::new(0.092, 0.096, 0.108, 1.0),
    stone_header: Color::new(0.130, 0.136, 0.152, 1.0),
    gold: Color::new(0.86, 0.80, 0.62, 1.0),
    gold_bright: Color::new(0.98, 0.94, 0.78, 1.0),
    gold_dim: Color::new(0.46, 0.44, 0.36, 0.85),
    ember: Color::new(0.92, 0.48, 0.34, 1.0),
    jade: Color::new(0.44, 0.80, 0.60, 1.0),
    ..HOARD
};

/// Avalanche. Snow-shadow blue-grey, kept cooler than Wyrmspire's bare stone.
pub const SLIDE: Theme = Theme {
    background: Color::new(0.036, 0.042, 0.048, 1.0),
    stone: Color::new(0.082, 0.094, 0.102, 1.0),
    stone_header: Color::new(0.118, 0.134, 0.144, 1.0),
    gold: Color::new(0.76, 0.84, 0.86, 1.0),
    gold_bright: Color::new(0.92, 0.97, 0.98, 1.0),
    gold_dim: Color::new(0.38, 0.44, 0.48, 0.85),
    ember: Color::new(0.94, 0.52, 0.32, 1.0),
    jade: Color::new(0.46, 0.84, 0.62, 1.0),
    ..HOARD
};

/// Tidepool. Deep water, with sand in the accent.
pub const TIDEPOOL: Theme = Theme {
    background: Color::new(0.024, 0.044, 0.050, 1.0),
    stone: Color::new(0.058, 0.096, 0.104, 1.0),
    stone_header: Color::new(0.084, 0.136, 0.146, 1.0),
    gold: Color::new(0.88, 0.80, 0.58, 1.0),
    gold_bright: Color::new(0.98, 0.92, 0.74, 1.0),
    gold_dim: Color::new(0.36, 0.52, 0.52, 0.85),
    ember: Color::new(0.96, 0.54, 0.34, 1.0),
    jade: Color::new(0.40, 0.86, 0.72, 1.0),
    ..HOARD
};

/// Every theme, with the name a cabinet asks for it by.
pub const THEMES: [(&str, Theme); 6] = [
    ("hoard", HOARD),
    ("frost", FROST),
    ("ember", EMBER),
    ("spire", SPIRE),
    ("slide", SLIDE),
    ("tidepool", TIDEPOOL),
];

/// Look a theme up. An unknown name falls back rather than failing, because a
/// cabinet with an odd palette is playable and one that refuses to load is not.
pub fn by_name(name: &str) -> Theme {
    THEMES
        .iter()
        .find(|(id, _)| *id == name)
        .map(|(_, theme)| *theme)
        .unwrap_or(HOARD)
}

thread_local! {
    static CURRENT: RefCell<Theme> = const { RefCell::new(HOARD) };
}

pub fn set(theme: Theme) {
    CURRENT.with(|slot| *slot.borrow_mut() = theme);
}

pub fn current() -> Theme {
    CURRENT.with(|slot| *slot.borrow())
}

// Tests live in the crate-level integration harness.
