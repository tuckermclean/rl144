# D06 — 486-scope

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | none | S |

**Owns:** `docs/design/486/`, the 486-boot scope note and its custom `i486-unknown-linux-musl` target spec, kept as reference.

## Purpose
- `2026-08-04-boot-on-a-486-scope.md`: a scoping and handover note (status: phase-1 de-risk complete). It records
  what was run on rustc 1.75.0: a cross-build of the terminal backend for `i686-unknown-linux-musl` whose
  `--replay tests/fixtures/ref.sav` state hash (`1f22670bf826471a`) matched both x86_64 backends, and i486
  codegen through the custom target spec, with the 486SX caveat. It draws the boundary: rl144 owns the source and
  its 32-bit cleanliness, the camera-console feature and this reference material; the ISO factory owns the
  nightly/`build-std` toolchain, the ebuild, the console video mode, the kernel pin and the hardware bring-up. Its
  invariant for this repository: the `make check` gate stays stable-1.75-only and never learns `nightly`,
  `build-std` or `i486`. It closes with a staged plan (stage 1 done) and the one rl144 code deliverable it found,
  the flexible camera console (built in `src/backend_term.rs`, commit f33a80e; its design is in D07).
- `i486-unknown-linux-musl.json`: a rustc custom target spec (`"cpu": "i486"`, features
  `-cmov,-mmx,-sse,-sse2`, static musl), described in the note as reference material for the factory, not wired
  into the `Makefile` or `make check`.
`src/backend_term.rs` cites this folder for its termios layout being correct on i686/i486 musl.

## Tests
None: no tracked test or `make check` step reads these files; the `Makefile` names no `nightly`, `build-std` or
`i486`. The replay hash the note quotes is the one `make xhash` checks today (M04), but nothing compares the note
with it.

## Changes
none: imported

## Gate
- **D06-AC-1** — The note states the boundary above, with the invariant that `make check` stays stable-1.75-only and never uses `nightly`, `build-std` or `i486`. *Verify:* unpinned (prose).
- **D06-AC-2** — `i486-unknown-linux-musl.json` is valid JSON naming `"cpu": "i486"` and `"llvm-target": "i486-unknown-linux-musl"`, and no `Makefile` target uses it. *Verify:* unpinned.

## Risks
- The note's measurements (258 tests, the replay hash, codegen) are of 2026-08-04; a later change to the save,
  the replay fixture or the test count makes them historical without a test noticing.
- It calls the camera console the one remaining deliverable "needing its own brainstorm"; that is done (owner,
  answer Q3b), so the note's staged plan is stale on that point.
- The JSON is hand-written for a nightly toolchain; a rustc change to the custom-target format would break it
  without this repository's gate noticing, by design.
