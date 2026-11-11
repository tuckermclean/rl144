# B02 — input-byte-layout

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | T01, T02 | none | S |

**Owns:** `src/game.rs` (`apply_input` and its doc comment, a new pure byte-decoding function), `src/save.rs` (the vocabulary comment), `src/tests/` (new tests), `tests/fixtures/input-bytes.sav` (new), `CLAUDE.md` (the input-byte vocabulary line)
**Decisions:** OD1

## Purpose
`apply_input` decodes set-down-by-kind from `17 + GAME.items.len()` (`game.rs:4502-4506`). With
today's 10 items that is bytes 27–36, but adding any item moves every set-down byte: byte 27,
"set down the potion", would become "use the new item", which costs no turn, so every v11 save
that set something down would replay differently. No test pins a literal byte ≥ 17
(`ref.sav` holds only bytes 1, 3 and 4) (audit #41, §5.2, confirmed by a second reviewer).
Games will add items (the donkey treat is the first planned), so the engine needs a layout that
grows without breaking replays (owner, scope). This node freezes bytes 0–36 as they are today
and assigns new bytes to item kinds ≥ 10 by the OD1 rule. With 10 items no byte changes meaning,
so no golden, hash or save version changes.

## Tests
Keep every existing test (including `v10_save_replays_under_v11_parsing`). Add:
- `tests/fixtures/input-bytes.sav`: a v11 save whose input log picks up held items and uses
  every byte from 17 to 36 at least once, at least one use-by-kind and one set-down-by-kind
  changing `held`; a test replays it and pins its state hash (a lock: it passes at the base too).
- tests of the pure decoding function for an item count of 10 (every byte 0–255 decodes as
  `apply_input` does today) and of 11 (bytes 0–36 unchanged; kind 10 uses byte 37 and sets down
  with byte 38; bytes past the last assigned one decode to nothing).
Red target: `cargo test --quiet --no-default-features --features backend-term input_byte`
(the new tests share that prefix). At the base the decoding function does not exist, so the
tests fail to compile with "cannot find" errors naming it (AGENTS.md "Building a node", red
target form).

## Changes
1. Add a pure function in `game.rs` that maps an input byte and an item count to an action
   (move, wait, talk, give, use-top, put-down-objective, use-by-kind k, set-down-by-kind k, none)
   with set-down for kinds 0–9 at the named constant base 27 and kinds ≥ 10 per OD1.
   *B02-AC-2, B02-AC-3.*
2. Make `apply_input` dispatch through it, so the rule lives in one place. *B02-AC-1, B02-AC-4.*
3. Update the vocabulary text in `apply_input`'s doc comment, `save.rs`'s version comment and
   `CLAUDE.md`'s "Input-byte vocabulary" bullet: bytes 0–36 frozen, the OD1 rule for new kinds,
   and that adding an item past the tenth grows the vocabulary and bumps `SAVE_VERSION`.
   *B02-AC-5.*

## Gate
- **B02-AC-1** — `tests/fixtures/input-bytes.sav` uses every byte 17–36 and replays to the hash pinned in its test, on both backends. *Verify:* `cargo test --quiet input_byte` and the `backend-term` run, in the CI log.
- **B02-AC-2** — For an item count of 10, every byte 0–255 decodes to the same action `apply_input` took at the base. *Verify:* the `input_byte` table test in the CI log.
- **B02-AC-3** — For an item count of 11, bytes 0–36 decode as for 10, byte 37 is use-by-kind 10, byte 38 is set-down-by-kind 10, and byte 39 decodes to nothing. *Verify:* the `input_byte` growth test in the CI log.
- **B02-AC-4** — No set-down byte is computed from `GAME.items.len()`; the set-down base is a named constant 27. *Verify:* `grep -nE 'const [A-Z_]+: u8 = 27' src/game.rs` shows one line; `grep -c '17 + GAME.items.len()' src/game.rs` prints 0.
- **B02-AC-5** — `CLAUDE.md`'s vocabulary bullet, `apply_input`'s doc comment and `save.rs`'s version comment state the frozen 0–36 layout, the OD1 rule, and the `SAVE_VERSION` bump for an 11th item. *Verify:* `grep -n '37' CLAUDE.md src/game.rs src/save.rs` shows the rule in each file.
- **B02-AC-6** — `SAVE_VERSION` stays 11; `make check` is green in CI with goldens, frames, `xhash` (`1f22670bf826471a`) and bands unchanged. *Verify:* `grep -n 'SAVE_VERSION: u8 = 11' src/save.rs`; the check run URL; `git diff --stat <base>..HEAD -- tests/golden 'tests/*-band.json'` empty.

## Risks
- The fixture must be built from a real run (seed + inputs reaching held items), not
  hand-edited bytes that never touch `held`, or it pins nothing.
- If the decoding change alters any hashed state, the save doctrine requires a bump; that is a
  stop-and-ask, since this node is planned not to change behaviour (principle 5).
- The bots (`headless.rs`) emit set-down and use bytes; they must keep emitting the same bytes.
- Out of scope: adding any item (game work, later), `SAVE_VERSION` 12 (answer Q2).
