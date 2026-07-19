# Batch 6 — the world gets varied (golem imports, ONE worldgen MAJOR)

*Human direction 2026-07-19: mechanics before presentation — sprites/audio/NPC cast deferred
until these settle. Source: the golem-repo inventory (doors/seals, push+pits, decor mobs, boss
tells, ledger grade). This batch is a SINGLE authorized worldgen MAJOR: every channel-touching
feature lands here, goldens regenerate once at the end, both sim bands re-baseline as needed
(sign-off = this brief + the CLAUDE.md re-sequencing note). The truth-envelope NPC pattern is
adopted LATER with the NPC cast — not this batch.*

## Design (decided; tune within gates)

- **Portals (human redirect 2026-07-19: doors are portals to other dungeons, known hash —
  "can build worlds that way").** New `Tile::Portal` (dump glyph `*`): placed via the worldgen
  channel, chance ~1/4 per depth, in a room interior that is neither the entrance nor the exit
  room. Destination seed DERIVED: `h64(world_seed, ["portal", depth_tag, index_tag])` — the
  whole multiverse is a pure function of the root seed, frozen by goldens like everything else.
  Walking onto a portal does NOT transit: it logs the door's description — destination theme
  label + world hash, both derived from the destination seed (grounded: the engine proves it by
  generation). TRANSIT = pressing wait (byte 4) while standing on the portal (deliberate; no
  new input byte; neither sim bot ever emits wait, so sims are structurally unaffected). Costs
  one turn of light like any wait.
  Multi-world state: `Game.worlds: <map seed -> per-depth saved LevelStates>` (or equivalent),
  `Game.world_seed` (current), root seed retained; insertion-order-deterministic; ALL of it
  hashed in state_hash (extend the saved-levels hashing to span worlds). Light, hp/maxhp/atk,
  kills, spared, has_amulet are GLOBAL across worlds — the torch is the multiverse's one clock.
  The amulet exists only in the ROOT world's depth 5; the win check fires only on the root
  world's depth-1 `<`. In a NON-root world, depth-1's `<` is the return portal: stepping on it
  transits back to the source world's portal tile (walk-on, like stairs — leaving is easy,
  entering is deliberate). Portal worlds are full 5-depth worlds and contain their own derived
  portals (infinite graph, light-bounded). Track per-world provenance (source seed + portal
  position) for the return hop — hashed.
  Solver: root-world reachability model UNCHANGED (portals never replace stairs, never gate the
  win path; transit needs wait, so BFS routing is untouched). Sims: bots never transit —
  document as policy; a portal-diving policy is future work.
- **Authored-floor destinations (human addition 2026-07-19).** A portal's destination is an
  enum: `Dest::World(u64)` (derived seed, as above) or `Dest::Floor(u8)` (index into a new
  `AUTHORED_FLOORS` const table in content.rs — hand-built single-level maps in an extended
  vault-style ASCII legend: `#` wall, `.` floor, `<` return portal, plus the standard item/
  monster/lore chars; full-map size up to 80x25, smaller maps centered and wall-padded).
  Which kind: worldgen-channel roll per portal (~1/3 authored when any floors exist). Authored
  floors are singular places (same floor reachable from different worlds is the SAME floor —
  world map keyed by a WorldId enum {Seed(u64), Floor(u8)}; its visited state persists like
  any level). No depths, no stairs, no amulet; the `<` returns to the source portal. Their
  describe-line uses an authored name from the table ("beyond it: <name>"), grounded. Ship 2
  modest starter floors this batch (one quiet lore shrine, one small hazard/loot room) —
  the heavy authoring belongs to the future NPC-cast batch. Floor content draws NOTHING from
  RNG (pure const); monsters/items on floors are hashed state once instantiated.
- **One seal: the keyed stair** (MOVED to T2 scope alongside vaults). On depths 3-5 (worldgen channel roll, ~1/2 of those depths),
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

**T1 — portals + multi-world state + authored floors** (game.rs/content.rs/save.rs/headless.rs/render.rs).
Tests: derived destination determinism; transit-on-wait only (walk-over logs, never transits);
round trip source->portal-world->back restores the source level exactly; light/kills/amulet
global across worlds; state_hash spans visited worlds; replay determinism across a multi-world
input log; root win/amulet unaffected; solver untouched (assert same budgets as pre-portal on
a seed sample... budgets shift only from placement draws changing layouts — solver RE-BASELINE
expected from the MAJOR itself, do it here with full comment). Dump glyph `*`; describe-line
grounded + <=78 chars. Authored-floor tests: legend well-formedness (bordered, return `<`
present, legal chars); floor round-trip persistence; same-floor-from-two-worlds shares state.
**T2 — pits + blocks + 2 new vaults + keyed stair seal** (game.rs/content.rs/headless.rs).
Keyed seal exactly as the Design bullet (solver models the key detour — budget += route via
key on sealed depths; second solver re-baseline folds into T4's final numbers). Tests:
push/refuse/fill semantics; vault well-formedness extended; blocks hashed; seal
refusal/unseal determinism; key-detour math unit test; solve 10000 zero unwinnable.
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
