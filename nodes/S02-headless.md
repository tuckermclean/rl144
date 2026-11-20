# S02 — headless

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | E01, E02, E03, E04, C02 | M01, M02, M03, P02, P03 | L |

**Owns:** `src/headless.rs`, the display-free verification tooling: dumps, the solver, the bot simulator and their gates.

## Purpose
`src/headless.rs` is the test harness that works without a display. `src/main.rs` dispatches its command-line
modes to the functions here.
- **World identity.** `world_hash(seed)` is FNV-1a (`fnv_bytes`) over `dump(seed)`: it names the generated world,
  so it changes exactly when a worldgen MAJOR would. The engine (`src/game.rs`) uses it for `Dest::World`, and the
  backends for the save and ghost file names.
- **Dumps.** `level_dump(g)` prints the live level as 25 rows of 80 characters (the legend: `#` wall, `.` floor,
  `>` and `<` stairs, `*` portal, `^` pit, `x` goal, `=` screen link, `V` hole, `+` shut door; then items by their
  cartridge glyph, `B` push blocks, monsters by their glyph, `@` the player, each drawn over the one before), then
  a `monsters=N items=M` line. `dump(seed)` (`--dump`) prints `seed=N` and, for each depth of `Game::new(seed)`,
  a `-- depth d : <theme label> --` header and that depth's `level_dump`. `dump_overworld()`
  (`--dump-overworld`) prints the three authored overworld screens the same way, independent of any seed.
- **Solver.** `solve_seed(seed)` generates each depth of `Game::new(seed)` and returns three times the sum of the
  `bfs_dist` distances from each depth's start to its down-stairs (on the last depth, to the objective), or `None`
  when one is unreachable. `solve_main(n, report)` (`--solve N`) prints JSON stats (`min`, `p50`, `p90`, `p99`,
  `max`, `worstSeed`, `unwinnable`) over seeds `0..n` and, unless `--report`, exits 1 with `UNWINNABLE:` on any
  unwinnable seed or `difficulty drift:` on a stat outside `tests/solver-band.json`.
- **Band files.** `band_range` and `band_scalar` read `"key": [lo, hi]` and `"key": N` out of a band file without
  a JSON crate, returning `None` for a missing key.
- **Bot simulator.** `sim_seed(seed, policy)` plays `Game::new(seed)` to a win, a death or `SIM_TURN_CAP` (6,000
  turns) through `apply_input` alone, with no randomness of its own, and returns a `SimResult` (won, dark death,
  combat death or stuck; turns, light left, kills, spares) and the final `WorldId`. The four `Policy` values:
  `Greedy` bump-attacks whatever it routes through; `Pacifist` shares every route and loot choice and talks
  (bytes 7-10) instead of bumping a non-calm monster; `Tactical` and `TacticalPacifist` prefer
  `tactical_routing_map` (push blocks and live monsters walled off), drop it for the rest of a leg once
  more than `STALL_LIMIT` (20) turns pass without progress (the check is `stall_count > STALL_LIMIT`, so after 21), rest (byte 4) while hurt and safe within `MAX_REST_TURNS_PER_RUN`
  (300) and above `REST_LIGHT_RESERVE` (150) light, and engage a cardinally adjacent seeing monster when no step
  escapes it (the violent bot attacks; the diplomat talks, or before a give-ground kind retreats one step and then
  talks). Every policy drinks
  a potion on top of `held` (byte 15) when hurt (half HP, or two thirds for the tactical ones), and both diplomat
  policies decline an adjacent polite-decline monster first (`diplomat_decline_byte`). Each turn's decision order
  and its history are in the doc comments at each step.
- **Sim gates.** `sim_main(n, report, policy)` (`--sim N --policy P`) prints one JSON line of aggregate stats and,
  unless `--report`, exits 1 when any run is stuck, when `win_pct` (and `deaths_dark` for `Greedy` and
  `Tactical`) falls outside the policy's band file (`tests/sim-band.json`, `tests/pacifist-band.json`,
  `tests/tactical-band.json`, `tests/tactical-pacifist-band.json`), or when greedy's dark deaths are not fewer
  than its combat deaths; a missing band file only warns. `sim_flip_main(n)` (`--sim-flip N`) runs both tactical
  policies over `0..n`, compares their win percentages as floats, and exits 1 with `FLIP INVERTED/THIN` when the
  diplomat's margin over the violent bot is below `flip_margin` from `tests/tactical-pacifist-band.json` (3 when
  the key is absent); a missing band file fails it, and a run whose `n` differs from `flip_n` only prints a note.
- **Floor probe.** `floor_dive_cost(i)` instantiates authored floor `i` and returns twice the BFS distance from
  its entry to its light cache (it panics on a floor without one); `probe_floors_main()` (`--probe-floors`)
  prints each floor's cost next to the cartridge's cache value and always exits 0.

## Tests
The unit tests are inline in `src/main.rs` (`mod tests`), run with `PATH="$HOME/.cargo/bin:$PATH"
RUSTUP_TOOLCHAIN=1.75 cargo test --quiet <name>`: `dump_deterministic`, `golden_dumps`,
`dump_overworld_deterministic_and_sane`, `solver_smoke`, `band_scalar_parses_flip_fields`, `sim_deterministic`,
`sim_bot_wins_some`, `bot_never_transits`, `pacifist_never_attacks`, `pacifist_policy_deterministic`,
`tactical_routing_map_walls_live_monsters_only`, `tactical_bot_deterministic_and_never_talks`,
`tactical_bot_wins_at_least_as_often_as_greedy`, `tactical_pacifist_never_attacks_and_deterministic`,
`tactical_pacifist_wins_at_least_as_often_as_pacifist`, `tactical_pacifist_previously_stuck_seeds_now_terminate`,
`tactical_pacifist_declines_the_mimic_deterministically`, the three `diplomat_decline_byte_*` tests and
`authored_floor_dive_costs_match_derivation`. The gates of the `Makefile` run these modes on the release binary:
`goldens` (`--dump` against `tests/golden/seed_*.txt`), `solve` (`--solve 10000`), `sim` (`--sim 5000` for each
policy) and `flip` (`--sim-flip 5000`).

## Changes
none: imported

## Gate
- **S02-AC-1** — `dump` is deterministic and prints the five golden seeds' committed dumps. *Verify:* `dump_deterministic` and `golden_dumps` in `src/main.rs`; `make goldens` (against `tests/golden/seed_*.txt`).
- **S02-AC-2** — `dump_overworld` is deterministic, prints exactly three screens, and shows a screen link (`=`) and the hole (`V`). *Verify:* `dump_overworld_deterministic_and_sane` in `src/main.rs`.
- **S02-AC-3** — `world_hash(seed)` is FNV-1a over `dump(seed)`. *Verify:* unpinned.
- **S02-AC-4** — Seeds 0 to 49 are winnable by `solve_seed`, and `--solve 10000` finds every seed winnable with its budget stats inside `tests/solver-band.json`. *Verify:* `solver_smoke` in `src/main.rs`; `make solve` (against `tests/solver-band.json`).
- **S02-AC-5** — `solve_main` exits 1 with an `UNWINNABLE:` line on an unwinnable seed and a `difficulty drift:` line on a stat outside the band; `--report` prints the stats and exits 0; a missing band file only warns. *Verify:* unpinned (the gate runs only the passing path).
- **S02-AC-6** — `band_scalar` reads an integer key and returns `None` for a missing key; `band_range` reads the `[lo, hi]` pairs the solve and sim gates check. *Verify:* `band_scalar_parses_flip_fields` in `src/main.rs` for `band_scalar`; `band_range` is unpinned: no unit test calls it, and `make solve` and `make sim` cannot catch it breaking, because both gates check a band only `if let Some((lo, hi)) = band_range(&band, k)`, so a `band_range` that returned `None` for every key would skip every band check and both gates would pass.
- **S02-AC-7** — `sim_seed` is deterministic under every policy. *Verify:* `sim_deterministic`, `pacifist_policy_deterministic`, `tactical_bot_deterministic_and_never_talks`, `tactical_pacifist_never_attacks_and_deterministic` and `tactical_pacifist_declines_the_mimic_deterministically` in `src/main.rs`.
- **S02-AC-8** — No policy's bot leaves the root world on seeds 0 to 99; the tactical bot gets stuck on none of seeds 0 to 49, wins at least one, and every run ends in exactly one outcome; the tactical diplomat terminates on the five seeds that once stuck it. *Verify:* `bot_never_transits`, `sim_bot_wins_some` and `tactical_pacifist_previously_stuck_seeds_now_terminate` in `src/main.rs`.
- **S02-AC-9** — The pacifist bot kills nothing on seeds 0 to 199 and spares at least one monster, and the tactical diplomat kills nothing on its tested seeds. *Verify:* `pacifist_never_attacks` and `tactical_pacifist_never_attacks_and_deterministic` in `src/main.rs`.
- **S02-AC-10** — The violent tactical bot never talks. *Verify:* unpinned (`tactical_bot_deterministic_and_never_talks` checks determinism and termination only).
- **S02-AC-11** — Over seeds 0 to 299 the tactical bot wins at least as often as the greedy one, and the tactical diplomat at least as often as the pacifist. *Verify:* `tactical_bot_wins_at_least_as_often_as_greedy` and `tactical_pacifist_wins_at_least_as_often_as_pacifist` in `src/main.rs`.
- **S02-AC-12** — `tactical_routing_map` walls a live monster's tile and leaves a becalmed one's walkable. *Verify:* `tactical_routing_map_walls_live_monsters_only` in `src/main.rs`.
- **S02-AC-13** — `routing_map` walls every push block's tile; a bot never waits on a portal tile; the stall guard and the rest cap apply only to the tactical policies; greedy and pacifist heal at half HP and the tactical policies at two thirds. *Verify:* unpinned.
- **S02-AC-14** — `diplomat_decline_byte` returns the talk byte toward a cardinally adjacent, non-calm polite-decline monster for both diplomat policies only, and `None` for a calm one or a diagonal one. *Verify:* `diplomat_decline_byte_fires_correctly_directed_and_only_for_diplomats`, `diplomat_decline_byte_ignores_a_calm_mimic` and `diplomat_decline_byte_ignores_diagonal_adjacency` in `src/main.rs`. Note: the diagonal test does not pin the adjacency check (a diagonal neighbour's direction is not a talk direction, so it passes with the check deleted); "only when adjacent" (a monster two tiles away in a row or column) is caught only by the tactical-diplomat sim tests `tactical_pacifist_never_attacks_and_deterministic` and `tactical_pacifist_previously_stuck_seeds_now_terminate`.
- **S02-AC-15** — `--sim 5000` for each of the four policies has no stuck run and stays inside its band file, and greedy's dark deaths stay fewer than its combat deaths. *Verify:* `make sim` (against `tests/sim-band.json`, `tests/pacifist-band.json`, `tests/tactical-band.json`, `tests/tactical-pacifist-band.json`).
- **S02-AC-16** — `sim_main` exits 1 with a `sim (<policy>): N runs stuck` line, a `sim drift` line, or the greedy dark-deaths line when its check fails; `--report` prints the stats and exits 0; a missing band file only warns. *Verify:* unpinned (the gate runs only the passing path).
- **S02-AC-17** — `--sim-flip 5000` finds the diplomat's win percentage at least `flip_margin` above the violent bot's. *Verify:* `make flip` (against `tests/tactical-pacifist-band.json`).
- **S02-AC-18** — `sim_flip_main` exits 1 with a `FLIP INVERTED/THIN` line when the margin is short and with an `error:` line when its band file is missing; a run at an `n` other than `flip_n` only prints a note. *Verify:* unpinned (the gate runs only the passing path).
- **S02-AC-19** — `floor_dive_cost` is 12 for authored floor 0 and 14 for floor 1. *Verify:* `authored_floor_dive_costs_match_derivation` in `src/main.rs`.
- **S02-AC-20** — `probe_floors_main` prints one line per authored floor with its dive cost, the cache value and their margin, and exits 0. *Verify:* unpinned.
- **S02-AC-21** — The file has no `cfg` block and no game noun as an identifier or a string literal; its only platform calls are the band-file reads (`std::fs::read_to_string`), stdout and stderr, and `std::process::exit` for a failed gate. *Verify:* unpinned (the `CLAUDE.md` grep rule, not a tracked check).

## Risks
- The dump format is what `tests/golden/seed_*.txt` freezes: a legend or layout change here diffs every golden
  without changing a map. `CLAUDE.md` calls that a dump-format-only diff, not seed-breaking, but the goldens must
  be regenerated and the commit must say so. `world_hash` hashes the dump, so the same change also renames every
  save and ghost file and changes every portal world's memoized hash.
- `src/game.rs` calls `world_hash` and this file calls into `src/game.rs`; the two depend on each other. E04 does
  not list this node as a dependency, so a change to `world_hash` or `dump` reaches the engine through that cycle.
- The bots are instruments, not players: `CLAUDE.md`'s instrument-parity rule says a new move on a route teaches
  the bot that models the route in the same change. The greedy and pacifist bands are floor references that must
  not move when other bands change; re-baselining any band needs the owner's sign-off.
- `SIM_SEEDS` in the `Makefile` should stay equal to `flip_n` in `tests/tactical-pacifist-band.json`; a mismatch
  only prints a note.
- `sim_main`'s greedy `deaths_dark < deaths_combat` check is code, not a band; the band files' own comments hold
  the measured history.
- `CLAUDE.md` says the core makes no platform calls, yet this file reads the band files and ends the process. The
  reads use paths relative to the working directory, so the gates must run from the repository root.
- The file is about 1,400 lines, most of them `sim_seed`'s documented heuristics.
- The tests these criteria name are all in `src/main.rs`, which only the implementer may write until the tests
  move to `src/tests/*.rs` (`SPEC.md`'s open decisions).
