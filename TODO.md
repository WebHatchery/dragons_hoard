# Outstanding tasks

## UI_STYLE review — 2026-09-20

This file began as an audit and implementation plan. The 2026-09-20 pass now
contains a substantial composition, responsive-layout, utility-menu and
evidence refresh. Unchecked items below remain open because their acceptance
matrix was not fully exercised or because the requested disclosure work is not
implemented yet. Keep the implementation history in `gdd.md`; these items do
not reopen already-fixed menu-screen access, seam captions, or duplicate
big-win announcements.

### Current implementation status — 2026-09-20

- Implemented and captured: the compact reel/wager hierarchy, contextual
  Gamble, separated Save & Recovery view, responsive portrait framing and
  active-grid geometry, persistent-fact cleanup, responsive hints and free-spin
  choices, categorized More menu, responsive rules/machine lists, and the
  dense Sound inspection row fix.
- Fresh evidence covers 1280x720, 960x720, 900x800, 810x1080 and 720x960
  canvases, including idle, settled win, free-spin choice, Gamble, Buy,
  Settings, storage, menu, hint, seam and machine states. The published build
  was also exercised with touch-style clicks through bet, Auto start/stop,
  spin, Gamble/Take, Buy, rules, Settings and Save/Load.
- `cargo fmt`, Clippy, the 607-test suite, the parameterless publisher and the
  direct waveform/960x720 touch audits passed. The complete verification gate
  reached the aspect sweep but was interrupted by Windows error 1224 while
  overwriting a verification PNG; no all-green gate is claimed.

### Evidence and scope

- Read the project `AGENTS.md`, updated `UI_STYLE.md`, `CODE_STANDARDS.md`,
  `GAME_DEVELOPMENT_GUIDE.md`, README, and the GDD's game design, screen layout,
  UI evolution, current-state and remaining-work sections. No `PROJECT_AGENTS.md`
  was found. Reviewed the UI composition, screen registry and dispatch, virtual
  viewport and pointer conversion, reels, wager, chrome, help, menus, feature
  choices and feedback code.
- Visually inspected the original `docs/verification/` images and then refreshed
  the relevant captures in this checkout: `ui_idle.png`, `ui_win.png`,
  `ui_hint.png`, `ui_freespins.png`, `ui_gamble.png`, `ui_featurebuy.png`,
  `ui_gameplay.png`, `ui_menu.png`, `ui_storage.png`, `ui_seam.png`, and the
  responsive/audit references. The refreshed free-spin image now shows the
  unobscured shape decision; historical files remain evidence of the prior
  layout, not current acceptance proof.
- The original audit confirmed the persistent chrome, mixed save/wager controls,
  fixed rules columns and portrait geometry findings below. The implementation
  pass has now launched the game, refreshed captures, exercised the published
  browser and touch audits, run tests and published the build. The historical
  1080x810 capture is still 4:3 landscape despite being described as portrait;
  it does not establish the current 810x1080 portrait layout.
- Preserve existing strengths: Spin is prominent; seam choices keep the board
  visible and give all three rites equal emphasis with concrete consequences;
  overlays dim the underlying game; the screen registry gives visible access
  to utilities; hints now have Open/Got it controls; feedback already distinguishes
  dry seams and avoids a second big-win announcement. The live reel screen is
  game-specific, not the template's grid demo. The demo remains in an obsolete
  screenshot, not in the inspected gameplay renderer.

### Confirmed findings — implement in dependency order

These tasks follow `UI_STYLE.md` §§1–9. Start with composition and viewport
ownership; do not spend a separate pass polishing borders before these changes.

- [ ] **U1 — Recompose normal play around the reels and the wager; separate utilities.**
  **Screen/files:** Idle, settled result and spinning; `src/ui/wager.rs`
  (`draw_control_panel`, `draw_session_buttons`, `draw_spin_button`),
  `src/ui/chrome.rs::draw_header`, `src/ui/menu.rs`, `src/game/screens.rs`,
  `src/actions.rs`, README/GDD screen brief.
  **Observed/effect:** Save, Load, New Game and Delete Save share the Wager box
  with Spin, Max Bet and Auto; Save uses the same positive treatment as Spin.
  Buy is grouped with More/Machines/Settings in the header. This mixes gameplay
  decisions and navigation and reserves a large permanent block for utilities.
  The older idle capture shows the composition and current code retains it.
  Also, `OfferGamble` is emitted by the G shortcut, with no visible entry button
  in the inspected UI; Gamble is excluded from `Screen::in_menu` as a dealt
  screen. Its hint has no screen target. Touch can operate an already-open
  gamble, but the normal entry path is missing in source.
  **Change:** First record the current decision, primary action, supporting and
  deferred facts for base play, free spins and seam choices. Move the four
  storage/session controls into a clearly labelled utility view reached through
  a quiet visible menu entry. Place Buy with secondary gameplay actions, keeping
  its price at entry and full costs/risks in its own view. Keep balance, the total
  charge including ante, and bet adjustment next to Spin. Remove the redundant
  Wager heading and its nested WIN card when composing the compact action area;
  use one quiet result readout instead. Preserve existing storage readiness and
  save-existence guards and explain unavailable actions locally.
  Add a contextual Gamble entry beside an eligible settled win, dispatching
  `UiAction::OfferGamble` and using `src/state/features.rs::can_gamble`; keep its
  amount at risk and double-or-nothing consequence visible. Retain the G shortcut
  as an alternative and update the hint to name the visible entry control.
  **Acceptance:** Normal play has at most 2–3 strong attention regions: reels,
  compact wager/result support, and at most contextual feedback. Utilities are
  visually separate and never as prominent as Spin. All save/recovery functions
  remain discoverable; no gameplay state or stake is changed by navigation.
  **Verify:** At 1280x720, 960x720, 810x1080 and 720x960 actual canvases, inspect
  idle, affordable/unaffordable ante and settled win. With touch alone change
  bet, spin, start/stop Auto, open Buy, then open utilities and exercise Save,
  Load and their disabled states using an isolated test save. From an eligible
  settled win, tap Gamble then Take; confirm the entry is unavailable when no
  win is eligible and does not overlap Spin or automatically risk credits.
  **Current remaining:** The main composition and published touch path are
  implemented and exercised. The four disabled storage states and the
  no-win Gamble absence still need isolated touch captures at every contract
  size before this item can be removed.

- [ ] **U2 — Fix viewport mode selection and give portrait play a useful reel area.**
  **Depends on:** U1's composition and removed utility rows.
  **Screen/files:** All layouts; `src/ui/frame.rs::logical_size`, `Frame::sized`,
  `Frame::portrait`, `is_portrait`, `Frame::panel`/`panel_at`;
  `src/game/present.rs::draw`, `src/ui/reels.rs::grid_rect`/`square_off`.
  **Observed/effect (source/calculation):** A 900x800 window produces a 720x640
  virtual canvas, but `Frame::sized` treats height <=720 as landscape and expands
  its width to 960. A square 720x720 canvas has the same width mismatch. Content
  is therefore positioned outside the virtual bounds. At 810x1080, the logical
  frame is 720x960: controls take 430 high, reels 254, and the grid only about
  183x110 after title/jackpot reservations and the nominal 5:3 fit. A large panel
  therefore contains a tiny board. `square_off` assumes 5x3 even for other board
  shapes. There is no world camera to zoom here; reel geometry is the framing.
  **Change:** Choose one explicit layout mode from available dimensions and use
  it consistently for frame, chrome and controls; never silently enlarge the
  frame beyond the virtual canvas. Use the reclaimed space for a compact portrait
  action area and a dominant board. Fit the active cabinet's grid/row counts,
  with deliberate stable framing during shifting reels. Derive overlay bounds
  and top clearance from the current frame rather than the fixed 720 height and
  84 header baseline. Retain toolkit virtual drawing and pointer conversion.
  **Acceptance:** Resize/rotation cannot put actions off canvas, create negative
  grid dimensions or collapse symbols into slits. The board is the largest
  meaningful play region, including portrait; all cells needed for seam/line
  decisions remain distinguishable. Overlays use the available screen without
  cutting through the taller portrait header.
  **Verify:** Normal/minimum sizes above plus 900x800, 720x720, 1024x768 and sizes
  immediately either side of the 4:3 breakpoint. Inspect all six cabinets,
  Wyrmspire's shortest/tallest columns, Tidepool, cascades and seam selection.
  Rotate/resize with an overlay open and verify taps still hit what is drawn.
  Add focused geometry regression cases under `tests/` for breakpoint agreement,
  bounded controls and useful grid dimensions; do not freeze the old layout.
  **Current remaining:** Geometry tests and the four principal canvases plus
  900x800/720x720 transition captures pass. A full six-cabinet runtime sweep,
  resize/rotation with an overlay open, and the longest Wyrmspire/cascade
  decisions remain unverified.

- [ ] **U3 — Consolidate persistent facts and remove tutorial/statistics wallpaper.**
  **Depends on:** U1–U2.
  **Screen/files:** Base play, Auto, free spins and settled results;
  `src/ui/chrome.rs::draw_header`/`draw_footer`, `src/ui/wager.rs::draw_feature_banner`
  and `draw_win_readout`, `src/ui/reels.rs::feature_title`/`draw_win_summary`,
  `src/ui/shortcuts.rs`, existing Ledger/history views.
  **Observed/effect:** Hoard count appears in the header badge and footer meter;
  the pot appears in the header and permanent hatch instruction. Best win,
  lifetime/session counts, a full shortcut sentence and version are always
  present. Free-spin multiplier/expansion facts are repeated in reel and wager
  banners. The old captures and current chrome show information competing with
  the board instead of helping the next spin.
  **Change:** Give hoard progress and banked pot one compact home near the board.
  Move session statistics to the existing Ledger/history and version to a quiet
  utility detail. Teach hatch behavior through first-use/contextual help; retain
  a visible way to reopen it. Replace the permanent shortcut list with help
  access. Give free spins one status block for remaining spins, multiplier and
  accumulated win; remove duplicate wording and unavailable base-bet controls
  during that phase. Keep a clearly labelled last/current result, but make
  change announcements brief and retain details through a visible result/history
  route. Preserve current-win amounts needed by Gamble and current-board value
  needed by seam choices; do not expire these while a decision depends on them.
  **Acceptance:** Each persistent fact has one home; completed teaching and event
  prose stop demanding attention. After feedback ends the balance, result and
  active feature state still explain what happened. Auto Stop and urgent session
  limits remain obvious. Reclaimed footer space enlarges/recomposes play.
  **Verify:** At the four base sizes, compare first-use and returning-player
  idle, no win, small/big win, hoard fill, Auto, free spins and seam completion.
  Wait for all transient feedback to finish, then retrieve the result by tap.
  **Current remaining:** The duplicate chrome and free-spin base controls are
  removed, but hoard-fill, Auto, seam completion and post-feedback retrieval
  still need a complete four-size comparison.

- [ ] **U4 — Anchor contextual hints to responsive layout and preserve legible touch controls.**
  **Depends on:** U2–U3's layout and help location.
  **Screen/files:** First-use hints and free-spin shape selection;
  `src/ui/hint.rs::draw`, `src/state/hints.rs`, `assets/data/hints.json`,
  `src/ui/wager.rs::draw_spin_choices`, `src/ui/nav.rs::control`.
  **Observed/effect (source):** Hint placement remains fixed at x=478,y=668
  rather than the actual footer. At logical width 720 its bar is 224 wide and
  an Open hint leaves only about 12 units for its text. Open/Got it are 24 high;
  free-spin shape buttons are 34 high with 12-size profile text beneath them.
  Touch-area growth exists, so small drawn buttons alone do not prove a failed
  hit target, but this geometry does not establish readable, unambiguous touch
  choices after scaling. Old `ui_hint.png` predates the Open button.
  **Change:** Allocate one responsive hint slot outside the board/actions with
  wrapped text and properly spaced Open/Got it controls; remove fixed footer
  coordinates. Reflow shape options with readable captions and separated touch
  areas, keeping both choices equally emphasized and their spin/multiplier
  consequences visible. Keep first-use dismissal and reopenable help.
  **Acceptance:** Hint text can be read in portrait and never covers Spin or the
  board. Each choice/dismissal has an effective non-overlapping target of at
  least 44x44 CSS pixels at the declared minimum, without depending on hover.
  **Verify:** Four base sizes, largest text setting and long hint/shape labels.
  Tap Open and Got it, reopen help, choose either shape on separate runs and
  start the free spins. Measure actual canvas-to-CSS scaling and target spacing.
  **Current remaining:** Responsive hint placement, readable shape choices and
  the 960x720 touch audit pass. Largest-text, reopen/choose flows and actual
  CSS-pixel target measurements remain open.

- [ ] **U5 — Replace shrink-to-fit reference overlays with readable disclosure and responsive lists.**
  **Depends on:** U2's overlay geometry.
  **Screen/files:** Rules, machine comparison and Feature Buy; `src/ui/rules.rs`
  (`COLUMNS`, `geometry`, `fitting_size`, `plan`), `src/ui/machines.rs`,
  `src/ui/featurebuy.rs`; shared scrolling/layout capabilities where appropriate.
  **Observed/effect:** Rules use four fixed columns and allow 11-size body text
  to fit every topic at once. Machines retain two columns as the panel narrows.
  Buy tiers permanently show small profile distributions, only the first band
  label, and measurement methodology prose. Current code and stored comparison
  captures show dense equal-weight information; portrait failures are also
  recorded historically in GDD §5.79, not freshly reproduced in this audit.
  **Change:** Present rules as a readable topic list with visible tap-to-expand
  details or pages, and single-column content at narrow widths. Keep core Spin,
  bet and cabinet rules first; advanced feature rules remain accessible when
  relevant. Reflow machine rows to one column with deliberate scrolling/paging;
  keep machine identity, play style, current selection and Play visible. In Buy,
  keep the feature contents, actual price, balance, prerequisite/shortage and
  below-cost risk visible before commitment. Move detailed distributions,
  sample methodology and best-observed figures behind a labelled Details control
  with all bands explained. Remove redundant nested boxes, not needed warnings.
  **Acceptance:** Reading does not require tiny type or interpretation of an
  unlabeled chart. Every machine, rule and tier is reachable by touch, including
  the last item. Comparison remains the focal task; no option is accidentally
  recommended by decoration. Users can close/cancel without spending credits.
  **Verify:** Four base sizes, maximum text scale, 40% pseudolocalisation, longest
  cabinet names, large balances, profiler loading/completed and unaffordable
  tiers. Touch-scroll/page to last items, expand/collapse details and close.
  **Current remaining:** Portrait rules and machine lists now reflow to readable
  single columns. Feature Buy still needs a labelled Details disclosure for
  distributions/methodology, and the maximum-text/pseudolocale/last-item touch
  matrix remains open.

- [ ] **U6 — Give the utility menu a player-facing hierarchy instead of an undifferentiated registry dump.**
  **Depends on:** U1's relocated utilities and U5's disclosure routes.
  **Screen/files:** More menu, settings/help routes; `src/ui/menu.rs::draw`,
  `src/game/screens.rs::Screen::in_menu`/`label`, `src/ui/settings.rs`,
  `src/ui/waveform.rs`, `src/ui/vision.rs`.
  **Observed/effect:** Every flag-openable screen is shown as an equal button in
  a two-column “Everything else” list. Buy/Machines, help, session statistics,
  settings and spin-proof tools share one visual group. “Sound” opens waveform
  inspection while ordinary volume lives in Settings, obscuring intent.
  **Change:** Keep registry-backed reachability but add explicit presentation
  categories/routes: gameplay selection, help, session records and utilities.
  Separate gameplay choices from settings/save actions visually. Keep common
  destinations first; put waveform/proof details behind a clearly labelled
  advanced entry, and make volume and colour-vision controls easy to find in
  Settings. Rename the waveform entry to describe inspection and remove the
  keyboard-shortcut footer prose. Preserve access without requiring unlocks.
  **Acceptance:** A player can predict where Help, Save and volume are; advanced
  tools no longer have the same prominence as ordinary destinations. No screen
  loses its tap route or return path when registry entries are added.
  **Verify:** At four base sizes, navigate from play to help, records, Save,
  volume, colour vision and advanced inspection using taps only, then return to
  the same game state. Check the full menu at maximum text size.
  **Current remaining:** Categorized More and the renamed Sound inspection route
  are implemented, and Settings/Save are reachable. Four-size taps through
  volume, colour vision and advanced inspection still need to be recorded.

### Further inspection and acceptance work — not verified runtime defects

- [ ] **U7 — Declare actual viewport support and complete the browser/touch matrix.**
  **Depends on:** U1–U6; use early measurements to guide U2/U4.
  **Screen/files:** README/GDD, `game_page.json`, `verify.ps1`, capture scenes,
  frame/nav/presentation code and `docs/verification/`.
  **Gap/effect:** The GDD's original 1280x720 logical-resolution statement,
  later portrait caveats and historical “portrait” 1080x810 report do not form
  a current minimum browser-canvas contract. Stored low-resolution captures
  cannot prove touch usability. At 390 CSS pixels wide a 44-logical-unit control
  in a 720-wide layout would draw at about 24 CSS pixels before target growth.
  This calculation is a risk to inspect, not a tested device failure.
  **Change:** Document normal 1280x720 and explicitly validate a minimum landscape
  and portrait canvas (start with 960x720 and 720x960), separate from outer window
  size and device pixels. Test 810x1080 tablet, 900x800/square transition sizes,
  and 390x844/844x390 phones to establish the additional compact layout work
  needed. Do not claim phone support from logical target sizes. Extend existing
  gates to real portrait and breakpoint cases instead of landscape alone.
  **Acceptance:** Supported sizes have measured readable text and at least 44
  CSS pixel touch targets with no ambiguous overlap; limitations are explicit
  and any needed compact-layout work is recorded concretely. Native and embedded
  browser rendering/input agree after resize, rotation and display scaling.
  **Verify:** Fresh load -> bet -> Spin -> Auto Stop -> Buy/Close -> Rules/Close
  -> Settings -> utility Save/Load with an isolated save. Also exercise bonus
  picks, free-spin shape choice, seam rites, Gamble/Take, insufficient credits,
  ruin recovery and session-limit acknowledgement without keyboard or hover.
  Run parameterless `.\publish.ps1` after meaningful implementation, report its
  result, and use the published browser build for browser acceptance.
  **Current remaining:** The published browser path was exercised at roughly
  1265x720 and the declared 960x720/720x960/810x1080 canvases were captured.
  Phone CSS sizes, rotation/resize in-browser and the remaining bonus, ruin,
  limit and accessibility paths are not yet certified. The full gate is also
  currently blocked by Windows error 1224 while overwriting a capture PNG.

- [ ] **U8 — Verify dense feature decisions and feedback ordering before accepting the new composition.**
  **Depends on:** U2–U5.
  **Screen/files:** `src/ui/seam.rs::draw_choice`/`promise`,
  `src/ui/gamble.rs`, `src/ui/bonus.rs`, `src/ui/holdspin.rs`,
  `src/ui/celebration.rs`, `src/ui.rs::draw_final_overlays`,
  `src/game/feedback.rs`, capture-scene fixtures and relevant tests under `tests/`.
  **Gap/effect:** GDD §5.87 explicitly leaves simultaneous longest seam captions
  unverified. Existing seam art supports the current board-visible composition,
  but does not prove long-text/touch behavior. The free-spin capture hides the
  actual decision under Vault Pick. No live audit here establishes dismissal,
  overlay input ownership or result comprehension after animations end.
  **Change:** Seed genuine dense states: longest rite/symbol captions together,
  zero-paying/full-frontier/top-rung seams, free-spin choices, large feature
  totals and a result coinciding with a session limit. Inspect first and fix
  reproduced layout/input failures; wrap captions/reflow choices rather than
  shrink their text. Preserve the board, zero-value consequences, equal rite
  emphasis, Gamble amount at risk and Take recovery. Retain the GDD's deliberate
  no-confirmation Gamble decision; do not add a confirmation on every flip.
  **Acceptance:** Each decision explains its costs/consequences and only its
  visible active controls respond. Dry results stay restrained; paid results
  remain understandable after effects finish. A limit/reality-check interruption
  has a visible actionable acknowledgement and cannot strand the player.
  **Verify:** Four base sizes at maximum text scale and 40% longer text; touch
  each rite on separate runs, pick a full bonus round, select free-spin shapes,
  Take a gamble and dismiss/acknowledge overlapping supported events. Inspect
  motion and settled states, not only a single final screenshot.
  **Current remaining:** Genuine free-spin choices, Gamble/Take, machine and
  seam captures are present, but the longest simultaneous seam captions still
  need a full four-size maximum-text touch pass; the 810x1080 seam evidence
  also shows the lowest payout line close to the footer boundary.

- [ ] **U9 — Replace stale template/old-layout evidence with traceable review captures.**
  **Depends on:** Accepted implementation and U7–U8 verification.
  **Screen/files:** `docs/verification/ui_gameplay.png`, existing equivalent
  `ui_*`/`verify_*` captures, `scripts/capture_ui.ps1`, README/GDD and
  `docs/verification/browser_wasm.md`.
  **Observed/effect:** `ui_gameplay.png` visibly contains “Macroquad Toolkit Game
  Template”, demo actions and debug prose. Older idle/hint images show controls
  that differ from current source; this can send a later agent after fixed bugs
  or make a template screen look like current gameplay.
  **Change:** Replace equivalent captures directly in `docs/verification/` with
  current base play, unobscured free-spin selection, contextual help, responsive
  references, utilities and dense decisions at normal/minimum sizes. Replace or
  retire the obsolete demo image. Record canvas/CSS dimensions, scene, build
  revision and actual touch paths in the verification note; correct the old
  landscape/portrait label without erasing its historical result.
  **Acceptance:** Each claimed visual result points to a current readable image
  and each interaction claim states what was actually exercised. No duplicate
  capture folders or stale template image are presented as shipping UI evidence.
  **Verify:** Reopen every replaced image and perform the `UI_STYLE.md` §9 review:
  dominant board, 2–3 attention regions, one home per fact, readable text,
  separated utilities, discoverable touch actions and understandable final state.
  **Current remaining:** The stale template gameplay image, idle/win/free-spin/
  hint captures and browser note were replaced or refreshed, and generated
  review folders were removed from the project. The full traceability review
  remains open until the final verification matrix is green.

## Settings and display

- [x] Add a Fullscreen toggle to Settings and persist/apply the player's display preference.
