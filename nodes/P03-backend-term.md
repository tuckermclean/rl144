# P03 — backend-term

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | E01, E02, E04, C02, S01, S02, P01 | none | M |

**Owns:** `src/backend_term.rs`, the ANSI terminal backend (the `backend-term` feature) and `--render-frame`.

## Purpose
`src/backend_term.rs` is the terminal frontend, built with `--no-default-features --features backend-term`.
It adds no crate. It calls the same parts of the core as the window backend (P02's Purpose lists them)
except the `fx_hit` field, plus `Game::camera_focus` (the camera console), `Game::new` (for
`--render-frame`) and `game::MAP_H`.
- **Terminal I/O.** Hand-written `extern "C"` declarations of `tcgetattr`, `tcsetattr`, `read`, `write` and
  `ioctl` (libc, linked through std), with a `Termios` layout for 32- and 64-bit Linux. `enter_raw_mode`
  turns off `ICANON`, `ECHO` and `ISIG` (Ctrl-C arrives as byte 3 and quits), sets blocking one-byte reads,
  and enters the alternate screen with the cursor hidden; `restore_terminal` undoes it, on exit and from a
  chained panic hook. `term_size` reads the window size with `TIOCGWINSZ`.
- **The encoder.** `frame_bytes(cells, w, h, prev, ascii)` is pure: with no `prev` it clears the screen
  (`ESC[2J ESC[H`) and writes every cell; with a `prev` it writes only the cells that changed. It emits a
  cursor move only where the next cell is not where the cursor already is, and a colour escape only when the
  256-colour index changes (`ESC[38;5;Nm`, `ESC[48;5;Nm`). `c256` maps 24-bit RGB to the xterm palette: a
  near-grey colour (channel spread under 16) to the grey ramp 232-255 (16 below 8, 231 above 238), else the
  6×6×6 cube. Glyphs of 128 and up are written as UTF-8, or with `ascii` through `ascii_fallback` (the full
  block `#`, the light shade `-`, any box-drawing glyph `#`, anything else `?`).
- **`--render-frame`** (`render_frame_main`): renders `Screen::Play` of a fresh `Game::new(seed)` and writes
  its full-redraw bytes to stdout, with no termios call; the frame goldens freeze it.
- **The camera console.** On a terminal smaller than 80×30 the interactive view windows the grid:
  `split_rows` splits the height into map rows, one pinned status row and up to four log rows (the log stays
  full while the map keeps at least `CAM_MAP_FLOOR` (5) rows, then shrinks to one line, then the map yields);
  `scroll_axis` slides the map viewport after the player with a dead zone of `CAM_MARGIN` (4) cells, clamped
  to the map; `viewport` composes them, returns the full grid unchanged at 80×30 or more, and crops Title and
  End to the top-left corner. `Present` keeps the last frame and the viewport origin and redraws in full when
  the view's size (the terminal size clamped to 80×30) changes; a resize between two sizes both at least
  80×30 leaves the view at 80×30 and redraws only the changed cells.
- **Input** (`run`). Bytes are read one at a time: `wasd`, `hjkl` and the arrow keys move (0-3), `.` waits
  (4), `t` then a direction talks (7-10) and `g` then a direction gives (11-14) (a repeated `t` or `g`
  re-arms; any other byte cancels and logs "Talk cancelled." or "Give cancelled."), `u` pushes 15 with empty
  hands and otherwise logs the use menu and reads a digit for `17 + kind` ("Use cancelled." otherwise), `p`
  pushes 16, F1 (`ESC O P` or `ESC [ 1 1 ~`) logs the seed and world hash, F5 (`ESC [ 1 5 ~`) saves with the
  same overwrite confirmation as the window backend, and `q`, a lone Escape, Ctrl-C or the end of input quit.
  The Title, the End screen's `r`/`n`/`q`, the ghost writes and the quit autosave work as in the window
  backend, with these differences: on the End screen a lone Escape, Ctrl-C or the end of input quit like
  `q` and write the ghost of a dead attempt, where the window backend's Escape quits without one; the
  End screen shows on the same read as the death or win, with no one-frame `pending_end` delay (there is
  no flash); with no window title, a restart logs the seed and world hash instead.

## Tests
The inline `#[cfg(test)] mod tests` of `src/backend_term.rs`: `split_rows_full_console_is_full_map_and_log`,
`split_rows_classic_80x25_keeps_full_log_scrolls_map`, `split_rows_short_console_floors_map_then_shrinks_log`,
`scroll_axis_no_scroll_when_view_covers_map`, `scroll_axis_holds_inside_dead_zone`,
`scroll_axis_follows_when_focus_pushes_margin`, `scroll_axis_clamps_at_map_edges`,
`viewport_identity_at_full_size`, `viewport_clamps_oversized_terminal_to_grid`,
`viewport_80x25_scrolls_map_pins_status_keeps_log`, `viewport_non_play_crops_top_left`, `c256_vectors`,
`dirty_diff_single_cell`, `ascii_mode_no_high_bytes`, `ascii_glyph_fallback_mapping` and
`full_frame_deterministic`. They compile only with the terminal feature: run them with
`PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.75 cargo test --quiet --no-default-features --features
backend-term <name>` (`make test-term` runs them all). `make frames` builds this backend (`make build-term`,
`RUSTFLAGS="-D warnings"`) and compares `--render-frame` for seeds 1 and 42 and `--render-frame --seed 1
--ascii` with `tests/golden/frame_seed_1.bin`, `tests/golden/frame_seed_42.bin` and
`tests/golden/frame_seed_1_ascii.bin`.

## Changes
none: imported

## Gate
- **P03-AC-1** — `--render-frame --seed N` writes the full-redraw encoding of the Play screen of `Game::new(N)` for seeds 1 and 42, and `--ascii` its 7-bit form for seed 1: the clear-and-home prefix, cursor moves only where the cursor is not already at the next cell, and colour escapes only when the colour changes. *Verify:* `make frames` (target of `Makefile`, against `tests/golden/frame_seed_1.bin`, `tests/golden/frame_seed_42.bin`, `tests/golden/frame_seed_1_ascii.bin`).
- **P03-AC-2** — Against a previous frame, `frame_bytes` writes no cursor move and no clear for an unchanged grid, and one cursor move and no clear for a grid with one changed cell. *Verify:* `dirty_diff_single_cell` in `src/backend_term.rs`.
- **P03-AC-3** — `frame_bytes` returns the same bytes for the same input. *Verify:* `full_frame_deterministic` in `src/backend_term.rs`.
- **P03-AC-4** — With `ascii`, every byte written is below 0x80, and `ascii_fallback` maps U+2588 to `#`, U+2591 to `-`, box-drawing glyphs to `#` and any other code point to `?`. *Verify:* `ascii_mode_no_high_bytes` and `ascii_glyph_fallback_mapping` in `src/backend_term.rs`.
- **P03-AC-5** — `c256` maps black to 16, white to 231, pure red to 196 and 0x808080 to the grey ramp's 244. *Verify:* `c256_vectors` in `src/backend_term.rs`.
- **P03-AC-6** — `split_rows` gives (25, 4) at 30 rows or more, (20, 4) at 25, (5, 4) at 10, (5, 2) at 8, (5, 1) at 7, (4, 1) at 6, (1, 0) at 2 and (0, 0) at 1. *Verify:* `split_rows_full_console_is_full_map_and_log`, `split_rows_classic_80x25_keeps_full_log_scrolls_map` and `split_rows_short_console_floors_map_then_shrinks_log` in `src/backend_term.rs`.
- **P03-AC-7** — `scroll_axis` returns 0 when the view covers the map, holds the origin while the focus is inside the dead zone, follows the focus when it is pushed into the left or right margin, and clamps the origin to `[0, map - view]`. *Verify:* `scroll_axis_no_scroll_when_view_covers_map`, `scroll_axis_holds_inside_dead_zone`, `scroll_axis_follows_when_focus_pushes_margin` and `scroll_axis_clamps_at_map_edges` in `src/backend_term.rs`, at the tested positions: the tests pass the margin in as 4, so a margin changed by one inside `scroll_axis` fails them, but shifting only one edge's trigger by one cell passes them; `CAM_MARGIN` reaches `scroll_axis` only through `viewport_80x25_scrolls_map_pins_status_keeps_log`, which catches 4→3 but not 4→5 (the viewport's y-origin clamps at 5), so the value 4 of `CAM_MARGIN` is unpinned.
- **P03-AC-8** — `viewport` returns the full grid with origin (0, 0) at 80×30, clamps a larger terminal to 80×30, at 80×25 shows map rows from the scrolled origin, the status row `MAP_H` at row 20 and the newest log row last, and crops a non-Play screen to its top-left corner. *Verify:* `viewport_identity_at_full_size`, `viewport_clamps_oversized_terminal_to_grid`, `viewport_80x25_scrolls_map_pins_status_keeps_log` and `viewport_non_play_crops_top_left` in `src/backend_term.rs`.
- **P03-AC-9** — The terminal release build (`backend-term`) compiles with no warning on rustc 1.75. *Verify:* `make build-term` (target of `Makefile`, `RUSTFLAGS="-D warnings"`).
- **P03-AC-10** — The bytes read push the input bytes described in Purpose, a repeated `t` or `g` re-arms its chord, a cancelled chord or use logs its cancel line and pushes nothing, and the End screen's `r`, `n` and `q` restart, reroll and quit as in the window backend. *Verify:* unpinned (needs interactive playtest).
- **P03-AC-11** — Raw mode is entered and the terminal restored on a normal exit and on a panic; a change of the view's size (the terminal size clamped to 80×30) forces a full redraw. *Verify:* unpinned (needs interactive playtest).
- **P03-AC-12** — The file names no game noun as an identifier or a string literal: the use menu names items by glyph. *Verify:* unpinned (the `CLAUDE.md` grep rule, not a tracked check).

## Risks
- On rustc 1.100 the lint `suspicious_runtime_symbol_definitions` (a warning, on by default) fires on the
  `read` and `write` declarations at `src/backend_term.rs:56-57`, and the Makefile's `-D warnings` turns it
  into an error; the gate pins rustc 1.75, where they build. The owner
  made the lint a later node, not a gate change (`SPEC.md`, planned item 2).
- The `Termios` struct, the flag values and `TIOCGWINSZ` are Linux's layout (glibc and musl, 32- and 64-bit);
  another platform needs its own.
- `frame_bytes`'s doc comment says `ascii` writes `#` for every glyph of 128 and up; the code writes `-` for
  the light shade and `?` for an unknown code point (`ascii_fallback`).
- The frame goldens are this file's encoding of P01's grid: a change to either changes them, and `SPEC.md`'s
  open decisions require the owner's approval to re-baseline a named golden.
- The camera console, the key handling and the raw-mode restore can only be checked interactively; the
  terminal backend has no window-title, so a restart logs the world identity into the game log.
- The tests these criteria name are in `src/backend_term.rs`, which only the implementer may write until the
  tests move to `src/tests/*.rs` (`SPEC.md`'s open decisions).
