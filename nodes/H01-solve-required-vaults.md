# H01 — solve-required-vaults

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | T01, T02 | none | S |

**Owns:** `src/game.rs` (recording required-vault placement in level generation), `src/headless.rs` (`solve_seed`, `solve_main`), `src/save.rs` (`state_hash`'s doc comment: the record is derived), `src/tests/` (new tests), `CLAUDE.md` (the `--solve` bullet)
**Decisions:** OD7

## Purpose
`GameDef::required_vaults` guarantees a cartridge's set-piece rooms at fixed depths
(`place_required_vault_room`, an exhaustive scan). The 1.0 manifest says `--solve` asserts those
rooms exist across its full N, but `--solve` only fails a seed when the last depth has no
objective; the D3 room is checked only by a unit test over seeds 0..2000, not the gate's 10000
(audit #62). Any future cartridge that relies on required vaults needs the gate to prove them on
every seed it solves, generically, without naming any game's rooms. This node adds that to
`--solve`. It does not change worldgen: no golden, band or hash moves.

## Tests
Keep every existing test, including `required_cast_vaults_placed_on_every_seed` and
`required_vaults_well_formed`. Add tests that:
- for seeds 0..200, every depth listed in `GAME.required_vaults` reports its vault placed;
- the solver's per-seed check fails a seed whose generated level reports a required vault
  missing (built by clearing the record on a generated level, not by changing worldgen), and
  passes it otherwise.
Red target: `cargo test --quiet --no-default-features --features backend-term solve_required`
(the new tests share that prefix); at the base they fail to compile with "cannot find" or
"no field" errors naming the record (AGENTS.md "Building a node", red target form).

## Changes
1. During level generation, record for each `required_vaults` entry of the depth whether it was
   stamped (OD7); the record is derived, never hashed, dumped or saved, and `state_hash`'s doc
   comment lists it as derived. *H01-AC-1, H01-AC-4.*
2. `solve_seed` treats a missing required vault as a failed seed, reported separately from
   unreachable exits; `solve_main` exits non-zero and prints the seed and depth when any seed
   misses one, and prints how many seeds it checked for required vaults. *H01-AC-2, H01-AC-3.*
3. Update `CLAUDE.md`'s `--solve` bullet to say it also proves required vaults. *H01-AC-6.*

## Gate
- **H01-AC-1** — For seeds 0..200, generating each depth in `GAME.required_vaults` reports every required vault of that depth placed. *Verify:* the `solve_required` placement test in the CI log.
- **H01-AC-2** — The solver's per-seed check fails a seed whose level reports a required vault missing and passes it when the vault is present. *Verify:* the `solve_required` check test in the CI log.
- **H01-AC-3** — `make solve` (10000 seeds) is green, prints that it checked required vaults on all 10000 seeds, and still reports 0 unwinnable with worst case 1476 at seed 82. *Verify:* the check run log's `solve` section.
- **H01-AC-4** — The placement record is not hashed: `--replay tests/fixtures/ref.sav` still gives `1f22670bf826471a`, and `state_hash`'s doc comment lists the record as derived. *Verify:* `xhash: OK (1f22670bf826471a)` in the CI log; `grep -n -i 'required' src/save.rs`.
- **H01-AC-5** — No game noun is added to `game.rs` or `headless.rs`; the check reads only `GAME.required_vaults`. *Verify:* `git diff <base>..HEAD -- src/game.rs src/headless.rs | grep '^+' | grep -iwE 'rat|goblin|ogre|mimic|trainer|donkey|potion|sword|cheese|coat|towel'` prints nothing.
- **H01-AC-6** — `CLAUDE.md`'s `--solve` bullet says it proves required vaults on every seed. *Verify:* `grep -n -i 'required vault' CLAUDE.md`.
- **H01-AC-7** — `make check` is green in CI with goldens, frames and bands unchanged. *Verify:* the check run URL; `git diff --stat <base>..HEAD -- tests/golden 'tests/*-band.json'` empty.

## Risks
- Recording placement must not draw from any RNG channel or change draw order; if a golden
  diffs, that is a worldgen MAJOR and a stop (principle 6).
- `solve_seed` today returns `Option<i32>`; changing its shape touches the existing tests that
  call it, which are frozen: keep its signature or add a new function next to it.
- Out of scope: new required vaults for D2/D4 (game work, later).
