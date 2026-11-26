# M05 — test-move

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | B03, B04, M01, P01, P02, P03 | P04 | M |

**Owns:** `src/tests/`, the crate's unit tests, moved out of the source files they test.

## Purpose
Every unit test is an inline `#[cfg(test)] mod tests { ... }` block: `src/main.rs` (from line 218, 245
tests), `src/render.rs` (from line 609, 8 tests), `src/backend_minifb.rs` (from line 602, 1 test) and
`src/backend_term.rs` (from line 1022, 16 tests); `cargo test` runs 254 tests on the window build and 269 on
the terminal build. The role guard (`.noodlebot/roles.yaml`) lets only the implementer write `src/*.rs`, so
a tester cannot add a unit test today (SPEC.md, open decision **Unit tests before the move**). This node
moves each block, unchanged apart from its indentation and the five `include_str!` paths of `golden_dumps`
(re-pointed to the same goldens from the new file's directory), to its own file under `src/tests/`, declared from its old place as
`#[cfg(test)] #[path = "tests/<file>.rs"] mod tests;`, so the module path, and with it every test's name,
stays the same and a tester can write unit tests the guard allows. The owner asked for it (answer Q10 on the
adoption's issue #3).

## Tests
No new test. The moved tests are the tests: their names and bodies are frozen (SPEC.md, settled decision
**Tests the imported criteria name**: E01 to E04 and others name them), so the move changes neither. The
red target is the layout check of M05-AC-1, not a failing unit test.

## Changes
1. `src/tests/main.rs`, `src/tests/render.rs`, `src/tests/backend_minifb.rs` and `src/tests/backend_term.rs`
   (or names of the implementer's choice, one file per moved block), each holding one moved block's body. In
   the moved `src/main.rs` block, the five `include_str!` paths of `golden_dumps` change from
   `"../tests/golden/seed_N.txt"` to `"../../tests/golden/seed_N.txt"`: the same golden files, reached from
   `src/tests/`.
2. In `src/main.rs`, `src/render.rs`, `src/backend_minifb.rs` and `src/backend_term.rs`: the inline block
   replaced by the `#[cfg(test)] #[path = ...] mod tests;` declaration; nothing else in those files changes.
3. `CLAUDE.md`: the text that says the unit tests are inline (the "Tests and fixtures" paragraph of
   "Building a node", and "Red target"'s "Until the inline unit tests move") says where they live now.

## Gate
- **M05-AC-1** — No `src/*.rs` file has an inline test module, and each of the four modules declares its tests from a file under `src/tests/`: `grep -n "mod tests *{" src/*.rs` prints nothing and `grep -c '#\[path = "tests/' src/main.rs src/render.rs src/backend_minifb.rs src/backend_term.rs` prints 1 for each file. *Verify:* those two commands' output before and after, recorded in the PR (the red target).
- **M05-AC-2** — The set of test names is unchanged on both builds: the sorted output of `cargo test -- --list` is identical before and after the move, for the default features (254 tests) and for `--no-default-features --features backend-term` (269 tests). *Verify:* the four lists recorded in the PR and an empty `diff` of each pair.
- **M05-AC-3** — Each moved test keeps its body: each new file's content equals the removed block's body, apart from one level of indentation (four spaces) taken off every line and, in the moved `src/main.rs` block, the five `include_str!` paths of `golden_dumps` re-pointed from `../tests/golden/` to `../../tests/golden/` (the same files, from the new file's directory); a recorded comparison prints no other difference. *Verify:* the comparison's command and its output (the five path lines only), recorded in the PR.
- **M05-AC-4** — No shipped code changes: the stripped sizes of both release binaries are the same before and after, and the packed sizes too. *Verify:* the four sizes recorded in the PR, with the toolchain.
- **M05-AC-5** — The gate passes on the PR's head commit. *Verify:* the CI run (B04) on the PR's head commit.
- **M05-AC-6** — `CLAUDE.md` names `src/tests/` as where the unit tests live and no longer says they are inline. *Verify:* `grep -nE "inline unit tests|unit tests are inline" CLAUDE.md` (two lines before, none after), recorded in the PR, and the PR's diff of `CLAUDE.md`.

## Risks
- The owner approved the move as a move of frozen tests (on the plan's spec PR, 2026-11-24; its decision
  **Approval of the test move**, SPEC.md, is settled): every inline unit test moves to `src/tests/`, with the
  indentation change that comes with it and the five `include_str!` paths of `golden_dumps` re-pointed (the
  `include_str!` bullet below), as long as every test keeps its name and its body (M05-AC-2 and M05-AC-3). The approval does not cover a change to any test's assertions or behaviour; such a change still
  needs the owner's approval. The session that builds this node does not stop to ask for the move.
- The imported criteria's *Verify:* lines name tests "in `src/main.rs`" (and the other three files). After the
  move they point at the old file. A spec PR after this node re-points them at the new files; this node
  changes no spec.
- `#[path]` on a `mod` declared in a non-`mod.rs` file resolves relative to that file's directory (`src/`),
  so the path is `tests/<file>.rs`; a wrong path is a build error, not a silent miss.
- `include_str!` resolves its path relative to the file that holds the macro. `golden_dumps` in the
  `src/main.rs` block reads `../tests/golden/seed_N.txt`, which from `src/tests/main.rs` would point at
  `src/tests/golden/` (no such folder: a build error). Its five paths are re-pointed to `../../tests/golden/`
  (owner's answer Q1 (a) on issue #21, 2026-11-25); no other moved line changes but for its indentation.
- The moved blocks use `super::*` and private items of their parent module; staying a child module keeps that
  access, while turning them into integration tests under `tests/` would lose it (and is not this node).
- `src/backend_minifb.rs` and `src/backend_term.rs` compile only under their own feature, so each moved file
  is checked by only one of the two `cargo test` runs.
- Merge conflicts: the four files are large and other nodes change them; this node goes first after B04, and
  P04 waits for it.
