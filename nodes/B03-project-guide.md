# B03 — project-guide

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | B02 | none | S |

**Owns:** `CLAUDE.md`, the project guide (rules, verification, doctrine, status and history), which `AGENTS.md` links to.

## Purpose
`CLAUDE.md` is the repository's working guide; `AGENTS.md` is a symlink to it (SPEC.md's open decisions).
`SPEC.md` makes its rules the principles every node keeps (owner, answer Q4). Its sections:
- **Where this stands**: the status snapshot (batches 1 to 17, the road-to-1.0 plan, the mimic's open
  playtest, the canon's §9 checklist, the post-1.0 list, open placeholders). It says itself that it goes stale.
- **Project shape**: the module list and the frozen dependencies (`minifb`, `font8x8`), no engines, no asset
  files.
- **Core/crust doctrine**, **Cartridge doctrine**, **Size budget rules** (1,474,560 bytes packed, the
  `Cargo.toml` size flags), **Verification** (`make check` and each headless mode), **Seed compatibility**
  (map-layout changes are MAJOR, `SAVE_VERSION` 11), **Code conventions** (channels, coordinates, the
  input-byte vocabulary, mercy, light, ASCII-only strings, MSRV 1.75), **Platform notes**, **Roadmap
  context**, **Definition of done** and **Process discipline**.
- **Building a node**: the regime's section of `AGENTS.md` (through the symlink): `/noodlebot:node <ID>`,
  the authority order, the roles, the spec checkers, the gate command and quick gate, known red, owns, the
  red target, the test and fixture paths, the **Spec files:** line, porting rules and CI evidence.
- **History**: one line per batch, and the instruction to append a dated entry per change.

## Tests
None: the file is prose, and no tracked test reads it. At a commit the regime's checkers read no **Spec
files:** line through the `AGENTS.md` symlink and use their default list, which is the same (SPEC.md's open
decisions).

## Changes
none: imported

## Gate
- **B03-AC-1** — The guide states the rules `SPEC.md` makes the principles: the 1,474,560-byte packed budget, the frozen dependencies, the core/crust and cartridge doctrines, the seed MAJOR and `SAVE_VERSION` doctrine, headless-first verification with instrument parity, ASCII-only strings and MSRV 1.75. *Verify:* unpinned (prose).
- **B03-AC-2** — The "Building a node" section names the same gate command as `docs/graph.yaml`'s `gate.command`, the quick gate through `cargo test --quiet <filter>`, the tester and spec paths of `.noodlebot/roles.yaml`, and a **Spec files:** line `SPEC.md`, `docs/graph.yaml`, `nodes/`. *Verify:* unpinned.
- **B03-AC-3** — The facts it states about the code hold: the module list of "Project shape", `SAVE_VERSION 11`, the input-byte vocabulary and the presentation-only exclusion set it repeats from `save.rs`. *Verify:* unpinned.

## Risks
- Done-rule 6 asks a change that makes one of its invariants false to update this file in the same change;
  SPEC.md's open decision limits that to a node that owns it or depends on B03's owner, else a follow-up node.
- Stale lines today: "Batches 1–17 merged to `master`" (the default branch is `main`); "This container's
  default rustup toolchain is a real 1.75.0" (on the adoption's machine rustup's default is stable, with 1.75
  installed beside it, which is why the gate command sets `RUSTUP_TOOLCHAIN=1.75`); story §12.14's second
  cheese give-target listed as open, though C01's Risks find it resolved in batch 13. The mimic's playtest is
  still open (owner, answer Q3a), as the file says.
- The size ballpark (~660 KB stripped, ~220 KB packed on rustc 1.75) matches today's build (674,152 bytes
  stripped; 222,488 packed in the owner's gate run) but is not checked.
- It repeats facts whose source of truth is elsewhere (the exclusion set in `state_hash`'s doc comment, the
  band numbers in `tests/*-band.json`, `SAVE_VERSION` in `src/save.rs`); a node changing those must keep the
  copies here true.
- Its own text is not ASCII-only (em-dashes, arrows); the ASCII rule applies to rendered strings, not to
  this file.
