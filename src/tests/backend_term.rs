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

// ---- camera console: row allocation (split_rows) ----

#[test]
fn split_rows_full_console_is_full_map_and_log() {
    assert_eq!(split_rows(ROWS), (MAP_H, ROWS - MAP_H - 1)); // (25, 4)
    assert_eq!(split_rows(ROWS + 5), (MAP_H, ROWS - MAP_H - 1)); // clamps down
}

#[test]
fn split_rows_classic_80x25_keeps_full_log_scrolls_map() {
    // 25 rows = 1 status + 4 log + 20 map viewport
    assert_eq!(split_rows(25), (20, 4));
}

#[test]
fn split_rows_short_console_floors_map_then_shrinks_log() {
    assert_eq!(split_rows(10), (5, 4)); // map floor 5, log still full
    assert_eq!(split_rows(8), (5, 2));
    assert_eq!(split_rows(7), (5, 1)); // one log line kept at the floor
    assert_eq!(split_rows(6), (4, 1)); // below the floor: map yields, log stays 1
    assert_eq!(split_rows(2), (1, 0)); // status + a single map row
    assert_eq!(split_rows(1), (0, 0)); // status only
}

// ---- camera console: dead-zone scroll (scroll_axis) ----

#[test]
fn scroll_axis_no_scroll_when_view_covers_map() {
    assert_eq!(scroll_axis(0, 40, 80, 80, 4), 0);
    assert_eq!(scroll_axis(0, 40, 100, 80, 4), 0);
}

#[test]
fn scroll_axis_holds_inside_dead_zone() {
    // view 20, margin 4 -> dead zone [o+4, o+15]; focus 10 at o=5 is inside
    assert_eq!(scroll_axis(5, 10, 20, 80, 4), 5);
}

#[test]
fn scroll_axis_follows_when_focus_pushes_margin() {
    assert_eq!(scroll_axis(5, 6, 20, 80, 4), 2); // pushed into left margin
    assert_eq!(scroll_axis(5, 25, 20, 80, 4), 10); // pushed past right margin
}

#[test]
fn scroll_axis_clamps_at_map_edges() {
    assert_eq!(scroll_axis(55, 79, 20, 80, 4), 60); // right edge = map - view
    assert_eq!(scroll_axis(2, 0, 20, 80, 4), 0); // left edge
}

// ---- camera console: composition (viewport) ----

/// A full 80x30 buffer whose every cell's glyph encodes its flat index,
/// so a windowed view's mapping back to source cells is exactly assertable.
fn indexed_full() -> Vec<Cell> {
    (0..CELLS)
        .map(|i| Cell { ch: i as u16, fg: 0, bg: 0 })
        .collect()
}

#[test]
fn viewport_identity_at_full_size() {
    let full = indexed_full();
    let (v, w, h, origin) = viewport(&full, true, COLS, ROWS, (10, 10), (0, 0));
    assert_eq!((w, h), (COLS, ROWS));
    assert_eq!(origin, (0, 0));
    assert!(v == full); // byte-for-byte the same grid
}

#[test]
fn viewport_clamps_oversized_terminal_to_grid() {
    let full = indexed_full();
    let (v, w, h, _) = viewport(&full, true, 200, 60, (0, 0), (0, 0));
    assert_eq!((w, h), (COLS, ROWS));
    assert_eq!(v.len(), CELLS);
}

#[test]
fn viewport_80x25_scrolls_map_pins_status_keeps_log() {
    let full = indexed_full();
    // focus near mid-map forces a vertical scroll; split_rows(25)=(20 map, 4 log)
    let (v, w, h, origin) = viewport(&full, true, 80, 25, (40, 20), (0, 0));
    assert_eq!((w, h), (80, 25));
    assert_eq!(origin, (0, 5)); // oy from scroll_axis(0,20,20,25,4)
    // map row 0, col 0 comes from source row oy=5
    assert_eq!(v[0].ch, (5 * COLS) as u16);
    // status row (index 20) is the pinned full status row MAP_H
    assert_eq!(v[20 * 80].ch, (MAP_H * COLS) as u16);
    // last visible log row (index 24) is the newest source log row (ROWS-1)
    assert_eq!(v[24 * 80].ch, ((ROWS - 1) * COLS) as u16);
}

#[test]
fn viewport_non_play_crops_top_left() {
    let full = indexed_full();
    let (v, w, h, _) = viewport(&full, false, 40, 10, (0, 0), (0, 0));
    assert_eq!((w, h), (40, 10));
    assert_eq!(v[0].ch, 0);
    assert_eq!(v[9 * 40 + 39].ch, (9 * COLS + 39) as u16); // bottom-right of crop
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

    let out = frame_bytes(&changed, COLS, ROWS, Some(&base), false);
    assert_eq!(count_cursor_moves(&out), 1);
    assert!(!contains(&out, b"\x1b[2J"));

    let same = frame_bytes(&base, COLS, ROWS, Some(&base), false);
    assert_eq!(count_cursor_moves(&same), 0);
    assert!(!contains(&same, b"\x1b[2J"));
}

/// ascii=true maps any glyph >=128 to a 7-bit stand-in, so the whole
/// byte stream stays 7-bit even when the grid contains a box-drawing
/// codepoint.
#[test]
fn ascii_mode_no_high_bytes() {
    let mut cells = vec![Cell { ch: b' ' as u16, fg: 0, bg: 0 }; CELLS];
    cells[10] = Cell { ch: 0x2500, fg: 0xFFFFFF, bg: 0 };
    let out = frame_bytes(&cells, COLS, ROWS, None, true);
    assert!(out.iter().all(|&b| b < 0x80));
}

/// The specific ascii>=128 mapping: block glyphs -> '#'/'-', any
/// box-drawing codepoint -> '#', anything else unmapped -> '?'.
#[test]
fn ascii_glyph_fallback_mapping() {
    assert_eq!(ascii_fallback(0x2588), b'#' as u16); // bar: filled block
    assert_eq!(ascii_fallback(0x2591), b'-' as u16); // bar: empty block
    assert_eq!(ascii_fallback(0x2500), b'#' as u16); // box-drawing: wall/border
    assert_eq!(ascii_fallback(0x253C), b'#' as u16); // box-drawing: cross (top of range)
    assert_eq!(ascii_fallback(0x1234), b'?' as u16); // unmapped high codepoint
}

/// frame_bytes is pure: same cells, same output, every time.
#[test]
fn full_frame_deterministic() {
    let mut cells = vec![Cell { ch: b' ' as u16, fg: 0, bg: 0 }; CELLS];
    cells[5] = Cell { ch: b'#' as u16, fg: 0x123456, bg: 0x000000 };
    let a = frame_bytes(&cells, COLS, ROWS, None, false);
    let b = frame_bytes(&cells, COLS, ROWS, None, false);
    assert_eq!(a, b);
}
