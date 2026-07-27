# Mimic Batch — Implementation Plan (cast batch one; the NPC-vault MAJOR)

> **For agentic workers:** execute task-by-task. The spec/rationale lives in the signed-off
> package `docs/superpowers/plans/2026-07-27-cast-npc-vault-major.md` — read it for the WHY;
> this doc is the WHAT/WHERE, decomposed. Authority: STORY-COMPILE §9-F/§9 > arc doc > this.

**Goal:** ship the first cast character (the mimic), the talk-minigame framework it proves,
the sword→Hold/set-down-for-regard mechanic, and the guaranteed NPC-vault worldgen MAJOR
(THE STAGE on D5 + the mimic room on D3, objective relocated to THE STAGE's pedestal) — such
that the human can play the mimic before cast batch two is briefed.

**Architecture:** engine stays generic/grep-clean; all cast facts are cartridge data in
`contractor.rs`. The talk-minigame framework is engine-side, cartridge-configured, and is the
thing the mantel exam (§9-I) later reuses verbatim.

## Global Constraints (copied from the signed package + doctrine)

- **Goldens regenerate ONCE** (in T1, after which no task touches worldgen). MAJOR discipline:
  re-commit `tests/solver-band.json`, re-derive `start_light`, `--solve` full-N clean.
- **Instrument-parity rule:** bots learn every new move IN THIS BATCH (T5) or the band
  measurement is an artifact. `--sim-flip` must survive (diplomat ≥ violent + 3 @ N=5000).
- Engine grep-clean of game nouns; new glyph is a cartridge fact. New lines grounded, ≤78
  chars, const tables, `flavor` channel only. Core hashed state stays explicit-width.
- `SAVE_VERSION` → **11**, ONE consolidated bump (T2), reserving ending-batch lesson-state
  fields (T3). `parse_save` accepts `1..=11`.
- Size reported per backend, noting toolchain (real 1.75 here). MSRV 1.75 stays green.
- Mimic disguise glyph: a NEW free single-byte cartridge glyph reading as a chest — **never
  the lore `?`** (amendment B). Default pick `0` (boxy); final byte is a one-line cartridge
  tune — must be a printable ASCII byte NOT in the taken set above.

---

## Task order & dependencies

T1 (worldgen MAJOR, regen goldens) → T2 (sword/put-down framework) → T3 (talk-minigame
framework) → T4 (mimic behavior + lines) → T5 (bots + bands + START_LIGHT numbers + nits) →
T6 (docs + whole-branch review). T2 and T3 are independent of each other (both after T1);
T4 needs T1(placement)+T3(framework); T5 needs T1–T4.

---

### Task 1: The worldgen MAJOR — guaranteed cast-vault placement, THE STAGE, objective relocation

**Files:** `src/game.rs` (`gen_level` ~900-1207, `stamp_vault` 1223-1246), `src/games/contractor.rs`
(MONSTERS ~287, VAULTS ~819, kind consts ~27), `src/gamedef.rs` (MonsterDef if a `disguise`
field is added — but DEFER behavior; T1 only needs the row to exist), `src/main.rs` (new
dump-test), `tests/golden/*.txt` (regen), `tests/solver-band.json` (re-commit),
`src/games/contractor.rs` (`start_light` re-derive).

**Interfaces produced:** `MIMIC` kind const + `MONSTERS[MIMIC]` row (chest glyph, minimal
behavior — a plain non-passive monster for now; ambush/courtship/minigame land in T3/T4 and do
NOT touch dumps). Two new guaranteed vaults: THE STAGE (D5, contains the objective-pedestal
tile) and the mimic room (D3, contains the MIMIC glyph). Objective moves from the deepest-BFS
room (`game.rs:1037`) to THE STAGE's pedestal.

**Steps:**
- [ ] Add `MIMIC` kind const in contractor.rs; add `MONSTERS[MIMIC]` row: chest glyph (see
  Global Constraints), hp/atk modest, `passive:false`, `bump:Fight`, all mercy fields 0/false,
  `talk_lines` placeholder (real lines T4). Grep-clean check: glyph is data only.
- [ ] Add guaranteed-vault mechanism in `gen_level`'s vault block (~940-960): when
  `self.depth` is a cast depth, stamp the required vault for that depth UNCONDITIONALLY on the
  `vault` channel BEFORE the existing `vr.chance(2,5)` optional roll; else keep today's roll.
  This batch's required vaults: D3 → mimic room, D5 → THE STAGE. (D2/D4 reserved for cast
  batch two — leave the mechanism general, only D3/D5 populated now.)
- [ ] Add the two vault strings to `VAULTS` (or a new required-vaults table): THE STAGE — a
  cleared room with a pedestal tile where the objective sits (use the objective glyph `&` at
  the pedestal so `stamp_vault`'s existing item-glyph lookup places it); the mimic room — a
  room containing the MIMIC glyph. Ensure neither collides with the exit room (reuse the
  sokoban-center exclusion precedent at 1023-1031, generalized to "a required-vault center is
  never the exit room").
- [ ] Objective relocation: on root last depth, the objective must now come from THE STAGE's
  pedestal (stamped by the vault), NOT the `deepest`-room push at `game.rs:1037`. Remove/guard
  that push so the objective isn't double-placed; confirm exactly one objective on D5.
- [ ] Solver invariant: extend `solve_seed` (`headless.rs:74`) reachability so it still finds
  the objective at THE STAGE (it BFS-targets the objective item on the last depth already —
  verify it works with the relocated item). Add a `#[test]` asserting every seed 0..N: THE
  STAGE exists on D5, the mimic room on D3, and the exit stays reachable.
- [ ] **Dump-test assertion (amendment C):** `#[test]` asserting the cast glyphs appear at
  canon depths in the golden seeds (1/2/3/42/1337): mimic-glyph present on D3, objective `&`
  present on D5. This survives the regen and guards guaranteed placement permanently.
- [ ] Regenerate goldens ONCE (`--dump --seed N > tests/golden/seed_N.txt` for 1/2/3/42/1337);
  eyeball the diff is exactly the two new rooms + objective relocation (no unrelated layout
  drift outside the stamped rooms).
- [ ] `--solve 10000`: 0 unwinnable; re-derive `start_light` from the new worst-case budget,
  update the derivation comment (contractor.rs ~941) with old vs. new numbers; re-commit
  `tests/solver-band.json`.
- [ ] `cargo test` green; `make check` green (xhash WILL shift — worldgen changed; both
  backends must agree). Report both sizes, noting real-1.75 toolchain.

**Acceptance:** goldens regenerated once and visibly show the cast; dump-test + solver-invariant
tests green; `--solve 10000` 0 unwinnable with re-derived start_light; xhash shifts, both
backends agree; all four sim bands may drift (re-baselined properly in T5, not here — but note
the drift). This is the MAJOR; nothing after T1 regenerates a golden.

---

### Task 2: Sword→Hold + ATK-while-held + set-down-for-regard + put-down kind-routing

**Files:** `src/games/contractor.rs` (SWORD row ~479, give_table ~54-region), `src/game.rs`
(atk computation, `put_down` 2982, `apply_input` 3916-3938, held handling), `src/gamedef.rs`
(maybe a `GiveRule`/regard hook for set-down), `src/save.rs` (`SAVE_VERSION`→11, `parse_save`
`1..=11`), `src/main.rs` (tests).

**Interfaces produced:** sword is a `Hold` item conferring +2 ATK while held; setting the sword
down in front of a monster fires a regard bonus; put-down is kind-routed (choose what to set
down). `SAVE_VERSION` = 11.

**Steps:**
- [ ] SWORD `on_pickup` `Consume`→`Hold` (contractor.rs:479). ATK model: `g.atk` becomes
  `base_atk + sum(AtkBonus of held items)`, recomputed whenever `held` changes (pickup,
  use, give, put-down, set-down). Add a `recompute_atk`-style helper; call it at every
  `held` mutation site. TDD: test that holding the sword gives +2, setting it down removes it.
- [ ] Set-down-for-regard: setting the sword down in front of a monster (adjacent) fires a
  visible-disarm regard bonus (a `give_table`-style hook or a branch in the set-down path).
  Value is `[TUNE]` per §11; wire the hook, leave the number a documented default. TDD.
- [ ] **Put-down kind-routing (amendment A):** the LIFO "acts on whatever's on top" is
  withdrawn. Make put-down choose WHAT to set down (objective vs sword), mirroring batch 15's
  use-by-kind: a `put_down_kind(kind)` + new input bytes (the range past `17+len(items)`, or a
  parallel `set-down-by-kind` range — pick one, document the vocabulary growth, bump comment in
  `apply_input` + `save.rs` format comment). Frontend selector is chrome (like the use-selector);
  engine just needs the byte→kind routing. TDD: replay-determinism of the new bytes.
- [ ] `SAVE_VERSION`→11 (save.rs:59), `parse_save` `1..=11` (save.rs:79-86); test that a
  `1..=10` log still replays byte-identical and an 11-log round-trips.
- [ ] `cargo test` green; goldens/frames byte-identical (no worldgen touch); `--solve`
  unchanged; xhash shifts only if hashed state changed (the set-down vocabulary is input, not
  state — held already hashed; confirm). Report sizes.

**Acceptance:** sword held confers ATK, set-down removes it + banks regard; put-down is
kind-routed (no LIFO ambiguity); SAVE_VERSION 11 accepts 1..=11; no golden regen.

---

### Task 3: Talk-minigame framework + polite-decline (mimic) + ECHO verification + lesson-state

**Files:** `src/gamedef.rs` (a `MonsterDef` field, e.g. `talk_minigame: Option<Minigame>`;
`Minigame` enum {Echo, AnswerSecondVoice, PoliteDecline}), `src/game.rs` (`try_talk_player`
seam ~2608; lesson-state fields on `Game`, hashed), `src/games/contractor.rs` (wire the mimic's
`PoliteDecline`), `src/save.rs` (lesson-state in `state_hash` + reserve ending fields),
`src/main.rs` (tests).

**Interfaces produced:** engine-side talk-minigame dispatch layered over `receptivity`;
`PoliteDecline` implemented (offer/decline loop — decline advances, accept damages, rudeness
fails, per §4 D3); hashed lesson-completion state (echo/answer/endure) the mantel will read.

**Steps:**
- [ ] Add `Minigame` enum + `MonsterDef::talk_minigame: Option<Minigame>`; default `None` for
  all existing rows (byte-identical behavior when None — the invariant that keeps every current
  test green). TDD: a `None` monster talks exactly as before.
- [ ] Branch `try_talk_player` at the receptivity seam (~2608): if `talk_minigame` is `Some`,
  run the minigame's resolution instead of the flat roll, still ending in the same
  `monsters_act_and_resolve_awe` tail. Implement `PoliteDecline`: the mimic offers; a decline
  (talk) advances toward becalm; accepting (a to-be-defined accept input, or a wrong response)
  costs HP; the winning line is repeated courteous decline while light burns.
- [ ] **Verify ECHO expressible in current talk rules** (§9-F's own check): try to express the
  rat's ECHO as `talk_minigame: None` + existing receptivity, or a trivial `Echo` variant. If
  it needs real engine work beyond a thin variant → STOP and raise a manifest amendment (do not
  silently build). Document the finding either way.
- [ ] Lesson-state: hashed `Game` fields recording completion of the three lessons
  (echo/answer/endure) — add to `state_hash` (save.rs:285-318), reserve the ending-batch fields
  now so the ending doesn't re-bump. (Version already bumped to 11 in T2.)
- [ ] `cargo test` green (esp. the `None`-is-byte-identical test); goldens/frames byte-identical;
  xhash shifts (lesson-state joins the hash), both backends agree. Sizes.

**Acceptance:** framework dispatches per-kind; mimic's polite-decline works; ECHO status
documented (expressible, or amendment raised); lesson-state hashed and reserved; no golden regen.

---

### Task 4: The mimic — disguise/ambush/courtship behavior + climb re-encounter + grounded lines

**Files:** `src/game.rs` (mimic disguise/ambush behavior in `monsters_act`; climb re-encounter
hook), `src/gamedef.rs` (a `disguise`/`ambush` MonsterDef field if needed; a `Monster.disguised`
hashed bool if the disguise is run-defining), `src/games/contractor.rs` (mimic talk_lines,
courtship register, climb-re-encounter line pool — grounded), `src/save.rs` (if `disguised`
joins the hash — reuse the SAVE_VERSION-11 bump), `src/main.rs` (tests).

**Interfaces produced:** the mimic renders as its chest glyph and does not act until triggered
(adjacency/approach), then reveals + runs courtship-as-hunting; the McGuffin reacts to the mimic
on the climb (§3.5, `[YOURS]` line pool).

**Steps:**
- [ ] Disguise/ambush: the mimic is inert (no chase/attack) while disguised; triggers on
  player adjacency/approach, then behaves. If `disguised` is run-defining (it changes whether
  the monster acts), it's a hashed `Monster` field (reuse SAVE_VERSION 11); if purely derived,
  keep it out of the hash. Decide and document. TDD the trigger.
- [ ] Courtship-as-hunting: the mimic's want never changes; regard refines register
  (workmanlike → professional → wistful hunger, §4 D3). Wire via `talk_lines` + the
  `PoliteDecline` minigame from T3.
- [ ] Climb re-encounter: dispatch a `CarryEvent`-style hook when the carrying player passes
  the becalmed mimic on the climb; the McGuffin's reaction is a grounded `[YOURS]` line pool
  (held/placeholder if not yet authored — grounded, ≤78, flavor channel).
- [ ] Grounded-lines review bar (batch-8 standard): the mimic's courtship must not invent
  unproven history; engine-fact only.
- [ ] `cargo test` green; goldens/frames byte-identical (behavior/lines don't change dumps —
  the mimic's glyph was already placed in T1); xhash shifts only if `disguised` joined the hash.
  Sizes.

**Acceptance:** mimic ambushes from disguise, runs courtship + polite-decline, McGuffin reacts
on the climb; lines grounded; no golden regen.

---

### Task 5: Instrument parity — bots learn the moves, re-baseline bands, START_LIGHT, nits

**Files:** `src/headless.rs` (bot decision block ~830-960; `sim_flip_main` missing-band fix),
`tests/*-band.json` (all four re-baselined), `Makefile` (flip_n comment),
`src/games/contractor.rs` (start_light final if not settled in T1), `src/main.rs` (tests).

**Steps:**
- [ ] Diplomat bots (Pacifist/TacticalPacifist) learn: the polite-decline response at a mimic,
  and set-down-for-regard where it helps. Violent bots (Greedy/Tactical): sword-Hold handling —
  pick up AND keep the sword to retain ATK (they no longer get it free on walk-over). At a
  mimic the violent bot fights it (canon-legal). Thread through the existing policy gates.
- [ ] Re-measure all four policies @ 5000; re-baseline the four band files with fresh comments
  (measured numbers, the deliberate violent reprice noted). `--sim-flip 5000`: diplomat ≥
  violent + 3 — MEASURE and confirm the flip survives; if it doesn't, the goblin `awe_threshold`
  / minigame tuning is the lever (measured, not by feel) — surface to human if it can't be hit.
- [ ] **Nit:** `sim_flip_main` exits NONZERO on a missing band file (today warns+returns success).
- [ ] **Nit:** note the `flip_n`-mismatch (informational, non-failing) in the Makefile `flip`
  comment.
- [ ] Finalize `start_light` numbers + solver-band (if T1 left them provisional).
- [ ] `make check` GREEN end-to-end (all four bands + flip + goldens + xhash + size + msrv).

**Acceptance:** bots exercise the new moves; four bands re-baselined; `--sim-flip` passes; nits
fixed; full `make check` green under real 1.75.

---

### Task 6: Docs, status log, whole-branch review

**Files:** `CLAUDE.md` (status log entry, frontier, new-glyph + new-mechanic doctrine, the
mimic-batch playtest-pending list), the plan/package docs (mark done).

**Steps:**
- [ ] CLAUDE.md: batch-17 status entry (numbers, xhash, sizes, bands, save v11, the MAJOR
  re-baseline); frontier update (mimic done pending human playtest → then cast batch two);
  doctrine for the new glyph, the talk-minigame framework, sword-Hold/set-down, put-down
  kind-routing, guaranteed-vault placement.
- [ ] Playtest-pending: the mimic's polite-decline feel, the chest-disguise reveal, sword
  set-down decision, the new put-down selector — all explicitly listed.
- [ ] **Whole-branch review (controller/Opus, per "do the final reviews yourself"):** the full
  branch diff, grep-clean check, canon-fidelity check against §9-F, gate re-verification.
- [ ] Commit; ff master; push; brief the human. **Then the human plays the mimic** (the gate)
  before cast batch two is briefed.

---

## Self-review notes (controller)

- The MAJOR (T1) is the only golden-regenerating task; T2–T6 must keep goldens/frames
  byte-identical (verify each).
- The `talk_minigame: None`-is-byte-identical invariant (T3) is what protects the ~200 existing
  tests through the framework addition — the same empty-pool discipline as CarryEvent (batch 8).
- Instrument parity (T5) is the sign-off's load-bearing promise: no band number is trustworthy
  until the bots exercise the new moves. The flip is the net.
- ECHO (T3) has a real STOP condition: if it needs engine work beyond a thin variant, it's a
  manifest amendment, not a silent build.
