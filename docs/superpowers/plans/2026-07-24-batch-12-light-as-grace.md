# Batch 12 — Light as Grace (the keystone) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make light the moral currency — every kill dims it, every spare feeds it — so a kill-heavy run runs out of torch on the climb while a merciful run stays light-rich, flipping diplomacy above violence as the reliable path.

**Architecture:** Two runtime hooks on existing events — subtract `kill_light_penalty` at the kill site, add `spare_light_gain` (capped at START_LIGHT) wherever a monster becalms — plus a re-tune against the batch-10/11 tactical bots so tactical-diplomat > tactical-violent, re-baselining all four bands. No new hashed fields (`light`/`kills`/`spared` are already hashed); the McGuffin's existing batch-8 voice becomes the mood/fuel-gauge surface.

**Tech Stack:** Rust (rustc 1.75), `src/game.rs` (the kill/spare hooks, spend_turn), `src/gamedef.rs` + `src/games/contractor.rs` (two new BalanceDef fields), `tests/*-band.json` (the re-baseline).

## Global Constraints

- **THIS IS A BALANCE MAJOR requiring human sign-off** (same class as batches 3/4/11). It re-baselines ALL FOUR sim bands (the light economy changes every policy's survival). Sign-off granted 2026-07-24 (this batch's go-ahead + the arc doc). It is NOT a worldgen MAJOR: `Game::new`/`gen_level`/worldgen are untouched, so **dump + frame goldens MUST stay byte-identical** (light-as-grace is pure runtime combat economy). `--solve 10000` also stays unchanged (the solver is geometric — it proves the pure walk fits START_LIGHT and never simulates combat; kill/spare light deltas only affect sim outcomes, not the walk budget — see the Winnability note below).
- **Winnability model (the decision the arc doc flagged):** the solver's guarantee stays GEOMETRIC — "a low-violence walk of the round trip fits within START_LIGHT" — UNCHANGED. Light-as-grace is the SIM-measured economy layered on top: killing drains light (you may not make the climb), sparing refills it (you stay lit). START_LIGHT stays 2000 unless the tuning genuinely needs it moved (if it does, that's a further explicit re-derivation, documented — but prefer tuning the deltas within the existing margin so the solver guarantee is untouched).
- **No new hashed state.** `light`, `kills`, `spared` are ALREADY hashed (`state_hash`); this batch only changes WHEN `light` moves, so no `state_hash`/`SAVE_VERSION` change. `xhash` shifts ONLY if `tests/fixtures/ref.sav`'s replay kills or spares anything (it's a 6-move overworld→dungeon session; likely no combat, so xhash likely UNCHANGED — verify, and if it does shift, both backends must agree).
- **Cartridge doctrine:** the two new numbers are `BalanceDef` fields (`kill_light_penalty`, `spare_light_gain`), set in `contractor.rs`; `game.rs` reads `GAME.balance.*` generically. No game nouns in engine files.
- **Explicit width** (`i32` light math), bots stay pure (no bot-side RNG), size budget (report packed; baseline 208,220).
- **Deferred out of this batch, to batch 13** (per the arc doc's batch-12 bundle, split for focus): goblin awe (walk-backward becalm), cheese→goblin, the becalm return-trip dividend (its farming-guard makes it its own task), the NPC category fix, the trainer-reads-your-last-life memory, and the donkey-follow seed. Batch 12 is the light-economy CORE only.
- Design canon: `docs/design/2026-07-22-mercy-economy-arc.md` §"Engine 2 — light as grace" and §"The measurement reframe".

---

## File Structure

- **Modify `src/gamedef.rs`** — add `BalanceDef.kill_light_penalty: i32` and `BalanceDef.spare_light_gain: i32`, documented.
- **Modify `src/games/contractor.rs`** — set both (initial `[TUNE]` values); re-tune in Task 3.
- **Modify `src/game.rs`** — subtract the penalty at the kill site (~1872); add the gain (capped at `start_light()`) at every becalm/spare site (route them through one helper so no site is missed); the `light <= 0` death check already runs in `spend_turn` — ensure a kill that drains light to 0 is a correct dark-death on the NEXT `spend_turn` (or immediately — match the existing convention).
- **Re-baseline `tests/{sim,pacifist,tactical,tactical-pacifist}-band.json`** (Task 3).
- **Modify `src/render.rs` / the McGuffin line data** (Task 4) — surface the mood/fuel-gauge minimally.
- **Modify `CLAUDE.md`** — doctrine + status log (Task 5).

---

## Task 1: Kills dim your light

**Files:** `src/gamedef.rs` (`BalanceDef.kill_light_penalty`), `src/games/contractor.rs`, `src/game.rs` (kill site); test in `src/main.rs`.

- [ ] **Step 1: Failing test** — killing a monster drops light by `kill_light_penalty` beyond the ordinary turn/tax burn.
```rust
#[test]
fn killing_dims_light() {
    let mut g = Game::new(7);
    let (ox, oy) = (g.px + 1, g.py);
    g.monsters.clear();
    g.monsters.push(crate::game::Monster { kind: RAT, x: ox, y: oy, hp: 1, ..crate::game::Monster::spawn(RAT, ox, oy) });
    let light_before = g.light;
    g.apply_input(3); // bump East, kill the 1-hp rat
    assert!(g.monsters.iter().all(|m| !(m.x == ox && m.y == oy)), "rat should be dead");
    let ordinary = GAME.balance.base_burn + GAME.balance.violence_tax;
    assert_eq!(g.light, light_before - ordinary - GAME.balance.kill_light_penalty,
        "a kill must dim light by kill_light_penalty on top of the ordinary attack-turn burn");
}
```
- [ ] **Step 2: Run — FAIL** (field + hook missing).
- [ ] **Step 3:** Add `BalanceDef.kill_light_penalty: i32` (`gamedef.rs`, doc: "light lost each time the player kills — the McGuffin recoils; the violence half of light-as-grace [batch 12]"). Set an initial `[TUNE]` value in `contractor.rs` (start with `8`).
- [ ] **Step 4:** At the kill site (`game.rs`, right where `self.kills += 1`): `self.light -= GAME.balance.kill_light_penalty;` with a comment. (Do NOT re-check death here — `spend_turn` later this same turn burns the base+tax and runs the single `light <= 0` check, so a kill that pushes light to 0 becomes a dark death there, preserving lose-before-win. Confirm this ordering against `spend_turn`; if the kill site is AFTER `spend_turn` in the turn, add the death check to match convention.)
- [ ] **Step 5: Run — PASS.** Confirm dump goldens byte-identical (runtime-only).
- [ ] **Step 6: Commit** `feat: batch 12 T1 — kills dim your light (the McGuffin recoils)`.

---

## Task 2: Spares feed your light

**Files:** `src/gamedef.rs` (`BalanceDef.spare_light_gain`), `src/games/contractor.rs`, `src/game.rs` (all becalm sites via one helper); test in `src/main.rs`.

**Interface:** wherever a monster becalms (`self.spared += 1` — currently the talk-threshold-cross, the awe-threshold-cross, and any other becalm site), light gains `spare_light_gain`, capped so `light` never exceeds `start_light()`. Route all sites through one helper `fn record_spare(&mut self)` (increments `spared`, adds capped light) so a future becalm site can't silently skip the light — same consolidation lesson as batch 11's awe helper.

- [ ] **Step 1: Failing test** — a becalm (via awe, the deterministic path) adds `spare_light_gain`, capped at start_light.
```rust
#[test]
fn sparing_feeds_light_capped() {
    let mut g = Game::new(11);
    g.light = 100; // low, so the gain is visible and uncapped
    let (ox, oy) = (g.px + 1, g.py);
    g.monsters.clear();
    g.monsters.push(crate::game::Monster { kind: OGRE, x: ox, y: oy, hp: 99, ..crate::game::Monster::spawn(OGRE, ox, oy) });
    let thr = crate::game::Monster::stats(OGRE).awe_threshold as usize;
    let light_before_each: Vec<i32> = { let mut v = vec![]; for _ in 0..thr { v.push(g.light); g.apply_input(4); } v };
    assert!(g.monsters.iter().any(|m| m.calm), "ogre becalmed via awe");
    // on the becalm turn, light gained spare_light_gain (minus that turn's base burn)
    // exact arithmetic checked against GAME.balance in the assertion:
    let _ = light_before_each;
    assert!(g.light > 100 - (thr as i32) * GAME.balance.base_burn,
        "sparing must have net-added light vs. pure burn");
}

#[test]
fn spare_light_is_capped_at_start_light() {
    let mut g = Game::new(11);
    g.light = start_light(); // already full
    // becalm something and assert light never exceeds start_light
    // (drive an awe becalm as above); assert g.light <= start_light() throughout.
}
```
(Refine the arithmetic to the exact `GAME.balance` values when implementing — the assertions must be exact, not `>`; the sketch shows intent. The cap test must assert `light <= start_light()`.)
- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3:** Add `BalanceDef.spare_light_gain: i32` (doc: "light gained each becalm — the only renewable light source; the mercy half of light-as-grace [batch 12]; capped at start_light"). Initial `[TUNE]` `40` in contractor.rs.
- [ ] **Step 4:** Add `fn record_spare(&mut self)` in `game.rs`: `self.spared += 1; self.light = (self.light + GAME.balance.spare_light_gain).min(start_light());`. Replace every `self.spared += 1;` with `self.record_spare();` (find ALL of them — the grep showed at least 3). Verify none is missed.
- [ ] **Step 5: Run — PASS.** Goldens byte-identical.
- [ ] **Step 6: Commit** `feat: batch 12 T2 — spares feed your light, the only renewable source (capped)`.

---

## Task 3: Tune to flip diplomacy above violence + re-baseline all four bands (controller-driven)

**Files:** `src/games/contractor.rs` (`kill_light_penalty`, `spare_light_gain`, possibly START_LIGHT); `tests/{sim,pacifist,tactical,tactical-pacifist}-band.json`.

Iterative measure-and-adjust against the tactical bots. **Targets:** tactical-DIPLOMAT becomes the reliable path — **win_pct clearly above tactical-VIOLENT's** (violence stays ~40–50%, capped; diplomacy rises above it — the whole arc's thesis). The diplomat banks ~63k spares/5000 runs today, so `spare_light_gain` should lift it substantially; `kill_light_penalty` should keep or push violence down.

- [ ] **Step 1:** Measure all four policies at `--sim 5000 --report` with the initial values (penalty 8, gain 40). Record.
- [ ] **Step 2:** Adjust: raise `spare_light_gain` to lift the diplomat above violence; tune `kill_light_penalty` to hold violence ~40–50%. Re-measure after each change (log values → win rates). If light can't be made to bite within the START_LIGHT margin, reduce START_LIGHT (documented as an explicit re-derivation, and re-check `--solve 10000` stays 0 unwinnable since the geometric walk budget shrinks — this is the one thing that could make solve fail, so watch it).
- [ ] **Step 3:** Once tactical-diplomat > tactical-violent with both in sane ranges, freeze and re-baseline all four band files (bracket each measured value with headroom; each comment records the new baseline, the final `kill_light_penalty`/`spare_light_gain`/START_LIGHT, and that this is the authorized batch-12 re-baseline). Confirm greedy/pacifist land somewhere sane (they may rise a little as spares help them too).
- [ ] **Step 4: Commit** `feat: batch 12 T3 — tune light-as-grace so diplomacy overtakes violence; re-baseline all four bands`.

---

## Task 4: The McGuffin's mood is the fuel gauge (minimal surfacing)

**Files:** `src/games/contractor.rs` (McGuffin `carried_lines`, and/or a light-tier string) and/or `src/render.rs` (status-bar note). Keep it MINIMAL — the mechanic is the light delta; this only makes the connection legible.

The batch-8 McGuffin already fires `CarryEvent::KillWitnessed`/`SpareWitnessed` lines while carried. This task's minimal job: make the light connection legible — e.g. wire one or two held-out `MCG_` mood lines (batch-8 T2 held MCG_060/062 pending §9-E; they may now fit) that reference the McGuffin's mood/the light, OR a single light-tier log line. Do NOT build a new render subsystem. If nothing lands cleanly without inventing ungrounded copy, ship the mechanic with the existing kill/spare McGuffin voice and note that fuller mood-surfacing is deferred — grounded content only.

- [ ] **Step 1:** Decide the minimal surface (a held-out MCG_ line wired, or a light-tier note), implement it, add/extend a test that the line/gate fires deterministically.
- [ ] **Step 2:** Goldens byte-identical (flavor is runtime); grep-clean.
- [ ] **Step 3: Commit** `feat: batch 12 T4 — surface the McGuffin's mood as the light fuel-gauge (minimal)`.

---

## Task 5: Full gate + docs

- [ ] **Step 1:** `make UPX=$(command -v upx) check` FOREGROUND — green. Expect: dump + frame goldens byte-identical (NOT a worldgen MAJOR); `--solve 10000` 0 unwinnable (unchanged, or re-checked if START_LIGHT moved); all four sim bands passing at their new baselines; `xhash` unchanged (or a both-backends-agreeing new value if ref.sav's replay now kills/spares); size.
- [ ] **Step 2:** `CLAUDE.md` — doctrine bullet for light-as-grace (kill/spare light deltas, the winnability model note, the McGuffin-mood surface) + a dated batch-12 status-log entry: the economic flip (diplomacy now > violence, with the final numbers), the two BalanceDef knobs, the four-band re-baseline, START_LIGHT's fate, xhash, test counts, sizes. Reference the arc doc; note the deferred ride-alongs (goblin awe / cheese / dividend / NPC fix / trainer-memory / donkey) as batch 13.
- [ ] **Step 3: Commit** `docs: batch 12 — light-as-grace doctrine + status log`.

---

## Self-review checklist

- **Spec coverage:** kills dim ✓T1, spares feed ✓T2, tune-to-flip + re-baseline ✓T3, McGuffin-mood surface ✓T4, gate/docs ✓T5. All from the arc doc's "Engine 2 — light as grace".
- **MAJOR handled:** four bands re-baselined (T3), sign-off noted; NOT a worldgen MAJOR (goldens byte-identical — verify each task); START_LIGHT change (if any) documented + solve re-checked.
- **Winnability:** solver stays geometric/unchanged unless START_LIGHT moves (then re-check solve). Documented in Global Constraints.
- **No new hashed state:** light/kills/spared already hashed; xhash shifts only if ref.sav replays combat.
- **DRY:** one `record_spare` helper so no becalm site skips the light gain.
- **Type consistency:** `kill_light_penalty: i32`, `spare_light_gain: i32`; capped at `start_light()`.
- **Deferred set** (goblin awe / cheese / dividend / NPC fix / trainer-memory / donkey) explicitly out of scope — batch 13.
