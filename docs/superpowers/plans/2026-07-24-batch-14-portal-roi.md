# Batch 14 — portal ROI (authored-floors MVP) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give portals a payoff — a portal's authored-floor destination holds a light-cache reward scaled to its dive risk, telegraphed truthfully at the threshold — so the multiverse becomes a pilgrimage the light-rich (merciful) player can afford and the slaughter-route player cannot.

**Architecture:** Pure content + a new walk-over item effect, all in the authored-floor destinations (which are NOT in the root-world goldens) and the portal describe line (a `StringsDef`). The root world's worldgen is untouched → **NOT a worldgen MAJOR** (every task verifies root goldens byte-identical). Bots ignore portals (transit needs an explicit wait no bot emits), so `--solve`/`--sim` are structurally unaffected — portal ROI ships as human-facing exploration content, not a sim-measured balance lever.

**Tech Stack:** Rust, the existing `AUTHORED_FLOORS`/`instantiate_floor` machinery, the `Dest::Floor` portal path, the `bfs_dist` round-trip budget (the model for the dive-cost probe).

## Provenance / sign-off

Batch 14 = the arc doc's original item-4 "portal ROI" (renumbered — the 2026-07-24 redesign split light-as-grace and the ride-alongs into batches 12/13). Human sign-off 2026-07-24: **Option 1 (authored-floors MVP), proceed autonomously**, with three required additions (below). Dest::World enrichment is pre-approved IN CONCEPT as the follow-on but is its OWN worldgen MAJOR needing separate sign-off with this batch's probe data in hand — NOT built here. No portal-diving bot this batch.

## Global Constraints

- **Shipped binary + assets ≤ 1,474,560 bytes UPX-packed.** Report stripped+packed each task. (Current: 211,016 packed, 14.31%.)
- **NOT a worldgen MAJOR.** Root-world dump goldens (seeds 1/2/3/42/1337) + frame goldens MUST stay byte-identical every task — the reward lives only in authored-floor DESTINATIONS (never dumped in the root goldens) and the telegraph is a `StringsDef` line. Any task that moves a root golden has overstepped scope — stop and re-scope.
- **`--solve 10000` and all four `--sim 5000` bands UNCHANGED.** Bots never dive portals; portal ROI cannot touch them. Assert it.
- **Engine grep-clean of game nouns** — the light-cache item, its glyph, its value all live in `contractor.rs`; the engine reads a generic `ItemEffect`. No `cache`/`floor`-noun literal in `game.rs`/`gamedef.rs`/`headless.rs`/`save.rs`/`render.rs`.
- **Grounding:** the threshold telegraph may promise a cache ONLY when the engine actually places one in that destination (addition #3). No lying about rewards.

## The three required additions (house-style, from the sign-off)

1. **Cache values are DERIVED from a geometric dive-cost probe, never hand-picked** (T2). Model it on `solve_seed`'s round-trip BFS budget: for an authored floor, the dive cost in light ≈ the round-trip walk from the floor's entry `<` to the cache and back (light burns 1/turn). A cache is +EV iff its value exceeds that cost. The derivation is documented at the value's definition site, exactly as `START_LIGHT`'s comment documents its solver derivation. A shallow/safe floor's cache clears its (small) dive cost with margin (+EV for the attentive); a deep/dangerous floor's is the gamble (bigger cache, bigger cost/risk).
2. **A NAMED per-run cache-light cap guards the brute-bailout failure mode** (T3), sized from sim shortfall data. The risk: a human brute (bleeding light to the `kill_light_penalty` and a dark McGuffin) uses portal caches to REFILL light and escape the light-as-grace punishment. The cap (`MAX_CACHE_LIGHT_PER_RUN` or similar, a `BalanceDef` value) bounds total cache light a run can collect BELOW what a slaughter route burns — so caches offset only a fraction of a brute's self-inflicted light loss (a merciful player, not burning light on kills, keeps the full benefit). Sizing derivation: measure a brute's light loss from the existing sim data (`kill_light_penalty` × the tactical bot's median kills/run, and the losing bots' dark-death light shortfall) and set the cap to a documented fraction below it. The cap needs a hashed per-run counter (`Game::cache_light_collected` or similar) → `SAVE_VERSION` bump + `state_hash` + `parse_save` back-compat.
3. **The threshold telegraph is grounded against actual cache presence** (T4). The portal describe line (`portal_describe_floor`) names the reward ONLY if the destination floor actually contains a cache — the portal "already knows its destination," so it can truthfully preview it; it must never promise a cache a barren floor won't deliver.

---

## Task 1: The light-cache item (walk-over light refill)

**Files:** `gamedef.rs` (`ItemEffect::LightCache(i32)`), `game.rs` (`Game::pickup`'s walk-over arm applies it, capped — cap lands in T3, a placeholder/uncapped add here with a TODO pointing at T3), `contractor.rs` (the light-cache item def + glyph), `main.rs` (tests).

**Interfaces:**
- Consumes: the existing walk-over `Consume` pickup path (`ItemEffect` is read there), the `light` pool.
- Produces: `ItemEffect::LightCache(i32)` — walk over a cache, `light += value`.

- [ ] **Step 1:** Failing test — walking onto a light-cache item raises `light` by its value (construct a game, place a cache adjacent, walk onto it, assert light rose). A `Consume` item, vanishes on pickup.
- [ ] **Step 2:** Add `ItemEffect::LightCache(i32)`; wire it in `pickup`'s `Consume` match (grep how `Heal`/`AtkBonus` walk-over effects apply). The value is a placeholder here (T2 derives it); leave a `// [DERIVED in T2]` marker. Cap is uncapped here (`// [CAPPED in T3]`).
- [ ] **Step 3:** Add the cache item to `contractor.rs`'s item table (a new `IKind`, its own glyph — a distinct, readable dump char not colliding with existing legend; `#[allow(dead_code)]` on the const index if not yet placed). Grounded pickup line.
- [ ] **Step 4:** Tests green; ROOT goldens byte-identical (the cache isn't placed in any root depth); build clean. **Commit** `feat: batch 14 — the light-cache item (walk-over light refill)`.

---

## Task 2: The geometric dive-cost probe — derive the cache values

**Files:** `headless.rs` (a probe fn measuring per-floor dive cost) OR a `#[test]`-based derivation, `contractor.rs` (cache values set from the probe), the value's doc comment (the derivation).

**Interfaces:**
- Consumes: `bfs_dist`, `instantiate_floor` (to build a floor and measure it), the authored-floor maps.
- Produces: documented, derived cache values per floor; a `--dump-floors` or probe surface if useful for eyeballing.

- [ ] **Step 1:** Add a probe: for each authored floor, instantiate it, BFS from the entry `<` to the cache tile and back = the round-trip dive cost in light (mirror `solve_seed`'s `bfs_dist` round-trip shape). A `--probe-floors` headless flag (or a `#[test]`) that prints/asserts each floor's dive cost.
- [ ] **Step 2:** Set each floor's cache value = its dive cost + a documented margin (shallow floors clear cost with healthy margin = +EV; deep floors bigger cache for bigger cost). Document the derivation at the value's definition site, `START_LIGHT`-comment style (the exact BFS numbers + the margin rationale).
- [ ] **Step 3:** Tests/probe green; ROOT goldens byte-identical; `--solve 10000` unchanged. **Commit** `feat: batch 14 — derive cache values from the geometric dive-cost probe`.

---

## Task 3: The anti-brute-bailout guard (named per-run cache-light cap)

**Files:** `gamedef.rs`/`contractor.rs` (`BalanceDef::max_cache_light_per_run` + its sizing comment), `game.rs` (hashed `Game::cache_light_collected`, the cap applied at cache pickup), `save.rs` (`SAVE_VERSION` bump, `state_hash`, back-compat), `main.rs` (tests).

- [ ] **Step 1:** Failing tests — collecting caches raises `cache_light_collected` and refunds light UP TO the cap; past the cap, further caches refund nothing (or the remainder to the cap). `cache_light_collected` IS hashed (two games differing only in it hash differently). An older-version save replays byte-identical under the new version.
- [ ] **Step 2:** Add the hashed `cache_light_collected` counter + `max_cache_light_per_run`. Apply the cap in the cache-pickup path (T1's refund becomes `light += min(value, max_cache_light_per_run - cache_light_collected)`). `SAVE_VERSION` bump (currently 7 → 8), `parse_save` `1..=8`, add to `state_hash`.
- [ ] **Step 3:** SIZE the cap from sim shortfall data: measure `kill_light_penalty × tactical median kills/run` (the light a slaughter route self-burns) and the losing bots' dark-death light shortfall; set the cap to a documented fraction BELOW the brute's self-inflicted loss so caches can't rescue a slaughter route. Document the derivation.
- [ ] **Step 4:** Tests green; ROOT goldens byte-identical; `--sim 5000` all four UNCHANGED (bots don't dive, so the cap/counter never fires in sim — assert it); build clean. **Commit** `feat: batch 14 — cap per-run cache light (anti-brute-bailout guard)`.

---

## Task 4: Place caches in authored floors (scaled to risk) + the grounded telegraph

**Files:** `contractor.rs` (cache glyphs in the `AUTHORED_FLOORS` maps, scaled to each floor's risk; possibly 1–2 new floors for risk variety; `portal_describe_floor` telegraph), `game.rs` (the telegraph reads actual cache presence), `main.rs` (tests).

- [ ] **Step 1:** Place a cache glyph in each authored floor's map ASCII, positioned/valued to its risk (a shallow safe floor: a modest cache near the entry, +EV; a deeper/guarded floor: a bigger cache behind risk). Add 1–2 new authored floors if needed for a real shallow-vs-deep contrast (authored ASCII, same convention — NOT worldgen, root goldens unaffected). `--probe-floors`/`--dump` a floor to eyeball.
- [ ] **Step 2:** The telegraph (addition #3): `portal_describe` names the cache in the threshold line ONLY when the destination floor actually contains one — grep how `portal_describe` builds the `Dest::Floor` line; gate the cache-hint clause on the floor's actual cache presence (a `AuthoredFloorDef` flag or a scan of its map). A barren floor telegraphs as before. Grounded, ASCII, ≤78.
- [ ] **Step 3:** Failing/passing tests — transiting to a cache floor logs the cache-promising telegraph AND the cache is actually there; a barren floor telegraphs no cache; the cache value matches the derived value. ROOT goldens byte-identical.
- [ ] **Step 4:** Build clean; tests green. **Commit** `feat: batch 14 — caches in authored floors, scaled to risk; the grounded threshold telegraph`.

---

## Task 5: Full gate + docs

- [ ] **Step 1:** `make UPX=$(command -v upx) check` FOREGROUND — GREEN. Confirm the NOT-a-MAJOR claim: root dump+frame goldens byte-identical; `--solve 10000` unchanged; all four sim bands unchanged; `xhash` shifts ONLY from the T3 hashed counter (SAVE_VERSION 8), both backends agree; size.
- [ ] **Step 2:** `CLAUDE.md` — doctrine bullet (portal ROI: light caches in authored-floor destinations, values geometric-dive-cost-derived, the anti-bailout cap, the grounded telegraph, NOT a MAJOR because destinations aren't goldened and bots don't dive); frontier updated (batch 14 done; the Dest::World enrichment follow-on + its pending MAJOR sign-off flagged, with this batch's probe data now available); status-log entry with the derived cache values + the probe numbers + the cap sizing + xhash/sizes/tests. Playtest-pending: does the telegraph make the dive decision legible; does a light-rich pacifist actually feel the pilgrimage pay off while a brute can't afford it.
- [ ] **Step 3: Commit** `docs: batch 14 — portal-ROI doctrine + status log`.

---

## Self-Review
- Scope = authored-floors MVP, NOT a worldgen MAJOR (root goldens byte-identical every task — the load-bearing invariant; confirm at T5). Dest::World enrichment explicitly deferred to its own MAJOR sign-off.
- The three sign-off additions each have a home: derived cache values ✓T2, named anti-bailout cap ✓T3, grounded telegraph ✓T4.
- New hashed state (`cache_light_collected`) → `SAVE_VERSION` 7→8 + back-compat + `state_hash` (T3).
- Bots never dive → `--solve`/`--sim` structurally unaffected; assert unchanged every task.
- No portal-diving bot this batch (deferred with the Dest::World follow-on).
