# B02 — makefile

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | B01, M01, M02, M03, M04 | B03 | M |

**Owns:** `Makefile`, the gate (`make check`) and the size scoreboard (`make targets`) as POSIX `sh` recipes.

## Purpose
`make check` is the repository's whole verification gate; the graph's gate command runs it as
`PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.75 make -k check`. Its prerequisites, in order:
`build test test-term goldens frames xhash solve sim flip size msrv`. Variables: `UPX ?= upx`,
`BUDGET ?= 1474560`, `SOLVE_SEEDS ?= 10000`, `SIM_SEEDS ?= 5000` (`UPX` is `unexport`ed, because `upx` reads
an environment variable of that name as options); `BIN` is `target/release/rl144`, `TERM_BIN`
`target/term/release/rl144`, `GOLDEN_SEEDS` `1 2 3 42 1337`, `FRAME_SEEDS` `1 42`, `REF_SAVE`
`tests/fixtures/ref.sav`.
- `build`: `RUSTFLAGS="-D warnings" cargo build --release` (the window backend). `build-term`: the same with
  `--no-default-features --features backend-term --target-dir target/term`, so the two never share artifacts.
- `test`, `test-term`: `cargo test --quiet` under each feature set (debug builds).
- `goldens` (after `build`): dumps each golden seed with `--dump --seed N` into a `mktemp -d` folder,
  `cmp`s it with `tests/golden/seed_N.txt`, prints `golden mismatch for seed N:` and a unified diff for each
  difference, then `goldens: FAIL` (exit 1) or `goldens: OK (...)`. It never writes into the working tree.
- `frames` (after `build-term`): the same for `--render-frame --seed N` against
  `tests/golden/frame_seed_N.bin` and `--render-frame --seed 1 --ascii` against `frame_seed_1_ascii.bin`,
  printing `frame golden mismatch for seed N` (or `... seed 1 (ascii)`), then `frames: FAIL` or `frames: OK`.
- `xhash` (after both builds): replays `REF_SAVE` through both binaries, extracts each `"hash":"<hex>"` with
  `sed`, and prints `xhash: FAIL — ...` when either is missing or they differ, else `xhash: OK (<hash>)`.
- `solve`: `--solve $(SOLVE_SEEDS)`. `sim`: `--sim $(SIM_SEEDS)` with the default (greedy), `pacifist`,
  `tactical` and `tactical-pacifist` policies. `flip`: `--sim-flip $(SIM_SEEDS)`. The binary checks the band
  files itself (S02, M03).
- `size` (after `build`): prints the stripped size, packs a `mktemp` copy with `upx --best --lzma -qq`
  (never the file in `target/`), prints `packed size: N bytes (budget B)` and `size: FAIL — ...` (exit 1) when
  the packed copy exceeds `BUDGET`, else `size: OK`.
- `msrv`: with `rustup` on `PATH`, installs 1.75 if missing and runs `rustup run 1.75 cargo check --release`
  for both feature sets, printing `msrv: OK (...)` or `msrv: FAIL — ...` (exit 1).
- `targets` (not part of `check`): after both builds, a table of the stripped and packed size of each binary
  with its percentage of `BUDGET`, and `targets: FAIL — ...` for a binary over budget.

## Tests
None: the Makefile is the gate itself, and no tracked test runs its recipes. Its targets are named as the
*Verify:* of the other nodes' criteria (for example `make goldens`, `make xhash`, `make sim`).

## Changes
none: imported

## Gate
- **B02-AC-1** — `make check` runs `build`, `test`, `test-term`, `goldens`, `frames`, `xhash`, `solve`, `sim`, `flip`, `size` and `msrv`, and exits non-zero when any of them fails. *Verify:* unpinned (no tracked test runs the gate against a known failure; dropping a prerequisite still passes).
- **B02-AC-2** — Both release builds run with `RUSTFLAGS="-D warnings"`, the terminal build in `target/term`, so a warning fails the gate. *Verify:* unpinned.
- **B02-AC-3** — `goldens`, `frames` and `xhash` compare the fresh outputs byte for byte (hashes for `xhash`) with the committed fixtures, write nothing into the working tree, and print the failure lines listed in Purpose. *Verify:* unpinned.
- **B02-AC-4** — `solve`, `sim` and `flip` run 10,000, 5,000 and 5,000 seeds, `sim` once per policy for all four policies. *Verify:* unpinned.
- **B02-AC-5** — `size` packs a copy outside `target/` and fails when the packed window binary exceeds 1,474,560 bytes. *Verify:* unpinned.
- **B02-AC-6** — `msrv` checks both feature sets under rustc 1.75 when `rustup` is present and fails on an error. *Verify:* unpinned.
- **B02-AC-7** — Every failure line the Makefile's own recipes print for a gate target (`golden mismatch for seed N`, `frame golden mismatch for seed N`, `xhash: FAIL`, `size: FAIL`, `msrv: FAIL`) matches the graph's `gate.fail_pattern`. *Verify:* unpinned.

## Risks
- Two targets pass without checking: `size` prints a warning and exits 0 when `upx` is missing or fails to
  pack, and `msrv` without `rustup` falls back to `cargo check` under the ambient toolchain with a warning. A
  CI runner needs both `upx` and rustup's 1.75 (SPEC.md's planned CI node).
- `size` checks only the window binary; the terminal binary's budget is checked only by `make targets`, which
  is not in `check`, although `CLAUDE.md` asks for the packed size of each backend against the budget.
- A compile error in `build`, `build-term` or `test` prints rustc's `error` lines and make's
  `*** [Makefile:N: <target>] Error` line, none of which `gate.fail_pattern` matches (it matches lines that
  name a failing test or check); under `-k`, the targets that need the failed build are skipped, not failed.
- `xhash` depends on `--replay`'s `"hash":"<hex>"` output (M01-AC-4); a format change there makes it fail
  with "could not extract hash".
- `SIM_SEEDS` must stay equal to `flip_n` in `tests/tactical-pacifist-band.json`; a mismatch only prints a
  note (M03's Risks). The golden and frame seed lists must match the files in `tests/golden/` and the list in
  `src/main.rs`'s `golden_dumps` (M02's Risks).
- Recipes run under `/bin/sh` without `~/.cargo/env`, hence the `PATH` prefix in the gate command.
- The `msrv` comment is stale: it says this environment has no rustup, while rustup with 1.75 is installed
  on the adoption's machine now (owner, answer Q6).
- The whole gate takes about 35 minutes (the owner's run: 34m43s); each debug `cargo test` run takes about
  five.
