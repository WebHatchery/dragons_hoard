# Published browser verification

## Current pass — 2026-09-20

Publish command: `.\publish.ps1` (parameterless)
Preview deployment: `\\wsl.localhost\Ubuntu\home\kalai\dev\games\dragons_hoard`
Browser fixture: `http://127.0.0.1:8766/dragons_hoard/`, served from the shared
preview root so the Macroquad runtime assets are present.

The published build was opened in the in-app browser at an approximately
1265x720 browser viewport. Touch-style pointer clicks exercised:

- fresh load and the visible `More` menu;
- bet decrease, `Auto 25` start/stop, and a settled spin;
- contextual `Gamble` followed by `Take`;
- `Buy` and close, rules and close, and Settings;
- Save & Recovery, including Save and Load notifications.

The current browser screenshot showed the compact reel/wager composition,
visible Buy price, contextual Gamble amount at risk, and separated utility
routes. Phone-sized CSS viewports, rotation, volume controls, colour vision,
bonus picks, free-spin shape choice and seam rites were not exercised in this
browser pass.

## Automated evidence

- `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  and `cargo test --all-targets --quiet` passed; the test run reported 595
  passed and 12 ignored.
- `\.\verify.ps1 -SkipBuild` was run with `PREVIEW_GAMES_ROOT` set to the
  published preview. Source, WASM, layout-theme/text/pseudolocale checks,
  published-page checks, save checks, screen checks, translation, motion and
  the direct waveform audit passed in the successful portions. The final run
  stopped during the aspect sweep when Windows reported error 1224 while the
  harness overwrote `verify_layout_audit.png`; a complete green run is not
  claimed.
- The direct 960x720 touch audit passed after the hint slot was made the sole
  owner of the footer while open. The 1280x720 waveform audit also passed
  after compacting its dense row typography. Current captures are stored in
  this directory, including `ui_idle.png`, `ui_freespins.png`, `ui_hint.png`,
  `ui_gameplay.png`, `ui_menu.png`, `ui_storage.png`,
  `ui_touch_audit_settings.png` and `ui_audit_waveforms.png`.

The browser automation can verify visible controls and notifications, but it
does not record or listen to speaker output. A human-audibility certification
is not claimed here.

## Historical result — 2026-09-16

The original browser pass loaded the WASM canvas, spun successfully, opened
paytable/feature-buy/settings, and exercised Save/Load and volume controls.
Its 1080x810 capture was a landscape-oriented window; it remains historical
evidence and is not used as the current portrait contract.
