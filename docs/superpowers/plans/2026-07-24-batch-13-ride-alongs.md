# Batch 13 — the mercy-economy ride-alongs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the six ride-along features the batch-12 redesign split out of the arc doc's original "batch 12" bundle, completing the mercy-economy arc's creature-by-creature diplomacy texture on top of the light-as-grace keystone.

**Architecture:** All six ride on already-built mechanisms (the give-table, the `Monster.awe`/`monsters_act_and_resolve_awe` machinery from batch 11, the `echo`-shaped presentation-carry, the `DONKEY_TALK` regard ladder, the light economy). Two are real new combat mechanics (goblin awe-by-giving-ground + the paired punish-behaviors; potion-enrage) and require a combat-balance sim re-baseline against the batch-10 tactical bots. The engine stays grep-clean (all creature-specific facts are cartridge data).

**Tech Stack:** Rust, the existing `GameDef`/`contractor.rs` cartridge, the tactical-bot instrument.

## Provenance / sign-off

Every feature here is spec'd in **`docs/design/2026-07-22-mercy-economy-arc.md`** (human-locked design canon), in its detailed sections — the arc doc's *numbered* roadmap (item 3) originally bundled these into "batch 12"; the 2026-07-24 human redesign split the light-as-grace keystone into the shipped batch 12 and pushed these ride-alongs to this batch. Portal ROI (arc item 4) moves to batch 14. **A docs task in this batch (T8) fixes the arc-doc roadmap numbering to reflect the split.**

Sign-off status: the mechanics are Rule-4 priced and human-locked as designs (arc doc §"Scope / sign-off": "a real new mechanic ... Rule-4 priced"). This batch is a **combat-balance batch** in the same sign-off class as batch 11 — it moves the four sim bands and must re-baseline them against the tactical instrument. It is NOT a worldgen MAJOR (no channel/layout/goldens change — every task verifies goldens byte-identical; only give-table/combat-behavior/dialogue data changes).

## Global Constraints

- **Shipped binary + assets ≤ 1,474,560 bytes UPX-packed.** Report stripped+packed after every task. (Current: 209,152 packed, 14.18%.)
- **Engine stays grep-clean of game nouns** — no `goblin`/`ogre`/`cheese`/`potion`/`donkey`/`trainer`/etc. as identifier or string literal in `rng`/`game`/`gamedef`/`headless`/`save`/`render`/`content`/backends. All creature-specific facts are `contractor.rs` data.
- **The measured-quantity discipline is the single most important process constraint** (arc doc): every tuning number — cheese becalm-roll odds, becalm dividend size, goblin-awe threshold, potion-enrage magnitude — is a `[TUNE]` value MEASURED against the tactical bots and re-baselined, NEVER hand-tuned to feel then band-regenerated around the feel. Start each at a documented `[TUNE]` value; T7 measures and re-baselines.
- **Telegraphing is mandatory** (arc doc §315): the goblin/ogre awe tells and the punish-behaviors must be legible so a wrong-move death is learn-by-death (carried by the resurrect→trainer-ribs-you loop), not a gotcha.
- **No input-vocabulary growth.** The vocabulary stays 0–16. Goblin awe is derived from the existing move+talk byte sequence (mirroring how the ogre's hold-vs-fled is read from a pre-chase snapshot), NOT a new byte. This is the [AGENT'S CALL] on the arc's open "awe input encoding" question — see T5.
- **Both saves and hashed state:** any new hashed field (e.g. a goblin-awe counter, if it can't reuse `Monster.awe`) bumps `SAVE_VERSION` and joins `state_hash`; presentation-only carries (the trainer's last-life memory) join the exclusion set like `echo`. `parse_save` keeps accepting all older versions.
- Every worldgen-adjacent claim verified by `--dump` goldens byte-identical + `--solve 10000` green; combat-balance claims by `--sim 5000` all four policies against re-baselined bands.

## Open design questions — resolved here as [AGENT'S CALL] (arc doc left these to "batch 12 with the instrument in hand")

1. **Goblin-awe input encoding** → reuse the ogre's exact machinery. Batch 11's ogre awe is read from a PRE-CHASE snapshot comparing the player's own move against the monster's position (`monsters_act_and_resolve_awe`). Goblin awe is the MIRROR: it builds when the player's own move that turn INCREASED distance to the goblin (gave ground) AND the player talked/held composure, and resets when the player held or advanced. No new input byte — same derive-from-move-sequence approach. (T5.)
2. **The paired punish-behaviors** → new per-kind `MonsterDef` flags, cartridge data: `punish_flight` (ogre — already has the guaranteed `retaliation`; formalize that fleeing an ogre is what its hit answers) and `punish_holding` (goblin — a planted target adjacent to a goblin eats a hit). Both ride the existing attack/retaliation path; the engine reads generic flags. (T5.)
3. **Potion-enrage mechanism** → a new `GiveRule` field `enrage: bool`. A landed enrage give crashes regard (existing `regard_delta`) and fires a free swing at the player via the existing failed-talk retaliation path, does NO damage to the monster. (T4.)
4. **All tuning magnitudes** → documented `[TUNE]` starts; T7 measures + re-baselines the four bands. Guard: the becalm dividend is the exact lever that made pacifism dominant in batch 5 — keep it small, measure it.

---

## Task 1: The trainer reads your last life (echo-shaped, presentation-only)

**Files:** `game.rs` (a presentation-only last-life-was-bloody carry, forwarded like `echo` across `INPUT_RETRY`), `contractor.rs` (trainer resurrection lines: wire held-out `TRA_007`; a merciful-return counterpart), `save.rs` (add to the presentation-only exclusion set doc), `main.rs` (test).

**Interfaces:**
- Consumes: the existing `echo`-carry across `INPUT_RETRY` (batch 4), the `kills > spared` read (same as the McGuffin pickup register).
- Produces: a presentation-only `Game::last_life_bloody: Option<bool>` (None = no prior life / fresh reroll), set at death, forwarded through the retry reconstruction exactly as `echo` is.

- [ ] **Step 1:** Failing test — a game reconstructed via `INPUT_RETRY` after a death with `kills > spared` carries `last_life_bloody == Some(true)`; after a merciful death `Some(false)`; a fresh `Game::new_overworld` has `None`. NOT hashed (assert two games differing only in `last_life_bloody` hash equal).
- [ ] **Step 2:** Add the field (presentation-only, joins the `killer`/`echo`/`facing`/`fx_hit`/`mcguffin_last_line_turn`/`died_out_of_her_light` exclusion set — document at `state_hash`'s doc comment). Set it at the death site from `kills > spared`; forward it through the retry path the same place `echo` is forwarded.
- [ ] **Step 3:** Wire the trainer dialogue: a resurrection-triggered line pool the overworld trainer speaks once on a fresh post-death life, branched on `last_life_bloody` — bloody → `TRA_007` ("Back already? Happens. I don't ask. You don't ask.") ironically saluting; merciful → a new grounded counterpart in the trainer's dry register. **Grounding:** the line may only restate the just-ended life's kill/spare fact. This is presentation-only; no persistence subsystem (§9-J ASK #7 stays deferred).
- [ ] **Step 4:** Tests green; goldens byte-identical (dialogue + a presentation-only field can't move them); build clean. **Commit** `feat: batch 13 — the trainer reads your last life (echo-shaped memory + TRA_007)`.

---

## Task 2: Cheese → goblin (stays + rolls for becalm)

**Files:** `gamedef.rs` (if the give-table needs a "stay + roll" outcome beyond the current regard/heal/line/consume shape), `game.rs` (`try_give_player`: cheese-on-goblin stays the target this turn AND rolls a `receptivity`-style becalm), `contractor.rs` (the `cheese → goblin` give row), `main.rs` (tests).

**Interfaces:**
- Consumes: the batch-5 stayed mechanic (transient per-turn stay parameter through `monsters_act`), `receptivity()` (or a dedicated cheese roll), the existing `give_table`/`GiveRule`.
- Produces: a cheese-on-goblin give that ALWAYS stays (guaranteed tempo) and rolls for an outright `calm` (gambled grace).

- [ ] **Step 1:** Failing tests — cheese given to a goblin always stays it that turn (it doesn't swing/advance); a landed becalm roll sets `calm` + increments `spared` (feeds light under §9-E); a failed roll leaves it hostile next turn but the stay still bought the turn. Cheese on a rat stays the existing PENALTY row (unchanged). Cheese on an ogre: no row (declines gracefully).
- [ ] **Step 2:** Implement the stay-and-roll give outcome (reuse `receptivity`/`parley` channel — never combat/ai/worldgen). `[TUNE]` the roll odds.
- [ ] **Step 3:** Wire `cheese → goblin` in `contractor.rs` (positive give-target, story §12.14 — the human-assigned slot; fills the deliberately-unassigned give_table comment). Grounded feedback line.
- [ ] **Step 4:** Tests green; goldens byte-identical; bots still never emit give bytes (unchanged); build clean. **Commit** `feat: batch 13 — cheese wins over goblins (guaranteed stay + gambled becalm)`.

---

## Task 3: Becalm return-trip dividend

**Files:** `game.rs` (light refund when the player passes a becalmed monster on the climb), `gamedef.rs`/`contractor.rs` (`[TUNE]` dividend amount + a once-per-monster guard field), `main.rs` (tests).

**Interfaces:**
- Consumes: `Monster.calm`, the light economy (`spend_turn`).
- Produces: a small light refund the first time the player is adjacent to (passes) a becalmed monster while carrying / on the climb, once per monster (farming-guarded, same spirit as `Monster.awe`'s cap).

- [ ] **Step 1:** Failing test — passing a becalmed monster once refunds `[TUNE]` light; passing it again refunds nothing (once-per-monster hashed flag); passing a hostile refunds nothing.
- [ ] **Step 2:** Implement with a new hashed per-`Monster` `dividend_paid: bool` (run-defining → `state_hash` + `SAVE_VERSION` bump + `parse_save` accepts all older). **GUARD (arc doc §215):** this is the exact lever that made pacifism dominant in batch 5 — keep the dividend small; T7 measures it.
- [ ] **Step 3:** Tests green; goldens byte-identical; `SAVE_VERSION` bumped, old saves replay (test). **Commit** `feat: batch 13 — becalm return-trip dividend (a becalmed monster lights your way home)`.

---

## Task 4: Potion is mammal-medicine (biology-coded give rows + enrage)

**Files:** `gamedef.rs` (`GiveRule::enrage: bool`), `game.rs` (`try_give_player`: an enrage give crashes regard + fires a free swing via the failed-talk retaliation path, no damage to the monster), `contractor.rs` (replace the blanket potion→any row with per-kind rows: potion→rat heal+becalm; potion→goblin/ogre enrage), `main.rs` (tests).

**Interfaces:**
- Consumes: the failed-talk retaliation path (a monster's free swing), the existing `GiveRule`.
- Produces: `GiveRule::enrage`; per-kind potion rows.

- [ ] **Step 1:** Failing tests — potion→rat heals-to-full + becalms (existing behavior, now rat-specific); potion→goblin and potion→ogre crash regard AND land a free swing at the player (reuse the exact failed-talk hit), do NO damage to the monster, consume the potion. A weak-ATK player still triggers the full enrage hit (not tied to player ATK).
- [ ] **Step 2:** Add `GiveRule::enrage`; implement the enrage branch in `try_give_player`. The blanket potion→any row is REPLACED by explicit rows — verify no monster silently loses potion-interaction unintentionally (ogre/goblin now enrage; rat heals; others decline).
- [ ] **Step 3:** Tests green; goldens byte-identical; build clean. **Commit** `feat: batch 13 — potion is mammal-medicine: heals rats, poisons goblinoids (enrage)`.

---

## Task 5: Goblin awe — becalm by giving ground (+ paired punish-behaviors)

**Files:** `gamedef.rs` (`MonsterDef` flags: goblin `awe_threshold` > 0 keyed to give-ground; `punish_holding`/`punish_flight` flags), `game.rs` (`monsters_act_and_resolve_awe`: mirror the ogre's hold-read for the goblin's give-ground-read; the paired punish hits), `contractor.rs` (goblin awe_threshold + punish flags + tells), `headless.rs` (extend the tactical-diplomat bot to read a goblin and give ground, per arc §317), `main.rs` (tests).

**Interfaces:**
- Consumes: batch 11's `Monster.awe` + `monsters_act_and_resolve_awe` + the pre-chase snapshot.
- Produces: goblin awe (give-ground-keyed), `punish_holding`/`punish_flight`, extended tactical-diplomat bot.

- [ ] **Step 1:** Failing tests — (a) ending a turn having MOVED AWAY from an adjacent goblin (gave ground) builds its `awe`; at `awe_threshold` it becalms via the existing `calm`/`spared` path. (b) HOLDING (staying planted) adjacent to a goblin resets its awe AND eats a punish hit (`punish_holding`). (c) The ogre's existing behavior is unchanged: holding builds awe, FLEEING resets it and (`punish_flight`) is what its guaranteed hit answers. (d) The two moves are mutually fatal (goblin-move vs ogre kills; ogre-move vs goblin kills).
- [ ] **Step 2:** Implement generically — the engine reads `MonsterDef` flags; goblin/ogre-ness stays in `contractor.rs`. Reuse `Monster.awe` (already hashed) — NO new hashed field if the give-ground count can live in `awe`. Telegraph both tells (a legible per-kind cue in the log — grounded).
- [ ] **Step 3:** Extend the tactical-diplomat bot (`headless.rs`) to read the creature and pick the opposite correct move (give ground vs a goblin, hold vs an ogre) — deterministic, pure function of state, no bot RNG. Greedy/pacifist bots untouched (their bands must not move).
- [ ] **Step 4:** Tests green; goldens byte-identical; build clean. **Commit** `feat: batch 13 — goblin awe: becalm by giving ground; the paired lethal punish-behaviors`.

---

## Task 6: The donkey-follow seed (rungs 1–2, overworld-only)

**Files:** `game.rs` (follow-mode: the donkey trails the player around the overworld + across screen-links, will NOT descend the hole), `contractor.rs` (flip at the top `DONKEY_TALK` rung), `main.rs` (tests).

**Interfaces:**
- Consumes: the `DONKEY_TALK` regard ladder (top rung `DON_005` "stands beside you now. On purpose."), `Monster.calm`, the overworld world.
- Produces: a follow-mode donkey (rung 2). Rung 3 (LOVE/descend/reveal) is explicitly a LATER batch — NOT here.

- [ ] **Step 1:** Failing tests — once the donkey reaches its top `DONKEY_TALK` rung (calm), it enters follow-mode: it moves toward/with the player each overworld turn and across a screen-link; it does NOT follow through the hole (`V`) into the dungeon (it stays in the overworld). Before that rung it stays put (aloof, rung 1).
- [ ] **Step 2:** Implement follow-mode (overworld-only movement; ~20–30 lines per the arc's own sizing). No new content — follow just makes `DON_005` true. Keep it grep-clean (a generic `follows_when_calm` flag on `MonsterDef`, donkey-ness in the cartridge). Hashed if it changes replay (the donkey's position becomes player-dependent) — bump `SAVE_VERSION` if needed.
- [ ] **Step 3:** Tests green; `--dump-overworld` sane; goldens byte-identical (dungeon untouched); build clean. **Commit** `feat: batch 13 — the donkey follows you (rung 2 seed; won't take the hole — yet)`.

---

## Task 7: Tune + re-baseline the four bands (the combat-balance closeout)

**Files:** `tests/*-band.json` (all four), `contractor.rs` (`[TUNE]` finalization), `.superpowers/sdd` (measurements).

- [ ] **Step 1:** Measure all four policies @ `--sim 5000` on the batch-13 mechanics. The diplomat gets NEW tools (cheese→goblin, goblin awe, becalm dividend) and a NEW trap to avoid (potion→goblinoid enrage — the bot must not do it); the violent bot is largely unaffected. **The flip must survive** (diplomat > violent) and the diplomat must not CREST ~65% (the design's "too generous" guard — the becalm dividend + cheese are exactly the levers that could over-reward pacifism, per the batch-5 lesson).
- [ ] **Step 2:** Tune the `[TUNE]` magnitudes (cheese roll odds, dividend size, goblin awe_threshold, enrage) against the measured numbers — measured, never by feel. Re-baseline all four band files with the post-tune numbers + provenance comments.
- [ ] **Step 3:** `make check` green (all four bands, solver, goldens byte-identical, xhash shift documented, sizes). **Commit** `feat: batch 13 — re-baseline four bands: the diplomacy texture holds the flip`.

---

## Task 8: Docs — doctrine + status log + arc-roadmap fix

- [ ] **Step 1:** `CLAUDE.md`: doctrine bullets for goblin awe (the paired lethal give-ground/hold tactics), potion biology-coding, cheese→goblin, the becalm dividend, the trainer's last-life memory (exclusion set), the donkey-follow seed; §9 checklist / frontier updated; status log entry with final numbers (bands, xhash, sizes, test counts, the measured tuning).
- [ ] **Step 2:** `docs/design/2026-07-22-mercy-economy-arc.md`: fix the roadmap numbering — the ride-alongs are batch 13 (not the original item-3 "batch 12" bundle), portal ROI is batch 14. A same-doc correction noting the 2026-07-24 redesign split.
- [ ] **Step 3:** **Commit** `docs: batch 13 — ride-along doctrine + status log + arc-roadmap renumber`.

---

## Self-Review

- Six features, each spec'd in the arc doc's detailed sections: trainer-last-life ✓T1, cheese→goblin ✓T2, becalm dividend ✓T3, potion biology ✓T4, goblin awe + punish-behaviors ✓T5, donkey-follow seed ✓T6. Combat-balance re-baseline ✓T7, docs ✓T8.
- Combat-balance sign-off class (not worldgen MAJOR): bands re-baseline, goldens byte-identical — every task verifies. Confirm at T7.
- Measured-quantity discipline: every magnitude is `[TUNE]` → measured at T7, never feel-tuned.
- Input vocabulary UNCHANGED (0–16): goblin awe derives from move+talk, no new byte. Confirm at T5.
- Telegraphing mandatory (T5 tells) so wrong-move death is learn-by-death via the T1 resurrect→trainer loop — T1 and T5 are paired by design; land T1's memory before or with T5's tells.
- New hashed state (becalm dividend flag, possibly donkey-follow) → `SAVE_VERSION` bump + `parse_save` back-compat + `state_hash` — confirm per task.
- Grounding: every new line restates only an engine fact (per-task review; batch-8/12 discipline).
- Deferred beyond this batch: portal ROI (14), the donkey LOVE/descend/reveal + ogre-becalm-by-presence (companion batch, §9-F NPC-vault MAJOR), scale/journey.
