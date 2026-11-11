# T01 — split-inline-tests

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | T02 | B02, E01, H01 | M |

**Owns:** `src/tests/` (new), `src/main.rs`, `src/render.rs`, `src/backend_term.rs`, `src/backend_minifb.rs` (removing the inline test modules and their `#[cfg(test)]` imports only), `CLAUDE.md` ("Project shape" line for `main.rs`), `tests/test-layout.test.sh` (new)
**Decisions:** OD8

## Purpose
All 270 unit tests are inline: `src/main.rs` (245, lines 217–5961, plus `#[cfg(test)]` imports at
lines 34–56), `src/backend_term.rs` (16), `src/render.rs` (8), `src/backend_minifb.rs` (1)
(audit §4). Path globs cannot tell them from production code, so the role guard cannot hold the
tester and implementer to their roles. This node moves them, unchanged, into
`src/tests/<module>.rs`, each included from its old module with `#[path]`, so `use super::*` and
private access keep working (answer Q8). It changes no test and no behaviour.

## Tests
Keep all 270 tests, byte-for-byte in body; only their file moves (the module stays `tests`, so
test names do not change). Add `tests/test-layout.test.sh`, which fails while any `#[test]` or
inline `mod tests {` is left in `src/*.rs` outside `src/tests/`, and passes when none is.
Red target: `sh tests/test-layout.test.sh` (non-zero at the base, 0 after). Before the move,
record `cargo test --quiet -- --list` for both backends as the baseline name lists.

## Changes
1. The tester writes `src/tests/main.rs`, `src/tests/render.rs`, `src/tests/backend_term.rs` and
   `src/tests/backend_minifb.rs`, each holding the body of the old inline module, including the
   `#[cfg(test)]` imports `main.rs` declares for its tests, and `tests/test-layout.test.sh`.
   *T01-AC-1, T01-AC-2.*
2. The implementer replaces each inline `mod tests { … }` with
   `#[cfg(test)] #[path = "tests/<module>.rs"] mod tests;` and removes the `#[cfg(test)]` imports
   that moved. *T01-AC-1, T01-AC-2, T01-AC-3.*
3. Update `CLAUDE.md` "Project shape": `main.rs` is arg parsing and backend dispatch; tests live in
   `src/tests/`. *T01-AC-5.*

## Gate
- **T01-AC-1** — No file in `src/` outside `src/tests/` contains `#[test]` or an inline `mod tests {`. *Verify:* `sh tests/test-layout.test.sh` exits 0 at the head and non-zero at the base.
- **T01-AC-2** — `src/main.rs`, `src/render.rs`, `src/backend_term.rs` and `src/backend_minifb.rs` each include their tests with `#[cfg(test)] #[path = "tests/<module>.rs"] mod tests;`. *Verify:* `grep -n 'path = "tests/' src/main.rs src/render.rs src/backend_term.rs src/backend_minifb.rs` shows four lines.
- **T01-AC-3** — The set of test names is unchanged on both backends: 254 under the default features and 269 under `backend-term`. *Verify:* `cargo test --quiet -- --list` and `cargo test --quiet --no-default-features --features backend-term -- --list` at the head diff empty against the base's lists; counts in the CI log.
- **T01-AC-4** — `make check` is green in CI and no golden, frame golden, band file or fixture changed. *Verify:* the check workflow's run URL for the PR head; `git diff --stat <base>..HEAD -- tests/golden tests/fixtures 'tests/*-band.json'` empty.
- **T01-AC-5** — `CLAUDE.md` "Project shape" names `src/tests/` as the home of unit tests and no longer lists tests under `main.rs`. *Verify:* `grep -n 'src/tests' CLAUDE.md`.

## Risks
- `main.rs`'s tests import crate items through `#[cfg(test)] use` lines at the crate root; they
  must move into `src/tests/main.rs` as `use crate::…` or `use super::…` without changing any
  test body.
- `#[path]` in a non-`mod.rs` file resolves relative to that file's directory (`src/`), which is
  what makes `tests/<module>.rs` land in `src/tests/`.
- The tester's new files are not compiled until the implementer wires them; the red target is the
  layout script, not `cargo test`.
- Out of scope: splitting `main.rs`'s 245 tests by topic, `tests/*.rs` integration tests (the
  crate has no library).
