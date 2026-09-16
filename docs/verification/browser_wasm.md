# Published browser verification

Date: 2026-09-16  
Publish command: `.\publish.ps1` (parameterless)  
Preview deployment: `\\wsl.localhost\Ubuntu\home\kalai\dev\games\dragons_hoard`  
Browser fixture: `http://127.0.0.1:8766/dragons_hoard/`, served from the shared
preview root so the Macroquad runtime assets are present.

## Observed in a fresh browser tab

- The published page loaded the WASM canvas and hid its loading overlay.
- The canvas started after a click gesture; a visible `SPIN` click changed the
  balance, reels, progressive meters, and spin count, then settled normally.
- The visible paytable, feature-buy panel, and settings panel opened and closed.
- Save and Load were exercised; the restored hoard state and notifications were
  visible.
- Settings volume controls reached `Music: Muted` and restored to `Sound: 100%`
  and `Music: 80%`.
- The fresh tab's console stayed empty at load, after the gesture and spin, and
  while opening paytable/settings and changing volume.

## Layout and harness evidence

- `.\verify.ps1 -SkipBuild` passed all 20 gates with
  `PREVIEW_GAMES_ROOT` pointed at the published preview root.
- The 1080x810 portrait-oriented touch/layout captures passed, including the
  footer-width regression found during this run. The resulting captures are
  stored beside this file (`verify_touch*.png`, `verify_audit_*.png`, and
  `verify_motion_*.png`).
- `python tools/check_web_build.py \\wsl.localhost\Ubuntu\home\kalai\dev\games\dragons_hoard`
  reported 113 WASM imports, 7 scripts, and 0 unsatisfied imports.

The browser automation can verify the user-gesture/startup path and the visible
volume/mute controls, but it does not record or listen to speaker output. A
human-audibility certification is therefore not claimed here.
