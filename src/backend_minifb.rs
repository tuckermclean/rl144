// backend_minifb.rs — the minifb/font8x8 presentation backend: everything
// platform-specific lives here so the core (render.rs and below) stays free
// of window/pixel/glyph-font concerns. This is the block a DOS or mobile
// port swaps; the fixed 80x30 Cell grid produced by render::render_cells is
// the seam it plugs into (see render.rs's header comment).

use crate::content::ghost_label_idx;
use crate::game::{COLS, Game, ROWS};
use crate::headless::world_hash;
use crate::render::{CELLS, Cell, Screen, render_cells};
use crate::rng::h64;
use crate::save::{
    GHOST_DIED_COMBAT, GHOST_DIED_DARK, INPUT_RESTART, INPUT_RETRY, ghost_bytes, ghost_filename,
    save_bytes, save_filename,
};
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

/// One-frame damage flash: brighten each RGB channel of `c` by 30%,
/// clamped to 255 per channel. `render::scale`'s multiply/divide has no
/// clamp because every caller there passes `pct <= 100` (a genuine
/// dimming); `flash` is the one place in this backend that legitimately
/// wants to scale PAST 100%, so it needs its own saturating math instead.
/// Pure RGB arithmetic, no game state, no RNG — headlessly testable
/// (`flash_channel_math`) even though the visual result (does a 1-frame
/// brighten actually read as "hit" at 60fps) is playtest-only.
fn flash(c: u32) -> u32 {
    let ch = |v: u32| -> u32 { (v * 13 / 10).min(255) };
    let r = ch((c >> 16) & 0xFF);
    let g = ch((c >> 8) & 0xFF);
    let b = ch(c & 0xFF);
    (r << 16) | (g << 8) | b
}

/// Squashed variant of `draw_glyph`, used only for the cell at
/// `Game::fx_hit` while it's `Some` (screen-feel, batch 4 task 3). A "real"
/// vertical squash would resample each glyph row through an interpolated
/// scale; this is the simple version the task scope asked for instead:
/// draw only the EVEN source rows of the 8-row glyph bitmap (0, 2, 4, 6 —
/// literally skipping every odd row, 4 of the 8 kept), one destination row
/// per kept source row, starting at `oy + CH - 6` (the top of a nominal
/// LOWER-6-ROW band, vs. `draw_glyph`'s vertically-centered
/// `oy + (CH - 8) / 2`). The 4 kept rows therefore occupy the UPPER 4 rows
/// of that 6-row band (`oy + CH - 6 ..< oy + CH - 2`), leaving the
/// cell's bottommost 2 rows as plain background — i.e. the glyph is both
/// half-scanlined AND shifted down, not stretched to fill all 6 rows. The
/// combined effect — half the scanlines, compressed toward (not filling)
/// the cell's bottom edge — reads as a one-frame vertical flatten/impact
/// squash with no resampling math at all. Deterministic and render-only:
/// same `(ch, fg, bg)` in, same pixels out, every call.
fn draw_glyph_squashed(buf: &mut [u32], ox: usize, oy: usize, ch: u16, fg: u32, bg: u32) {
    for y in 0..CH {
        for x in 0..CW {
            buf[(oy + y) * WIDTH + ox + x] = bg;
        }
    }
    let Some(glyph) = glyph_bits(ch) else {
        return;
    };
    let base = oy + CH - 6; // lower-6-row band starts here; see doc comment
    for (gy, bits) in glyph.iter().enumerate() {
        if gy % 2 == 1 {
            continue; // skip every other glyph row
        }
        let dy = base + gy / 2; // 4 kept rows land at base..base+3
        for gx in 0..8 {
            if bits >> gx & 1 == 1 {
                buf[dy * WIDTH + ox + gx] = fg;
            }
        }
    }
}

/// Rasterize a full Cell grid into the WIDTH*HEIGHT pixel framebuffer.
/// `fx_hit`, when `Some((x, y))`, is the grid cell to render with the
/// screen-feel treatment (palette flash + vertical squash) instead of the
/// plain glyph draw — callers pass `None` outside `Screen::Play` (see
/// `run`'s call site) so a stale `Game::fx_hit` can never paint a random
/// cell on the Title/End screens, which don't share Play's coordinate
/// meaning.
fn rasterize(cells: &[Cell], buf: &mut [u32], fx_hit: Option<(i32, i32)>) {
    for row in 0..ROWS {
        for col in 0..COLS {
            let c = cells[row * COLS + col];
            if fx_hit == Some((col as i32, row as i32)) {
                draw_glyph_squashed(buf, col * CW, row * CH, c.ch, flash(c.fg), c.bg);
            } else {
                draw_glyph(buf, col * CW, row * CH, c.ch, c.fg, c.bg);
            }
        }
    }
}

/// Auto-ghost capture (batch 4 task 2, save v2 substrate — Phase 1, no
/// rendering/playback yet). Called only for a DEAD run, right before R/N/Q
/// moves on from the End screen (never for a won run — Phase 1 never writes
/// GHOST_WON/GHOST_ABANDONED). Writes the just-ended attempt's (already
/// terminal, so already effectively trimmed) input log as an RLG1 file
/// named from the world hash, overwriting any earlier ghost for this world
/// (latest death wins — see `save::ghost_filename`). `outcome` reads
/// `game.killer`: `Some` means a combat kill, `None` means the run ended
/// dead with no killer, i.e. darkness (see `Game::killer`'s doc comment).
/// Best-effort: a write failure is silently ignored, same as this project's
/// "ghost is runtime user data, not a shipped asset" doctrine treats any
/// other player file.
fn write_ghost(game: &Game, whash: u64, attempt_log: &[u8]) {
    let outcome = if game.killer.is_some() { GHOST_DIED_COMBAT } else { GHOST_DIED_DARK };
    let final_depth = game.depth as u8;
    let label_idx = ghost_label_idx(outcome, final_depth);
    let bytes = ghost_bytes(game.seed, whash, outcome, final_depth, game.turns, label_idx, attempt_log);
    let _ = std::fs::write(ghost_filename(whash), bytes);
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
    // ACT chord (batch 5 task 3, the Henson ruling's frontend half): `t`
    // arms this flag; the NEXT direction key (arrows/wasd/hjkl) completes
    // the chord and pushes ONE ACT byte (7-10), consuming the arm. Any
    // other key pressed while armed — including a second bare `t`, which
    // just re-arms rather than progressing anything — disarms with no byte
    // logged, per the task spec's "simplest rule: any non-direction input
    // disarms." Frontend-local, like `confirm_armed`: never saved, never
    // hashed, never touches replay.
    let mut act_armed = false;
    // The CURRENT attempt's input bytes only (cleared on every R/N), as
    // opposed to `input_log` which is the whole session across attempts —
    // this is what a captured ghost should replay, not the full history
    // (see `write_ghost`). A `--load`ed session starts this empty too: the
    // ghost only ever covers what happened in THIS run of the program.
    let mut attempt_log: Vec<u8> = Vec::new();
    // Title at launch unless resuming a --load; End once the run is over
    // (checked at the bottom of the Play arm, same frame the game reports
    // dead/won). See render::Screen's doc comment for the full state
    // machine.
    let mut screen = if loaded { Screen::Play } else { Screen::Title };
    // Deferred Play->End transition: when the run ends WITH a killing-blow
    // flash pending (`game.fx_hit.is_some()`), the Play arm below sets this
    // instead of flipping `screen` immediately, so the bottom-of-loop
    // render still draws one Play-screen frame (with the flash/squash) for
    // the dead/won game state before anything shows End. The NEXT loop
    // iteration flips `screen` to End here, at the top, before the match —
    // so that iteration's Play arm (key polling, input_log/attempt_log
    // pushes) never runs for a game that's already over: no extra input
    // bytes, no double-processing, purely a one-frame render delay. A
    // death with no flash (darkness) or a win (never sets fx_hit) is
    // unaffected — screen flips to End the same frame as before.
    let mut pending_end = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if pending_end {
            screen = Screen::End;
            pending_end = false;
        }
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
                // `t` arms the ACT chord (edge-triggered: holding it down
                // doesn't repeatedly (re-)arm every frame's worth of key
                // state, though re-arming while already armed is harmless —
                // see `act_armed`'s doc comment).
                if window.is_key_pressed(Key::T, KeyRepeat::No) {
                    act_armed = true;
                }
                let mut act_consumed = false;
                for (key, (dx, dy)) in moves {
                    let dir = match (dx, dy) {
                        (0, -1) => 0,
                        (0, 1) => 1,
                        (-1, 0) => 2,
                        _ => 3,
                    };
                    if act_armed {
                        // Chord completion: a FRESH direction-key press
                        // (KeyRepeat::No — deliberate, not a held-key
                        // repeat) while armed produces one ACT byte
                        // (7=N,8=S,9=W,10=E, mirroring the move bytes'
                        // direction order) instead of a move byte, then
                        // disarms. Only the first matching direction this
                        // frame counts.
                        if !act_consumed && window.is_key_pressed(key, KeyRepeat::No) {
                            let b = dir + 7;
                            input_log.push(b);
                            attempt_log.push(b);
                            game.apply_input(b);
                            confirm_armed = false;
                            act_armed = false;
                            act_consumed = true;
                        }
                    } else if window.is_key_pressed(key, KeyRepeat::Yes) {
                        input_log.push(dir);
                        attempt_log.push(dir);
                        game.apply_input(dir);
                        confirm_armed = false;
                    }
                }
                // Chord cancel: still armed (no direction completed it this
                // frame) and some OTHER key was pressed this frame — disarm
                // silently, no byte logged. `t` itself is excluded so
                // arming and re-arming in the same frame never
                // self-cancels.
                if act_armed && !act_consumed {
                    let pressed = window.get_keys_pressed(KeyRepeat::No);
                    if pressed.iter().any(|&k| k != Key::T) {
                        act_armed = false;
                    }
                }
                if window.is_key_pressed(Key::Period, KeyRepeat::Yes) {
                    input_log.push(4);
                    attempt_log.push(4);
                    game.apply_input(4);
                    confirm_armed = false;
                    // Wait is held-repeat (KeyRepeat::Yes), so it never
                    // shows up in the single-frame get_keys_pressed(No)
                    // disarm check above — a held `.` after a `t` arm would
                    // otherwise leave the talk chord armed across many
                    // turns, silently converting a later direction press
                    // into a talk instead of a move. Disarm explicitly.
                    act_armed = false;
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
                    if game.fx_hit.is_some() {
                        // Killing-blow flash pending: delay the transition
                        // by one rendered frame (see `pending_end`'s doc
                        // comment above) instead of cutting straight to
                        // End, so the flash is actually visible.
                        pending_end = true;
                    } else {
                        screen = Screen::End;
                    }
                }
            }
            Screen::End => {
                // R: retry, same seed (byte 6, save v2). Carries `echo`
                // forward from the death position/depth so a future
                // renderer (Phase 4) can mark it — see `Game::echo`'s doc
                // comment. N: new world, same reroll byte 5 always was.
                // Both, plus Q, auto-capture a ghost first if the run that
                // just ended was DEAD (never for a win — see `write_ghost`).
                if window.is_key_pressed(Key::R, KeyRepeat::No) {
                    if game.dead {
                        write_ghost(&game, whash, &attempt_log);
                    }
                    let echo = if game.dead { Some((game.px, game.py, game.depth)) } else { None };
                    input_log.push(INPUT_RETRY);
                    attempt_log.clear();
                    let s = game.seed;
                    game = Game::new(s);
                    game.echo = echo;
                    window.set_title(&title(s));
                    confirm_armed = false;
                    // Defense-in-depth (a stuck-armed talk chord shouldn't
                    // survive a death mid-chord and leak into the next
                    // attempt) — see the Period-block fix above for the
                    // primary case this closes.
                    act_armed = false;
                    screen = Screen::Play;
                }
                if window.is_key_pressed(Key::N, KeyRepeat::No) {
                    if game.dead {
                        write_ghost(&game, whash, &attempt_log);
                    }
                    input_log.push(INPUT_RESTART);
                    attempt_log.clear();
                    let s = h64(game.seed, &["restart"]);
                    game = Game::new(s);
                    whash = world_hash(s);
                    window.set_title(&title(s));
                    confirm_armed = false;
                    act_armed = false;
                    screen = Screen::Play;
                }
                if window.is_key_pressed(Key::Q, KeyRepeat::No) {
                    if game.dead {
                        write_ghost(&game, whash, &attempt_log);
                    }
                    break;
                }
            }
        }
        render_cells(&game, screen, &mut cells);
        // Screen-feel only applies to the Play-screen map grid, whose
        // coordinates are what `Game::fx_hit` is expressed in — Title/End
        // panels use a different layout, so a leftover fx_hit from the run
        // that just ended must not paint one of their cells.
        let fx = if screen == Screen::Play { game.fx_hit } else { None };
        rasterize(&cells, &mut buf, fx);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// flash() channel math: 30% brighten per channel, clamped to 255.
    /// Pure RGB arithmetic — the visual feel is playtest-only, but the
    /// math itself is headlessly checkable.
    #[test]
    fn flash_channel_math() {
        assert_eq!(flash(0x000000), 0x000000);
        assert_eq!(flash(0x646464), 0x828282); // 100 * 1.3 = 130, no clamp
        assert_eq!(flash(0xC8C8C8), 0xFFFFFF); // 200 * 1.3 = 260 -> clamp 255
        assert_eq!(flash(0xFFFFFF), 0xFFFFFF); // already saturated
    }
}
