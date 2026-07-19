// backend_term.rs — the ANSI terminal presentation backend: raw termios
// input, a dirty-cell ANSI encoder, and the interactive loop. Like
// backend_minifb.rs, everything platform-specific lives here so the core
// (render.rs and below) stays free of window/terminal concerns; this module
// consumes the same core Cell-grid surface (render::render_cells) the
// minifb backend consumes. cfg is allowed here (and in main.rs's wiring)
// only — the core modules stay cfg-free.
//
// Two halves:
//   - I/O: hand-rolled extern "C" FFI against tcgetattr/tcsetattr/read/write
//     (already linked via libc under std; no new crate). Raw mode disables
//     ICANON/ECHO/ISIG so input arrives byte-at-a-time, unechoed, and Ctrl-C
//     is just a byte (0x03) instead of a signal — the terminal is restored
//     on OUR terms (normal exit or a panic hook), never left in raw mode by
//     an uncaught SIGINT.
//   - Encoding: `frame_bytes` is a PURE function (no I/O, no statics) that
//     turns a Cell grid into an ANSI byte stream, diffing against the
//     previous frame when given one. This purity is what makes it testable
//     and what makes `--render-frame` (see `render_frame_main`) safe to
//     pipe to a file with no termios/alt-screen side effects at all.

use crate::game::{COLS, Game, ROWS};
use crate::headless::world_hash;
use crate::render::{CELLS, Cell, render_cells};
use crate::rng::h64;
use crate::save::{INPUT_RESTART, save_bytes, save_filename};
use std::sync::OnceLock;

// ---------- termios FFI (Linux x86_64 layout) ----------

#[repr(C)]
#[derive(Clone, Copy)]
struct Termios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_line: u8,
    c_cc: [u8; 32],
    c_ispeed: u32,
    c_ospeed: u32,
}

extern "C" {
    fn tcgetattr(fd: i32, termios_p: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn write(fd: i32, buf: *const u8, count: usize) -> isize;
}

const ICANON: u32 = 0x2;
const ECHO: u32 = 0x8;
const ISIG: u32 = 0x1;
const TCSANOW: i32 = 0;
// Indices into c_cc, not flag bits: c_cc[VMIN]/c_cc[VTIME] set the blocking
// read policy (POSIX termios.h names these the same way).
const VMIN: usize = 6;
const VTIME: usize = 5;
const STDIN_FD: i32 = 0;
const STDOUT_FD: i32 = 1;

/// Original termios, captured once on entry to raw mode so both the normal
/// exit path and the panic hook can restore it. Never overwritten after the
/// first `enter_raw_mode` call in a process.
static ORIG_TERMIOS: OnceLock<Termios> = OnceLock::new();

fn get_termios() -> Termios {
    let mut t: Termios = unsafe { std::mem::zeroed() };
    unsafe { tcgetattr(STDIN_FD, &mut t) };
    t
}

fn set_termios(t: &Termios) {
    unsafe { tcsetattr(STDIN_FD, TCSANOW, t) };
}

/// Write the full buffer via the raw fd, looping past partial writes.
fn raw_write(bytes: &[u8]) {
    let mut off = 0;
    while off < bytes.len() {
        let n = unsafe { write(STDOUT_FD, bytes[off..].as_ptr(), bytes.len() - off) };
        if n <= 0 {
            break;
        }
        off += n as usize;
    }
}

/// Read exactly one byte from the raw fd, honoring whatever VMIN/VTIME is
/// currently set. Returns None on EOF/error (VMIN=0 timeout, or stdin
/// closed) — callers decide what "no byte" means in context.
fn raw_read_byte() -> Option<u8> {
    let mut b = [0u8; 1];
    let n = unsafe { read(STDIN_FD, b.as_mut_ptr(), 1) };
    if n == 1 { Some(b[0]) } else { None }
}

/// Enter raw mode: ICANON/ECHO/ISIG off (unechoed, byte-at-a-time, no
/// signal generation — Ctrl-C arrives as 0x03, handled as quit so we always
/// restore on our own terms), VMIN=1/VTIME=0 (blocking single-byte reads).
/// Stashes the original termios in `ORIG_TERMIOS` for restore, and enters
/// the alt screen with the cursor hidden. Returns the raw termios in effect
/// so callers can toggle VMIN/VTIME (for escape-sequence lookahead) without
/// re-querying tcgetattr.
fn enter_raw_mode() -> Termios {
    let orig = get_termios();
    let _ = ORIG_TERMIOS.set(orig);
    let mut raw = orig;
    raw.c_lflag &= !(ICANON | ECHO | ISIG);
    raw.c_cc[VMIN] = 1;
    raw.c_cc[VTIME] = 0;
    set_termios(&raw);
    raw_write(b"\x1b[?1049h\x1b[?25l");
    raw
}

/// Restore the terminal to its pre-raw-mode state: original termios, leave
/// the alt screen, show the cursor, reset SGR, trailing newline so the
/// shell prompt doesn't land mid-line. Idempotent-ish (a no-op on termios if
/// `ORIG_TERMIOS` was never set, e.g. called from a panic before raw mode
/// was entered) and safe to call twice (normal exit, then — if a panic
/// somehow still occurred — the panic hook).
fn restore_terminal() {
    if let Some(orig) = ORIG_TERMIOS.get() {
        set_termios(orig);
    }
    raw_write(b"\x1b[?1049l\x1b[?25h\x1b[0m\n");
}

/// Chain onto the previous panic hook so the terminal is always restored
/// before a panic message prints — otherwise a mid-run panic leaves the
/// terminal in alt-screen raw mode with the backtrace invisible.
fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        prev(info);
    }));
}

/// Toggle VMIN/VTIME on the raw-mode base termios without touching
/// ICANON/ECHO/ISIG. Used to switch between blocking single-byte reads
/// (the normal case) and a short-timeout read used to peek for the rest of
/// an escape sequence after a lone ESC byte.
fn set_timing(mut base: Termios, vmin: u8, vtime: u8) {
    base.c_cc[VMIN] = vmin;
    base.c_cc[VTIME] = vtime;
    set_termios(&base);
}

// ---------- input mapping ----------

/// One interpreted input event. `Move` carries the apply_input byte
/// (0=N,1=S,2=W,3=E) directly so callers don't re-derive it.
enum Input {
    Move(u8),
    Wait,
    Restart,
    Save,
    Info,
    Quit,
    Ignore,
}

/// Read a possible escape-sequence follow-up byte under the VTIME=1/VMIN=0
/// timing already installed by the caller. None means the 100ms timeout
/// elapsed with nothing to read.
fn read_esc_byte() -> Option<u8> {
    raw_read_byte()
}

/// Interpret the bytes after a lone ESC (0x1b) has already been consumed.
/// A timeout with no follow-up byte is a lone ESC — quit. Recognizes arrow
/// keys (ESC [ A/B/C/D), F5 (ESC [ 1 5 ~), and F1 (ESC O P or ESC [ 1 1 ~);
/// anything else is ignored (but still drains what it can so a partial,
/// unrecognized sequence doesn't leak bytes into the next read).
fn read_escape_seq() -> Input {
    let Some(b1) = read_esc_byte() else { return Input::Quit };
    match b1 {
        b'[' => match read_esc_byte() {
            Some(b'A') => Input::Move(0),
            Some(b'B') => Input::Move(1),
            Some(b'D') => Input::Move(2),
            Some(b'C') => Input::Move(3),
            Some(b'1') => match read_esc_byte() {
                Some(b'1') => {
                    read_esc_byte(); // trailing '~'
                    Input::Info
                }
                Some(b'5') => {
                    read_esc_byte(); // trailing '~'
                    Input::Save
                }
                _ => Input::Ignore,
            },
            _ => Input::Ignore,
        },
        b'O' => match read_esc_byte() {
            Some(b'P') => Input::Info,
            _ => Input::Ignore,
        },
        _ => Input::Ignore,
    }
}

/// Blocking-read one input byte and interpret it. `raw` is the raw-mode
/// base termios (VMIN=1/VTIME=0); on an ESC byte this switches to a
/// VMIN=0/VTIME=1 lookahead for the rest of the sequence and switches back
/// before returning. wasd/hjkl/arrows move, '.' waits, 'r' requests a
/// restart (gated on dead/won by the caller, matching the minifb backend),
/// 'q'/lone-ESC/Ctrl-C quit. Unknown bytes are ignored.
fn read_input(raw: Termios) -> Input {
    let Some(b) = raw_read_byte() else { return Input::Quit }; // EOF (stdin closed)
    match b {
        0x03 => Input::Quit, // Ctrl-C; ISIG is off so no SIGINT is generated
        0x1b => {
            set_timing(raw, 0, 1);
            let ev = read_escape_seq();
            set_timing(raw, 1, 0);
            ev
        }
        b'q' => Input::Quit,
        b'w' | b'k' => Input::Move(0),
        b's' | b'j' => Input::Move(1),
        b'a' | b'h' => Input::Move(2),
        b'd' | b'l' => Input::Move(3),
        b'.' => Input::Wait,
        b'r' => Input::Restart,
        _ => Input::Ignore,
    }
}

// ---------- pure encoder ----------

/// 6x6x6 color cube + grayscale ramp, mapping a 24-bit RGB color down to an
/// xterm 256-color palette index. Near-gray colors (channel spread < 16)
/// route through the finer 24-step grayscale ramp instead of the coarse
/// cube, since the cube's gray-ish corners are sparse and banding badly.
pub(crate) fn c256(rgb: u32) -> u8 {
    let r = ((rgb >> 16) & 0xFF) as i32;
    let g = ((rgb >> 8) & 0xFF) as i32;
    let b = (rgb & 0xFF) as i32;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    if mx - mn < 16 {
        let v = (r + g + b) / 3;
        return if v < 8 {
            16
        } else if v > 238 {
            231
        } else {
            (232 + (v - 8) / 10) as u8
        };
    }
    fn q(c: i32) -> i32 {
        if c < 48 {
            0
        } else if c < 115 {
            1
        } else {
            (c - 35) / 40
        }
    }
    (16 + 36 * q(r) + 6 * q(g) + q(b)) as u8
}

/// Encode a Cell grid as an ANSI byte stream. `prev = None` is a full
/// redraw (`\x1b[2J\x1b[H` then every cell); `prev = Some(..)` emits
/// escapes only for cells that changed (dirty-cell discipline) — this is
/// the whole point of carrying a previous frame at all. Cursor position and
/// last-emitted fg/bg are tracked across the whole grid so a run of
/// adjacent, same-colored cells costs nothing beyond the raw glyph bytes:
/// no repeated cursor-address or SGR escapes. `ascii` replaces any glyph
/// >=128 with '#' so the output stays pure 7-bit.
///
/// Pure: no I/O, no statics, deterministic in cells+prev+ascii alone. This
/// is the verification surface — the interactive loop and `--render-frame`
/// both just write whatever this returns.
pub(crate) fn frame_bytes(cells: &[Cell], prev: Option<&[Cell]>, ascii: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let full = prev.is_none();
    if full {
        out.extend_from_slice(b"\x1b[2J\x1b[H");
    }
    // (row, col), both 1-based, matching \x1b[{row};{col}H addressing.
    let mut cur_pos: Option<(i32, i32)> = if full { Some((1, 1)) } else { None };
    let mut last_fg: Option<u8> = None;
    let mut last_bg: Option<u8> = None;

    for row in 0..ROWS {
        for col in 0..COLS {
            let i = row * COLS + col;
            let cell = cells[i];
            if let Some(p) = prev {
                if cell == p[i] {
                    continue;
                }
            }
            let want = ((row + 1) as i32, (col + 1) as i32);
            if cur_pos != Some(want) {
                out.extend_from_slice(format!("\x1b[{};{}H", want.0, want.1).as_bytes());
            }
            let fg = c256(cell.fg);
            let bg = c256(cell.bg);
            if last_fg != Some(fg) {
                out.extend_from_slice(format!("\x1b[38;5;{}m", fg).as_bytes());
                last_fg = Some(fg);
            }
            if last_bg != Some(bg) {
                out.extend_from_slice(format!("\x1b[48;5;{}m", bg).as_bytes());
                last_bg = Some(bg);
            }
            let mut ch = cell.ch;
            if ascii && ch >= 128 {
                ch = b'#' as u16;
            }
            if ch < 128 {
                out.push(ch as u8);
            } else if let Some(c) = char::from_u32(ch as u32) {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
            cur_pos = Some((want.0, want.1 + 1));
        }
    }
    out
}

// ---------- headless entry point ----------

/// `--render-frame`: render one initial frame (Game::new(seed), turn 0) and
/// write its full-redraw byte stream to stdout, then return. No termios
/// calls, no alt screen — this is the frame-golden verification surface, so
/// it must work identically whether stdout is a real terminal or a
/// redirected file.
pub(crate) fn render_frame_main(seed: u64, ascii: bool) {
    let game = Game::new(seed);
    let mut cells = vec![Cell { ch: b' ' as u16, fg: 0, bg: 0 }; CELLS];
    render_cells(&game, &mut cells);
    let bytes = frame_bytes(&cells, None, ascii);
    raw_write(&bytes);
}

// ---------- interactive loop ----------

/// Run the terminal window loop: blocking read -> input event -> apply to
/// Game exactly as backend_minifb does (input_log push, apply_input,
/// confirm_armed discipline) -> render_cells -> frame_bytes(dirty diff) ->
/// write. First frame is a full redraw. `seed0`/`input_log`/`game` come
/// from either a fresh seed or a `--load`ed save, same as backend_minifb.
/// `ascii` is the `--ascii` flag; term-only, so it's an extra trailing
/// param rather than threading through backend_minifb's signature (the two
/// backend dispatch call sites in main.rs are already independently
/// cfg-gated, so this doesn't touch backend_minifb at all).
pub(crate) fn run(seed0: u64, mut input_log: Vec<u8>, mut game: Game, _daily: bool, _day: u64, ascii: bool) {
    let mut whash = world_hash(game.seed);
    let raw = enter_raw_mode();
    install_panic_hook();

    let mut cells = vec![Cell { ch: b' ' as u16, fg: 0, bg: 0 }; CELLS];
    let mut confirm_armed = false;

    render_cells(&game, &mut cells);
    raw_write(&frame_bytes(&cells, None, ascii));
    let mut prev = cells.clone();

    loop {
        match read_input(raw) {
            Input::Quit => break,
            Input::Move(b) => {
                input_log.push(b);
                game.apply_input(b);
                confirm_armed = false;
            }
            Input::Wait => {
                input_log.push(4);
                game.apply_input(4);
                confirm_armed = false;
            }
            // Restart only fires once the run is over, matching the minifb
            // backend's `(game.dead || game.won) && ...` gate.
            Input::Restart if game.dead || game.won => {
                input_log.push(INPUT_RESTART);
                let s = h64(game.seed, &["restart"]);
                game = Game::new(s);
                whash = world_hash(s);
                // No window title in a terminal — log the world identity
                // instead, same info the minifb backend puts in the title.
                game.log(format!("Seed {}  world {:016x}", s, whash));
                confirm_armed = false;
            }
            Input::Restart => {}
            // F5 double-press overwrite confirm, identical to backend_minifb.
            Input::Save => {
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
            // F1: identify the world. Log-only — consumes no turn, no input
            // byte, and touches no RNG channel, so replay is unaffected.
            Input::Info => {
                game.log(format!("Seed {}  world {:016x}", game.seed, whash));
            }
            Input::Ignore => {}
        }
        render_cells(&game, &mut cells);
        raw_write(&frame_bytes(&cells, Some(&prev), ascii));
        prev.copy_from_slice(&cells);
    }

    restore_terminal();

    // Autosave on quit: only for a still-live run with unsaved progress.
    // Never clobber a manual save — if the hashed filename already exists,
    // fall back to a .auto.sav sibling. Terminal is restored, so print,
    // don't log.
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

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        needle.len() <= hay.len() && hay.windows(needle.len()).any(|w| w == needle)
    }

    /// Count ANSI cursor-move escapes (`\x1b[{row};{col}H`), distinct from
    /// SGR (`...m`) or clear-screen (`\x1b[2J`) sequences that also start
    /// with `\x1b[`.
    fn count_cursor_moves(bytes: &[u8]) -> usize {
        let mut n = 0;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                let mut j = i + 2;
                while j < bytes.len() && !bytes[j].is_ascii_alphabetic() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'H' {
                    n += 1;
                }
                i = j + 1;
            } else {
                i += 1;
            }
        }
        n
    }

    /// c256 vectors: pure black -> palette 16, pure white -> 231, pure red
    /// -> 196 (all captured by hand from the 6x6x6 cube math), and one
    /// mid-gray landing in the 232..=255 grayscale ramp.
    #[test]
    fn c256_vectors() {
        assert_eq!(c256(0x000000), 16);
        assert_eq!(c256(0xFFFFFF), 231);
        assert_eq!(c256(0xFF0000), 196);
        assert_eq!(c256(0x808080), 244); // gray: spread < 16, hits the ramp
    }

    /// Two CELLS-sized grids differing in exactly one cell: exactly one
    /// cursor-move escape, no clear-screen. Identical grids: no cursor
    /// moves and no clear-screen either (empty or SGR-only).
    #[test]
    fn dirty_diff_single_cell() {
        let base = vec![Cell { ch: b' ' as u16, fg: 0, bg: 0 }; CELLS];
        let mut changed = base.clone();
        changed[42] = Cell { ch: b'@' as u16, fg: 0xFFFFFF, bg: 0 };

        let out = frame_bytes(&changed, Some(&base), false);
        assert_eq!(count_cursor_moves(&out), 1);
        assert!(!contains(&out, b"\x1b[2J"));

        let same = frame_bytes(&base, Some(&base), false);
        assert_eq!(count_cursor_moves(&same), 0);
        assert!(!contains(&same, b"\x1b[2J"));
    }

    /// ascii=true maps any glyph >=128 to '#', so the whole byte stream
    /// stays 7-bit even when the grid contains a box-drawing codepoint.
    #[test]
    fn ascii_mode_no_high_bytes() {
        let mut cells = vec![Cell { ch: b' ' as u16, fg: 0, bg: 0 }; CELLS];
        cells[10] = Cell { ch: 0x2500, fg: 0xFFFFFF, bg: 0 };
        let out = frame_bytes(&cells, None, true);
        assert!(out.iter().all(|&b| b < 0x80));
    }

    /// frame_bytes is pure: same cells, same output, every time.
    #[test]
    fn full_frame_deterministic() {
        let mut cells = vec![Cell { ch: b' ' as u16, fg: 0, bg: 0 }; CELLS];
        cells[5] = Cell { ch: b'#' as u16, fg: 0x123456, bg: 0x000000 };
        let a = frame_bytes(&cells, None, false);
        let b = frame_bytes(&cells, None, false);
        assert_eq!(a, b);
    }
}
