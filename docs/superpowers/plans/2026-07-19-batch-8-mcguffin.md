# Batch 8 — voice two comes online (story §9-B/C/D + pickup scene)

*Implements the McGuffin's mechanical presence per STORY-COMPILE-v1: event lines (B), the
record-keyed pickup register (C), put-down/pick-up as world events (D). All CHEAP per §9 —
const tables keyed to counters the engine already tracks. Everything lands in the contractor
cartridge; the engine gains only generic hooks. FLAVOR-DRAFT-v0 IDs wire verbatim (MCG_001-112,
NAR_050-054); [YOURS] slots ship as the standing placeholders.*

## Design

- **Engine hook (generic): `EventLine` dispatch.** The def gains an event-line table:
  `GAME.carried_lines: &[(CarryEvent, &[&str])]` where CarryEvent is an engine enum
  { PickedUp, PutDown, PickedBackUp, StairsUp, MonsterAdjacent, KillWitnessed, SpareWitnessed,
  TierCrossed, Idle(n) } — fired by the engine at the existing code points (pickup, the new
  put-down verb, ascend, monsters_act, kill/spare bookkeeping) ONLY while the objective is
  carried. Line selection: flavor_rng over the event's pool (replay-safe). Rate-limit: at most
  one McGuffin line per N turns [TUNE 6] except PickedUp/PutDown (always) — the amulet is loud,
  not spammy.
- **C: pickup register.** At objective pickup, branch on the descent record (kills vs spared —
  the counters exist): bloody (kills > spared) → MCG_010-013 pool; merciful → MCG_020-023.
  Both preceded by the six words + interruption (MCG_001-004, in order, once).
- **D: put-down / pick-back-up.** The objective becomes a Hold-style item interaction: USE
  while carrying = set it down on the current tile (it re-enters the level's item list; carry
  burn reverts to 1×; MCG_070-073 + NAR_053); walking over it again re-carries (MCG_074-075).
  Put-down is LEGAL anywhere; a run that abandons it and exits without it cannot win
  (win predicate already requires has_objective). Replay/hash: it's ordinary item/carry state,
  already covered; add tests (put-down/pick-up replay determinism; hash covers the dropped
  objective's position; light burn rate flips correctly both ways).
- **Speech re-entry ladder (climb):** StairsUp events walk MCG_030-033 in ORDER (an index,
  hashed? — no: derived from count of StairsUp events while carrying, which is derivable from
  turns/depth history... simplest honest: a small hashed counter `speech_attempts: u8`).
- **Mood → light (§9-E) is NOT this batch** (SMALL but tunable-heavy; batch 9+). Note it.
- Sim impact: bots never emit the put-down USE while carrying? The bots DO use byte 15 now
  (potion drinking, batch 7). Guard: USE-while-carrying-objective = put-down. Bot policy must
  never put the objective down: assert in sim tests (bots only USE when hp-low + potion held —
  putting down requires objective carried AND takes priority? DESIGN: USE semantics = LIFO held
  item; the carried objective is NOT in `held` (it is `has_objective`); so USE stays potion-only
  unless we add an explicit byte. DECISION: put-down gets its own byte 16 (chord `p`? or
  g+self?). Frontends: `p` key, no chord. Bots never emit 16. Bands untouched.)
- Save vocabulary: 0-16 after this batch. Old logs unaffected (test).

## Tasks

T1 — engine hooks (CarryEvent dispatch, byte 16 put-down, speech_attempts counter) + tests.
Goldens/sims/solve untouched (no worldgen, no bot changes — verify identity).
T2 — cartridge data: the full MCG table wiring (90 lines from FLAVOR-DRAFT verbatim), the
pickup-register branch, NAR_050-054. Length tests across every line. Grounding audit is the
review's job (Rule 5: grep the wired lines for duplicate-foreshadowing).
T3 — docs + status (vocab 0-16, CarryEvent doctrine, the §9 checklist with B/C/D marked DONE).

## Gates

House process; FOREGROUND-only; full make check green per task; byte-identity for T1
(goldens/xhash/sims/solve all unchanged); T2 changes nothing but strings-at-runtime (gates
identical — the McGuffin speaks only in play, never in dumps/frames); sizes reported
(budget ≤ +8 KB packed; 90 talk lines ≈ 6-7 KB raw, compresses well). rustc 1.75; zero
crates; engine grep-clean maintained; Rule 5 binds every wired line.
