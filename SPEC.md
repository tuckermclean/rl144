# rl144 — SPEC

The plan of record for building rl144 node by node. The graph is `docs/graph.yaml`; each node's
spec is `nodes/<ID>-<name>.md`; how to build one is the "Building a node" section of `AGENTS.md`.
Authority order: this file, then `docs/graph.yaml`, then the node spec, then `AGENTS.md`.
The evidence behind this plan (what the code does today, run results, defects) is
`docs/AUDIT.md` (audited at `f33a80e`, 2026-11-10).

## What this is

rl144 is a deterministic, headless-verifiable roguelike **engine** in Rust whose shipped binary
fits a 3.5" floppy (1,474,560 bytes, UPX-packed, per backend), plus its first cartridge,
`src/games/contractor.rs` (the "contractor" game). One binary crate; dependencies frozen to
`minifb` + `font8x8`; two backends (`backend-minifb` window, `backend-term` terminal).

## Scope

**Now (the graph): the engine.** The shared core (`rng`, `content`, `gamedef`, `game`, `save`,
`render`, `headless`), the backends, save/replay, the headless sim and bots, tooling, CI and the
ship pipeline, so that one or more games can be finished on it later (owner, 2026-11-11).
Where the audit shows the engine is missing something a game will clearly need (an input-byte
layout that can grow without breaking replays, an enforced engine/game boundary), that is engine
work and is in the graph.

**Later: game content and game design** for the contractor cartridge. These are not `now`
nodes; they are listed under "Later work" and become nodes when the owner re-prioritizes.

**Not this repository:** the real-i486 artifact (nightly, `build-std`, the ebuild, the ISO) is
owned by the external ISO factory (`docs/design/486/2026-08-04-boot-on-a-486-scope.md`).
**Permanently cut:** mod support, config files, localization.

## Principles

Every node respects these. The doctrine sections of `AGENTS.md` (`CLAUDE.md`) named here are
adopted as principles of this spec; a node that would break one stops and asks the owner.

1. **Engine first.** No node adds or changes game content: no new monsters, items, rooms, lines
   or balance, no draft flavor text or draft overworld maps, and never a `[YOURS]` slot (owner
   answers Q6, scope).
2. **The floppy budget is hard.** Each backend's UPX-packed binary stays ≤ 1,474,560 bytes; every
   node that adds code reports stripped and packed sizes with the toolchain ("Size budget rules").
3. **Frozen toolchain and dependencies.** MSRV 1.75; `minifb` + `font8x8` only; no new crate
   without the AGENTS.md justification. `make check` stays stable-1.75-only and never learns
   nightly, `build-std` or i486.
4. **Core/crust and cartridge doctrine.** The core has no platform calls and no `cfg` (except
   `games/mod.rs`); engine and backend files hold no game nouns; game facts live on `GameDef`.
5. **Determinism and saves.** Saves are seed + input log. Every old save replays byte-identically.
   `SAVE_VERSION` bumps on input-vocabulary growth or a hashed-state shape change, as the save
   doctrine says (owner answer Q2: a node that changes hashed state bumps; the cast's v12 bump
   comes with the cast, later). No `usize` in hashed state.
6. **Human gates (answer Q7).** A node that changes a golden map layout (a worldgen MAJOR) or
   moves a band carries a graph decision the owner signs before it is built. No `now` node is
   planned to do either; a node that finds it must stops and asks. Acceptance criteria are
   headless only. "Needs interactive playtest" is a post-merge owner step, never a criterion.
7. **Frozen fixtures (answer Q8).** Golden dumps (`tests/golden/**`) and band files
   (`tests/*-band.json`) are rewritten only in nodes the owner has signed. Every node's gate
   includes "goldens, frames, `xhash` and bands unchanged" unless its signed decision says
   otherwise.
8. **Instrument parity (answer Q4).** A node that adds a move a bot archetype could take teaches
   that bot the move in the same node. The batch-17 exception (diplomat bots never set the sword
   down) is accepted as it stands; the flip held at +9.9.
9. **Headless-first verification.** Every behaviour is observable through `--dump`, another
   headless flag, a unit test or a scripted run; `make check` is the gate.

## Architecture

As `AGENTS.md` "Project shape" and "Core/crust doctrine" describe, and `docs/AUDIT.md` §2a–2c
confirms: the core exposes `render_cells`, `scene()` (reserved, no consumer yet), the input-byte
vocabulary through `Game::apply_input(u8)`, and the headless entry points; backends consume only
that surface. The active cartridge is selected in `src/games/mod.rs`, the one place that names a
game module.

**Already done (not nodes).** Batches 1–17 and the camera console (`f33a80e`): channel RNG and
goldens, save/replay (`SAVE_VERSION` 11), `xhash`, `--solve`/`--sim`/`--sim-flip` gates, four bot
policies, the cartridge split, give/use/held items, mercy-as-talk, light-as-grace, portals and
authored floors, the overworld skeleton, the talk-minigame framework, guaranteed NPC vaults,
ghost writing, the terminal camera console (works on x86_64; broken on 32-bit, B01). `make check`
is green at `f33a80e` under rustc 1.75.0 (audit §3).

**Test layout.** Today every unit test is inline (`mod tests` in `main.rs`, `render.rs`,
`backend_term.rs`, `backend_minifb.rs`). T01 moves them to `src/tests/<module>.rs`. The role
guard's paths are in `.noodlebot/roles.yaml`: tests `src/tests/**`, `tests/fixtures/**`,
`**/*.test.sh`; specs `SPEC.md`, `docs/graph.yaml`, `nodes/**`, `.noodlebot/**`. Implementers may
edit `CLAUDE.md` (= `AGENTS.md`) except its "Building a node" section (answer Q8).

## Order

T02 (CI) first: every node's evidence is a CI run. Then T01 (test layout) before any node that
adds Rust tests. B01 and B02 are the audit's defects (answer Q1). E01 and H01 enforce the
engine/game boundary and the required-vault guarantee for future cartridges. D01 brings the docs
in line. S01 and S02 are the ship pipeline; S01 waits for B01 so the 32-bit artifact it ships has
a working console.

## Later work (not in the graph; owner re-prioritizes)

Game content and design for the contractor cartridge (owner, 2026-11-11), in the audit's order:
- cast MAJOR 2: D2/D4 required rooms plus `SAVE_VERSION` 12 reserving the custody, exit-axis,
  carrier-identity and corpse fields (answer Q2);
- the coat: `Minigame::AnswerSecondVoice`, the first mid-run split-spawn, bot parity;
- the lost guy and the towel; the tired ones and THE STAGE dressing;
- donkey rung 3 (LOVE tier, descent, ogre becalm, the treat; IN for 1.0 per the 2026-07-27
  manifest; needs B02 first because it adds an item);
- the §9-J back half: corpse/tombstone/inventory persistence, the wake/drag-marks lines,
  the contract screen, trainer quest flags;
- the ending (§9-I/K): mantel exam, Judge, three exits, win relocation, donkey re-glyph;
- the content pass (real overworld screens, the remaining flavor lines, `[YOURS]` slots by the
  owner) and the pre-budgeted feel-revision batch;
- the game's acceptance list in `SHIP.md` (S01 leaves a section for it).
Engine pieces those games need (mid-run spawn, a contract `Screen`, new hashed fields) are built
in the game node that first needs them, with instrument parity.

Post-1.0 (as `AGENTS.md` and the road-to-1.0 plan say): ghost playback, `Dest::World`
enrichment, scale/journey B, audio, sprites, wasm/net, `monomyth.md`, and DECISION.md's
"C-complete" items (answer Q9c). The fbdev backend: see OD5.

## Owner steps (outside the graph)

- **Mimic playtest** (answer Q3): not done and not signed off. An owner step for later, before
  cast batch two; nothing in the `now` graph depends on it.
- **Camera console interactive playtest** on a real console, after B01 merges.
- **First release tag**: after S01 merges, the owner pushes the first `v*` tag (OD3).
- Signing any graph decision that a MAJOR or band-moving node carries (principle 6).

## Open decisions

Each has a default; a node whose `decisions` lists one is built on the default unless the owner
says otherwise before it starts.

| ID | Node | Decision | Default |
|---|---|---|---|
| OD1 | B02 | Bytes for item kinds ≥ 10 | Bytes 0–36 keep today's meaning (use-by-kind 17–26, set-down-by-kind 27–36 for kinds 0–9). Kind k ≥ 10 uses byte 37+2(k−10) and sets down with 38+2(k−10); at most 119 kinds. The first cartridge change that adds an 11th item grows the vocabulary and bumps `SAVE_VERSION`. |
| OD2 | T02 | CI triggers and environment | `pull_request`, `push` to `main`, `workflow_dispatch`; one job in the `rust:1.75-slim` container with `make`, `g++`, `xz-utils` and upx 4.2.4 from the upstream release (SHA-256 checked); timeout 120 min. |
| OD3 | S01 | Release artifacts and gate | `make ship` runs `make check`, then builds x86_64-linux-gnu minifb, x86_64-linux-gnu term and i686-linux-musl term (static), packs each with `upx --best --lzma` into `dist/`, fails on any packed size over budget, writes `dist/SHA256SUMS`. A release workflow runs it on `v*` tags (publishing a GitHub release) and on `workflow_dispatch` (workflow artifacts only). The owner pushes tags; the first is `v0.1.0`, matching `Cargo.toml`. |
| OD4 | S02 | Floppy image contents and tool | mtools (`mformat`, `mcopy`, no root); `dist/rl144-floppy.img`, 1,474,560 bytes, FAT12, holding the `make ship` binaries, `SHA256SUMS` and a `README.TXT`. |
| OD5 | — | fbdev backend for the 486 target (answer Q9a, delegated) | Post-1.0, no node. The term backend on a Linux text console covers the 486 target once B01 fixes 32-bit; the factory's `fb640` boot label is a later glow-up. |
| OD6 | D01 | `--sim` default: doc says 5000, code says 1000 | Fix the doc to 1000; the code is unchanged (the Makefile passes 5000 explicitly). |
| OD7 | H01 | How `--solve` sees required-vault placement | Level generation records, per depth, whether each `GameDef::required_vaults` entry was stamped. The record is derived, unhashed and listed as derived in `state_hash`'s doc comment. |
| OD8 | T01 | Test file layout | One file per former inline module: `src/tests/main.rs`, `render.rs`, `backend_term.rs`, `backend_minifb.rs`, each included as `#[cfg(test)] #[path = "tests/<module>.rs"] mod tests;`. Splitting `main.rs`'s 245 tests by topic is later. |
| OD9 | B01 | Where the 32-bit check runs | A `make i686` target outside `make check` (i686-unknown-linux-musl is a stable target, but `check` stays host-only), run by its own CI job. The pty check uses `script` from util-linux. |
| OD10 | E01 | Which nouns the guard checks | A fixed list in the test (rat, goblin, ogre, mimic, trainer, donkey, potion, sword, cheese, coat, towel) plus every name in the cartridge's `ThemeDef::mobs` tables, matched case-insensitively as whole words. Files: `rng`, `content`, `gamedef`, `game`, `save`, `headless`, `render` and both backends (`games/` and `main.rs` excluded). Line and block comments are stripped first; string literals and identifiers are checked. |

## Node spec template

Every `nodes/<ID>-<name>.md` has:
- the title `# <ID> — <name>`;
- the header row `| Track | Deps | Unlocks | Size |` matching the graph (`none` when empty);
- **Owns:** the paths the node creates or changes; **Decisions:** the open decisions it relies on;
- these `##` headings, in order:
  - `Purpose`: why the node exists and what is already there;
  - `Tests`: the tests to keep, the tests to add, and the red target;
  - `Changes`: numbered; each names the criterion it produces (`*<ID>-AC-n.*`);
  - `Gate`: the criteria, one line each,
    `- **<ID>-AC-<n>** — <testable statement> *Verify:* <command, test or CI log>`, numbered from 1;
  - `Risks`: what could go wrong, and what is out of scope.

## Conventions

- Branches: `node/<ID>-<name>` for a node, `spec/<topic>` for a spec change. The owner merges.
- Node ids: one letter per family, two digits: T tooling and tests, B defects, E engine boundary,
  H headless verification, D docs, S ship.
- Sizes: S under a day, M two or three days, L more.
- A node is done when every criterion has its evidence (a CI run URL for anything that builds),
  `make check` is green in CI, sizes are reported, and the owner has merged its PR.
- Criteria are checked against the principles above before a node spec changes.

## Known issues not planned

Recorded from the audit; each needs the owner before it becomes work.
- An unknown flag (e.g. `--help`) opens the window; with no display it panics (audit §3).
- `cargo clippy` reports one deny-level lint (`render.rs:272`, harmless) and 5 warnings; the code
  is not rustfmt-formatted. Neither is a gate.
- On non-play screens a small terminal crops the top-left of the title (audit #74).
- `tests/solver-band.json`'s comment says "exits in BFS-deepest room", no longer true on D5; it
  changes with the next owner-signed band node (principle 7).
- `room_visited` gates a flavor draw but is not hashed (audit §5.4); low risk, documented by D01.
