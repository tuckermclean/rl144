# Batch 6 — the world gets varied (golem imports, ONE worldgen MAJOR)

*Human direction 2026-07-19: mechanics before presentation — sprites/audio/NPC cast deferred
until these settle. Source: the golem-repo inventory (doors/seals, push+pits, decor mobs, boss
tells, ledger grade). This batch is a SINGLE authorized worldgen MAJOR: every channel-touching
feature lands here, goldens regenerate once at the end, both sim bands re-baseline as needed
(sign-off = this brief + the CLAUDE.md re-sequencing note). The truth-envelope NPC pattern is
adopted LATER with the NPC cast — not this batch.*

## Design (decided; tune within gates)

- **Doors.** New `Tile::Door` (closed): blocks movement AND line of sight. Walking into it
  opens it (becomes `Tile::OpenDoor`, passable, LOS-transparent; costs a turn, no tax, like a
  move). Doors never re-close. Placement: worldgen puts a door where a corridor meets a room
  wall (probabilistic per junction via the worldgen channel — tune so roughly 1 in 3 junctions
  get one). Dump glyphs: `+` closed, `'` open. Autotile mask treats doors as non-wall.
- **One seal: the keyed stair.** On depths 3-5 (worldgen channel roll, ~1/2 of those depths),
  the down-stairs is sealed: walking onto `>` without the key logs a themed refusal line and
  costs nothing. A key item (`IKind::Key`, glyph `k`) spawns via the spawns channel in a
  reachable room. Walk-over pickup as usual; stepping on the sealed `>` WITH the key unseals
  (consumes key, logs line, then descends normally). SOLVER: solve_seed must model the detour —
  budget = walk to key + key to stairs (+ existing legs); tests/solver-band.json WILL move:
  re-baseline with full stats in the comment (this is the batch's expected solver re-baseline).
- **Pits + push-blocks (vault-only this batch).** New vault legend chars: `^` pit, `B` block.
  `Tile::Pit`: impassable to the player (stepping is refused with a message, no turn), lethal
  to anything pushed in. Blocks are entities (occupy tiles like monsters): walking into a block
  pushes it one tile if the destination is free floor (costs a turn) — pushing into a pit
  destroys the block and FILLS the pit (tile becomes Floor): the sokoban bridge. Chains (block
  into block) do NOT move this batch (keep it simple; golem's chain-push is a later import).
  Blocks block LOS? No — waist-high, LOS passes. Blocks are hashed state (position matters).
  Author 2 new vaults using pits/blocks (a bridge puzzle guarding loot; a pit-ringed lore
  room); existing 3 vaults untouched. Pit/block rooms must never gate the EXIT (loot only) —
  the solver proves exits stay reachable regardless.
- **Decor mobs.** New `MKind::Critter`-tier? NO — keep the 3 combat kinds; decor mobs are a
  separate lightweight entity: `Critter { x, y }`, one per theme (themed name in Theme.mobs
  gains a 4th slot or a new field — implementer's call, document), spawned ~1/3 of depths via
  the spawns channel, wanders via ai_rng (1/3 chance per turn, never blocks doorways
  deliberately — just never attacks, never chases). Dump glyph: `c`. Killable by bump (1 hp):
  counts as a kill (the Judge will notice). Talking to it: one flavor line, no regard system.
  Hashed state.
- **Ogre tell.** Ogres telegraph: when adjacent and about to attack, an ogre instead RAISES
  (one turn, no damage, themed log line + distinct render tint via existing palette machinery);
  the NEXT turn it swings for 1.5× damage (6+roll) if still adjacent — stepping away resets.
  Raised state is hashed (it changes outcomes). Rats/goblins unchanged. This changes combat:
  BOTH sim bands re-measure and re-baseline (greedy will shift — that's expected and signed
  off; keep win_pct within [10,25] using the standing approved knobs if it drifts out).
- **Ledger letter grade.** Pure `fn grade(&Game) -> &'static str` ("A+".."F") over engine
  facts (won/depth/kills/spared/lore-read/light-left/turns; exact rubric = implementer's,
  documented ONLY in a code comment — never shown to the player). End screen shows the grade
  with one deadpan line. The rubric being withheld in-game is the joke; grounding doctrine
  still applies to the line itself.

## Tasks

**T1 — doors + keyed seal + solver extension** (game.rs/content.rs/headless.rs). Tests: door
open/LOS semantics; seal refusal/unseal determinism; solver models key detour; replay with
doors deterministic. Solver band re-baseline expected here.
**T2 — pits + blocks + 2 new vaults** (game.rs/content.rs). Tests: push/refuse/fill semantics;
vault well-formedness extended for new legend; blocks hashed; solve 10000 zero unwinnable.
**T3 — decor mobs + ogre tell + sim re-baselines** (game.rs/content.rs/headless.rs bands).
Tests: critter never attacks; tell sequence (raise→heavy/reset); both policies measured at
5000, both bands re-baselined with full comments.
**T4 — ledger grade + goldens regen + docs** (render.rs/game.rs/CLAUDE.md). Regenerate ALL
dump goldens + frame goldens ONCE here (after T1-T3 land); verify layout-vs-content diff
discipline no longer applies (this MAJOR changes layout via doors — full regen, note in
commit); status entry with all re-baselined numbers; playtest list.

## Gates & constraints

House process (implementer → reviewer → fix → final whole-branch review). FOREGROUND-only
gate runs (blocking Bash, timeout 600000). After T4: full `make check` green end-to-end.
Goldens regenerate exactly once (T4) — T1-T3 run `make check` with goldens EXPECTED-failing;
use `cargo test -- --skip golden` equivalents plus the other gates, and say so in reports
(the golden_dumps unit test + goldens/frames targets are the only permitted reds mid-batch,
green by T4). Dump legend additions (`+ ' ^ k c B`) documented in CLAUDE.md. Sizes reported
per task (budget ≤ +10 KB packed total). rustc 1.75; zero deps; no cfg in core; grounding
doctrine on all new lines; input vocabulary UNCHANGED (no new bytes — doors/pushes ride
existing move semantics).
