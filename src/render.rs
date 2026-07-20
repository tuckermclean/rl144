// render.rs — core cell composer: walks Game state into an 80x30 grid of
// Cell {ch, fg, bg}. This is a CORE module: zero platform calls, zero cfg,
// no font8x8. Cells are the natural unit both for a terminal backend
// (dirty-cell diffing is cheap on a Cell grid, expensive on raw pixels) and
// for a pixel backend (rasterizing a Cell into an 8x12 glyph rect is a
// mechanical, backend-local concern). Keeping composition here and
// rasterization in the backend is the core/crust seam: this module answers
// "what does the world look like," backends answer "how do I draw that."

use crate::content::{
    PAL_ALERT, PAL_BAR_EMPTY, PAL_BAR_HP, PAL_BAR_TORCH, PAL_BLOCK, PAL_CALM_TINT, PAL_GOAL,
    PAL_LOG_FADE, PAL_LORE, PAL_PIT, PAL_PLAYER, PAL_PORTAL, PAL_STAIRS, PAL_STATUS, lore_line,
    theme_for,
};
use crate::game::{
    COLS, Game, IKind, MAP_H, MKind, Monster, ROWS, Tile, fov_radius, idx, in_map, max_depth,
    start_light,
};
use crate::games::GAME;

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
/// shrinks the radius (see `fov_radius`) and dims what's still visible.
/// Tiers come from `BalanceDef::light_tiers` — exact-match lookup, falling
/// back to the table's last entry for any radius not otherwise listed.
fn light_pct(radius: i32) -> u32 {
    let tiers = GAME.balance.light_tiers;
    tiers
        .iter()
        .find(|&&(r, _)| r == radius)
        .map(|&(_, pct)| pct)
        .unwrap_or(tiers[tiers.len() - 1].1)
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
/// `[&]` objective-carried flag) starting at column 1 of `row`. Returns the column one
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
    col = put_bar(cells, col, row, bar_fill(light, start_light(), 10), 10, torch_fg);
    col = put_str(
        cells,
        col,
        row,
        &format!("]  ATK {}  D{}/{}  K{}", atk, depth, max_depth(), kills),
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

// ---------- scene() core surface (batch 4 task 3, C-impossible-object.md
// §3, DECISION.md sign-off item 5) ----------
/* `render_cells`/`Cell` throw away identity: a glyph carries no facing, no
   animation phase, no "this is entity #N, still alive next frame"
   continuity — exactly what a sprite backend (a later batch) needs and a
   flat character grid can't give it. `scene()` is the additive answer:
   every field on `SceneEntity` is DERIVED from `Game` with zero new stored
   state beyond `Game::facing` itself (see that field's doc comment for why
   even that one field is cheap and presentation-only). Nothing here is
   hashed, saved, or dumped — `--dump`'s `level_dump` (headless.rs) never
   calls into this module at all, so `scene()` existing has zero effect on
   any golden. */

/// Cardinal facing for a `SceneEntity`'s sprite. Presentation-only, same
/// exclusion list as `Game::facing`/`Game::fx_hit`/`Game::killer`/
/// `Game::echo` (see `save::state_hash`'s doc comment).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Facing {
    N,
    S,
    W,
    E,
}

impl Facing {
    /// Facing implied by a movement/attack delta. Every real caller passes
    /// a delta with exactly one nonzero axis (the four cardinal
    /// `try_move_player` directions, or a monster's `(signum, signum)`
    /// toward the player) — dx is checked first, so a genuinely diagonal
    /// delta would read as E/W, never a crash or a fifth direction. `(0,0)`
    /// (never a real move) falls through to the default `S`.
    pub(crate) fn from_delta(dx: i32, dy: i32) -> Facing {
        if dx > 0 {
            Facing::E
        } else if dx < 0 {
            Facing::W
        } else if dy < 0 {
            Facing::N
        } else if dy > 0 {
            Facing::S
        } else {
            Facing::S
        }
    }
}

/// What kind of sprite a `SceneEntity` is. `Echo` carries no payload beyond
/// the entity's own `x`/`y` (see `scene()`'s echo block).
///
/// `#[allow(dead_code)]` on this and the four items below it
/// (`SceneEntity`, `monster_facing`, `anim_phase`, `scene`): this is Phase 1
/// substrate exactly like `save::Ghost`/`parse_ghost` — the roadmap's own
/// words are "sprites arrive in a later phase" (this task's scope note).
/// `scene()` exists now so the surface is frozen and unit-tested
/// (`scene_is_deterministic`, `echo_appears_only_under_three_conditions`),
/// but its first real caller (a sprite-rasterizing backend) is a later
/// batch — a release build has no live path into any of these five items
/// yet, hence the explicit allow rather than a false "unused" signal.
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub(crate) enum SpriteKind {
    Player,
    Monster(MKind),
    Item(IKind),
    Echo,
}

/// One thing to draw this frame, richer than a `Cell`: kind, grid position
/// (same coordinate convention as `Cell`/`idx()` — no second coordinate
/// system), facing, an animation phase, and the current light percentage
/// (reuses `light_pct(fov_radius(..))`, the same brightness `render_play`
/// already applies to visible tiles/items/monsters). All fields
/// `pub(crate)`, all derived — see this section's header comment.
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub(crate) struct SceneEntity {
    pub(crate) kind: SpriteKind,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) facing: Facing,
    pub(crate) anim_phase: u8,
    pub(crate) light_pct: u32,
}

/// Monster facing: the sign of (player - monster) when the monster can
/// currently see the player (`Game::monster_sees_player` — the SAME
/// predicate `monsters_act` uses to decide whether to chase/attack, so this
/// never drifts from what the monster is "actually" doing), else the
/// default `S`. Pure: no stored per-monster state, no RNG draw.
#[allow(dead_code)]
fn monster_facing(g: &Game, m: &Monster) -> Facing {
    if g.monster_sees_player(m) {
        Facing::from_delta((g.px - m.x).signum(), (g.py - m.y).signum())
    } else {
        Facing::S
    }
}

/// `anim_phase` formula: `(g.turns + x + y) mod 4`, cast to `u8`. Folding
/// the entity's own grid position in means two entities on screen the same
/// turn land on different phases (no synchronized-blink look) without
/// persisting anything — it's a pure function of `(g.turns, x, y)`, so two
/// `scene()` calls on an unchanged `Game` are byte-identical (see
/// `scene_is_deterministic`). `rem_euclid` keeps the result in `[0, 4)`
/// even though `x`/`y` are signed (they never are in practice, since every
/// caller passes an in-map coordinate, but the formula stays correct either
/// way).
#[allow(dead_code)]
fn anim_phase(g: &Game, x: i32, y: i32) -> u8 {
    (g.turns as i64 + x as i64 + y as i64).rem_euclid(4) as u8
}

/// Compose this frame's sprite-level scene: the player (always), every
/// currently-visible monster and item, and the last-death echo. The echo
/// appears iff ALL THREE hold: `g.echo` is `Some`, its depth matches the
/// CURRENT depth (an echo from a depth the player isn't on right now must
/// not leak through), and the tile has actually been `seen` (same
/// never-leak-unseen-topology discipline `wall_mask` already applies to
/// walls). Zero RNG draws, zero mutation of `g`: calling `scene(g)` twice
/// in a row with no `Game` mutation between returns equal vectors.
#[allow(dead_code)]
pub(crate) fn scene(g: &Game) -> Vec<SceneEntity> {
    let pct = light_pct(fov_radius(g.light));
    let mut out = Vec::new();

    out.push(SceneEntity {
        kind: SpriteKind::Player,
        x: g.px,
        y: g.py,
        facing: g.facing,
        anim_phase: anim_phase(g, g.px, g.py),
        light_pct: pct,
    });

    for m in &g.monsters {
        if g.vis[idx(m.x, m.y)] {
            out.push(SceneEntity {
                kind: SpriteKind::Monster(m.kind),
                x: m.x,
                y: m.y,
                facing: monster_facing(g, m),
                anim_phase: anim_phase(g, m.x, m.y),
                light_pct: pct,
            });
        }
    }

    for it in &g.items {
        if g.vis[idx(it.x, it.y)] {
            out.push(SceneEntity {
                kind: SpriteKind::Item(it.kind),
                x: it.x,
                y: it.y,
                facing: Facing::S, // items have no orientation
                anim_phase: anim_phase(g, it.x, it.y),
                light_pct: pct,
            });
        }
    }

    if let Some((ex, ey, ed)) = g.echo {
        if ed == g.depth && g.seen[idx(ex, ey)] {
            out.push(SceneEntity {
                kind: SpriteKind::Echo,
                x: ex,
                y: ey,
                facing: Facing::S,
                anim_phase: anim_phase(g, ex, ey),
                light_pct: pct,
            });
        }
    }

    out
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
                Tile::Portal => (b'*' as u16, PAL_PORTAL),
                Tile::Pit => (b'^' as u16, PAL_PIT),
                Tile::Goal => (b'x' as u16, PAL_GOAL),
            };
            let c = if g.vis[i] { scale(color, pct) } else { dim(color) };
            put(cells, x as usize, y as usize, ch, c);
        }
    }
    // last-death echo (batch 4 task 3, C's shrunk "one echo, not a ghost
    // system" pitch): a dim '@' at the previous attempt's death tile,
    // drawn UNDER live entities (before items/monsters/player below, which
    // take visual priority if they share this tile) — same three-condition
    // visibility gate `scene()` uses (see its doc comment): matches
    // `g.echo`'s depth to the CURRENT depth, and requires the tile to have
    // actually been seen. Presentation-only; `--dump`'s `level_dump` never
    // calls into this file, so dump goldens are untouched regardless, and
    // a fresh game's `echo` is always `None` (`Game::new`), so frame
    // goldens are untouched too.
    if let Some((ex, ey, ed)) = g.echo {
        if ed == g.depth && g.seen[idx(ex, ey)] {
            put(cells, ex as usize, ey as usize, b'@' as u16, scale(PAL_PLAYER, 35));
        }
    }
    // items (visible only)
    for it in &g.items {
        if g.vis[idx(it.x, it.y)] {
            let def = &GAME.items[it.kind as usize];
            put(cells, it.x as usize, it.y as usize, def.glyph as u16, scale(def.color, pct));
        }
    }
    // push-blocks (batch 6 T2, sokoban; visible only). Drawn AFTER items so
    // a block visually covers an item hidden underneath it (see
    // `Game::blocks`' doc comment) — matches `headless::level_dump`'s same
    // item-then-block layering.
    for &(bx, by) in &g.blocks {
        if g.vis[idx(bx, by)] {
            put(cells, bx as usize, by as usize, b'B' as u16, scale(PAL_BLOCK, pct));
        }
    }
    // monsters (visible only). Becalmed monsters (batch 5) render with
    // PAL_CALM_TINT wholesale in place of the kind's own stats color — see
    // that const's doc comment for why a fixed tint rather than a blend.
    // Impossible in a fresh game (Monster::calm starts false, only set by
    // Game::try_talk_player), so this branch never fires for a turn-0 render
    // and frame goldens are untouched.
    for m in &g.monsters {
        if g.vis[idx(m.x, m.y)] {
            let def = Monster::stats(m.kind);
            let fg = if m.calm { PAL_CALM_TINT } else { def.color };
            put(cells, m.x as usize, m.y as usize, def.glyph as u16, scale(fg, pct));
        }
    }
    // player — the torch itself gutters at the lowest radius (dim to 85%);
    // otherwise it's always full brightness, since it IS the light source.
    let player_fg = if radius <= 3 { scale(PAL_PLAYER, 85) } else { PAL_PLAYER };
    put(cells, g.px as usize, g.py as usize, b'@' as u16, player_fg);

    // status: labeled HP/Torch bars.
    draw_status(cells, MAP_H, g.hp, g.maxhp, g.light, radius, g.atk, g.depth, g.kills, g.has_objective);

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
    put_centered(cells, row, "Move: arrows / wasd / hjkl    Wait: .    Talk: t+dir", PAL_STATUS);
    row += 1;
    put_centered(cells, row, "Give: g+dir    Use: u", PAL_STATUS);
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
        let objective = theme_for(g.seed, max_depth()).objective_name;
        put_centered(cells, row, "YOU WON", PAL_PLAYER);
        row += 2;
        put_centered(cells, row, &format!("You climbed into daylight with {}.", objective), PAL_LORE);
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
        &format!("Depth {}/{}  Kills {}  Turns {}  Light {}", g.depth, max_depth(), g.kills, g.turns, g.light),
        PAL_STATUS,
    );
    row += 1;
    put_centered(cells, row, &format!("Seed {}", g.seed), PAL_STATUS);
    row += 2;
    put_centered(cells, row, "[R] retry this world  [N] new world  [Q] quit", PAL_ALERT);
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
    /// 2000 (the cartridge's own start_light() — the torch bar is
    /// fixed-width regardless of the numeric value, but this exercises the
    /// real constant), a
    /// double-digit ATK, and a four-digit kill count, both generous
    /// overestimates of anything a real run reaches.
    #[test]
    fn status_row_fits_80_cols() {
        let mut cells = vec![BLANK; CELLS];
        let end_col = draw_status(&mut cells, 0, 40, 40, 2000, 8, 99, max_depth(), 9999, true);
        assert!(end_col <= COLS, "status row overflowed: ended at col {}", end_col);
    }

    /// scene() is a pure function of `Game`: two calls with no mutation in
    /// between must produce identical output. `SceneEntity: PartialEq`, so
    /// a plain `==` on the two `Vec`s is the whole assertion — this is what
    /// makes a later sprite backend's "same entity, still alive next
    /// frame" identity tracking trustworthy.
    #[test]
    fn scene_is_deterministic() {
        let g = Game::new(1);
        assert!(scene(&g) == scene(&g));
    }

    /// The echo entity appears iff ALL THREE conditions hold: `g.echo` is
    /// `Some`, its depth matches the CURRENT depth, and the tile has
    /// actually been `seen`. Each condition is falsified independently to
    /// prove none of the three is redundant with the others.
    #[test]
    fn echo_appears_only_under_three_conditions() {
        fn has_echo(g: &Game) -> bool {
            scene(g).iter().any(|e| e.kind == SpriteKind::Echo)
        }

        let g0 = Game::new(1);
        assert!(!has_echo(&g0), "no echo set: must not appear");

        // Matching depth, tile seen (the entrance tile is always seen —
        // compute_fov marks the player's own tile on every gen_level):
        // appears.
        let mut g1 = Game::new(1);
        let (ex, ey) = (g1.px, g1.py);
        g1.echo = Some((ex, ey, g1.depth));
        assert!(has_echo(&g1), "matching depth + seen tile: must appear");

        // Wrong depth: must not appear, even though the tile is seen.
        let mut g2 = Game::new(1);
        g2.echo = Some((ex, ey, g2.depth + 1));
        assert!(!has_echo(&g2), "depth mismatch: must not appear");

        // Right depth, but a tile that hasn't been seen: must not appear.
        let mut g3 = Game::new(1);
        let unseen = (0, 0); // map corner: never touched by starting FOV
        assert!(!g3.seen[idx(unseen.0, unseen.1)], "fixture assumption: (0,0) unseen at turn 0");
        g3.echo = Some((unseen.0, unseen.1, g3.depth));
        assert!(!has_echo(&g3), "unseen tile: must not appear");
    }

    /// `Game::facing` updates to the move direction on a successful move,
    /// and stays at its default `S` when nothing moves (a wall bump returns
    /// before any facing update — see `try_move_player`).
    #[test]
    fn facing_updates_on_move() {
        let g0 = Game::new(1);
        assert!(g0.facing == Facing::S, "default facing is S");

        // Every room is larger than 1x1, so at least one of the four
        // cardinal directions from the entrance is floor; try each and
        // check the one that actually moves the player.
        let dirs = [(0, -1, Facing::N), (0, 1, Facing::S), (-1, 0, Facing::W), (1, 0, Facing::E)];
        let (px0, py0) = (g0.px, g0.py);
        let mut moved = false;
        for (dx, dy, want) in dirs {
            let mut g = Game::new(1);
            g.try_move_player(dx, dy);
            if (g.px, g.py) != (px0, py0) {
                assert!(g.facing == want, "facing must match the move direction");
                moved = true;
                break;
            }
        }
        assert!(moved, "fixture assumption: at least one cardinal direction from spawn is floor");
    }

    /// Becalmed-monster tint (batch 5 task 3): a monster with `calm = true`
    /// renders with `PAL_CALM_TINT` in place of its kind's own stats color,
    /// while an otherwise-identical non-calm monster renders with its
    /// normal color — proving the two differ AND that the calm cell equals
    /// the documented tint exactly (Game::new's initial light is full,
    /// light_pct(fov_radius(START_LIGHT)) == 100, so `scale` is a no-op
    /// here). Seed 1 depth 1 always spawns >=1 monster (golden fixture:
    /// `monsters=4`), so indexing `g.monsters[0]` is safe.
    #[test]
    fn becalmed_monster_uses_calm_tint() {
        let mut g_normal = Game::new(1);
        assert!(!g_normal.monsters.is_empty(), "fixture assumption: seed 1 depth 1 has a monster");
        let (mx, my) = (g_normal.monsters[0].x, g_normal.monsters[0].y);
        let normal_kind = g_normal.monsters[0].kind;
        g_normal.vis[idx(mx, my)] = true;
        g_normal.seen[idx(mx, my)] = true;
        let mut cells_normal = vec![BLANK; CELLS];
        render_cells(&g_normal, Screen::Play, &mut cells_normal);
        let normal_fg = cells_normal[my as usize * COLS + mx as usize].fg;
        let kind_color = Monster::stats(normal_kind).color;
        assert_eq!(normal_fg, kind_color, "non-calm monster renders its kind's own color");

        let mut g_calm = Game::new(1);
        g_calm.monsters[0].calm = true;
        g_calm.vis[idx(mx, my)] = true;
        g_calm.seen[idx(mx, my)] = true;
        let mut cells_calm = vec![BLANK; CELLS];
        render_cells(&g_calm, Screen::Play, &mut cells_calm);
        let calm_fg = cells_calm[my as usize * COLS + mx as usize].fg;

        assert_eq!(calm_fg, PAL_CALM_TINT, "calm monster renders the documented tint exactly");
        assert_ne!(calm_fg, normal_fg, "calm tint must differ from the normal kind color");
    }

    /// Title-screen legend lines (incl. the talk chord addition, batch 5 task
    /// 3) must fit the 80-col grid with margin to spare for `put_centered`'s
    /// centering — same 78-col discipline the log-row flavor lines use
    /// (`main.rs::theme_lines_fit_log_row`), even though these are drawn via
    /// `put_centered` rather than `Game::log`.
    #[test]
    fn title_legend_fits_78_cols() {
        let lines = [
            "Move: arrows / wasd / hjkl    Wait: .    Talk: t+dir",
            "Give: g+dir    Use: u",
            "Save: F5    World info: F1    Quit: q",
            "[R] retry this world  [N] new world  [Q] quit",
        ];
        for line in lines {
            assert!(line.len() <= 78, "too long ({}): {}", line.len(), line);
        }
    }
}
