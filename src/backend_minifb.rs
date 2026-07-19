// backend_minifb.rs — the minifb/font8x8 presentation backend: everything
// platform-specific lives here so the core (render.rs and below) stays free
// of window/pixel/glyph-font concerns. This is the block a DOS or mobile
// port swaps; the fixed 80x30 Cell grid produced by render::render_cells is
// the seam it plugs into (see render.rs's header comment).

use crate::game::{COLS, Game, ROWS};
use crate::headless::world_hash;
use crate::render::{CELLS, Cell, Screen, render_cells};
use crate::rng::h64;
use crate::save::{INPUT_RESTART, save_bytes, save_filename};
use font8x8::legacy::{BASIC_LEGACY, BLOCK_LEGACY, BOX_LEGACY};
use minifb::{Key, KeyRepeat, ScaleMode, Window, WindowOptions};

const CW: usize = 8;
const CH: usize = 12;
pub(crate) const WIDTH: usize = COLS * CW; // 640
pub(crate) const HEIGHT: usize = ROWS * CH; // 360

/// Resolve a cell glyph's 8x8 bitmap. ASCII (<128) comes from font8x8's
/// legacy BASIC table (unchanged from before this task). Box-drawing
/// (U+2500..=U+257F, wall autotiling + Title/End panel borders) and block
/// (U+2580..=U+259F, the status bars' filled/empty glyphs) glyphs come from
/// their own legacy tables. All three are DIRECT array indices, not a
/// search: font8x8 0.3's box.rs/block.rs build BOX_UNICODE/BLOCK_UNICODE as
/// exactly `FontUnicode(base + i, LEGACY[i])` for consecutive codepoints
/// starting at 0x2500/0x2580 respectively (confirmed by reading the
/// vendored source under font8x8-0.3.1/src/{box,block}.rs), so
/// `codepoint - base` is the legacy-table index — O(1), no linear scan
/// despite covering ~160 codepoints across the two tables. Any other
/// ch >= 128 is unmapped -> None -> blank cell (unchanged from before this
/// task). This only needs the `legacy` module, which font8x8 compiles
/// unconditionally (see Cargo.toml: the "unicode" feature — the FontUnicode
/// search API we don't use — was dropped as dead weight against the size
/// budget).
fn glyph_bits(ch: u16) -> Option<[u8; 8]> {
    match ch {
        0..=127 => Some(BASIC_LEGACY[ch as usize]),
        0x2500..=0x257F => Some(BOX_LEGACY[(ch - 0x2500) as usize]),
        0x2580..=0x259F => Some(BLOCK_LEGACY[(ch - 0x2580) as usize]),
        _ => None,
    }
}

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
    let Some(glyph) = glyph_bits(ch) else {
        return;
    };
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
/// logged by the caller before `run` is entered). `loaded` is whether
/// `game` came from `--load`: a loaded run skips the Title screen (the
/// player already knows this world) and opens straight on Play.
pub(crate) fn run(seed0: u64, mut input_log: Vec<u8>, mut game: Game, daily: bool, day: u64, loaded: bool) {
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
    // Title at launch unless resuming a --load; End once the run is over
    // (checked at the bottom of the Play arm, same frame the game reports
    // dead/won). See render::Screen's doc comment for the full state
    // machine.
    let mut screen = if loaded { Screen::Play } else { Screen::Title };

    while window.is_open() && !window.is_key_down(Key::Escape) {
        match screen {
            Screen::Title => {
                // Any key dismisses the title — consumed here WITHOUT
                // logging an input byte or touching `game`, so a title
                // screen never perturbs replay.
                if !window.get_keys_pressed(KeyRepeat::No).is_empty() {
                    screen = Screen::Play;
                }
            }
            Screen::Play => {
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
                // F1: identify the world. Log-only — consumes no turn, no
                // input byte, and touches no RNG channel, so replay is
                // unaffected.
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
                if game.dead || game.won {
                    screen = Screen::End;
                }
            }
            Screen::End => {
                if window.is_key_pressed(Key::R, KeyRepeat::No) {
                    input_log.push(INPUT_RESTART);
                    let s = h64(game.seed, &["restart"]);
                    game = Game::new(s);
                    whash = world_hash(s);
                    window.set_title(&title(s));
                    confirm_armed = false;
                    screen = Screen::Play;
                }
                if window.is_key_pressed(Key::Q, KeyRepeat::No) {
                    break;
                }
            }
        }
        render_cells(&game, screen, &mut cells);
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
