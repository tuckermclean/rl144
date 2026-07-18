// render.rs — core cell composer: walks Game state into an 80x30 grid of
// Cell {ch, fg, bg}. This is a CORE module: zero platform calls, zero cfg,
// no font8x8. Cells are the natural unit both for a terminal backend
// (dirty-cell diffing is cheap on a Cell grid, expensive on raw pixels) and
// for a pixel backend (rasterizing a Cell into an 8x12 glyph rect is a
// mechanical, backend-local concern). Keeping composition here and
// rasterization in the backend is the core/crust seam: this module answers
// "what does the world look like," backends answer "how do I draw that."

use crate::game::{COLS, Game, IKind, MAP_H, MAX_DEPTH, Monster, ROWS, Tile, fov_radius, idx};

/// A single terminal-style cell: one glyph plus its foreground/background
/// color. `ch` is a Unicode BMP codepoint (u16); today every glyph is ASCII
/// (<128) since box-drawing glyphs arrive in a later task. `bg` is
/// 0x000000 (black) everywhere today — no cell currently paints a
/// background — but backends should still honor it rather than assuming
/// black, since that's the whole point of carrying it separately from fg.
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
/// shrinks the radius (see `fov_radius`) and dims what's still visible.
fn light_pct(radius: i32) -> u32 {
    match radius {
        8 => 100,
        6 => 90,
        5 => 80,
        4 => 65,
        3 => 50,
        _ => 40,
    }
}

fn put(cells: &mut [Cell], col: usize, row: usize, ch: u8, fg: u32) {
    if col < COLS && row < ROWS {
        cells[row * COLS + col] = Cell { ch: ch as u16, fg, bg: 0 };
    }
}

fn put_str(cells: &mut [Cell], col: usize, row: usize, s: &str, fg: u32) {
    for (i, ch) in s.bytes().enumerate() {
        if col + i >= COLS {
            break;
        }
        put(cells, col + i, row, ch, fg);
    }
}

/// Compose the current frame as an 80x30 grid of cells. `cells` must be
/// exactly `CELLS` long. Preserves today's visual output exactly: same
/// glyphs, same colors, same status line, same 4-line faded log.
pub(crate) fn render_cells(g: &Game, cells: &mut [Cell]) {
    cells.iter_mut().for_each(|c| *c = BLANK);
    let theme = g.theme();
    // Brightness percentage for currently-visible tiles/items/monsters only;
    // seen-but-not-visible tiles keep the existing dim() treatment instead
    // (memory stays legible; the dark closes in on what's currently seen).
    let pct = light_pct(fov_radius(g.light));
    // map
    for y in 0..MAP_H as i32 {
        for x in 0..COLS as i32 {
            let i = idx(x, y);
            if !g.seen[i] {
                continue;
            }
            let (ch, color) = match g.map[i] {
                Tile::Wall => (b'#', theme.wall),
                Tile::Floor => (b'.', theme.floor),
                Tile::Stairs => (b'>', 0xFFFF60),
                Tile::UpStairs => (b'<', 0xFFFF60),
            };
            let c = if g.vis[i] { scale(color, pct) } else { dim(color) };
            put(cells, x as usize, y as usize, ch, c);
        }
    }
    // items (visible only)
    for it in &g.items {
        if g.vis[idx(it.x, it.y)] {
            let (ch, c) = match it.kind {
                IKind::Potion => (b'!', 0xFF50A0),
                IKind::Sword => (b')', 0x70B0FF),
                IKind::Amulet => (b'&', 0xFFD700),
                IKind::LoreA | IKind::LoreB | IKind::LoreC => (b'?', 0xC0A0FF),
            };
            put(cells, it.x as usize, it.y as usize, ch, scale(c, pct));
        }
    }
    // monsters (visible only)
    for m in &g.monsters {
        if g.vis[idx(m.x, m.y)] {
            let (_, _, ch, c, _) = Monster::stats(m.kind);
            put(cells, m.x as usize, m.y as usize, ch, scale(c, pct));
        }
    }
    // player — the player is the torch; always full brightness
    put(cells, g.px as usize, g.py as usize, b'@', 0xFFFFFF);

    // status; light is the run's clock, so it sits right after HP
    let status = format!(
        "HP {:>2}/{}  Light {:>4}  ATK {}  Depth {}/{}  Kills {}{}",
        g.hp,
        g.maxhp,
        g.light,
        g.atk,
        g.depth,
        MAX_DEPTH,
        g.kills,
        if g.has_amulet { "  [&]" } else { "" }
    );
    let low_light = fov_radius(g.light) <= 4;
    let sc = if g.hp <= g.maxhp / 4 || low_light { 0xFF5050 } else { 0xE0E0E0 };
    put_str(cells, 1, MAP_H, &status, sc);
    // log: last 4, older lines faded
    let n = g.msgs.len();
    let recent = &g.msgs[n.saturating_sub(4)..];
    let fade = [0x707070u32, 0x909090, 0xB0B0B0, 0xE0E0E0];
    for (r, m) in recent.iter().enumerate() {
        let shade = fade[fade.len() - recent.len() + r];
        put_str(cells, 1, MAP_H + 1 + r, m, shade);
    }
}
