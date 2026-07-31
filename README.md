# Dragon's Hoard

A dragon-themed slot floor built in Rust + `macroquad`, wired to
`macroquad-toolkit`. Five reels, twenty paylines, coins and gems paying small
and often, treasure and dragon eggs paying big, and the Dragon itself as the
wild that completes lines and drives the features.

**This is a play-money arcade slot.** Credits are a local score — no purchases,
no real currency, nothing of value is wagered.

Six cabinets share one bankroll and one floor-wide Grand: the original Dragon's
Hoard, Frost Wyrm, Emberfall (ways-to-win), Avalanche (cascading), Tidepool
(cluster pays) and Wyrmspire. On top of the base spin sit free spins with
expanding wilds, the Dragon's Hoard egg meter and its Hatch bonus, progressive
jackpots, the Vault Pick, the Dragon's Wrath, the Dragon's Gamble, the Feature
Buy, seams, achievements, a session ledger and per-session limits.

Read [`gdd.md`](gdd.md) first — it is the design spec, the build plan, and the
running record of what has been built. §15 carries the current state and the
short list of what is genuinely still outstanding.

## Layout

```
src/
├── data.rs, data/       # typed mirror of assets/data/*.json — no engine or UI knowledge
├── engine.rs, engine/   # pure reel spinning, payline evaluation, headless simulation
├── state.rs, state/     # GameSession, features, persistence, save migration
├── game.rs, game/       # frame loop, state machine, capture scenes
├── ui.rs, ui/           # pure view layer; every screen returns UiAction intents
├── audio.rs, music.rs   # SFX and music, synthesised at boot — no audio files ship
└── actions.rs           # the intent dispatcher; the only thing that mutates the session
```

Symbol art and the whole SFX set are generated in code rather than shipped as
assets (§7.1). Paytables, reel strips, paylines, machine profiles, symbol sets,
achievements, hints and limits are all data under `assets/data/`.

## Build and run

```powershell
cargo run                     # native debug build
cargo test                    # tests
cargo fmt -- --check          # CI enforces this
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release --target wasm32-unknown-unknown   # WebGL
```

## Verify

```powershell
.\verify.ps1                  # every gate this game has, in one command
.\verify.ps1 -Long            # plus the million-spin RTP run and the conservation soak
.\verify.ps1 -SkipBuild       # reuse the release binary already built
```

The layout, contrast, motion and audio audits need a real window and a real
font, so they run through the capture harness rather than as `cargo test`.

## Publish

```powershell
.\publish.ps1                 # build Windows + WebGL, deploy to the local preview root
.\publish.ps1 -WebGLOnly      # -WindowsOnly, -DeployOnly, -Production (-p), -FTP, -DryRun
```

## Screenshots

```powershell
.\scripts\capture_ui.ps1 -Scenes idle,spin,win -SkipBuild
```

Drives the headless capture harness through the `DRAGONS_HOARD_CAPTURE_*` env
vars. Output lands in `docs/verification/`; §15 of the GDD lists the scenes.

## Project docs

`AGENTS.md`, `CODE_STANDARDS.md`, `MACROQUAD_TOOLKIT.md`, and
`GAME_DEVELOPMENT_GUIDE.md` are synced copies of the canonical versions in
`rust_management/docs/` — don't hand-edit them. Project-specific guidance goes
here or in `gdd.md`.
