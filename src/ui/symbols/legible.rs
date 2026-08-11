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
mod tests;
