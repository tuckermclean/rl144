# B01 — build-config

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | B02 | S |

**Owns:** `Cargo.toml` and `.gitignore`, the crate manifest (package, MSRV, the two optional dependencies, the backend features, the size-tuned release profile) and the ignore list.

## Purpose
`Cargo.toml` declares one binary crate, `rl144` 0.1.0, edition 2021, `rust-version = "1.75"`, MIT licence.
- **Dependencies**: `minifb` 0.25 (`default-features = false`, `features = ["x11"]`) and `font8x8` 0.3
  (`default-features = false`), both `optional`. Its comment records why `font8x8`'s `unicode` feature is off:
  the window backend indexes only the `legacy` tables directly. `CLAUDE.md` freezes this list.
- **Features**: `default = ["backend-minifb"]`; `backend-minifb` pulls in both dependencies;
  `backend-term` pulls in none. `src/main.rs`'s two `compile_error!`s require exactly one of the two, so the
  default build is the window backend and the terminal build is
  `--no-default-features --features backend-term`.
- **Release profile**: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`,
  `strip = true`, the size flags `CLAUDE.md` forbids removing. On rustc 1.75 the stripped window binary is
  674,152 bytes (local build, 2026-11-20); the owner's gate run packed it to 222,488 bytes.

`.gitignore` ignores `/target`, `*.sav` (the save files a session writes) and `.superpowers/` (the
machine-local progress ledger `CLAUDE.md` mentions).

## Tests
No test reads these files directly. The `Makefile` targets build through them: `build` (default features,
`RUSTFLAGS="-D warnings"`), `build-term` (`--no-default-features --features backend-term`), `msrv`
(`cargo check` under the pinned rustc 1.75 for both feature sets) and `size` (the packed budget).

## Changes
none: imported

## Gate
- **B01-AC-1** — The default feature set is `backend-minifb`, so a plain release build selects exactly one backend and compiles. *Verify:* `make build` (target of `Makefile`; with `default = []` it fails on `src/main.rs`'s `compile_error!`).
- **B01-AC-2** — The manifest declares a `backend-term` feature that builds the terminal backend. *Verify:* `make build-term` (target of `Makefile`).
- **B01-AC-3** — The declared `rust-version` lets the crate build on rustc 1.75, under both feature sets. *Verify:* `make msrv` (target of `Makefile`; with `rust-version = "1.80"` cargo 1.75 refuses the package and it prints `msrv: FAIL`).
- **B01-AC-4** — The only dependencies are `minifb` (no default features, `x11`) and `font8x8` (no default features), both optional and enabled only by `backend-minifb`. *Verify:* unpinned (making either non-optional, or adding a crate, still builds and passes the gate).
- **B01-AC-5** — The release profile keeps `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"` and `strip = true`. *Verify:* unpinned (`make size` checks only the packed budget, which the binary meets without these flags).
- **B01-AC-6** — `.gitignore` keeps build output, save files and `.superpowers/` out of git. *Verify:* unpinned.

## Risks
- The dependency freeze and the size flags are rules of `CLAUDE.md`, not checks: a node that adds a crate or
  drops a flag passes the gate as long as the packed binary stays under 1,474,560 bytes. A new crate needs the
  measured size cost `CLAUDE.md` asks for.
- `minifb`'s `x11` feature is set for every target; `CLAUDE.md` notes that macOS and Windows need its default
  features as a target-specific flag (P02's Risks).
- `Cargo.lock` (out of scope) pins the resolved versions; the owner's gate run confirmed they build on 1.75.
  A dependency update that raises a transitive MSRV fails `make msrv`.
- `*.sav` is ignored everywhere, but `tests/fixtures/ref.sav` is tracked (force-added despite the rule);
  a new fixture save must be added with `git add -f`.
