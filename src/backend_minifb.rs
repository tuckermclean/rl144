// backend_minifb.rs — the minifb/font8x8 presentation backend: everything
// platform-specific lives here so the core (render.rs and below) stays free
// of window/pixel/glyph-font concerns. This is the block a DOS or mobile
// port swaps; the fixed 80x30 Cell grid produced by render::render_cells is
// the seam it plugs into (see render.rs's header comment).

use crate::game::{COLS, Game, ROWS};
use crate::headless::world_hash;
use crate::render::{CELLS, Cell, render_cells};
use crate::rng::h64;
use crate::save::{INPUT_RESTART, save_bytes, save_filename};
use font8x8::legacy::BASIC_LEGACY;
use minifb::{Key, KeyRepeat, ScaleMode, Window, WindowOptions};

const CW: usize = 8;
const CH: usize = 12;
pub(crate) const WIDTH: usize = COLS * CW; // 640
pub(crate) const HEIGHT: usize = ROWS * CH; // 360

/// Rasterize one cell's glyph into the pixel buffer: fill its 8x12 rect with
/// `bg`, then stamp the glyph's set pixels in `fg`. `bg` is 0x000000
/// everywhere today, so this is equivalent to a full-buffer clear, but
/// filling per-cell keeps the door open for real per-cell backgrounds later.
fn draw_glyph(buf: &mut [u32], ox: usize, oy: usize, ch: u16, fg: u32, bg: u32) {
    for y in 0..CH {
        for x in 0..CW {
            buf[(oy + y) * WIDTH + ox + x] = bg;
        }
    }
    if ch >= 128 {
        return; // unicode box/block glyphs land in a later task
    }
    let glyph = BASIC_LEGACY[ch as usize & 0x7F];
    let gy0 = oy + (CH - 8) / 2; // keep font8x8's 8px glyph centered in the 12px cell
    for (gy, bits) in glyph.iter().enumerate() {
        for gx in 0..8 {
            if bits >> gx & 1 == 1 {
                buf[(gy0 + gy) * WIDTH + ox + gx] = fg;
            }
        }
    }
}

/// Rasterize a full Cell grid into the WIDTH*HEIGHT pixel framebuffer.
fn rasterize(cells: &[Cell], buf: &mut [u32]) {
    for row in 0..ROWS {
        for col in 0..COLS {
            let c = cells[row * COLS + col];
            draw_glyph(buf, col * CW, row * CH, c.ch, c.fg, c.bg);
        }
    }
}

/// Run the minifb window loop: input -> Game::apply-equivalent calls,
/// render_cells -> rasterize -> present. Owns everything platform-specific
/// (window, key polling, save-file I/O, title). `seed0`/`input_log`/`game`
/// come from either a fresh seed or a `--load`ed save; `daily`/`day` are
/// only used for the title and the once-per-run daily log line (already
/// logged by the caller before `run` is entered).
pub(crate) fn run(seed0: u64, mut input_log: Vec<u8>, mut game: Game, daily: bool, day: u64) {
    // The 80x30 cell grid (640x360 logical pixels) is engine API — COLS and
    // MAP_H are baked into worldgen, so the grid can never follow the
    // window. The window is presentation: minifb scales the fixed buffer,
    // preserving aspect. A DOS or mobile frontend swaps this whole module,
    // not the grid.
    let title = |seed: u64| {
        if daily {
            format!("rl144 — daily #{} — seed {}", day, seed)
        } else {
            format!("rl144 — seed {}", seed)
        }
    };
    let mut whash = world_hash(game.seed);
    let mut window = Window::new(
        &title(game.seed),
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: true,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .expect("window");
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

    let mut buf = vec![0u32; WIDTH * HEIGHT];
    let mut cells = vec![Cell { ch: b' ' as u16, fg: 0, bg: 0 }; CELLS];
    let moves: [(Key, (i32, i32)); 12] = [
        (Key::Up, (0, -1)),
        (Key::Down, (0, 1)),
        (Key::Left, (-1, 0)),
        (Key::Right, (1, 0)),
        (Key::W, (0, -1)),
        (Key::S, (0, 1)),
        (Key::A, (-1, 0)),
        (Key::D, (1, 0)),
        (Key::K, (0, -1)),
        (Key::J, (0, 1)),
        (Key::H, (-1, 0)),
        (Key::L, (1, 0)),
    ];

    // F5 overwrite confirmation: first press on an existing save file arms
    // this flag and logs a warning instead of writing; a second F5 press
    // while armed writes. Any real game input (move/wait/restart) disarms
    // it, so a stray F5 days later doesn't silently clobber a save.
    let mut confirm_armed = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        for (key, (dx, dy)) in moves {
            if window.is_key_pressed(key, KeyRepeat::Yes) {
                input_log.push(match (dx, dy) {
                    (0, -1) => 0,
                    (0, 1) => 1,
                    (-1, 0) => 2,
                    _ => 3,
                });
                game.try_move_player(dx, dy);
                confirm_armed = false;
            }
        }
        if window.is_key_pressed(Key::Period, KeyRepeat::Yes) {
            input_log.push(4);
            game.wait_turn();
            confirm_armed = false;
        }
        if (game.dead || game.won) && window.is_key_pressed(Key::R, KeyRepeat::No) {
            input_log.push(INPUT_RESTART);
            let s = h64(game.seed, &["restart"]);
            game = Game::new(s);
            whash = world_hash(s);
            window.set_title(&title(s));
            confirm_armed = false;
        }
        // F1: identify the world. Log-only — consumes no turn, no input byte,
        // and touches no RNG channel, so replay is unaffected.
        if window.is_key_pressed(Key::F1, KeyRepeat::No) {
            game.log(format!("Seed {}  world {:016x}", game.seed, whash));
        }
        if window.is_key_pressed(Key::F5, KeyRepeat::No) {
            let fname = save_filename(whash);
            if std::path::Path::new(&fname).exists() && !confirm_armed {
                confirm_armed = true;
                game.log(format!("{} exists. F5 again to overwrite.", fname));
            } else {
                confirm_armed = false;
                match std::fs::write(&fname, save_bytes(seed0, &input_log)) {
                    Ok(()) => game.log(format!("Saved to {}.", fname)),
                    Err(_) => game.log(String::from("Save failed!")),
                }
            }
        }
        render_cells(&game, &mut cells);
        rasterize(&cells, &mut buf);
        window.update_with_buffer(&buf, WIDTH, HEIGHT).expect("update");
    }

    // Autosave on quit: only for a still-live run with unsaved progress.
    // Never clobber a manual save — if the hashed filename already exists,
    // fall back to a .auto.sav sibling. Window is gone, so print, don't log.
    if !game.dead && !game.won && !input_log.is_empty() {
        let fname = save_filename(whash);
        let path = if std::path::Path::new(&fname).exists() {
            format!("rl144-{:016x}.auto.sav", whash)
        } else {
            fname
        };
        match std::fs::write(&path, save_bytes(seed0, &input_log)) {
            Ok(()) => println!("Autosaved to {}.", path),
            Err(e) => println!("Autosave failed: {}", e),
        }
    }
}
