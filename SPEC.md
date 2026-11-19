# SPEC: rl144

## What this is
rl144 is a top-down roguelike in Rust whose shipped binary plus all its assets fit on a 3.5" floppy: at most
1,474,560 bytes, UPX-packed, per backend. It is being taken to 1.0 along the plan of record,
`docs/design/2026-07-26-road-to-1.0.md`: cast, then the ending as an exam, then polish, legibility and ship
(its finish line, a `SHIP.md` acceptance list gated by `make ship` plus a `make floppy` image, is not
written yet).

It is not: the post-1.0 work `CLAUDE.md` lists (Dest::World multiverse enrichment, scale/journey "B", ghost
playback, audio, sprites, wasm/net; `docs/design/monomyth.md` is deferred), none of which is built without a
re-prioritization by the owner; nor the permanent cuts (mod support, config files, localization). The 486
boot build lives in a separate factory project, not here (its scope note and target spec are in
`docs/design/486/`; the target spec is reference material, not wired into the gate).

## Principles
The rules in `CLAUDE.md` are the principles every node keeps (owner, answer Q4). In short:
- **The floppy budget** is the one hard constraint: UPX-packed binary at most 1,474,560 bytes per backend;
  every change that adds code or data reports the stripped and packed sizes with the delta and the toolchain
  that measured them.
- **Frozen dependencies**: `minifb` and `font8x8` only, behind the `backend-minifb` feature; no engines, no
  `serde`, `rand` or `image`; no asset files.
- **Core/crust**: the core makes zero platform calls and has zero `cfg` blocks (sole exception: the
  `games/mod.rs` wiring); backends only consume `render_cells`, `scene`, `Game::apply_input` and the
  headless entry points.
- **Cartridge doctrine**: the engine is generic and grep-clean of game nouns; every game fact lives on
  `GameDef` in `src/games/contractor.rs`.
- **Seed and save doctrine**: a change that diffs a golden map layout is a MAJOR bump that needs the owner's
  explicit sign-off, written in the node's spec before the node is built; `SAVE_VERSION` is bumped on
  input-vocabulary growth or a hashed-state-shape change, and old saves stay accepted.
- **Headless-first verification**: every system is observable through `--dump` or a headless flag;
  `make check` is the whole gate; window-path changes are flagged "needs interactive playtest", never
  reported as verified.
- **Instrument parity**: a node that adds a move to a route teaches the bot that models the route in the
  same node; a band-moving change says "the flip is expected to survive — measure it".
- **ASCII-only strings**, explicit-width integers in hashed state, MSRV 1.75.
- **Canon precedence**: on game content the human-locked story canon (`docs/story/STORY-COMPILE-v1.md`,
  `docs/story/FLAVOR-DRAFT-v0.md`) outranks this file, which outranks `docs/design/*.md`, which outrank
  plans and briefs. Each piece of game content is briefed from the story canon's own §9 entry when its turn
  comes, never guessed ahead. Open design questions go to the owner as one sign-off package of
  recommendations with defaults.

## State
Done (owner, answer Q3): batches 1 to 17 as `CLAUDE.md`'s history records them, including the mimic (cast
batch one) and its post-merge accept-damage 2→7 tuning; the flexible camera console of the terminal backend
(commit f33a80e; the "awaiting human approval" status line of
`docs/superpowers/specs/2026-08-04-flexible-camera-console-design.md` is stale).

Open owner step: **the mimic is not signed off.** It still needs the owner's playtest. That is an owner
step, not a node. It blocks cast batch two and the ending (the road-to-1.0 gate "Human plays the mimic", and
`CLAUDE.md`'s "Only after sign-off ... is cast batch two briefed"), not the nodes before them. It stays
listed here and in the plan until the owner signs it off (owner, answer Q3a).

Baseline: the gate passes on rustc 1.75 (no known red; see `docs/graph.yaml`).

Planned, in this order (owner, answer Q2 for items 1 and 3; item 2's place is an open decision):
1. **The gate first.** The gate pins the 1.75 toolchain (done in the gate command); then a CI workflow that
   runs the gate on every PR, on a runner with `upx` and rustup's 1.75 (without them `make size` skips the
   budget check and `make msrv` falls back to the ambient toolchain, and both still pass). The repository
   has no CI today. This is the first node.
2. Later work found during adoption: moving the inline unit tests to `src/tests/*.rs` (via `#[path]`), so
   that a tester can write unit tests the guard allows (answer Q10); the
   `suspicious_runtime_symbol_definitions` lint (a warning on rustc 1.100 that the Makefile's `-D warnings`
   turns into an error) on the `read`/`write` FFI at `src/backend_term.rs:56-57`, a node, not a gate change
   (answer Q6).
3. Then the road-to-1.0 order, after the owner's mimic sign-off above: cast batch two (the coat with the
   first mid-run split-spawn, the lost guy, the tired ones, THE STAGE's dressing, the donkey rung-3
   companion, corpse/tombstone persistence; manifest
   `docs/superpowers/plans/2026-07-27-cast-npc-vault-major.md` §A), then the ending as an exam (§9-I), then
   polish, legibility and ship. Each is briefed from the story canon when its turn comes.

## Architecture
One binary crate (`Cargo.toml`, edition 2021, `rust-version = "1.75"`). `src/rng.rs` holds the frozen hash
primitives and named channels; `src/content.rs` the content helpers and `PAL_*` palette; `src/gamedef.rs`
the `GameDef` data contract; `src/games/` the cartridge (`contractor.rs`, selected in `games/mod.rs`);
`src/game.rs` the engine core (tiles, entities, worldgen, turns, `apply_input`); `src/save.rs` the save
format, replay and `state_hash`; `src/headless.rs` the dump, solve, sim and world-hash tooling;
`src/render.rs` the 80×30 cell grid; `src/backend_minifb.rs` and `src/backend_term.rs` the two backends;
`src/main.rs` the argument parsing, backend dispatch and most unit tests (an inline `mod tests`). `tests/`
holds the map and frame goldens, the solver and sim band files and the replay fixture
`tests/fixtures/ref.sav`. `docs/` holds the design docs, the story canon and the batch plans. `Makefile`
encodes the gate (`make check`).

## The regime
The graph (`docs/graph.yaml`) holds the gate (the command, its failure pattern and the failures it already
had: none), the paths out of scope (`Cargo.lock`, the packed `rl144-*-upx` release artifacts, `LICENSE`),
and the adoption areas still to map, one per PR in this order: `cartridge`, `save-headless`, `presentation`,
`main-tests`, `project`, `docs`. Each mapped area leaves that list and becomes imported nodes
(`status: imported`) that own its files and describe what the code does today (`engine-core` became E01 to
E04); the planned work above becomes planned nodes once every area is mapped.

## Node spec template
A node spec `nodes/<ID>-<name>.md` has the headings Purpose, Tests, Changes, Gate and Risks, a header row
`| Track | Deps | Unlocks | Size |`, an **Owns:** line, and criteria `- **<ID>-AC-<n>** — … *Verify:* …`. A
node whose change diffs a golden map layout names the owner's MAJOR sign-off in its Risks before it is
built; a node that moves a band names the band files it re-baselines and carries the flip sign-off line.

## Open decisions
- **AGENTS.md is a symlink to CLAUDE.md** (answer Q5): the "Building a node" section lives in `CLAUDE.md`.
  At a commit, `git show <rev>:AGENTS.md` yields the symlink's target, not the text, so the checkers read no
  **Spec files:** line from a commit and fall back to their default list (`SPEC.md`, `docs/graph.yaml`,
  `nodes/`), which is the same list. Default: keep the symlink, and keep the spec files list equal to that
  default; widening it needs AGENTS.md to become a real file first.
- **Who may write `CLAUDE.md`**: its done-rule 6 has a change update it in the same change, so
  `.noodlebot/roles.yaml` drops it (and `AGENTS.md`) from the guard's spec paths, and it belongs to the
  `project` area. Default: a node that makes a `CLAUDE.md` invariant false updates `CLAUDE.md` only when it
  owns it or depends on its owner, as the owns rule says; otherwise the change is a follow-up node.
- **Unit tests before the move**: the guard lets only the implementer write `src/*.rs`, where every unit
  test lives, so a tester can add no compiled unit test until the inline tests move to `src/tests/*.rs`.
  Default: the test move is the first node after the CI node, and a node before it takes its red target from
  a build or a headless mode (`--dump`, `--solve`, `--sim`), not from a new unit test.
- **Order of the adoption follow-ups** (planned item 2): the answers place the gate first and the
  road-to-1.0 work after it, but not the follow-ups found during adoption. Default: the test move, then the
  FFI lint node, both after the CI node and before cast batch two.
- **Golden and band re-baselines**: the goldens and band files under `tests/` are re-generated after an
  implementation, so the guard's tester-only paths are narrowed to `src/tests/**` and `tests/fixtures/**`.
  Default: an implementer re-baselines `tests/golden/*` or `tests/*-band.json` only when the node's spec
  says so (with the MAJOR sign-off for a map-layout change).
- **Tests the imported criteria name**: the criteria of E01 to E04 name 186 unit tests in `src/main.rs` (the
  `main-tests` area) and, through `Makefile` targets, the map and frame goldens, the band files and
  `tests/fixtures/ref.sav`. The node skill freezes the tests an imported node's criteria name: a later node
  may add cases, and changes or removes one only with the owner's approval. Default: the owner's MAJOR
  sign-off, or a band re-baseline that a node's spec names, is that approval for the files it names; the test
  move keeps each named test's name and body, needs the owner's approval as a move of frozen tests, and is
  followed by a spec PR that points the *Verify:* lines at the new files.
- **Gate run time and CI evidence**: the whole gate takes about 35 minutes and there is no CI yet. Default:
  once the CI node lands, CI runs the gate on every PR and a session runs the quick gate locally; until then
  a node's PR records a local run of the gate command on its head commit as its evidence.

## Conventions
Branches `node/<ID>-<name>` and `spec/<topic>`; sizes S (under a day), M (two or three days), L (more); a
node is done when its gate passes (in CI once the CI node lands; see the open decisions) and its PR is
merged. Commit messages and the `CLAUDE.md` status entries follow the repository's existing style.
