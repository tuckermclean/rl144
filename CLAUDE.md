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

- **`--dump` mode is the test harness.** `./target/release/rl144 --dump` generates a level and prints it as ASCII plus a `monsters=N items=M` summary line. It must always work and must not open a window.
- Any new system (new gen algorithm, new entity type, new item) should be observable via `--dump` or a new headless flag (e.g. `--sim N` to run N random turns and print the outcome). Extend headless modes rather than claiming "it probably works."
- Sanity checks worth running after gen changes: every level is fully connected (player can reach stairs/amulet); depth 1–4 have exactly one `>`; depth 5 has the amulet and no `>`.
- Logic that can't be seen in a dump (combat math, FOV symmetry) should get `#[test]` functions. `cargo test` must stay green. Tests are free — they don't ship in the release binary.
- Window-path changes (input handling, frame pacing, minifb API usage) can only be compile-checked headlessly. Flag them explicitly in your summary as "needs interactive playtest" — do not report them as verified.

## Code conventions

- Hand-rolled over imported: PRNG is xorshift64 in `Rng`; keep using it. Determinism matters — all randomness must flow through `Game::rng` so a seed reproduces a run. Never call system entropy mid-game.
- Coordinates are `i32`, grid indices via `idx(x, y)`, bounds via `in_map(x, y)`. Don't introduce a second coordinate convention.
- Entity/item stats live in the `Monster::stats`-style const match tables, not scattered magic numbers. New content = new table row.
- Turn structure: player acts → `monsters_act()` → `compute_fov()`. Preserve this ordering; monsters must never act on a level the player just left (see the early `return` after stair descent — it's intentional).
- Bumping a wall costs no turn (intentional). Pickup is on walk-over (intentional for v0; an inventory is a v1 discussion, not a drive-by refactor).
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
5. Save/load (single slot, small binary format, no serde)

Cut permanently unless the human says otherwise: networking, mod support, config files, localization.

## Definition of done for any change

1. `cargo build --release` clean (warnings count as not clean).
2. `cargo test` green.
3. `--dump` (and any relevant headless mode) output eyeballed and sane.
4. Both size numbers (stripped, packed) reported, with delta vs. baseline.
5. Anything unverifiable headlessly explicitly flagged for human playtest.
6. If you changed gameplay behavior, update this file's conventions section when the change makes a stated invariant false.
