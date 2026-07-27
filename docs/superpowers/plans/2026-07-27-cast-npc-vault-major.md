# Cast NPC-vault MAJOR — sign-off package (mimic batch first; cast-arc frame)

**One package, one sign-off, then build.** Authority: `docs/story/STORY-COMPILE-v1.md`
§9-F/§9 is the source; the arc doc is next; this is the plan. Built on the canon-verified
manifest after the human's four rulings (2026-07-27). Every remaining open decision below is
a **recommendation + the default I'll take absent objection** — not a question to serialize.
Approve the MAJOR (with any amendments) and the mimic batch builds; **no golden regenerates
before this is signed.**

Rulings folded (2026-07-27): ① disarm = adopt canon (sword→`Hold` + set-down-for-regard;
violent re-baseline deliberate); ② donkey = compose (arc-doc descent companion + STORY-COMPILE
mantel re-glyph as its payoff); ③ two new minigames, verify ECHO expressible, lesson-state
tracks three; ④ corpse/tombstone/inventory persistence is IN for 1.0.

---

## A. The frozen manifest (corrected — THE RATCHET, complete enumeration)

Anything not on this list is post-1.0 by prior agreement. A cast batch that finds it needs a
mechanic not listed here STOPS and brings a manifest amendment — it does not build first.

1. **Talk-minigame framework** — engine-side, cartridge-configured, reused VERBATIM by the
   mantel exam (§9-I). Per §9-F ("F. Two talk minigames"): **two NEW minigames** —
   coat *answer-the-second-voice* (D2) and mimic *polite-decline* (D3). **ECHO** (D1 rats)
   is verified against current talk rules first (§9-F's own "may already be expressible —
   check"); if it needs engine work, that's a manifest amendment, not a silent build. The
   framework is designed to carry all THREE lessons because the mantel reuses all three
   (echo/answer/endure, §3.6) — see item 7.
2. **Mimic disguise/ambush** — renders as scenery/an item glyph until triggered; courtship-
   as-hunting (want never changes, regard refines it); the *polite no* (offers rest/comfort;
   acceptance = damage; rudeness fails; courteous decline repeatedly = win, §4 D3); the
   climb re-encounter (§3.5, "one encounter slot, high value", McGuffin reaction `[YOURS]`).
3. **Sword custody = canon Hold + set-down-for-regard** (ruling ①, replacing the withdrawn
   stat-flag). `SWORD.on_pickup` moves `Consume` → `Hold`; setting it down (the existing
   put-down verb) in front of any monster is a visible-disarm **regard** bonus (§5 l.390,
   §9-G); the "safety cost of an unheld sword" (§11) is the intended tradeoff and the
   pacifist route's standing decision. The mimic is the flavored special case (the "most
   qualified holder," §4 D3) — same mechanic, its own line set. **Holding the sword confers
   its ATK; setting it down removes it** (recommendation, §E). The violent-route re-baseline
   is deliberate (§D).
4. **Coat give-transform** — give the second coat → one tall monster becomes two short ones
   (§4 D2): the codebase's first mid-run entity spawn. Determinism rider: the split derives
   from existing state or a named per-run channel (`channel(seed, ["coat-split", depth,
   idx])`), never fresh entropy / wall-clock; both spawned monsters' hashed state (positions,
   kinds, regard/calm) is fully determined at split time (§F).
5. **Guaranteed NPC-vault placement (D2–D5) + THE STAGE as the D5 pickup room** — fixed cast
   rooms stamped into the root procedural depths on EVERY seed via the vault channel
   (DECISION.md item 4, the worldgen MAJOR). THE STAGE (D5) holds the pickup pedestal; the
   objective moves from deepest-BFS placement to that pedestal (§C). (D1's MIDDEN + rats are
   existing/decor; the NPC cast is D2 coat, D3 mimic, D4 lost guy, D5 tired ones.)
6. **Donkey rung 3 = compose** (ruling ②): arc-doc **descent companion** (arc doc l.142–171 —
   earn LOVE above the follow rung; he descends the hole; his **presence becalms ogres** via
   the existing `Monster.calm`, "donkeys and ogres are best buds"; follows across levels) —
   with **STORY-COMPILE's mantel re-glyph (§9-K, "whole now, true form, NO behavior") as its
   PAYOFF beat**: the descent seeds the "why he's unafraid," the mantel spends the reveal.
   Plus the **giftable donkey-treat** (arc doc l.171 — new item + give row). Band-orthogonality
   is ASSERTED by a test (bots never schmooze him, never enter the overworld), not stated.
7. **Lesson-state for the exam** — hashed record the mantel reads: all THREE lesson-completions
   (echo/answer/endure), custody-honored, and the run-log axes the three exits key off. Fields
   specified NOW in this MAJOR's save-version bump so the ending batch reads state, never
   re-bumps. (The mantel itself — three exits HAND-IT-OVER / KEEP-IT / TALK + swing, §3.6 — is
   the ending batch's scope, not this one; item 7 is only its state substrate.)
8. **Corpse / tombstone / inventory persistence on retry** (ruling ④, IN for 1.0) — §3.7 +
   §9-J back half (the batch-9 SIGN-OFF-ASK #7 mechanism): your inventory stays below by your
   tombstone; retry-runs loot their own corpse; a dropped D4 item may surface up top by the
   donkey. Lands in cast batch two (with the death-loop content), not the mimic batch.

**NOT on the manifest** (post-1.0): ghost playback, Dest::World enrichment, audio, sprites,
scale/journey "B", wasm/net/platform-matrix. The mantel's three exits are the ending batch.

---

## B. Placement plan (guaranteed NPC-vault MAJOR)

- **Mechanism (recommendation + default):** extend the existing vault-channel stamping with a
  GUARANTEED class — today's vaults roll ~occasionally; cast rooms must stamp on every seed at
  their canon depth. Default: a per-depth "required vault" slot drawn on the existing `vault`
  channel BEFORE the optional ones, so cast placement is deterministic per seed and the
  optional-vault draws that follow shift but stay on-channel. THE STAGE is the D5 required
  vault; its pedestal tile is where the objective instantiates.
- **New solver-side invariant:** on every seed — THE STAGE exists on D5, each cast room exists
  at its canon depth, and the level exit stays BFS-reachable with the rooms stamped (the
  sokoban exit-room-exclusion precedent applies: a cast room never occupies the exit room).
  `--solve` asserts this across full N.
- **Golden coverage (fixture-blind-spot lesson):** the regenerated dump goldens must VISIBLY
  contain the new rooms (a golden that can't see the cast doesn't guard it). Verify by eye on
  regen that D2–D5 of seeds 1/2/3/42/1337 show the cast glyphs.

## C. START_LIGHT re-derivation

The objective moving from deepest-BFS placement to THE STAGE's fixed pedestal changes the
round-trip walk-budget distribution. **Default:** re-run the derivation via `--probe`-style
machinery over the NEW worldgen, present old vs. new worst-case budget with the margin math in
`START_LIGHT`-comment style, and re-commit `tests/solver-band.json`. I do NOT assume the old
constant survives — the sign-off is on the METHOD; the actual numbers land with the one golden
regen after approval (standard MAJOR discipline).

## D. Expected band motion + instrument parity (MANDATORY)

- **Expected motion:** the mimic's ambush strikes bots pathing the spine; guaranteed rooms
  reshape everyone's geometry; sword→`Hold` reprices the violent route (pickup no longer
  auto-confers ATK — the bot must pick up AND keep the sword). Direction: violent bands likely
  DOWN (deliberate — the reprice is the point); diplomat gains the set-down-regard + minigame
  routes. All four bands re-baseline under sign-off.
- **Instrument parity (the rule, non-negotiable):** the tactical-diplomat bot learns
  *polite-decline*, *answer-the-second-voice*, and *set-down-for-regard* IN THIS BATCH; the
  tactical-violent bot's sword handling is updated for `Hold` (pick up and wield to keep ATK).
  What the violent bot does at a mimic: **fights it** — canon allows attacking anything
  ("swing at him… legal, probably fatal", §3.6; the mimic is a monster). Priced in.
- **Commitment:** the flip is EXPECTED to survive; `--sim-flip` MEASURES it (the relational
  gate is the net — a post-cast measurement without the bots learning these moves is an
  artifact and voids the sign-off).

## E. Disarm ruling restated (canon Hold model) + lifecycle/edge table

Recommendation (default): **holding the sword confers its ATK; setting it down removes it** —
cleanest match to §11's "safety cost of an unheld sword," and makes the pacifist's standing
decision legible (bank regard OR keep your teeth). Edges, specified not discovered:

| Situation | Behavior |
|---|---|
| Set sword down in front of monster | put-down verb; sword enters tile's item list; regard bonus fires; ATK drops to base |
| Pick it back up | walk-over re-`Hold`; ATK restored (it's a `Hold` item now, not re-`Consume`) |
| Mimic "holds it" | the same set-down, in front of the mimic, with the mimic's own line set (`[YOURS]`); no separate custody state |
| Monster dies on your set-down sword's tile | sword stays on the tile (an item), pick up as normal |
| Portal out while sword is set down | sword remains on the tile in that world (world state persists, per existing multi-world `LevelState`) |
| Retry after death | sword goes to the corpse/inventory-below per manifest item 8 (ruling ④) |
| Put-down objective while holding sword | both are in the `held` LIFO; put-down/set-down act on the right one by the existing verb routing |

## F. Split-spawn determinism (coat)

Default: positions/kinds/regard of the two short monsters derive from the pre-split coat's own
hashed state plus a named `channel(seed, ["coat-split", depth, idx])` for any placement choice
— never fresh entropy, never wall-clock. Fully determined at split; replay-identical.

## G. Save / hash plan

**One consolidated `SAVE_VERSION` bump for the whole cast arc → 11**, reserving fields the
ending batch needs (item 7's lesson-state) so the ending doesn't re-bump. What joins hashed
state: lesson-completion flags (3), custody/regard hooks as needed, companion carrier state
(the horizon-proofing "carrier identity" field, §10 — keyed to carrier not "the player"),
corpse-persistence state. Sword joining `held` is data, not a new hashed-shape (the `held`
LIFO is already hashed). Expected: `xhash` shifts (new hashed fields); `parse_save` accepts
`1..=11`, older logs replay byte-identical (no old version can contain the new state). The
actual xhash lands with the batch.

## H. Sequencing

1. **Mimic batch (first — framework-proving):** the talk-minigame framework + the mimic
   (disguise/ambush, polite-no, climb re-encounter) + sword→`Hold`/set-down-for-regard +
   the guaranteed-placement MAJOR for THE STAGE + mimic room (D3) and the objective's move to
   THE STAGE + START_LIGHT re-derivation + bands re-baseline + `SAVE_VERSION` 11. This is the
   MAJOR (one golden regen). Bot parity in-batch.
2. **HUMAN PLAYS THE MIMIC** (the gate) — feel iteration on the offer/decline loop is
   expected and pre-budgeted (road-to-1.0's feel-revision slack). Cast batch two is NOT
   briefed until this happens.
3. **Cast batch two:** coat (+ split-spawn) / lost guy / tired ones / the rest of THE STAGE
   dressing + donkey rung 3 (descent companion + treat) + corpse/tombstone persistence.
4. **Ending batch (§9-I):** the mantel's three exits reading the run log (assembles from the
   framework + lesson-state); donkey mantel re-glyph payoff.

## I. Nits (ride the mimic batch, do first — from the batch-16 review)

- `sim_flip_main` should exit NONZERO on a missing band file (today it warns and returns
  success — a deleted band silently disables the thesis check).
- Note the `flip_n`-mismatch behavior (informational, non-failing) in the Makefile `flip`
  comment so CI config doesn't misread it.

## J. Standing obligations (restated — this batch touches all)

- Worldgen MAJOR discipline: goldens regenerate ONCE, after sign-off; solver-band re-commit;
  `--solve` full-N clean on the new worldgen; record the re-baseline.
- New lines grounded (engine-fact only), ≤78 chars, through the const tables, `flavor` channel
  only. The mimic's courtship and the tired ones' four centuries will tempt ungrounded history
  — the batch-8 grounding-reviewer bar applies.
- Size: report stripped + packed per backend, note the toolchain (real 1.75 here); budget is
  law; the cast is content-heavy — watch and state the delta.
- MSRV 1.75 genuinely enforced by `make check` on this container — keep it green.
- Every feel-dependent surface added goes on the playtest-pending list explicitly.

---

## Open decisions folded as recommendations-with-defaults (per the one-package rule)

Absent objection at sign-off, I proceed on the **defaults** below:

1. **Guaranteed-vault mechanism** → per-depth required-vault slot drawn before optional vaults
   (§B). *Alt: a dedicated `authored-room` channel — heavier, rejected as more surface.*
2. **Sword ATK model** → held confers ATK, set-down removes it (§E). *Alt: ATK only via an
   explicit `use` — rejected, adds a step and reads worse.*
3. **Mimic disguise glyph** → the mimic renders as a plausible item glyph (e.g. the lore `?`
   or a chest-like glyph if we add one to the cartridge — cartridge data, engine stays
   grep-clean) until triggered by adjacency/approach. *Default: reuse an existing item glyph;
   no new engine tile.*
4. **ECHO** → verified expressible in current talk rules per §9-F; if it is, no new minigame,
   lesson-state still tracks it. *If it is NOT expressible → manifest amendment before build
   (I stop and bring it).*
5. **Donkey presence-becalm scope** → ogres only (arc doc). *The reveal is spent at the
   mantel re-glyph; the descent becalm is the seed, not the explanation.*
6. **SAVE_VERSION** → single bump to 11 now, reserving ending-batch fields (§G).
