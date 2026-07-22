# Batch 10 — Tactical Sim Bots + Band Re-Anchor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two *competent* deterministic sim-bot policies — a tactical-violent and a tactical-diplomat — that route around fights instead of blundering through them, then measure the current game with them to establish the real difficulty baseline (the greedy bot's 17% is a too-dumb reference player).

**Architecture:** Both new policies reuse `sim_seed`'s existing shared routing/loot/BFS spine unchanged. The only new behavior is a *preferred routing view that stamps live monsters as obstacles* (mirroring the existing `routing_map`, which already stamps sokoban blocks as walls) plus a proactive-heal threshold — so a tactical bot prefers any monster-free path and only fights when a monster genuinely walls off the objective. Everything stays a pure function of `Game` state: **no bot-side RNG, ever.** New band files record the *current measured* numbers (a baseline, not the aspirational ~50%, which batches 11–12 tune toward); greedy/pacifist bands stay frozen.

**Tech Stack:** Rust (rustc 1.75), the existing `src/headless.rs` sim harness, `tests/*-band.json` gate files, the `Makefile` gate.

## Global Constraints

- **Bots are pure functions of `Game` state — no bot-side RNG, no system entropy, ever.** Determinism/replay depends on it; two `sim_seed` runs of one seed must be identical.
- **Greedy and pacifist bands MUST NOT MOVE.** `tests/sim-band.json` and `tests/pacifist-band.json` stay byte-identical; their measured numbers (greedy 849/4137/14/0, pacifist 464/4525/11/0 at `--sim 5000`) must reproduce exactly. The new policies share the engine, not those gates.
- **Not a worldgen MAJOR.** `Game::new`/`gen_level`/worldgen untouched. All 5 dump goldens + both frame goldens byte-identical; `--solve 10000` stats unchanged (min615/p50 951/p90 1107/p99 1263/max1494, 0 unwinnable); `xhash` unchanged at `5f4d94c2f9e6aaca`.
- **New tactical bands record the CURRENT measured baseline** (so `make check` stays green today), NOT the ~50% target. The design doc's ~50%/>50% targets are batch-11/12 tuning goals; batch 10 only builds the instrument and captures honest current numbers.
- **`headless.rs` stays grep-clean of game nouns** (cartridge doctrine) — the bots read `Game` fields and `GAME.*` generically; no `rat`/`goblin`/`ogre`/etc. string literals or identifiers.
- **Explicit-width types** in anything that could feed a hash (none here, but keep `i32`/`u32`); `usize` only for indexing.
- **Size budget** — report packed size; a measurement-only batch should barely move it (baseline 206,420 packed).
- Design canon: `docs/design/2026-07-22-mercy-economy-arc.md` §"The measurement reframe (ships FIRST, batch 10)". This plan implements exactly that section — nothing player-facing ships.

---

## File Structure

- **Modify `src/headless.rs`** — add two `Policy` variants; add `tactical_routing_map` + a `tactical_heal_threshold` helper; branch `sim_seed`'s routing-view and heal-threshold selection on policy; add the two new band-path arms in `sim_main`.
- **Modify `src/main.rs`** — extend the `--policy` parse (2 new names); add tests in the `#[cfg(test)] mod tests`.
- **Create `tests/tactical-band.json`** — tactical-violent band (recorded baseline).
- **Create `tests/tactical-pacifist-band.json`** — tactical-diplomat band (recorded baseline).
- **Modify `Makefile`** — `sim` target runs the two new policies too.
- **Modify `CLAUDE.md`** — Verification section (new policies) + a batch-10 status-log entry recording the measured finding.

---

## Task 1: Monster-avoidant routing view

**Files:**
- Modify: `src/headless.rs` (add `tactical_routing_map`, near `routing_map`)
- Modify: `src/main.rs` (test in `mod tests`)

**Interfaces:**
- Consumes: `routing_map(&Game) -> Vec<Tile>` (existing; returns a COLS×MAP_H tile grid with sokoban blocks stamped `Tile::Wall`), `crate::game::{Game, idx, in_map}`, `crate::game::Tile`.
- Produces: `pub(crate) fn tactical_routing_map(g: &Game) -> Vec<Tile>` — `routing_map` plus every live (non-`calm`) monster tile stamped `Tile::Wall`.

- [ ] **Step 1: Write the failing test** (in `src/main.rs` `mod tests`)

```rust
/// batch 10: the tactical routing view stamps a live monster's tile as Wall
/// (so the bot prefers to route around it), but leaves a becalmed monster
/// walkable (the engine swaps on that tile, no fight).
#[test]
fn tactical_routing_map_walls_live_monsters_only() {
    use headless::tactical_routing_map;
    let mut g = Game::new(7);
    // find a live monster on depth 1
    let m = g.monsters.iter().find(|m| !m.calm).expect("a live monster on d1");
    let (mx, my) = (m.x, m.y);
    let view = tactical_routing_map(&g);
    assert_eq!(view[idx(mx, my)], Tile::Wall, "live monster tile must be walled in the tactical view");

    // becalm every monster; now none should be stamped
    for mon in g.monsters.iter_mut() { mon.calm = true; }
    let view2 = tactical_routing_map(&g);
    assert_ne!(view2[idx(mx, my)], Tile::Wall, "a becalmed monster tile must stay walkable");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --release tactical_routing_map_walls_live_monsters_only`
Expected: FAIL — `tactical_routing_map` not found / unresolved import.

- [ ] **Step 3: Write the implementation** (in `src/headless.rs`, immediately after `routing_map`)

```rust
/// Preferred routing view for the tactical policies (batch 10): `routing_map`
/// (sokoban blocks stamped `Tile::Wall`) PLUS every live (non-`calm`) monster's
/// tile stamped `Tile::Wall`, so a tactical bot PREFERS a path that never walks
/// into a fight. Pure function of `Game` state — no RNG — like every other bot
/// decision. Like `routing_map`, this is a PREFERENCE view only, never a
/// reachability proof: if the objective is unreachable in this view (monsters
/// wall off the only corridor), the caller falls back to the ordinary
/// `routing_map` and fights through. A `calm` monster is deliberately NOT
/// stamped — you can walk onto it (the engine swaps, no fight), matching how the
/// step logic already treats calm tiles.
pub(crate) fn tactical_routing_map(g: &Game) -> Vec<Tile> {
    let mut m = routing_map(g);
    for mon in &g.monsters {
        if !mon.calm && in_map(mon.x, mon.y) {
            m[idx(mon.x, mon.y)] = Tile::Wall;
        }
    }
    m
}
```

(If `routing_map` is currently private, change its declaration to `pub(crate) fn routing_map` — Task 2 also needs it. If `in_map`/`idx`/`Tile`/`Game` are not already imported in `headless.rs`, they are — `routing_map`/`sim_seed` already use them.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --release tactical_routing_map_walls_live_monsters_only`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/headless.rs src/main.rs
git commit -m "feat: batch 10 T1 — tactical monster-avoidant routing view"
```

---

## Task 2: `Policy::Tactical` (tactical-violent) wired end to end

**Files:**
- Modify: `src/headless.rs` (`Policy` enum, `Policy::name`, `sim_seed` routing-view + heal-threshold selection)
- Modify: `src/main.rs` (`--policy` parse, tests)

**Interfaces:**
- Consumes: `tactical_routing_map` (Task 1), existing `sim_seed`/`Policy`/`SimResult`.
- Produces: `Policy::Tactical` variant; `Policy::name(Policy::Tactical) == "tactical"`; `--policy tactical` selects it; `sim_seed(seed, Policy::Tactical)` returns a `SimResult` that never emits a talk byte (`spared == 0`) and is deterministic.

**Design of the tactical-violent decisions (all deterministic):**
1. **Preferred routing** uses `tactical_routing_map` instead of `routing_map`. If the objective's `player_d < 0` in that view (monsters wall it off), fall back to the ordinary `routing_map` for this turn and take the shortest-path step (fighting through) exactly as greedy does. This is the whole tactical spine: route around fights, fight only when forced.
2. **Proactive heal**: heal at `3 * g.hp <= 2 * g.maxhp` (≤ two-thirds HP) instead of greedy's `2 * g.hp <= g.maxhp` (≤ half) — a competent player tops up before a fight, not after nearly dying. Same "potion on top of `held`" guard, same USE byte 15.
   - Threshold `2/3` is the **initial measured baseline**, intentionally tunable in batch 11 (documented in the band file), not a magic constant to defend by feel.

- [ ] **Step 1: Write the failing tests** (in `src/main.rs` `mod tests`)

```rust
/// batch 10: the tactical-violent bot is deterministic (two runs identical)
/// and never talks (it's a violence policy — spared stays 0).
#[test]
fn tactical_bot_deterministic_and_never_talks() {
    use headless::{sim_seed, Policy};
    for seed in [1u64, 7, 42, 100, 1337] {
        let (a, _) = sim_seed(seed, Policy::Tactical);
        let (b, _) = sim_seed(seed, Policy::Tactical);
        assert_eq!(
            (a.won, a.dead_dark, a.dead_combat, a.stuck, a.turns, a.light_left, a.kills, a.spared),
            (b.won, b.dead_dark, b.dead_combat, b.stuck, b.turns, b.light_left, b.kills, b.spared),
            "tactical sim must be deterministic for seed {seed}"
        );
        assert_eq!(a.spared, 0, "tactical (violent) policy must never talk, seed {seed}");
        assert!(!a.stuck, "tactical bot must not get stuck, seed {seed}");
    }
}

/// batch 10, the whole point: a competent violent bot wins MORE than greedy on
/// the same seeds — the current game is easy for a player who routes around
/// fights. (A weak inequality over a small sample; the full measured delta is
/// captured in the band file, not asserted here.)
#[test]
fn tactical_bot_wins_at_least_as_often_as_greedy() {
    use headless::{sim_seed, Policy};
    let mut greedy = 0u32;
    let mut tactical = 0u32;
    for seed in 0u64..300 {
        if sim_seed(seed, Policy::Greedy).0.won { greedy += 1; }
        if sim_seed(seed, Policy::Tactical).0.won { tactical += 1; }
    }
    assert!(tactical >= greedy, "tactical {tactical} should win >= greedy {greedy} over 300 seeds");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --release tactical_bot_`
Expected: FAIL — `Policy::Tactical` does not exist.

- [ ] **Step 3: Add the variant + name** (`src/headless.rs`)

```rust
pub(crate) enum Policy {
    Greedy,
    Pacifist,
    Tactical,
    TacticalPacifist,
}
```

```rust
    pub(crate) fn name(self) -> &'static str {
        match self {
            Policy::Greedy => "greedy",
            Policy::Pacifist => "pacifist",
            Policy::Tactical => "tactical",
            Policy::TacticalPacifist => "tactical-pacifist",
        }
    }
```

(Adding `TacticalPacifist` now keeps the `match` exhaustive; it's fully wired in Task 3.)

- [ ] **Step 4: Branch the heal threshold and routing view in `sim_seed`** (`src/headless.rs`)

Replace the heal check
```rust
        if 2 * g.hp <= g.maxhp && g.held.last() == Some(&GAME.balance.loot_potion_item) {
```
with a policy-aware threshold:
```rust
        // batch 10: tactical policies top up earlier (<=2/3 HP) — a competent
        // player heals before a fight, not after nearly dying. Greedy/pacifist
        // keep the <=1/2 rule so their frozen bands don't move. The 2/3 value
        // is the initial measured baseline (tune in batch 11), not a felt
        // constant. Same potion-on-top guard, same USE byte.
        let heal_now = match policy {
            Policy::Tactical | Policy::TacticalPacifist => 3 * g.hp <= 2 * g.maxhp,
            Policy::Greedy | Policy::Pacifist => 2 * g.hp <= g.maxhp,
        };
        if heal_now && g.held.last() == Some(&GAME.balance.loot_potion_item) {
```

Then make the routing view policy-aware. Replace
```rust
        let rmap = routing_map(&g);
```
with
```rust
        // batch 10: tactical policies PREFER a monster-free path (route around
        // fights); if that view leaves the objective unreachable this turn, the
        // `player_d < 0` fallback below re-runs on the ordinary `routing_map`.
        let tactical = matches!(policy, Policy::Tactical | Policy::TacticalPacifist);
        let rmap = if tactical { tactical_routing_map(&g) } else { routing_map(&g) };
```

And add the fallback: where the objective/`dist`/`player_d` are computed, if `tactical && player_d < 0`, recompute on the ordinary `routing_map` once. Concretely, immediately after `let player_d = dist[idx(g.px, g.py)];`, insert:
```rust
        // Tactical fallback: monsters walled off the objective in the preferred
        // view. Re-route on the ordinary map (fight through) — never a false
        // "stuck" over a corridor a fight would open. Rebind rmap/dist/player_d.
        let (rmap, dist, player_d) = if tactical && player_d < 0 {
            let rmap2 = routing_map(&g);
            let dist2 = bfs_dist(&rmap2, objective);
            let pd2 = dist2[idx(g.px, g.py)];
            (rmap2, dist2, pd2)
        } else {
            (rmap, dist, player_d)
        };
```
(Adjust the surrounding `let`s so `rmap`/`dist`/`player_d` are shadowed correctly — the implementer verifies the borrow/shadow shape compiles; `objective` is already bound above this point.)

The final `match step { Some(b) => ... }` stays: for `Policy::Tactical` the `blocked` talk-branch condition (`policy == Policy::Pacifist`) is false, so it bump-attacks exactly like greedy when a fight is genuinely unavoidable. No change needed there for the violent policy.

- [ ] **Step 5: Wire the CLI** (`src/main.rs`, the `--policy` parse ~line 112)

```rust
        let policy = match str_val("--policy").as_deref() {
            Some("pacifist") => Policy::Pacifist,
            Some("tactical") => Policy::Tactical,
            Some("tactical-pacifist") => Policy::TacticalPacifist,
            _ => Policy::Greedy,
        };
```

- [ ] **Step 6: Run the tests** (they should pass now)

Run: `cargo test --release tactical_bot_`
Expected: PASS (both tests).

- [ ] **Step 7: Verify greedy is untouched**

Run: `./target/release/rl144 --sim 5000` (after `cargo build --release`)
Expected: `wins 849 ... deaths_combat 4137, deaths_dark 14, stuck 0` — byte-identical to the frozen greedy baseline. If greedy moved, the policy branch leaked into the shared path — stop and fix.

- [ ] **Step 8: Commit**

```bash
git add src/headless.rs src/main.rs
git commit -m "feat: batch 10 T2 — tactical-violent policy (route around fights, heal early)"
```

---

## Task 3: `Policy::TacticalPacifist` (tactical-diplomat) wired end to end

**Files:**
- Modify: `src/headless.rs` (`sim_seed` talk-branch condition)
- Modify: `src/main.rs` (tests)

**Interfaces:**
- Consumes: everything from Tasks 1–2 (`Policy::TacticalPacifist` already exists and already uses the tactical routing view + heal threshold via the `matches!(policy, Tactical | TacticalPacifist)` branches).
- Produces: `sim_seed(seed, Policy::TacticalPacifist)` never attacks (`kills == 0`), talks the fights it can't route around, is deterministic, and wins ≥ the plain pacifist over a sample.

**Design:** identical to the tactical-violent bot (monster-avoidant routing + early heal) EXCEPT the final step talks instead of bump-attacks when a live monster is genuinely in the way — i.e. the existing pacifist talk-branch must also fire for `TacticalPacifist`. The routing view already means it only reaches that branch when a monster walls off the objective (so it talks far less often than the plain pacifist, and from a stronger position). No awe/cheese here — those mechanics don't exist until batches 11–12; this is "smarter pacifist," and it will be extended when awe lands.

- [ ] **Step 1: Write the failing tests** (`src/main.rs` `mod tests`)

```rust
/// batch 10: the tactical-diplomat never attacks (kills stay 0) and is
/// deterministic.
#[test]
fn tactical_pacifist_never_attacks_and_deterministic() {
    use headless::{sim_seed, Policy};
    for seed in [1u64, 7, 42, 100, 1337] {
        let (a, _) = sim_seed(seed, Policy::TacticalPacifist);
        let (b, _) = sim_seed(seed, Policy::TacticalPacifist);
        assert_eq!(a.kills, 0, "tactical-diplomat must never kill, seed {seed}");
        assert_eq!(
            (a.won, a.turns, a.light_left, a.spared),
            (b.won, b.turns, b.light_left, b.spared),
            "tactical-diplomat must be deterministic, seed {seed}"
        );
        assert!(!a.stuck, "must not get stuck, seed {seed}");
    }
}

/// A diplomat who routes around fights and only talks when forced wins at least
/// as often as the blunt pacifist that talks its way through everything.
#[test]
fn tactical_pacifist_wins_at_least_as_often_as_pacifist() {
    use headless::{sim_seed, Policy};
    let mut base = 0u32;
    let mut tac = 0u32;
    for seed in 0u64..300 {
        if sim_seed(seed, Policy::Pacifist).0.won { base += 1; }
        if sim_seed(seed, Policy::TacticalPacifist).0.won { tac += 1; }
    }
    assert!(tac >= base, "tactical-diplomat {tac} should win >= pacifist {base} over 300 seeds");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --release tactical_pacifist_`
Expected: FAIL — `TacticalPacifist` attacks (the talk-branch only fires for `Policy::Pacifist`), so `kills != 0`.

- [ ] **Step 3: Extend the talk-branch condition** (`src/headless.rs`, in `sim_seed`'s `Some(b)` arm)

Replace
```rust
                let blocked = policy == Policy::Pacifist
                    && g.monsters.iter().any(|m| m.x == nx && m.y == ny && !m.calm);
```
with
```rust
                let talks = matches!(policy, Policy::Pacifist | Policy::TacticalPacifist);
                let blocked = talks
                    && g.monsters.iter().any(|m| m.x == nx && m.y == ny && !m.calm);
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --release tactical_pacifist_`
Expected: PASS.

- [ ] **Step 5: Verify pacifist is untouched**

Run: `./target/release/rl144 --sim 5000 --policy pacifist`
Expected: `wins 464 ... deaths_combat 4525, deaths_dark 11, stuck 0` — byte-identical to the frozen pacifist baseline.

- [ ] **Step 6: Commit**

```bash
git add src/headless.rs src/main.rs
git commit -m "feat: batch 10 T3 — tactical-diplomat policy (route around, talk only when forced)"
```

---

## Task 4: Band files, gate, Makefile, and the baseline measurement

**Files:**
- Create: `tests/tactical-band.json`
- Create: `tests/tactical-pacifist-band.json`
- Modify: `src/headless.rs` (`sim_main` band-path + band-key arms)
- Modify: `Makefile` (`sim` target)

**Interfaces:**
- Consumes: the four `Policy` variants; `sim_main(n, report, policy)`'s existing band-loading logic.
- Produces: `--sim N --policy tactical` and `--policy tactical-pacifist` each gate against their own band file; `make sim` runs all four policies; greedy/pacifist gates unchanged.

- [ ] **Step 1: Measure the current baseline (report mode, no gate yet)**

Run:
```bash
cargo build --release
./target/release/rl144 --sim 5000 --policy tactical --report
./target/release/rl144 --sim 5000 --policy tactical-pacifist --report
```
Record both JSON lines. Expect the tactical win rates to be **well above** greedy's 17% / pacifist's 9% — that gap *is the batch's finding* (the current game is easy for a competent player). Note the exact `win_pct` and `deaths_dark` for each.

- [ ] **Step 2: Write the band files** using the measured numbers

`tests/tactical-band.json` (fill `<WP>`/`<DD>` with Step-1 measurements; band brackets the measured value with headroom, per the sim-band.json convention):
```json
{
  "comment": "tactical-violent bot band for --sim 5000 --policy tactical (batch 10). This bot routes AROUND fights (tactical_routing_map stamps live monsters as walls) and heals at <=2/3 HP, so it is a competent-player proxy — unlike greedy (a too-dumb reference player that only fights what blocks its beeline). Measured on the CURRENT (pre-combat-hardening) game: win_pct <WP>, deaths_dark <DD>, stuck 0. This band RECORDS the current baseline; the design-canon target (docs/design/2026-07-22-mercy-economy-arc.md) is ~50% AFTER batch 11's combat hardening + batch 12's light-as-grace, at which point this band is re-anchored under that sign-off. The 2/3 heal threshold and the routing rule are the tunable knobs. Re-baselining is a balance change: human sign-off required (batch 11 grants it). Band calibrated for 5000 seeds.",
  "win_pct": [<WP-lo>, <WP-hi>],
  "deaths_dark": [<DD-lo>, <DD-hi>]
}
```
`tests/tactical-pacifist-band.json` (same shape, `win_pct` only — mirroring pacifist-band.json which gates `win_pct` alone):
```json
{
  "comment": "tactical-diplomat bot band for --sim 5000 --policy tactical-pacifist (batch 10). Pacifist that routes around fights and only talks when a monster walls off the objective — a competent-mercy proxy. No awe/cheese yet (those land batches 11-12; this bot is extended then). Measured on the CURRENT game: win_pct <WP>, kills_total 0 (never attacks), stuck 0. Records the baseline; design target is >50% and 'harder than today' after the economy lands, re-anchored under batch 12's sign-off. Band calibrated for 5000 seeds.",
  "win_pct": [<WP-lo>, <WP-hi>]
}
```

- [ ] **Step 2b: Choose bracket widths.** Use the same convention as `sim-band.json`: center on the measured value with sane headroom (e.g. measured 55 → `[45, 65]`), wide enough to not be flaky at 5000 seeds, tight enough to catch a real regression. Record the exact reasoning in the `comment`.

- [ ] **Step 3: Add the band-path + band-key arms in `sim_main`** (`src/headless.rs`)

Extend the `band_path` match:
```rust
    let band_path = match policy {
        Policy::Greedy => "tests/sim-band.json",
        Policy::Pacifist => "tests/pacifist-band.json",
        Policy::Tactical => "tests/tactical-band.json",
        Policy::TacticalPacifist => "tests/tactical-pacifist-band.json",
    };
```
Extend the band-key match (the arm that lists which keys each policy checks):
```rust
                Policy::Greedy => &[("win_pct", win_pct), ("deaths_dark", deaths_dark as i32)],
                Policy::Tactical => &[("win_pct", win_pct), ("deaths_dark", deaths_dark as i32)],
                Policy::Pacifist => &[("win_pct", win_pct)],
                Policy::TacticalPacifist => &[("win_pct", win_pct)],
```
Leave the `deaths_dark < deaths_combat` structural check gated on `policy == Policy::Greedy` as-is (it was never claimed for the mercy policies; do not extend it).

- [ ] **Step 4: Verify each new policy gates green**

Run:
```bash
cargo build --release
./target/release/rl144 --sim 5000 --policy tactical            ; echo "exit: $?"
./target/release/rl144 --sim 5000 --policy tactical-pacifist   ; echo "exit: $?"
```
Expected: both print `... inside band` and exit 0. If a band is too tight, widen it (Step 2b) — do not tune the bot to hit a guessed band.

- [ ] **Step 5: Wire `make sim`** (`Makefile`, the `sim:` target)

```makefile
sim: build
	./$(BIN) --sim $(SIM_SEEDS)
	./$(BIN) --sim $(SIM_SEEDS) --policy pacifist
	./$(BIN) --sim $(SIM_SEEDS) --policy tactical
	./$(BIN) --sim $(SIM_SEEDS) --policy tactical-pacifist
```

- [ ] **Step 6: Full gate**

Run: `make UPX=$(command -v upx) check`
Expected: green. Specifically confirm — greedy 849/4137/14/0 and pacifist 464/4525/11/0 **unchanged**; dump+frame goldens byte-identical; `--solve 10000` unchanged; `xhash 5f4d94c2f9e6aaca` unchanged; the two new policies inside their bands. Report packed size (≈206,420 baseline).

- [ ] **Step 7: Commit**

```bash
git add tests/tactical-band.json tests/tactical-pacifist-band.json src/headless.rs Makefile
git commit -m "feat: batch 10 T4 — tactical band gates + measured baseline; make sim runs all four policies"
```

---

## Task 5: Docs — record the instrument and the finding

**Files:**
- Modify: `CLAUDE.md` (Verification section + a batch-10 status-log entry)

- [ ] **Step 1: Verification section** — add, alongside the existing `--sim ... --policy pacifist` bullet, that `--policy tactical` and `--policy tactical-pacifist` are competent-player proxies gated by `tests/tactical-band.json` / `tests/tactical-pacifist-band.json`, that `make sim` now runs all four, and that greedy/pacifist stay as floor-of-competence references whose bands must not move when the tactical bands change.

- [ ] **Step 2: Status-log entry** (append, dated) — record: the two new deterministic policies (monster-avoidant routing + 2/3 heal, no bot-side RNG); the **measured finding** (tactical win rates vs greedy 17% / pacifist 9% — the concrete proof the reference player was too dumb); that greedy/pacifist bands + goldens + `--solve` + `xhash` all reproduced unchanged (not a MAJOR); test counts; sizes. Reference `docs/design/2026-07-22-mercy-economy-arc.md` as the governing canon and note batches 11–12 re-anchor these bands toward the ~50%/>50% targets under their own sign-offs.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: batch 10 — tactical-bot instrument + measured difficulty baseline"
```

---

## Self-review checklist (run before handoff)

- **Spec coverage:** design-doc §measurement-reframe asks for (a) tactical violent policy ✓ T2, (b) smarter diplomacy policy ✓ T3, (c) targets as measurable numbers ✓ T2/T3 tests + T4 bands, (d) re-anchor bands with greedy frozen ✓ T4 (greedy/pacifist bands untouched, verified T2/T3/T4), (e) ships nothing player-facing ✓ (headless-only). No player-facing change, no worldgen touch — consistent with "not a MAJOR."
- **No 50% gate in batch 10:** bands record the current baseline (T4 Step 1–2); the ~50% target is documented as batch-11/12 tuning. ✓
- **Determinism:** no bot-side RNG anywhere in the new code; every decision is a function of `Game` state. Tests assert two-run identity (T2, T3). ✓
- **Type consistency:** `tactical_routing_map(&Game) -> Vec<Tile>`; `Policy::{Tactical, TacticalPacifist}`; `Policy::name` returns `"tactical"`/`"tactical-pacifist"`; band files `tests/tactical-band.json`/`tests/tactical-pacifist-band.json`. Used identically across tasks. ✓
- **Frozen invariants:** greedy 849/4137/14/0, pacifist 464/4525/11/0, goldens, `--solve`, `xhash 5f4d94c2f9e6aaca` — each has an explicit verification step (T2 S7, T3 S5, T4 S6). ✓
