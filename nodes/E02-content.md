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
  `lore_line(seed, depth, tier)` fills the tier's lore template's `{A}` with the picked slot. Both are pure:
  `theme_for` serves live play (`Game::theme`, a portal's arrival and describe lines), the headless dump and
  the Title and End screens; `lore_line` serves `Game::lore_line` (lore pickups) and the Title screen's preview.
- `ghost_label_idx(outcome, final_depth)`: picks a ghost label without a channel, from a band of
  `GAME.ghost_labels` per outcome (four outcomes) and the final depth within the band.
- The `PAL_*` constants: the colours of the engine's own tiles and chrome (player, stairs, status, alert,
  lore, the log fade, the HP and torch bars and their empty part, the calm tint, portal, pit, goal, block,
  screen-link, hole and shut door). Theme wall and floor colours stay on `ThemeDef`; monster and item colours on their defs.

## Tests
Inline in `src/main.rs`, run with `PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.75 cargo test --quiet
<name>`: `golden_dumps` (each depth's header names its theme through `theme_for`), `theme_lines_fit_log_row`,
`ghost_labels_fit_16_bytes`, `ghost_bytes_round_trip`. The frame goldens `tests/golden/frame_seed_*.bin`
(`make frames`, terminal build) pin the 256-colour indices of the play screen at a fresh `Game::new`.

## Changes
none: imported

## Gate
- **E02-AC-1** — `theme_for` is a pure function of (seed, depth) on the `theme` channel: each golden seed's depths name the frozen themes. *Verify:* `golden_dumps` in `src/main.rs` (against `tests/golden/seed_*.txt`).
- **E02-AC-2** — Every lore template filled with every slot of its theme (the lines `lore_line` builds) fits the 78-column log row; the test fills the templates itself and does not call `lore_line`. *Verify:* `theme_lines_fit_log_row` in `src/main.rs`.
- **E02-AC-3** — `ghost_label_idx` returns, for one outcome and depth, an index inside `GAME.ghost_labels`, which the ghost file round-trips. *Verify:* `ghost_bytes_round_trip` in `src/main.rs` (one outcome and depth).
- **E02-AC-4** — `ghost_label_idx` stays inside `GAME.ghost_labels` for every outcome (clamped to 3) and every depth, given a non-empty table whose length is a multiple of four. *Verify:* unpinned (only one outcome and depth is exercised).
- **E02-AC-5** — On a fresh play screen, `PAL_PLAYER`, `PAL_STATUS`, `PAL_BAR_HP`, `PAL_BAR_TORCH` and the last three `PAL_LOG_FADE` entries map to the 256-colour indices of the frozen frames of seeds 1 and 42 and the ASCII frame of seed 1. The frames pin those indices, not the 24-bit values, and no other `PAL_*` colour. *Verify:* `make frames` (the `frames` target of `Makefile`, against `tests/golden/frame_seed_1.bin`, `tests/golden/frame_seed_42.bin`, `tests/golden/frame_seed_1_ascii.bin`).
- **E02-AC-6** — The file holds no game fact: no game noun as identifier or string, no `cfg` block, no platform call. *Verify:* unpinned (the `CLAUDE.md` grep rule, not a tracked check).

## Risks
- The `theme` channel's tag scheme and draw order (one theme draw, then three slot draws in `0..4`) fix every
  seed's themes and lore lines, so changing them is a MAJOR change (`CLAUDE.md`). The goldens show only the
  theme label, the first draw: a change to the slot draws alone changes the lore lines and diffs no golden.
- The slot range `0..4` is a literal tied to the four slots per theme that `ThemeDef` fixes; the two must
  change together.
- `ghost_label_idx` divides by four and relies on `GAME.ghost_labels.len()` being a non-zero multiple of four
  (an empty table makes it compute `final_depth % 0` and panic), which `gamedef.rs` documents but no test
  checks.
