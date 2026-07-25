# Dragon's Hoard — Game Design Document

A dragon-themed slot machine built in Rust + `macroquad`, wired to
`macroquad-toolkit`. This GDD is both the design spec and the build plan; it
maps every system onto the template's existing `data → state → engine → ui`
skeleton so implementation is mechanical rather than exploratory.

---

## 1. Concept

**Dragon's Hoard** is a 5-reel, 3-row video slot themed around a dragon guarding
a treasure vault. The player wagers credits, spins the reels, and wins by
landing matching symbols on paylines. The fantasy is *raiding the dragon's
hoard*: coins and gems pay small, treasure and dragon eggs pay big, and the
dragon itself is the wild that completes lines and drives the bonus features
(Free Spins and an Egg-collection "hatch" meta).

- **Genre:** Casino / slot machine (single-player, offline, play-money)
- **Session:** 30 seconds to a few minutes; instantly resumable via save slot
- **Platforms:** WebGL (browser) + native Windows, via the standard publish pipeline
- **Logical resolution:** 1280 × 720 (`ui::LOGICAL_WIDTH/HEIGHT`, unchanged)
- **Tone:** warm, glowing gold-on-dark-stone; satisfying spin/stop/win feedback

> This is a play-money arcade slot for the WebHatchery catalog — **no real
> money, no purchases, no wagering of value.** Credits are a local score.

---

## 2. Core Loop

```
Idle ──(Spin, if balance ≥ bet)──▶ Spinning ──▶ Decelerate/Stop
  ▲                                                    │
  │                                                    ▼
  └──── Payout (credit wins, floating text) ◀──── Evaluate paylines
                     │
                     ├─ 3+ Scatters ─▶ Free Spins sub-loop (auto-spins, no cost)
                     └─ Egg symbols  ─▶ add to Hoard meter ─▶ full = Hatch bonus
```

1. Player sets **bet** with `−` / `+` controls. All 20 paylines are always
   active (lines are **not** player-adjustable in v1); total bet = line bet × 20.
2. Player presses **Spin** (button or `Space`). Balance is debited by total bet.
3. Reels spin, then stop left-to-right with a staggered deceleration.
4. Engine evaluates all active paylines against the final grid.
5. Wins are summed, credited, and shown as floating text + line highlights.
6. Feature triggers (Scatter → Free Spins, Eggs → Hoard meter) resolve.
7. Return to Idle. Autosave.

---

## 3. Symbols & Paytable

5 low symbols, 3 high symbols, plus Wild and Scatter. All values are
**data-driven** (see §7) — the table below is the *initial balance*, not
hardcoded truth.

| Symbol (id)             | Tier    | Role            | x3   | x4   | x5    |
|-------------------------|---------|-----------------|------|------|-------|
| Copper Coin (`copper`)  | Low     | filler          | —    | 11   | 40    |
| Gold Coins (`gold`)     | Low     | filler          | —    | 19   | 58    |
| Green Gem (`jade`)      | Low     | filler          | 8    | 29   | 76    |
| Blue Gem (`sapphire`)   | Mid     | filler          | 14   | 46   | 115   |
| Ruby (`ruby`)           | Mid     | filler          | 17   | 71   | 144   |
| Treasure Chest (`chest`)| High    | premium         | 36   | 142  | 384   |
| Dragon Egg (`egg`)      | High    | premium + meter | 44   | 192  | 480   |
| Dragon (`dragon`)       | **Wild**| substitutes all | 88   | 374  | 960   |
| Dragon Fire (`fire`)    | **Scatter** | triggers FS | pays anywhere: 3→2× bet, 4→10×, 5→50× |

The two lowest symbols **do not pay 3-of-a-kind** — they start at four. On a
20-line machine with only nine symbols, paying every symbol from three pushes
hit frequency past 55%; starting the lows at four lands it at ~41% (§4) without
touching the premium feel.

Payout values are **multipliers of the line bet** (not total bet). Wins pay
**left-to-right** starting from reel 1, longest matching run per line.

**Wild (Dragon):** substitutes for any symbol except Scatter to complete lines.
When a Wild lands on a reel it may **expand** to fill its whole column during
Free Spins (feature flag, tunable). When a line could pay multiple ways (e.g.
`W W W Chest Chest` pays as 3× Wild = 100 or 5× Chest = 400), the evaluator
**takes the best-paying interpretation** — this is the classic slot-evaluator
bug and gets explicit unit coverage (§11).

**Scatter (Dragon Fire):** pays anywhere on the grid (position-independent) and
**3+ trigger Free Spins**. Scatter wins multiply *total* bet, not line bet.

**Dragon Egg meta:** every Egg that lands adds to the persistent **Hoard
meter**. Filling it (e.g. 15 eggs) awards a **Hatch bonus** (instant credit
prize + a short celebratory sequence), then resets the meter. To avoid the
"collect at min bet, cash out at max bet" exploit, each egg contributes its
landing **line bet** to a persistent *hatch pot*; the Hatch prize is a
multiplier of that pot, not of the bet at hatch time.

---

## 4. Math Model (RTP)

- **Target RTP:** ~95% (tunable via reel strips + paytable; verified by sim).
- **Two machines, both verified.** Figures below are Dragon's Hoard; Frost Wyrm
  measures **0.9475** at a **0.259** hit frequency (§5.8). The sim runs every
  machine in the catalog — a second cabinet is a second maths model.
- **Measured:** RTP **0.9567**, hit frequency **0.410** over 1,000,000 spins
  (`cargo test --release -- --ignored --nocapture`). Contributions as a fraction
  of turnover: base game 0.665, free spins 0.193, Hatch bonus 0.058,
  **progressives 0.041**. (Scatter pays, 0.009, are a subset of the base and free
  figures.) Re-run and update these numbers whenever the data JSON changes.
- **The jackpot layer forced a retune.** Adding progressives (§5.6) put ~4 points
  on top of an already on-target game, taking it to 0.9996. The paytable was
  scaled down ~5.5% to make room. This is the intended workflow: the sim measures,
  the JSON absorbs the change, and no Rust logic moves.
- **Volatility:** medium — frequent small coin/gem wins, rare Treasure/Dragon lines.
- **Strip shape:** five 40-symbol strips. Each carries exactly **one scatter**
  (a second one roughly quadruples the free-spin trigger rate, and free spins
  compound through the ×2 multiplier and retriggers — it was worth +50% RTP on
  its own). Wilds are deliberately scarce on the outer reels: **1 on reels 1 and
  5, 2 on reels 2–4.** Because runs must start on reel 1, wild density there is
  the single strongest RTP lever in the whole data set.
- **Reels are strip-based:** each of the 5 reels is an ordered list ("strip") of
  symbol IDs. A spin picks a random stop index per reel via `SeededRng`; the 3
  visible cells are `strip[stop]`, `strip[stop+1]`, `strip[stop+2]` (wrapping).
  Symbol frequency on each strip *is* the math — Dragon/Treasure appear few
  times, coins many times.
- **Hit frequency:** aim ~30–40% of spins return something (mostly small).
- **Verification:** a `#[cfg(test)]` Monte-Carlo test spins the engine with a
  fixed seed and asserts measured RTP lands within a tolerance band of target.
  The sim must run the **full engine** — Free Spins sub-loops (multiplier,
  expanding wilds, retriggers), egg accrual, and Hatch payouts — because those
  features contribute real EV; a base-game-only sim would understate RTP.
  RTP is a property of the data JSON (`reels.json` + `symbols.json` paytable +
  `paylines.json` + `freespins.json` + hoard config) — never of Rust logic —
  so designers tune the JSON and re-run the sim. CI runs a smaller-N smoke
  version (`cargo test` is a debug build; 1M full spins would be slow); the
  full N=1,000,000 run is `#[ignore]`d and run via
  `cargo test --release -- --ignored` when tuning.

Determinism: all outcome-affecting randomness flows through a single state-owned
`SeededRng` (serde-serializable, saved with the session) — cosmetic-only
randomness (particle jitter) may use the shared generator. No `Math::random`,
no wall-clock seeding in gameplay.

---

## 5. Features

### 5.1 Free Spins (Scatter-triggered)
- 3 / 4 / 5 Scatters award **10 / 15 / 20** free spins.
- Free spins replay the core loop at the **triggering bet**, cost nothing.
- During Free Spins: **Wilds expand** and a **win multiplier** (×2, tunable)
  applies to **line wins only** — Scatter pays are *not* multiplied (they
  already retrigger). Additional 3+ Scatters **retrigger** (+N spins).
- A dedicated `FreeSpins` sub-state tracks `remaining`, `multiplier`,
  `total_won`; a banner + counter replace the normal control panel.

### 5.2 Dragon's Hoard Meter (Egg collection)
- Persistent counter of Dragon Eggs collected across spins (survives save/load),
  plus the **hatch pot** (each egg banks its landing line bet — see §3).
- Reaching capacity triggers the **Hatch** bonus: an instant credit prize of
  `hatch_pot × hatch_pot_multiplier`, a floating-text/particle celebration,
  screen shake, then both counter and pot reset.
- Gives a long-horizon goal layered over individual spins.

### 5.3 Autospin
- Spin N times automatically (`autospin_spins`, default 25). One button: it
  starts a run when idle and stops one that is running.
- **Stops early on anything worth watching** — a free-spin trigger, a Hatch, a
  win of `big_win_multiple` total bets or more, or an empty balance. Each stop
  reason carries its own message, so the counter never just quietly halts. An
  autospin that ploughs through a feature has taken the moment away from the
  player, which is the whole point of the stop conditions.
- **Free spins are not billed to the run.** They cost nothing, so burning an
  autospin on one would short-change the player; the run is suspended while the
  feature plays and its budget is untouched.
- The bet ladder is locked for the duration — the run was started at a stake the
  player chose.

### 5.5 Celebration cards (feature presentation)
- A queue of full-screen cards: free-spins entry, retrigger, feature summary,
  Hatch, and big win.
- **A showing card holds the game.** The reels do not turn, the payout does not
  count, and free spins do not chain. Without this the auto-chain runs straight
  through the trigger and buries the moment the whole game is built around.
- Dismissed with space/click, or after a few seconds on their own.
- Free spins also re-skin the reel cabinet (ember chrome, feature title) so the
  player can see at a glance that the rules on screen are not the base rules.

### 5.6 Progressive jackpots (post-v1)

Four pots — Mini, Minor, Major, Grand — that grow off every stake and pay out at
random. `assets/data/jackpots.json` is the whole configuration.

**The trigger is a roll, not a symbol.** Tying jackpots to a reel combination
would mean re-cutting the strips, and the strips *are* the RTP (§4) — every tier
would drag the base game around with it. A mystery trigger keeps the two layers
mathematically separate: the reels pay what they always paid, and the jackpots
add an exactly computable slice on top.

**Odds are expressed per credit wagered** — "one hit per N credits of turnover".
That makes the trigger **bet-fair by construction**: doubling the stake doubles
the chance, so expected return per credit is identical at every rung of the bet
ladder. It is the same principle that makes the hoard bank each egg at its
landing bet (§3), and it is why a player cannot farm a jackpot cheaply and cash
it in expensive.

It also makes the maths **closed-form**. Exactly `odds` credits are wagered
between wins by definition, so a tier returns:

```
seed / odds  +  contribution_rate × share
```

Summed over the shipped tiers that predicts **0.0380** of turnover; a 4M-spin sim
measures **0.0391**. `the_jackpot_layer_matches_its_closed_form` asserts the two
agree — a check that catches a contribution or trigger bug which a total-RTP band
would quietly absorb.

Pots accrue in **milli-credits**, so a 20-credit minimum spin still moves the
smallest tier instead of rounding to zero. No floating point anywhere (§12).

Other rules:
- **Free spins neither feed nor draw a jackpot.** They staked nothing, so they
  contribute nothing and cannot win the pots. Both halves are tested.
- A jackpot **suppresses the big-win card** — otherwise the same money is
  announced twice, with the smaller headline second — and stops an autospin run.
- The ladder is drawn above the reels, where a real cabinet puts it, and the pots
  survive save/load.

### 5.7 Settings (post-v1)

An overlay reachable from the header or `O`, with five rows: **Sound** (master
volume in tenths, down to true silence), **Spin Speed**, **Autospin Length**,
**Screen Shake** and **Particles**.

The shared half is the toolkit's `GameSettings` — volumes, screen shake, UI
scale — rather than a private reimplementation, since every game in the
workspace already stores those under the same shape. `state/preferences.rs`
wraps it with the two genuinely slot-specific settings.

**Preferences are not part of the save slot.** They persist under their own
`preferences` key, so a New Game, a Load, or a deleted save all leave them
alone. A test asserts a save round-trip does not carry them.

**Spin Speed scales time, not outcomes.** `SpinSpeed::time_scale` multiplies
every duration in `state/spin.rs` — reel travel, payout count-up, the auto-spin
beat — while leaving `travel` untouched. A Turbo spin is the same spin, revealed
sooner: the outcome was decided before either started (§8.2). Two tests hold that
line: one asserts Normal and Turbo from one seed land on the same grid and the
same balance while Turbo takes fewer frames, the other that no speed compresses
the durations so far that two reels land in a single frame and a `ReelStopped`
event is lost.

**Screen shake and particles are separate toggles**, not one "reduced motion"
switch. Shake is the one that troubles motion-sensitive players; some want it off
and the sparkle kept. Both are hard gates in `game.rs`, routed through
`add_trauma`/`burst` helpers so no call site can bypass them.

Being able to reach **silence** matters here more than usual: the effects are
synthesised (§7.1) and have never been listened to.

### 5.8 Multiple machines (post-v1)

The catalog ships two cabinets, and the whole difference between them is JSON.

| | Dragon's Hoard | Frost Wyrm |
|---|---|---|
| Volatility | medium | high |
| Hit frequency | 0.410 | 0.259 |
| Strip length | 40 | 50 |
| Symbols paying from 3 | 7 of 8 | 5 of 8 |
| Free spins | 10/15/20 at ×2 | 8/12/18 at ×3 |
| Hoard | 15 eggs, ×1 pot | 20 eggs, ×2 pot |
| Grand jackpot seed | 25,000 | 40,000 |
| Biggest win in 1M spins | 73,758 | 99,720 |
| Measured RTP | 0.9567 | 0.9475 |

**`GameData` is still exactly one machine's worth.** Switching rebuilds it
rather than indexing into a collection — which is why adding the second cabinet
touched almost no other module: every `data.config.*` and `data.symbols` reader
kept working unchanged. The only Rust a machine needs is its entry in
`MACHINES`, because `include_str!` runs at compile time.

**Each machine has its own save slot** (`<machine>_<slot>`), so a balance and
hoard built on one cabinet are never overwritten by the other. A test asserts
the slots are distinct — sharing one would silently destroy progress. The
last-played machine is remembered in preferences, and an id that no longer
exists falls back to the first rather than stranding the player outside the
game.

**The sim tests every machine, not just the one that boots.** A second cabinet
is a second maths model; without `every_machine_loads_and_lands_in_band` and its
long-run counterpart, a new machine could ship at any RTP and nothing would
notice. A further test asserts the machines actually differ in hit frequency —
proof the catalog offers a choice rather than a reskin.

Frost Wyrm reuses every procedural art routine (§7.1) with new colours and
names, which is the payoff for making `art` a data key: a whole second symbol
set cost no new drawing code.

### 5.9 Achievements (post-v1)

Twelve goals in `assets/data/achievements.json`, each an id, name, description
and one `condition` — a counter and a threshold. The registry itself is the
toolkit's `Achievements`; what `state/achievements.rs` adds is the part the
toolkit cannot know: **what counts as earning one.**

**Progress is cumulative and cross-machine.** `SessionStats` is per-machine
because it lives in a machine's save slot (§5.8), so counting from it would
silently reset "1,000 spins" every time the player walked to another cabinet.
`AchievementProgress` keeps its own running totals, folded in from every settled
spin whichever machine raised it, and persists under its own key alongside
preferences (§5.7).

**Definitions are read from JSON every load; only unlock flags and counters are
restored.** Renaming an achievement therefore takes effect immediately, and
adding one cannot invalidate a save — the same "editing the data must not
strand a player" rule as the jackpot tiers and the autospin ladder.

The `Balance` condition is a **high-water mark**, not a current reading: holding
25,000 once earns Hoarder even if it is lost again, because punishing a player
for continuing to play is the wrong incentive. Achievements are written the
moment one unlocks rather than on the autosave beat — losing one to a crash
would sting more than losing a spin's worth of credits.

The overlay shows locked entries with their progress (`18 / 100`) rather than
greying them out: a goal you cannot see the shape of is a surprise, not a goal.

### 5.10 The Vault Pick (post-v1)

Filling the hoard no longer pays on the spot. It deals a board of twelve chests;
the player picks until three come up empty, and each prize revealed adds a share
of the hatch prize.

**It replaces a payout rather than adding one.** Every prize is *permille of the
hatch base*, and the table is built so the expected sum is 1000‰ — so the bonus
pays what the instant hatch paid, on average, and only the **variance** changes.
That is why a whole interactive feature could be added to a game already tuned to
0.9567 and 0.9475 without re-cutting a single reel strip: measured after, Frost
Wyrm was unmoved at 0.9475 and Dragon's Hoard shifted 0.9567 → 0.9591, drift from
the RNG stream rather than from the feature. A deliberate contrast with the
jackpot layer (§5.6), which cost a full paytable retune.

The same normalisation is why **one shared `bonus.json` serves both machines**:
each cabinet's own `hatch_pot_multiplier` already scales the base, so the board
does not need to know which machine it is on.

**The board is dealt at trigger, not at pick.** Contents are drawn and shuffled
from the session RNG the moment the bonus opens, exactly like reel stops (§8.2).
Picking only reveals a decided board — which is what lets the headless path
auto-play it so the sim measures the real feature. Because the board is shuffled,
picking in index order is statistically identical to picking at random, so the
sim's auto-play is honest rather than a convenient fiction. A test asserts
hand-picking and auto-play reach the same total from one seed.

`expected_permille` gives the feature's value in closed form: with `p` prizes and
`b` blanks shuffled together, `p · b / (b + 1)` prizes are revealed before the
last blank. Two tests hold the table to it — one against the formula, one against
20,000 simulated boards.

An open board **holds the game** for the same reason a celebration card does
(§8.2.1): the reels must not turn and the auto-chain must not run on underneath
it. `is_settled()` gained a third clause. Re-picking a revealed chest is ignored
rather than an error — a double click must not cost the player a blank.

### 5.11 Reel feel — blur, bounce, anticipation (post-v1)

Everything above is maths the player cannot see. This is the opposite: no number
moves, and the difference is entirely in how a spin *reads*. Verified by the sim,
which measures both cabinets unchanged at **0.9591** and **0.9475** after the
work — as it must, since none of it touches the outcome.

**Real motion blur.** A fast reel used to drop its symbol labels and fall back to
a flat colour tint, which reads as "the art vanished" rather than "the reel is
moving". Now the strip is drawn five times per frame across the distance it
covers, each pass at a fifth alpha, so the symbols streak. The one thing that
made this hard was discovered from a capture: drawing all five passes whole
stacked five translucent *tiles* as well, which summed toward white and bleached
the vault. Cells are now split into a tile and its art (`StripLayer`); the tiles
go down once at the reel's true position, and only the art repeats. Drawing the
tiles on the leading pass instead — the obvious first fix — left them visibly
trailing the art they were supposed to sit under.

**A landing bounce.** The last 18% of a reel's travel overshoots and settles back
by a sixth of a cell, decaying to nothing. It is presentation only, and a test
holds it to that: for every target, the reel must still come to rest exactly on
its decided stop.

That test found a bug that had been there since the animation was written.
Revolutions were `2.0 + index * 0.5`, so reels 2 and 4 turned two and a half
times and landed **exactly half a strip** from their decided stop — twenty
symbols out — then popped to the right symbols the instant the resting draw took
over. Every landing test happened to use an even reel index, so nothing caught
it for five iterations. Revolutions are now whole (`2 + index / 2`), and a second
test asserts the invariant directly: every reel's travel is a whole number of
strip lengths. The landing test now sweeps all five reels rather than one.

**Anticipation.** When the scatters still showing could complete the free-spins
trigger, the reels that could complete it are stretched to 2.6× their spin time —
the held breath a physical cabinet draws out, and the reason a near-miss is
agonising rather than instant. The held reel gets an ember-bordered frame so the
pause reads as the game making something of the moment rather than as a stutter.

Two details matter. The trigger count is **derived** from the free-spins award
table (`FreeSpinsConfig::trigger_count`) rather than configured separately, so
the two can never disagree. And *every* reel still to land is held, not just the
next one — with two scatters showing and three reels to go, any of the three
could be the third. A cap of `reel_count - 1` keeps a scatter-rich board from
turning one spin into a slideshow.

The effect lasts under a second and depends on where the scatters fall, so it
cannot be photographed by waiting. The `anticipation` capture scene searches for
a spin that raises it and freezes at the moment the held reels are the only ones
still turning.

### 5.12 The Dragon's Wrath — hold and spin (post-v1)

The one modern slot mechanic the game was missing. Four or five Dragon Eggs on
one grid wake the dragon: those eggs lock in place as coins, each stamped with a
credit value, and the player gets three respins. Every respin rolls only the
cells still empty, and **any coin that lands restores the allowance in full** —
so the round is not three spins, it is "three spins without a coin". Fill all
fifteen cells and a large flat bonus pays on top.

**The trigger reuses the egg rather than adding a coin symbol.** Hold-and-spin
normally wants a dedicated symbol on the strips, and the strips *are* the RTP
(§4) — adding one means re-cutting all five and retuning everything downstream.
The egg is already on the strips, already the game's collectible (§5.2), and
already rare in quantity. It now feeds the hoard *and* opens the round from the
same grid.

**It adds EV, and the paytable pays for it.** This is the jackpot bargain (§5.6),
not the Vault Pick one (§5.10): a genuinely new prize, measured by the sim and
absorbed by the JSON. The first tuning pass shows why the sim is not optional —
at a four-egg trigger the feature was worth **0.367 of turnover** and took RTP to
**1.32**. Raising the trigger, trimming the coin chance and scaling both
paytables ~2–3% down landed it back at 0.9612 and 0.9450.

**The config had to become per-machine, and the sim is what proved it.** It
started shared like `bonus.json` — every value is a multiple of total bet, so it
looked machine-independent. It is not: the *trigger* reads the strips, and Frost
Wyrm's strips carry fewer eggs. At a five-egg trigger Frost fired the feature
**23 times in a million spins** — a player would never once have seen it. The
Vault Pick can be shared precisely because it is triggered by something already
normalised (a full hoard); anything triggered off the reels cannot be.

| | Dragon's Hoard | Frost Wyrm |
|---|---|---|
| Trigger | 5 eggs | 4 eggs |
| Rounds per 1M spins | 548 (1 in 1,825) | 942 (1 in 1,062) |
| Share of turnover | 0.0161 | 0.0250 |

**Awarding the Grand on a full board was considered and rejected.** It is the
iconic version of the mechanic, and the game already has progressives. But the
jackpot layer's return is asserted against a closed form assuming one bet-fair
trigger per credit wagered (§5.6); a second route into the same pot would
invalidate that check. Losing a test that can catch a contribution bug costs more
than the moment is worth, so the full board pays a configured multiple of total
bet instead.

**The round advances itself.** Unlike the Vault Pick there is nothing to click,
so it runs on a beat (`HOLD_SPIN_BEAT`) rather than waiting on input, and
`ui/holdspin.rs` returns no `UiAction` at all. It still *holds* the game exactly
as a card or an open board does — `is_settled()` gained a fourth clause — and it
tears down an autospin run rather than merely pausing it, or the run would
resume the instant the round ended and take the board away.

It renders **into the reel window itself** rather than as a second screen, so the
coins sit in the cells their eggs landed in and the round visibly grew out of the
spin. `state/features.rs` now holds the resolve-and-pay tail of both second-screen
features side by side, which is where the shape they share is easiest to see.

The feature has **no closed form** — the coin count is a Markov process with
resets — so it is pinned by measurement instead: a 20,000-round test asserts the
band (a round pays 20–80× total bet; the shipped table gives 32.7× over 8.1
coins, filling 0.4% of the time) and a separate test checks the coin table's
weighted mean, which *is* closed-form.

### 5.4 Juice / feel (toolkit FX)
- Reel deceleration with easing (`Tween` / easing curves).
- Winning lines: pulse highlight (`blink`/`pulse`), floating win amounts
  (`FloatingTextLayer`), particle burst (`ParticleSystem`) on premium wins.
- `ScreenShake` (trauma) on big wins / hatch. `ScreenFade` on feature entry.
- SFX via toolkit `sound.play_sfx(...)`: spin start, reel stop (per reel),
  small win, big win, scatter, coin cascade. Volume respects settings.

---

## 6. UI Layout (1280 × 720)

```
┌──────────────────────────────────────────────────────────────┐
│  HEADER: Dragon's Hoard      Balance: 💰 1,250   Hoard 🥚 8/15 │
├───────────────────────────────────────────┬──────────────────┤
│                                            │   WIN: 0          │
│   R1     R2     R3     R4     R5           │   ────────────    │
│  ┌──┐   ┌──┐   ┌──┐   ┌──┐   ┌──┐          │   Line Bet:  −10+ │
│  │🪙│   │🐉│   │💎│   │🗝│   │🔥│  row 0    │   Lines:     20   │
│  ├──┤   ├──┤   ├──┤   ├──┤   ├──┤          │   Total Bet: 200  │
│  │💎│   │🪙│   │🥚│   │💰│   │💎│  row 1    │                   │
│  ├──┤   ├──┤   ├──┤   ├──┤   ├──┤          │   ┌────────────┐  │
│  │🐉│   │💰│   │🪙│   │💎│   │🪙│  row 2    │   │   S P I N  │  │
│  └──┘   └──┘   └──┘   └──┘   └──┘          │   └────────────┘  │
│         (payline overlays on win)          │   [Auto] [Max]    │
├───────────────────────────────────────────┴──────────────────┤
│  FOOTER: last win / feature banner / paytable toggle           │
└──────────────────────────────────────────────────────────────┘
```

- **Reels panel** (left, ~812 wide) reuses the template's `draw_surface_with_title`
  chrome. Each cell renders a symbol texture (or a colored-rect + short-code
  placeholder early on — see §7).
- **Control panel** (right, ~410 wide) reuses `virtual_button`, `meter`, badges.
- Free Spins mode swaps the control panel for a counter + multiplier banner.
- A **Paytable overlay** (toggle) lists symbol values — data-driven from JSON.

The existing grid/camera/fog demo in `ui.rs` is **removed**; the reel grid and
control panel replace it. The template's header/footer/badge/button/meter
helpers are kept and re-skinned to a gold-on-stone palette.

---

## 7. Data-Driven Content (assets/data/*.json)

Per repo hard rule, all balance/content lives in JSON loaded via `serde` /
`data_loader`, embedded with `include_str!` for WASM (mirroring `data.rs`).
Files to add/replace under `assets/data/`:

- **`game_config.json`** — reshaped `GameConfig`:
  ```json
  {
    "game_name": "dragons_hoard",
    "display_name": "Dragon's Hoard",
    "save_slot": "autosave",
    "version": "1.0.0",
    "starting_balance": 1000,
    "reel_count": 5,
    "row_count": 3,
    "line_bets": [1, 2, 5, 10, 25],
    "default_line_bet_index": 3,
    "hoard_capacity": 15,
    "hatch_pot_multiplier": 2,
    "free_spin_multiplier": 2
  }
  ```
- **`symbols.json`** — id, display name, tier, `is_wild`, `is_scatter`, texture key,
  and paytable `{ "3": n, "4": n, "5": n }`.
- **`reels.json`** — 5 strips, each an ordered `["copper","dragon","gem",...]`.
  **This file is the RTP.** Editing it is how designers tune the game.
- **`paylines.json`** — array of 20 lines, each 5 row-indices `[1,1,1,1,1]`
  (middle), `[0,0,0,0,0]` (top), zig-zags, etc.
- **`freespins.json`** — scatter-count → spins-awarded, retrigger rules, multiplier.
- **`texture_manifest.json`** — symbol PNGs (keep template's manifest pattern).

### 7.1 Art and audio are generated, not shipped

The placeholder-then-swap-in-PNGs plan above was overtaken: **the game ships no
image or audio assets at all.** Both are produced in code.

- **`ui/symbols.rs`** draws each symbol from macroquad primitives — a coin, a
  coin stack, a cut gem, a chest, an egg, a dragon head, a flame. `symbols.json`
  gains an `art` key naming the shape, so which art a symbol uses stays data.
  Every piece is tinted from the single `color` already in the JSON, so
  re-theming is a data edit; the art scales to any cell size with no mipmap; and
  a win highlight brightens the art itself rather than washing a sprite. An
  unrecognised `art` value falls back to the three-letter code, so adding a
  symbol can never render an empty cell.
- **`audio.rs`** synthesises each effect as 16-bit PCM and hands the bytes to
  `macroquad::audio::load_sound_from_bytes`. An effect is a list of `Voice`s —
  waveform, pitch glide, attack/decay envelope, start offset — so tuning the
  game's sound is editing one function. Noise voices draw from a `SeededRng`, so
  a given commit always produces byte-identical audio.

The manifest and `AssetManager` are still wired up for anything that genuinely
needs a texture later; they simply load nothing today. (Emoji were never an
option: macroquad's default font has no emoji glyphs, so `draw_text("🐉")`
renders tofu.)

**`audio.rs` is a candidate for promotion into `macroquad-toolkit`.** The
toolkit's `SoundManager` can only load from files or asset packs, so every game
wanting a placeholder blip has to source a `.wav`. It is kept project-local for
now because changing the shared crate touches every other game in the workspace.

---

## 8. Technical Architecture (mapped to the template)

Follow the toolkit's UI-is-a-pure-view-layer rule: UI returns `UiAction`/intents,
a dispatcher applies them, engine is stateless (state in → results out). Keep
every `.rs` under the **800-line hard limit**; split by responsibility.

### 8.1 Module plan

| File | Responsibility |
|------|----------------|
| `src/main.rs` | Unchanged skeleton: `window_conf` (rename env prefix to `DRAGONS_HOARD` — must match the Cargo package name, since `capture_ui.ps1` derives the prefix from `cargo metadata`), capture harness, main loop. |
| `src/data.rs` | Load `GameConfig`, `symbols`, `reels`, `paylines`, `freespins` from embedded JSON into a `GameData` (registries + strips + paytable). |
| `src/state.rs` | `GameSession`: `balance`, `line_bet_index`, `grid`, `phase`, `free_spins`, `hoard`, `celebrations`, `autospin`, state-owned `SeededRng`. All paylines are always active — no `active_lines` field. |
| `src/state/hoard.rs` | `HoardState` — the egg meter and its hatch/carry-over arithmetic. |
| `src/state/save.rs` | `SaveData`, `SessionStats`, `migrate_save_value`. |
| `src/state/autospin.rs` | `AutospinState` + `AutospinStop` (why a run ended). |
| `src/state/celebration.rs` | `CelebrationKind`/`Celebration`/`CelebrationQueue` — the cards that hold the game. |
| `src/state/jackpot.rs` | Progressive pots: contribution, bet-fair trigger, closed-form RTP (§5.6). |
| `src/state/preferences.rs` | Player settings on top of the toolkit's `GameSettings` (§5.7). |
| `src/state/achievements.rs` | Unlock conditions and cross-machine progress (§5.9). |
| `src/state/bonus.rs` | The Vault Pick board, deal and auto-play (§5.10). |
| `src/ui/bonus.rs` | The pick board renderer. |
| `src/ui/achievements.rs` | The achievements overlay. |
| `src/ui/settings.rs` | The settings overlay. |
| `src/ui/paytable.rs` | The paytable overlay (split out of `ui.rs` at the size limit). |
| `src/ui/machines.rs` | The machine picker (§5.8). |
| `src/state/tests.rs` | Session integration tests: spin lifecycle, features, autospin, cards. Split at the size limit into `tests/{jackpots,preferences,machines}.rs`. |
| `src/ui/celebration.rs` | Full-screen card rendering. |
| `src/ui/symbols.rs` | Procedural symbol art (§7.1). |
| `src/audio.rs` | WAV synthesis + the sound bank (§7.1). |
| `src/engine/reels.rs` | Pure: pick stop indices from strips via RNG → produce the 5×3 result grid. |
| `src/engine/evaluate.rs` | Pure: given grid + paylines + paytable → `SpinOutcome` (line wins, scatter win, egg count, feature triggers, total). |
| `src/engine.rs` | Re-exports `reels` + `evaluate`; a `spin(data, &mut rng, bet)` orchestrator returning `SpinOutcome`. |
| `src/state/spin.rs` | `SpinState` machine: `Idle → Spinning{per-reel timers} → Resolving → Payout → Idle`, plus `FreeSpins`. Owns reel animation progress. |
| `src/ui.rs` | Thin: draws header/reels/controls/footer, returns `Vec<UiAction>`. Delegates reel drawing to `ui/reels.rs`. |
| `src/ui/reels.rs` | Reel strip rendering, spin blur/scroll, win-line overlays, symbol drawing. |
| `src/actions.rs` | Dispatcher: interprets `UiAction` (Spin, BetUp/Down, MaxBet, ToggleAuto, TogglePaytable, Save/Load/New) against session + engine. |

`game.rs` stays the orchestrator (owns `GameData`, `GameSession`, `AssetManager`,
`NotificationManager`, `EventBus<UiAction>`, FX managers); its `apply_action`
grows the new `UiAction` variants and delegates to `actions.rs`.

### 8.2 Spin state machine (in `state/spin.rs`) — built

```rust
enum SpinPhase {
    Idle,
    Spinning(ReelSpinner),      // per-reel closed-form animations
    Payout(PayoutCounter),      // win count-up
    AutoPause(Timer),           // beat between free spins
}
```

**The outcome is decided when the player commits, not when the reels stop.** The
original sketch had `engine::spin` run on full settle, but a reel has to know
where it is landing in order to decelerate onto it — otherwise it snaps. So
`GameSession::begin_spin` takes the stake, rolls the outcome, and stores it as a
private `PendingSpin`; the reels then reveal a decision already made. Winnings
are not applied until `update_spin` sees every reel land, which is what keeps the
balance from jumping ahead of the animation.

The session therefore has two entry points sharing one implementation:

| | `spin()` | `begin_spin()` + `update_spin()` |
|---|---|---|
| Used by | sim, tests, capture harness | play |
| Reels | snap | animate |
| Both call | `roll_spin` → `settle_spin` | `roll_spin` → … → `settle_spin` |

A test asserts the two paths produce identical grids, balances and hoards from
the same seed, which is what proves the animation consumes no randomness of its
own.

`ReelAnimation` positions itself as a closed-form function of elapsed time
(`start + travel · ease_out_cubic(t)`) rather than integrating a velocity, so a
reel lands on exactly the right symbol regardless of how the frame times fall —
tested against a deliberately ragged frame sequence.

`update_spin` returns `Vec<SpinEvent>` (`ReelStopped`, `Settled`, `PayoutFinished`,
`AutoSpinReady`, `CelebrationOpened`) which `game.rs` turns into shake, particles,
floating text, notifications and autosaves. Free spins *and* autospin re-enter
through `AutoSpinReady` raising a normal `UiAction::Spin`, so every spin in the
game — manual or automatic — goes through the same dispatcher path.

UI only *reads* phase to decide what to draw; it never mutates it.

### 8.2.1 What "busy" means

Three separate things can be in flight, and conflating them caused real bugs:

- `phase.is_busy()` — reels turning or a payout counting.
- `celebrations.is_active()` — a card is up.
- `is_settled()` — **neither**. This is the guard `can_spin`, `begin_spin` and
  the save/load buttons use. An earlier version checked only `phase.is_idle()`,
  which left Save enabled behind a celebration card.

`update_spin` ticks the card queue *first* and returns early while a card is up,
which is what makes a card hold the reels. `CelebrationQueue::skip` promotes the
next card in the same call rather than waiting for the next tick, so there is
never a single frame with nothing showing and the reels free to advance.

One subtlety worth keeping: `CelebrationQueue::push` makes the first card active
immediately, but the "it opened" event has to come out of `update`. A per-card
`announced` flag carries it across — without it the first card of every sequence
(which is most of them) silently lost its particles and shake.

### 8.3 UiAction additions
`Spin`, `BetUp`, `BetDown`, `MaxBet`, `ToggleAutospin`, `TogglePaytable`,
`DismissCelebration`, plus the existing `NewGame`, `Save`, `Load`, `DeleteSave`.
(Removed `RunAction`, `SelectTile`.) While a card is showing, the spin key maps
to `DismissCelebration` instead — one key always moves the game forward.

### 8.4 Toolkit systems used
`SeededRng` (outcomes), `VirtualUi` + `SurfaceStyle`/`ButtonStyle`/`TextStyle`
(chrome), `meter`/badges (balance/hoard), `Tween`/easing + `Timer`/`IntervalTimer`
(reel timing), `FloatingTextLayer`/`ParticleSystem`/`ScreenShake`/`ScreenFade`
(juice), `NotificationManager` (feature messages), `AssetManager` (symbol textures),
persistence save slots + migration, `sound.play_sfx` (audio), `capture` harness
(headless screenshots).

---

## 9. Persistence

`SaveData` (serde) holds: `version`, `balance`, `line_bet_index`, `hoard`
(`count` + `pot` as one nested struct), `stats` (`total_spins`, `total_wagered`,
`total_won`, `biggest_win`, `hatches`, `free_spins_played`), and the `SeededRng`
state. Uses the template's `save_to_slot_with_version` /
`load_from_slot_with_migration` verbatim; `migrate_save_value` handles older shapes
(reuse the existing legacy-fallback pattern) and re-clamps `line_bet_index` to the
current bet ladder. Autosave after each spin resolves — **but not mid-feature**:
an in-progress free-spin run is not persisted, so a reload lands back in the base
game. The grid is not saved either; load always shows a default (non-winning)
display grid.

---

## 10. Build Phases / Milestones

**Phase 0 — Rename & strip — ✅ DONE**
Package and env prefix renamed to `dragons_hoard` / `DRAGONS_HOARD` in
`Cargo.toml`, `main.rs`, and `scripts/capture_ui.ps1`; `game_page.json` rewritten
(title, `"wasm": "dragons_hoard"`, `roost_slug`, slot controls, play-money
framing). The fog/grid/camera demo is gone from `state.rs` and `ui.rs`, the
template's `assets/data/actions.json` and the stale `dist/` artifacts are
deleted, and the project is now its own git repo (`git init`, nothing committed
yet). `asset_packs.json` was kept — it is live pipeline config, not demo data.

**Phase 1 — Data & engine, headless — ✅ DONE**
`symbols.json`, `reels.json`, `paylines.json`, `freespins.json` added and
`data.rs` rewritten around an index-resolved `Symbols` registry with validation
(strip/payline/row shapes, exactly one wild + scatter + hoard symbol).
`engine/reels.rs` and `engine/evaluate.rs` are pure and unit-tested, and
`engine/sim.rs` drives a **whole `GameSession`** — free spins, retriggers,
expanding wilds and the hoard all included — for the Monte-Carlo RTP run.

**Phase 2 — Playable static slot — ✅ DONE (instant spin, no animation)**
`GameSession` owns balance, bet ladder, hoard, free-spin state, stats and the
`SeededRng`; `actions.rs` is the intent dispatcher; `ui.rs` + `ui/reels.rs` draw
header / reel window / control panel / footer with placeholder symbol cells;
win-line highlighting, the paytable overlay, save/load/autosave and keyboard
shortcuts all work. Free spins and the Hoard/Hatch meta already resolve and pay
(Phase 4's *logic*, ahead of schedule) — what Phase 4 still owes is presentation.

**Phase 3 — Feel & animation — ✅ DONE except SFX**
`state/spin.rs` holds the phase machine (§8.2). Reels scroll the real strip at a
fractional position, clipped to the window, and stop left-to-right on a cubic
ease-out; fast reels drop their labels rather than faking a blur texture. Wins
pulse their cells, the WIN readout counts up, each line spawns a rising `+N`,
premium wins burst particles, reel stops and big wins add screen-shake trauma
(applied to the reels panel only). `game.rs` owns the FX managers and feeds them
from `SpinEvent`s. The capture harness gained real scenes — `idle`, `spin`,
`win`, `freespins`, `paytable` — via `Game::begin_capture_scene`, which uses the
headless spin path to fast-forward.

**SFX is the one outstanding item and is deferred to Phase 5:** the project has
no audio assets at all, and `sound.play_sfx` calls without them would be dead
code. Audio lands with the art pass.

**Phase 4 — Features — ✅ DONE**
`state/celebration.rs` adds the card queue (§5.5) and `state/autospin.rs` the
unattended run (§5.3). Free spins now announce themselves with a full-screen
entry card, re-skin the reel cabinet in ember while they run, and close with a
summary card carrying the feature total; retriggers get their own card. The Hatch
raises its own card with a 120-particle burst and heavy shake. `state.rs` was
over the 800-line limit by this point and was split into `hoard.rs`, `save.rs`
and `tests.rs` siblings as part of the same change.

**Phase 5 — Art, audio, polish, ship — ✅ DONE**
Symbol art and the SFX set are both **generated in code** rather than shipped as
assets (§7.1) — that is the one real departure from the original plan, and it is
what unblocked a phase that otherwise needed an artist and a sound designer.
`catalog_thumbnail.png` is produced by the capture harness itself. `publish.ps1`
ran clean: Windows release, WebGL release, asset pack, thumbnail, catalog entry,
and a Project Roost deployment record. Verified live — see §15.

---

## 11. Testing Strategy

- **Unit (`evaluate.rs`, in `engine/evaluate/tests.rs`):** line rules are asserted
  against `best_line_result` **directly, not through a grid**. With 20 weaving
  paylines there is no filler symbol that isolates a single line — whatever you
  pick forms its own runs on the zig-zags, so a grid fixture cannot assert
  "exactly one win" without re-encoding the payline set. Covered: 3-of-a-kind
  pays, 2 does not, runs must start on reel 1, longest-run wins, wild
  substitution, wild-led lines pay the **best** interpretation (`W W W Chest
  Chest` → 5× Chest, not 3× Wild), all-wild pays the wild value, and wilds never
  reach through or stand in for the scatter. Grid-level tests cover
  scatter-anywhere, egg counting, the free-spin multiplier and wild expansion.
- **Unit (`reels.rs`):** fixed-seed RNG → deterministic stop indices → expected grid.
- **Property/Monte-Carlo:** N-spin RTP within tolerance of target; hit-frequency sane;
  balance is conservative (credited == evaluated).
- **State machine (`state.rs`):** spin can't start with `balance < total_bet`;
  a second Spin mid-spin is refused without taking another stake; nothing is
  credited until the reels land; the reels come to rest on exactly the grid that
  was evaluated; reels report stopping left-to-right *before* the outcome is
  applied; Free Spins cost 0, chain themselves via `AutoSpinReady`, and stop
  asking once the feature is spent; retrigger adds spins; Hoard resets at
  capacity.
- **Celebrations (`state/celebration.rs`):** the first card announces itself
  (the bug that would otherwise cost most sequences their particles); cards
  announce exactly once each and in order; skipping never leaves a frame with
  no card showing; the backlog is bounded so a million-spin headless run cannot
  grow it; a card fades in, holds solid, fades out, and even the longest card
  reaches full opacity within `FADE_TIME`.
- **Autospin (`state.rs`, `state/autospin.rs`):** a run counts down and cannot
  go negative; it never survives a feature trigger, a hatch or a big win; it
  cannot start on top of a committed stake; a free spin is not billed to it.
- **Cards hold the game (`state/tests.rs`):** a showing card stops the reels
  advancing and the spin being counted; a spin is refused while a card is up,
  without taking a stake; ending the feature raises a summary card carrying
  every spin it awarded.
- **Symbol art (`ui/symbols.rs`):** every symbol in `symbols.json` names art the
  renderer actually has; an unknown name reports rather than draws; shades stay
  inside 0..1 even at full win-pulse brightness; a lit symbol is brighter than a
  resting one; the canvas maps normalised coordinates onto the cell and sizes
  from the shorter axis so circles do not become ellipses.
- **Audio (`audio.rs`):** the WAV header is well-formed (PCM, mono, correct rate
  and bit depth); the declared RIFF and data sizes match the real payload; every
  effect renders audibly (peak > 1000); **nothing clips** — if the mix starts
  hitting the limiter this test fails rather than shipping a crunchy sound;
  synthesis is deterministic; envelopes open and decay to silence; pitch glides
  are geometric, so the midpoint is the geometric mean.
- **Jackpots (`state/jackpot.rs`, `state/tests.rs`):** the shipped config
  validates (shares total 1000‰, tiers ordered, odds positive); a fresh pot shows
  its seed; every stake feeds every pot and **a minimum stake still moves the
  smallest tier** (the reason for milli-credits); winning a tier resets only that
  tier; the trigger is bet-fair; **rolling consumes the same randomness whether
  or not it hits**, so a near-miss cannot desync the RNG stream and break
  save/reload determinism; an older save with fewer tiers is topped up rather
  than rejected. End to end: a free spin neither feeds nor draws a pot, a win is
  credited and raises its own card while suppressing the big-win one, autospin
  stops, and the pots survive a reload.
- **Machines (`engine/sim.rs`, `state/tests.rs`):** **every** machine in the
  catalog loads, validates, and lands in the RTP band — at 20k spins in CI and
  over a million per machine in the ignored long run. The machines must differ
  in hit frequency (a reskin fails), must not share a save slot (sharing one
  would silently overwrite a balance and hoard), must not share a symbol set,
  and an unknown machine id falls back to the first rather than failing.
- **The Dragon's Wrath (`state/holdspin.rs`, `state/tests/holdspin.rs`):** the
  triggering eggs open already locked; every locked coin holds a value from the
  table; **a coin restores the full respin allowance** and a dry board runs out;
  a full board pays its bonus on top; respinning a finished round is ignored;
  auto-play terminates even on a hand-edited config that never runs dry; the same
  seed replays the same round; a bigger stake pays proportionally more; the coin
  table's weighted mean matches what rolling it produces; and a 20,000-round run
  holds the feature to its designed 20–80× band. End to end: a clutch of eggs
  opens a round covering the whole reel window, an open round holds the game and
  refuses a spin without taking a stake, the round advances on its own beat and
  credits exactly once, waking the dragon stops an autospin run, `spin()` resolves
  its own round so the sim never stalls, and a round is deliberately not saved
  while its counter is.
- **The Vault Pick (`state/bonus.rs`, `state/tests/bonus.rs`):** the board holds
  exactly the configured blanks; **nothing is visible before it is picked** (the
  renderer only ever sees `revealed_cell`); a round ends on the last blank and
  not before; re-picking a chest, or picking after the round is over, costs
  nothing; auto-play always terminates; the same seed deals the same board; a
  bigger pot pays proportionally more; a hand-edited config with more blanks than
  cells still deals a playable board. End to end: filling the hoard deals a board
  instead of paying, an open board holds the game and refuses a spin without
  taking a stake, hand-picking and auto-play reach the same total, `spin()`
  resolves its own board so the sim never stalls, and **boards pay the permille
  the closed form predicts** — the assertion that the swap did not move the money.
- **Achievements (`state/achievements.rs`):** the shipped definitions validate;
  a zero threshold is rejected (it would unlock before the player did anything)
  and so are duplicate ids; nothing is unlocked before playing; the first spin
  earns the first achievement and **only ever earns it once** (otherwise the
  toast repeats every spin); free spins count separately from paid ones; the
  balance condition is a high-water mark; progress accumulates across machines;
  a machine counts once however often it is played; the save shape round-trips
  and a save predating an achievement still loads.
- **Preferences (`state/preferences.rs`, `state/tests.rs`):** defaults are
  playable; both cycles reach every option and wrap; volume steps in tenths,
  clamps, and **can reach true silence**; the motion toggles are independent; a
  choice that no longer exists in the JSON falls back to the configured default
  rather than stranding; a partial file fills in the rest; a raw
  `Preferences::default()` is *not* the resolved default (the regression that
  made the panel and the Auto button both read 10 instead of 25). End to end:
  Turbo lands on the same grid and balance as Normal in fewer frames, every
  speed still reports all five reel stops in order, and a save round-trip does
  not carry preferences.
- **Spin animation (`state/spin.rs`):** a reel lands exactly on its target stop
  from a fixed *and* a deliberately ragged frame sequence; a reel whose stop is
  unchanged still spins a full revolution; reels announce their stop exactly
  once each, in order; the payout counter starts at 0, ends on target, and
  reports finishing exactly once.
- **Save round-trip:** save → load → identical session; legacy migration test (reuse
  template's pattern).
- **Visual:** `.\scripts\capture_ui.ps1 -Scenes idle,spin,win,freespins,paytable,feature_card,hatch,autospin`
  writes one PNG per scene into `docs/verification/`. `Game::begin_capture_scene`
  reseeds from a fixed seed and fast-forwards headlessly, so each capture is
  reproducible run to run. The card scenes spin until the *last* spin raised the
  card being photographed, clearing earlier ones as they go.
- **CI:** the shared `rust-ci.yml` (fmt + clippy -D warnings + test + wasm build) is
  the gate; run `cargo clippy` + `cargo test` for tight iteration, full `publish.ps1`
  for end-to-end validation.

---

## 12. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| RTP drifts when editing data | Monte-Carlo test in CI catches it; RTP lives only in the data JSON (strips, paytable, paylines, feature config — sim covers features too, §4). |
| `evaluate.rs` grows > 800 lines | Split payline eval / scatter / feature detection into siblings early. |
| Float rounding in payouts | Keep credits as **integers** (`i64`); multipliers applied as integer math. |
| Non-determinism sneaks in | Single state-owned `SeededRng`; forbid `Math::random`/wall-clock in gameplay (matches repo rule). |
| Reel animation coupling to logic | Outcome is computed once, at commit; the animation only *reveals* a decided grid and draws no randomness (§8.2). A test spins the animated and headless paths from one seed and asserts identical results. |
| A second Spin press mid-spin taking a second stake | `begin_spin` refuses with `SpinBlocked::Busy` before touching the balance; covered by a test that asserts the balance is untouched. |
| A feature firing unseen underneath its own auto-chain | A showing celebration card holds the reels, the payout and the auto-chain (§8.2.1); tested. Autospin also stops on every notable outcome. |
| Autospin quietly draining the balance | It stops on features, hatches, big wins and an empty balance, each with its own message; the bet ladder is locked for the run. |
| Files growing past 800 lines | Five splits so far: `state.rs` into siblings, `ui.rs`'s paytable into `ui/paytable.rs`, `state/tests.rs` (793) into `tests/{jackpots,preferences,machines}.rs`, `game.rs` (779) into `game/capture_scenes.rs`, and `state.rs` (812) into `state/features.rs`. **`state.rs` (742) is still the one to watch.** |
| A presentation bug hiding behind uniform test data | Reels 2 and 4 landed twenty symbols from their stop for five iterations because every landing test used an even reel index (§5.11). Tests over an indexed family must sweep the whole family, not a representative member. |
| A feature the second machine can never see | The Dragon's Wrath fired 23 times per million spins on Frost Wyrm under a shared config, because the trigger reads strips that differ per cabinet (§5.12). Anything triggered off the reels must be per-machine data and must be measured on **every** machine, not just the one that boots. |
| A new machine shipping at the wrong RTP | The sim iterates `MACHINES`; a cabinet cannot be added without being measured (§5.8). |
| Two machines sharing a save slot | Slots are `<machine>_<slot>`; a test asserts they are distinct. |
| Jackpots exploitable by bet-switching | Odds are per credit wagered, so the trigger is bet-fair by construction (§5.6) and tested. The bet-ladder sim test excludes jackpots deliberately — they are too high-variance to compare over 20k spins — and their return is checked against its closed form instead. |
| A new layer silently moving RTP | Adding jackpots took RTP from 0.961 to 1.000; the Monte-Carlo gate caught it and the paytable JSON absorbed it. Any future layer must be added to the sim in the same change. The Vault Pick (§5.10) shows the other approach: normalise the feature to the payout it replaces and the RTP does not move at all. |
| A bonus round that cannot end | `blanks` must be non-zero and leave at least one prize, checked in `GameData::validate`; `BonusRound::new` additionally clamps a hand-edited board, and a test plays out a deliberately malformed config. |
| Gambling optics | Explicit play-money framing in `game_page.json`; no purchases, no real value. Progressives grow only from play-money stakes and reset to a fixed seed. |

---

## 13. Out of Scope (v1)

Real currency / IAP, networked leaderboards, mobile-touch gestures beyond what
the shared web shell already provides.

Built after v1 shipped: ~~progressive jackpots~~ (§5.6), ~~sound settings
persistence~~ (§5.7) and ~~multiple slot machines/themes~~ (§5.8).

---

## 14. Definition of Done (v1)

Spin/win/lose loop, 20 paylines, wild + scatter, Free Spins, Hoard/Hatch meta,
data-driven balance with a passing RTP test, save/load with migration, juiced
feedback (anim + FX + SFX), symbol art, `catalog_thumbnail.png`, clean
`cargo fmt`/`clippy`/`test`, and a successful `.\publish.ps1` verified at
`http://127.0.0.1/games/dragons_hoard/`.

**All of it is met** — see §15. (The published URL sits under `/games/`, not at
the web root as this document originally guessed.)

---

## 15. Current State — v1 shipped, plus seven post-v1 systems

**All five phases are done, every item in §14 is met**, and seven systems have
been built on top since: progressive jackpots (§5.6), settings (§5.7), multiple
machines (§5.8), achievements (§5.9), the Vault Pick (§5.10), the reel-feel pass
(§5.11) and the Dragon's Wrath (§5.12). The game is
published and serving at `http://127.0.0.1/games/dragons_hoard/`, with a Project
Roost deployment recorded and a catalog entry created.

189 tests pass; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
and the `wasm32-unknown-unknown` release build are clean. Every `.rs` file is
under the 800-line limit; `state.rs` reached 812 adding the Dragon's Wrath and
its second-screen feature tail was split into `state/features.rs`, leaving it the
largest at 742.

Measured RTP over 1,000,000 spins: Dragon's Hoard **0.9612** at **0.411** hit
frequency, Frost Wyrm **0.9450** at **0.258**. Both paytables were scaled ~2–3%
down to make room for the Dragon's Wrath (§5.12).

Captures in `docs/verification/`: `ui_idle`, `ui_spin`, `ui_win`, `ui_freespins`,
`ui_paytable`, `ui_settings`, `ui_machines`, `ui_frost`, `ui_achievements`,
`ui_bonus`, `ui_feature_card`, `ui_hatch`, `ui_jackpot`, `ui_autospin`,
`ui_anticipation`, `ui_wrath`. The catalog card image at the project root is
produced by the same harness. `ui_spin` is captured at 20 frames rather than 150 — at the default
the spin has already finished, so the blur it is meant to show is not there.

### Verified, and not

Tested and seen: the maths, the spin lifecycle, the features, and every screen
above — the art was reviewed from real captures and revised twice off them (the
gems read as kites, the coin stack as a blob, the egg as a teardrop).

Reviewed from captures again this iteration: the Dragon's Wrath banner rendered
its respin counter as tofu (a rotation arrow — macroquad's default font has no
arrows, the same trap §7.1 records for emoji), and the paytable's fourth rules
paragraph spilled out through the bottom of its panel. Before that, the first
motion-blur attempt bleached the reels to near-white. None of the three is
something a test would have caught.

**Not verified: how the sound actually sounds.** The synthesis is covered by unit
tests — well-formed header, correct rate and bit depth, audible peak, no clipping,
deterministic output — but those prove the *bytes* are right, not that the effects
are pleasant or well-balanced against each other. Nobody has listened to them, and
audio playback under WASM in a browser is untested. Treat the mix in
`voices_for` as a first draft.

### Remaining work

A jackpot **has** now been seen: the `jackpot` capture scene photographs a real
Mini win, with its ladder plate reset to seed while the other three keep
accruing. That closes the gap this section previously listed.

- **Listen to the SFX** and rebalance `voices_for`; confirm audio works in the
  browser build, not just natively. Mitigated but not fixed by §5.7: the volume
  can now be turned down or off, which is a workaround for an unverified mix,
  not a substitute for hearing it.
- Work is committed per iteration following `rust_management/docs/COMMIT_STYLE.md`
  — a diegetic subject, a plain parenthetical tag, and a prose body.
- `audio.rs` is worth promoting into `macroquad-toolkit` (§7.1). So, now, is the
  blur/bounce/anticipation work in `state/spin.rs` — none of it is specific to a
  slot machine beyond the anticipation trigger, and any game with a spinning or
  scrolling strip would want it.
- **The `CoinLock` effect has never been heard either**, and it is the one that
  most needs to be: in a full round it fires up to fifteen times inside a second,
  so if it has any tail at all it will smear into a wash. It was written short on
  that theory alone.
