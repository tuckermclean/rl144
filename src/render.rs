// render.rs — presentation layer: glyph drawing into the u32 framebuffer and
// the per-frame render() that walks Game state into pixels. This is the
// block a DOS or mobile frontend would swap; the engine (game.rs) never
// reaches back into it.

use crate::game::{COLS, Game, IKind, MAP_H, MAX_DEPTH, Monster, ROWS, Tile, fov_radius, idx};
use font8x8::legacy::BASIC_LEGACY;

const CW: usize = 8;
const CH: usize = 12;
pub(crate) const WIDTH: usize = COLS * CW; // 640
pub(crate) const HEIGHT: usize = ROWS * CH; // 360

// ---------- Rendering ----------
fn draw_char(buf: &mut [u32], col: usize, row: usize, ch: u8, color: u32) {
    let glyph = BASIC_LEGACY[ch as usize & 0x7F];
    let ox = col * CW;
    let oy = row * CH + (CH - 8) / 2;
    for (gy, bits) in glyph.iter().enumerate() {
        for gx in 0..8 {
            if bits >> gx & 1 == 1 {
                buf[(oy + gy) * WIDTH + ox + gx] = color;
            }
        }
    }
}

fn draw_str(buf: &mut [u32], col: usize, row: usize, s: &str, color: u32) {
    for (i, ch) in s.bytes().enumerate() {
        if col + i >= COLS {
            break;
        }
        draw_char(buf, col + i, row, ch, color);
    }
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

pub(crate) fn render(g: &Game, buf: &mut [u32]) {
    buf.iter_mut().for_each(|p| *p = 0);
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
            draw_char(buf, x as usize, y as usize, ch, c);
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
            draw_char(buf, it.x as usize, it.y as usize, ch, scale(c, pct));
        }
    }
    // monsters (visible only)
    for m in &g.monsters {
        if g.vis[idx(m.x, m.y)] {
            let (_, _, ch, c, _) = Monster::stats(m.kind);
            draw_char(buf, m.x as usize, m.y as usize, ch, scale(c, pct));
        }
    }
    // player — the player is the torch; always full brightness
    draw_char(buf, g.px as usize, g.py as usize, b'@', 0xFFFFFF);

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
    draw_str(buf, 1, MAP_H, &status, sc);
    // log: last 4, older lines faded
    let n = g.msgs.len();
    let recent = &g.msgs[n.saturating_sub(4)..];
    let fade = [0x707070u32, 0x909090, 0xB0B0B0, 0xE0E0E0];
    for (r, m) in recent.iter().enumerate() {
        let shade = fade[fade.len() - recent.len() + r];
        draw_str(buf, 1, MAP_H + 1 + r, m, shade);
    }
}
