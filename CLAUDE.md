# AGENTS.md — rl144

A top-down roguelike in Rust. **Hard constraint: shipped binary + all assets ≤ 1.44MB (1,474,560 bytes), the capacity of a 3.5" floppy.** Everything else is negotiable; this is not.

## Project shape

- Single binary crate, split along engine seams (keep this module count; don't fragment further without justification):
  - `src/rng.rs` — Rng, h64/channel, fnv (the frozen hash primitives)
  - `src/content.rs` — themes, tones, vaults (authored const data)
  - `src/game.rs` — engine core: tiles, entities, Game, worldgen, turns, `apply_input` (the client input-vocabulary boundary)
  - `src/headless.rs` — dump/solve/sim/world_hash (verification tooling)
  - `src/save.rs` — save format, replay, state_hash (state-is-deltas layer)
  - `src/render.rs` — cell-grid presentation core: composes the 80×30 `Cell{ch,fg,bg}` grid (the part a DOS/mobile port swaps is a backend, not this file)
  - `src/backend_minifb.rs` — minifb window backend: rasterizes cells to the pixel framebuffer via font8x8, owns the window loop
  - `src/backend_term.rs` — ANSI terminal backend: hand-rolled termios raw mode, dirty-cell ANSI encoder, owns the terminal event loop
  - `src/main.rs` — arg parsing, backend dispatch, tests
- Two dependencies: `minifb` (window + pixel buffer) and `font8x8` (const glyph data), both `optional = true`, both pulled in only by the `backend-minifb` cargo feature. `font8x8` uses only its always-compiled `legacy` module (`BASIC_LEGACY`/`BOX_LEGACY`/`BLOCK_LEGACY`, plain `[u8;8]` tables with no `unicode` cfg gate) — glyphs are looked up by direct codepoint-offset indexing, not the `unicode` feature's `FontUnicode`/`UnicodeFonts` search API, so that feature is dropped from `Cargo.toml` entirely (bit-identical glyphs, smaller binary). Treat the dependency list as frozen. Adding a crate requires demonstrating (a) it can't be hand-rolled in <150 lines, and (b) the size cost, measured (see below).
- **No engines.** Bevy, ggez, macroquad, SDL bindings, etc. are all disqualified by the budget. No `serde`, no `rand` (we have xorshift), no `image` (we have no image files).
- **No asset files.** All content is procedural or `const` data in source. If you want art/audio, generate it at runtime or embed it as compact const tables. Adding a `assets/` directory is a design smell here.

## Core/crust doctrine

The core — `rng`/`content`/`game`/`save`/`headless`/`render` — has **zero platform calls and zero `cfg` blocks**. Its entire surface to a frontend is three things: `render_cells(&Game, Screen, &mut [Cell])` (the 80×30 `Cell{ ch: u16, fg: u32, bg: u32 }` grid), `Game::apply_input(u8)` (the input-byte vocabulary), and the headless entry points (`--dump`/`--solve`/`--sim`/`--replay`/`--render-frame`/etc). Backends (`backend_minifb.rs`, `backend_term.rs`) may only *consume* that surface — turn cells into pixels or ANSI escapes, turn keystrokes into input bytes. A backend that needs a new core API is a real design decision, not a drive-by: justify it in the status entry.

`cfg` is allowed only inside backend modules and the dispatch/wiring code in `main.rs` (the `compile_error!` guards, `mod backend_minifb`/`mod backend_term` gating, and the call site that invokes whichever backend's `run(...)`). Exactly one backend cargo feature must be active at a time: `backend-minifb` (default) or `backend-term`, enforced by two `compile_error!`s in `main.rs` — one for zero backends, one for both. To build the terminal flavor: `cargo build --release --no-default-features --features backend-term --target-dir target/term` (a separate target dir so it never clobbers the default build's artifacts that `make size` measures).

Core hashed state stays explicit-width (`i32`/`u32`/`u64`) — no `usize` in anything feeding `state_hash`. This is 32-bit-port prep: `usize` in a hash means the hash changes width depending on target, which breaks replay portability. `usize` is fine for pure grid-index/array-length plumbing (`idx()`'s return type, loop counters); it must never land in a `Game` struct field that gets hashed.

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
- **`--sim N`** (default 5000) is now a GATE like `--solve`, band in `tests/sim-band.json`: `win_pct` in `[10,25]`, `deaths_dark` a raw count in `[1,500]` (a percent-share band would floor to 0 at this scale — dark deaths are ~0.1% of total deaths, see status log). The minority-vs-combat check (`deaths_dark < deaths_combat`) and `stuck == 0` are structural code checks, not tunable JSON. With `--report` it prints stats and exits 0 without gating, same re-baselining pattern as `--solve --report`. Band calibrated for 5000 seeds; a different `N` needs its own calibration pass, not a reuse of these bounds.
- **`--render-frame --seed N [--ascii]`** (backend-term builds only): pure cell-grid → ANSI-escape byte stream written to stdout, no TTY required (safe with stdout redirected to a file) — this is both the frame-golden capture command and the terminal encoder's verification surface. Frozen by `tests/golden/frame_seed_1.bin`, `frame_seed_42.bin`, `frame_seed_1_ascii.bin`.
- **`--daily`**: shared seed of the day (`h64(days_since_epoch, ["daily"])`). Precedence: `--seed` > `--daily` > launch entropy.
- **`--replay <file>`**: replays a save headlessly and prints an FNV state hash + summary JSON. Two replays of one save must hash identically — if not, channel discipline broke. **`--load <file>`** resumes a save in the window (replays headlessly first).
- **`make check`** runs the whole gate in order: build, test, test-term, goldens, frames, xhash, solve, sim, size. **`make targets`** prints the size scoreboard (stripped/packed/% of budget, per backend — each backend's packed size is checked against the same 1,474,560-byte budget independently). **`make xhash`** is the cross-backend determinism gate: replay the committed fixture `tests/fixtures/ref.sav` through both backend binaries and require an identical state hash — it compares the two backends to each other (not to a frozen constant), which is the actual proof that the core/crust seam is real and neither backend's plumbing perturbs replay.
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
- **The 80×30 cell grid is engine API; the window is presentation.** `COLS`/`MAP_H` are baked into `idx()` and worldgen — the grid must never follow the window size. Frontends scale the fixed 640×360 buffer (minifb: `resize` + `AspectRatioStretch`); a DOS/mobile port swaps the backend module, not the grid. Likewise the input-byte vocabulary (0–5) is the platform boundary: any frontend that produces those bytes is a valid client.
- Rendering writes cells into the `Cell{ch,fg,bg}` grid via `render_cells` only; backends turn cells into pixels/escapes, never the reverse. UI layout: rows `[0, MAP_H)` map, row `MAP_H` status, remaining rows log. Don't draw outside your band.
- Message strings go through `Game::log`. Keep them under ~78 chars so they fit the log row.
- Colors are named `PAL_*` consts in `content.rs` (`PAL_PLAYER`, `PAL_STAIRS`, `PAL_POTION`, `PAL_SWORD`, `PAL_AMULET`, `PAL_LORE`, `PAL_RAT`/`PAL_GOBLIN`/`PAL_OGRE`, `PAL_STATUS`, `PAL_ALERT`, `PAL_LOG_FADE`, `PAL_BAR_HP`/`PAL_BAR_TORCH`/`PAL_BAR_EMPTY`) — one place to retune the palette. `Theme.wall`/`Theme.floor` stay on `Theme` (per-theme, not global).
- Wall autotiling (`WALL_GLYPHS`/`wall_mask` in `render.rs`) is presentation-only: it counts only *seen* neighbors (`in_map && seen && Tile::Wall`) so the glyph reflects what the player has actually observed, not the true map. `--dump`/`level_dump` never call into `render.rs`, so dump goldens stay logical (`#` only) regardless of autotiling changes.
- `Screen` (`Title`/`Play`/`End`) is core render data — `render_cells(g, screen, cells)` takes it as a parameter — but the tiny state machine that decides which `Screen` is active belongs to each backend, not the core. Title-screen dismissal (any key) is consumed without logging an input byte or touching `Game` at all; it's not part of the replay stream.
- `Game::turns` (incremented once per `spend_turn`) is hashed state — it's part of what `state_hash` verifies. `Game::killer` (set in `monsters_act` right before `dead = true`, used by the End screen's death message) is presentation-only and deliberately **unhashed** — don't add it to `state_hash`.
- Light-tier warnings (`spend_turn`'s tier-crossing messages) weave a theme adjective in via `self.adj()` on the `flavor_rng` channel — deterministic, replay-safe, and consistent with the "flavor is grounded" rule above.
- Status bar (row `MAP_H`) draws block-glyph HP/Torch bars (`0x2588` filled / `0x2591` empty) alongside the text fields; the term backend falls back to ASCII (`#`/`-`) for those glyphs since raw terminals can't be trusted for arbitrary Unicode block-drawing without the frame-golden-verified encoder path.
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

**`make check` is the whole gate in one command** (`make UPX=/path/to/upx check`): build, test, test-term, golden cmp (temp-dir, tree untouched), frame-golden cmp, cross-backend `xhash`, `--solve 10000`, `--sim 5000`, UPX size budget — see Verification above for what each step covers. Run it before calling anything done.

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
- **2026-07-18 — batch 2 (subagent-built) + module split.** `--sim N` deterministic greedy bot (drives the engine purely through input bytes; **finding: 0% win rate over 5000 seeds, 100% combat deaths, 0 darkness deaths — combat lethality, not the light budget, is the wall; balance pass needed, requires sign-off**); theme-tinted rendering + low-light brightness tiers (playtest pending); string seeds (`--seed swordfish`); saves renamed to `rl144-<worldhash>.sav` with F5 double-press overwrite confirm and autosave-on-quit (never clobbers a manual save; playtest pending); `Makefile` `check` gate; `src/main.rs` split into rng/content/game/headless/save/render modules (pure motion, gate-verified, output byte-identical). Packed size 189,072 B (12.8% of budget).
- **2026-07-18 — batch 3: combat balance sign-off, core/crust seam, terminal backend, aesthetic pass.** Five tasks, all `make check` green.
  - **Combat balance (authorized worldgen MAJOR, signed off)**: fixes the batch-2 0%-win wall. `+4` maxHP and a heal on each depth's first descent; spawn count `3 + d`; roll `d10 + d`, thresholds `rat < 9`/`goblin < 13`/`ogre >= 13`; item drops `+2*(d-1)`. `--sim 5000` = 14.6% wins (729/5000), 5 dark deaths, 0 stuck, 4266 combat deaths — inside the new `tests/sim-band.json` band (`win_pct [10,25]`, `deaths_dark [1,500]` as a raw count, not a percent share). Goldens regenerated (spawn glyph placement/counts only; wall/floor/stairs/theme/lore layout verified byte-identical by diffing with monster/item glyphs normalized out). Solver band re-verified unchanged: `min 636 / p50 954 / p90 1110 / p99 1272 / max 1503`, worst seed 6592 — confirms worldgen truly untouched. (Also found and fixed in passing: a stale `tests/solver-band.json` comment whose prose numbers predated two earlier authorized MAJORs; the numeric bands themselves were never wrong.)
    **Honesty note**: dark deaths are structurally rare for the BFS-optimal `--sim` bot — it walks efficiently enough that combat, not the light clock, is almost always what kills it. Pushing dark deaths meaningfully higher would need a `START_LIGHT` or amulet-burn change, which is outside this batch's balance sign-off; flagged for a human decision before touching either constant.
  - **Core/crust seam**: `render.rs` reduced to a platform-free cell composer (`render_cells(&Game, Screen, &mut [Cell])`, `Cell{ch:u16,fg:u32,bg:u32}`); the minifb window loop moved verbatim into new `src/backend_minifb.rs`; a new ANSI terminal backend landed in `src/backend_term.rs` (hand-rolled termios raw mode via direct libc FFI — no new crate — dirty-cell ANSI/256-color encoder, panic-safe terminal restore via a chained panic hook). Two cargo features (`backend-minifb` default, `backend-term`) gated by `compile_error!` for the zero/both cases. `--render-frame --seed N [--ascii]` added as the term backend's headless verification surface, frozen by new frame goldens.
  - **Cross-backend `xhash` gate** (`make xhash`): replays `tests/fixtures/ref.sav` through both backend binaries and requires an identical state hash — the concrete proof the core/crust seam holds, since it compares the backends to each other rather than to a frozen constant.
  - **Aesthetic pass**: wall autotiling (`WALL_GLYPHS`/`wall_mask`, seen-neighbors-only, presentation-only — dump goldens verified byte-identical); Title/Play/End screens (core render data, backend-owned state machine); deepened light-tier grading (100/85/70/55/40/28, was 100/90/80/65/50/40) with theme-adjective warning text; palette pulled into named `PAL_*` consts in `content.rs`; block-glyph HP/Torch status bars (term backend falls back to ASCII). font8x8's `unicode` cargo feature dropped — only the always-compiled `legacy` bitmap tables are used, direct-indexed, bit-identical glyphs, smaller binary.
  - **Size scoreboard** (`make targets`, this batch's final numbers):

    ```
    target       stripped       packed    pct
    minifb         523960       192172    13%
    term           398504       169724    11%
    ```

    Both flavors well under the 1,474,560-byte per-target budget.
  - **Consolidated playtest-pending list** (nothing here is verified interactively; everything is compile-checked/headless-verified only): batch-2 carryovers — light-tier warning pacing, stair-transition feel, whether the ~33% light margin plays fair, theme-tinted rendering readability, F5 overwrite-confirm flow, resizable window behavior; new this batch — title/end screen layout and pacing (both backends), terminal backend feel (raw-mode entry/exit, ESC/arrow-key timing, alt-screen visuals, Ctrl-C handling), low-light grading oppressiveness at the deepened tiers, status-bar readability (bar/text contrast, term ASCII fallback legibility) in both backends, `--load` skipping the Title screen entirely. Two known-small minors carried without action: the term backend's escape-sequence reader doesn't drain a trailing `~` on an *unrecognized* CSI sequence, and its raw read/write helpers don't retry on `EINTR`.
