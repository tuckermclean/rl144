# Batch 9 — the overworld skeleton (middle path, story §9-J prep)

**STATUS: DRAFT — decision-ready, nothing implemented.** Human has already picked the direction
(overworld skeleton, minimal trainer/donkey, full NPC substrate and §9-F deferred) per
`docs/superpowers/plans/2026-07-19-batch-9-decision.md`'s Path B analysis. This doc turns that
choice into a concrete, gate-safe design and prices the `=` mechanism Gate 1 flagged as unbriefed.
**No code has been written for this batch.** Read the SIGN-OFF ASKS below before anything lands.

Baseline this brief is priced against (batch 8 T3, `c577ff7`): **560,752 B stripped / 204,152 B
packed (13.84% of 1,474,560)**, xhash `3bb93aae84ab61fb`, `--solve 10000` 0 unwinnable
(min615/p50 951/p90 1107/p99 1263/max 1494, worst seed 2108), `--sim 5000` greedy 849 wins
(17.0%)/4137 combat/14 dark/0 stuck, pacifist 464 wins (9.3%)/4525 combat/11 dark/0 stuck,
`cargo test` 106 passed (minifb) / 110 (term).

---

## SIGN-OFF ASKS (read this first)

The human needs to approve each of these before an implementer touches code. Each is expanded in
the Design section below; this is the checklist form.

1. **The `=` screen-link mechanism shape.** Recommend: a new `WorldId::Overworld` unit variant
   (not three, not a `Floor` extension), with `Game::depth` reused as "current screen index"
   (1..=3) exactly the way `Floor` worlds already pin `depth=1` — i.e. the overworld is modeled
   as a 3-"depth" world using the SAME `stash_level`/`restore_level_at`/`saved`-per-slot machinery
   `descend`/`ascend` already use, not a new mechanism from scratch. See Design §1.
2. **Transition style: instant edge-walk, no confirming input.** Crossing an authored `=` tile
   behaves like `Tile::Stairs` (walk-onto triggers immediately), never like `Tile::Portal`
   (walk-onto only describes; a separate `wait` transits). Confirmed against SPACES-DRAFT's
   diagrams, which show no step-then-wait choreography. See Design §2.
3. **`Game::new(seed)` does NOT change.** It stays the frozen "start directly in the root
   dungeon's depth 1" constructor solve/sim/dump/goldens/xhash/~110 existing tests already depend
   on. A NEW sibling, `Game::new_overworld(seed)`, is the real interactive front door. This is the
   load-bearing decision that keeps every existing gate byte-identical. See Design §4.1.
4. **`ref.sav` gets regenerated, not preserved.** `save::replay()` — used by both `--load` and
   headless `--replay`/`make xhash` — starts calling `Game::new_overworld` instead of `Game::new`,
   since that's what reconstructing a REAL save now means. `tests/fixtures/ref.sav` was recorded
   pre-overworld and its bytes would replay as nonsense overworld moves under the new entry point;
   it needs a fresh recording (a short session: a few overworld moves, cross `V`, a few dungeon
   moves). This is a fixture regen, not a worldgen MAJOR — no golden diff, no band re-baseline.
   See Design §4.2.
5. **This is NOT a worldgen MAJOR.** The three overworld screens are fixed ASCII (zero RNG, like
   `AUTHORED_FLOORS`), and `Game::new`/`gen_level`/the seed→dungeon pipeline are untouched. All 5
   dump goldens and both frame goldens stay byte-identical. See Design §4.4.
6. **`MonsterDef` gains `passive: bool` + `bump: BumpResponse`** (RESOLVED by the human,
   2026-07-21 — no new `invincible` primitive). `passive` (default `false`), checked in
   `monsters_act` beside the existing `calm` skip, gates AI only (never chases/attacks, from
   spawn). Invincibility is NOT a new flag: becalmed monsters are ALREADY effectively invincible
   because a bump against them yields (position-swap, no damage) instead of attacking — the
   trainer/donkey reuse that same "bump can't kill me" shape via a new `bump: BumpResponse`
   field, `{ Fight (default, today's bump-attack), Yield, Shove }`:
   - **TRAINER** = `passive: true, bump: Yield` — swaps position on a bump exactly like a
     becalmed monster (no damage, no death).
   - **DONKEY** = `passive: true, bump: Shove` — stubborn: a bump pushes him ONE tile in the
     bump direction IF the target is plain walkable floor (reusing the sokoban push
     destination check), otherwise he plants and doesn't budge (refused, no damage). You can
     nudge him around the paddock for comedy but never kill your own resurrection point.
   Neither is `calm`, so talk/regard/`talk_lines` still climb normally (the dialogue ladder
   works). `bump` never applies to `Fight` monsters' existing behavior, and runtime `calm` still
   forces a yield regardless of the field (existing becalm behavior unchanged). See Design §3.
7. **§9-J's "corpse/tombstone/inventory persistence on retry" is EXPLICITLY DEFERRED**, split out
   of this batch despite being named in the same story bullet as the overworld skeleton. Today,
   `INPUT_RETRY` fully reconstructs `Game::new(seed)` (now `Game::new_overworld(seed)`) from
   scratch — a genuinely fresh dungeon, discarding whatever the dying attempt left behind. Making
   a corpse/dropped-loot persist across a full death-and-retry is a second, separately-priced
   mechanism (which fields reset vs. carry forward across a `Game` object being thrown away and
   rebuilt) that this brief does NOT design or cost. "Wake up beside the donkey" (the position/
   screen half of resurrection) IS in scope and falls out of decision #3 above for free — see
   Design §3.3. The corpse/loot-persistence half is not.
8. **Donkey's "~5 regard stages"** get authored INTO the existing 4-slot `talk_lines` shape
   (`[[&str;2];4]`: first-landed / mid-landed / threshold-crossing-or-already-calm / failed) —
   the engine only ever indexes 3 pre-calm stages plus a failed stage, regardless of
   `talk_threshold`'s numeric value. "~5 lines" is a content-fit exercise (spread across the 2
   variant slots per stage), not a table-shape change. Flagging so nobody is surprised when 5
   authored lines don't map to 5 distinct engine states. See Design §3.2.

---

## Design

### 1. The `=` screen-link mechanism

**Rejected alternative (from the decision doc's framing): extending `AUTHORED_FLOORS`/
`Dest::Floor` with next/prev pointers.** `Dest::Floor` and `Game::instantiate_floor`
(`src/game.rs:1288-1337`) are built around a SINGLE authored level with exactly one `<` that
always returns to wherever a portal brought you in (`Game::return_to_source`,
`src/game.rs:1370-1377`, keyed off `Game::from`). Retrofitting "this floor also has a next/prev
floor" onto that shape means touching `enter_world_forward`'s Floor arm, `return_to_source`'s
single-exit assumption, and `Game::saved`'s length-1 sizing for `Floor` worlds in three separate
places for one new concept — more surface than adding one new `WorldId` variant.

**Recommended: `WorldId::Overworld` (unit variant), `Game::depth` reused as screen index.**
`Game::depth: u32` already means "current level slot within the current world" — 1..=5 for a
`Seed` world, pinned to 1 for a `Floor` world (`src/game.rs:1288` area). The overworld's 3
screens map onto exactly the same slot: `depth` 1/2/3 = OVR_1/OVR_2/OVR_3, `Game::saved` sized 3
(one stash slot per screen, same shape as `(0..GAME.win.max_depth).map(|_| None).collect()` in
`Game::new` today, `src/game.rs:546`). This means:
- `Game::stash_level()` (`src/game.rs:1091-1104`) needs ZERO changes — it already indexes by
  `self.depth` unconditionally, with no world-kind branch.
- Crossing `=` becomes a new sibling of `descend`/`ascend` (`src/game.rs:1417-1462`): stash the
  current screen, `self.depth = target` (screen index ± 1), restore-from-stash-or-instantiate-
  fresh (a new `instantiate_overworld_screen(i)`, parsed the same way `instantiate_floor` parses
  `AuthoredFloorDef`'s ASCII — zero RNG draws), place the player at the linked edge, log an
  arrival line. This is a near-verbatim copy of `descend`'s shape, not new machinery.
- `land_on_tile`'s tile-kind match (`src/game.rs:1993-2029`) gains one new arm: a new
  `Tile::ScreenLink(bool)` variant (`true` = link to the next screen/east edge, `false` = link to
  the previous screen/west edge — the direction is derivable from which edge column the `=`
  glyph sits on at parse time, so no separate per-tile destination table is needed: OVR_1 has
  only an east-edge `=`, OVR_2 has both, OVR_3 has only a west-edge `=`, matching the linear
  `OVR_1 == OVR_2 == OVR_3` chain SPACES-DRAFT draws). Entry point on the far screen: same row
  `y`, opposite edge column (one tile in from the border, so the player doesn't spawn exactly on
  another link tile).
- `WorldId` (`src/game.rs:158-161`) gains the `Overworld` arm; `hash_world_id` (`save.rs:122-127`)
  gains a third discriminant tag (`2`); `Game::world_seed()` (`src/game.rs:580-585`) gains
  `WorldId::Overworld => self.seed` — the SAME "borrow the root world's depth-1 theme for
  incidental flavor" convention `Floor` already uses (needed because combat messages against a
  training rat still call `self.mob_name()` → `self.theme()` → `world_seed()`, even though the
  overworld itself has no worldgen/theme of its own).
- **The win check needs ZERO changes.** `land_on_tile`'s `UpStairs` arm
  (`src/game.rs:1998-2020`) already gates on `self.world != WorldId::Seed(self.seed)` to decide
  "is this a portal-return, or the real win/lose check" — that comparison is true or false based
  on which world you're CURRENTLY in, not on how `Game::new` started you. Entering
  `WorldId::Seed(seed)` via the hole (below) makes `self.world == WorldId::Seed(self.seed)`
  exactly as it always has been for a direct `Game::new`-started run, so the existing
  `has_objective → won = true` branch fires unchanged. This is the single strongest piece of
  evidence that reusing the existing world-transition machinery, rather than inventing a
  parallel one, is the right call.

**New dump-legend glyphs, checked against collisions**: current legend is
`# . > < * ^ x B @` (tiles) plus `r g O` (monsters), `! ) & ? o [ ~` (items, per
`src/games/contractor.rs:135-264`). None of `V`, `+`, `Y`, `D`, `=` collide. Additions:
- `=` — `Tile::ScreenLink`, walkable, presentation like `Portal`.
- `V` — `Tile::Hole` (or reuse a similar shape), the one true engine-plumbing tile: walking onto
  it transits from `WorldId::Overworld` into `WorldId::Seed(seed)` (see §3.3 below). Distinct
  from `ScreenLink` because it crosses INTO the procedural game, not between two fixed screens.
- `+` — new `Tile::ShutDoor` (name TBD), impassable, bump-message-only THIS BATCH (always logs
  `POS_003`, "not until it's in hand" — regardless of `has_objective`). SPACES-DRAFT flags this
  tile as "doubles as the §3.6 mantel transition once amulet is held" — that state-dependent
  branch and the ending sequence behind it are explicitly OUT OF SCOPE (needs the collector NPC,
  which isn't part of the minimal trainer/donkey ask, and §9-I's "scripted final encounter" isn't
  priced yet either). Ship `+` dumb this batch; flag the smarter version as future work under
  §9-I, not silently half-build it.
- `Y` / `D` — NOT tile glyphs. Trainer and donkey are `MonsterDef` rows (like `r`/`g`/`O`),
  placed by the overworld screen parser exactly the way `instantiate_floor` already places
  monsters from an `AuthoredFloorDef`'s ASCII (`src/game.rs:1317-1321`) — no Tile enum change,
  no new entity-kind engine surface, just two new rows in `GAME.monsters` with `passive: true`.

### 2. Transition style

Confirmed instant edge-walk, not portal-style: SPACES-DRAFT's diagram
(`[OVR_1] == [OVR_2] == [OVR_3]`, `docs/story/SPACES-DRAFT-v0.md:161-165`) draws a direct chain
with no intermediate "you are standing on a link, press wait to cross" step, and its own
mechanical note (lines 172-175) frames `=` as "a genuinely different shape than either existing
format," not a reuse of the portal's rolled-destination-plus-explicit-transit convention. Walking
onto `=` (or `V`) triggers the same turn it's stepped on, mirroring `Tile::Stairs`/`Tile::UpStairs`
(`land_on_tile`'s `Stairs`/`UpStairs` arms already fire on walk-onto, not on a subsequent wait).

### 3. Minimal trainer/donkey

**3.1 — `passive: bool` on `MonsterDef` is sufficient for the ask.** Confirmed against `monsters_act`
(`src/game.rs:2190-2250`): the per-monster loop already has one early-skip branch (`if
self.monsters[i].calm { continue; }`, line 2194). Adding `|| Monster::stats(kind).passive` to that
same condition is the entire engine change for the AI half — no chase, no attack, forever, from
spawn (unlike `calm`, which only applies AFTER crossing `talk_threshold` via a landed talk). Talk
(`try_talk_player`) and give work against a passive monster exactly as they do against any other.
The bump half is the `bump: BumpResponse` field (SIGN-OFF ASK #6): a player's bump into a passive
monster routes to `Yield` (swap, reusing the becalmed-yield path already in `try_move_player`) or
`Shove` (push one tile via the sokoban destination check) instead of the `Fight` bump-attack —
so the trainer/donkey are un-killable by construction, without a new `invincible` flag. `Fight`
(every existing kind) is unchanged. Net engine change: the one `monsters_act` skip line plus a
small bump-dispatch branch in `try_move_player` that reuses two existing code paths (yield, push).

Training rats (SPACES-DRAFT `SPC_OVR_2`) are explicitly NOT passive — the draft itself is clear
they're ordinary `r` rats, fully combat/receptivity-capable, deliberately avoiding a new
decor-mob category (`docs/story/SPACES-DRAFT-v0.md:245-252`). `passive` is spent on exactly two
new rows: TRAINER and DONKEY.

**3.2 — Donkey's regard stages fit the existing 3-bucket shape, not literally "5 stages."**
`try_talk_player` (`src/game.rs:1722-1780`) indexes `talk_lines` by a `stage` value that is only
ever 0 (first landed talk), 1 (a later landed talk still below threshold), 2 (the landed talk
that crosses `talk_threshold`, also reused forever once calm), or 3 (a failed roll) — see the
`stage` computation at `src/game.rs:1751-1757`. Whatever `Monster::talk_threshold(DONKEY)` is set
to (independent of how many *lines* exist), the runtime only ever visits those 4 buckets × 2
variants = 8 authored-line slots. "~5 regard stages" (story §3.2, §12.6) should be understood as
"~5 lines total to place across those 8 slots," not a request for 5 structurally distinct
states. No engine change needed here beyond `passive` itself; flagged so the content-authoring
task doesn't stall waiting for a table-shape widening that was never actually required.

**3.3 — Resurrection reuses retry machinery, and it falls out of decision #3 for free.**
`INPUT_RETRY` (`save::replay`, `src/save.rs:79-96`, byte 6) currently does
`g = Game::new(seed)` unconditionally on death-and-retry — a full fresh reconstruction. Once
`Game::new_overworld(seed)` is the interactive front door and `replay()` calls it (see §4.1-4.2),
`INPUT_RETRY`'s existing `g = Game::new(seed)` line becomes `g = Game::new_overworld(seed)` —
*zero new mechanism*: retry already meant "reconstruct fresh from the top," and "the top" is now
the overworld, beside the donkey, exactly matching story §3.7 ("you wake up top, beside the
donkey"). This is the batch's cleanest win: the story's own claim ("resurrection reuses retry
machinery") is true by construction once the front-door decision is made, not something that
needs its own new code path.

What this does NOT give you (see SIGN-OFF ASK #7): the dungeon state the dying attempt left
behind — dropped items, the corpse/tombstone tile, killed monsters staying dead — is gone,
because `Game::new_overworld` builds an entirely new `Game` object with empty `saved`/`worlds`.
Persisting that across a full object rebuild is a separate mechanism (which fields survive a
death vs. reset) that this brief deliberately does not design.

### 4. THE INTERACTION ANALYSIS

**4.1 — Does the game now start in the overworld? Recommendation: only for real interactive
play, never for the verification surfaces.**

Grep-confirmed call sites of `Game::new(seed)` today: `headless::solve_seed` (`src/headless.rs:69`,
immediately overwrites `g.depth`/calls `g.gen_level()` in a `for d in 1..=max_depth()` loop —
never reads the constructor's initial world/position at all), `headless::sim_seed`
(`src/headless.rs:284`, drives the fresh Game purely through `apply_input` move/talk/use bytes,
with BFS routing logic that assumes it's already standing in a depth with a `Stairs`/objective
target reachable — see `sim_seed`'s doc comment, `src/headless.rs:249-282`), `headless::dump`
(`src/headless.rs:652`, same 5-depth iteration pattern as `solve_seed`), plus ~110 `Game::new(seed)`
call sites across `main.rs`'s test module that assert dungeon-specific behavior (`RAT`/`GOBLIN`
indices, depth-1 map facts, etc.).

If `Game::new(seed)` started in the overworld, `solve_seed`/`dump` would still happen to work
(they overwrite `depth`/regenerate before reading anything), but `sim_seed`'s bots would need
real new routing logic to navigate 3 fixed screens and find `V` — a bot that currently has zero
concept of "walk to a specific named tile that isn't the objective/loot/stairs" — and every one
of those ~110 tests would need auditing for whether they implicitly depended on starting in D1.

**Recommendation: `Game::new(seed)` does not change.** Add `Game::new_overworld(seed) -> Game` as
a new, additive constructor: factor `Game::new`'s body so the shared field-init prefix (RNG
channels, starting stats, etc.) is common, but the tail differs — `Game::new` sets
`world = WorldId::Seed(seed)` and calls `gen_level()` (byte-identical to today), while
`Game::new_overworld` sets `world = WorldId::Overworld`, `depth = 1`, and calls
`instantiate_overworld_screen(1)`. `solve_seed`, `sim_seed`, `dump`, and every existing test keep
calling `Game::new` — **zero changes, zero re-verification risk** for `tests/solver-band.json`,
`tests/sim-band.json`, `tests/pacifist-band.json`, or the ~110 existing tests. Only the
interactive front door (`main.rs`'s fresh-game dispatch, currently
`None => (seed, Vec::new(), Game::new(seed), false)` at `src/main.rs:167`) switches to
`Game::new_overworld(seed)`.

**4.2 — `ref.sav`/`xhash`: `replay()` needs to change, so the fixture needs to change with it.**
`save::replay` (`src/save.rs:79-96`) is the ONE function behind both `--load` (real player saves,
`src/main.rs:159`, `let g = replay(s0, &inputs)`) and headless `--replay <file>`/`make xhash`'s
comparison against `tests/fixtures/ref.sav`. For a REAL save recorded after this batch ships to
reconstruct correctly, `replay()`'s `let mut g = Game::new(seed0)` (line 80) must become
`Game::new_overworld(seed0)` — a real player's session now always begins at the overworld, and
`--load`/ghost-eventually-playback need that reconstructed faithfully. This is a real, necessary
behavior change to `replay()`, not an optional one.

Consequence: `tests/fixtures/ref.sav`'s existing byte log (recorded pre-overworld, meant as
direct D1 dungeon moves) would now replay as a *different, meaningless* sequence of overworld
moves under the new entry point — the fixture stops proving what it's supposed to prove. Per this
brief's own sign-off ask #4: **regenerate `ref.sav`**, not preserve it. `ref.sav`'s only job is
"two backends replaying the same bytes must hash identically" — any valid short session works
equally well for that (record: a couple of overworld moves, cross a screen link, cross `V`, a
couple of dungeon moves, done). This is a one-time, explicit fixture regen exactly like a golden
regen after an authorized change — it needs a status-log line saying so, but it is NOT a
worldgen MAJOR (see 4.4) and needs no band re-baseline.

`INPUT_RESTART`/`INPUT_RETRY`'s other reconstruction calls in `replay()` (`src/save.rs:83-91`)
get the same treatment: `Game::new(h64(...))` → `Game::new_overworld(h64(...))` for restart,
`Game::new(seed)` → `Game::new_overworld(seed)` for retry (see §3.3).

**4.3 — Dump/frame goldens stay byte-identical; the overworld needs its OWN new headless
coverage, not a change to existing goldens.** `headless::dump`/`level_dump` (`src/headless.rs:20-60,
642-660`) and the frame-golden capture (`--render-frame`) both operate on whatever `Game` they're
handed via `Game::new(seed)` — unchanged per §4.1, so all 5 dump goldens (`tests/golden/seed_*.txt`)
and both frame fixtures stay byte-identical; nothing about the procedural D1-D5 pipeline is
touched by this batch. The overworld's 3 screens need their OWN observability, as a NEW gate, not
a retrofit of the existing one: recommend a new headless flag, e.g. `--dump-overworld` (prints
the 3 screens' ASCII the same way `--dump --seed N` prints the 5 depths, since they're
seed-independent there's no `--seed` argument needed), with its own new golden fixture
(`tests/golden/overworld.txt` or similar) `cargo test` string-compares. This gives the overworld
the same "must always work, must not open a window" verification story as the dungeon has,
without touching a single existing fixture.

**4.4 — Not a worldgen MAJOR.** The doctrine (CLAUDE.md, "Seed compatibility") gates on any
change that diffs a golden **map layout** — channel hash constants, the tag scheme, draw
order/count on a worldgen/spawns/vault/theme channel. This batch's overworld content is fully
authored ASCII (like `AUTHORED_FLOORS`, zero RNG — `instantiate_overworld_screen` parses a
`&'static str` exactly the way `instantiate_floor` does, drawing from no channel at all), and
`Game::new`/`gen_level`/`solve_seed`/`sim_seed`/`dump` are provably untouched per §4.1. All 5
dump goldens, both frame goldens, `tests/solver-band.json`, `tests/sim-band.json`, and
`tests/pacifist-band.json` are expected to be **byte-identical, not just in-band** — this is a
strictly additive, parallel piece of content and machinery, not a change to the existing
pipeline. What WOULD make it a MAJOR: touching `gen_level`, the `worldgen`/`spawns`/`vault`/
`theme` channels, or `Game::new`'s existing tail — none of which this design does. `xhash` WILL
change (expected, and fine): `state_hash` folds in `Game::world`/`Game::from`, and any
interactive session that ever touches the overworld produces a different hash than one that
doesn't — same category of "expected shift" as every prior batch that added hashed state
(`speech_attempts`/`objective_dropped` in batch 8, `held` in batch 7, etc.), not a sign that
something broke.

---

## Tasks

**T1 — engine mechanism.** `WorldId::Overworld`, `Tile::ScreenLink(bool)` + `Tile::Hole` (or
similarly named) + `Tile::ShutDoor` (dumb/always-shut version), `GameDef::overworld:
OverworldDef` (3 screen ASCII maps + name/describe strings, same shape as `AuthoredFloorDef`),
`instantiate_overworld_screen(i)`, the screen-link crossing function (stash/restore/place-at-edge,
modeled on `descend`), the hole-crossing function (`leave_current_world` +
`enter_world_forward(WorldId::Seed(seed), (WorldId::Overworld, vx, vy))`), `spend_turn`'s
light-clock exemption (early-return before the deduction when `self.world ==
WorldId::Overworld`, still increments `turns`), `MonsterDef::passive` + the one-line
`monsters_act` gate, `Game::new_overworld`, `world_seed()`'s new arm, `hash_world_id`'s new
discriminant. **Acceptance**: `cargo build`/`cargo test` clean; a new `overworld_screens_well_formed`
test (mirroring `authored_floors_well_formed`, `src/main.rs:2309-2312`) proves the 3 screens are
legal (bordered, only recognized legend chars, correctly linked edges); `--solve 10000`,
`--sim 5000` (both policies), all 5 dump goldens, both frame goldens **byte-identical** to the
batch-8 baseline numbers listed at the top of this doc — this is the task's own self-check, not
just a hope. `xhash` is EXPECTED to change (see §4.4) — verify both backends still agree with
EACH OTHER on the new value, not that the value is unchanged.

**T2 — content + minimal cast.** The 3 screens' ASCII (already drafted in SPACES-DRAFT-v0.md,
transcribe/adjust as needed), TRAINER and DONKEY `MonsterDef` rows (`passive: true`,
`talk_threshold`/`talk_lines` per §3.2), the overworld's own flavor lines (`OVR_ENTER_001`-style
threshold line, screen arrival lines), donkey regard-stage lines fitted into the 4-bucket shape.
Existing `TRA_001`-`TRA_008`/`DON_001`-`DON_005`/`POS_001`-`POS_004`/`NAR_060`-`NAR_063` from
FLAVOR-DRAFT-v0 wire in verbatim per SPACES-DRAFT's own flavor-anchor notes — same "same-ID text
swap only" discipline as every prior content task. **Acceptance**: `--dump-overworld` (new,
T1-built) output eyeballed sane; no change to any dungeon-side content table.

**T3 — save/replay integration + gates + docs.** `Game::new_overworld` becomes the real
interactive front door (`src/main.rs:167`'s fresh-game dispatch); `save::replay` switches its
three `Game::new(...)` calls to `Game::new_overworld(...)` (§4.2); regenerate
`tests/fixtures/ref.sav` with a fresh short session covering an overworld move, a screen-link
cross, a hole cross, and a couple of dungeon moves; re-run `make xhash` against the new fixture
and record the new hash; full `make check` pass with every dungeon-side number reproduced
byte-identical per T1's acceptance criteria; status log entry (this is the pattern-establishing
batch for "overworld skeleton," so document the `Game::new` vs `Game::new_overworld` split
explicitly — future batches touching the front door need to know which one to call).

**Explicitly NOT this batch** (per the human's middle-path choice): the collector NPC, the
mantel/ending sequence (§3.6, needs §9-I), `+`'s state-dependent behavior, corpse/loot
persistence across a death-retry (SIGN-OFF ASK #7), the coat/mimic/lost-guy dungeon cast and its
minigames (§9-F — same NPC-vault MAJOR gate as J, but its two bespoke minigames are a materially
bigger ask than a `passive` flag; explicitly deferred, not implied by this batch).

---

## Gates (what must stay identical vs. what's new)

| Gate | Expected result | Why |
|---|---|---|
| `tests/golden/seed_{1,2,3,42,1337}.txt` (5-depth dungeon dumps) | **byte-identical** | `Game::new`/`gen_level` untouched |
| Frame goldens (`frame_seed_1.bin`, `frame_seed_42.bin`, `frame_seed_1_ascii.bin`) | **byte-identical** | same reason; term backend's `--render-frame` still starts via `Game::new` |
| `tests/solver-band.json` (`--solve 10000`) | **byte-identical** (min615/p50 951/p90 1107/p99 1263/max 1494, worst seed 2108, 0 unwinnable) | `solve_seed` calls `Game::new` unchanged, never touches the overworld |
| `tests/sim-band.json` (greedy `--sim 5000`) | **byte-identical** (849 wins/4137 combat/14 dark/0 stuck) | `sim_seed` calls `Game::new` unchanged; bot logic untouched |
| `tests/pacifist-band.json` (pacifist `--sim 5000`) | **byte-identical** (464 wins/4525 combat/11 dark/0 stuck) | same reason |
| `make xhash` (cross-backend agreement) | **hash VALUE changes**, agreement between backends must hold | `state_hash` folds in `Game::world`/`from`; both backends still run the same code, so they must still match each other on the new value |
| `tests/fixtures/ref.sav` | **regenerated** (new session, new bytes) | `replay()`'s entry point changes to `Game::new_overworld` (§4.2) |
| New: `--dump-overworld` + its golden | **new gate, new fixture** | the overworld needs its own observability, not a retrofit of the dungeon dumps (§4.3) |
| New: `overworld_screens_well_formed` test | **new test**, mirrors `authored_floors_well_formed` | proves the 3 screens are legal ASCII with correctly-linked edges before anything ships |
| `cargo test` (both feature sets) | grows by however many new tests T1-T3 add; **all existing ~106/110 tests pass unchanged** | `Game::new`'s behavior is frozen; no existing test's assumptions about starting in D1 are disturbed |
| Worldgen MAJOR sign-off | **not required** | additive fixed content, zero RNG, zero touch to `gen_level`/the worldgen/spawns/vault/theme channels (§4.4) |

---

## Size estimate

Current baseline: 204,152 B packed (13.84% of 1,474,560). This batch adds:
- T1 (engine): one new `WorldId` variant, two new `Tile` variants, one new `MonsterDef` field +
  one `monsters_act` condition, one new constructor, a handful of new small functions modeled
  closely on existing ones (`descend`-shaped screen-link crossing, `instantiate_floor`-shaped
  screen instantiation). Comparable in kind to batch 6 T1's portal machinery, but smaller in
  scope (3 fixed screens vs. an open-ended multiverse) — estimate 3-6 KB packed.
- T2 (content): 3 screens' worth of ASCII (comparable to 2-3 vaults' worth of const string data)
  plus ~30 lines of overworld dialogue/flavor per the story's own line-budget table (§8: "Overworld:
  trainer, donkey, posting — 30 lines"), mostly already drafted in FLAVOR-DRAFT-v0.md and reused
  verbatim. Estimate 2-4 KB packed (ASCII compresses well under UPX, consistent with every prior
  content-heavy batch's actual numbers).
- **Total estimate: 5-10 KB packed**, landing around 210,000-214,000 B packed
  (14.2%-14.5% of budget) — comfortably inside the project's size headroom (current budget usage
  is 13.84%; even the high end of this estimate leaves >85% of the budget unused). Size is not
  the constraint on this batch; engine-surface correctness and gate protection are, which is why
  this brief spends most of its length there.
