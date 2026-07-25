# Batch 15 — playtest refinements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development.

**Goal:** Two playtest-driven fixes: (1) goblin awe must be earned by *talking while giving ground*, not a silent retreat, and never against a goblin that has struck you; (2) a real item-use UI — select which held item to use and see how many you have.

**Source:** direct human playtest feedback, 2026-07-25.

## Global Constraints
- ≤1,474,560 bytes UPX-packed (current 212,112). Engine grep-clean of game nouns. Every tuning number measured against the tactical bots (the arc's iron discipline). Root/dump goldens stay byte-identical (neither feature touches worldgen).

---

## Task 1: Goblin awe — talk-gated, and dead once it has struck you

**The bug:** a *silent* give-ground move currently awes a goblin (`resolve_awe`'s `gave_ground` branch for `awe_by_giving_ground` kinds), even one that just hit the player. Per the arc ("walk backward WHILE talking") and the human: awe must require an active talk gesture while retreating, and a goblin that has landed a hit can't be awed at all.

**Design (engine):**
- New hashed `Monster.struck_player: bool` — set the instant this monster lands ANY hit on the player (the three sites: `monsters_act`'s attack loop, `resolve_awe`'s punish hit, the ogre `retaliation` hit). For a give-ground kind (goblin), `struck_player == true` forces `awe = 0` and blocks any further awe — you must fight, flee, or cheese it. (Hold kinds / the ogre are NOT gated on this: enduring its hit IS its awe.)
- New hashed `Monster.yielded: bool` — set true in `resolve_awe` when the player GIVES GROUND to this goblin (`gave_ground`), cleared when the player holds/advances or when the awe is consumed. A give-ground move NO LONGER directly builds awe; it only arms `yielded`.
- The awe now builds on a TALK: `resolve_awe` gains a `talked: Option<usize>` param (the monster the player talked this turn — thread it from `try_talk_player` the way `attacked` is already threaded). For a give-ground kind, if `talked == Some(i)` AND `yielded` AND `!struck_player` → `awe += 1` (becalm at threshold via the existing `calm`/`record_spare` path), then clear `yielded`. So the two-beat rhythm is: **give ground → goblin chases back → talk it → awe builds → repeat.**
- **Talking a goblin is composure, not planting:** `held_adjacent` (the punished move for a goblin) must also exclude `talked == Some(i)` — a talk turn is not "standing planted," so it isn't punished. (Today a talk turn reads as `held_adjacent` → punished, which makes "talk while backing away" impossible; this is required for the mechanic to work at all.)
- `Monster.struck_player`/`yielded` are run-defining → `state_hash` + `SAVE_VERSION` bump (8→9) + `parse_save` `1..=9` + a back-compat test. `awe_tell` telegraph unchanged.

**Design (bot — `headless.rs`, essential or the diplomat craters):** the `TacticalPacifist` bot's cornered-goblin handling must become the retreat-then-talk rhythm: when adjacent to a give-ground goblin it can't route around, ALTERNATE giving ground (step away) and talking, rather than the current one-shot. Deterministic, pure function of state. The violent bot and greedy/pacifist are untouched.

**Tests:** silent retreat does NOT awe a goblin; give-ground-then-talk DOES; a goblin that has struck the player can never be awed (even with the talk+retreat rhythm); talking a goblin is not punished as planting; the ogre is unchanged (enduring its hit still awes); a lethal punish still attributes `killer`; older-version save replays byte-identical under v9.

**Sim + bands:** this is combat-band-moving. Measure all four `--sim 5000`; the diplomat will likely drop (goblins are harder now). Re-baseline the tactical bands to the measured values if they move, keeping the flip (diplomat > violent) and `stuck == 0`; greedy/pacifist untouched. If the flip breaks, STOP and report (it's a design signal).

- [ ] Implement engine + bot + tests, measure, re-baseline. **Commit** `feat: batch 15 — goblin awe is earned by talking while giving ground, and dies once it has struck you`.

---

## Task 2: Item-use UI — select which item, show counts

**The gap:** USE (byte 15) blindly applies `held.last()` (LIFO top); no way to choose, no count display.

**Design (engine):** add a "use item of a chosen kind" path. The held list stays the hashed LIFO `Vec<u8>`; add `Game::use_item_kind(kind)` that finds and applies the TOP-MOST held item of that kind (or a no-op feedback line if none/unusable). Input encoding: extend the vocabulary with **use-by-selection** — the cleanest minimal encoding is a small fixed set (the frontend resolves a menu pick to a "use held-slot N" or "use kind K" byte). Vocabulary grows past 16 (a deliberate platform-boundary change — document it, bump `SAVE_VERSION` if the byte space changes what a replay can contain, keep `parse_save` back-compat). Keep byte 15 (use-top) working for back-compat.
- A `held_summary()` helper (derived, presentation-only): the held items grouped by kind with counts, for the status/log/menu to render.

**Design (frontends — `backend_minifb.rs` + `backend_term.rs`):** a use-selector: pressing the use key opens a small overlay/log listing held items with counts and a select key per entry; selecting emits the corresponding use-input byte. The count display (`Potion ×2  Cheese ×1`) shows in the status band or the selector. The selector's armed/disarmed state is frontend-local chrome (like the talk/give chords), never engine state.

**Tests:** `use_item_kind` applies the right kind and no-ops gracefully; `held_summary` groups/counts correctly; the new input byte(s) replay byte-identically; byte 15 still works.

**Verification note:** the frontend selector overlay is only verifiable in interactive play — FLAG it for human playtest; the engine path + counts are headless-testable.

- [ ] Implement engine + frontends + tests. **Commit** `feat: batch 15 — item-use selector + held counts (choose what to use)`.

---

## Task 3: Full gate + docs
- [ ] `make check` GREEN (bands re-baselined if T1 moved them). CLAUDE.md doctrine (the goblin awe two-beat + struck-player rule; the item-use vocabulary growth + selector) + status log + input-vocabulary bullet update. **Commit** `docs: batch 15 — playtest-refinement doctrine + status log`.

## Self-review
- T1 is combat-band-moving (re-baseline, keep the flip). T2 grows the input vocabulary (platform boundary — deliberate, documented). Neither is a worldgen MAJOR (goldens byte-identical). Frontend selector needs human playtest.
