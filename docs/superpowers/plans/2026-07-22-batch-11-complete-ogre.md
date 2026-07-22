# Batch 11 — The Complete Ogre + Heal Scarcity — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the ogre a two-answer creature — fighting it always costs HP (guaranteed retaliation), standing tall through its hits awes it into a becalm — and make healing scarce, then tune violence toward ~50% against the batch-10 tactical bots.

**Architecture:** Two runtime combat mechanics (a `MonsterDef.retaliation` field applied in the player-attack branch; a hashed `Monster.awe` counter that builds while the player holds adjacent without attacking and becalms at `MonsterDef.awe_threshold`) plus one worldgen loot change (cut the potion drop rate). The tactical-diplomat bot is extended to "stand tall" against ogres. Then an explicit tuning loop re-anchors all four sim bands and regenerates the dungeon goldens.

**Tech Stack:** Rust (rustc 1.75), `src/game.rs` (combat + worldgen), `src/gamedef.rs` + `src/games/contractor.rs` (the cartridge data), `src/headless.rs` (the bots), `tests/golden/` + `tests/*-band.json` (the gates).

## Global Constraints

- **THIS IS A COMBAT-BALANCE MAJOR requiring human sign-off** (same class as batches 3/4). It moves ALL FOUR sim bands (greedy/pacifist/tactical/tactical-pacifist — every policy fights or endures ogres and picks up potions) AND regenerates the dungeon dump goldens (the potion-drop-rate cut changes item placement). Sign-off for landing the band re-baseline + golden regen was granted 2026-07-22 (this batch's go-ahead + the arc doc). Do the re-baseline in Task 5 only, with the numbers documented.
- **Cartridge doctrine — engine stays grep-clean of game nouns.** The ogre-ness of these mechanics lives entirely in cartridge DATA: `game.rs` reads `Monster::stats(kind).retaliation` and `.awe_threshold` generically, never `OGRE`. Only `src/games/contractor.rs` names the ogre.
- **Hashed-state discipline.** `Monster.awe` is run-defining (it changes whether a future turn becalms the ogre — same status as `regard`/`calm`) → it MUST join `state_hash` (`save.rs`), alongside `regard`/`calm`. It is NOT presentation-only. `retaliation`/`awe_threshold` are `MonsterDef` constants, not per-`Game` state — nothing to hash.
- **Explicit-width types** in any hashed `Game`/`Monster` field (`i32`/`u8`/`u32`), never `usize`.
- **Bots stay pure functions of Game state — no bot-side RNG.**
- **Determinism/replay:** `SAVE_VERSION` does NOT change (no new input byte — standing tall is ordinary movement/wait, awe is a passive engine reaction). `xhash` WILL change (new hashed `Monster.awe` state) — that is the one sanctioned hash shift, both backends must agree on the new value.
- Design canon: `docs/design/2026-07-22-mercy-economy-arc.md` §"Batch 11" and §"Goblinoid awe" and §"Engine 1 — HP attrition". This plan implements exactly those.
- Size budget: report packed (baseline 207,348).

---

## File Structure

- **Modify `src/gamedef.rs`** — add `MonsterDef.retaliation: i32` and `MonsterDef.awe_threshold: u8`; document both.
- **Modify `src/games/contractor.rs`** — set `retaliation`/`awe_threshold` on all monster rows (0/0 for rat/goblin/trainer/donkey; the ogre's tuning values); cut the potion loot share in `BALANCE`.
- **Modify `src/game.rs`** — apply retaliation in the player-attack branch; add the `Monster.awe` field + the hold-adjacent-builds-awe / retreat-resets / threshold-becalms logic; (Task 4) the loot-draw change is a `BALANCE` value, so `gen_level` needs no structural edit.
- **Modify `src/save.rs`** — fold `Monster.awe` into `state_hash`.
- **Modify `src/headless.rs`** — extend the tactical-diplomat to stand tall against ogres.
- **Regenerate `tests/golden/seed_{1,2,3,42,1337}.txt`** (Task 4) and re-baseline `tests/{sim,pacifist,tactical,tactical-pacifist}-band.json` (Task 5).
- **Modify `CLAUDE.md`** — doctrine + status log (Task 6).

---

## Task 1: Ogre guaranteed retaliation (the violence tax on ogres)

**Files:** Modify `src/gamedef.rs` (`MonsterDef`), `src/games/contractor.rs` (rows), `src/game.rs` (attack branch); Test in `src/main.rs`.

**Interfaces:**
- Produces: `MonsterDef.retaliation: i32` (damage dealt to the player when they bump-attack this kind, guaranteed, even on a killing blow; 0 = none). Read via `Monster::stats(kind).retaliation`.

- [ ] **Step 1: Failing test** (`src/main.rs` mod tests) — attacking an ogre costs the player HP even when the blow kills it.
```rust
#[test]
fn attacking_an_ogre_always_costs_hp() {
    let mut g = Game::new(7);
    // place a 1-hp ogre cardinally adjacent to the player, nothing else in the way
    let (ox, oy) = (g.px + 1, g.py);
    g.monsters.clear();
    g.monsters.push(crate::game::Monster { kind: OGRE, x: ox, y: oy, hp: 1, ..crate::game::Monster::spawn(OGRE, ox, oy) });
    let hp_before = g.hp;
    g.apply_input(3); // move/bump East onto the ogre
    assert!(g.monsters.iter().all(|m| !(m.x == ox && m.y == oy)) , "the 1-hp ogre should be dead");
    assert!(g.hp < hp_before, "killing an ogre must still cost the player HP (guaranteed retaliation)");
}
```
(The exact `Monster` construction may differ — use whatever constructor/fields the codebase exposes; the assertion is what matters: ogre dies AND `g.hp` dropped. If there's no easy `Monster::spawn`, build the struct with its real fields.)

- [ ] **Step 2: Run — FAIL** (`cargo test --release attacking_an_ogre_always_costs_hp`): ogre dies but `hp` unchanged (no retaliation yet).

- [ ] **Step 3: Add the field** (`src/gamedef.rs`, in `MonsterDef`, with a doc comment):
```rust
    /// batch 11: HP the player loses when they bump-ATTACK this kind — a
    /// guaranteed retaliation applied even on a killing blow, so engaging this
    /// kind always costs HP no matter how well you play. `0` for every kind
    /// that doesn't bite back on contact (rat/goblin/passive NPCs); the ogre's
    /// value is the batch-11 tuning knob. Distinct from the monster's ordinary
    /// `monsters_act` turn (which still happens if it survives).
    pub(crate) retaliation: i32,
```

- [ ] **Step 4: Set it on every monster row** (`src/games/contractor.rs`): `retaliation: 0` on RAT/GOBLIN/TRAINER/DONKEY; `retaliation: 3` on OGRE (**initial tuning value — Task 5 tunes it**; comment it `// [TUNE] batch 11`).

- [ ] **Step 5: Apply it in the player-attack branch** (`src/game.rs`, right after the player deals `dmg` to `self.monsters[mi].hp`, ~line 1823):
```rust
        // batch 11: the ogre (any kind with retaliation > 0) always lands a
        // hit back the instant you swing — even a killing blow costs you. This
        // is separate from its ordinary monsters_act turn (which only happens
        // if it survives). Runs the same death path any HP loss does.
        let retal = Monster::stats(self.monsters[mi].kind).retaliation;
        if retal > 0 {
            self.hp -= retal;
            if self.hp <= 0 { self.hp = 0; self.dead = true; self.killer = Some(self.monsters[mi].kind); }
        }
```
(Verify against the real death-handling convention in `game.rs` — match how `monsters_act` sets `dead`/`killer`. If death is centralized in a helper, call that instead of open-coding it.)

- [ ] **Step 6: Run — PASS.** Then confirm no golden/xhash impact yet is NOT expected — this changes combat, so `--sim` numbers move (that's fine, Task 5 re-baselines) but goldens must still be byte-identical (retaliation is runtime, not worldgen): diff `--dump --seed 1` against `tests/golden/seed_1.txt` → identical. If a golden moved, something touched worldgen — stop.

- [ ] **Step 7: Commit** `feat: batch 11 T1 — ogre guaranteed retaliation (fighting an ogre always costs HP)`.

---

## Task 2: Ogre stand-tall-awe-becalm (the diplomat's ogre answer)

**Files:** Modify `src/game.rs` (`Monster` struct + awe logic), `src/gamedef.rs` (`MonsterDef.awe_threshold`), `src/games/contractor.rs` (rows), `src/save.rs` (`state_hash`); Test in `src/main.rs`.

**Interfaces:**
- Produces: `Monster.awe: u8` (hashed run state); `MonsterDef.awe_threshold: u8` (holds-adjacent needed to becalm this kind; 0 = cannot be awed). When `awe >= awe_threshold` the monster becalms exactly like a talk-becalm (`calm = true`, `Game::spared += 1`).

**Design (decided — implement exactly this):** each turn, AFTER the player acts, for every monster with `awe_threshold > 0` that is cardinally adjacent to the player and is NOT already `calm` and was NOT bump-attacked by the player this turn: `awe += 1`; if `awe >= awe_threshold` → `calm = true`, `spared += 1`, log the becalm. For every such monster the player is NOT adjacent to (or that the player attacked): `awe = 0` (fleeing or fighting breaks the stare). "Standing tall" is therefore: end your turn adjacent to the ogre without swinging at it — you endure its `monsters_act` hit and your nerve builds. This reuses the existing becalm state (`calm`/`spared`), so all downstream mercy behavior (no chase/attack, yield-on-bump, light-as-grace in batch 12) works unchanged.

- [ ] **Step 1: Failing test** — waiting adjacent to an ogre for `awe_threshold` turns becalms it, without attacking it.
```rust
#[test]
fn standing_tall_awes_an_ogre_into_calm() {
    let mut g = Game::new(11);
    let (ox, oy) = (g.px + 1, g.py);
    g.monsters.clear();
    g.monsters.push(crate::game::Monster { kind: OGRE, x: ox, y: oy, hp: 99, ..crate::game::Monster::spawn(OGRE, ox, oy) });
    let thr = crate::game::Monster::stats(OGRE).awe_threshold as usize;
    assert!(thr > 0, "ogre must be awe-able");
    for _ in 0..thr { g.apply_input(4); } // WAIT adjacent = stand tall
    assert!(g.monsters.iter().any(|m| m.x == ox && m.y == oy && m.calm), "ogre should becalm via awe");
    assert_eq!(g.kills, 0, "standing tall is not violence — no kill");
}
```
(Use enough starting HP/light that the loop survives; `Game::new(11)` starts with full light. If ogre hits drop the player HP to 0 within `thr` turns, give the test player extra hp first — the point is the awe counter, not survival.)

- [ ] **Step 2: Run — FAIL** (field/logic missing).

- [ ] **Step 3: Add `Monster.awe`** (`src/game.rs`, `Monster` struct): `pub(crate) awe: u8,` with a doc comment (hashed; built by standing tall; reset on flee/attack). Initialize `awe: 0` everywhere a `Monster` is constructed (spawn + tests).

- [ ] **Step 4: Add `MonsterDef.awe_threshold`** (`src/gamedef.rs`): `pub(crate) awe_threshold: u8,` (doc: holds-adjacent-without-attacking needed to becalm via awe; 0 = not awe-able). Set on rows in `contractor.rs`: `0` for RAT/GOBLIN/TRAINER/DONKEY, `3` for OGRE (**[TUNE] batch 11**).

- [ ] **Step 5: The awe logic** — add a `Game` method `fn resolve_awe(&mut self, attacked: Option<usize>)` called once per player action (find the right call site — after the player's move/attack resolves and before/after `monsters_act`; document the ordering choice). It implements the Design paragraph above. `attacked` is the index of a monster the player bump-attacked this turn (so it's excluded), or `None`.

- [ ] **Step 6: Hash it** (`src/save.rs`, `state_hash`): fold `Monster.awe` into the per-monster hash right beside `regard`/`calm`. Update the doc comment enumerating hashed monster state.

- [ ] **Step 7: Run — PASS.** Add a second test `fleeing_resets_ogre_awe` (build awe part-way, then step AWAY one turn, assert `awe == 0` and not calm).

- [ ] **Step 8: Commit** `feat: batch 11 T2 — ogre stand-tall-awe-becalm (hold ground, don't flinch)`.

---

## Task 3: Extend the tactical-diplomat to stand tall against ogres

**Files:** Modify `src/headless.rs` (`sim_seed`); Test in `src/main.rs`.

**Interface:** the `tactical-pacifist` bot, when its path is blocked by (or it is cornered by) an ogre it cannot route around, emits a HOLD (stand tall) instead of a talk, so it awes the ogre rather than making a low-odds talk roll. It still never attacks (`kills == 0`).

**Design decision (resolve the wait/portal wrinkle):** "hold" = emit **wait (byte 4)** — but the sim bots have never emitted wait (the `bot_never_transits` invariant relies on it, since wait transits a portal). Guard it: the diplomat emits wait to stand tall ONLY when it is adjacent to an awe-able non-calm monster AND is NOT standing on a `Tile::Portal`. That preserves `bot_never_transits` (the bot still never waits on a portal). Only `tactical-pacifist` gets this; the violent bots and greedy/plain-pacifist are unchanged. Add an assertion/test that the bot never emits wait while on a portal.

- [ ] **Step 1: Failing test** — `tactical-pacifist` becalms ogres via awe on some seed where it previously could only talk-or-die; assert `kills == 0` still holds and it wins at least one seed involving an ogre stand-off. (Pick a concrete seed after exploring; assert the invariant, not an exact rate.)
- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3: Implement** the stand-tall branch in `sim_seed`'s tactical-diplomat path (emit byte 4 under the guard above).
- [ ] **Step 4: Run — PASS**; also confirm `bot_never_transits` still passes.
- [ ] **Step 5: Commit** `feat: batch 11 T3 — tactical-diplomat stands tall against ogres`.

---

## Task 4: Heal scarcity — cut the potion drop rate (WORLDGEN, regenerates goldens)

**Files:** Modify `src/games/contractor.rs` (`BALANCE` loot values); regenerate `tests/golden/seed_{1,2,3,42,1337}.txt`.

- [ ] **Step 1:** In `BALANCE` (contractor.rs), cut the potion share of the loot draw (the `chance(num,den)` that picks `loot_potion_item`) — the arc doc's primary heal-scarcity lever. Initial cut: make potions roughly half as common (**[TUNE] batch 11** — exact value set in Task 5). Optionally reduce the per-depth heal-on-descent grant (secondary lever) — a runtime value, no golden impact.
- [ ] **Step 2:** This changes worldgen loot placement → dump goldens WILL differ. Regenerate: `for s in 1 2 3 42 1337; do ./target/release/rl144 --dump --seed $s > tests/golden/seed_$s.txt; done`. Eyeball the seed-1 diff: it must be ONLY item-content changes (fewer potions, different loot mix), never wall/floor/corridor/stairs structure — the potion-count cut must not move room geometry.
- [ ] **Step 3:** `--solve 10000` must still be 0 unwinnable (fewer potions doesn't affect BFS reachability, but confirm — potions aren't gating). Re-commit `tests/solver-band.json` only if its numbers actually drift (they shouldn't — solve is geometry).
- [ ] **Step 4: Commit** `feat: batch 11 T4 — heal scarcity (cut potion drops); regenerate dungeon goldens (worldgen MAJOR)`.

---

## Task 5: Tune to violence ~50% and re-baseline all four bands (controller-driven)

**Files:** Modify `src/games/contractor.rs` (the `[TUNE]` values: OGRE `retaliation`, `awe_threshold`, potion drop rate, maybe heal-on-descent); re-baseline `tests/{sim,pacifist,tactical,tactical-pacifist}-band.json`.

This is an iterative measure-and-adjust task, not a fixed transcription. Target: **`--sim 5000 --policy tactical` win_pct in [45,55]** (violence capped ~50%); observe tactical-pacifist (should drop from 49% but stay viable — the arc's "diplomacy harder"; batch 12's light-as-grace is what later makes it the reliable >50% path, so do NOT try to force diplomacy above violence here). Greedy/pacifist will also move — record their new numbers.

- [ ] **Step 1:** Measure all four policies at `--sim 5000 --report` with the current [TUNE] values.
- [ ] **Step 2:** Adjust OGRE `retaliation` / `awe_threshold` / potion drop rate until tactical-violent lands in [45,55]. Re-measure after each change. Keep a short log of (values → win rates). Heal scarcity lowers everyone; ogre retaliation specifically taxes the violent bots.
- [ ] **Step 3:** Once tactical-violent ∈ [45,55], freeze the values and re-baseline ALL FOUR band files to the final measured numbers (bracket each with headroom, per the sim-band.json convention; each comment records the new baseline, the values that produced it, and that this is the authorized batch-11 combat-balance re-baseline).
- [ ] **Step 4:** If Task-4 goldens were regenerated with a placeholder potion rate, regenerate them once more with the final rate.
- [ ] **Step 5: Commit** `feat: batch 11 T5 — tune ogre/heal-scarcity to violence ~50%; re-baseline all four sim bands`.

---

## Task 6: Full gate + docs

- [ ] **Step 1:** `make UPX=$(command -v upx) check` FOREGROUND — green. Expect: goldens regenerated-and-matching, `xhash` a NEW value (both backends agree — `Monster.awe` joined the hash), `--solve 10000` 0 unwinnable, all four sim bands passing at their new baselines, size reported.
- [ ] **Step 2:** `CLAUDE.md`: add a doctrine bullet for the complete-ogre mechanic (retaliation field + awe-becalm, both cartridge-data-driven; `Monster.awe` hashed) and a dated batch-11 status-log entry (the two ogre answers, heal scarcity, the tuning result with final violence/diplomacy numbers, the golden regen + four-band re-baseline as the authorized MAJOR, the new xhash, test counts, sizes). Reference the arc doc.
- [ ] **Step 3: Commit** `docs: batch 11 — complete ogre + heal scarcity, status log`.

---

## Self-review checklist

- **Spec coverage:** ogre guaranteed hit ✓T1; stand-tall-awe-becalm ✓T2; diplomat's ogre answer ✓T3; heal scarcity ✓T4; tune violence ~50% + re-baseline ✓T5; goldens/gate/docs ✓T4/T6. All from the arc doc's batch-11 + awe + engine-1 sections.
- **MAJOR handled:** goldens regenerated (T4), all four bands re-baselined (T5), sign-off noted (Global Constraints). Not silent.
- **Hash discipline:** `Monster.awe` hashed (T2 S6); `retaliation`/`awe_threshold` are constants (not hashed). `xhash` expected to shift (documented).
- **Grep-clean:** ogre-ness stays in contractor.rs; game.rs reads `.retaliation`/`.awe_threshold` generically. No new game-noun literal in engine files.
- **Type consistency:** `MonsterDef.retaliation: i32`, `MonsterDef.awe_threshold: u8`, `Monster.awe: u8`. Used identically across tasks.
- **Determinism:** no bot-side RNG (T3 wait-guard preserves `bot_never_transits`); `SAVE_VERSION` unchanged (no new byte).
- **Open (tuning, by design, resolved in T5 with the bots):** OGRE retaliation, awe_threshold, potion drop rate, heal-on-descent — measured, not guessed.
