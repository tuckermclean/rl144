# E01 — engine-noun-guard

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | T01, T02 | none | S |

**Owns:** `src/tests/engine_nouns.rs` (new), `src/main.rs` (wiring the new test module only), `CLAUDE.md` (the cartridge doctrine's "checked by grep" sentence)
**Decisions:** OD10

## Purpose
The cartridge doctrine says engine and backend files stay clean of game nouns, "checked by grep
on every change", but no test or make target checks it; the audit found the files clean by a
manual grep (audit #6). A second game needs that boundary to hold, so this node turns the manual
grep into a unit test that runs in `make test` and `make test-term` (owner, scope: "a cleaner
engine/game boundary"). Today the only noun hits in those files are inside comments, several of
them `/* … */` block comments (`save.rs`, `game.rs`), which the doctrine allows.

## Tests
Keep every existing test. Add `src/tests/engine_nouns.rs` with:
- a scanner self-test: on sample source text, a noun in a string literal or an identifier is
  reported, and the same noun in a `//` comment, a `///` doc comment or a `/* */` block comment
  is not;
- the guard: it reads the engine and backend files named in OD10 from
  `env!("CARGO_MANIFEST_DIR")`, strips comments, and fails listing file, line and noun for every
  noun from OD10's list and the cartridge's `ThemeDef::mobs` names (whole words or phrases,
  case-insensitive).
Red target: the new tests must be listed and pass:
`sh -c 'n=$(cargo test -q --no-default-features --features backend-term engine_nouns -- --list | grep -c ": test$"); [ "$n" -ge 2 ] || { echo "FAIL engine_nouns: $n tests listed"; exit 1; }; cargo test -q --no-default-features --features backend-term engine_nouns'`.
The tester does not declare the module anywhere (not in `src/tests/main.rs` either): at RED the
file is not yet in the build, so the target lists 0 tests and prints its FAIL line; after GREEN it lists them and they pass.

## Changes
1. Wire `src/tests/engine_nouns.rs` into the test build from `src/main.rs`:
   `#[cfg(test)] #[path = "tests/engine_nouns.rs"] mod engine_nouns;`. *E01-AC-1, E01-AC-2, E01-AC-3.*
2. In `CLAUDE.md`'s cartridge doctrine, replace "checked by grep on every change" with the test's
   name and what it checks. *E01-AC-4.*

## Gate
- **E01-AC-1** — The scanner reports a noun in a string literal or an identifier and ignores it in line, doc and block comments. *Verify:* the `engine_nouns` self-test passes in the CI log.
- **E01-AC-2** — The guard scans `rng`, `content`, `gamedef`, `game`, `save`, `headless`, `render`, `backend_minifb` and `backend_term` for OD10's nouns and passes on the head. *Verify:* the `engine_nouns` guard test passes in the CI log under both feature sets.
- **E01-AC-3** — The guard fails, naming file, line and noun, when a game noun is put in a string literal of an engine file. *Verify:* a local run with a temporary `"goblin"` literal added to `src/render.rs` fails with that file and line (log recorded under `out/node/E01/`), and the change is not committed.
- **E01-AC-4** — `CLAUDE.md` names the guard test instead of a manual grep. *Verify:* `grep -n 'engine_nouns' CLAUDE.md`.
- **E01-AC-5** — `make check` is green in CI; no production source file changes except the test wiring. *Verify:* the check run URL; `git diff --stat <base>..HEAD -- src ':!src/tests' ':!src/main.rs'` empty.

## Risks
- A noun list that includes common words would flag legitimate engine identifiers; OD10's list
  is the cartridge's proper nouns only. Generic engine words (objective, cache, portal) are not
  game nouns.
- The scanner must handle raw strings, byte strings and char literals well enough not to misread
  a `"` inside a comment; a false failure is a stop to fix the scanner, never the engine.
- Out of scope: a second cartridge or a compile-time cartridge selector (later, with a second
  game).
