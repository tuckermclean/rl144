# C02 — games-seam

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | C01 | none | S |

**Owns:** `src/games/mod.rs`, the cartridge selection seam.

## Purpose
`src/games/mod.rs` is the one engine-side file that names a game module (`src/main.rs`'s tests also import
`games::contractor`). It declares
`pub(crate) mod contractor;` and re-exports `pub(crate) use contractor::GAME;`, so every engine file reaches
the active cartridge as `crate::games::GAME` and stays free of any one game's nouns. The module is
`pub(crate)` so that `src/main.rs`'s tests can import the cartridge's named kind indices
(`games::contractor::{...}`). Swapping the shipped cartridge is a change to these two lines; a future second
cartridge may select one with a `cfg` gate here, the one `cfg` the core may hold. The file has no `cfg` today.

## Tests
No test exercises the seam alone; the build checks it (both backends), and every test and headless mode
reaches the cartridge through it.

## Changes
none: imported

## Gate
- **C02-AC-1** — `crate::games::GAME` resolves to the contractor cartridge; the crate builds warning-free under both backends (`make build`, `make build-term`) and checks on rustc 1.75 (`make msrv`). *Verify:* `make build`, `make build-term` and `make msrv` (targets of `Makefile`).
- **C02-AC-2** — `src/main.rs`'s tests reach the cartridge's named indices through `games::contractor`. *Verify:* `make test` (target of `Makefile`).
- **C02-AC-3** — No code outside `src/games/` (other than `src/main.rs`'s tests) names a game module (some comments do); engine files use only `crate::games::GAME`. *Verify:* unpinned (the `CLAUDE.md` rule, not a tracked check).

## Risks
- Pointing the re-export at another cartridge is a MAJOR event for seeds and saves (`CLAUDE.md`): every
  golden, band and save changes.
- A `cfg` selector added here must stay the only `cfg` in the core; `CLAUDE.md` permits it in this file
  alone.
