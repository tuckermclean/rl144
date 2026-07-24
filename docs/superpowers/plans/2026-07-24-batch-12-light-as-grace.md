# Batch 12 — Light as Grace: the McGuffin as a Second Lantern (REDESIGNED) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

> **This plan was REWRITTEN 2026-07-24 to the human's signed-off resolution of the batch-12 impasse** (see the resolution doc in the conversation / the arc doc's amended "Engine 2 — light as grace"). It supersedes the original T2–T5. **T1 (kills dim light, commit `3cdee5e`) stays.** The A/B fork (spare→HP vs. nerf-violence) is REJECTED; the McGuffin becomes a *positional light source whose radius is set by her verdict on your record*.

**Goal:** Make mercy the reliable path by lifting the diplomat's *ceiling*, not by stipend: the McGuffin, picked up at the bottom, anchors her opinion to your kill/spare record and thereafter shines a light of mood-set radius from her own tile — so a merciful player can finish the climb walking in her light after the torch dies, while a brute gets radius ~0 and dies in the dark exactly as today.

**Architecture:** Descent unchanged (kills still dim light; mercy pays nothing yet). Waiting while hurt heals slowly (HP was the diplomat's real bottleneck). At pickup, snapshot kills/spared into two new hashed ints (SAVE_VERSION bump). Her shine composes with the torch in FOV; the death check becomes "no light reaches you" (torch dead AND outside her radius). Tune so tactical-diplomat lands ~[50,60], tactical-violent high-30s–low-40s. Mood-keyed line tables ARE the telegraphing.

**Tech Stack:** Rust (rustc 1.75 — now enforced), `src/game.rs` (rest-heal, anchor, death check), `src/render.rs` (composite FOV/light), `src/gamedef.rs`+`contractor.rs` (mood tiers, line tables), `src/save.rs` (SAVE_VERSION, hashed anchor), `src/headless.rs` (bot rest branches), the bands + goldens + `ref.sav`.

## Global Constraints

- **BALANCE MAJOR (sign-off granted 2026-07-24):** all four sim bands re-baseline. **NOT a worldgen MAJOR:** dump + frame goldens stay byte-identical — if they drift, presentation/economy leaked into generation; stop and fix.
- **SAVE_VERSION BUMPS this batch** (5 → 6): the anchor snapshot (two new hashed ints) is run-defining. `parse_save` must accept `1..=6` and replay older byte-identically. **`ref.sav` regenerates**, `xhash` shifts (both backends must agree). This intentionally amends the original plan's "no new hashed state / no SAVE_VERSION bump" clause — do NOT violate it silently; it is amended here on purpose.
- **Solver stays GEOMETRIC and its guarantee is now conservative by design:** it still proves the pure walk fits START_LIGHT; her shine only ever ADDS survivability on top, never subtracts. `--solve 10000` stays 0 unwinnable. Document the conservatism.
- **Grounding doctrine binds hard:** her mood and shine radius are engine state; every line about her tending/dimming/refusing is a verifiable fact. Rest copy frames RELATIONSHIP, not economy — register: *"There is not much left. It warms you with it anyway."* Never write "light heals you"; status bars state the exchange, lines state the relationship. Mood-keyed line tables (bright/dim × rest/refuse/ascent) are IN this batch — they ARE the telegraphing, not deferred flavor.
- **Telegraphing is mandatory:** the pickup-verdict must be legible BEFORE depth 5 (trainer / lore tiers / her descent-phase lines). Nobody may learn "she judges you at the bottom" by dying in the dark feeling cheated. The depth-5 warning MUST land this batch; trainer/lore hooks MAY stub toward batch 13's trainer-memory.
- **Band targets (the batch-5 ghost police):** tactical-diplomat → ~[50,60]; tactical-violent → high-30s–low-40s (T1 stays, so near current 37.9%); greedy/pacifist stay the <1% floor. **If diplomacy crests ~65%, rest-rate or her radius is too generous — a number, not a debate.**
- Cartridge doctrine: engine grep-clean of game nouns; mood tiers/radii/lines are `GameDef` data. Explicit-width hashed ints. Bots pure (no RNG). Size budget.
- Design canon: `docs/design/2026-07-22-mercy-economy-arc.md` §"Engine 2 — light as grace" (amend it in Task 2 to this design).

---

## Task 1: THIRD STRIKE — pin rustc 1.75 + fix the recurring E0716 (FIRST COMMIT)

**Files:** `Cargo.toml` (`rust-version`), `Makefile` (pinned-1.75 build step in `check`), `src/headless.rs` (the E0716 match-arm-temporary fix).

The E0716 (temporary value dropped while borrowed) match-arm break has been reported twice, fixed-in-review twice, never committed, and DUPLICATED by batch 10 into two new `sim_main` band-key arms. Land the fix AND the enforcer together so it can never recur silently.

- [ ] **Step 1:** Add `rust-version = "1.75"` under `[package]` in `Cargo.toml`.
- [ ] **Step 2:** Fix the E0716 in `sim_main`'s band-key match (`src/headless.rs`, the `Policy::* => &[("win_pct", ...), ...]` arms). Bind the arrays BEFORE the match and collapse to two arms:
```rust
let with_dark = [("win_pct", win_pct), ("deaths_dark", deaths_dark as i32)];
let without_dark = [("win_pct", win_pct)];
let checks: &[(&str, i32)] = match policy {
    Policy::Greedy | Policy::Tactical => &with_dark,
    Policy::Pacifist | Policy::TacticalPacifist => &without_dark,
};
```
(Adapt names/types to the real code; the point is the temporaries become named locals whose lifetime covers the match.)
- [ ] **Step 3:** Add a pinned-1.75 build to `make check` — a step that builds (or `cargo +1.75 check`, or a `RUSTFLAGS`/toolchain guard) against 1.75 so the MSRV is actually exercised in the gate. If no `+1.75` toolchain is installable here, at minimum add `cargo check` under the declared `rust-version` and document how CI pins it; the enforcer must FAIL on a reintroduced E0716.
- [ ] **Step 4:** `cargo build --release` clean; `make check`-equivalent build steps green; confirm the two collapsed arms still gate identically (greedy/tactical check win_pct+deaths_dark; pacifist/tactical-pacifist check win_pct only). Goldens/bands untouched.
- [ ] **Step 5: Commit** `fix: batch 12 — pin rustc 1.75 + kill the recurring E0716 band-key match temporary (third strike)`.

---

## Task 2: Strip T2's light-gain (keep the helper); amend the docs

**Files:** `src/gamedef.rs` (remove `spare_light_gain`), `src/games/contractor.rs` (remove its value), `src/game.rs` (`record_spare` drops the light line, keeps `spared += 1`), the plan doc + arc doc.

- [ ] **Step 1:** Remove `BalanceDef.spare_light_gain` (field + contractor value). In `record_spare`, drop the `self.light = (…).min(start_light())` line, keeping `self.spared += 1;`. **Keep `record_spare` as the one consolidated spare site** — the mood/anchor work wants exactly one.
- [ ] **Step 2:** Delete/adjust T2's now-obsolete tests (`sparing_feeds_light`, `spare_light_is_capped_at_start_light`) — mercy no longer feeds light. Leave a test that `record_spare` still counts `spared` (the consolidation invariant).
- [ ] **Step 3:** Amend the arc doc's "Engine 2 — light as grace" section to the second-lantern design (descent honest; rest-heal; pickup verdict; positional shine + new death check; cheese as poor-man's torch). Amend this plan's Global Constraints note about SAVE_VERSION (done above).
- [ ] **Step 4:** Build clean, goldens byte-identical, `cargo test` green. **Commit** `refactor: batch 12 — strip spare-light stipend (keep record_spare); mercy pays at the top now`.

---

## Task 3: Rest — waiting while hurt heals slowly (+ both bots' rest branch)

**Files:** `src/game.rs` (wait handler), `src/gamedef.rs`+`contractor.rs` (`rest_heal` [TUNE]), `src/headless.rs` (bot rest branches); tests.

**Design:** on a `wait` (byte 4), if `hp < maxhp` AND no non-calm hostile is cardinally adjacent AND (light guard) → `hp = min(maxhp, hp + rest_heal)`. `rest_heal` [TUNE] start 1 (or 1-per-2-turns if bands want). No new verb/byte. Rest-heal ONLY when no hostile cardinally adjacent — so awe-holding (standing tall while an ogre pummels you) stays its OWN act and rest doesn't muddy the awe read ([AGENT'S CALL], taken).

- [ ] **Step 1:** Failing test — waiting while hurt with no adjacent hostile heals `rest_heal`; waiting adjacent to a live hostile does NOT heal (awe-hold stays clean).
- [ ] **Step 2:** Implement in the wait handler. **Portal-footing guard:** wait-on-a-portal transits (batch 6). Rest-heal must not entangle with that — the heal is a separate effect; confirm a wait that heals still transits if on a portal (or explicitly doesn't — pick the clean rule and test it), and that the transit path is unaffected.
- [ ] **Step 3:** Both tactical bots get a SYMMETRIC rest branch: `hp` below a threshold AND no adjacent non-calm hostile AND light above a reserve → emit wait (byte 4). **The bots must never emit wait while standing on a portal** (preserve `bot_never_transits`) — guard it, add/extend the assertion. Keep the branch identical between the violent and diplomat bots; outcome asymmetry must come from the economy, not bot code.
- [ ] **Step 4:** Tests green, goldens byte-identical, `bot_never_transits` still holds. **MEASURE rest alone** (all four policies at `--sim 5000`, report) and record — so rest's effect is separable from her shine in the log. **Commit** `feat: batch 12 — rest: waiting while hurt heals slowly (+ bot rest branch, portal-guarded)`.

---

## Task 4: The pickup verdict — anchor snapshot + mood function (SAVE_VERSION bump; SIGN-OFF on the formula)

**Files:** `src/game.rs` (snapshot at pickup, `mood()` derivation), `src/save.rs` (SAVE_VERSION 5→6, hash the anchor, accept 1..=6), `tests/fixtures/ref.sav` (regen), `src/gamedef.rs`+`contractor.rs` (mood tiers).

- [ ] **Step 1:** At amulet/objective pickup, snapshot `kills` and `spared` into two new hashed `Game` fields (`anchor_kills`, `anchor_spared`, explicit width). Hash them in `state_hash` beside `regard`/etc. `SAVE_VERSION` 5→6; `parse_save` accepts `1..=6`, older logs replay byte-identically (test it). Regenerate `tests/fixtures/ref.sav` (a session that reaches pickup, or keep the current short one and note xhash shift source); `xhash` shifts, both backends agree.
- [ ] **Step 2:** `fn mood(&self) -> i32` (or a tier enum): pure, derived from hashed state only — the anchor PLUS post-pickup conduct on the ascent, which has THREE inputs: post-pickup kills (dim her), post-pickup spares (brighten her), and — **the human's addition, 2026-07-24 — a bounded mood lift EACH TIME she sees a becalmed monster on the climb** ("every time she sees a becalmed monster, she likes you that much more"). This IS the becalm return-trip dividend, reimagined as MOOD/radius income rather than a light stipend: the monsters you spared on the descent literally brighten the way home as you pass them, tile by tile. Farming-guarded — once per monster (a per-becalmed-monster "greeted-by-the-McGuffin" flag, hashed, or an equivalent guard; a monster can't re-brighten her by pacing past it). "Sees" = within her shine radius / adjacent to the carried McGuffin — pick the clean rule and test it. **CONSTRAINT (human-set): late redemption is permitted but anchored LOW** — a brute who climbs mercifully brightens *some*, never to pure-diplomat shine; a pure diplomat's anchor is max (full shine). **This is a SIGN-OFF point:** write 2–3 candidate `mood()` formulas into the brief/report with their band consequences (how each maps to radius tiers and what that predicts for the diplomat/violent win rates), and STOP for the human to pick before wiring the final one. Do not pick unilaterally.
- [ ] **Step 3:** Once the human picks a formula, wire it + the mood→tier mapping (`GameDef` data: tier → shine radius). Tests: anchor snapshots correctly at pickup; mood is deterministic/hashed; a pure-diplomat run anchors max, a pure-brute anchors min; the anchored-low-redemption constraint holds.
- [ ] **Step 4: Commit** `feat: batch 12 — the pickup verdict: her opinion anchors to your record (SAVE_VERSION 6)`.

---

## Task 5: She is a light source — positional shine + the death-condition rewrite (OWN REVIEW)

**Files:** `src/render.rs` (composite FOV/light from her tile), `src/game.rs` (death check rewrite), End-screen message; probe tests. **This touches the single most-playtested check in the game — own task, own review, probe tests.**

- [ ] **Step 1:** Her shine = a second light source centered on HER tile, radius = mood-tier radius (Task 4). Compose with the torch in FOV/visibility (`render.rs` — a tile is lit if within torch radius OR within her radius). Radius-from-her-position IS the implementation of "the room she's in" (rooms aren't first-class at runtime).
- [ ] **Step 2: The death rewrite.** Today death = `light <= 0`. Now death = **no light reaches you**: torch dead (`light <= 0`) AND you are outside her lit radius. Consequences, all intended: a max-shine diplomat finishes the climb after the torch dies, walking in her light (THE FLIP); a mood-zero brute gets radius ~0 and dies exactly as today (T1's nerf preserved). Keep lose-before-win ordering; keep it deterministic/hashed-only.
- [ ] **Step 3: Put-down becomes strategy** (batch 8's byte 16): setting her down → 1× burn (already reverts via `has_objective`), but her light stays at her tile. Torch-dead and two rooms from her → the dark gets you. **Name the shuttle (park / scout at 1× / return) as intended play in the doc, not an exploit** — self-limiting (leaving her light is the risk); bots needn't do it, humans will.
- [ ] **Step 4:** End-screen death message distinguishes "the dark got you" (torch dead, no McGuffin help) from "the dark got you, two rooms from her light" (torch dead, she was shining but you strayed) — IF the `killer`/`echo` machinery can state it groundedly (presentation-only; don't force it if it can't be grounded).
- [ ] **Step 5: Probe tests:** a lit-by-her tile survives a dead torch; a tile outside her radius with a dead torch is death; put-down-then-stray kills you; the shuttle works. Goldens byte-identical (render/death are runtime). **Commit** `feat: batch 12 — the McGuffin is a light source; death is now "no light reaches you"`.

---

## Task 6: Tune to targets + re-baseline all four bands (controller-driven)

**Targets:** tactical-diplomat ~[50,60]; tactical-violent high-30s–low-40s; greedy/pacifist <1% floor. Knobs: `rest_heal` rate, her shine radii per mood tier, the mood formula's steepness (within the signed-off shape). If diplomacy crests ~65% → radius/rest too generous, pull back (a number, not a debate).

- [ ] **Step 1:** Measure all four at `--sim 5000 --report`. Adjust radii/rest toward the targets; re-measure each change (log values→rates).
- [ ] **Step 2:** Freeze; re-baseline all four band files (bracket measured + headroom; comments record the new baseline, the final rest/radius/mood values, START_LIGHT unchanged, and the batch-12 sign-off). Confirm `--solve 10000` still 0 unwinnable.
- [ ] **Step 3: Commit** `feat: batch 12 — tune second-lantern economy so diplomacy is the reliable path; re-baseline four bands`.

---

## Task 7: Mood/rest/ascent line tables + the telegraphing pass

**Files:** `contractor.rs` (mood-keyed `carried_lines`/new line tables), the depth-5 warning, trainer/lore hooks (may stub → batch 13).

- [ ] **Step 1:** Mood-keyed line tables: bright/dim × rest/refuse/ascent. Grounded — every line a verifiable fact about her mood/shine/tending. Register: relationship, not economy. Wire held-out `MCG_` lines where they fit; author grounded new ones where they don't; hold out anything ungrounded (batch-8 discipline).
- [ ] **Step 2:** The depth-5-warning telegraphing MUST land: before pickup, the player must have been told (trainer line and/or lore tier and/or her descent lines) that she will judge them. Trainer/lore hooks may be stubs pointing at batch-13 trainer-memory, but the warning itself lands now.
- [ ] **Step 3:** `talk_lines_fit_log_row`/ASCII/≤78-char guards pass; goldens byte-identical. **Commit** `feat: batch 12 — mood/rest/ascent line tables + depth-5 telegraphing (grounded)`.

---

## Task 8: Full gate + docs

- [ ] **Step 1:** `make UPX=$(command -v upx) check` FOREGROUND (now incl. the 1.75-pinned step) — green. Goldens+frames byte-identical; `--solve 10000` 0 unwinnable; four bands at new baselines; `xhash` new value (SAVE_VERSION 6 + anchor), both backends agree; `parse_save` 1..=6; size.
- [ ] **Step 2:** `CLAUDE.md` — doctrine: the death-condition rewrite ("no light reaches you"), the McGuffin as a second positional light source, the pickup anchor (hashed, SAVE_VERSION 6), rest-heal, cheese-as-poor-man's-torch. Status-log entry: the second-lantern redesign, the economic flip with final numbers, the mood formula chosen, the four-band re-baseline, SAVE_VERSION/xhash, the third-strike enforcer, test counts, sizes. Playtest-pending: does the verdict-at-pickup read as judgment or ambush; does resting feel like being tended; does the brute's cheese-lit climb land as tragicomic.
- [ ] **Step 3: Commit** `docs: batch 12 — second-lantern doctrine + status log`.

---

## Self-review checklist
- Five commitments covered: descent-honest+strip-stipend ✓T2, rest ✓T3, pickup-verdict ✓T4, positional-shine+death-rewrite ✓T5, cheese-as-torch (no new mechanic; noted) ✓doctrine. Third strike ✓T1.
- SAVE_VERSION 5→6 (T4), `parse_save` 1..=6, ref.sav regen, xhash shift documented. NOT a worldgen MAJOR (goldens byte-identical — every task verifies).
- Solver conservatism documented; her shine only adds.
- Sign-off point: the mood formula (T4 Step 2) — present candidates, STOP for the human.
- Bands: diplomat [50,60], violent high-30s-low-40s, floors <1%; >65% diplomat = pull back.
- Telegraphing (depth-5 warning) lands this batch (T7). Grounding register enforced.
- Deferred to batch 13: goblin awe, cheese→goblin gift, becalm return-dividend, NPC category fix, trainer-memory (full), donkey-follow.
