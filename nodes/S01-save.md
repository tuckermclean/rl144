# S01 — save

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | E01, E04, C02 | none | M |

**Owns:** `src/save.rs`, the save format, replay, the state hash and the ghost file format.

## Purpose
`src/save.rs` holds the "state is deltas" layer: a run is its launch seed plus its input log, so a save is that
pair and reconstruction is replaying it. All of it is pure functions; the backends and `src/main.rs` own every
file read and write.
- **Save format.** `save_bytes(seed0, inputs)` writes `"RL14"`, the version byte `SAVE_VERSION` (11), the seed as
  a little-endian `u64`, then one byte per input. `parse_save` returns `(seed, inputs)` for a blob of at least 13
  bytes with that magic and a version in `1..=11`, else `None`. The file's block comment documents the input
  vocabulary per version (bytes 0-4, 5 restart, 6 retry, 7-10 talk, 11-14 give, 15 use, 16 put-down,
  `17+kind` use-by-kind from v10, `17+len(items)+kind` set-down-by-kind from v11) and why every older version's
  log replays unchanged under v11. `save_filename(whash)` is `rl144-<16 hex digits>.sav`, named by the world hash.
- **Replay.** `replay(seed0, inputs)` starts from `Game::new_overworld(seed0)` (the real front door) and feeds
  each byte to `Game::apply_input`, except the two reconstruction bytes: `INPUT_RESTART` (5) rebuilds
  `Game::new_overworld(h64(seed, ["restart"]))`, a new seed; `INPUT_RETRY` (6) rebuilds the same seed and, only
  when the ended attempt was dead, sets `echo` to its death tile and `last_life_bloody` to `kills > spared`.
- **State hash.** `state_hash(g)` is FNV-1a (`fnv_bytes`) over the run-defining state: the player's scalars and
  flags, the run counters (`spared`, `speech_attempts`, `objective_dropped`, `mood_sum`, `mood_count`,
  `cache_light_collected`, `echo_done`, `answer_done`, `endure_done`), the four run RNG states (`combat`, `ai`,
  `flavor`, `parley`), the LIFO `held` list (length-prefixed, ordered), the current world and its provenance,
  the live level (tiles via `tile_hash_byte`, each monster's position, kind, hp and mercy fields, items, push
  blocks, the cached portal destination via `hash_portal`), the current world's stashed depths, and every other
  visited world in insertion order (skipping the current world's stale entry). Its doc comment is the one place
  that lists the presentation-only fields left out of the hash (`killer`, `echo`, `facing`, `fx_hit`,
  `mcguffin_last_line_turn`, `died_out_of_her_light`, `last_life_bloody`, `last_life_greeting_spoken`).
  `tile_hash_byte` pins the seven dungeon tiles to their old discriminants 0-6 and appends 7-10 for the overworld
  tiles; `hash_world_id` tags `Seed` 0, `Floor` 1, `Overworld` 2.
- **Ghost files.** `ghost_bytes` builds an `RLG1` blob: magic, version 1, seed, world hash, outcome
  (`GHOST_DIED_COMBAT`, `GHOST_DIED_DARK`, `GHOST_WON`, `GHOST_ABANDONED`), final depth, turns, a label index, then
  the input log capped at `GHOST_MAX_LOG` (6,000) bytes; the header is 28 bytes. `parse_ghost` reverses it,
  rejecting a short blob, a wrong magic or version, an unknown outcome, or a label index outside
  `GAME.ghost_labels`. `parse_ghost` and `Ghost` are `#[allow(dead_code)]`: ghost playback is post-1.0, only the
  write side is live (the backends' `write_ghost`). `ghost_filename(whash)` is `rl144-<16 hex digits>.ghost`.

## Tests
The unit tests are inline in `src/main.rs` (`mod tests`), run with `PATH="$HOME/.cargo/bin:$PATH"
RUSTUP_TOOLCHAIN=1.75 cargo test --quiet <name>`. The format and replay tests: `save_bytes_writes_current_version_and_unknown_versions_are_rejected`,
`save_filename_format`, `save_replay_roundtrip`, `put_down_byte_round_trips_through_save_parse_replay`,
`replay_with_restart_deterministic`, `retry_byte_replay_deterministic_and_seed_semantics`, the ten
`v<N>_save_replays_under_v<M>_parsing` tests, `echo_records_death_position_after_retry_byte`,
`last_life_bloody_true_after_a_bloody_retry`, `last_life_bloody_false_after_a_merciful_retry`. The hash-coverage
tests are listed under the Gate. The ghost tests: `ghost_bytes_round_trip`,
`parse_ghost_rejects_out_of_bounds_label_idx`. Many engine tests (E04's replay criteria) also compare
`state_hash` values. The `Makefile` target `xhash` replays `tests/fixtures/ref.sav` (a v5 blob: seed 4242, then six move and
wait bytes, `1 3 3 3 4 1`, and no restart byte) through both backends' `--replay` and requires the same hash.

## Changes
none: imported

## Gate
- **S01-AC-1** — `save_bytes` writes version 11 and `parse_save` accepts it, while version 0 and version 12 are rejected. *Verify:* `save_bytes_writes_current_version_and_unknown_versions_are_rejected` in `src/main.rs`.
- **S01-AC-2** — A blob shorter than 13 bytes or without the `RL14` magic is rejected by `parse_save`. *Verify:* unpinned.
- **S01-AC-3** — A live run saved, parsed and replayed reaches the same `state_hash` as the live game, the parsed seed and log equal the saved ones, and a put-down byte survives the round trip. *Verify:* `save_replay_roundtrip` and `put_down_byte_round_trips_through_save_parse_replay` in `src/main.rs`.
- **S01-AC-4** — Every older save version (1 to 10) still parses and replays to the same `state_hash` as the same log replayed directly. *Verify:* `v1_save_replays_under_v7_parsing`, `v2_save_replays_under_v7_parsing`, `v3_save_replays_under_v7_parsing`, `v4_save_replays_under_v7_parsing`, `v5_save_replays_under_v7_parsing`, `v6_save_replays_under_v7_parsing`, `v7_save_replays_under_v8_parsing`, `v8_save_replays_under_v9_parsing`, `v9_save_replays_under_v10_parsing` and `v10_save_replays_under_v11_parsing` in `src/main.rs`.
- **S01-AC-5** — `replay` is deterministic; byte 5 rerolls to a new seed and byte 6 keeps the seed; after a death, byte 6 sets `echo` to the death tile and `last_life_bloody` to whether kills exceeded spares. *Verify:* `replay_with_restart_deterministic`, `retry_byte_replay_deterministic_and_seed_semantics`, `echo_records_death_position_after_retry_byte`, `last_life_bloody_true_after_a_bloody_retry` and `last_life_bloody_false_after_a_merciful_retry` in `src/main.rs`.
- **S01-AC-6** — `replay` starts from `Game::new_overworld`, the restart byte's new seed is `h64(seed, ["restart"])`, and a retry byte after an attempt that did not die leaves `echo` and `last_life_bloody` at `None`. *Verify:* `save_replay_roundtrip` in `src/main.rs` for the start from `Game::new_overworld` (replay from `Game::new` fails it); the restart label and the retry-after-a-live-attempt clause are unpinned (a different label, or `echo` set on such a retry, passes every save, retry, echo and last-life test).
- **S01-AC-7** — `state_hash` changes when any of these changes: `spared`, `held`, push-block positions, a monster's `regard`, `calm`, `dividend_paid`, `struck_player`, `yielded` or `disguised`, `cache_light_collected`, `echo_done`, `answer_done`, `endure_done`, `mood_sum`, `mood_count`, `speech_attempts`, `objective_dropped`, or a dropped objective's position. *Verify:* `spared_is_hashed`, `held_is_hashed`, `blocks_are_hashed`, `monster_regard_and_calm_are_hashed`, `monster_dividend_paid_is_hashed`, `monster_struck_player_and_yielded_are_hashed`, `monster_disguised_is_hashed`, `cache_light_collected_is_hashed`, `lesson_state_fields_are_hashed`, `state_hash_covers_mood_sum_and_mood_count`, `state_hash_covers_speech_attempts_and_objective_dropped` and `state_hash_covers_dropped_objective_position` in `src/main.rs`.
- **S01-AC-8** — `state_hash` also covers a monster's `awe` and `disarm_regard_paid`, the world id and provenance, the cached portal destinations, the stashed depths and every other visited world. *Verify:* unpinned (no test changes only these fields; the replay tests cover them only as part of whole runs).
- **S01-AC-9** — The presentation-only `echo`, `mcguffin_last_line_turn`, `last_life_bloody` and `last_life_greeting_spoken` do not change `state_hash`. *Verify:* `echo_records_death_position_after_retry_byte`, `mcguffin_last_line_turn_is_not_hashed`, `last_life_bloody_is_not_hashed` and `last_life_bloody_true_after_a_bloody_retry` in `src/main.rs`. Per field: `echo` is pinned by `last_life_bloody_true_after_a_bloody_retry` (not by `echo_records_death_position_after_retry_byte`, whose two compared games have the same `echo`, so it pins `last_life_bloody`); `mcguffin_last_line_turn` by `mcguffin_last_line_turn_is_not_hashed`; `last_life_bloody` by `last_life_bloody_is_not_hashed` and `echo_records_death_position_after_retry_byte`; `last_life_greeting_spoken` by `last_life_bloody_is_not_hashed`.
- **S01-AC-10** — The other presentation-only fields, `killer`, `facing`, `fx_hit` and `died_out_of_her_light`, and the render buffer `lit_r` do not change `state_hash`. *Verify:* unpinned.
- **S01-AC-11** — Both backends replay `tests/fixtures/ref.sav` to the same `state_hash`. *Verify:* `make xhash` (target of `Makefile`, against `tests/fixtures/ref.sav`).
- **S01-AC-12** — The hash's byte encoding is append-only: the dungeon tiles keep bytes 0-6, the overworld tiles take 7-10, and the world ids keep tags 0 (`Seed`), 1 (`Floor`), 2 (`Overworld`). *Verify:* unpinned (`make xhash` compares the two backends with each other, not with a recorded hash).
- **S01-AC-13** — `ghost_bytes` and `parse_ghost` round-trip the seed, world hash, outcome, final depth, turns, label index and input log, and `parse_ghost` accepts the last label index of `GAME.ghost_labels` and rejects the one past it. *Verify:* `ghost_bytes_round_trip` and `parse_ghost_rejects_out_of_bounds_label_idx` in `src/main.rs`.
- **S01-AC-14** — `parse_ghost` rejects a blob shorter than 28 bytes, a wrong magic, a version other than 1 and an unknown outcome byte; `ghost_bytes` keeps at most 6,000 input bytes. *Verify:* unpinned.
- **S01-AC-15** — `save_filename` gives `rl144-` plus the world hash as 16 lower-case hex digits plus `.sav`; `ghost_filename` gives the same with `.ghost`. *Verify:* `save_filename_format` in `src/main.rs` for the `.sav` name; the `.ghost` name is unpinned.
- **S01-AC-16** — The file has no `cfg` block, no platform call or file I/O, and no game noun as an identifier or a string literal. *Verify:* unpinned (the `CLAUDE.md` grep rule, not a tracked check).

## Risks
- `CLAUDE.md`'s save-version doctrine: a new input byte or a change to the shape of hashed state bumps
  `SAVE_VERSION`, and `parse_save` keeps accepting every older version. The accepted range is a literal
  (`1..=11`) next to the `SAVE_VERSION` constant, so a bump must change both; the block comment records that the
  cast arc's later lesson-state fields extend the hash under version 11 without a further bump.
- A field added to `Game` or `Monster` is not hashed until `state_hash` names it, and no gate fails when it is
  forgotten: only a per-field `*_is_hashed` test catches it. A field left out on purpose belongs in the
  exclusion list of `state_hash`'s doc comment, the one place `CLAUDE.md` names for it. `CLAUDE.md`'s list also
  names `lit_r`, which that doc comment does not mention.
- Monster and item coordinates and monster hp are folded in as `u8` casts, and the RNG states as their `u64`
  words; any reordering or re-encoding changes every hash (and the `xhash` value) without failing a test that
  compares against a stored hash.
- `tests/fixtures/ref.sav` and every save in the wild were recorded through `replay`'s use of the front door,
  `Game::new_overworld` (which `headless::dump_overworld` and both backends also call); changing the constructor
  `replay` uses breaks them all.
- The ghost reader is dead code until ghost playback, which `CLAUDE.md` defers to after 1.0.
- The tests these criteria name are all in `src/main.rs`, which only the implementer may write until the tests
  move to `src/tests/*.rs` (`SPEC.md`'s open decisions).
