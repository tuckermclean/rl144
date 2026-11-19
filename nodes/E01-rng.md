# E01 — rng

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | E02, E04 | S |

**Owns:** `src/rng.rs`, the frozen random and hash primitives.

## Purpose
`src/rng.rs` holds the engine's only sources of randomness and its byte hash, all pure functions of
their inputs:
- `Rng`, a xorshift64 generator (`Rng::new` forces the seed odd; `next`, `range(lo, hi)` in `[lo, hi)` that
  returns `lo` when `hi <= lo`, and `chance(num, den)`);
- `h64(seed, tags)`, FNV-1a(64) over the seed's little-endian bytes and each tag after a `0x1f` unit
  separator, then a finalizer the file calls splitmix-style (its constants are MurmurHash3's fmix64), and
  `channel(seed, tags)`, an `Rng` seeded from it. Named channels keep the random streams apart, so combat, AI,
  flavor and parley draws never perturb worldgen;
- `fnv_bytes(h, bytes)`, the FNV-1a byte hasher that `src/headless.rs` (`world_hash`) and `src/save.rs`
  (`state_hash`) build on.

The file's own comment states the doctrine: the hash is a public API once golden fixtures exist, and changing
any constant or the tag scheme "invalidates every seed in the wild (MAJOR version bump)"; `hash_vectors`'
comment adds that every save breaks too (`CLAUDE.md` "Seed compatibility").

## Tests
The unit tests are inline in `src/main.rs` (`mod tests`), run with `PATH="$HOME/.cargo/bin:$PATH"
RUSTUP_TOOLCHAIN=1.75 cargo test --quiet <name>`: `hash_vectors`, `worldgen_isolated_from_combat`,
`parley_isolated_from_combat`. The map goldens `tests/golden/seed_*.txt` (`golden_dumps`, and `make goldens`)
pin the maps and theme names these primitives generate.

## Changes
none: imported

## Gate
- **E01-AC-1** — `h64` and `channel` return the captured vectors: `h64(0, &[])` is `0x7bd3_144f_29c0_cc9e`, `h64(42, &["spawns", "5"])` is `0x2973_9991_fe04_caf0`, and `channel(42, &["worldgen", "1"])` yields `0xeda9_7859_383a_600c`, `0xf4e6_8a47_e74b_97cc`, `0x7162_bb71_2f5f_73e3` as its first three draws (with three more `h64` vectors in the test). *Verify:* `hash_vectors` in `src/main.rs`.
- **E01-AC-2** — Draws on one named channel never change another channel's stream: 1000 draws each on the combat and AI streams leave depth 2's generated level identical, and 1000 parley draws leave combat damage identical. *Verify:* `worldgen_isolated_from_combat` and `parley_isolated_from_combat` in `src/main.rs`.
- **E01-AC-3** — `Rng` (xorshift64, odd seed, `range` and `chance`) drives worldgen deterministically, so the five golden seeds generate their frozen maps. *Verify:* `golden_dumps` in `src/main.rs` (against `tests/golden/seed_*.txt`).
- **E01-AC-4** — `fnv_bytes` is FNV-1a(64) with prime `0x100_0000_01b3`; no test pins its output to fixed values (the replay tests compare two runs' hashes with each other, and `make xhash` compares the two backends). *Verify:* unpinned.
- **E01-AC-5** — The file has no `cfg` block, no platform call and no game noun. *Verify:* unpinned (the `CLAUDE.md` grep rule, not a tracked check).

## Risks
- Any change to a constant, the tag separator or the finalizer is a MAJOR, seed-breaking change that needs the
  owner's explicit sign-off; `hash_vectors` fails even when the goldens happen to pass.
- `fnv_bytes` feeds `state_hash`, `world_hash` (the 16-hex world id a portal's describe line shows) and the
  hash a ghost file stores, and no test or gate target catches a change to it: `tests/fixtures/ref.sav` holds
  a seed and inputs but no hash, and `make xhash` compares two builds of the same code.
- `range` uses a modulo, so its draws are slightly biased; that bias is frozen with the goldens.
