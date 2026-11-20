# P02 — backend-minifb

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | E01, E02, E04, C02, S01, S02, P01 | M01 | M |

**Owns:** `src/backend_minifb.rs`, the window backend (the default `backend-minifb` feature).

## Purpose
`src/backend_minifb.rs` is the pixel frontend, built with the default `backend-minifb` feature (the only
user of the `minifb` and `font8x8` crates). What it calls of the core: `render::render_cells` (with `Cell`,
`CELLS` and `Screen`); `Game::apply_input` with the input-byte vocabulary; `Game::new_overworld`,
`Game::held_summary` and `Game::log`, and the `Game` fields `seed`, `dead`, `won`, `depth`, `px`, `py`,
`turns`, `killer`, `fx_hit` and `echo` (written on a retry); `game::COLS` and `ROWS`; the save builders and
constants of `src/save.rs` (`save_bytes`, `save_filename`, `ghost_bytes`, `ghost_filename`, `INPUT_RETRY`,
`INPUT_RESTART`, `GHOST_DIED_COMBAT`, `GHOST_DIED_DARK`); `headless::world_hash`; `rng::h64`;
`content::ghost_label_idx`; and `GAME.items[..].glyph` through `crate::games::GAME`. These are why its
deps include E01, E02, E04, C02, S01 and S02 as well as P01.
- **Rasterizing.** Each cell is an 8×12 rect of a 640×360 buffer (`WIDTH`, `HEIGHT`). `glyph_bits` resolves a
  glyph by direct index into font8x8's legacy tables: ASCII from `BASIC_LEGACY`, U+2500 to U+257F from
  `BOX_LEGACY`, U+2580 to U+259F from `BLOCK_LEGACY`, any other code point a blank cell. `draw_glyph` fills
  the rect with `bg` and stamps the 8-pixel glyph centred vertically. On the Play screen only, the cell at
  `Game::fx_hit` is drawn by `draw_glyph_squashed` (the even glyph rows, shifted down) in `flash(fg)`, each
  channel brightened by 30% and clamped at 255. The window scales the fixed buffer, keeping its aspect
  (`ScaleMode::AspectRatioStretch`), at about 60 frames a second.
- **Input** (`run`). On Play: arrows, `wasd` and `hjkl` push the move bytes 0-3 (held keys repeat); `.` waits
  (4); `t` then a direction pushes a talk byte (7-10) and `g` then a direction a give byte (11-14), a second
  `t` or `g` re-arming and any other key cancelling the chord silently; `u` pushes 15 with empty hands, and
  otherwise logs the use menu (`use_menu_line`, items named by glyph) and takes a digit 1-9 for `17 + kind`;
  `p` pushes 16. Each pushed byte goes to the session's `input_log`, the attempt's `attempt_log` and
  `Game::apply_input`. F1 logs the seed and world hash; F5 saves `save_bytes` to `save_filename(world_hash)`,
  asking for a second F5 before it overwrites an existing file. The armed chords are frontend state, never
  logged or hashed.
- **Screens.** The run opens on Title unless `--load`ed; any key dismisses it without an input byte. When the
  game is dead or won it shows End, one rendered frame late when a killing-blow flash is pending
  (`pending_end`). On End, `R` writes a ghost of a dead attempt, pushes `INPUT_RETRY` (6) and restarts
  `Game::new_overworld` on the same seed with `echo` set to the death tile; `N` writes the ghost, pushes
  `INPUT_RESTART` (5) and restarts on `h64(seed, ["restart"])`; `Q` writes the ghost and quits. The window
  title shows the seed (and the daily number for a daily run).
- **Files.** `write_ghost` writes `ghost_bytes` of a dead attempt to `ghost_filename`, ignoring a failed write.
  On quit (Escape or closing the window), a live run with input is autosaved to its save file, or to a
  `rl144-<hash>.auto.sav` sibling when the save file exists. `Q` acts only on the End screen, where no run
  is live, so it never autosaves.

## Tests
The inline `#[cfg(test)] mod tests` of `src/backend_minifb.rs` holds `flash_channel_math`, run with
`PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.75 cargo test --quiet flash_channel_math`. The release build
of the default feature (`make build`, with `RUSTFLAGS="-D warnings"`) compiles the file. No test opens a
window or drives the input loop; `make xhash` runs this build's `--replay`, which `src/main.rs` handles
without entering this file.

## Changes
none: imported

## Gate
- **P02-AC-1** — `flash` brightens each RGB channel by 30% (integer `* 13 / 10`) and clamps it at 255. *Verify:* `flash_channel_math` in `src/backend_minifb.rs`.
- **P02-AC-2** — The default release build (`backend-minifb`) compiles with no warning. *Verify:* `make build` (target of `Makefile`, `RUSTFLAGS="-D warnings"`).
- **P02-AC-3** — The keys push the input bytes described in Purpose (moves, wait, the talk and give chords with re-arm and silent cancel, use with the digit selector, put-down), each to `input_log`, `attempt_log` and `Game::apply_input`; F1, F5 and the Title dismissal push none. *Verify:* unpinned (needs interactive playtest).
- **P02-AC-4** — The End screen's `R` pushes byte 6 and restarts on the same seed with `echo` at the death tile, `N` pushes byte 5 and restarts on `h64(seed, ["restart"])`, both through `Game::new_overworld`; a dead attempt's ghost is written before `R`, `N` or `Q`, never after a win. *Verify:* unpinned (needs interactive playtest).
- **P02-AC-5** — Glyphs come from font8x8's legacy tables by direct index (other code points draw blank); the `fx_hit` cell is squashed and flashed on the Play screen only; End follows a killing-blow flash by one frame. *Verify:* unpinned (needs interactive playtest).
- **P02-AC-6** — F5 asks for a second press before it overwrites a save; quitting a live run with input (Escape or closing the window) autosaves, to the `.auto.sav` sibling when the save file exists. *Verify:* unpinned (needs interactive playtest).
- **P02-AC-7** — The file names no game noun as an identifier or a string literal: the use menu names items by glyph. *Verify:* unpinned (the `CLAUDE.md` grep rule, not a tracked check).

## Risks
- The backend calls more of the core than the four-item surface of `CLAUDE.md`'s core/crust section
  (`render_cells`, `scene`, `apply_input`, the headless entry points): `world_hash`, `h64`,
  `ghost_label_idx`, `held_summary`, `log`, `new_overworld`, the `Game` fields and `GAME.items` listed in
  Purpose. `CLAUDE.md` calls a backend's need for a new core API a design decision to justify; these
  calls predate the regime and no spec records that justification.
- Nothing but a build and one colour test checks this file; every key, chord, screen transition and file
  write is "needs interactive playtest" in `CLAUDE.md`'s terms. A change here can only be compile-checked
  headlessly.
- The two backends duplicate `use_menu_line`, `write_ghost`, the End screen's restart logic and the autosave
  on purpose (their doc comments say so); the restart logic must stay the same as `save::replay`'s
  reconstruction bytes, and the bytes the same physical keys push must stay the same in both backends.
  `CLAUDE.md` asks that the title's key legend (`src/render.rs`) land in the same commit as any key it
  documents.
- The title's legend says "Quit: q", but this backend has no `q` on the Play screen: Escape quits there,
  and `Q` works only on the End screen. The terminal backend does quit on `q` during play. Under the
  `CLAUDE.md` legend rule, the legend and this backend's keys disagree.
- `Cargo.toml` builds `minifb` with `default-features = false, features = ["x11"]` for every target;
  `CLAUDE.md` notes that macOS and Windows need its default features, as a target-specific flag.
- The window title holds a non-ASCII em dash; it goes to the window system, not to the cell grid.
- The test this node names is in `src/backend_minifb.rs`, which only the implementer may write until the
  tests move to `src/tests/*.rs` (`SPEC.md`'s open decisions).
