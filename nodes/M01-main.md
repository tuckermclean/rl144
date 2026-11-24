# M01 — main

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | E01, E02, E03, E04, C01, C02, S01, S02, P01, P02, P03, M02 | B02, M05 | L |

**Owns:** `src/main.rs`, the binary's entry point (argument parsing, headless-mode and backend dispatch) and the crate's main inline unit-test module.

## Purpose
`src/main.rs` is the crate root: it declares every module (`content`, `game`, `gamedef`, `games`, `headless`,
`render`, `rng`, `save`, and exactly one of `backend_minifb` or `backend_term`) and turns the command line into
one headless mode or one interactive session.
- **Backend wiring.** Two `compile_error!`s stop a build with neither backend feature or with both. Only the enabled backend's module is declared. Apart from
  `#[cfg(test)]`, the `cfg` blocks here and in the backends are the only ones `CLAUDE.md` allows outside the
  cartridge seam (`src/games/mod.rs`).
- **Argument parsing** is a flat scan of `std::env::args()` with no parser crate: `flag_val(name)` reads the
  next argument as a `u64` (absent or not numeric: `None`), `str_val(name)` reads it as a string. An unknown
  argument is ignored.
- **The seed.** `--seed S` wins; `seed_from_arg` parses a numeric `S` as it is and hashes any other string with
  `h64(0, &["seedstr", S])`, so `--seed swordfish` is stable. Without `--seed`, `--daily` gives
  `h64(day, &["daily"])`, `day` being the days since the Unix epoch; otherwise the seed is the launch time in
  nanoseconds (`0xDEAD_BEEF` if the clock reads before the epoch). These clock reads (the
  seed, and `day`, read on every run) are the program's only entropy.
- **Headless modes**, checked in this order, each returning when done: `--solve [N]` (`solve_main`, N
  default 10000, `--report` for stats only); `--sim [N]` (`sim_main`, N default 1000, `--report`, `--policy`
  `pacifist`, `tactical` or `tactical-pacifist`, any other value or none meaning greedy); `--sim-flip [N]`
  (`sim_flip_main`, N default 5000); `--dump` (prints `dump(seed)`); `--dump-overworld`; `--probe-floors`;
  in the terminal build only, `--render-frame` (`backend_term::render_frame_main(seed, ascii)`, with
  `--ascii`); `--replay <file>`, which parses the save, replays it and prints one JSON line
  `{"hash":"<16 hex digits>","seed":…,"turns":…,"depth":…,"hp":…,"light":…,"dead":…,"won":…}` from
  `state_hash`, or prints `bad save file: <path>` to stderr and exits 1.
- **The interactive session.** `--load <file>` replays a save into the starting game (a bad file: the same
  message and exit 1) and tells the backend it was loaded; otherwise the game is `Game::new_overworld(seed)`
  with an empty input log. A `--daily` run with an empty log logs `Daily dungeon #<day>. Everyone gets this one
  today.` Then the enabled backend's `run` takes the seed, the log, the game, `daily`, `day`, `loaded` (and
  `ascii` in the terminal build).
- **The unit tests.** The inline `#[cfg(test)] mod tests` (about 5,700 of the file's 5,961 lines, 245
  `#[test]`s) holds most of the crate's unit tests: worldgen, combat, mercy, the cartridge, saves, headless
  tooling and rendering. Its `#[cfg(test)]` imports reach into every core module. `golden_dumps` compiles the
  five map goldens in with `include_str!`. Only `string_seeds` tests this file's own code; the others, all but
  the four named in Risks, are tests the other imported nodes' criteria name.

## Tests
`string_seeds` (for `seed_from_arg`), run with `PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.75 cargo test
--quiet string_seeds`. The `Makefile` gates run the release binaries through this dispatch: `goldens`
(`--dump --seed N`), `frames` (the terminal build's `--render-frame --seed N [--ascii]`), `xhash` (`--replay`
in both builds), `solve` (`--solve 10000`), `sim` (`--sim 5000` bare, for greedy, and with `--policy pacifist`, `tactical`
and `tactical-pacifist`) and `flip`
(`--sim-flip 5000`). `make test` and `make test-term` run the whole test module under each feature set.

## Changes
none: imported

## Gate
- **M01-AC-1** — `seed_from_arg` returns a numeric string's value unchanged (`"123"` gives 123), and hashes a non-numeric string to the same value every time (`"swordfish"` twice), and `"swordfish"` and `"herring"` to different values. *Verify:* `string_seeds` in `src/main.rs`.
- **M01-AC-2** — `--dump --seed N` prints the dump of seed N. *Verify:* `make goldens` (target of `Makefile`, against `tests/golden/seed_*.txt`).
- **M01-AC-3** — In the terminal build, `--render-frame --seed N` renders seed N's first frame and `--ascii` selects its 7-bit variant. *Verify:* `make frames` (target of `Makefile`, against `tests/golden/frame_seed_*.bin`).
- **M01-AC-4** — `--replay <file>` prints a line with a `"hash":"<hex>"` field, and both builds print the same hash for `tests/fixtures/ref.sav`. *Verify:* `make xhash` (target of `Makefile`).
- **M01-AC-5** — The `--replay` line's hash is `state_hash` of the replayed game, and its other fields are the seed, the log length, the depth, HP, light, `dead` and `won` of that game; a save that does not parse prints `bad save file: <path>` and exits 1, for `--replay` and `--load` alike. *Verify:* unpinned (`make xhash` compares the two builds with each other, not with a recorded hash).
- **M01-AC-6** — The non-numeric seed hash is `h64(0, &["seedstr", s])`; without `--seed`, `--daily` gives `h64(day, &["daily"])` and otherwise the seed is the launch time in nanoseconds; `--seed` beats `--daily`. *Verify:* unpinned.
- **M01-AC-7** — `--policy pacifist`, `tactical` and `tactical-pacifist` select those bots and any other value selects greedy; the default counts are 10000 for `--solve`, 1000 for `--sim` and 5000 for `--sim-flip`; `--report` is passed through to `solve_main` and `sim_main`. *Verify:* unpinned (see Risks: `make sim` passes with every policy mapped to greedy).
- **M01-AC-8** — `--dump-overworld` prints `dump_overworld()` and `--probe-floors` runs `probe_floors_main()`. *Verify:* unpinned.
- **M01-AC-9** — A build with neither backend feature, or with both, fails with the `compile_error!` messages. *Verify:* unpinned (no target builds those feature sets).
- **M01-AC-10** — Without a headless flag, the session starts from `Game::new_overworld(seed)` with an empty log and `loaded` false, or from the `--load`ed save's replay with its log and `loaded` true; a `--daily` run with an empty log logs the daily line; the enabled backend's `run` receives them. *Verify:* unpinned (needs interactive playtest).

## Risks
- `make sim` cannot catch a broken `--policy` mapping: `sim_main` picks its band file from the policy it is
  given, so with every policy mapped to greedy each of the four runs is checked against the greedy band and
  passes. `make flip` calls `sim_flip_main`, which picks its two policies itself.
- The argument scan is order-sensitive in one way: the first headless flag found in the order above wins, and
  a flag's value is simply the next argument (`--seed --dump` seeds from the string `--dump`).
- The only entropy is the launch-time seed here (`CLAUDE.md`: no system entropy mid-game); the daily seed and
  the daily log line read the wall clock too.
- The test module is exempt from `CLAUDE.md`'s grep rule against game nouns, and it imports items from every
  core module and the cartridge (`games::contractor`), so a rename anywhere breaks its build. Four of its tests
  are named by no imported node's criteria: `bot_use_rule_heals_when_wounded_and_holding_potion`,
  `flip_margin_comparison_is_correct`, `last_life_bloody_none_for_a_fresh_life` and
  `trainer_resurrection_greeting_fires_once_bloody_and_merciful`.
- The tests in this file are named by the criteria of the other imported nodes, which freezes them
  (`SPEC.md`'s open decisions); only the implementer may write this file until the tests move to
  `src/tests/*.rs`, and that move keeps each named test's name and body.
- `golden_dumps` reads the map goldens at compile time (`include_str!`), so a missing golden breaks the test
  build of both feature sets, not just that test.
- The interactive path and the `--load` path can only be compile-checked headlessly.
