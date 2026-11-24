# P01 — render

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | E02, E04, C02 | M01, M05, P02, P03 | M |

**Owns:** `src/render.rs`, the core cell composer: the 80×30 `Cell` grid and the derived `scene()`.

## Purpose
`src/render.rs` is the core half of presentation: it turns a `Game` into what the world looks like, and the
backends decide how to draw it. It is a core module: no platform call and no `cfg` outside its test module.
- **The grid.** `Cell { ch: u16, fg: u32, bg: u32 }` is one glyph and its colours (`bg` is black everywhere
  today); `CELLS` is `COLS * ROWS` (80×30). `render_cells(g, screen, cells)` blanks the grid and composes one
  of the three `Screen`s: `Title`, `Play` or `End`. Which screen is active is the backends' state machine.
- **The Play screen** (`render_play`): rows `[0, MAP_H)` are the map, row `MAP_H` the status row, the rows
  below it the log. Only `seen` tiles are drawn: walls autotile through `wall_mask` (a 4-bit mask, N=1, S=2,
  W=4, E=8, counting a neighbour only when it is in the map, seen and a wall) into the box-drawing
  `WALL_GLYPHS`; floor `.`, stairs `>` and `<`, portal `*`, pit `^`, goal `x`, and the overworld's screen link
  `=`, hole `V` and shut door `+` take their `PAL_*` or theme colour. A visible cell is graded by
  `light_pct(g.lit_r[i])`, the strongest light source reaching that tile (torch or the McGuffin's shine), not
  by the frame's torch; a seen but not visible cell is drawn `dim` (a quarter). Over the map, in this order:
  the last-death echo (a dim `@` at 35% when `g.echo` is on the current depth and its tile is seen), the
  visible items, the visible push blocks `B` (over items), the visible monsters (`PAL_CALM_TINT` in place of
  the kind's colour when becalmed), then the player `@`, each graded by the light at its own tile.
  `draw_status` writes the HP bar, the resource bar (`GAME.strings.resource_label`, filled against
  `start_light()`), ATK, depth, kills and `[&]` while the objective is carried, with bars of full-block and
  light-shade glyphs (alert colour at a quarter of HP or a torch radius of 4 or less). The log shows the last
  four messages, older ones faded through `PAL_LOG_FADE`.
- **Title and End.** `render_title` draws a bordered panel with the name `rl144`, the depth-1 theme's label
  and a lore line (`theme_for`, `lore_line`), the seed, the key legend and "press any key". `render_end` draws
  `YOU WON` with the objective's name, or `YOU DIED` with the cause (the killer's name, a darkness death, or
  a darkness death while her light was elsewhere, `died_out_of_her_light`), then depth, kills, turns, light
  and seed, and the retry, new-world and quit keys.
- **Helpers.** `scale(c, pct)` scales each RGB channel by `pct` percent; `light_pct(radius)` reads
  `GAME.balance.light_tiers` (an exact match, else the table's last entry).
- **`scene()`**, the additive sprite-level surface: the player (always), each visible monster and item and the
  echo (under the same three conditions as the Play screen's echo), each a `SceneEntity` with a `SpriteKind`,
  its position, a `Facing`, an `anim_phase` of `(turns + x + y) mod 4` and its tile's `light_pct`. A monster
  faces the player when `Game::monster_sees_player`, else `S`. Everything is derived from `Game` and nothing is
  hashed, saved or dumped. `scene`, `SpriteKind`, `SceneEntity`, `monster_facing` and `anim_phase` are
  `#[allow(dead_code)]`: no backend calls them yet (sprites are post-1.0).
- **`Facing`** and `Facing::from_delta` (dx before dy, `(0,0)` reads `S`) are also what `src/game.rs` stores
  in `Game::facing`, a presentation-only field.

## Tests
The inline `#[cfg(test)] mod tests` of `src/render.rs`: `wall_mask_representative`, `status_row_fits_80_cols`,
`scene_is_deterministic`, `echo_appears_only_under_three_conditions`, `facing_updates_on_move`,
`becalmed_monster_uses_calm_tint`, `mcguffin_shine_renders_at_her_brightness_not_the_dying_torch` and
`title_legend_fits_78_cols`; `scale_color_channels` in `src/main.rs`. Run them with
`PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.75 cargo test --quiet <name>`. The frame goldens
`tests/golden/frame_seed_1.bin`, `tests/golden/frame_seed_42.bin` and `tests/golden/frame_seed_1_ascii.bin`
(`make frames`, through the terminal backend's `--render-frame`) freeze the Play screen of a fresh
`Game::new` for seeds 1 and 42. No test renders the Title or End screen.

## Changes
none: imported

## Gate
- **P01-AC-1** — For a fresh `Game::new` of seeds 1 and 42, `render_cells` with `Screen::Play` produces the frozen frames: the seen map's glyphs and the 256-colour index each colour maps to (floor, wall autotiles, the player `@`), the status row's text and bars, and the log lines at row `MAP_H + 1` from column 1 with their `PAL_LOG_FADE` shades. The frames pin those indices, not the 24-bit values. *Verify:* `make frames` (target of `Makefile`, against `tests/golden/frame_seed_1.bin`, `tests/golden/frame_seed_42.bin`, `tests/golden/frame_seed_1_ascii.bin`).
- **P01-AC-2** — `wall_mask` sets N=1, S=2, W=4, E=8 and counts a neighbour only when it is seen and a wall; `WALL_GLYPHS` maps the isolated, N|S, W|E, N|E, N|S|W and all-four masks to U+2500, U+2502, U+2500, U+2514, U+2524 and U+253C. *Verify:* `wall_mask_representative` in `src/render.rs`.
- **P01-AC-3** — A visible tile is graded by the light at its own tile (`light_pct(g.lit_r[i])`): a tile lit only by the carried McGuffin's shine past a dying torch renders at her brightness, not the torch's. *Verify:* `mcguffin_shine_renders_at_her_brightness_not_the_dying_torch` in `src/render.rs`.
- **P01-AC-4** — A becalmed monster renders in `PAL_CALM_TINT` exactly, and a hostile one in its kind's own colour. *Verify:* `becalmed_monster_uses_calm_tint` in `src/render.rs`.
- **P01-AC-5** — The status row ends at or before column 80 with 40/40 HP, 2,000 light, ATK 99, the deepest depth, 9,999 kills and the objective carried. *Verify:* `status_row_fits_80_cols` in `src/render.rs`.
- **P01-AC-6** — `scale` multiplies each RGB channel by `pct` / 100, truncating. *Verify:* `scale_color_channels` in `src/main.rs`.
- **P01-AC-7** — `scene()` returns equal vectors on two calls with no change to the game between them. *Verify:* `scene_is_deterministic` in `src/render.rs`.
- **P01-AC-8** — `scene()` includes the echo entity only when `g.echo` is set, its depth is the current depth and its tile is seen. *Verify:* `echo_appears_only_under_three_conditions` in `src/render.rs`.
- **P01-AC-9** — `Game::facing` starts at `S`, and after a move north from seed 1's spawn faces `N` through `Facing::from_delta`. *Verify:* `facing_updates_on_move` in `src/render.rs`.
- **P01-AC-10** — A seen tile that is not visible is drawn at a quarter brightness (`dim`). *Verify:* unpinned (a fresh game's frames have no such tile).
- **P01-AC-11** — The Play screen's echo is a dim `@` at 35%, drawn under items, blocks, monsters and the player; blocks are drawn over items; the HP bar and the resource bar switch to `PAL_ALERT` at a quarter of max HP and at a torch radius of 4 or less; `scene()` lists every visible monster and item with the facing and `anim_phase` described above. *Verify:* unpinned.
- **P01-AC-12** — The Title screen shows the name, the depth-1 theme label and lore line, the seed, the key legend and "press any key"; the End screen shows the win or the death cause and the run's numbers; the key-legend lines fit 78 columns. *Verify:* unpinned (`title_legend_fits_78_cols` measures its own copies of the lines, not what `render_title` draws).
- **P01-AC-13** — The file has no platform call, no `cfg` outside its test module, and no game noun as an identifier; its only game-specific string literals are the Title and End screens' lines named in Risks (one death-cause line speaks of "her light"). *Verify:* unpinned (the `CLAUDE.md` grep rule, not a tracked check).
- **P01-AC-14** — After a move south, west or east, `Game::facing` faces `S`, `W` or `E`; `Facing::from_delta` reads `dx` before `dy` and `(0, 0)` as `S`. *Verify:* unpinned (`facing_updates_on_move` moves only north from seed 1's spawn; a `from_delta` that gives `W` for `dx > 0` or `N` for `dy > 0` passes it).

## Risks
- `src/render.rs` and `src/game.rs` import each other: `render` reads `Game`, and `game` stores
  `render::Facing` in `Game::facing`. This node lists E04 in its Deps (E04's Unlocks lists this node, and
  E04's Risks names the `render::Facing` import), but E04's Deps does not list this node: adding it would
  make a cycle in the graph, which `check-regime.py` rejects.
- The frame goldens freeze only a fresh `Game::new` on the Play screen at full torch, where every visible
  tile's `light_pct` is 100. The per-tile light grading went wrong once without any golden failing (the
  batch 14 render fix and its regression test); the Title and End screens and every mid-run state are
  covered by no golden.
- A change to what the Play screen draws at turn 0 changes the frame goldens, which the imported criteria
  of E02 and this node name: `SPEC.md`'s open decisions require the owner's approval to re-baseline them.
- The Title and End screens hold their strings as literals here (the name `rl144`, the key legend, the
  death-cause lines, one of which speaks of "her light"), not in `GAME.strings`. `CLAUDE.md` asks that the
  title's key legend change in the same commit as any key it documents.
- `Cell::ch` is a `u16` code point: the box-drawing and block glyphs (U+2500 to U+259F) go through it, but
  `put_str` writes strings byte by byte, so every string drawn must be ASCII (`CLAUDE.md`).
- The tests these criteria name are in `src/render.rs` and `src/main.rs`, which only the implementer may write
  until the tests move to `src/tests/*.rs` (`SPEC.md`'s open decisions).
