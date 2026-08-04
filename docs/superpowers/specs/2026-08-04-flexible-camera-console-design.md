# Design: flexible camera console (backend-term)

**Status: DESIGN — awaiting human approval before any implementation.** This is the
brainstorming output for the one remaining rl144-side deliverable from the 486-boot scope
(`docs/design/486/2026-08-04-boot-on-a-486-scope.md`). Written as an approval-ready package
(decisions as recommendations-with-defaults) because it was drafted autonomously; the three
real forks below are genuinely the human's call. **No code has been written.**

## The problem (grounded in the code)

`backend_term.rs` renders a fixed grid: `ROWS=30`, `COLS=80`, with `MAP_H=25` — map rows
0–24, status row 25, log rows 26–29 (`game.rs:34`). `frame_bytes` (`backend_term.rs:447`)
walks all 80×30 cells and emits dirty-cell ANSI at 1-based `\x1b[row;colH` positions. It
assumes the console is *at least* 80×30. On a real 486 text console (often 80×25, sometimes
80×50 or 40-col), rows/cols past the console wrap or scroll and the frame corrupts.

The dungeon map is itself a fixed **80×25** (`COLS`×`MAP_H`) — the world *is* the grid,
never larger — so "a world bigger than the screen" bites precisely when **the console is
smaller than 80×30**. Rather than force a boot video mode (the factory's lever), the game
should adapt to whatever console it's given.

## Scope boundary

- **In scope:** `backend_term.rs` only. Presentation. A viewport/camera between
  `render_cells` (fills the full 80×30 buffer) and the ANSI emitter.
- **Out of scope:** the engine grid stays 80×30 — it is engine API and MAJOR-frozen
  (`CLAUDE.md`: "The 80×30 grid is engine API"). No worldgen, no hashed state, no save
  format. `backend-minifb` (graphical/X11) is untouched — it's not on the disc.
- **Frozen invariant:** `--render-frame` (`render_frame_main`, `backend_term.rs:521`) must
  keep emitting the fixed full 80×30 frame so `tests/golden/frame_*.bin` never move. The
  camera path is **interactive-only**, gated on an actual `TIOCGWINSZ` query; the golden
  path never queries winsize. This keeps goldens / xhash / solve / sim byte-identical.

## Architecture

One new pure function does the geometry; everything else is plumbing:

```
render_cells(&game, screen, &mut cells)   // unchanged: fills the full 80x30 buffer
        |
        v
viewport(full_cells, term_w, term_h, focus, prev_origin) -> (windowed_cells, geom)
        |                                                    // NEW pure fn, unit-tested
        v
frame_bytes_windowed(windowed_cells, prev, ascii, geom)    // frame_bytes, viewport-aware
        |
        v
raw_write(bytes)
```

`viewport` is a pure function of (buffer, terminal size, camera focus, previous origin) →
(visible cells + geometry). Pure ⇒ exhaustively unit-testable with zero I/O, which is how
this feature earns its verification (a TTY can't be golden-tested headlessly).

## The three decisions (recommendations first)

### D1 — Camera focus source
The camera centers on the player. The player cell is needed by the backend.
- **(a) RECOMMENDED: a one-line derived accessor `Game::camera_focus() -> (i32, i32)`**
  returning `(self.px, self.py)`. Additive to the core/crust surface, explicit, doctrine-
  clean (documented in a status entry as a justified new backend-facing API, per
  `CLAUDE.md` "A backend needing a new core API is a real design decision").
- (b) Pull it out of `scene()`'s always-present player entity — zero new API, but the term
  backend doesn't currently consume `scene()`, and digging a coordinate out of a
  `Vec<SceneEntity>` is more code than (a).
- (c) Read `game.px/py` directly — they're already `pub(crate)` and it works, but it bends
  the "backends only consume the named surface" doctrine. Rejected.

### D2 — Layout when the console is smaller than 80×30
The 80×30 buffer is map(0–24) + status(25) + log(26–29). When rows are scarce:
- **(a) RECOMMENDED: pin status, keep as much log as fits, give the rest to a scrolling
  map viewport.** You always see HP/light (status) and the latest log line; the map is what
  scrolls. Allocation: 1 row status (always) + `min(4, remaining-after-map-floor)` log rows
  + the rest to the map camera, with a map floor (e.g. ≥5 rows) below which we accept a
  1-row log. Degrade gracefully under ~8 rows (status + 1 log + tiny map).
- (b) Crop the whole 80×30 uniformly and let status/log scroll off — simpler, but losing
  the status line on a small console is unacceptable. Rejected.
- (c) Reflow status/log into the map margins — most work, deferred.

Horizontal (< 80 cols): crop the map viewport in x (camera follows in x too) and truncate
status/log text to the console width. `put_str`-style clipping already implies truncation.

### D3 — Scroll feel ("kick around as you'd expect")
- **(a) RECOMMENDED target: dead-zone (margin) scrolling.** The player moves freely inside
  a central dead-zone; the viewport only scrolls when the player pushes into the margin,
  then clamps at map edges. This is the classic "world bigger than the screen" feel your
  phrasing describes. Needs the viewport origin to persist frame-to-frame — a few ints of
  **backend-local presentation state**, which is doctrine-clean (`CLAUDE.md`: "Chord/selector
  armed state is frontend-local chrome, never engine state" — same category).
- (b) Center-on-player with edge clamp — trivial, stateless, but scrolls on nearly every
  step (jittery near center). Good enough as a v1 fallback if (a) proves fiddly.

Recommendation: build the pure `viewport` fn to take `prev_origin` so (a) is the default and
(b) is just "ignore prev_origin" — one function, both behaviors, decided by a constant.

## When the console is ≥ 80×30
Show the full grid exactly as today — **zero behavior change for normal terminals.** The
viewport transform is identity when `term_w ≥ 80 && term_h ≥ 30`. (tmux, xterm, CI, the
QEMU serial console at 80×30 all keep today's output.)

## Terminal-size acquisition
Add `TIOCGWINSZ` via the existing libc FFI pattern (one more `ioctl` alongside the termios
externs). Query it **each present** (a cheap ioctl) rather than installing a SIGWINCH
handler — resize is handled for free with no signal-safety complexity. Fall back to 80×30 if
the ioctl fails or reports 0 (piped/non-tty), which also keeps any non-tty path at today's
behavior.

## Verification plan
- **Unit tests on `viewport`** (pure fn): identity at ≥80×30; correct crop rect and origin
  for representative small sizes (80×25, 40×25, 24×80, 8-row degrade); dead-zone hysteresis
  (player crossing the margin scrolls, inside it doesn't); edge clamping (never shows past
  map bounds); status/log allocation at each size. Tests are free — they don't ship.
- **Goldens frozen:** `--render-frame` stays full 80×30; `make check` (goldens, frames,
  xhash, solve, sim, flip, size, msrv) must come back **byte-identical** — the whole feature
  is presentation and touches nothing hashed. That identical `make check` is the sign-off
  line.
- **Needs interactive playtest** (flag, per doctrine — window-path can't be headlessly
  verified): actually resize a terminal / boot the QEMU console at 80×25 and confirm the
  camera follows and status/log stay legible.

## Size
Negligible: one pure function, one ioctl, a windowed variant of an existing emitter. No new
deps, no new tables. Well under any threshold; report the delta at implementation.

## Explicitly NOT decided here (needs your nod)
1. Approve D1(a) / D2(a) / D3(a) as defaults, or redirect.
2. Whether a genuine 486SX (soft-float) matters — orthogonal to this feature, tracked in the
   486 brief, but it's the same "how small/old do we really target" question.

On approval I'll invoke `writing-plans` and TDD it (pure `viewport` fn first, red→green).
Until then this stays design-only.
