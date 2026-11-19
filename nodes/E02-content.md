# E02 — content

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | E01, E03 | E04 | S |

**Owns:** `src/content.rs`, the generic content helpers and the engine's UI-chrome palette.

## Purpose
`src/content.rs` holds small algorithms that read the active cartridge (`crate::games::GAME`, its one seam)
but hold no game fact of their own, plus the engine's own colours:
- `theme_pick(seed, depth)`: the theme index and one slot index per lore tier, drawn on the per-depth `theme`
  channel (so flavor never perturbs worldgen or spawns); `theme_for(seed, depth)` returns that `ThemeDef`, and
  `lore_line(seed, depth, tier)` fills the tier's lore template's `{A}` with the picked slot. Both are pure
  functions shared by live play (`Game::theme`, `Game::lore_line`), the headless dump and the Title/End
  screens.
- `ghost_label_idx(outcome, final_depth)`: picks a ghost label without a channel, from a band of
  `GAME.ghost_labels` per outcome (four outcomes) and the final depth within the band.
- The `PAL_*` constants: the colours of the engine's own tiles and chrome (player, stairs, status, alert,
  lore, the log fade, the HP and torch bars, the calm tint, portal, pit, goal, block, screen-link, hole and
  shut door). Theme wall and floor colours stay on `ThemeDef`; monster and item colours on their defs.

## Tests
Inline in `src/main.rs`, run with `PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.75 cargo test --quiet
<name>`: `golden_dumps` (each depth's header names its theme through `theme_for`), `theme_lines_fit_log_row`,
`ghost_labels_fit_16_bytes`, `ghost_bytes_round_trip`. The frame goldens `tests/golden/frame_seed_*.bin`
(`make frames`, terminal build) pin the play screen's colours at a fresh `Game::new`.

## Changes
none: imported

## Gate
- **E02-AC-1** — `theme_for` is a pure function of (seed, depth) on the `theme` channel: each golden seed's depths name the frozen themes. *Verify:* `golden_dumps` in `src/main.rs` (against `tests/golden/seed_*.txt`).
- **E02-AC-2** — Every lore line `lore_line` can produce (each theme, each tier, each slot) fits the 78-column log row. *Verify:* `theme_lines_fit_log_row` in `src/main.rs`.
- **E02-AC-3** — `ghost_label_idx` returns an index whose label round-trips through the ghost file format. *Verify:* `ghost_bytes_round_trip` in `src/main.rs` (one outcome and depth).
- **E02-AC-4** — `ghost_label_idx` stays inside `GAME.ghost_labels` for every outcome (clamped to 3) and every depth, given a table whose length is a multiple of four. *Verify:* unpinned (only one outcome and depth is exercised).
- **E02-AC-5** — The `PAL_*` values that appear on a fresh play screen (player, stairs, status text, bars, log) render the frozen frames of seeds 1 and 42 and the ASCII frame of seed 1; the Title and End screens' colours and lore preview are not frame-pinned. *Verify:* `make frames` (the `frames` target of `Makefile`, against `tests/golden/frame_seed_1.bin`, `tests/golden/frame_seed_42.bin`, `tests/golden/frame_seed_1_ascii.bin`).
- **E02-AC-6** — The file holds no game fact: no game noun as identifier or string, no `cfg` block, no platform call. *Verify:* unpinned (the `CLAUDE.md` grep rule, not a tracked check).

## Risks
- The `theme` channel's tag scheme and draw order (one theme draw, then three slot draws in `0..4`) are part of
  the frozen worldgen output; changing them diffs the goldens (MAJOR).
- The slot range `0..4` is a literal tied to the four slots per theme that `ThemeDef` fixes; the two must
  change together.
- `ghost_label_idx` divides by four and relies on `GAME.ghost_labels.len()` being a multiple of four, which
  `gamedef.rs` documents but no test checks.
