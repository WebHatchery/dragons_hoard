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

The catalog ships five cabinets. For the first two the whole difference is JSON;
the third (§5.14) changes the evaluator and the fourth (§5.15) changes what a
spin *is*, which is what makes them different games rather than different
tunings.

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

### 5.13 The Feature Buy (post-v1)

Ten features in, the player's only decision was still how much to bet. The buy
menu is the first system that asks them to choose something: pay a fixed price
and skip straight to a feature, or keep spinning for it.

**The price is derived from the feature, not chosen for it.** A bought feature is
only honest if it costs what it is worth:

```
price = feature_expected_value / target_rtp
```

Price it below that and never touching the reels beats spinning; price it above
and the menu is a trap. The buy must be **the same game, only faster** — not a
better or worse one.

**The price is JSON, and a test is what keeps it true.** Measuring a feature's EV
means playing thousands of them, which is not something to do at load time, so
the price is a plain `price_multiple` in `featurebuy.json`. What stops it drifting
is `feature_buy_prices_are_exact`: it buys every tier of every machine 200,000
times through the real session and asserts the measured return lands on that
cabinet's RTP. Edit `freespins.json` and the test fails — with the correct new
price in the failure message, because a test that has already done the
measurement may as well hand over the answer:

```
dragon/freespins  price  100x  rtp 0.5105  (target 0.9500)  fair price 53.7x
```

That is how the shipped prices were set. Every tier was priced at a placeholder
100×, the ignored test was run once, and the six numbers it printed became the
six prices. Measured after: **0.9454 / 0.9484 / 0.9627** on Dragon's Hoard and
**0.9446 / 0.9496 / 0.9486** on Frost Wyrm — every tier within 1.3 points of its
machine.

| Tier | Dragon's Hoard | Frost Wyrm |
|---|---|---|
| Free Spins | 54× total bet | 56× |
| Super Free Spins | 108× | 127× |
| The Dragon's Wrath | 31× | 29× |

The spread is the interesting part. Doubling the free spins from 10 to 20 **more
than doubles** the price (54 → 108, and 56 → 127 on the high-volatility cabinet),
because retriggers and the hoard compound over a longer run. Nobody worked that
out; the sim did.

**The measurement runs the whole session, deliberately.** `simulate_buys` buys,
then plays out everything that follows — retriggers, expanding wilds, eggs banked
into the hoard, a Vault Pick the free spins happened to fill, a Dragon's Wrath a
bought free spin woke. A tier's price therefore includes its downstream, which is
the only definition of "what the feature is worth" that a player would recognise.

**A buy is a stake, but it is not a spin.** The price is wagered: it leaves the
balance, counts toward turnover, and feeds the progressives (§5.6). It does *not*
roll for a jackpot, and the free spins it grants still cost nothing and still
cannot draw one. Getting that backwards either way is a real exploit — a buy that
rolled would hand the player a second draw per credit that the bet-fairness
argument assumes does not exist, and a buy that did not contribute would let
someone play the whole game without ever feeding the pots they can win.

Prices are multiples of **total bet**, so buying at a high stake costs
proportionally more and a player cannot buy cheap to collect expensive — the same
argument as the jackpot odds (§5.6) and the hoard's per-egg banking (§3).

**Refusals name the actual reason.** A running feature reports itself as one
rather than as generic busyness; the guard order was flipped for this, since both
answers are true while a feature's entry card is up and only one of them is
useful. `can_buy` drives the greying-out in the menu, and a test sweeps every
tier at four balances asserting it agrees with what buying actually does —
otherwise a live-looking button could be refused, or a legitimate buy blocked.

Feature buys are restricted or banned in several real jurisdictions for
accelerating loss rates. That is not a concern for play money with no purchases
(§1), but it is the reason the menu says what it is priced from rather than only
what it costs.

### 5.14 Ways to Win, and a third cabinet (post-v1)

Two machines that differ only in their numbers are two tunings of one game. This
is a different game: **Emberfall** pays 243 ways instead of 20 lines, and it is
the first thing built here that changes the evaluator rather than the data it
reads.

**What a way is.** A payline asks whether *one path* through the grid reads as
five chests. A ways machine asks whether **any** path does: a symbol pays if it
appears somewhere on each of reels 1..n, multiplied by how many paths there are —
the product of its per-reel counts. Two on reel 1, one on reel 2, three on reel 3
is `2 × 1 × 3 = 6` ways, all paid. On a 5×3 grid that is `3⁵ = 243`, always
active. There is no line to be off.

**Which evaluator runs is a data key, not a caller's choice.** `game_config.json`
gains `evaluation: "lines" | "ways"`, defaulting to lines so the two existing
cabinets needed no edit — a key that did not exist yesterday must not invalidate
data that was already correct. Everything above `evaluate()` reads the same
`SpinOutcome` either way; a machine *is* its evaluation model.

**Wins now carry their own cells.** `LineWin.line` was an index into the payline
table, which a ways win has nothing to look up. `Win` carries `cells` instead, so
the highlight, the floating `+N` and the summary line all work without knowing
which model produced them. That removed a lookup rather than adding one.

**Every symbol pays, except an all-wild run.** This is the rule that separates
ways from lines and it was a decision, not an inheritance. On a payline the run
is one contest and `best_line_result` picks the best single reading. In a ways
game symbols genuinely pay *alongside* each other — a gem run and a chest run are
two different sets of paths and both are real. The one thing that must not happen
is the same cells paying twice under two names, and there is exactly one way for
that to arise: a run of pure wilds reads as every symbol at once. So a symbol is
suppressed when no genuine copy of it appears in its run; the wild pays for those
cells itself, once. `W W W C C` therefore pays both a three-wild run *and* a
five-chest run, because reels 4 and 5 hold real chests.

**The bet model had to generalise.** `total_bet` was `line_bet × paylines.len()`.
A ways machine has no lines to count and 243 units of line bet would be an absurd
stake, so it declares `bet_units` outright — Emberfall buys all 243 ways for 25
units. Validation makes the two mutually exclusive: a ways cabinet that declared
paylines would evaluate by ways while charging for lines that do nothing.

**Ways strips are shaped differently, and the sim had to teach me how.** "Appears
somewhere on this reel" is a far easier bar than "appears on this row", so the
first pass — premiums thinned and pays cut hard — measured **0.4147**. Scaled
2.65× it went to 0.9702, and a final 1.75% trim landed **0.9596**. The resulting
machine feels nothing like the other two:

| | Dragon's Hoard | Frost Wyrm | Emberfall |
|---|---|---|---|
| Model | 20 lines | 20 lines | 243 ways |
| Hit frequency | 0.411 | 0.258 | **0.622** |
| Bet units | 20 | 20 | 25 |
| Free spins | 10/15/20 at ×2 | 8/12/18 at ×3 | 10/15/20 at ×3 |
| Base game share | 0.654 | 0.723 | 0.763 |
| Measured RTP | 0.9612 | 0.9450 | 0.9596 |

Nearly two spins in three return something. That is the ways feel — constant
small wins rather than long droughts — and it comes out of the model, not out of
a volatility dial.

**The Feature Buy caught its own mispricing.** Emberfall inherited Dragon's Hoard
prices, and `feature_buy_prices_are_exact` (§5.13) failed at once with the right
answers printed: 46× and 92× rather than 54× and 108×, because tripled free spins
over 243 ways are worth less per unit of a larger stake. That is the system built
last iteration doing exactly the job it was built for, on data it had never seen.

**Testing a ways win in isolation is harder than a line.** §11 records that no
filler symbol isolates a single payline; on ways it is worse, because any filler
forms its own genuine run — that *is* the mechanic. The all-wild test walls its
run off with a column of scatters, the only symbol that cannot be substituted for
and cannot be run into.

### 5.15 Cascading reels, and a fourth cabinet (post-v1)

§5.14 changed what counts as a win. This changes what a *spin* is. On
**Avalanche** a win is not the end: the winning symbols clear, everything above
falls into the gap, fresh symbols drop in, and the new grid is evaluated again.
While each grid pays the chain continues, and a multiplier ladder climbs with
it — 1, 2, 3, 5, 8.

**The whole chain is decided at commit, and that was the hard part.** The
obvious implementation draws fresh symbols from the RNG whenever a grid clears.
That would break §8.2's invariant outright: the reveal would consume randomness,
and the animated and headless paths would diverge on the second grid. So refills
are **not rolled**. Each reel keeps reading *up* its own strip from where it
stopped — the symbols that drop in are the ones that were already above the
window, exactly as a physical cascade would show. The entire chain is therefore
a function of the stop indices alone, and a test spins both paths from one seed
and asserts they come to rest on the same grid with the same balance.

**Features are read from the landing grid; only payouts cascade.** A scatter
arriving on a refill would make the free-spin award depend on how long a chain
ran, and the Dragon's Wrath (§5.12) would open from symbols the reels never
landed. Keeping features on the first grid leaves every other system in the game
reading exactly what it read before.

**A new phase.** `SpinPhase` gained `Cascading(CascadeReveal)`, a cursor into the
decided chain that advances on a beat. It holds no symbols of its own — it cannot
change what was decided. `display_grid()` (added a section earlier for an
unrelated bug) already knew how to show a grid other than the settled one, so
mid-cascade it returns whichever grid the chain has reached.

The reveal exposed a presentation bug of its own on the first run: the chain
settled the *instant* it reached its last grid, so that grid never had a beat as
part of the chain. `CascadeReveal` now tracks a separate `done` flag, set when
the final grid's beat elapses rather than when it is reached.

**A cascading cabinet cannot reuse a ways cabinet's maths.** Emberfall pays on
62% of grids, and on a cascading machine every paying grid starts another one.
The first measurement was **RTP 17.18**. Isolating the ladder by flattening it to
×1 gave **1.05** — so the chain itself only adds about 10%, and the multipliers
were contributing a factor of three. That is the number that told me what to fix:
not the chain, the ladder, and the paytable that has to make room for it.

Avalanche therefore pays the low and mid symbols only from **four**, not three.
That is the single biggest lever on how often a grid pays at all, and it keeps a
chain to a handful of steps. Strips are 44 long rather than 50, so the catalog
cannot mistake it for a reskin. The base paytable is held at 8× its intended
magnitude in the generator, because the scale factor the sim asked for (0.0322)
would otherwise have rounded the whole table onto the same two integers.

| | Dragon's Hoard | Frost Wyrm | Emberfall | Avalanche |
|---|---|---|---|---|
| Model | 20 lines | 20 lines | 243 ways | 243 ways, cascading |
| Lows pay from | 4 | 4 | 3 | **4** |
| Strip length | 40 | 50 | 50/51 | 44 |
| Hit frequency | 0.411 | 0.258 | 0.622 | 0.475 |
| Measured RTP | 0.9612 | 0.9450 | 0.9596 | 0.9429 |
| Biggest win in 1M | 44,904 | 66,211 | 51,629 | **84,645** |

The largest single win in the catalog is Avalanche's, which is what a multiplier
ladder is for.

**The Feature Buy caught it again.** Avalanche inherited Emberfall's prices and
`feature_buy_prices_are_exact` (§5.13) failed with the answers printed: 30× and
59× rather than 46× and 92×, because a bought free spin on a cascading machine is
worth much more per spin and the price has to come *down* relative to a stake
that buys a chain.

`state.rs` reached 843 lines and the spin lifecycle — commit, reveal, settle —
moved to `state/lifecycle.rs`.

### 5.16 The Dragon's Gamble (post-v1)

Twelve systems in, the player still had exactly one decision: how much to bet.
Everything else either happened to them (free spins, the Dragon's Wrath), revealed
something already decided (the Vault Pick), or was a purchase at a fixed price
(the Feature Buy). **This is the first system that lets them get something wrong.**

After a paying spin the win can be staked on the colour of a dragon scale —
**ember** or **ash**. Right doubles it, wrong takes it. A win can be pushed up a
five-rung ladder, with a ceiling in total bets, and `take` banks whatever is
standing. Half-gamble risks half and banks the rest.

**It is exactly fair, and that is the design.** A double-or-nothing at even odds
has expected value `0.5 × 2x + 0.5 × 0 = x`, so **the gamble cannot move RTP at
all** — only variance. That completes a set of four distinct relationships this
game now has to its own maths:

| System | Relationship to RTP | What it cost |
|---|---|---|
| Progressive jackpots (§5.6) | **adds** EV | a full paytable retune |
| The Vault Pick (§5.10) | **normalised** to what it replaced | nothing |
| The Feature Buy (§5.13) | **priced at** EV | measuring every tier |
| The Dragon's Gamble | **neutral** by construction | nothing |

Real cabinets usually shave the gamble — a 47.5% win chance dressed as a coin
flip. There is no reason to here: it is play money (§1), the house edge already
lives in the paytable, and "provably fair" is a better thing to be able to say.
Two tests hold it: `the_scale_is_fair` measures the coin over 200,000 flips, and
`the_gamble_returns_what_it_risks` pushes every round to the end of the ladder —
the worst case for the claim — and asserts the ratio.

**The strongest test compares two whole simulations.** `gambling_cannot_move_rtp`
runs 120,000 spins twice from one seed, once gambling nothing and once gambling
every win to the ladder's end, and asserts the two RTPs match: **0.9254 against
0.9187**. It is the only test in the suite with no target to aim at, because
"unchanged" *is* the assertion.

**What a fair gamble does still cost the player is time to zero.** Neutral EV is
not neutral risk, and a balance runs out faster when every win is doubled or
lost. That is why the ladder is capped, why the ceiling exists, and why Take is
the calmest-looking button on the panel.

**Base game only.** During free spins the chain spins itself and an autospin run
does the same, so either would have the reels turning underneath a decision the
player is still making. `can_gamble` requires a settled session, a win, no
feature and no run.

**Staking takes the win back out of the balance.** It was credited when the spin
settled, so gambling it has to remove it again — otherwise a lost gamble would
cost nothing and a won one would pay the original twice. A busted round closes
itself rather than leaving a panel asking the player to press Take on zero, and
raises the one card in the game that is not good news.

The odd credit on a half-gamble goes to the player. Rounding against them on
their own money is a bad look for one credit.

### 5.17 Machine profiles, measured live (post-v1)

Four cabinets, hit frequencies from 0.258 to 0.622, two evaluation models, one
that cascades — and the player chose between them on one line of blurb. §5.13
already recorded the same gap on the buy menu: "the price tells a player what a
feature costs but nothing about what to expect for it."

**The figures are measured, not written down.** The obvious fix is to bake them
into JSON. The problem is that they are *derived*: every strip edit or retune
makes them wrong, silently, and nothing would notice. The Feature Buy price gets
away with being data because a price is a **choice** a test can then check; a hit
frequency is not a choice, it is an **observation**. So the profiler runs the
real headless spin path against the cabinet in front of the player. There is
nothing to drift from, because there is nothing written down.

That required making `GameSession::spin` and `engine::sim` non-test for the first
time. The headless path is now a runtime capability rather than a testing
convenience, which is the honest description of what it always was.

**It never touches the player's session.** The profiler owns a scratch session
with its own seed. If it drew from the live RNG, opening the machine picker would
change the spins that came after it — a save-and-reload divergence a player could
see and nobody could explain. A test spins two identical sessions with a profiler
running between them and asserts they stay in lockstep.

It runs 400 rounds per frame while the picker is open, so a full 20,000-round
profile lands in about a second with no frame doing enough work to be felt.

**What a 20,000-round sample can and cannot support.** The first version printed
a return percentage. It was wrong: Dragon's Hoard profiled at **87.4%** against a
true 92.7%, and Frost Wyrm at **95.5%** against 91.2%. Excluding progressives —
the trick §5.6 already uses on the bet-ladder test, for exactly this reason —
narrowed it but did not fix it. **A slot's RTP needs millions of rounds to settle,
and no sample a player will wait for can measure it.** So the number is not
shown. Quoting it would have been inventing precision, and a figure that is wrong
by five points is worse than no figure.

What the sample *does* support is printed instead: how often the machine pays,
how unevenly, the largest round seen, and a band bar. The bar is what actually
communicates the difference — two cabinets can both return 95% and feel nothing
alike, and the shape of that bar is why.

| | Dragon's Hoard | Frost Wyrm | Emberfall | Avalanche |
|---|---|---|---|---|
| Pays on | 40% of spins | 26% | 62% | 48% |
| Volatility | Medium | High | Medium | Low |
| Best seen | 284× | 795× | 194× | 81× |

**A volatility index**, the standard deviation of return per round, is the number
that separates the cabinets in a way RTP cannot. It is reported as a word rather
than a figure — the number alone means nothing to anyone who has not seen another
one to compare it against.

`SimReport` split as a result: the *statistics of a run* (`RoundStats` — rounds,
return sums, bands) compile into the game, and the batch drivers and their
per-feature breakdown stay test-only.

**A live RNG bug fell out of this.** The profiler's seed was originally
`0x9E3779B97F4A7C15`, the golden-ratio constant — which is *exactly* what
`SeededRng::new` xors its seed with. The state became zero, and an xorshift at
zero is a fixed point: every draw returns 0, forever, with nothing about it
looking broken. Every cabinet profiled to the same grid on every round. Fixed in
`macroquad-toolkit` rather than worked around here, since it would silently kill
the generator for any game that picked that seed — and a "nice" constant is
exactly the sort of number someone reaches for. The guard fires only in that one
case, and a test asserts every ordinary seed's stream is byte-identical to before.

### 5.18 The Ledger (post-v1)

§5.17 measures what a cabinet *does*. This measures what the player has *seen* —
with the same `RoundStats`, so the two are directly comparable — and puts them
one above the other.

**The comparison is the feature, and it is meant to be uncomfortable.** A few
hundred rounds will not look anything like the machine's own figures, and a
player on a cold run will have a chart that looks like evidence. So the panel
says what the gap actually is:

> Over 749 rounds your return could plausibly sit 17 points either side of the
> machine's, purely by chance. The gap between the two bars above is variance,
> not the cabinet changing its mind. It narrows with the square root of how much
> you play, which is slowly.

That margin is the machine's own measured volatility over `sqrt(n)` — the
standard error of the mean. It is the honest answer to "am I being cheated",
and the game is in an unusually good position to give it, because it has just
measured the machine itself.

**A round is a paid spin and everything it led to.** Free spins cost nothing, so
they belong to the round that bought them, as do a Vault Pick or a Dragon's Wrath
that round opened. This is exactly the profiler's definition and has to be, or
the two bars would answer different questions. The session tracks an `OpenRound`
that a new stake closes — the moment a stake is taken is the only unambiguous
boundary between one round and the next.

**Two things are deliberately left out.** Progressives, for the reason §5.17
established. And **the gamble (§5.16)**, because it is the player's decision
rather than the machine's behaviour and the profile has none in it.

The ledger is per cabinet — averaging four different games would describe none of
them — and persists under its own key alongside preferences and achievements, so
it is a record of what the player has seen rather than of one bankroll. The
session cannot write it itself; it hands closed rounds to the orchestrator, which
owns the book.

**The capture harness caught its own reproducibility bug.** The ledger persists,
so the first two capture runs of the panel showed 375 rounds and then 749 — each
run adding to the last. The scene now starts from an empty ledger. Nothing else
would have noticed; the harness is only reproducible because every other scene
happens to be stateless.

### 5.19 Synthesis promoted, and the sounds finally looked at (post-v1)

Two long-standing entries in §15 closed together, because closing the first made
the second possible.

**`audio.rs` is now `macroquad_toolkit::synth`.** §7.1 flagged it as a candidate
for promotion the day it was written: the toolkit's `SoundManager` can only load
from a file or an asset pack, so any game in the workspace wanting a blip has to
source a `.wav`. The split was already clean — `Wave`, `Voice`, the envelope and
glide maths and the WAV container are generic; `Sfx` and `voices_for` are this
game's sound design — so the move was lifting the first half out. `audio.rs` went
from 442 lines to 289, and `render_waveform` was added on the way: the summed
signal before quantising, for a caller that wants to *look* at an effect rather
than play it.

**The move is proved to have changed nothing.** Lifting a renderer into a shared
crate is exactly the kind of refactor that can quietly alter every sound in a
game, and no existing test would have noticed — they all asked whether the bytes
were *well-formed*, never whether they were the *same* bytes. So the length and
checksum of all eight effects were recorded first, and they came through
identical.

**Then the sounds were looked at, for the first time.** They still cannot be
heard here. But a waveform can be *seen*, and a surprising amount of what §15 had
been calling unverifiable turns out to be visible: a tail that will smear when an
effect repeats, an attack so slow the sound arrives late, one effect twice as
loud as the rest. `ui/waveform.rs` plots all eight on a shared time and amplitude
axis, so they can be compared rather than each filling its own box.

**It found a real problem in its first frame.** `WinSmall` peaked at **0.14**
against `ReelStop`'s **0.28** — a win was half the volume of a reel merely
stopping — and `WinBig` came in at 0.21, also under a reel stop. Nothing in the
suite could see that; the peak test only ever asked for "above zero and below the
limiter". Three effects were raised, the baseline test failed exactly as designed,
and the new figures were adopted deliberately:

| | before | after |
|---|---|---|
| WinSmall | 0.14 | **0.26** |
| WinBig | 0.21 | **0.32** |
| CoinLock | 0.16 | **0.21** |

The hierarchy now reads Click 0.11 → Scatter 0.20 → CoinLock 0.21 → WinSmall 0.26
→ ReelStop 0.28 → WinBig 0.32 → Hatch 0.34, which is the order those events
deserve.

The first version of the panel drew every effect as a thin line down the middle,
because it plotted against a ±1.0 axis and nothing peaks above a third. The axis
is ±0.4 now. That is the sort of thing only looking at it can tell you, which is
rather the point.

**`CoinLock` was the specific worry** — it fires up to fifteen times inside a
second during a Dragon's Wrath round (§5.12), so any tail smears into a wash. It
runs 0.16s with a sharp attack and most of that at near-silence, and a test now
pins it under 0.2s.

### 5.20 Shifting reels, and a fifth cabinet (post-v1)

§5.14 changed what counts as a win and §5.15 changed what a spin is. This
changes the **shape of the board**: on **Wyrmspire** every reel rolls its own
height, two to six rows, on every spin — so the number of ways changes with it,
anywhere from 32 to 7,776.

**The grid had to learn that reels differ.** `Grid` was `reels × rows` with the
flat index `reel * rows + row` written out at eight call sites. It now holds a
height per reel and a cached prefix sum, and `Grid::index` is the only place that
arithmetic lives. That refactor stands on its own — duplicated index maths across
the evaluator, the cascades, the win highlight and the hold-and-spin was a latent
bug however tall the reels are — and all 278 tests passed on it before a single
height varied.

**The shape is decided at commit, like everything else.** `pick_heights` runs
straight after `pick_stops` and the two travel together, so the animation reveals
a board whose *shape* was settled before a reel moved, not just its symbols. A
reel spins at the height it is going to land on, because spinning at the
configured maximum and settling to something shorter makes every reel jump as it
stops.

**Heights are uniform across the range.** Weighting them toward the tall end
would be a way to move RTP without touching the paytable, and RTP lives in the
data (§4).

**Shifting reels require ways evaluation, and validation says so.** A payline
names a row on every reel; on a cabinet where a reel might be two rows tall, half
of them would point at cells that are not there.

**The ways figure is read off the board, not the config.** `ways_count()` returns
`None` on a shifting cabinet and the panel reads `grid.ways()` instead — "4320
ways this spin (up to 7776)". Quoting only the ceiling would be advertising a
grid the player is almost never looking at.

**Tuning it took four passes.** The first measured **3.33** — six reels of up to
six rows is a great many ways, and the ways cabinet's paytable is built for 243
of them. Starting from Avalanche's table (lows paying from four, which is the
lever on hit frequency) and scaling landed **0.9552**. Two tests then failed for
real reasons: Wyrmspire shared a strip length and symbol set with Emberfall, so
it got its own 46-symbol strips; and the fixed `ways_count() == 243` assertion
needed to learn that a shifting cabinet has a ceiling rather than a figure.

| | Emberfall | Avalanche | Wyrmspire |
|---|---|---|---|
| Reels | 5 × 3 fixed | 5 × 3 fixed, cascading | 5 × **2–6** |
| Ways | 243 | 243 | **32 – 7,776** |
| Bet units | 25 | 25 | 30 |
| Hit frequency | 0.622 | 0.475 | **0.673** |
| Measured RTP | 0.9596 | 0.9429 | 0.9552 |

The Feature Buy caught its inherited prices for the fifth time and printed the
right ones; the Wrath tier in particular went from 29× to 70×, because a
four-egg trigger on a board that can be thirty cells tall is a very different
proposition from the same trigger on fifteen.

### 5.21 Refining free spins (post-v1)

Sixteen systems in, the feature set was essentially complete for a modern slot
except for one thing: free spins were still N identical spins. **Frost Wyrm's now
burn the cheapest symbols off the reels as they run**, so the feature escalates
instead of repeating.

Each free spin removes the next symbol in a configured order from every strip.
By the last spin the reels hold only what pays well, and the banner names what
has gone — "burned FRC RIM" — because the escalation is otherwise invisible: the
reels simply feel luckier and the player has no way to know why.

**Strips are dynamic for the first time.** Everything before this treated
`data.reels` as fixed. `refined_reels(burned)` returns a filtered set, and stops
are drawn against *that* rather than the raw strips — a subtle but essential
point, since a shorter strip has a different stop range.

**The burn deepens before the spin it applies to**, so the feature's first spin
already runs refined. Deepening it afterwards would make the first free spin
indistinguishable from a base one and start the escalation a beat late.

**A retrigger does not reset it.** Rebuilding the strips would undo everything
the feature had burned, making extra spins a punishment.

**Validation refuses to burn the wild, the scatter or the hoard symbol**: no
scatter means no retrigger, no wild means no substitution, and the egg feeds the
meter. And a reel that would empty keeps what it has — validation cannot catch
that one, because whether it happens depends on how a designer laid out one
particular strip.

**Tuning took four passes and the answer was not the paytable.** Burning four of
Frost's eight payables took RTP to **6.92** — free spins alone at 6.04 of
turnover. Two burns brought it to 1.41. The instinct was to cut the paytable, but
a 36% cut would have gutted the base game to pay for the feature. Rebalancing the
*feature* instead — spins 8/12/18 → 5/8/12 and the multiplier ×3 → ×2, since the
refining now provides the escalation a flat multiplier used to — got it to 1.08,
and a 14% paytable trim landed **0.9491**.

**The Feature Buy repriced itself by a factor of three.** Frost's free-spin tier
went from 56× to **168×** and the super tier from 127× to **396×**, because the
feature really is three times more valuable than it was. Nobody worked that out;
the price test measured it and printed the answer.

`data.rs` reached 848 lines and its feature configs moved to `data/features.rs`.

### 5.22 Profiling the buy menu (post-v1)

§5.13 closed with an admission: "the price tells a player what a feature costs
but nothing about what to expect for it." §5.17 then built the machinery to fix
that and pointed it at the cabinets instead. This points it at the tiers.

**Each tier is measured the same way a machine is** — bought four thousand times
on a scratch session and played out to the end — and shows the same band bar. But
the headline is a statistic the machine profile has no use for and no real
cabinet displays:

> **71% of buys come back under the price.**

That is the number a purchase actually turns on. All three of Dragon's Hoard's
tiers are priced at exactly their expected value (§5.13), return the machine's
RTP, and *still* hand back less than they cost about seven times in ten. Nothing
is wrong: it is simply what "fair" means for a bet with a long tail. A few large
returns carry the average while the median sits well below the price.

A test pins it — `a_fairly_priced_buy_still_loses_most_of_the_time` asserts every
tier lands between 20% and 95%. If it ever came out near zero the headline would
be worthless and something would be wrong with either the pricing or the
measurement.

| Dragon's Hoard | price | under the price | best seen |
|---|---|---|---|
| Free Spins | 54× | **71%** | 15× |
| Super Free Spins | 108× | 69% | 11× |
| The Dragon's Wrath | 31× | 67% | 10× |

**Tiers profile on their own slot**, not queued behind the machine profiler: the
two are looked at on different screens, and sharing one would leave a panel blank
for no reason. Each tier gets its own seed, or three tiers on one cabinet would
be three views of the same run of luck.

The same rule as §5.17 applies and is tested: profiling must not consume a draw
the player's next spin was going to use.

### 5.23 Reel motion promoted to the toolkit (post-v1)

The last item on §15's list. §5.11 built motion blur, a landing bounce and
anticipation; §5.19 noted that none of it is specific to a slot machine and any
game with a spinning or scrolling strip would want it. It is now
`macroquad_toolkit::strip`.

`state/spin.rs` went from **615 lines to 298**. What moved is the mechanism —
`StripAnimation`, `StripSpinner`, the blur offsets. What stayed is the judgement:

- **The tuning**, as a `StripFeel` this cabinet returns. How long a reel should
  turn for is a decision about *this* game, and the toolkit ships defaults rather
  than opinions.
- **The anticipation trigger.** `anticipating_reels` asks whether the scatters
  still showing could complete a free-spins award, which is slot logic. The
  toolkit knows only that a strip can be *held* and turns for longer when it is.

**The invariant travelled with the code.** The module documents why travel must
be a whole number of revolutions, and names the bug: `2.0 + index * 0.5` shipped
for five iterations, landing strips 2 and 4 exactly half a strip from their stop.
`StripAnimation::travel_symbols` is public so a caller can assert it directly,
and the toolkit's own test sweeps six indices rather than one.

Two tests were added that this game never had. `no_time_scale_drops_a_stop`
sweeps four time scales asserting no strip is lost to a compressed duration — the
game tested one scale. And `blur_offsets_straddle_the_position_and_sum_to_nothing`
pins the smear symmetrical, which had only ever been inline arithmetic.

**Proof the move changed nothing:** all five cabinets measure exactly the RTP
they did before — 0.9612, 0.9491, 0.9596, 0.9552, 0.9429, unmoved to four
decimal places. The animation touches no outcome, and now there is a
million-spin run per machine saying so across a crate boundary.

The game keeps a test of its own for the part the toolkit cannot know:
`this_cabinets_feel_still_lands_every_reel_on_its_stop`, because a `base_time` or
`revolutions` edit here is exactly the sort of change that could break the
landing without the toolkit noticing.

### 5.24 Reading the symbols without colour (post-v1)

With the remaining-work list empty, this iteration went looking for a defect
rather than a feature — and found one that had been there since §7.1.

**Every machine drew three of its symbols as the same hexagonal gem, separated
only by hue.** That falls straight out of the art being tinted from a single
`color` key, which is a good decision (one field re-themes a cabinet) with a
blind spot. Roughly one man in twelve has some form of red-green colour
blindness, and to a deuteranope those three stones are one picture.

**The instrument came first.** `ui/legibility.rs` simulates protanopia,
deuteranopia and tritanopia — in *linear* RGB, because applying the matrices
straight to sRGB exaggerates the effect and a test that cries wolf gets turned
off — and measures how far apart two colours land. A test then asserts that any
two symbols **sharing a shape** stay above a threshold under every vision.
Symbols drawn differently are told apart by shape and need no colour gap at all.

It failed immediately, and not where expected. Dragon's Hoard's jade/sapphire/
ruby were far enough apart to pass. **Frost Wyrm's glacier and amethyst were
0.061 apart under deuteranopia** — the same picture in the same colour.

**The fix is not a colourblind mode.** A mode is something a player has to know
to look for, and it splits the art into a version that is tested and a version
that is not. Instead there are now three gem cuts — hexagonal, round brilliant,
and a stepped emerald cut — and every cabinet's three stones use one each. Shape
carries the difference; colour only reinforces it. That is better for everyone
and needs no setting.

**Then it was checked by looking.** `ui/vision.rs` draws the whole symbol set
four times, one row per vision, colours simulated and art untouched (a dichromat
sees the same shapes as everyone else — the shapes are the point). The
deuteranopia row is the proof: jade and ruby collapse to nearly the same olive,
exactly as the numbers said, and remain trivially distinguishable because one is
a hexagon and the other a rectangle.

Two tests keep the instrument honest as well as the art: the simulation must
leave greys untouched, and red and green must visibly collapse for a
deuteranope. A measurement device that quietly tinted everything would fail both
the art it judged and every future judgement.

All five cabinets measure exactly the RTP they did before. `art` is a display key
and nothing downstream of it reads the maths — but a data edit across five
machines is worth a million-spin run per machine to say so.

### 5.25 The art becomes testable (post-v1)

§5.24 ended with a claim and a screenshot: the three gem cuts are different
shapes, look at the picture. This iteration built something that could check
that, and the claim turned out to be **wrong**.

**The art now draws through a trait.** `Painter` has four primitives — triangle,
circle, ellipse, rectangle — and two implementations: `ScreenPainter`, which is
what ships, and `Buffer`, which rasterises into a plain pixel array with no
window, no GL context and no frame. `Canvas` is generic over it, so the same art
routines serve both. Nothing below `Canvas` knows which it is drawing into.

One primitive had been reaching past the canvas to macroquad directly — a
`draw_poly` for the coin's embossed face — which is why the art could not be
drawn headless at all. It is a fan of triangles now.

**What that buys is measurement.** The first run said:

```
dragon: 'copper' and 'jade' differ in only 2.6% of their pixels
dragon: 'copper' and 'sapphire' differ in only 4.1% of their pixels
```

A regular hexagon at that radius **is** a circle once the cell is 64 pixels tall,
and the "round brilliant" was a twelve-sided circle. §5.24's capture looked
convincing because the *facets* differ; the outlines did not. The hexagon is
narrow and pointed now, the round cut is a flat oval, and the step cut was
already a rectangle — three shapes rather than three shadings.

**The metric was wrong twice before it was right.** Counting differing pixels
across the whole cell is dominated by the empty background both symbols share.
Jaccard distance over the union fixed that and produced meaningful numbers
(18–30%) — which then showed that *silhouette alone is the wrong standard for
this art*. Every symbol here is a centred object filling most of its cell; a coin
and a chest overlap heavily in outline and always will, and what separates them
is the lid and the keyhole. The shipped test measures **monochrome difference**,
which sees outline and interior at once and is the strictest realistic case —
what a symbol has left after colour blindness, a dim screen and a cell a sixth of
the reel window tall.

The gem-cut test keeps the silhouette standard deliberately, because §5.24's
claim was about *shape* and colour must not be allowed to prop it up.

**64 pixels is not arbitrary.** It is the cell height on a six-row reel of the
shifting cabinet (§5.20) — the smallest this game ever draws a symbol, and the
question §5.24 left open.

### 5.26 The rasteriser promoted, and a golden image for the art (post-v1)

§5.25 built a CPU rasteriser so the symbol art could be measured. It is now
`macroquad_toolkit::paint`, because procedural art is cheap to ship and
impossible to test in **every** game that draws it — the art only exists once
there is a window, a context and a frame, so the only check available is a person
looking at a screenshot.

Unlike the audio (§5.19) and the reel motion (§5.23), nothing stayed behind. The
whole module was already general; it was only living here because this is where
it was needed first.

**And it gained the thing that makes it worth promoting.** `Buffer::fingerprint`
is a stable hash of the rendered image, quantised to 8 bits so it does not move
with floating-point noise between platforms. Record it once, assert it
afterwards, and a change to the art has to be a **decision**.

That matters more than it sounds. The art has been changed four times by someone
looking at a capture and deciding it was wrong — gems reading as kites, a coin
stack as a blob, an egg as a teardrop, a hexagon that was really a circle. Every
one of those was deliberate. What nothing could catch was an *accidental* change:
a shared helper nudged, a constant tweaked for one shape that four others also
use. `hex_vertex` is used by the gem cut and the coin's embossed face; §5.25
changed it for the gem and nothing would have said the coin moved too.

All nine art routines are now pinned, and the fingerprints recorded before the
move came through **identical** after it — the same bargain the sound set makes,
and the same proof that a promotion changed nothing.

The toolkit's own tests cover the part a game cannot: that the same drawing
fingerprints the same, that a shape moved by one pixel does not, that an empty
buffer still has one rather than being a special case a caller has to remember,
and that silhouette difference ignores colour while monochrome difference does
not — which is what makes them the right tools for two different questions.

### 5.27 Keyboard navigation (post-v1)

§5.24 and §5.25 took colour vision seriously. This is the other accessibility
axis, and it was hiding a **soft-lock**.

The game had shortcuts to *open* every panel and no way to do anything inside
one. Mostly an inconvenience — but an open Vault Pick (§5.10) **holds the game**:
the reels do not turn and a spin is refused until a chest is picked, and picking
required a mouse. Fill the hoard without one and the game stops for good. The
gamble panel (§5.16) was the same shape of problem, one decision short of a
dead end.

**Focus in an immediate-mode UI is just an index.** There is no widget tree to
walk, only a sequence of draw calls — and it is the *same* sequence every frame
while the same panels are open. So `Nav` reads the movement and activation keys
once per frame, hands each control a `Hit` as it draws, and wraps the index
against however many there turned out to be. Nothing registers in advance and no
control needs to know its own number.

Because it lives inside `virtual_button`, **every button in the game answered to
the keyboard the moment that one function changed**. Only two controls needed
touching by hand: the Vault Pick's chests and the gamble's colour buttons, both
of which had rolled their own hit tests.

Three decisions worth naming:

- **Enter activates, not Space.** Space already spins. A key that both spins and
  presses whatever is focused is a trap.
- **Focus resets when the control count changes**, because the order is only
  stable while the same panels are open. Landing on a different button because
  something else appeared is worse than starting again. It takes effect the
  frame *after*, since the count is not known until everything has drawn.
- **Focus never lands on a disabled control.** Stepping onto a greyed-out button
  and pressing Enter to no effect reads as a broken key, not a disabled button.

**The dead-code check caught the one bug that mattered.** `finish` was never
called — focus was registered every frame and never moved. Clippy noticed the
method was unused; nothing else would have, because the ring still drew on the
first control and looked perfectly plausible.

Moving focus also had to happen in `begin` rather than `finish`. Applying the
step after the controls had drawn showed the move a frame late and, worse, would
have activated the control the player had just left.

### 5.28 Hints — telling the player the game exists (post-v1)

Twenty-two systems, five cabinets, twelve overlays, twelve keyboard shortcuts —
and a new player sees a Spin button. They will never find the gamble, the buy
menu, the ledger or the machine picker, because nothing ever mentions them.

**Not a tutorial.** A scripted tour is the obvious answer and the wrong one: it
arrives before the player wants any of it, it gets skipped, it never comes back,
and it has to be maintained against a game that grows a system every iteration.

This is data. A hint has a **condition** — the same counter-and-threshold shape
the achievements use (§5.9) — and an **earned** counter that retires it. "You
have won eight times and never gambled" is a fact the game already knows; the
hint is that fact said out loud, once.

So a hint arrives when the player is ready rather than when the game loaded, and
**a player who works something out on their own is never told about it at all**.
Following the hint retires it just as surely as dismissing it, which is the whole
value of a separate `earns` counter.

**One at a time, in file order**, so a designer sets the teaching sequence by
moving a line in `hints.json` rather than by editing code. Dismissals persist
under their own key alongside preferences and achievements: a hint that has been
read is done with, whatever happens to the bankroll.

Validation rejects a set that could never work — a hint earned at zero would
never appear, and one that shows at forty spins but retires at ten is the same
fault spelled differently.

**Where it goes is the joke that writes itself.** The hint bar sits on the
footer's shortcut line — the small grey text listing every key, which is exactly
the thing that does not work and the reason this section exists. While a hint is
showing it takes that space rather than fighting it for room.

`ui.rs` reached 803 lines and the wager panel moved to `ui/wager.rs`.

### 5.29 Rules — what this cabinet does, derived from what it is (post-v1)

§5.28 told the player the panels existed. This one makes the panels tell the
truth.

**The paytable described a game we stopped shipping.** Its rules were four
hardcoded paragraphs written when there were two cabinets and both played the
same way. Since then Emberfall grew 243 ways (§5.14), Avalanche grew cascades
(§5.15), Wyrmspire grew reels that change height every spin (§5.20) and Frost
grew free spins that burn symbols off the strips (§5.21). The paragraphs never
changed. So a player on Avalanche watched symbols vanish from the grid with
nothing anywhere in the game to say why — and the gamble and the buy menu were
never explained on any cabinet at all.

**Writing five paragraphs would work until the sixth cabinet**, which is exactly
how this happened the first time. The problem was never that the text was wrong;
it is that nothing could *tell* it was wrong.

So the config is read twice, by two functions that share no code:

- `Topic::present` asks which mechanics a cabinet **has**, from config presence
  alone — a `cascade.json` exists, `reel_heights` is set, the award table is not
  empty.
- `rules` produces the prose that **explains** them.

A test asserts the two agree for every machine, and `validate` runs the same
check when a cabinet is loaded. Add a machine with a mechanic and forget to
describe it and the build fails naming the topic. That is the failure that
should have fired three iterations ago and could not.

Deriving prose from config also settles the numbers: every figure is read from
the data the engine pays out of, so a rule cannot quote a trigger of three
scatters at a cabinet that wants four.

**Layout is measured, not guessed.** A cabinet produces nine to eleven rules and
a sixth could produce more, so the whole set is measured and the type size
chosen to fit two columns — everything on screen at once, no scroll bar. Because
`wrap_text` needs a loaded font, the measurement is injected: the game passes the
real font, the tests pass a deliberately pessimistic estimate, so a layout that
fits under test has room to spare in the game.

**The keyboard became a table too** (`ui/shortcuts.rs`). The bindings were twenty
`is_key_pressed` branches and the footer listing them was a hand-written string.
They drifted, and then they collided: `L` opened the Ledger **and** triggered
Load, so pressing it to read the statistics discarded the bankroll they
described. Two branches, each individually correct, and nothing able to see the
pair. Now one table drives the key handling, generates the footer line, and is
asserted collision-free. Load moved to `K`.

Two affordances follow from §5.28's own lesson — a shortcut nobody is told about
is not an affordance: a full-width **"How <cabinet> plays"** button above the
spin block, and a rules hint placed first in `hints.json` so it is the first
thing a new player is told. Hints are now suppressed while any overlay is open;
a hint offers something to do next, and behind a modal there is nothing to do
next.

The paytable keeps the symbol table and points at `R`. `game.rs` reached 801
lines and the save/load block moved to `game/persistence.rs`.

### 5.30 Session limits and the reality check (post-v1)

Twenty-five systems have gone into being honest about the maths. The jackpots
are solved in closed form (§5.7), the buy tiers priced at the machine's own
return (§5.13), the gamble proved neutral (§5.16), the ledger set against the
profile (§5.18), the rules generated from the config (§5.29).

**All of it describes the machine. None of it describes the session.** How long
this has been going on, what has gone in, and what has come back are the numbers
a player actually loses track of — and a slot machine is specifically good at
making them hard to hold on to, because the balance is one number that moves in
both directions and it is the only one on screen.

**The reality check** states three figures at an interval the player chooses:
spins, staked, returned, and the net between them. It holds the game, because a
notification that can be played through is one that will be played through, and
it stops an unattended autospin run — which is exactly the state it exists to
interrupt. It does not congratulate, warn or advise. The one coloured figure is
the net, and it is coloured by fact rather than sentiment. The one editorial line
is that a few hundred spins cannot measure a return, which is what §5.17 learned
about twenty thousand, said where it is most likely to be misread.

It also states plainly that **every credit is play money**. A game that borrows
the shape of a slot machine this closely should be unambiguous about the one way
it differs, and burying that in an about box would be the dishonest choice.

**Three caps** — time, net loss, paid spins — all off by default, because
capping play by default would be making a decision that is not the game's to
make. A cap refuses the spin before the session ever sees it, so nothing
downstream knows limits exist; everything else stays open, and a player who has
stopped playing can still read their ledger and their figures.

**The whole system is one asymmetry.** A limit you can lift the moment it binds
is a suggestion; a limit you can never lift is a trap, and this is play money.
So **tightening takes effect immediately** — deciding you have had enough should
never involve waiting — and **loosening takes effect at the next session**. That
one rule does all the work: it moves the decision to raise a limit out of the
moment that made you want to raise it. The panel shows both values at once when
they differ, because hiding it would make the button feel broken.

A breach is sticky. Winning back does not lift it — "play until you are even" is
the exact thought the cap was set to interrupt. Only a new game clears one, and a
new game is also when filed loosenings land.

Caps live beside preferences rather than in the save, so a fresh bankroll does
not clear them; it is the *pending* set that persists, since that is what the
player asked for and the tighten-now rule reconstructs the rest.

`game.rs` reached 856 lines and the outcome dispatch moved to
`game/outcomes.rs`, on the seam `actions.rs` already draws: that module decides
what happened, this one decides what the game does about it.

### 5.31 Music (post-v1)

§7.1 shipped the effects generated in code and left the room silent between
them. This fills it: four tracks, four bars of i–VI–III–VII in D minor, written
in Rust and rendered by the same synth the blips come from.

**Vertical remixing.** All four tracks are the same length, started together on
the first frame and **left running for the whole session**. Nothing is ever
started or stopped in response to gameplay — only the four volumes move. That
buys two things a start/stop approach cannot: it **cannot glitch**, because a
track begun when free spins trigger would enter wherever the bar happened to be
and would need scheduling against the beat to avoid sounding like a mistake; and
every transition is a **fade**, so the arrangement thickens and thins rather than
cutting. Base is bass and pad, free spins bring the arpeggio, the Wrath adds the
drum and pushes everything else down. The mood is *derived* from session state
each frame rather than set at trigger points, so a state the music should react
to cannot be added without that line seeing it.

**`macroquad-toolkit::score`** is the new layer between notes and tones. `synth`
speaks frequencies and envelopes, which is right for a blip and wrong for eight
bars of anything: writing a bass line as hertz values and second offsets makes
every edit arithmetic and a wrong note indistinguishable from a typo. `score`
adds a scale, a tempo, and notes placed on beats by degree, and `lay` turns them
into the `Voice` list the synth already rendered. Nothing in `synth` changed.

**Written by someone who has never heard it**, like the effects. That rules out
mixing by ear and puts the weight on what can be measured — and the three faults
that matter are all inaudible until they are not:

- **Phase drift.** Every track must render to exactly `Timing::samples()`. The
  synth sizes its buffer from the last voice that sounds, so a track whose final
  bar is empty renders short; one sample of drift per loop is inaudible on the
  first pass and a disaster on the fiftieth.
- **The seam.** A loop whose last sample is nowhere near its first steps
  discontinuously every wrap — a click once per repeat, easy to miss once and
  impossible to ignore after five minutes.
- **The summed clip.** Four tracks each peaking at a comfortable 0.6 sum to 2.4.
  It is the one arrangement nobody auditions, because it only happens in the
  game, so `mixed_peak` checks every mood at its real gains.

Tests also hold every written note in key, since a mistyped degree is
indistinguishable from a deliberate one, and prove the three moods are actually
different arrangements rather than three names for the same mix.

**The waveform inspector (§5.19) earned its keep again.** The music went into it
for the same reason the effects did, and the first capture showed the arpeggio
peaking at **0.04** and the drum at **0.07** against the bass's 0.19 — the two
tracks that carry every transition, written so quiet they would never have been
heard under the effects at all. Both were levelled and a test now holds every
track within a factor of three of the loudest. Each row also lists **every**
mood's gain, not just the current one, so the whole arrangement is readable at a
glance rather than one mix at a time.

Music gets its own volume row, separate from the effects: it plays constantly and
they do not, so a player who wants one quiet rarely wants both quiet.

### 5.32 The session graph (post-v1)

The profiler (§5.17) simulates twenty thousand rounds and reports a distribution.
The ledger (§5.18) records what the player has actually seen and sets it beside
that distribution. The reality check (§5.30) states the session totals. All three
are **summaries**, and every one answers "how much" while carefully avoiding
"what did it feel like".

Which is the question a player is really asking. A hit frequency of 0.41 and a
return of 95% describe a session perfectly and convey nothing about the forty
spins that paid nothing followed by one that paid two hundred times. **That is
the shape of the thing, and it is only visible over time.**

So the bankroll is recorded after every round and drawn. The graph makes an
argument no table can: the long grinding decline is the normal state, the spikes
are where the money comes back, and the two are the same machine.

**`macroquad-toolkit::series`** is what makes it affordable. A session is
unbounded and memory is not, and both usual answers are wrong for a graph whose
*point* is the variation:

- A **ring buffer** drops the beginning — the player's first hour vanishes, and
  with it any sense of where they started.
- **Averaging into buckets** keeps the span and destroys the detail. `+40,000`
  and `-200` average to a shrug. The spike *is* the information.

Instead each slot holds the min, max and closing value over its span, and a full
series merges adjacent pairs — min of mins, max of maxes, last of the later.
Halving the count doubles the time per slot, and the merge is **lossless in the
extremes**: however many times a series has decimated, its reported minimum and
maximum are still exactly the smallest and largest ever pushed. Resolution
decays; the envelope never does. The plot then draws the **band** between each
bucket's extremes with the closing line over it, so a bucket covering hundreds of
rounds still shows every spike at the height it reached rather than a tidy line
implying a calm that never happened.

The baseline is the **opening balance**, not zero and not the middle of the
range, so "am I up or down" is answered without arithmetic and the reference is
always kept in view.

**Marks** give the spikes causes. Every feature, hatch, Wrath, jackpot and big
win is recorded where it happened — taken from the celebration queue, since a
card is raised exactly when something worth pointing at occurred, which is one
seam rather than five scattered through the spin handling.

**The capture found the flaw in the marks.** The first budget dropped the oldest
when full, which is the obvious policy and looks broken: three hundred hatches
over four thousand rounds meant the surviving sixty-four were all from the last
few minutes, so the graph drew a wall against the right-hand edge and said
nothing about the session it was describing. They are **halved** now, the same
principle the series uses, which spreads coverage across the whole session at
declining density. A jackpot is exempt — it is the rarest thing the game does —
but the exemption yields to the budget, because a session of nothing but jackpots
would otherwise keep every mark and grow without bound.

Deepest fall is reported as **"at least"**: once buckets merge, a peak and the
trough after it can share one and their order is no longer known, so the figure
is an honest lower bound rather than a number pretending to be exact.

Unlike the ledger, this does not persist. It is a session in the same sense
§5.30 means it, and a graph spanning six sittings would be a different and much
less interesting picture.

### 5.33 The conservation harness (post-v1)

Every RTP figure in this document comes from `GameSession::spin` — the headless
path the sim and the profiler drive. It settles a spin and then resolves any
feature immediately through `auto_play_bonus` and `auto_play_holdspin`, because a
Monte-Carlo run cannot wait for a beat timer.

**Nobody plays that game.** A player goes through `begin_spin` and then
`update_spin` a frame at a time, and their features resolve through `pick_bonus`
and `tick_holdspin` instead. Four functions, two per feature, and until now
nothing anywhere asserted the two pairs pay the same. If they ever diverged,
twenty-seven sections of published figures would describe a game that is not the
one being shipped — and no existing test would notice, because the sim would go
on measuring itself, correctly, forever.

So this drives the **interactive** path headless: whole rounds frame by frame at
a fixed timestep, dismissing cards, picking chests, letting respin rounds beat
themselves out, playing every free spin a round bought. And it holds the session
to conservation laws while it does:

- **The books balance.** `opening + won - wagered == balance`, exactly, after
  every round. `total_won` and `total_wagered` are maintained by different code
  from `balance`, so this is a cross-check rather than a tautology: any path that
  moves credits without accounting for them breaks it.
- Nothing goes negative, no round leaves the machine stuck, the hoard never
  overfills, and a progressive pot only ever falls on a round that won.

**Measured, over 200,000 rounds per cabinet, against the sim on the same seed:**

| Cabinet | Interactive RTP | Sim RTP | Interactive hits | Sim hits |
|---|---|---|---|---|
| Dragon's Hoard | 0.9512 | 0.9370 | 0.4113 | 0.4099 |
| Frost Wyrm | 0.9257 | 0.9123 | 0.2596 | 0.2584 |
| Emberfall | 0.9655 | 0.9575 | 0.6247 | 0.6236 |
| Wyrmspire | 0.9527 | 0.9520 | 0.6735 | 0.6723 |
| Avalanche | 0.9480 | 0.9474 | 0.4763 | 0.4747 |

Hit frequency is all but free of sampling noise at this scale, and the two paths
agree on it to **0.0016 on every cabinet**. That is structural agreement rather
than luck. Return still swings, being dominated by the rare enormous payouts —
and the residual gaps rank in **volatility order**: Frost and Dragon's Hoard, the
two with the fattest tails, sit about 1.4% apart, while Wyrmspire and Avalanche
land within 0.1%. That is the signature of sampling error, not of a difference
between the paths, which would not care how volatile the cabinet was.

**Two things the harness taught on the way.** Its first version picked chest zero
on every open board and hung on every round, because a chest already turned over
cannot be picked again — the driver now takes the first unopened one, which is
what a player clicking blind does. And the round-level hit rate turned out to
equal the sim's spin-level counter exactly, for a reason worth knowing: the
scatter pays from anywhere, so a round that awards free spins has **always
already paid something**. There is no such thing here as a round that pays only
inside its feature.

The module is `#[cfg(test)]`, like the parts of `sim` it is checked against. It
exists to prove the shipped game pays what the published figures say, not to be
part of the shipped game.

### 5.34 The game's own words and numbers (post-v1)

Every symbol carries a `short` — a three-letter code the reel renderer draws when
it cannot draw the art, and the paytable puts in its swatch. That is what the
field is for, and it works.

**It had also leaked into prose.** A win read `CHS x3 on line 19` while the
paytable two keystrokes away called the same symbol "Treasure Chest", and a
refining free spin announced `burned FRC RIM` (§5.21). The player was being told
what had just happened in a code they were never given, about symbols the game
names perfectly well everywhere else.

It survived twenty-eight iterations because nothing was wrong with it *locally*:
each call site had a symbol in hand and reached for the nearest string on it. So
the fix is not the two edits — it is one place that decides how anything is
named, and **a test that no short code can reach prose again**. A win now reads
`Ruby ×3 on line 17`.

**The same audit found the other half.** A game entirely about quantities was
rendering them with `to_string()`: `Balance 1000150`, a peak of `1009419`, a
stake of `35800`. Seven digits a player has to count with their eye to know
whether they have a million or ten.

The sharpest evidence was already in the codebase. `ui/reels.rs` held a private
`format_credits` with thousands separators, used by the jackpot ladder and
nothing else — so the game had known separators were needed since the ladder was
written, in exactly one place. That is why the Grand read `25,000` while the
balance beside it read `1000150`. It is deleted; everything goes through the one
function now.

**`macroquad-toolkit::ui::number`** is where the formatting lives, since every
game in the workspace counts something. `grouped` keeps every digit, for anything
the player might do arithmetic on. `compact` trades low digits for width — `1.2M`
— for axis labels and bars, and **rounds toward zero**, so a compact figure is
never larger than the number it stands for: a bar labelled `1.3M` beside a total
of `1,249,999` invites the reader to think a digit went missing. `compact` is
explicitly wrong for a balance, because being unable to see your own money to the
credit costs more trust than it saves pixels.

The dividing line is **magnitude against identifier**. Credits and large counts
are grouped; multipliers and payline numbers are not, because `×1,000` is worse
than `×1000` and a grouped line number would read as money.

Two of the author's own test expectations were wrong and the toolkit corrected
them: `compact(999_999)` is `999K`, not `999.9K` — three whole digits, so the
decimal goes — and `i64::MIN` has no unit above `B`, so it stops shortening rather
than becoming wrong. The reason that input is tested at all is that `-i64::MIN`
overflows: it is the one value that sails through every test written with small
numbers and then panics in a release build.

### 5.35 Cluster pays — a sixth cabinet (post-v1)

The five cabinets differ in what a win *is*, but all five read the grid the same
way: **reel by reel, left to right**. Lines walk a fixed path across it (§3),
ways pay every route through it (§5.14), and a shifting cabinet changes how tall
each column is (§5.20). All three inherit the same assumption from a physical
machine — that a reel is a thing, and order along it matters.

**Tidepool does not care.** A win is a connected group of the same symbol,
orthogonally adjacent, anywhere on a 6×5 grid, paid on how many cells are in it.
A blob in the corner spanning three columns is a win; the same eight symbols
spread evenly are nothing. Reel one is not special and neither is direction. It
is the first model where the **shape** of the grid matters more than its columns,
which is why it belongs beside cascades (§5.15): removing a cluster drops symbols
into a hole with edges.

**Flood fill, and one rule that needs care.** Wilds join any cluster they touch,
so a wild may belong to several at once — it is one cell that reads as a gem to
the gems beside it and as a coin to the coins. But it must never *start* one, or
a run of adjacent wilds would pay as a cluster of nothing. Ordinary cells are
consumed, so no cell is ever paid twice: a cluster is a connected component, and
connected components do not overlap. That is the fault the model invites and
where the tests spend most of their effort.

**Three existing systems refused to let this ship half-finished**, which is
exactly what they were built for:

- §5.29's closed `Topic` enum **failed the build** until the new model had prose
  explaining it. The rules panel now describes clusters on the cabinet that has
  them and nowhere else, with the minimum group size read from the engine
  constant rather than retyped.
- §5.34's `WinSource` match refused to compile until a cluster win had words. It
  reads `Copper Coin cluster of 6`.
- §5.33's conservation harness validated the new cabinet without being touched:
  interactive **0.9005** against the sim's **0.8906**, hit rates **0.4576** and
  **0.4563** — the same 0.0013 structural agreement the other five show.

**Tuning it was the whole difficulty.** A 30-cell grid is twice a 5×3 one, and
every count-based trigger doubles with it. The first honest measurement was
**RTP 8.27**: thirteen thousand free spins and fourteen hundred Wrath rounds out
of twenty thousand paid spins, because four scatters and four eggs are common on
thirty cells and rare on fifteen. Scaling the triggers overcorrected to 0.26 with
no features at all. The settled cabinet reaches **0.9630** over a million spins,
with base 0.681, free spins 0.148 and the rest split across the shared features.

Two things were learned in the tuning. The paytable had to be **written for
clusters rather than inherited from a line machine** — a "5" rung that means
"five in a row" is worth 2× the line bet and a five-cell cluster needs to be worth
far more. And §5.17's lesson landed again: a 20,000-round smoke test read 0.952
while the million-spin run said **0.9099**. The short sample was luck, and the
long test is the only gate that counts.

Three defects the capture caught and no test could: the cabinet name overflowed
into the Buy button, the wager panel read `Lines: 0` on a machine that has none,
and the win line ran its parts together — `cluster of 6 Gold Coins cluster of 6`
— because three spaces are not a separator.

### 5.36 A second symbol set — and the gate that was not one (post-v1)

§5.35 shipped Tidepool with a genuinely new win model and Dragon's Hoard's
symbols: a machine named for a rock pool showing treasure chests and a dragon.
Six cabinets drawing the same nine shapes was the weakest thing about the game to
look at, and the part with least to do with what any of them actually did.

So Tidepool has its own nine: **a spiral shell, a pearl, a starfish, a sea
urchin, an anemone, a crab, coral, a kraken and a breaking wave**, drawn from the
same canvas primitives on the same contract — one colour in the JSON, [`Shades`]
derives the rest. `symbols.rs` was split into `symbols/hoard.rs` and
`symbols/tidepool.rs` on the obvious seam: everything in both takes a canvas and
some shades and draws, and neither knows what a symbol or a machine is.

**The real find was a guard that could not guard.** §5.25 has a fingerprint
baseline so a change to the art has to be a deliberate one, and a companion test
named `the_baseline_covers_every_art_routine` whose comment reads *"a new shape
added without a baseline entry would slip past the test above entirely"*. It read
`GameData::load()` — the **first** cabinet. So nine new routines arrived on the
sixth, none of them baselined, and the test written to catch exactly that passed
without comment. A gate scoped to one machine is not a gate on a game with six;
it and the two beside it now walk every cabinet.

Widening it surfaced a second thing: **a single baseline per art id had never
been valid**. Cabinets give the same routine different colours — Frost Wyrm's
coin is not Dragon's Hoard's — so a fingerprint taken from the shipped colour is
six numbers for one shape, and pinning one fails the other five. The baseline
renders on a fixed neutral grey now, because it is about the **routine**; what a
colour change should trip is the dichromacy gate, which is a different test
asking a different question. Every fingerprint was regenerated.

The two art gates that already ran per cabinet did their job the moment the file
existed: no two symbols may look alike at the smallest cell the game draws, and
any two sharing a shape must separate under three simulated dichromacies. A
tidepool wants to be blue and green, and blue-green is the axis a deuteranope
loses — so the silhouettes carry the difference and the palette runs from sand
through coral to deep water rather than sitting in one band.

**The capture caught the one thing the tests could not.** The scatter — a circle
with a darker circle inside it and a triangle on top — read unmistakably as a
flying saucer. It passed every gate, because "distinct from the other eight" and
"looks like a wave" are different claims and only the first is measurable. A wave
is not a round thing with a dome; it is a **hook**. Rebuilt as a rising flank, a
crest thrown forward past its own base, and the lip falling back inside the
curve.

### 5.37 The layout audit (post-v1)

Four separate text-overflow defects shipped during this game's development and
every one was found **by looking at a screenshot**: the generated shortcut line
clipped its last entry (§5.29), the cabinet name ran into the Buy button
(§5.35), the waveform panel's mood list was cut off (§5.31), and the paytable's
prose spilled onto the footer behind it.

Text that runs past its panel is the one UI fault no ordinary test sees. The draw
call succeeds, the frame renders, nothing is out of range — the sentence is
simply cut off, or drawn over the thing beside it. It is found by looking, which
means it is found late and only if someone happens to open that screen.

**Bound the region, not the call.** `draw_ui_text_ex` takes a position and no
width, and there are 107 of them here. Giving every one an explicit box is 107
edits and 107 chances to write the wrong number. But text is always drawn
*inside something* — a panel, a row — and that something already knows how wide
it is, because it was drawn from a `Rect`. So `macroquad-toolkit::ui::bounds`
adds a `Region` guard that pushes those bounds while it lives, and every text
draw inside compares what it measured against what it had. **Fifteen guards cover
all 107 draws**, including code written later that never heard of the module.

It is an RAII guard on purpose: half the panels in a game return early when they
have nothing to show, and a stack that leaked would bound every later draw by a
dead panel.

**Recording, not clipping.** Nothing here changes what is drawn. Overflowing text
still overflows, because silently shrinking or truncating it would replace a
visible bug with an invisible one — a sentence quietly losing its last three
words is worse than one that obviously collides.

**Measured with the real font**, which means the audit runs inside the capture
harness rather than as a unit test: a `layout_audit` scene opens every overlay at
once, seeds the widest numbers the game can hold, and the frame reports what did
not fit. Recording is off unless asked for, so a shipped frame pays one
thread-local read per draw.

**It is a gate, not a report** — findings exit non-zero. A printout nobody reads
is exactly the state this replaced.

**And it was proved before it was trusted.** The first real run said "nothing
overflowed", which is a suspicious result for a detector's first outing, so a
deliberately over-wide string went into the header to see whether it fired. It
did not — and the reason was instructive: the probe was a hundred characters of
ordinary prose, and the header is 1,244 pixels wide, so it genuinely fitted. A
longer probe reported **932px past the edge**. Only then was the clean result
worth anything.

The audit covers the fifteen bounded regions: every overlay, the header, the
footer and the wager panel. The gamble, the Vault Pick board and the respin round
draw from session state rather than a flag and are not yet in the scene.

### 5.38 Text size — the accessibility leg that was missing (post-v1)

The accessibility work so far covers colour (§5.24, §5.25), motor (§5.27),
cognitive (§5.28, §5.29) and harm reduction (§5.30). **Visual acuity was the one
leg with nothing under it** — the game drew at one size and offered no way to make
it bigger.

The mechanism turned out to already exist and be unused, exactly as
`music_volume` was before §5.31: the toolkit has `set_ui_text_scale`, and
`TextStyle::params()` runs every size through it, so **every draw in the game
already honoured a scale nobody was setting**. Wiring the preference was a
morning's work.

**The reason this could not be done before §5.37 is the whole point.** Making text
bigger inside a layout of fixed rectangles breaks things, and it breaks them
invisibly — a sentence is simply cut off. Guessing which panels would suffer, at
which size, across fifteen overlays, is not a job anyone does well. With the
layout audit it is a list:

```
=== scale 1.15 ===
layout audit: 94px past the edge — "Each machine keeps its own balance, hoard..."
=== scale 1.30 ===
layout audit: 234px past the edge — "Each machine keeps its own balance, hoard..."
layout audit: 7px past the edge — "Settings are kept separately from your save..."
layout audit: 26px past the edge — "Prices come from what each feature actually pays..."
```

Three findings, all of them the same mistake: a long footnote set as **one
unwrapped line** rather than a wrapping block. At the design size each fitted by
a comfortable margin, which is why none had ever been noticed. All three are
`draw_text_block` now and reflow instead of insisting on their width — which is
what a low-priority footnote should have done from the start.

The audit runs at every offered size through `DRAGONS_HOARD_TEXT_SCALE`, so
adding a size to the list means re-running it rather than hoping.

**Three sizes, not a slider.** 100%, 115% and 130%, and every one is a size the
panels have actually been measured at. A continuous control would let a player
pick a size nobody ever laid the game out for, and the failure would be silent.
The list only goes **up**: the design size is the floor, because anything smaller
fails the legibility standard §5.25 holds the art to, and it is the default, so a
player who never opens settings sees exactly what was drawn for them.

### 5.39 Pseudolocalisation (post-v1)

The game has about 130 distinct user-facing strings and every one is a hardcoded
English literal. Migrating them to a catalogue is a large, mechanical job.
**Discovering afterwards that half the panels were laid out to the exact width of
their English copy is a much worse one**, because German runs roughly 35% longer
and the fix is a redesign rather than a retranslation.

Pseudolocalisation finds that today, without a translator and without migrating
anything. Every string is transformed on its way to the screen into something
still readable as English but carrying the properties translated text has:

- **Longer.** Padded 40%, so a panel that only just fits its own copy fails now
  rather than in the German build.
- **Accented.** `Settings` becomes `Śéttíñgś`, which instantly shows a font with
  no glyph for `é` — a missing glyph draws as a box, and finding that after
  shipping a language is finding it late.
- **Bracketed.** `[Śéttíñgś···]` marks the whole string, so a label built by
  **gluing two strings together** appears as `[..][..]`. That is the fault a
  translator cannot work around: word order is not universal, and a sentence
  assembled from fragments cannot be reordered.

Anything the brackets do not touch never went through a text helper at all,
which is its own finding.

**It ships no translations and claims none.** It is a measurement, run with the
layout audit (§5.37) to produce a list of what would have to change.

**The first run reported eleven overflows, and every one was the tool's own
fault.** The output showed `[[doubly marked]]` text: the string was expanded
once for layout and again for each wrapped line, measuring a width no
translation would ever produce. The fix is a re-entrancy guard — `Pseudo::Once`
— held across layout *and* drawing, because **the unit that gets expanded is the
whole block**. `draw_text_block` holds it internally; the rules panel (§5.29)
wraps its own text and draws it a line at a time, so it holds it explicitly.

With that corrected the audit is **clean at 40% expansion**, and the capture
shows no tofu anywhere — the shipped font has full accented-Latin coverage. So
the layout is translation-ready and the font is too; what remains is the words.

That the eleven findings were all artefacts is worth stating plainly. A detector
whose first output is a long list is as likely to be describing itself as the
thing it points at, which is the same lesson §5.37 learned from the opposite
direction when its first clean run had to be disproved before it could be
believed.

### 5.40 Contrast — every button in the game was below the standard (post-v1)

Picking UI colours is done by eye, on a good monitor, by the person who chose
them and therefore already knows what they say. Contrast is one of the few things
about a visual design that is genuinely **objective**, and this game had never
measured it.

`macroquad-toolkit::ui::contrast` is the WCAG figure: relative luminance of the
lighter colour over the darker, 1.0 for identical colours and 21.0 for black on
white. Two details matter more than the formula. Luminance is **weighted**, not
averaged — averaging raw channels overstates blue by a factor of twelve. And a
colour drawn at less than full alpha is **composited first**, because 40%-alpha
white on black is grey; measuring the un-composited value would claim 21:1 for
something barely legible.

**§5.37's `Region` already knew where text was drawn; it now knows what it was
drawn on.** `Region::on(rect, surface)` turns the layout audit into a check for
both fit *and* legibility, reported the same way.

The first useful run found **seventeen failures, and they were the buttons** —
every one of them. White labels on the bright tones measured **2.1:1** where 4.5
is asked for; the blue ones 3.0, the red 3.8. The fills had been chosen to look
right against a dark panel rather than against the text on top of them, which is
exactly the mistake the eye cannot catch.

**The fix derives the fill from the requirement.** `darken_until` scales a
background toward black only as far as its label needs — a legible pairing is
returned untouched. Hand-picking a colour per tone would work until someone
adjusted one; this way a fill **cannot** be unreadable, and the buttons stay
recognisably green, blue and red because they are darkened by a step or two, not
flattened.

**One finding was the tool's fault and one was the fix being reported as the
fault.** The 1.1:1 on the hoard meter's label turned out to be its *outline* —
the same string drawn four times in near-black behind the label, which is
precisely what makes it readable over a bright fill. `Decorative` marks draws
that are not meant to be read. It is not for silencing an inconvenient finding:
it says *this is a stroke, not a word*, and only something drawing twice should
hold it.

Widening the hook also closed a real coverage gap. `draw_text_block` and
`draw_text_centered_in_box` go straight to macroquad rather than through
`draw_ui_text_ex`, so **every button label and every wrapped paragraph in the
game had been outside the audit** since §5.37 — which is why the first contrast
run looked clean. That is twice now that a clean result has had to be disbelieved
before it meant anything.

The audit is clean at the design size, at 130% text (§5.38) and under
pseudolocalisation (§5.39).

### 5.41 Symbol sets — what a symbol *is* against what it pays here (post-v1)

Six cabinets each carried a full `symbols.json`: nine definitions apiece, and
four of them byte-identical. Frost Wyrm had its own **names** — Frost Coin, Rime
Hoard, Glacier Gem — since §5.21 and had never had its own anything else, while
Emberfall, Wyrmspire and Avalanche were unedited copies of Dragon's Hoard.

The duplication was the symptom; the modelling was the cause. A symbol
definition mixed two different things:

- **What the symbol is** — name, art, colour, and whether it is the wild, the
  scatter or the hoard symbol. Shared, and the same wherever it appears.
- **What it pays on this cabinet** — which is emphatically not shared. Tidepool's
  rungs are tuned for clusters (§5.35) and mean something different from a
  five-in-a-row.

So identity moved to `assets/data/symbols/<set>.json` and payouts stayed with the
cabinet as `paytable.json`, joined at load. Retheming a machine is now naming a
different set. A paytable that names a symbol the set does not have is rejected
outright — that is a rename half-finished, and the symbol would otherwise just
never pay.

**Emberfall got the first new set**: Ember, Cinder Heap, Sparkstone, Slag Glass,
Firebrand, The Forge, Wyrm Egg, Emberwyrm and Pyre, with one new art routine — an
anvil, since a treasure chest means nothing in a forge. The palette runs hot, and
the two cool symbols are not an accident: an all-red set collapses for a
protanope, so §5.24's gate pushes toward a counterpoint whether or not anyone was
thinking about it.

**Two existing gates did their jobs unprompted.** §5.36's widened baseline check
failed the build with `ways: 'anvil' has no baseline` the moment the shape
existed — which is exactly the failure it was widened to catch, one iteration
after it had missed nine of them. And the RTP of all six cabinets is unchanged to
four decimal places, confirming the split moved nothing but structure.

**The tests had to stop naming symbols.** `ways.rs` asserted about `"dragon"` and
`"jade"`, which are theme rather than rule, and broke the moment a cabinet was
rethemed. They resolve by **role or position** now — `"wild"`, `"scatter"`, or an
index into the set — so a test says *the wild leads a run* rather than *the
Dragon leads a run*, which is what it always meant.

### 5.42 Three more themes — and how much of one is the palette (post-v1)

§5.41 built the mechanism and used it once. Frost Wyrm, Wyrmspire and Avalanche
were still drawing Dragon's Hoard's coins and treasure chest, which made three of
six cabinets visually indistinguishable from a fourth.

**Two new shapes each, not nine.** Tidepool needed a full set because a rock pool
has nothing in common with a hoard. These three do — they are dragons and
treasure at altitude, and the low symbols are coins and gems on every one of
them. Redrawing a coin three more times would be work with no reader.

So each keeps the shared low-tier silhouettes and gets **its own premiums**: the
symbols a player actually looks for, and the ones that carry a theme. Frost Wyrm
has an **icefall** and a **snowflake**, Wyrmspire a **spire** and an **updraft
feather**, Avalanche a **boulder** and a **lone pine**. What separates the three
at a glance is then the palette, which is the honest answer for three cabinets
that really are variations on one idea.

The palette is doing more work than it looks like, and §5.24 is why. Two symbols
sharing a silhouette must stay apart under three simulated dichromacies, so an
ice cabinet cannot be six blues and a scree cabinet cannot be six greys. Each set
runs an accent against its base — Frost's amethyst, Wyrmspire's garnet, the
cinnabar in Avalanche — because the gate does not allow otherwise.

**Every gate fired, and two found real faults.**

- §5.41's paytable check: `frost: no paytable entry for symbol 'icicle'`. Frost's
  vault had become an icefall in the set and nowhere else — exactly the
  half-finished rename that check exists for, caught on the first run.
- §5.25's legibility gate: `wyrmspire/updraft covers 0.091 of its cell`. The
  feather's vanes were small triangles along a shaft, which at reel size is a
  scratch rather than a symbol. Redrawn as quads spanning most of the width.
- §5.36's baseline check demanded fingerprints for all six new routines before
  the build would pass.

Every cabinet's RTP is unchanged to four decimal places, and the conservation
harness (§5.33) still agrees with the sim across all six.

### 5.43 A room per cabinet (post-v1)

§5.41 and §5.42 gave every cabinet its own symbols. The panels, reel frame,
header and footer stayed the same stone and gold on all six, so Tidepool's
shells sat in a dragon's vault and Frost Wyrm's ice sat in a warm gold surround.
The symbols were themed; **the room was not.**

**This is the change that could not be made before §5.40.** The palette was
eleven constants used in **298 places**, and recolouring a whole interface by
hand is exactly the edit whose failures are invisible: a label legible on stone
is not legible on ice, and nothing says so. The contrast gate is what makes it
safe — each theme is run through the layout audit, and a pairing that cannot be
read fails rather than ships.

That is the argument for having built the accessibility work first. **A gate is
not only a check on what exists; it is permission to change it.**

The constants became functions reading a thread-local theme, so every call site
reads as it did — `palette::gold()` where it said `palette::GOLD` — which is why
298 of them could be converted mechanically and reviewed by the gate instead of
by eye. The theme changes when a cabinet loads and at no other moment.

Six palettes: gold on stone, blue-black ice, forge-dark ember, bare stone and
sky, snow-shadow, and deep water with sand. Three tests hold them:

- **Every theme is readable** — every ink on every surface, at the size it is
  drawn.
- **No theme is the same as another**, or two cabinets look alike again.
- **Every accent leaves the hue of its theme.** An ice cabinet whose "down"
  colour is also blue has no down colour, so the ember accent must stay warm and
  the jade cool whatever the surround does. §5.24's argument, applied to chrome
  rather than symbols.

**Buttons deliberately do not follow the theme.** Green means go and red means
destructive; a Delete Save that turned ice-blue on one cabinet would stop reading
as dangerous. Semantic colour is not decoration.

The capture scenes were assigning `self.data` directly, and a cabinet now brings
its palette with it — eight sites that would each have had to remember. They go
through one helper.

The audit is clean under all six themes, at 130% text and under
pseudolocalisation.

### 5.44 A score per cabinet (post-v1)

§5.41 gave every cabinet its own symbols and §5.43 its own room. All six still
played the same four-track loop, which is the same gap one sense over.

**The constraint shaped the design.** `Music::load` is async and switching
cabinets is not — re-rendering four tracks mid-game is not something the machine
picker can await, and pre-rendering six scores at boot is six times the work and
the memory for five arrangements nobody is listening to.

So a cabinet does not get its own *notes*; it gets its own **instrumentation**.
The stem palette grew from four to eight — bass, pad, arpeggio, drum, plus a
**bell** on the bar line, a **drone** under everything, an offbeat **pluck** and a
sixteenth **rattle** — rendered once, and each cabinet names a mix. It is the same
shape as the symbol sets: a shared resource, a per-cabinet selection.

Key and tempo are therefore shared, and **have to be**: stems that did not agree
on both could not be layered at all. That is a real limit and worth stating — the
cabinets differ in what is playing, not in what it is playing.

The mix is written as a **grid** rather than a match, because that is what it is
and because a grid reads down a column: it is immediately visible that the drum
belongs to the held rounds everywhere, and that no two rooms sound alike. Frost
is bell and drone with the bass held back; Emberfall runs rattle and drum from
the first spin, which nothing else does; Wyrmspire is pad and air with almost no
low end; Avalanche is weight waiting to move; Tidepool keeps the arpeggio going
even at rest.

**Switching cabinet is a fade, not a cut** — the stems never stop, so a machine
change sounds like a mood change rather than like the music restarting.

Six tests carry it, and two are new in kind: **no two cabinets sound the same**,
and **every cabinet plays at least three stems in every mood** — fewer reads as a
sound rather than as music.

**The waveform panel earned its keep a third time.** The drone came out at
**0.012 peak against the bass's 0.202** — a fifty-percent attack on an eleven-
second note never arrives — and the pluck and rattle at 0.08, which is audible in
principle and not in a mix. All three were levelled. That is the same fault, found
the same way, that §5.31 found in the arpeggio and the drum.

### 5.45 Touch (post-v1)

The game ships to WebGL and is served in a browser, which means it is served to
phones. It was mouse-and-keyboard only.

**Both are the same question.** A game built for a mouse checks
`is_mouse_button_released` at every control; adding touch that way means finding
all of them and remembering the differences. Miss one and it is simply dead on a
phone, silently. So both become a **`Pointer`** — a position and whether it was
released — and it turned out the game already had a single seam: `Nav::control`
is the only place a control asks whether it was pressed, because \u00a75.27 made every
button keyboard-reachable through it. Touch went in at one site.

`Pointer::pressing` reads only its own fields. Reaching for
`is_mouse_button_down` inside it would make every control untestable without a
window, which is how a UI ends up with no tests at all.

**Hover is a mouse idea.** `hovering` is false for touch: a finger is not *over*
anything until it is *on* it, and a UI that lights up under a finger is showing
the player what they have already committed to.

**How big is big enough.** Every guideline puts a touch target near **44 CSS
pixels**. A game drawn at a fixed logical size has no size in those units until
it is on a screen, so the useful question is inverted: *how wide must the window
be before every control clears the standard?* One number, and it goes down when
the layout improves.

The first answer was **2347px** — wider than most laptops, with a 24-pixel
control setting it. Twelve distinct undersized sizes.

**The fix is an expanded hit area, not a bigger button.** Growing every control
would change a layout designed at those sizes and push panels past their bounds;
a small precise control with a generous invisible margin is the usual answer and
the right one. **Visual weight is a design decision and target size is an
accessibility one, and they do not have to be the same number.** After expansion
the requirement is a uniform **1280px** — the design width, and no narrower.

The cost of that is neighbours: two buttons eight pixels apart, each grown to
forty-four, now overlap, and a press landing on the wrong control is worse than
one landing on nothing. So overlaps are checked — and **finding none required
getting the question right**. Run against the layout-audit scene it reported
seven clashes, all between controls in *different panels* that the scene opens
simultaneously and the game never does. Touch targets are measured **one screen
at a time**; sizes want every panel, overlaps want one room.

Two numbers are reported because they answer different questions: the hit side is
what a finger can reach and expansion fixes it; the **drawn** side is what an eye
can find, and it does not. A control too small to see is a control nobody
presses, which is a separate fault from one too small to hit.

A fixed 1280-logical layout cannot meet the standard on a phone-width window at
all — that would need a responsive layout, which this is not.

### 5.46 A layout that follows the window (post-v1)

§5.45 ended by naming its own limit: a fixed 1280×720 layout cannot meet the
touch standard on a narrow window, because everything is shrunk to fit a shape
the screen does not have. This is that limit removed.

**Letterboxing was a decision nobody made.** On 16:9 it is invisible and correct.
On anything else it is bars, and on a phone held sideways — nearer 20:9 — it is
bars *and* controls a third the size they need to be.

**Fixed height, flexible width.** The height stays 720: it is what every panel's
vertical layout was written against, and a reel window has a natural height a
taller screen should not stretch. The **width follows the window**, so a wide
screen gets genuinely more room rather than a scaled-up copy, and a narrow one
gets less — which makes every control a larger fraction of it, which is the same
thing as being bigger.

Clamped at 4:3 and 21:9. Below the first the wager panel and the reels stop
fitting side by side; above the second they read as two separate screens. Outside
the clamp the letterbox returns, which is the right failure: bars beat a broken
layout.

**Extra width goes to the reels.** The wager panel is a column of controls and a
wider one is the same column with more space between the words; the reel window
is the part that benefits.

`LOGICAL_WIDTH` became a function reading a per-frame value — the same move
§5.43 made for the palette, and for the same reason: the alternative is a
parameter on every panel that would say the same thing every time. Overlays
centre through `frame::centred`, because an absolute `Rect::new(340.0, ...)` is a
centre offset with the centre baked in, and a constant is only the right way to
write one when the edges never move.

**And it found a real gap in §5.37.** At 1000 logical pixels the header buttons —
anchored 946 from the right edge — land exactly where the cabinet name is drawn,
and the capture showed the title simply gone. **The layout audit called it
clean**, correctly by its own rule: the title never crossed its *region's* edge,
it collided with something else inside it. Overflow past a boundary and collision
between siblings are different faults, and only the first is checked.

The title is dropped when there is not room for it, which is a responsive
decision rather than a fix: the cabinet's name is also on the "How X plays"
button and at the top of the rules panel, so the header is the one place it can
be spent. Sibling collision remains unmeasured.

### 5.47 Collision — the other half of the layout audit (post-v1)

§5.46 ended on a debt it had just exposed. The layout audit checks text against
its **region's edge** and nothing else, so a title drawn straight through a
button was reported clean — correctly, by its own rule. Overflow past a boundary
and collision with a sibling are different questions, and only the first was
being asked.

Now both are. Every string records its footprint, every control records its own,
and anything landing on something it does not belong to is a finding.

**Three exclusions, and each one is a real distinction rather than a silencer.**

- Text lying **wholly inside** a control is that control's label. Text that only
  *partly* covers one is a collision — which is exactly the shape the header
  fault took.
- A `Decorative` stroke does not collide with the label it exists to make
  readable. The hoard meter draws its text four times in near-black behind
  itself; reporting that is reporting the fix as the fault, the same call §5.40
  made for contrast.
- **A panel hides what is under it.** A region that names its surface has painted
  one, so whatever was already recorded inside it is gone.

That last one is the whole of the first real run. Five findings, and every one
was an overlay's text "colliding" with the wager panel behind an opaque
surface — a panel the player cannot see at all. **Three iterations running, a new
detector's first output has described the detector rather than the game** (§5.39's
eleven, §5.40's seventeen-of-which-two, and now five of five). It is beginning to
look less like a coincidence than like the normal cost of measuring something new.

**And collisions are a one-screen question**, exactly as touch overlaps are
(§5.45). Run against the audit scene that opens twelve overlays at once it
reported nine, all between panels that are never open together. They are recorded
only when asked for, and the per-screen scenes are what ask.

**Then a probe found something real.** Restoring the header title at 1000 logical
pixels to check the detector still fired reported a *different* fault instead:
the footer hint running **446px² under the "Got it" button**. That is the second
thing visible in the narrow capture from §5.46 and the one that had not been
fixed, and no amount of looking at a 16:9 screenshot would ever have shown it.
The hint is fitted to the space before the button now.

Seven tests hold the rules, because every exclusion above is a place where a
future change could quietly switch the check off.

### 5.48 One command (post-v1)

Forty-seven systems have each added a check, and **every one of them found a real
defect the first time it ran**. None of that is worth anything if running them
depends on remembering they exist. This session has already skipped the
million-spin RTP gate except when something looked wrong, and the last iteration
took twelve capture invocations typed by hand.

`verify.ps1` is the list made executable: format, lint and tests in both repos,
the wasm build, then the audit matrix — six themes, three text sizes, the
pseudolocale, three aspect ratios, and the per-screen collision and touch checks.
`-Long` adds the million-spin RTP band and the 200,000-round conservation soak.
Twelve gates in about fifty seconds; fourteen in about two minutes.

The axes are varied **one at a time** rather than crossed. The full product is
over a hundred runs of something that has never failed on two axes at once, and a
gate nobody waits for is a gate nobody runs.

**Writing it was three PowerShell traps and one real bug.** The function was named
`Cargo`, and PowerShell resolves a function before an executable — so `& cargo`
called *itself* and the first run died of call-stack overflow. A parameter named
`$Args` shadows an automatic variable. And `$ErrorActionPreference = 'Stop'` turns
anything a native command writes to stderr into a thrown exception **before the
exit code is read**, so a clean clippy run reported as a failure. Cargo writes its
progress to stderr.

**Then the harness earned itself on its first complete run.** It checks the touch
audit at 1000 logical pixels as well as 1280, which nothing had ever done — and
found the expanded hit areas from §5.45 overlapping by **thousands of square
pixels** on the narrower screen. The controls sit closer together there, and
growing every one to forty-four made them ambiguous. *A press landing on the
wrong control is worse than one landing on nothing* was the stated rule from the
start, and it had only ever been checked at the design width.

So growth is now bounded by the neighbours: each side expands at most halfway to
whatever is beside it. A control with room takes the full standard; one in a
tight row takes what is going and stays unambiguous. The limits come from the
**previous** frame's controls, which is sound for the same reason the keyboard
focus ring is (§5.27) — the set is stable while the same panels are open.

Two consequences fell out of that, and both were the measurement rather than the
game. The first frame of a scene has no neighbours, so its numbers describe a
state the game is never in; the report waits until they are warm. And a control
under an opaque overlay cannot be pressed, so it cannot be ambiguous with
anything — the same occlusion rule §5.47 needed, which had been applied to
collisions and not to targets.

### 5.49 Whether an old save still loads (post-v1)

"Adding content must never strand a player" appears throughout this document —
§5.9 when achievements gained a field, §5.12 when the Wrath arrived, §5.13 when
the buy menu did. Every time the answer was `#[serde(default)]` and a note in the
comment.

That is the right mechanism and **nothing enforced it**. Six structures are
written to disk — the save, preferences, achievements, hints, the ledger and the
session limits — and between them they have gained a field in most iterations of
this game. Whether any of them could still read what an earlier build wrote was
never tested; it was *inferred from the presence of an attribute*.

**The invariant that generalises.** A list of historical shapes goes stale the
moment someone forgets to add one. The sharp version needs no list:

> A type that loads from `{}` loads from every earlier version of itself.

Every earlier version is a subset of the current fields, and a type that
tolerates *all* of them missing tolerates any of them missing. One line per
structure, and it cannot go stale, because it does not describe history — it
describes a property.

The converse is the same argument run forwards: a build must read what a *later*
build wrote, or a player who opens a newer version once loses everything by going
back. That means unknown fields are ignored, which serde does by default and
`deny_unknown_fields` would silently undo.

**`SaveData` is the one exclusion, and it has to earn it.** It is the structure
where a defaulted field would be a lie rather than a gap: a save whose balance
quietly defaults to zero has loaded successfully and taken the player's bankroll
with it, which is worse than refusing. So it keeps its explicit migration, and a
separate test holds that migration to the claim — a save carrying only the two
fields every version has ever had must still come back whole, with its money.

**And this one was disproved before it was believed.** All ten passed on the
first run, which this session has learned to distrust: three detectors running,
a clean first result has meant the detector rather than the code (§5.39, §5.40,
§5.47). Swapping one structure's `default` for `deny_unknown_fields` failed both
properties immediately and named the field — `missing field 'shared'` — so the
green is worth something.

Beyond loading, four cases where the value has to survive as well as parse: a bet
index past the end of a ladder that shrank, a preference index into a list that
changed (§5.38, §5.30), a ledger naming a cabinet that no longer exists (§5.41
renamed most of them), and a session cap stored as a value rather than an index,
which is why a changed list of offered caps cannot strand one.

### 5.50 Every screen, measured the same way (post-v1)

§5.37 built the layout audit and ended by naming what it did not cover: "the
gamble, the Vault Pick board and the respin round draw from session state rather
than a flag and are not yet in the scene." That was **twelve iterations ago**.
Three modal screens a player certainly sees had never been through the layout,
contrast, collision or touch checks, because adding a scene was always something
to do next time.

Three more scenes would have closed it and would have gone stale the same way.
The debt was never that three screens were missing — it was that **nothing said
they were**.

**So the list became the definition.** `Screen::ALL` is the single registry, and
`Game::any_overlay_open` is *derived* from it. A screen that holds the reels must
appear there, because that is now what holding the reels means, and appearing
there is what the audit enumerates. A new modal cannot be added without becoming
auditable; it would not hold the reels at all. Same move §5.49 made for saves:
replace a maintained list with a property that cannot be forgotten.

`Screen::reachable_by_flag` names the two kinds. Twelve are a flag the player
raises; three are **session state** — an open Vault Pick, a gamble in flight, a
respin round. That is exactly why those three were skipped: there is no boolean
to set, and reaching them means playing until the game deals one. The predicate
is the gate rather than a description of one, so the flag map cannot drift from
it, and `audit:<screen>` **asserts the screen actually opened** before measuring
anything — the scene dispatcher ends in a fallthrough, so asking for a scene that
does not exist would otherwise measure the base game and report it clean, which
is indistinguishable from the twelve iterations of silence.

**Six findings on the first run, and they were the fix rather than the fault.**
The Vault Pick and the Wrath board were the only two panels in the game that
never declared a surface, so the audit was measuring their text against the
jackpot ladder and the win line *behind* them. The Wrath banner even carried a
comment reading "fully opaque: at 0.96 the jackpot ladder underneath read
straight through" — the defect had been fixed in pixels and never told to the
audit. Declaring the surfaces cleared all six, and exposed four more: the coin
values were being judged against the dark window rather than the gold disc they
are printed on, which is a different surface and now says so.

**Then the sweep found what the ad-hoc scenes never had.** Measuring all fifteen
the same way failed four more screens, and the largest was not a layout fault at
all: six cabinets at 140px each wanted a **960px picker inside a 720px frame**.
The panel was clamped, the last rows were drawn past the bottom edge, and
**Tidepool could not be selected** — a cabinet present in the catalog and
unreachable in the game. A two-column grid fits all six, and the rule that the
picker must fit the screen is now a test rather than an assumption.

The other three were one bug with one cause: an overlay centred tall enough to
start *above* the header sliced the cabinet name through the middle of the
glyphs. `frame::BELOW_HEADER` is the line, `Frame::panel` clamps to it, and a
fourth panel that only showed the fault under a 40% translation was fixed by the
same clamp.

**And the detector needed the same treatment as the code.** Occlusion had been
decided by whether an element's *centre* fell inside the covering panel, which
gets both cases wrong: a title sliced clean through went unreported while its
centre stayed outside, and a label whose tail a panel hides — ordinary layering —
was reported as a collision. The sharp question is not how much is covered but
**which way the cut runs**. A panel edge landing inside a line of text severs
every glyph in it; a panel edge landing mid-label hides the tail and shows whole
letters up to the edge. The first is always wrong, the second is what overlays
are for.

One harness bug fell out of it: `DRAGONS_HOARD_PSEUDO` was read with
`env::var(...).is_ok()`, so clearing it by setting it to `""` turned the
pseudolocale **on** for every later run, and the findings that produced looked
like faults on innocent screens. An empty value now means off, and each audit
starts from a known state rather than inheriting the last one's knobs.

Fifteen screens now pass at English widths and under a 40% translation. That
crossed pair is the one exception to §5.48's one-axis-at-a-time rule, and it
earned the exception on its first run.

### 5.51 Hearing the sound without ears (post-v1)

This document has said three times, in three different iterations, that **nobody
has heard any of this**. §5.19 built a waveform panel and the mix was *looked*
at, which caught three effects quieter than a reel stop. What a plot cannot show
is timbre — "whether a triangle wave at 1568Hz is a pleasant chime or a nasty
one" — and that was written down as something only a listener could settle.

**It is not.** A chime sounds nasty when it carries partials that are not
harmonics of the note, and that is a measurable property of a waveform.

**Where the nastiness comes from.** `Wave::Square` and `Wave::Triangle` were
generated by comparing a phase against a threshold. That is the honest shape and
it carries infinite harmonics — `3f, 5f, 7f…`, falling off as `1/n` for a square
and `1/n²` for a triangle. Sampling cannot represent a partial above Nyquist and
does not drop it: it **reflects** it back to `sample_rate - p`, landing at a
frequency with no musical relation to the note or to anything else. That
inharmonic partial sitting under the note *is* what "it sounds nasty" means, and
it is worse the brighter the sound.

Measured, at the default 22,050Hz:

| Effect | Note | Worst reflection |
|---|---|---|
| **Click** — every button press | square, 1200Hz | 8850Hz, **21dB** under the note |
| **CoinLock** — up to fifteen a second in the Wrath | triangle, 2349Hz | 10305Hz, 28dB under |
| **SpinStart** | square, gliding to 420Hz | 10836Hz, 30dB under |

The button blip was the nastiest sound in the game, and it fires on every press.

**The fix is the oscillator, not the notes.** Retuning to avoid the folds would
have been a tuning constraint nobody could see in the code. Instead the series is
summed directly and stops at Nyquist: nothing is generated that cannot be
represented, so there is nothing to reflect. The classical partials of a square
and a triangle, `4/π · Σ sin(2πnφ)/n` and `8/π² · Σ (-1)^((n-1)/2)·sin(2πnφ)/n²`,
capped at the 129th where a square is already 42dB down. The cost is one `sin`
per partial per sample, for effects rendered once at startup.

**It fixed the music for free.** `render_track` is `render_waveform` with a
length, so the Arp and the Pluck — both squares — stopped aliasing without the
score being touched. Eight stems, none of which had ever been asked what they
were doing above Nyquist.

**The detector's first long list was describing itself.** The first version
rendered each *effect* and read the folded frequencies off an FFT. It reported
reflections **at the level of the note itself** on the four-note arpeggio — for
partials whose true amplitude is `1/49²`, about -68dB. Game effects are 50–600ms
transients, and an envelope decaying over a tenth of a second smears every bin
far wider than the partials being looked for. The measurement is now taken on a
**steady probe** at the same pitch and shape, where the spectrum means what it
says; the arithmetic is used only to decide which frequencies to look at.

Which leaves the question of whether the audit works at all, now that everything
passes. So a test hand-builds the naive phase-threshold oscillator this toolkit
used to have and requires the measurement to find its reflections — the detector
is disproved on a signal known to be dirty before its clean verdict is believed.

**And §5.19's own finding had never actually been fixed.** It found a win peaking
at half a reel stop, raised the gains by hand and moved on. A small win still
peaked at **0.25 against a reel merely stopping at 0.27**, with identical RMS —
the reward sounded exactly like the machinery. Nothing was watching, because
"louder" had never been written down as something to check. The sound set now has
a stated order — a button, a coin, the scatter cue, the routine mechanics, then
wins in the order they are worth — and seven assertions holding it.

What this still cannot say is whether the sound set is *good*. A tasteful
arpeggio and a tasteless one pass identically. But "nobody has heard it" is no
longer the same sentence as "nobody knows anything about it".

### 5.52 What the game looks like while it is moving (post-v1)

Two bugs in this game came from a player rather than from the tests, and §15
wrote down why and left it there:

> everything here is asserted about *state*, and both of these were about what is
> on screen at a moment when the state is **mid-flight**. The capture harness
> photographs settled frames, so it could not have caught them either.

That was recorded as a lesson and nothing was built for it. Fifty-one systems of
verification, and the one class of fault that had actually reached a player twice
was the one nothing watched.

**Both are frame-level properties, and both can be stated.** A reel that had
landed kept drawing the previous spin's symbols until the last reel settled, and
then the whole board snapped: *once a reel has landed, what it shows must not
change again*. The reels were too fast to read the art: *a reel drawing detailed
art must not travel more than half a symbol between frames*. Neither is
observable from one state — a landed reel changing is a comparison against an
earlier frame, and travel is a difference between two positions. That is why a
hundred state tests and a capture harness both missed them.

**The strip.** `capture::filmstrip` takes a shot every few frames, box-filters it
down and tiles them into one image. A spin becomes something you can look at
rather than infer, which turned out to matter within minutes.

**Three times the harness lied, and each was worth more than the gate.**

*The run was too short.* The first audit reported clean over 96 frames — of a
spin that takes 129. It never once watched a reel come to rest, so the invariant
that matters was never evaluated, and "clean" meant "unexercised". Nothing in the
verdict said so; it was visible in the filmstrip, where the reels were still
turning in the last tile. A run that has not seen a spin reach rest now **fails**,
because a check that ran zero times is not a pass.

*The last reel was unreachable.* With that fixed, five reels turned and only four
were ever seen settling. The spin resolves on the same frame the final reel
lands, so there is no frame with every reel stopped and a spinner still to ask —
and the final reel is exactly where the original snap was most visible. The
invariant now spans the resolution: what each reel showed as it landed is
compared against the board at rest.

*The picture ran backwards.* The strip showed settled reels first and blurred
ones last. `get_screen_data` returns the framebuffer in OpenGL's order, origin
bottom-left, and `export_png` flips it on the way out — so each tile came out
upright while the tile *rows* came out reversed. It took single frames captured
at known counts to establish that the game was right and the picture was wrong.
A visualisation that lies is worse than none, and this one lied convincingly.

**Then the proof.** Everything passing means nothing on its own, so the fix for
the original bug was reverted — `display_grid` put back to returning the stale
board — and the audit run again. It named it exactly:

```
frame 129: reel 0 had settled showing [0, 1, 2] and now shows [0, 4, 6]
frame 129: reel 1 had settled showing [1, 0, 3] and now shows [5, 4, 1]
frame 129: reel 2 had settled showing [2, 0, 1] and now shows [5, 2, 4]
frame 129: reel 3 had settled showing [3, 0, 2] and now shows [1, 0, 3]
```

Frame 129 is the moment the last reel lands. Four already-settled reels changing
at once *is* "the whole board snapped", and it is now something the harness finds
before a player does.

Six cabinets pass, one gate in `verify.ps1`, and the game was independently
confirmed correct in real play while this was being written.

### 5.53 Running out (post-v1)

Twenty paylines at the cheapest stake is twenty credits a spin against a thousand
to start with. **Fifty spins**, at a measured 96% return with a slot machine's
variance behind it. Going broke is not an edge case in this game — it is the
ordinary way a session ends, and it is the single most likely thing that can
happen to anyone who plays.

The whole of the game's answer was a notification:

> Not enough credits — lower the bet or start a new game

Lowering the bet buys a few more spins. Starting a new game throws away the
session, the hoard meter, the graph, and everything collected on the way to a
hatch. Fifty-two systems of features and verification, and the most common route
to the end of play was a dead end with a reset button at the bottom of it.

**What a broke player still owns is the hoard.** Eggs collected toward a hatch,
and a pot banked behind them — real value they earned, worth nothing while they
cannot spin. So the first lifeline is **breaking the hoard**: take the banked pot
now, at a cut, and lose the eggs.

That is a decision rather than a rescue. The pot is what the hatch would have
paid; taking it early costs the salvage rate and every egg on the meter, so a
player near a full hoard should hold on and a player who has just hatched has
nothing to break. It is offered **only when it buys a spin** — a pot of forty at
half salvage is twenty credits against a twenty-credit spin, and taking that
leaves the player just as stuck with their eggs gone as well.

**And when there is nothing left to break, the vault stakes them.** This is play
money and refusing to let someone keep playing is worse than the alternative. But
a stipend that appeared from nowhere and was never mentioned again would quietly
make every figure in the session a lie, so a stake is counted and the count is
shown on the figures row the moment there is one.

**Where the money comes from, for the books.** §5.33 asserts
`opening + won - wagered == balance` after every round, and both lifelines move
the balance without a spin. Breaking the hoard is *not* new money — the pot is
funded by eggs at the line bet and would have been paid at the hatch — so it is
counted as winnings and the identity is untouched. A vault stake genuinely is new
money: it gets its own term, and it is the only credit in this game that nobody
won.

That distinction had never been checked, because the soak floats the balance
before every spin and has therefore **never once gone broke**. A session played
to actual ruin and rescued repeatedly now asserts
`opening + won + staked - wagered == balance`, and the test guards itself — if it
completes without going broke, it fails rather than passing vacuously.

**Two things the screen refuses to do.** It has no close button, because
dismissing it would leave the player looking at a cabinet that ignores the spin
button. And it names lowering the bet as an option beside its own offer, because
that is frequently the better move and a game that only showed the door it
profits from would be a different kind of game to this one.

**The offer waits for the card.** `is_ruined` requires a settled session, so a
rescue is never dealt over the top of a Hatch or a jackpot — the game does not
interrupt its own good news to tell someone they are broke. That fell out of a
test that would not go broke, and turned out to be the right behaviour rather
than an obstacle to it.

**And the harness had already grown the drift §5.50 exists to stop.** The
per-screen sweep in `verify.ps1` kept its own copy of the screen list, so by the
time the ruin screen was registered, tested, reachable and auditable, the sweep
still did not know it existed. The registry is now *asked for* rather than
repeated: the game prints `Screen::ALL` and the harness reads it. A list in a
second language is a list that goes stale, one file over from the module written
to make that impossible.

### 5.54 The limit nothing was checking, and the validator nobody had seen refuse anything (post-v1)

Two findings from the same look at the code, and both are the same shape: a rule
written down everywhere and enforced nowhere.

**The 800-line limit.** It is the oldest constraint in this workspace, stated in
`AGENTS.md` and `CODE_STANDARDS.md` and duplicated verbatim into every game here.
There is even a script for it in `rust_management`. Nothing runs it — and the
shared toolkit, the crate twenty games depend on, was sitting at **851 lines** in
`ui/font.rs` while this game's largest file was **one line under** the limit.
Adding §5.53's two config fields had taken `data.rs` to 799.

So the check joins `verify.ps1`, and then had to be disproved like everything
else: a 900-line probe file dropped into the toolkit made it fail and name the
file, which is the only evidence that a passing run means anything.

**Three splits, and each seam was already confessed in a comment.**

`ui/font.rs` opened with "UI font loading, text measurement/layout/drawing, **and
value formatting**" — two responsibilities joined only by both being vaguely
about text. Nothing in the formatting half touches a font, a glyph or macroquad:
a money string is the same string whether it is drawn, logged, or asserted on in
a headless test, which is exactly why those are the functions a test reaches for.
851 → 656, with `ui/format.rs` at 212.

`data.rs` split along a seam that matters beyond the line count. What stayed
describes *what a cabinet is* and is read every frame; what moved to `data/load.rs`
is the one-off act of **building one and proving it makes sense**, and runs once
per cabinet switch. 799 → 531.

`music.rs` split the composition from the performance — the key, the timing and
the notes are pure data rendered once, while the stems, the mood gains and the
crossfade are stateful and touched every frame. 793 → 680.

**And moving the validation into its own file made it obvious that nobody had
ever watched it work.** `GameData::validate` is two hundred lines and thirty-odd
rejections standing between a typo in a JSON file and a panic on the first spin.
Every test that had ever run it handed it the **shipped** data, which is valid.
So as far as this suite was concerned it was indistinguishable from `Ok(())`, and
would have stayed that way if someone deleted its body.

Thirteen mutations now take real cabinet data and break one thing each — no
reels, a free line bet, a hoard that never fills, a bonus that is all blanks, a
respin round that ends at once, a gamble nobody can take — and require a refusal
that **says which**. The message is compared, not just the `Err`, because a
validator that rejects everything with the same words is barely better than one
that rejects nothing: the message is what tells whoever edited the file what they
did.

Disproved the same way: stubbing `validate` to return `Ok(())` fails on the first
case. And a control test requires the pristine data to pass, without which a
validator that rejected *everything* would sail through all thirteen.

### 5.55 One bankroll, six cabinets — and the harness that was eating the save (post-v1)

**§5.53 was sitting on a trapdoor.** Every cabinet had its own save slot with a
balance inside it, so walking from Dragon's Hoard to Frost Wyrm handed the player
a **fresh thousand credits**. Six cabinets, six starting stacks, and a New Game
button under each of them.

That made the previous iteration meaningless. Running out is supposed to be a
real decision — break the hoard at half its value and lose every egg on the
meter, or take a stake the game counts against you — and neither costs anything
at all if a full stack is one click away in the machine picker. It also made the
Ledger's per-cabinet returns describe six unrelated economies rather than one
player's session.

**Money belongs to the player, not the machine**, which is how a real floor
works: you carry your money between machines and the machines keep their own
jackpots. So the balance moved into a wallet of its own and switching carries it
across untouched. Everything else stayed, because everything else genuinely does
belong to the cabinet — a hoard is *that* cabinet's eggs at *that* cabinet's line
bet, a progressive is funded by play on it, and the Ledger is per-cabinet
precisely so they can be compared. The cabinet stops being a separate game you
start over and becomes a different table to take your money to, which is what the
picker always claimed it was. The panel's own words had to change with it: "each
machine keeps its own balance" is simply no longer true.

Money already saved is **absorbed and added together** — not the largest, not the
current one, all six. Whatever economy the player was in, that money was theirs,
and a migration that quietly deleted five sixths of it would be the worst bug
this game could ship. A cabinet saved at zero is absorbed as a real zero rather
than read as "no save", or a player who went broke and reloaded would be handed a
new stack.

**And then the capture scene wrote it all to disk.**

The screenshot harness deals winning boards, fast-forwards into features, empties
a balance to reach the ruin screen, and now walks between cabinets. The game loop
autosaves whenever a spin resolves. Nobody had ever connected those two facts, so
**every capture scene had been writing its fabricated state straight into the
real save slots** — and `verify.ps1` runs about fifty captures.

Running the verification suite destroyed the player's game. Every time. It had
been true since the harness was written, and nothing said so, because nothing in
a capture ever reads a save back and notices it is wrong. The tell was there all
along: `Game::new` already asks `capture_requested` before opening an audio
device, because a headless run has no sound card. Nobody asked the same question
about the disk.

The first fix guarded the two writers that were obvious and the ledger kept
recording fabricated rounds — six things persist through three different toolkit
functions. So the question is asked **once**, and every writer consults it.

The gate is a fingerprint of the save directory taken either side of five capture
scenes, and it was disproved the same way as everything else here: removing the
guard makes it fail and name `ledger.json`.

### 5.56 Every game on the site was saving into one drawer (post-v1)

§5.55 went looking for the next thing a cabinet switch quietly reset. It found
something bigger, one layer down, and not in this game at all.

**The toolkit stored web saves under a key with no game in it.** The native path
writes `{app_data}/{game_name}/save_{slot}.json`, so two games can both have an
"autosave" and never meet — the directory does the qualifying for free. The web
path built `save_{slot}` and discarded the game name outright, with a comment
saying so: `let _ = game_name; // unused in WASM`.

`localStorage` is scoped to an **origin**, not a page. Every game in this
workspace is published to the same host.

That is not a hypothetical collision. Across the catalog **three games ship with
`save_slot: "autosave"`** — `biofoundry`, `dragons_den`, `iron_fauna` — and two
more with `"campaign"`. On the web they were all writing the same key. Playing
one overwrote another's save, silently, with no error and nothing in either game
able to notice: a save that loads is a save that loads, whoever wrote it.

**The tell was inside the toolkit.** `persistence::keys`, which stores
preferences and the like, had always qualified its keys by game. Only
`persistence::slots` — the module holding the actual game — did not. Two
sibling modules, one storage backend, and a disagreement about whether the game
name mattered.

Dragon's Hoard escaped by accident: its slots carry a machine id, so
`dragon_autosave` was unlikely to collide. Except for `save_wallet`, added one
iteration earlier, which is exactly the kind of generic name that would have.

**Fixed by qualifying the key, and by adopting what is already there.** A key
that changes is a save that vanishes, so a read that misses the qualified key
falls back to the legacy one and the next write moves it across (§5.49's rule:
adding content must never strand a player). Two players of different games stop
colliding; a player mid-game keeps their game.

The four tests run on **every** target, not just `wasm32`. The rule they encode
is the one that lost saves, and a rule that can only be checked by opening a
browser is a rule nobody checks — which is how it survived this long. Reverting
the key to its old form fails two of them by name.

This one is worth more outside this game than inside it: twenty games share that
crate, and five of them were demonstrably colliding.

### 5.57 The Grand belongs to the floor (post-v1)

**Six Grands nobody was ever going to fill.** The top tier sits behind odds of one
win per 12.5 million credits of turnover — at twenty credits a spin, **625,000
spins** — and every cabinet had its own, fed only by play on that one machine and
reset to seed whenever it paid. Six pots that would realistically never be seen,
and a headline number on the reel window that was decoration.

§5.55 had just made the money one bankroll across all six. The player is one
player on a floor of machines; the biggest prize on that floor should be the
floor's.

**The change is where the pot lives, not what it costs.** The Grand is not a new
tier and none of its maths moves: same seed, same share of the contribution, same
odds. Only its **accrual** moves out of the per-cabinet save into a store of its
own, so every spin on every cabinet feeds one pot and any cabinet can drop it.

That distinction is what keeps §4's numbers honest. A single-cabinet simulation
contributes to the shared pot and wins it back at exactly the rate it always did,
so measured RTP is untouched — the million-spin gate passed on all six unchanged,
which is the evidence that this is a plumbing change and not a payout change.
What differs is the *real* game, where six cabinets fill one pot six times as
fast. That is what a linked jackpot is for, and why real floors run them.

**Keyed by tier id, not by index.** Cabinets are free to declare different
ladders and one of them eventually will; a shared pot addressed by position would
hand one cabinet's third tier the money another cabinet's third tier had banked.
A test also requires every cabinet to agree on *which* tier is the floor's, or a
player walking between them would watch the pot appear and vanish.

**Marked rather than labelled.** The plate is barely a hundred pixels wide, so the
shared tier gets an ember border and a heavier rule instead of a second word that
would not survive 130% text, let alone a translation. The explanation goes where
explanations go — the generated rules (§5.29).

**And the rules panel refused it.** Adding the sentence as a topic of its own
pushed Frost Wyrm into a third column and failed two of §5.29's gates by name;
folding it into the Progressives paragraph still failed. The panel was simply
full. It is 1,180px wide now rather than 1,040 — the width every other overlay in
the game already uses — because the alternative was shrinking the type into the
readability floor. A layout gate that says "no" to real content and cannot be
argued with is doing precisely its job.

### 5.58 The game had never once read its own save (post-v1)

§5.57 made the Grand grow across all six cabinets. Which raised the question of
the other three pots — and the answer was that they had been resetting to their
seeds **every time the game started**, since it shipped.

`Game::new` built a fresh `GameSession` and never looked at the disk. Not once.
The autosave had been firing after every resolved spin for fifty-odd iterations,
writing a perfectly good file that nothing ever opened. Every launch reset:

- the **hoard meter** to zero eggs, out of the fifteen a hatch needs;
- the **Mini, Minor and Major** pots to their seeds, so three of four
  progressives could not grow past one sitting;
- the **session stats** — spins, biggest win, hatches.

The balance survived only because §5.55 had accidentally rescued it two
iterations earlier by moving it into a wallet of its own. Before that, closing
the game lost everything.

§9 of this document describes the opposite behaviour in detail — "autosave after
each spin resolves… a reload lands back in the base game" — which reads as a
promise that a reload restores your game. There was a Load button, and pressing
it worked. Nobody pressed it, because nothing suggested they had to.

**Why nothing caught it.** Writing a save nobody reads looks exactly like writing
one that is read. The save file was correct. The autosave test passed. §5.49
checked at length that an old save still *loads* — thirteen historical shapes,
every persisted type, a corpus of legacy formats — and never once asked whether
anything **called** the loader. Every test in this project was on one side of
that seam or the other; none crossed it.

**The fix is that boot and switching cabinets are the same act.** You sit down at
a machine and it is how you left it. `load_machine_session` already did exactly
this for the machine picker; `Game::new` now calls it too, so there is one path
for arriving at a cabinet instead of two that disagreed. A save that exists and
refuses to load now says so out loud rather than silently starting over — a
player's hoard going quiet deserves a sentence.

**Proving it needed a different kind of test.** The boot path wants a GL context,
so the round-trip property went in `state::compat` — everything a player
accumulates survives a write and a read, with guards that fail if the played
session produced nothing to check. But those tests would have passed *before* the
fix, because the fault was never in the round trip.

So the real check plants a save with eleven eggs on the meter, starts the game,
and reads back what it found. Reverting the two lines makes it report zero. It
restores the file afterwards, because a gate that eats the save it is verifying
would be a poor joke one iteration after §5.55.

### 5.59 Showing the line that won (post-v1)

This one came from sitting and looking at the game rather than reading it. Under
the reels, after a win:

> Ruby ×3 on line 17

**Line 17 is a number.** Twenty paylines cross the grid in twenty different
shapes, the winning *cells* were lit — which says which symbols paid — and
nothing anywhere said what path they were on. A player could not have told you
what line 17 is, and the game never once drew it.

The data had known all along. Every payline in `paylines.json` carries a **name**
— "Middle", "Top", "Mid Trough" — and the game was printing an index instead. A
slot machine that cannot show you its lines is asking to be taken on trust, which
is the one thing this cabinet has spent fifty-odd systems refusing to do.

**The winning run *is* the line.** A `Win` already carries the cells that formed
it, in reel order, because §5.14 made ways and cluster wins report their own
cells rather than being looked up. So the bright path needs no payline lookup at
all: it is a polyline through the cells that paid, correct by construction, and
it works unchanged whatever the cabinet's win model is.

The rest of the line is drawn dim behind it from the payline definition. A
three-of-five win stops at reel three, and showing where the line *would* have
continued is what turns a fragment into a shape — the capture that settled the
design shows a bright run through three rubies and a faint trough continuing past
them, which is exactly why the line is called Mid Trough.

**One at a time.** Six simultaneous line wins drawn together are a cat's cradle,
and drawing only the first would be worse — a player would see one line beside a
payout that did not match it. They take turns on a beat, with a test that every
win gets one: two full cycles, sampled inside each beat rather than on its edge,
and all three lines must appear.

**And the sentence had to change with it.** "Ruby ×3 on **Mid Trough**" now, from
the name that was in the data the whole time. A test used to require the win text
to quote the payline's *id*, because the index would have been off by one against
the paytable; it now requires the text and the drawn line to name the same thing,
which is the same guard pointed at the thing that can now actually go wrong.

### 5.60 The twenty lines, on a page (post-v1)

§5.59 draws the winning line and names it, which answers the question **after** it
has been asked. A player who wants to know what the twenty lines are — before one
pays, or to work out why a near miss was not a win — still had nowhere to look.

The paytable is the obvious place and does not have the room. It ends with about
a hundred spare pixels; twenty diagrams need five times that. So the lines get a
screen, and that turned out to be the right shape for them rather than a
consolation: five across and four down, each a miniature of the grid with the
line's own path on it, drawn **in the colour the reels will use**. That last part
is the whole point — a blue zig-zag called Mid Trough here is the same blue
zig-zag that appears over the grid when it pays, because both come from one
function. Two palettes would have made this a decoration instead of a reference.

The names turn out to be worth showing on their own: Dive, Climb, Low Cradle,
High Cradle, Arch Down, Zig Up, Top Dip, Bottom Bump, Mid Trough, Wide Weave.
Twenty shapes that a player can now recognise, all of which have been sitting in
`paylines.json` since the game shipped.

**Registering it was the cheap part, and that is the result.** Adding `Screen::Lines`
to the registry (§5.50) is what put the new panel through the layout, contrast,
collision and touch audits, at three text sizes, six themes, three window widths
and a 40% translation — without a line of harness being touched, because
`verify.ps1` asks the game for its screen list rather than keeping one (§5.53). A
seventeenth screen appeared and seventeen were swept. That is the whole argument
for the registry, finally paid out.

**And the footer stopped being ambiguous.** It read "Every payout above is a
multiple of the line bet, at the 10 line bet you have set" — which sounds like the
numbers in the table are already at that bet. They are multipliers. It now works
the sum: "at your 10 line bet, an x10 pays 100". A cabinet that shows its
arithmetic is harder to mistrust than one that asserts it.

A cabinet with no paylines gets a sentence saying so rather than an empty panel,
because three of the six pay by ways or clusters and a blank screen reads as a
bug.


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
current bet ladder. **Starting the game loads the current cabinet's slot**, by
the same call the machine picker makes — this section described that from the
beginning and the code did not do it until §5.58. Autosave after each spin
resolves — **but not mid-feature**:
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
- **Hints (`state/hints.rs`):** the shipped set validates, and a hint that could
  never appear, one that retires before it appears, or a duplicate id is
  rejected; **a fresh player is told nothing**, since every hint waits for them
  to have done something; a hint appears once its condition is met and never
  returns once dismissed; **acting on a hint retires it without dismissing**,
  which is what stops a player being told about something they already found;
  only one shows at a time and it is the first in file order; and dismissals
  round-trip.
- **Keyboard navigation (`ui/nav.rs`):** focus starts on the first control and
  **exactly one is ever focused**; it wraps in both directions; activating fires
  the focused control and nothing else; **a disabled control is skipped
  entirely**; focus resets when a panel opens or closes; and a frame with no
  controls at all leaves it somewhere valid rather than out of range.
- **The rasteriser (`macroquad-toolkit/src/paint.rs`):** a filled rectangle
  covers exactly its area, a triangle about half its bounding box, a circle
  π/4 of one; **winding order does not matter**, since art is rarely consistent
  about it and a rasteriser that cared would silently drop half the facets; alpha
  blends rather than replaces; drawing outside the buffer is ignored rather than
  panicking; **the same drawing fingerprints the same and a changed one does
  not**; an empty buffer still has a fingerprint; and silhouette difference
  ignores colour where monochrome difference does not.
- **Art legibility (`ui/symbols/legible.rs`):** every symbol draws something at
  the smallest cell the game produces and does not fill it edge to edge; **no
  two symbols look alike in monochrome at that size**; the three gem cuts are
  distinct in *silhouette*, which is the §5.24 claim stated as a property; and
  art scales rather than shrinking into a corner, which would catch a routine
  written in absolute units by mistake.
- **Colour legibility (`ui/legibility.rs`):** **no two symbols sharing a shape
  are too close under any vision** — the check that found Frost Wyrm's glacier
  and amethyst 0.061 apart under deuteranopia; the simulation leaves greys
  untouched, since a device that tinted them would skew every measurement it
  made; and red and green visibly collapse for a deuteranope, which is the
  sanity check on the instrument itself.
- **Strip motion (`macroquad-toolkit/src/strip.rs`):** a strip lands exactly on
  its target from every index and from a ragged frame rate; **every strip travels
  a whole number of revolutions**; the bounce never changes where it stops; a
  strip that does not move still turns a full revolution; a held strip takes
  longer and still lands right; strips report stopping once each in order;
  **no time scale drops a stop**; speed falls to nothing as a strip settles; and
  the blur offsets straddle the position and sum to nothing.
- **Buy-tier profiles (`state/profile.rs`):** every tier on every cabinet
  profiles near its price, since the tier profiler and `simulate_buys` are two
  loops over the same purchase; **a fairly priced buy still loses most of the
  time**, which is the figure the panel exists to show; the bands account for
  every buy; each tier is measured on its own stream; and profiling a tier does
  not touch the player's session.
- **Refining free spins (`state/tests/refine.rs`):** the burn deepens one symbol
  per free spin and stops at the length of the order; **a refined strip really
  loses the symbol**, or the feature would be N ordinary spins with a longer
  banner; a strip never burns down to nothing; nothing is burned outside the
  feature; **a retrigger does not take the reels back**; a cabinet without a
  refine order is untouched, so the other four behave exactly as they did; and
  refining never burns the wild, the scatter or the egg.
- **Synthesis (`macroquad-toolkit/src/synth.rs`):** the container is a
  well-formed WAV and its declared sizes match the real payload; synthesis is
  deterministic; **a different seed changes only the noise**, so one effect can
  be tuned without disturbing the rest; nothing clips; an envelope opens and
  decays to silence; a glide is geometric; an effect is as long as its last
  voice; and an empty effect still renders a file a player would accept.
- **This game's sound set (`audio.rs`):** every effect's length and checksum is
  **pinned against a baseline**, so a change to the mix has to be a decision
  rather than a side effect — it is what proved the move into the toolkit altered
  nothing, and what caught the three deliberate changes afterwards; the baseline
  covers every effect, so a new one cannot slip past it; every effect is audible
  and none clip; every effect decays to silence, since one ending mid-tone would
  click on every play; and **`CoinLock` is short enough to repeat**.
- **The Ledger (`state/ledger.rs`, `state/tests/ledger.rs`):** a fresh ledger
  knows nothing; recording accumulates rounds, stake, return, hits, features and
  the best round; each cabinet keeps its own record; **a round with no stake is
  ignored**, so a stray free spin cannot be counted as a round the player never
  paid for; the bands account for every round; the margin shrinks as the sample
  grows and is meaningless at one round; and the whole thing round-trips through
  its own key. In a live session: a round opens on a paid spin and closes on the
  next; **a free spin never opens a round of its own**; a round's credits include
  the free spins it bought; every closed round carries the stake that paid for
  it; a ledger built from real play matches the rounds it saw; and a Feature Buy
  does not leave a half-formed round behind.
- **Machine profiles (`state/profile.rs`):** every cabinet profiles to a
  plausible hit frequency and volatility; the bands account for every round; the
  profiler and the batch sim agree, since they are two loops over the same
  engine; **profiling does not touch the player's session** — two identical
  sessions stay in lockstep across a running profiler; the same cabinet profiles
  identically twice, because a figure that wobbled between viewings would read as
  a fault; and the catalog spans more than one volatility, or the picker would be
  advertising a choice that does not exist.
- **The RNG's one dead seed (`macroquad-toolkit/src/rng.rs`):** the seed that
  zeroes the xorshift state still generates; no seed in a swept range produces a
  dead stream; and **every ordinary seed's first draw is unchanged** by the
  guard, so no existing game's determinism moved.
- **The Dragon's Gamble (`state/gamble.rs`, `engine/sim.rs`, `state/tests/gamble.rs`):**
  a right guess doubles and a wrong one ends the round; **the scale is fair** over
  200,000 flips and **a gamble returns what it risks** when every round is pushed
  to the end of the ladder; half-gambling is fair too and banks half out of reach;
  the odd credit goes to the player; the ladder stops at its cap and a stake over
  the ceiling cannot be gambled again; a finished round cannot be gambled and
  cannot be taken twice; half is refused where the machine forbids it; **a flip
  consumes the same randomness whether it wins or loses**; and one seed replays
  one round. In the sim: **gambling everything measures the same RTP as gambling
  nothing** — the claim the whole feature rests on. End to end: staking takes the
  win back out of the balance; taking without flipping returns exactly the win; a
  won flip doubles what reaches the balance and a lost one leaves nothing and
  closes the round; nothing is offered without a win, during free spins, or during
  an autospin run; an open gamble holds the reels and refuses a spin without
  taking a stake; and a gamble is never written to the save.
- **Cascades (`engine/cascade.rs`, `state/tests/cascade.rs`):** a chain always
  has at least the landing grid; it **ends on a grid that cleared nothing** (one
  that stopped mid-collapse would leave holes on screen); every step but the last
  actually paid; the multiplier follows the ladder and climbs; **a chain is a
  function of the stops alone** — the invariant the design rests on; survivors
  fall and keep their order; a refill comes from the strip above the stop; a
  chain cannot run forever; and a step pays its wins times its multiplier. End to
  end: a chain is revealed grid by grid and **nothing is credited until it
  finishes**; the board comes to rest on the last grid; the animated and headless
  paths agree on a cascading machine; the credit is the sum of the steps; a
  non-cascading cabinet still produces exactly one step so consumers never
  branch; and a spin mid-chain never reads as settled.
- **Ways evaluation (`engine/evaluate/ways.rs`):** one of each across three reels
  is a single way; ways multiply across reels and the payout multiplies with
  them; a run must start on reel 1; **several symbols pay at once** (the mechanic
  a payline machine cannot express); wilds substitute and the genuine symbol
  still pays; **an all-wild run pays only as the wild** and nothing else; a
  wild-led run with real symbols behind it pays both; wilds never substitute for
  the scatter; a full grid of one symbol pays all 243; and every cell a win
  reports really holds that symbol or a wild, since those cells drive the
  highlight.
- **Two win models in the catalog (`state/tests/machines.rs`):** the catalog
  contains both a lines machine and a ways machine — otherwise "multiple
  machines" is one game tuned twice — and each declares exactly what its model
  needs: paylines and one bet unit per line, or no paylines and explicit
  `bet_units`.
- **The Feature Buy (`state/featurebuy.rs`, `engine/sim.rs`, `state/tests/featurebuy.rs`):**
  the shipped menu validates and a **free tier is rejected** (it would return
  infinite RTP and make the reels pointless), as are duplicate ids and a Wrath
  tier that opens every cell; price scales with the stake; an opening board
  spreads its coins and never repeats a cell. In the sim: **every tier returns
  its machine's RTP** — the assertion the whole design rests on — at a coarse
  band in CI and 200,000 buys per tier in the ignored run, which prints the fair
  price for any tier that has drifted; a buy feeds every pot without rolling for
  one; the price charged is the price the menu quoted. End to end: bought free
  spins run at the stake that paid for them and **still cost nothing to play**;
  a bought Wrath opens with its coins locked and room left to respin; a buy with
  too little credit, during a feature, mid-spin, or for a tier that is not on the
  menu takes nothing; the price counts as turnover; and `can_buy` agrees with
  `buy_feature` across every tier at four balances.
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
| A bought feature priced away from its value | `feature_buy_prices_are_exact` buys every tier 200,000 times and asserts the return matches the machine (§5.13). Mispricing downward makes never spinning the optimal strategy, and nothing else in the suite would notice. |
| A new evaluation model quietly breaking the old one | `evaluation` defaults to `lines`, so existing data needed no edit, and both models are asserted present in the catalog (§5.14). Wins carry their own cells, so no consumer branches on the model. |
| A reveal that consumes randomness | A cascade refills from each reel's own strip rather than rolling (§5.15), so the chain is a function of the stops. Tested by running the animated and headless paths from one seed on the cascading cabinet. |
| A gamble quietly shaved | The scale is asserted fair over 200,000 flips, and a whole simulation that gambles every win is compared against one that gambles none (§5.16). A shaved coin would look like ordinary RTP drift in any single-number band. |
| Quoting a number a sample cannot support | The live profile shows hit frequency, volatility and a band bar, and deliberately **not** RTP — 20,000 rounds put Dragon's Hoard 5 points out (§5.17). A wrong figure is worse than no figure. |
| A player reading variance as a rigged machine | The Ledger shows their sample against the measured cabinet *and* the margin of error on it (§5.18). Showing the two bars without the caveat would have been worse than showing neither. |
| A refactor silently changing every sound | Length and checksum of all eight effects are pinned (§5.19). The move into the toolkit was proved byte-identical; the three later changes were deliberate and re-baselined. |
| Duplicated grid index arithmetic | `reel * rows + row` lived at eight call sites until §5.20; `Grid::index` owns it now. Reels that differ in height would have silently read the wrong cells at every one of them. |
| A feature tuned by gutting the base game | Refining took Frost to 6.92; the fix was rebalancing the feature's own spins and multiplier, not a 36% paytable cut that would have paid for the feature out of the base game (§5.21). |
| Symbols that only differ by colour | Any two sharing a shape must stay apart under three simulated dichromacies (§5.24). The three gems now have three cuts, so the check has nothing left to catch. |
| Art verified only by someone looking at it | The symbol routines rasterise to a buffer in a unit test (§5.25). It disproved §5.24's own screenshot-backed claim on its first run. |
| Art changed by accident | All nine routines are fingerprinted (§5.26). A shared helper nudged for one shape moves four others, and nothing before this could have said so. |
| A panel reachable only with a mouse | Every control registers with `Nav` (§5.27). The Vault Pick holds the game until a chest is picked, so a mouse-only board was a soft-lock rather than an inconvenience. |
| Systems no player can find | Hints surface a feature once the player's own counters say they are ready for it, and retire when acted on (§5.28). The alternative was a tutorial nobody reads for a game that grows every iteration. |
| Lines you can only learn by winning on them | All twenty are drawn on a screen of their own, in the colours the reels use (§5.60). Registering it put it through every audit with no harness change. |
| A win the player cannot see the shape of | The winning payline is drawn across the grid and named from the data (§5.59). The game said "line 17" for fifty-eight systems and never drew a line. |
| An autosave nobody reads | Booting loads the cabinet through the same path as switching to it, checked by planting a save and reading it back (§5.58). The game had autosaved since it shipped and never once opened the file. |
| A jackpot nobody could ever win | The Grand is shared by every cabinet, fed by all of them and payable on any (§5.57). Six independent pots at 625,000 spins each were decoration. |
| Two games sharing a browser save | Web storage keys are qualified by game, with the old key adopted forward (§5.56). Five games in the catalog were writing the same `localStorage` key. |
| A cabinet switch that refills the wallet | One bankroll travels with the player; hoards and jackpots stay with the machine (§5.55). Six separate balances made running out cost nothing. |
| A test harness that overwrites the game | Capture runs are read-only, checked by fingerprinting the save directory (§5.55). Every capture scene had been autosaving fabricated state over the player's save since the harness was written. |
| A rule written down and never checked | The 800-line limit is a gate in `verify.ps1`, and the data validator has thirteen mutations proving it refuses things (§5.54). The toolkit was already over the limit; the validator had never once been seen to reject anything. |
| A player who runs out of credits | Breaking the hoard, or a stake from the vault, with the cost stated (§5.53). Going broke is the most likely end of a session and the answer was a reset button. |
| A fault that only exists mid-animation | Every cabinet's spin is watched frame by frame and tiled into a filmstrip (§5.52). Both bugs a player reported were of this kind, and the harness photographed only settled frames. |
| A sound nobody has heard | Every effect and music stem is measured for level, offset, clicks and inharmonic partials (§5.51). Timbre was written off as needing ears; the reflections that make a chime nasty are arithmetic. |
| A screen nobody ever measured | Every screen that holds the reels is in `Screen::ALL`, and `any_overlay_open` is derived from it (§5.50). Three screens went twelve iterations unaudited because the list of them was a note rather than the definition. |
| A save that a later build cannot read | Every persisted type must load from `{}` and ignore fields it has never heard of (§5.49). The rule was stated a dozen times and enforced by an attribute nobody checked. |
| A gate that nobody remembers to run | `verify.ps1` runs all fourteen in one command (§5.48). Its first complete run found hit areas overlapping at a width nothing had ever been checked at. |
| Text drawn straight through a button | Every string and control records its footprint and collisions are reported, with labels, strokes and occluded panels excluded (§5.47). Overflow and collision are different questions. |
| Bars on every screen that is not 16:9 | The logical width follows the window between 4:3 and 21:9, with the height fixed and overlays centred per frame (§5.46). Extra width goes to the reels. |
| A control that is dead on a phone | Mouse and touch became one `Pointer` at the single seam every control already passed through (§5.45), and hit areas grow to the 44px standard while the drawn size stays. |
| Six cabinets that sound like one cabinet | Eight shared stems rendered once, with a per-cabinet mix (§5.44). Async loading rules out re-rendering on a machine switch, so cabinets differ in instrumentation rather than in key. |
| Recolouring an interface by hand | 298 palette uses became theme-backed accessors, and every theme is run through the contrast gate and the layout audit (§5.43). A gate is permission to change, not only a check. |
| Six cabinets that look like one cabinet | Each set keeps the shared low-tier shapes and gets its own premiums and palette (§5.42), with the dichromacy gate forcing an accent rather than a single hue. |
| Retheming a cabinet meaning editing a copy | A symbol's identity is a shared set and its payouts stay with the cabinet (§5.41). Four cabinets carried byte-identical definitions before the split. |
| Text nobody can read off its background | Contrast is measured against the declared surface at every draw (§5.40), and button fills are derived from the requirement rather than picked by eye. Every button was below the standard. |
| Panels laid out to the width of their English copy | Pseudolocalisation expands every string 40%, accents it and brackets it, and the layout audit measures the result (§5.39). Found the layout translation-ready and the font glyph-complete. |
| Making text bigger silently breaking panels | The layout audit runs at every offered size, so a text-size setting is a checklist rather than a guess (§5.38). It found three unwrapped footnotes at 130%. |
| Text that runs past its panel | A `Region` guard bounds each panel and every text draw inside reports what did not fit, measured with the real font in the capture harness (§5.37). Four such defects shipped and were caught by eye. |
| A test scoped to one machine | The art baseline guard read only the first cabinet, so nine new shapes slipped past the check written to catch them (§5.36). Every art gate walks all six now. |
| A new cabinet shipping unexplained | Adding a win model failed the build until it had prose and a name (§5.29, §5.34), and the soak harness validated its payouts untouched (§5.33). |
| A code in place of a name | Symbol short codes are a rendering fallback and a test now keeps them out of prose (§5.34). They read as correct at every individual call site, which is why they lasted twenty-eight iterations. |
| The sim measuring a game nobody plays | The interactive path is driven headless and held to conservation laws, then compared against the sim on the same seed (§5.33). Features resolve through different functions on the two paths. |
| A summary that hides the shape | The session graph draws the band between bucket extremes, and the toolkit series decimates by extremes rather than averages (§5.32). Averaging would smooth away the spikes the graph exists to show. |
| Music that is inaudible or clips | Tracks are levelled against each other by test, and every mood's summed peak is checked at its real gains (§5.31). The panel found the arpeggio at a fifth of the bass. |
| A session you lose track of | Three figures at a chosen interval, and caps that tighten now but loosen only next session (§5.30). A limit you can lift in the moment is a suggestion. |
| Rules that describe a different game | The panel is generated from the cabinet's own config, and a test asserts every mechanic a machine has is explained (§5.29). Hand-written prose drifted silently across four new cabinets. |
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

## 15. Current State — v1 shipped, plus fifty-five post-v1 systems

**All five phases are done, every item in §14 is met**, and twenty-three systems have
been built on top since: progressive jackpots (§5.6), settings (§5.7), multiple
machines (§5.8), achievements (§5.9), the Vault Pick (§5.10), the reel-feel pass
(§5.11), the Dragon's Wrath (§5.12), the Feature Buy (§5.13), ways-to-win
(§5.14), cascading reels (§5.15), the Dragon's Gamble (§5.16), live machine
profiles (§5.17), the Ledger (§5.18), the synthesis promotion (§5.19) and
shifting reels (§5.20), refining free spins (§5.21), buy-tier profiles (§5.22)
the reel-motion promotion (§5.23), colour legibility (§5.24), testable art (§5.25) and
the rasteriser promotion (§5.26) and keyboard
navigation (§5.27), hints (§5.28), generated rules (§5.29), session limits (§5.30), music (§5.31), the session graph (§5.32) and the conservation harness (§5.33) the naming layer (§5.34) a cluster-pays cabinet (§5.35) its own symbol set (§5.36) a layout audit (§5.37) a text-size setting (§5.38) pseudolocalisation (§5.39) a contrast gate (§5.40) shared symbol sets (§5.41) a theme per cabinet (§5.42) a room to match (§5.43) a score of its own (§5.44) touch input (§5.45) a responsive frame (§5.46) a collision check (§5.47) one command to run every gate (§5.48) a save-compatibility gate (§5.49) a screen registry every audit enumerates (§5.50) an audio audit (§5.51) a motion audit (§5.52) an answer for running out (§5.53) a size limit that is actually enforced (§5.54) one bankroll across six cabinets (§5.55) a web save that belongs to this game alone (§5.56) a floor-wide Grand (§5.57) a game that reads its own save (§5.58) a payline you can actually see (§5.59) and a page of all twenty (§5.60). The game is
published and serving at `http://127.0.0.1/games/dragons_hoard/`, with a Project
Roost deployment recorded and a catalog entry created.

504 tests pass here and 313 in `macroquad-toolkit`; `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` and the `wasm32-unknown-unknown`
release build are clean. Every `.rs` file is under the 800-line limit and a gate now says so
(§5.54); `game.rs` (740) and `ui/reels.rs` (689) are the largest. `data.rs` went
from 799 to 531, `music.rs` from 793 to 680, and the toolkit's `ui/font.rs` from
851 — over the limit, unnoticed — to 656.

Measured RTP over 1,000,000 spins: Dragon's Hoard **0.9612** at **0.411** hit
frequency, Frost Wyrm **0.9491** at **0.259**, Emberfall **0.9596** at **0.622**
(§5.14), Avalanche **0.9429** at **0.475** (§5.15). Both paytables were scaled ~2–3%
down to make room for the Dragon's Wrath (§5.12). The Feature Buy (§5.13) moved
neither, by construction — it is a second door into features that already
existed, priced to return exactly what the reels return.

Captures in `docs/verification/`: `ui_idle`, `ui_spin`, `ui_win`, `ui_freespins`,
`ui_paytable`, `ui_settings`, `ui_machines`, `ui_frost`, `ui_achievements`,
`ui_bonus`, `ui_feature_card`, `ui_hatch`, `ui_jackpot`, `ui_autospin`,
`ui_anticipation`, `ui_wrath`, `ui_featurebuy`, `ui_ways`, `ui_cascade`,
`ui_gamble`, `ui_ledger`, `ui_waveforms`, `ui_shifting`, `ui_refining`,
`ui_vision`, `ui_keyboard`, `ui_hint`. The catalog card image at the project root is produced by the same
harness. `ui_spin` is captured at 20 frames rather than 150 — at the default
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

**Two bugs came from the player, not from the tests.** The reels were too fast
to read the art in flight (`BASE_SPIN_TIME` 0.62 → 0.95, stagger 0.26 → 0.30,
blur cap 1.4 → 0.85), and — worse — **a reel that had landed kept drawing the
previous spin's symbols** until the last reel settled, then the whole board
snapped. `self.grid` is only written when every reel is down, so the resting draw
was reading stale state for up to a second. A win hid it because the payout
count-up holds the board afterwards, which is exactly why it was reported as "it
doesn't stay locked on the result unless I won". `display_grid()` returns the
decided grid while a spin is in flight, and a regression test steps a spin to a
partial landing and asserts it.

That neither had a test was the lesson: everything here is asserted about
*state*, and both of these were about what is on screen at a moment when the
state is mid-flight. The capture harness photographs settled frames, so it could
not have caught them either. **§5.52 closed this**: every cabinet's spin is now
watched frame by frame, and reverting the fix makes the audit name the bug at the
exact frame the board used to snap.

**Measured, though still unheard: how the sound actually sounds.** §5.19 built a
waveform panel and the mix was *looked* at. §5.51 went further and measured it —
the timbre question this section used to call unanswerable turned out to be
arithmetic, and three effects were carrying inharmonic partials, the button blip
only 21dB under its own note. The oscillators are band-limited now and the set is
balanced against a stated order. **Nobody has still heard it**, and audio playback
under WASM in a browser remains untested.

### Remaining work

A jackpot **has** now been seen: the `jackpot` capture scene photographs a real
Mini win, with its ladder plate reset to seed while the other three keep
accruing. That closes the gap this section previously listed.

- **Confirm audio works in the browser build**, not just natively. This is what
  is left of the old "listen to the SFX" item: §5.51 measured the mix, rebalanced
  it against a stated order, and removed the aliasing that made the bright
  effects harsh, so the sound is no longer unexamined — but nobody has still
  *heard* it, and whether it plays at all under WASM is untested.
- Work is committed per iteration following `rust_management/docs/COMMIT_STYLE.md`
  — a diegetic subject, a plain parenthetical tag, and a prose body.
- `audio.rs` **has** been promoted (§5.19). Still outstanding is the
  blur/bounce/anticipation work in `state/spin.rs` — none of it is specific to a
  slot machine beyond the anticipation trigger, and any game with a spinning or
  scrolling strip would want it.
- The gamble panel is the only screen where a **losing** decision is possible,
  and it has no confirmation. That is deliberate — a cabinet that asked "are you
  sure?" on every flip would be unusable — but it does mean a misclick on Ember
  costs the whole win.
- The cascade multiplier badge overlaps the top-right symbol. It is transient
  and only appears above ×1, but a real cabinet would find it somewhere of its
  own rather than over a cell.
- Nothing is left on the promotion list. `audio.rs` went in §5.19 and the reel
  motion in §5.23; what remains in this project is either this game's tuning or
  this game's rules.
- The promotion list is empty again. Three modules have gone into the toolkit
  now — synthesis, strip motion, and the rasteriser — and each left this project
  smaller and better tested than it found it. A real cabinet would show each feature's
  volatility or a sample of what it pays; the price alone tells a player what it
  costs but not what to expect for it.
- **Listen to the effects.** The waveform panel closed the visible part and
  §5.51 the measurable part; what is left is a human ear and a browser.
