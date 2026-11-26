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
    // batch 14 render fix: grading is now per-tile via `lit_r`, so a test
    // that hand-forces a tile visible must also set its light-source
    // radius (this test bypasses `compute_fov`). Full torch radius →
    // light_pct 100 → `scale` a no-op, keeping this a pure color/tint check.
    g_normal.lit_r[idx(mx, my)] = fov_radius(g_normal.light) as u8;
    let mut cells_normal = vec![BLANK; CELLS];
    render_cells(&g_normal, Screen::Play, &mut cells_normal);
    let normal_fg = cells_normal[my as usize * COLS + mx as usize].fg;
    let kind_color = Monster::stats(normal_kind).color;
    assert_eq!(normal_fg, kind_color, "non-calm monster renders its kind's own color");

    let mut g_calm = Game::new(1);
    g_calm.monsters[0].calm = true;
    g_calm.vis[idx(mx, my)] = true;
    g_calm.seen[idx(mx, my)] = true;
    g_calm.lit_r[idx(mx, my)] = fov_radius(g_calm.light) as u8;
    let mut cells_calm = vec![BLANK; CELLS];
    render_cells(&g_calm, Screen::Play, &mut cells_calm);
    let calm_fg = cells_calm[my as usize * COLS + mx as usize].fg;

    assert_eq!(calm_fg, PAL_CALM_TINT, "calm monster renders the documented tint exactly");
    assert_ne!(calm_fg, normal_fg, "calm tint must differ from the normal kind color");
}

/// Batch 14 render fix — the regression guard for the bug a human caught
/// only by playing (every gate stayed green because the turn-0 frame
/// goldens can never reach a carried-and-shining state). While the
/// McGuffin is carried and her mood shines a WIDE radius, a tile she
/// alone lights beyond a DYING torch must render at HER brightness, not
/// the torch's murk. Before the fix, `render_play` graded the whole frame
/// by one `light_pct(fov_radius(g.light))` (torch-only), so her light read
/// as torch-capped. This constructs that exact scenario directly (no
/// seed-dependent depth-5 replay needed) and asserts the tile she alone
/// lights renders bright — and specifically NOT at the old buggy torch
/// murk.
#[test]
fn mcguffin_shine_renders_at_her_brightness_not_the_dying_torch() {
    use crate::game::{Tile, fov_radius, idx, in_map, mood_shine_radius};
    let mut g = Game::new(1);
    let (px, py) = (g.px, g.py);
    // Open a clear box around the player so LOS to a far tile is guaranteed.
    for dy in -7..=7 {
        for dx in -7..=7 {
            if in_map(px + dx, py + dy) {
                g.map[idx(px + dx, py + dy)] = Tile::Floor;
            }
        }
    }
    // Dying torch: a small radius (torch-only grading would be dim murk).
    g.light = 1;
    let torch_r = fov_radius(g.light);
    // Carried + top mood -> a wide shine centered on the player.
    g.has_objective = true;
    g.mood_sum = 100 * 20;
    g.mood_count = 20; // mood() == 100
    let her_r = mood_shine_radius(g.mood());
    assert!(her_r > torch_r, "fixture: her shine ({}) must exceed the dying torch ({})", her_r, torch_r);
    assert_ne!(light_pct(her_r), light_pct(torch_r), "fixture: her brightness must differ from the torch's");
    g.compute_fov();
    // A tile she alone lights: one past the torch radius, within hers.
    let d = torch_r + 1;
    assert!(d <= her_r, "fixture: the probe tile must be inside her shine");
    let (tx, ty) = if in_map(px + d, py) {
        (px + d, py)
    } else if in_map(px - d, py) {
        (px - d, py)
    } else if in_map(px, py + d) {
        (px, py + d)
    } else {
        (px, py - d)
    };
    assert!(g.vis[idx(tx, ty)], "fixture: the far tile must be lit by her shine");
    assert_eq!(g.lit_r[idx(tx, ty)] as i32, her_r, "her shine is the strongest source at this tile");
    let mut cells = vec![BLANK; CELLS];
    render_cells(&g, Screen::Play, &mut cells);
    let rendered = cells[ty as usize * COLS + tx as usize].fg;
    let floor = g.theme().floor;
    assert_eq!(rendered, scale(floor, light_pct(her_r)), "her-lit tile renders at HER brightness");
    assert_ne!(
        rendered,
        scale(floor, light_pct(torch_r)),
        "her-lit tile must NOT render at the dying-torch murk (the exact bug this fixes)"
    );
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
        "Give: g+dir    Use: u    Put down: p",
        "Save: F5    World info: F1    Quit: q",
        "[R] retry this world  [N] new world  [Q] quit",
    ];
    for line in lines {
        assert!(line.len() <= 78, "too long ({}): {}", line.len(), line);
    }
}
