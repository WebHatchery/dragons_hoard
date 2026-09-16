# RustGames Agent Checklist

Applies to all Rust game projects in this workspace. `CODE_STANDARDS.md` is the detailed authority; `MACROQUAD_TOOLKIT.md` and `GAME_DEVELOPMENT_GUIDE.md` provide API examples and setup guidance. Edit shared documents in `rust_management/docs/`, then distribute them with its sync script; keep project-specific guidance in the project's README or `PROJECT_AGENTS.md`.

## Implementation

- Use Rust, `macroquad`, and `macroquad-toolkit` by default. Consider missing shared capabilities as toolkit upgrades before adding local alternatives; diverge only for an established project pattern or a game-specific need.
- Follow `CODE_STANDARDS.md`: cohesive modules, named module files (no new `mod.rs`), explicit state ownership, UI actions, clear errors, and no unused code.
- Keep every `.rs` file within the 800-total-line hard limit, with no exemptions; follow §2.2 for counting and restructuring.
- Load JSON game data through the toolkit; projects own schemas and semantic validation (§5.3).
- Make browser gameplay fully touch-accessible, with visible controls and explicit tutorial instructions (§7.5).
- Keep gameplay deterministic where practical; isolate randomness in small helpers or state-owned RNG.
- Match existing style, avoid unrelated refactors, and add dependencies only when they remove real complexity or match an established pattern.
- Keep a root `catalog_thumbnail.png` for publishing (§8.5).

## Validation

- Keep tests in each crate's `tests/` directory and strongly target five cases per major feature; preserve useful regression coverage (§11).
- After meaningful game changes, run `.\publish.ps1` without parameters in the affected project and report the result or blocker. Do not substitute a local run unless requested (§8.3).
- Store screenshots directly in `docs/verification/`, replacing captures of the same screen or state (§12).

## Commits

- Follow `rust_management/docs/COMMIT_STYLE.md` (relative to the workspace root): a subject in the game's voice ending with a clear parenthetical tag, an honest explanatory body, and AI co-authorship. No Conventional-Commits prefixes or forced metaphors for mechanical changes.
- Read `mytherra` or `stellar_legacy` history before the first commit in a new game.
- Finish, validate, and commit each independently useful major change before starting the next. Keep exploratory edits uncommitted until their outcome is known.
- After implementation and validation, check the working tree and commit unless the user asks otherwise. Stage all modified and untracked project files, including pre-existing changes; report the hash and validation result.
