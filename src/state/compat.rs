//! Whether an old save still loads (§5.49).
//!
//! # A rule stated a dozen times and never checked
//!
//! "Adding content must never strand a player" appears throughout this design:
//! §5.9 when achievements gained a field, §5.12 when the Wrath arrived, §5.13
//! when the buy menu did. Every time, the answer was `#[serde(default)]` on the
//! new field and a note in the comment.
//!
//! That is the right mechanism and nothing enforced it. Six structures are
//! written to disk — the save, preferences, achievements, hints, the ledger and
//! the session limits — and between them they have gained a field in most
//! iterations of this game. Whether any of them can still read what an earlier
//! build wrote was never tested; it was inferred from the presence of an
//! attribute.
//!
//! # The invariant that generalises
//!
//! A list of historical shapes goes stale the moment someone forgets to add
//! one. The sharp version needs no list:
//!
//! > **A type that loads from `{}` loads from every earlier version of itself.**
//!
//! Because every earlier version is a subset of the current fields, and a type
//! that tolerates *all* of them missing tolerates any of them missing. One line
//! per structure, and it cannot go stale, because it does not describe history —
//! it describes a property.
//!
//! The converse matters too and is the same argument run forwards: a build must
//! read what a *later* build wrote, or a player who opens a newer version once
//! loses everything by going back. That means **unknown fields are ignored**,
//! which serde does by default and `deny_unknown_fields` would silently undo.
//!
//! # What this cannot check
//!
//! That the loaded value is *sensible*, only that it is loadable. A save whose
//! balance defaults to zero has loaded successfully and lost the player's money.
//! So the fields where a default would be wrong are named individually below,
//! and the save format keeps its explicit migration for exactly that reason.

// Tests live in the crate-level integration harness.

// Persistence tests live in the crate-level integration harness.
