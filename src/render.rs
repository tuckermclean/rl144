// render.rs — core cell composer: walks Game state into an 80x30 grid of
// Cell {ch, fg, bg}. This is a CORE module: zero platform calls, zero cfg,
// no font8x8. Cells are the natural unit both for a terminal backend
// (dirty-cell diffing is cheap on a Cell grid, expensive on raw pixels) and
// for a pixel backend (rasterizing a Cell into an 8x12 glyph rect is a
// mechanical, backend-local concern). Keeping composition here and
// rasterization in the backend is the core/crust seam: this module answers
// "what does the world look like," backends answer "how do I draw that."

use crate::content::{
    PAL_ALERT, PAL_AMULET, PAL_BAR_EMPTY, PAL_BAR_HP, PAL_BAR_TORCH, PAL_LOG_FADE, PAL_LORE,
    PAL_PLAYER, PAL_POTION, PAL_STAIRS, PAL_STATUS, PAL_SWORD, lore_line, theme_for,
};
use crate::game::{
    COLS, Game, IKind, MAP_H, MAX_DEPTH, Monster, ROWS, START_LIGHT, Tile, fov_radius, idx,
    in_map,
};

/// A single terminal-style cell: one glyph plus its foreground/background
/// color. `ch` is a Unicode BMP codepoint (u16): most glyphs are ASCII
/// (<128), but wall autotiling and the status bars use box-drawing/block
/// glyphs in the U+2500..=U+259F range. `bg` is 0x000000 (black) everywhere
/// today — no cell currently paints a background — but backends should
/// still honor it rather than assuming black, since that's the whole point
/// of carrying it separately from fg.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Cell {
    pub(crate) ch: u16,
    pub(crate) fg: u32,
    pub(crate) bg: u32,
}

/// Total cells in the fixed 80x30 grid; `render_cells` expects a slice of
/// exactly this length.
pub(crate) const CELLS: usize = COLS * ROWS;

const BLANK: Cell = Cell { ch: b' ' as u16, fg: 0, bg: 0 };

/// Which top-level screen `render_cells` composes. Play is the original
/// (and, pre-task-5, only) map rendering; Title and End are new bookends
/// around a run. `--render-frame` always renders Play regardless of where
/// a live session would actually be (see backend_term::render_frame_main)
/// — that's the map-view surface the frame goldens freeze.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Screen {
    Title,
    Play,
    End,
}

fn dim(c: u32) -> u32 {
    (c >> 2) & 0x3F3F3F
}

/// Scale each RGB channel of `c` by `pct` percent (0..=100), independently.
/// Used to dim visible tiles/items/monsters as the torch (light) burns down.
pub(crate) fn scale(c: u32, pct: u32) -> u32 {
    let r = (c >> 16) & 0xFF;
    let g = (c >> 8) & 0xFF;
    let b = c & 0xFF;
    let r = r * pct / 100;
    let g = g * pct / 100;
    let b = b * pct / 100;
    (r << 16) | (g << 8) | b
}

/// Brightness percentage for the current FOV radius: the torch burning down
/// shrinks the radius (see `fov_radius`) and dims what's still visible. The
/// last two tiers were deepened (was 50/40) so the closing dark reads as
/// oppressive rather than merely dim.
fn light_pct(radius: i32) -> u32 {
    match radius {
        8 => 100,
        6 => 85,
        5 => 70,
        4 => 55,
        3 => 40,
        _ => 28,
    }
}

/// Wall autotile table: 4-bit neighbor mask (N=1, S=2, W=4, E=8) -> a
/// single-line box-drawing codepoint that connects toward exactly those
/// neighbors. Index 0 (isolated wall cell) and 12 (W|E, a horizontal
/// corridor wall) both land on the plain horizontal glyph; every other
/// index is a corner, tee, or the full cross at 15.
const WALL_GLYPHS: [u16; 16] = [
    0x2500, 0x2502, 0x2502, 0x2502, 0x2500, 0x2518, 0x2510, 0x2524, 0x2500, 0x2514, 0x250C,
    0x251C, 0x2500, 0x2534, 0x252C, 0x253C,
];

/// Neighbor mask for wall autotiling at map cell (x, y). A neighbor counts
/// ONLY if `in_map` AND `seen` AND `Tile::Wall` — unseen topology must
/// never leak through wall shapes (a wall the player hasn't discovered yet
/// must not silently round a corner), and out-of-map always counts as
/// not-wall. This function is presentation-only: `--dump` (headless.rs's
/// `level_dump`) never calls it, so dump goldens are untouched by
/// anything in this file.
fn wall_mask(g: &Game, x: i32, y: i32) -> usize {
    let is_wall = |dx: i32, dy: i32| -> bool {
        let (nx, ny) = (x + dx, y + dy);
        in_map(nx, ny) && g.seen[idx(nx, ny)] && g.map[idx(nx, ny)] == Tile::Wall
    };
    (is_wall(0, -1) as usize)
        | (is_wall(0, 1) as usize) << 1
        | (is_wall(-1, 0) as usize) << 2
        | (is_wall(1, 0) as usize) << 3
}

fn put(cells: &mut [Cell], col: usize, row: usize, ch: u16, fg: u32) {
    if col < COLS && row < ROWS {
        cells[row * COLS + col] = Cell { ch, fg, bg: 0 };
    }
}

/// Write `s` (ASCII only) starting at `col`. Returns the column one past
/// the last character written, so callers composing a line from several
/// differently-colored segments (see `draw_status`) can chain calls.
/// Callers that don't need the chain (most call sites) simply ignore it.
fn put_str(cells: &mut [Cell], col: usize, row: usize, s: &str, fg: u32) -> usize {
    for (i, ch) in s.bytes().enumerate() {
        put(cells, col + i, row, ch as u16, fg);
    }
    col + s.len()
}

fn center_col(len: usize) -> usize {
    (COLS.saturating_sub(len)) / 2
}

fn put_centered(cells: &mut [Cell], row: usize, s: &str, fg: u32) {
    put_str(cells, center_col(s.len()), row, s, fg);
}

/// Draw a `total`-wide status bar starting at `col`: `filled` cells use the
/// full-block glyph (0x2588) in `fill_fg`; the remainder use the light-
/// shade glyph (0x2591) in PAL_BAR_EMPTY. Returns the column one past the
/// bar (see `put_str`'s doc comment — same chaining convention).
fn put_bar(cells: &mut [Cell], col: usize, row: usize, filled: usize, total: usize, fill_fg: u32) -> usize {
    for i in 0..total {
        let (ch, fg) = if i < filled { (0x2588u16, fill_fg) } else { (0x2591u16, PAL_BAR_EMPTY) };
        put(cells, col + i, row, ch, fg);
    }
    col + total
}

/// How many of `total` bar cells should read as "filled" for `value/max`,
/// rounded to nearest and clamped to `[0, total]`. `max <= 0` reads as
/// empty (guards the division; doesn't occur in practice since maxhp and
/// START_LIGHT are always positive).
fn bar_fill(value: i32, max: i32, total: usize) -> usize {
    if max <= 0 {
        return 0;
    }
    let v = value.clamp(0, max) as i64;
    let t = total as i64;
    ((v * t + max as i64 / 2) / max as i64).clamp(0, t) as usize
}

/// Draw the labeled status row (HP bar, Torch bar, ATK, depth, kills, the
/// `[&]` amulet flag) starting at column 1 of `row`. Returns the column one
/// past the last cell written — doubles as the "does this fit 80 cols"
/// measurement the `status_row_fits_80_cols` unit test uses directly,
/// rather than maintaining a second plain-text formula that could drift
/// from what's actually drawn.
fn draw_status(
    cells: &mut [Cell],
    row: usize,
    hp: i32,
    maxhp: i32,
    light: i32,
    radius: i32,
    atk: i32,
    depth: u32,
    kills: u32,
    carrying: bool,
) -> usize {
    let mut col = put_str(cells, 1, row, "HP [", PAL_STATUS);
    let hp_fg = if hp <= maxhp / 4 { PAL_ALERT } else { PAL_BAR_HP };
    col = put_bar(cells, col, row, bar_fill(hp, maxhp, 10), 10, hp_fg);
    col = put_str(cells, col, row, &format!("] {}/{}  Torch [", hp, maxhp), PAL_STATUS);
    let torch_fg = if radius <= 4 { PAL_ALERT } else { PAL_BAR_TORCH };
    col = put_bar(cells, col, row, bar_fill(light, START_LIGHT, 10), 10, torch_fg);
    col = put_str(
        cells,
        col,
        row,
        &format!("]  ATK {}  D{}/{}  K{}", atk, depth, MAX_DEPTH, kills),
        PAL_STATUS,
    );
    if carrying {
        col = put_str(cells, col, row, "  [&]", PAL_STATUS);
    }
    col
}

// ---------- Title/End panel chrome ----------

const BOX_H: u16 = 0x2500;
const BOX_V: u16 = 0x2502;
const BOX_TL: u16 = 0x250C;
const BOX_TR: u16 = 0x2510;
const BOX_BL: u16 = 0x2514;
const BOX_BR: u16 = 0x2518;

const PANEL_X0: usize = 2;
const PANEL_Y0: usize = 1;
const PANEL_W: usize = COLS - 2 * PANEL_X0;
const PANEL_H: usize = ROWS - 2 * PANEL_Y0;

/// Single-line box-drawing border for a Title/End panel, top-left
/// (x0, y0) to bottom-right (x0+w-1, y0+h-1) inclusive.
fn draw_border(cells: &mut [Cell], x0: usize, y0: usize, w: usize, h: usize, fg: u32) {
    for x in x0..x0 + w {
        put(cells, x, y0, BOX_H, fg);
        put(cells, x, y0 + h - 1, BOX_H, fg);
    }
    for y in y0..y0 + h {
        put(cells, x0, y, BOX_V, fg);
        put(cells, x0 + w - 1, y, BOX_V, fg);
    }
    put(cells, x0, y0, BOX_TL, fg);
    put(cells, x0 + w - 1, y0, BOX_TR, fg);
    put(cells, x0, y0 + h - 1, BOX_BL, fg);
    put(cells, x0 + w - 1, y0 + h - 1, BOX_BR, fg);
}

/// Compose the current frame as an 80x30 grid of cells. `cells` must be
/// exactly `CELLS` long.
pub(crate) fn render_cells(g: &Game, screen: Screen, cells: &mut [Cell]) {
    cells.iter_mut().for_each(|c| *c = BLANK);
    match screen {
        Screen::Title => render_title(g, cells),
        Screen::Play => render_play(g, cells),
        Screen::End => render_end(g, cells),
    }
}

/// The map view: same glyphs-and-colors composition as before task 5, plus
/// wall autotiling, the deepened low-light grading, the gutter-dim on the
/// player glyph, and the bar-based status row.
fn render_play(g: &Game, cells: &mut [Cell]) {
    let theme = g.theme();
    // Brightness percentage for currently-visible tiles/items/monsters only;
    // seen-but-not-visible tiles keep the existing dim() treatment instead
    // (memory stays legible; the dark closes in on what's currently seen).
    let radius = fov_radius(g.light);
    let pct = light_pct(radius);
    // map
    for y in 0..MAP_H as i32 {
        for x in 0..COLS as i32 {
            let i = idx(x, y);
            if !g.seen[i] {
                continue;
            }
            let (ch, color): (u16, u32) = match g.map[i] {
                Tile::Wall => (WALL_GLYPHS[wall_mask(g, x, y)], theme.wall),
                Tile::Floor => (b'.' as u16, theme.floor),
                Tile::Stairs => (b'>' as u16, PAL_STAIRS),
                Tile::UpStairs => (b'<' as u16, PAL_STAIRS),
            };
            let c = if g.vis[i] { scale(color, pct) } else { dim(color) };
            put(cells, x as usize, y as usize, ch, c);
        }
    }
    // items (visible only)
    for it in &g.items {
        if g.vis[idx(it.x, it.y)] {
            let (ch, c) = match it.kind {
                IKind::Potion => (b'!', PAL_POTION),
                IKind::Sword => (b')', PAL_SWORD),
                IKind::Amulet => (b'&', PAL_AMULET),
                IKind::LoreA | IKind::LoreB | IKind::LoreC => (b'?', PAL_LORE),
            };
            put(cells, it.x as usize, it.y as usize, ch as u16, scale(c, pct));
        }
    }
    // monsters (visible only)
    for m in &g.monsters {
        if g.vis[idx(m.x, m.y)] {
            let (_, _, ch, c, _) = Monster::stats(m.kind);
            put(cells, m.x as usize, m.y as usize, ch as u16, scale(c, pct));
        }
    }
    // player — the torch itself gutters at the lowest radius (dim to 85%);
    // otherwise it's always full brightness, since it IS the light source.
    let player_fg = if radius <= 3 { scale(PAL_PLAYER, 85) } else { PAL_PLAYER };
    put(cells, g.px as usize, g.py as usize, b'@' as u16, player_fg);

    // status: labeled HP/Torch bars.
    draw_status(cells, MAP_H, g.hp, g.maxhp, g.light, radius, g.atk, g.depth, g.kills, g.has_amulet);

    // log: last 4, older lines faded
    let n = g.msgs.len();
    let recent = &g.msgs[n.saturating_sub(4)..];
    for (r, m) in recent.iter().enumerate() {
        let shade = PAL_LOG_FADE[PAL_LOG_FADE.len() - recent.len() + r];
        put_str(cells, 1, MAP_H + 1 + r, m, shade);
    }
}

/// The launch screen: game name, the depth-1 theme's identity (label + a
/// filled tier-0 lore line — both pure f(seed), no RNG draws: theme_pick's
/// channel draw is the only randomness involved, same as live play), the
/// world's seed, and the input legend. Shown until the player presses any
/// key (see backend_minifb::run / backend_term::run), unless the run was
/// resumed via `--load` (which starts straight on Play).
fn render_title(g: &Game, cells: &mut [Cell]) {
    draw_border(cells, PANEL_X0, PANEL_Y0, PANEL_W, PANEL_H, PAL_STATUS);
    let theme = theme_for(g.seed, 1);
    let mut row = PANEL_Y0 + 2;
    put_centered(cells, row, "rl144", PAL_PLAYER);
    row += 2;
    put_centered(cells, row, &format!("Depth 1: {}", theme.label), PAL_STATUS);
    row += 1;
    put_centered(cells, row, &lore_line(g.seed, 1, 0), PAL_LORE);
    row += 2;
    put_centered(cells, row, &format!("seed {}", g.seed), PAL_STATUS);
    row += 2;
    put_centered(cells, row, "Move: arrows / wasd / hjkl    Wait: .", PAL_STATUS);
    row += 1;
    put_centered(cells, row, "Save: F5    World info: F1    Quit: q", PAL_STATUS);
    row += 2;
    put_centered(cells, row, "press any key", PAL_ALERT);
}

/// The results screen: win or death cause, then the run's numbers (depth
/// reached, kills, turns, light left, seed) and the restart/quit legend.
/// Shown once `g.dead || g.won` (see backend_minifb::run / backend_term::run).
fn render_end(g: &Game, cells: &mut [Cell]) {
    draw_border(cells, PANEL_X0, PANEL_Y0, PANEL_W, PANEL_H, PAL_STATUS);
    let mut row = PANEL_Y0 + 2;
    if g.won {
        let amulet = theme_for(g.seed, MAX_DEPTH).amulet;
        put_centered(cells, row, "YOU WON", PAL_PLAYER);
        row += 2;
        put_centered(cells, row, &format!("You climbed into daylight with {}.", amulet), PAL_LORE);
    } else {
        put_centered(cells, row, "YOU DIED", PAL_ALERT);
        row += 2;
        let cause = match g.killer {
            Some(name) => format!("Slain by the {}.", name),
            None => String::from("The dark took you."),
        };
        put_centered(cells, row, &cause, PAL_STATUS);
    }
    row += 2;
    put_centered(
        cells,
        row,
        &format!("Depth {}/{}  Kills {}  Turns {}  Light {}", g.depth, MAX_DEPTH, g.kills, g.turns, g.light),
        PAL_STATUS,
    );
    row += 1;
    put_centered(cells, row, &format!("Seed {}", g.seed), PAL_STATUS);
    row += 2;
    put_centered(cells, row, "[R] restart   [Q] quit", PAL_ALERT);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    /// Wall autotile mask covers: isolated (0), a vertical corridor (N|S),
    /// a horizontal corridor (W|E), a corner (N|E), a tee (N|S|W), and the
    /// full cross (all four) — one representative per WALL_GLYPHS shape
    /// family. Also proves an unseen wall neighbor doesn't count: seen
    /// topology must never leak through the shape of what's actually seen.
    #[test]
    fn wall_mask_representative() {
        let mut g = Game::new(1);
        // Clear a 3x3 patch to floor/seen so wall_mask starts from a known
        // all-non-wall neighborhood, then flip specific neighbors to walls.
        for dy in -1..=1 {
            for dx in -1..=1 {
                let i = idx(10 + dx, 10 + dy);
                g.map[i] = Tile::Floor;
                g.seen[i] = true;
            }
        }
        assert_eq!(wall_mask(&g, 10, 10), 0);
        assert_eq!(WALL_GLYPHS[0], 0x2500);

        let set = |g: &mut Game, dx: i32, dy: i32| {
            let i = idx(10 + dx, 10 + dy);
            g.map[i] = Tile::Wall;
            g.seen[i] = true;
        };

        // N|S: vertical corridor wall.
        let mut g2 = Game::new(1);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let i = idx(10 + dx, 10 + dy);
                g2.map[i] = Tile::Floor;
                g2.seen[i] = true;
            }
        }
        set(&mut g2, 0, -1);
        set(&mut g2, 0, 1);
        assert_eq!(wall_mask(&g2, 10, 10), 1 | 2);
        assert_eq!(WALL_GLYPHS[wall_mask(&g2, 10, 10)], 0x2502);

        // W|E: horizontal corridor wall.
        let mut g3 = Game::new(1);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let i = idx(10 + dx, 10 + dy);
                g3.map[i] = Tile::Floor;
                g3.seen[i] = true;
            }
        }
        set(&mut g3, -1, 0);
        set(&mut g3, 1, 0);
        assert_eq!(wall_mask(&g3, 10, 10), 4 | 8);
        assert_eq!(WALL_GLYPHS[wall_mask(&g3, 10, 10)], 0x2500);

        // N|E corner.
        let mut g4 = Game::new(1);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let i = idx(10 + dx, 10 + dy);
                g4.map[i] = Tile::Floor;
                g4.seen[i] = true;
            }
        }
        set(&mut g4, 0, -1);
        set(&mut g4, 1, 0);
        assert_eq!(wall_mask(&g4, 10, 10), 1 | 8);
        assert_eq!(WALL_GLYPHS[wall_mask(&g4, 10, 10)], 0x2514);

        // N|S|W tee.
        let mut g5 = Game::new(1);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let i = idx(10 + dx, 10 + dy);
                g5.map[i] = Tile::Floor;
                g5.seen[i] = true;
            }
        }
        set(&mut g5, 0, -1);
        set(&mut g5, 0, 1);
        set(&mut g5, -1, 0);
        assert_eq!(wall_mask(&g5, 10, 10), 1 | 2 | 4);
        assert_eq!(WALL_GLYPHS[wall_mask(&g5, 10, 10)], 0x2524);

        // All four: full cross.
        let mut g6 = Game::new(1);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let i = idx(10 + dx, 10 + dy);
                g6.map[i] = Tile::Floor;
                g6.seen[i] = true;
            }
        }
        set(&mut g6, 0, -1);
        set(&mut g6, 0, 1);
        set(&mut g6, -1, 0);
        set(&mut g6, 1, 0);
        assert_eq!(wall_mask(&g6, 10, 10), 15);
        assert_eq!(WALL_GLYPHS[15], 0x253C);

        // Unseen doesn't leak: a wall neighbor that hasn't been seen yet
        // must not count, even though it's genuinely Tile::Wall.
        let mut g7 = Game::new(1);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let i = idx(10 + dx, 10 + dy);
                g7.map[i] = Tile::Floor;
                g7.seen[i] = true;
            }
        }
        let i = idx(10, 9);
        g7.map[i] = Tile::Wall;
        g7.seen[i] = false; // wall exists, but not yet seen
        assert_eq!(wall_mask(&g7, 10, 10), 0);
    }

    /// The status row (HP bar + Torch bar + text) must fit within COLS=80
    /// even at the widest realistic values: maxhp 40 (the batch-3 HP
    /// progression tops out at 20 + 4*4 = 36; 40 leaves margin), light
    /// 2000 (START_LIGHT itself — the torch bar is fixed-width regardless
    /// of the numeric value, but this exercises the real constant), a
    /// double-digit ATK, and a four-digit kill count, both generous
    /// overestimates of anything a real run reaches.
    #[test]
    fn status_row_fits_80_cols() {
        let mut cells = vec![BLANK; CELLS];
        let end_col = draw_status(&mut cells, 0, 40, 40, 2000, 8, 99, MAX_DEPTH, 9999, true);
        assert!(end_col <= COLS, "status row overflowed: ended at col {}", end_col);
    }
}
