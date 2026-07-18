# AGENTS.md — rl144

A top-down roguelike in Rust. **Hard constraint: shipped binary + all assets ≤ 1.44MB (1,474,560 bytes), the capacity of a 3.5" floppy.** Everything else is negotiable; this is not.

## Project shape

- Single binary crate. All game code currently lives in `src/main.rs` (~550 lines). If you split it into modules, keep the module count low and justified — this is a small game, not a framework.
- Two dependencies: `minifb` (window + pixel buffer) and `font8x8` (const glyph data). Treat the dependency list as frozen. Adding a crate requires demonstrating (a) it can't be hand-rolled in <150 lines, and (b) the size cost, measured (see below).
- **No engines.** Bevy, ggez, macroquad, SDL bindings, etc. are all disqualified by the budget. No `serde`, no `rand` (we have xorshift), no `image` (we have no image files).
- **No asset files.** All content is procedural or `const` data in source. If you want art/audio, generate it at runtime or embed it as compact const tables. Adding a `assets/` directory is a design smell here.

## Size budget rules

Current baseline (Linux x86_64, Ubuntu rustc 1.75):
- stripped release build: ~506 KB
- after `upx --best --lzma`: ~166 KB

The number that counts against the 1.44MB limit is the **UPX-packed binary** (plus any asset files, of which there must be none).

After ANY change that adds code or data:

```sh
cargo build --release
ls -la target/release/rl144          # unpacked size
cp target/release/rl144 /tmp/p && upx --best --lzma -qq /tmp/p && ls -la /tmp/p
```

Report both numbers in your summary of work. Rules of thumb:
- A feature that adds >50 KB packed needs an explicit justification.
- If packed size ever exceeds 1 MB, stop adding features and shrink first (`cargo bloat --release` is the tool; it may need installing).
- Never remove the size flags in `Cargo.toml` (`opt-level = "z"`, `lto`, `codegen-units = 1`, `panic = "abort"`, `strip`). If a change requires unwinding panics or debug symbols, it's the wrong change.
- Do not commit UPX-packed binaries into `target/`; pack into a copy. Packed binaries in the repo root named `rl144-<platform>-upx` are release artifacts.

## Verification (headless-first)

There is no display in CI/container environments. The window cannot be opened there. Therefore:

- **`--dump` mode is the test harness.** `./target/release/rl144 --dump --seed N` generates all 5 depths of seed N and prints each as ASCII (theme header + lore line + map + `monsters=N items=M`). It must always work and must not open a window. Without `--seed` it uses a time-derived seed.
- **`--solve N`** (default 10000): the winnability + difficulty gate. Generates all depths per seed, BFS-checks the exit is reachable, computes the round-trip walk budget (descend ×1 + climb out ×2 per step), prints JSON stats (`min/p50/p90/p99/max`, worst seed), and exits nonzero on any unwinnable seed or drift outside `tests/solver-band.json`. Run it after ANY worldgen-adjacent change. With `--report` it prints stats and exits 0 without gating (the re-baselining flow).
- **`--daily`**: shared seed of the day (`h64(days_since_epoch, ["daily"])`). Precedence: `--seed` > `--daily` > launch entropy.
- **`--replay <file>`**: replays a save headlessly and prints an FNV state hash + summary JSON. Two replays of one save must hash identically — if not, channel discipline broke. **`--load <file>`** resumes a save in the window (replays headlessly first).
- Any new system (new gen algorithm, new entity type, new item) should be observable via `--dump` or a new headless flag. Extend headless modes rather than claiming "it probably works."
- Sanity checks worth running after gen changes: `--solve 10000` green; every depth has one `<` at the entrance; depth 1–4 have exactly one `>`; depth 5 has the amulet and no `>`. (In dumps the player `@` sits on top of the entrance `<`.)
- Logic that can't be seen in a dump (combat math, FOV symmetry) should get `#[test]` functions. `cargo test` must stay green. Tests are free — they don't ship in the release binary.
- Window-path changes (input handling, frame pacing, minifb API usage) can only be compile-checked headlessly. Flag them explicitly in your summary as "needs interactive playtest" — do not report them as verified.

## Seed compatibility (MAJOR-version doctrine)

Worldgen output is a public API, frozen by the golden fixtures in `tests/golden/` (full 5-depth dumps for seeds 1, 2, 3, 42, 1337; `cargo test` string-compares them). Any change that diffs a golden **map layout** — the channel hash constants, the tag scheme, the order or count of draws on a worldgen/spawns/vault/theme channel, room/corridor/placement logic — breaks every seed and save in the wild. That is a MAJOR version bump requiring explicit human sign-off, never a drive-by. To re-baseline after sign-off: regenerate with `--dump --seed N > tests/golden/seed_N.txt`, re-run `--solve 10000`, re-commit `tests/solver-band.json`, and re-derive `START_LIGHT` from the new worst-case budget (its comment documents the derivation — keep it current). Dump-format-only diffs (headers, flavor lines) are not seed-breaking but still need the goldens regenerated; say so in the commit message.

## Code conventions

- **All randomness flows through named channels.** `channel(seed, &[tags])` (FNV-1a-64 + finalizer → xorshift64 `Rng`) isolates streams: `worldgen`/`spawns`/`vault`/`theme` are keyed per depth and frozen by the goldens; `combat`, `ai`, and `flavor` are per-run streams on `Game`. Never let one channel's draws leak into another — combat rolls must not perturb worldgen (there's a test). Never call system entropy mid-game; the only entropy is the launch-time seed.
- Coordinates are `i32`, grid indices via `idx(x, y)`, bounds via `in_map(x, y)`. Don't introduce a second coordinate convention.
- Entity/item stats live in the `Monster::stats`-style const match tables, not scattered magic numbers. New content = new table row.
- Turn structure: player acts (burning light via `spend_turn`) → `monsters_act()` → `compute_fov()`. Preserve this ordering; monsters must never act on a level the player just arrived on (see the early `return`s after stair transitions — they're intentional).
- Light is the run's clock: 1 per turn, 2 while carrying the amulet; 0 = death in the dark, and the lose check runs **before** the win check. `START_LIGHT` is solver-derived — its comment documents the derivation; don't retune it by feel.
- The win is a round trip: the amulet is picked up on depth 5 and carried to depth 1's `<`. Visited depths persist via `LevelState` snapshots — the climb out is through the world the player left. Don't regenerate visited levels.
- Bumping a wall costs no turn and burns no light (intentional). Pickup is on walk-over (intentional for v0; an inventory is a v1 discussion, not a drive-by refactor).
- Flavor is grounded: theme lore/adjectives may only restate things the engine did — never invent entities, exits, or events. Themes are const tables (`THEMES`); vaults are const strings (`VAULTS`) with the legend documented at the definition.
- Saves are seed + input log (`RL14` header format in `save_bytes`), never serialized world state. Anything that makes replay diverge from live play is a bug by definition.
- **The 80×30 cell grid is engine API; the window is presentation.** `COLS`/`MAP_H` are baked into `idx()` and worldgen — the grid must never follow the window size. Frontends scale the fixed 640×360 buffer (minifb: `resize` + `AspectRatioStretch`); a DOS/mobile port swaps the presentation block in `main`, not the grid. Likewise the input-byte vocabulary (0–5) is the platform boundary: any frontend that produces those bytes is a valid client.
- Rendering writes glyphs into the `u32` framebuffer via `draw_char`/`draw_str` only. UI layout: rows `[0, MAP_H)` map, row `MAP_H` status, remaining rows log. Don't draw outside your band.
- Message strings go through `Game::log`. Keep them under ~78 chars so they fit the log row.
- `rustc` here is 1.75 (Ubuntu apt). Don't use language/std features newer than that, and don't pick crate versions whose MSRV exceeds it.

## Platform notes

- Linux builds use `minifb` with `default-features = false, features = ["x11"]` to avoid wayland build deps. macOS/Windows builds need minifb's default features — if you add cross-platform CI or docs, that's a target-specific feature flag, not a global change.
- `upx-ucl` and `rustc`/`cargo` install from Ubuntu apt in sandboxed environments (no rustup — its domain is often blocked).

## Roadmap context (so you don't re-litigate v0 cuts)

Deliberately cut from v0, in rough priority order for v0.1+:
1. Audio: small software synth + tracker-style const song data (~20–30 KB budget)
2. Procedural tile sprites replacing/augmenting glyphs (~10 KB budget)
3. Inventory UI + deliberate item use (replaces walk-over pickup)
4. Ranged combat, more monster/item variety

Landed in v0.1 (was cut from v0): save/load — implemented as seed + input log, single slot, no serde.

**Direction (2026-07-18, per the human): rl144 is drifting engine-ward, not just game-ward.** Two win conditions: (a) a ≤1.44MB executable game, (b) a tiny MMORPG. Networking is therefore no longer permanently cut — it may return behind a compile-time feature flag, built as lockstep input-sharing over the existing deterministic replay core (state = `replay(seed, input_log)`; multiplayer = relaying input bytes, never serializing the world). This makes channel discipline and replay convergence engine API, not test hygiene. Still cut permanently: mod support, config files, localization.

## Definition of done for any change

1. `cargo build --release` clean (warnings count as not clean).
2. `cargo test` green.
3. `--dump` (and any relevant headless mode) output eyeballed and sane; `--solve 10000` green after worldgen-adjacent changes.
4. Both size numbers (stripped, packed) reported, with delta vs. baseline.
5. Anything unverifiable headlessly explicitly flagged for human playtest.
6. If you changed gameplay behavior, update this file's conventions section when the change makes a stated invariant false.

## Status log (append-only; date each entry)

- **2026-07-18 — v0.1 landed.** Channel RNG (`h64`/`channel`, worldgen frozen by goldens); `--solve` winnability + difficulty-band gate (10K seeds, band in `tests/solver-band.json`); torch mechanic (run-wide light pool, tiered FOV, `START_LIGHT=2000` derived from worst-case round-trip budget 1503); return-trip win with up-stairs and persistent `LevelState` snapshots; exit/amulet in BFS-deepest room; 4 const themes with grounded slot-filled lore; 3 ASCII vaults stamped via their own channel; save/replay as seed + input log (`--replay`, `--load`, F5). Sizes (local rustc 1.97, Arch): stripped 478,680 B, packed 173,176 B (baseline was 440,912 / 157,664 on this box; Ubuntu 1.75 baseline in this doc predates it). Two authorized worldgen MAJOR re-baselines: task 3 (deepest-room placement + `<`), task 5 (vaults).
- **2026-07-18 — human playtest:** F5 save + `--load` resume confirmed working in the window. Still unverified interactively: light-tier warning pacing, new status bar readability, stair-transition feel, whether the ~33% light margin plays fair.
- **2026-07-18 — golem cheap wins + engine reframing.** `hash_vectors` test freezes the h64/channel primitive directly; `--solve --report` (ungated stats); `--daily` shared seed; room kinds+tones (message-only, goldens verified byte-identical); story-buried-by-depth lore inscriptions (`?` items at BFS shallow/mid/deep rooms — authorized worldgen MAJOR, goldens regenerated, solver stats unchanged); resizable window over the fixed 80×30 grid (needs playtest). Roadmap updated: engine direction, networking may return behind a compile flag. Also: world identity in-game — seed in the window title, F1 logs `seed + world_hash` (FNV over the 5-depth dump: names the generator's *output*, so it shifts exactly when a worldgen MAJOR would).
