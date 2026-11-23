# D07 — batch-plans

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | none | M |

**Owns:** `docs/superpowers/`, the batch implementation plans and design briefs from batch 3 to batch 17, including the signed-off cast package and the camera-console design.

## Purpose
The plans and briefs, the lowest rank of the design docs (below D01 and D03). Each was written for one batch and
records its scope, tasks, sign-offs and gates:
- `plans/`: batch 3 (balance, core/crust seam, terminal backend), 5 (mercy as talk, the Henson ruling), 6
  (variety MAJOR: portals, authored floors, sokoban), 7 (the cartridge split), 8 (the McGuffin's voice), 9 (the
  overworld skeleton), 10 (tactical bots and band re-anchor), 11 (the complete ogre, heal scarcity), 12 (light as
  grace), 13 (mercy ride-alongs), 14 (portal ROI), 15 (playtest refinements), and for batch 17 the two cast
  files: `2026-07-27-cast-npc-vault-major.md`, the signed-off sign-off package (SIGNED OFF 2026-07-27, with the
  human's four rulings and three amendments) whose §A is the frozen terminal-mechanics manifest for 1.0
  (`CLAUDE.md` names it), and `2026-07-27-mimic-batch.md`, the mimic batch's task plan.
- `specs/2026-08-04-flexible-camera-console-design.md`: the design of the terminal backend's camera console
  (three decisions D1 to D3, verification plan, size), built in commit f33a80e.
Code comments cite them (`src/save.rs` cites the cast package's §G for the single save-version bump;
`src/headless.rs` a mimic-batch nit).

## Tests
None: no tracked test reads these files.

## Changes
none: imported

## Gate
- **D07-AC-1** — `plans/2026-07-27-cast-npc-vault-major.md` records its sign-off (2026-07-27) and its §A lists the frozen manifest; a mechanic not on it is post-1.0 unless a manifest amendment adds it. *Verify:* unpinned (prose).

## Risks
- Status lines are stale: the batch-9 plan says "DRAFT — decision-ready, nothing implemented" (batch 9 shipped),
  and the camera-console design says "awaiting human approval before any implementation" (it was approved and
  built: owner, answer Q3b). Most plans keep their task checkboxes unticked (`- [ ]`) though the batches merged.
- The batch-9 plan cites `docs/superpowers/plans/2026-07-19-batch-9-decision.md`, which is not in the repository;
  there is no plan file for batches 1, 2, 4 and 16.
- `src/game.rs` cites `docs/superpowers/specs/2026-08-04-flexible-camera-console`, without the file's `-design.md`
  suffix.
- The plans repeat numbers (bands, sizes, `START_LIGHT`, test counts) whose sources of truth are the band files,
  the code and the gate; they are history as of their batch.
- The files contain non-ASCII characters; they are not rendered.
