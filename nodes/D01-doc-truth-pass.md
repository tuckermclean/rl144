# D01 — doc-truth-pass

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | T02 | none | S |

**Owns:** `README.md` (new), `CLAUDE.md` (all but the "Building a node" section), `docs/superpowers/specs/2026-08-04-flexible-camera-console-design.md` (status lines), `src/save.rs` and `src/games/contractor.rs` (comments only), `tests/docs-truth.test.sh` (new)
**Decisions:** OD6

## Purpose
The engine and its doctrine match the code almost everywhere, but the status lines and some
comments are stale, and some contradict the owner's answers (audit §1, §2, §4, §5):
- `CLAUDE.md` says batches merged to `master` (the branch is `main`), that the next action is the
  mimic playtest before cast batch two (the plan is now engine-first, SPEC.md; the playtest is a
  later owner step, answer Q3), that §9-G is not briefed (it is done) and §9-J is done (it is
  partial), and that a second cheese give-target slot waits in `contractor.rs` (it was assigned
  to goblins in batch 13) (audit #19, #66, #68, #69);
- `--sim` is documented as defaulting to 5000; the code defaults to 1000 (audit #25, OD6);
- build prerequisites omit `g++` (minifb's C++ shim) and point at `upx-ucl` from apt, which
  Debian 12 lacks (audit #14); there is no README (audit #18); `--daily`, `--load` and string
  seeds are undocumented (audit #32);
- `save.rs:66-70` says later cast state "extends `state_hash` under this same 11", which the save
  doctrine forbids (audit §5.3; answer Q2);
- the instrument-parity exception for batch 17's set-down is unrecorded (answer Q4, audit #64);
- the camera console spec still says "awaiting approval, no code written" (audit #73);
- `contractor.rs:43-55` (MIMIC "plain, minimal" row) and `:588-592` (potion "same amount (8)")
  are stale comments (audit §5);
- the exclusion-set doc omits derived unhashed fields, `room_visited` among them (audit #37, §5.4);
- "Process discipline" prescribes its own subagent dispatch and machine-local files
  (`.superpowers/sdd/progress.md`, a sibling repository path) a node builder cannot read.

## Tests
No Rust tests change. Add `tests/docs-truth.test.sh`, which greps for each stale statement below
and for each required new one, printing a FAIL line per miss. Red target:
`sh tests/docs-truth.test.sh` (non-zero at the base, 0 after).

## Changes
1. Rewrite `CLAUDE.md` "Where this stands": engine-first per SPEC.md and `docs/graph.yaml` (owner,
   2026-11-11); batches 1–17 on `main`; the mimic playtest is an owner step for later, not signed
   off; §9 status as the audit found it (G done, J partial, I/K not started); no open cheese slot.
   *D01-AC-1, D01-AC-2.*
2. Document `--sim`'s default as 1000 (OD6) and list `--daily`, `--load <file>` and string seeds.
   *D01-AC-3.*
3. Fix "Platform notes" prerequisites: rustc 1.75 via rustup, `make`, `g++`, upx from the
   upstream release where the distribution has none. *D01-AC-4.*
4. Add `README.md`: what rl144 is, prerequisites, `make check` and the other targets, the CLI
   flags, and pointers to `SPEC.md` and `CLAUDE.md`. *D01-AC-4.*
5. Rewrite the `save.rs` version comment: the next hashed-state change bumps to 12 (the cast's
   fields land with the cast), never under 11. *D01-AC-5.*
6. Record the instrument-parity exception (batch 17's set-down not taught to the diplomat bots,
   flip held at +9.9, accepted by the owner) next to the instrument-parity rule. *D01-AC-6.*
7. Mark the camera console spec implemented in `f33a80e`, with the 32-bit defect pointing at
   node B01. *D01-AC-7.*
8. Correct the two stale `contractor.rs` comments; extend `state_hash`'s doc comment with the
   derived, unhashed fields (`seen`, `vis`, `rooms`, `room_meta`, `room_visited`, `msgs`), noting
   that `room_visited` gates a flavor draw whose effect the hashed flavor RNG captures.
   *D01-AC-8.*
9. In "Process discipline", say node builds use `/noodlebot:node` and its tester, implementer and
   reviewer roles, and mark the machine-local references as not available to builders.
   *D01-AC-9.*

## Gate
- **D01-AC-1** — `CLAUDE.md` names SPEC.md and `docs/graph.yaml` as the plan, says the scope is engine-first, says the mimic is not playtested or signed off, and no longer says "merged to `master`". *Verify:* `sh tests/docs-truth.test.sh`; `grep -c 'merged to .master' CLAUDE.md` prints 0.
- **D01-AC-2** — `CLAUDE.md`'s §9 line says G done, J partial, I and K not started, and the "second cheese give-target" placeholder is gone. *Verify:* `grep -c 'second cheese' CLAUDE.md` prints 0; `sh tests/docs-truth.test.sh`.
- **D01-AC-3** — `CLAUDE.md` gives `--sim`'s default as 1000 and documents `--daily`, `--load` and string seeds; `README.md` lists them too. *Verify:* `grep -n -- '--sim N' CLAUDE.md` shows 1000; `grep -c -- '--daily' CLAUDE.md README.md`.
- **D01-AC-4** — `README.md` exists and lists rustc 1.75, `make`, `g++` and upx as prerequisites and `make check` as the gate; `CLAUDE.md` names `g++` and no longer recommends `upx-ucl` from apt alone. *Verify:* `grep -n 'g++' README.md CLAUDE.md`.
- **D01-AC-5** — `save.rs` no longer says to extend `state_hash` under version 11, and says the next hashed-state change bumps the version. *Verify:* `grep -c 'under this same 11' src/save.rs` prints 0.
- **D01-AC-6** — `CLAUDE.md` records the batch-17 set-down instrument-parity exception as accepted by the owner. *Verify:* `grep -n -i 'exception' CLAUDE.md` shows it next to the instrument-parity rule.
- **D01-AC-7** — The camera console spec's status says implemented in `f33a80e` and points at B01 for the 32-bit defect. *Verify:* `grep -n 'f33a80e\|B01' docs/superpowers/specs/2026-08-04-flexible-camera-console-design.md`.
- **D01-AC-8** — The two stale `contractor.rs` comments are corrected and `state_hash`'s doc comment lists the derived unhashed fields; only comment lines change in `.rs` files. *Verify:* `grep -n 'room_visited' src/save.rs`; the reviewer confirms every changed line in `git diff <base>..HEAD -- '*.rs'` lies inside a comment; goldens, frames and `xhash` unchanged in the CI log.
- **D01-AC-9** — "Process discipline" names `/noodlebot:node` and its roles for node builds and marks the machine-local references unavailable. *Verify:* `grep -n 'noodlebot:node' CLAUDE.md` shows a line outside "Building a node".
- **D01-AC-10** — `make check` is green in CI with goldens, frames, `xhash` and bands unchanged, and the "Building a node" section is unchanged. *Verify:* the check run URL; `git diff --stat <base>..HEAD -- tests/golden 'tests/*-band.json'` empty; `git diff <base>..HEAD -- CLAUDE.md` shows no hunk in that section.

## Risks
- Comments in `contractor.rs` sit next to golden-frozen data; a stray edit outside a comment
  could change a string or a table. Only comment lines may change in `.rs` files.
- Canon precedence (`STORY-COMPILE-v1.md` above design docs above plans) still governs game
  design text; this node changes no story or design doc except the camera spec's status.
- Out of scope: the band file's stale comment (principle 7), rewriting plan checkboxes (audit #78:
  trust commits over checkboxes; the README can say so).
