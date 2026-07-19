// save.rs — state-is-deltas layer: a save is just the seed plus the input
// log, so persistence is serializing that pair and reconstruction is
// replaying it through Game::apply_input. Also the state hash used to prove
// a replay reproduced the live run exactly. Batch 4 task 2 (save v2, per
// DECISION.md sign-off item 2) adds the ghost file format (RLG1) here too:
// like a save, a ghost is just bytes describing a run, built by pure
// functions with zero I/O — the backends/main own writing the file.

use crate::game::{Game, Item, Monster, Tile};
use crate::rng::{fnv_bytes, h64};

// ---------- Save / replay: state is deltas (seed + input log) ----------
/* A save is the original seed plus one byte per input; the world is
   reconstructed by replaying. Byte format, no serde:
     "RL14" | version u8 (=1 or 2) | seed u64 LE | input bytes...
   Inputs: 0=N 1=S 2=W 3=E 4=wait 5=restart(reroll to a new seed)
   6=retry(same seed, save v2 — see INPUT_RETRY). Tens of bytes per save.
   `save_bytes` always writes v2; `parse_save` accepts v1 or v2 — a v1 log
   only ever contains bytes 0-5, so it replays byte-identical either way
   (see the `v1_save_replays_under_v2_parsing` test in main.rs). */
const SAVE_MAGIC: &[u8; 4] = b"RL14";
const SAVE_VERSION: u8 = 2;
pub(crate) const INPUT_RESTART: u8 = 5;
/// Save v2 (batch 4 task 2, DECISION.md sign-off item 2): reconstruct
/// `Game::new(g.seed)` — same world, next attempt — instead of rerolling
/// like INPUT_RESTART. Handled in `replay()` exactly where INPUT_RESTART is
/// handled: it's a reconstruction byte, not an `apply_input` byte (that
/// match's `_ => {}` arm still ignores everything >= 5).
pub(crate) const INPUT_RETRY: u8 = 6;

pub(crate) fn save_bytes(seed0: u64, inputs: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(13 + inputs.len());
    out.extend_from_slice(SAVE_MAGIC);
    out.push(SAVE_VERSION);
    out.extend_from_slice(&seed0.to_le_bytes());
    out.extend_from_slice(inputs);
    out
}

pub(crate) fn parse_save(bytes: &[u8]) -> Option<(u64, Vec<u8>)> {
    if bytes.len() < 13 || &bytes[..4] != SAVE_MAGIC || (bytes[4] != 1 && bytes[4] != 2) {
        return None;
    }
    let mut s = [0u8; 8];
    s.copy_from_slice(&bytes[5..13]);
    Some((u64::from_le_bytes(s), bytes[13..].to_vec()))
}

/// Save filename is derived from the world hash, not the seed, so it
/// identifies the generated world (`--seed`/`--daily`/restart all funnel
/// through `whash`). One place for the format so F5 and autosave agree.
pub(crate) fn save_filename(whash: u64) -> String {
    format!("rl144-{:016x}.sav", whash)
}

/// Reconstruct a game by replaying inputs from the original seed. Restart
/// bytes (5) re-derive the seed exactly as the N key does (reroll); retry
/// bytes (6, save v2) reconstruct the SAME seed exactly as the R key does,
/// and additionally set `echo` from where the just-ended attempt died — but
/// only if it actually ended dead (a retry logged after a win or mid-run,
/// which shouldn't happen from either backend's UI but is defensively
/// handled the same way for a hand-built/replayed log, leaves `echo` at its
/// `Game::new` default of `None`).
pub(crate) fn replay(seed0: u64, inputs: &[u8]) -> Game {
    let mut g = Game::new(seed0);
    for &b in inputs {
        match b {
            INPUT_RESTART => {
                g = Game::new(h64(g.seed, &["restart"]));
            }
            INPUT_RETRY => {
                let echo = if g.dead { Some((g.px, g.py, g.depth)) } else { None };
                let seed = g.seed;
                g = Game::new(seed);
                g.echo = echo;
            }
            _ => g.apply_input(b),
        }
    }
    g
}

/// FNV-1a over a canonical serialization of everything that defines the run:
/// player, light, flags, RNG channel states, live level, and every stashed
/// level. If two replays of one save ever hash differently, channel
/// discipline broke somewhere.
///
/// `g.killer`, `g.echo`, `g.facing`, and `g.fx_hit` are all deliberately
/// NOT hashed: every one of them is presentation-only (the End screen's
/// cause-of-death line; the retry-echo tile; the player sprite's facing;
/// the screen-feel flash/squash tile, respectively), none affects anything
/// replay needs to reproduce, and each is fully determined by state that
/// IS hashed anyway (the same move/attack/death sequence that produces
/// `dead`/`px`/`py`/the monster-hp deltas). See each field's own doc
/// comment in `game.rs` for the field-specific rationale; this is the one
/// place that enumerates them together as a set.
pub(crate) fn state_hash(g: &Game) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for v in [
        g.seed,
        g.depth as u64,
        g.px as u64,
        g.py as u64,
        g.hp as u64,
        g.maxhp as u64,
        g.atk as u64,
        g.light as u64,
        g.kills as u64,
        g.turns as u64,
        g.has_amulet as u64,
        g.dead as u64,
        g.won as u64,
        g.combat_rng.0,
        g.ai_rng.0,
        g.flavor_rng.0,
    ] {
        h = fnv_bytes(h, &v.to_le_bytes());
    }
    let level = |h: u64, map: &[Tile], monsters: &[Monster], items: &[Item]| -> u64 {
        let mut h = h;
        for t in map {
            h = fnv_bytes(h, &[*t as u8]);
        }
        for m in monsters {
            h = fnv_bytes(h, &[m.x as u8, m.y as u8, m.kind as u8, m.hp as u8]);
        }
        for it in items {
            h = fnv_bytes(h, &[it.x as u8, it.y as u8, it.kind as u8]);
        }
        h
    };
    h = level(h, &g.map, &g.monsters, &g.items);
    for s in &g.saved {
        match s {
            Some(ls) => {
                h = fnv_bytes(h, &[1]);
                h = level(h, &ls.map, &ls.monsters, &ls.items);
            }
            None => h = fnv_bytes(h, &[0]),
        }
    }
    h
}

// ---------- Ghost files: "RLG1" (batch 4 task 2, substrate only) ----------
/* A ghost is runtime user data describing one ended attempt, sibling to a
   `.sav`: magic(4) "RLG1" + version u8(=1) + seed u64 LE + world_hash u64
   LE + outcome u8 + final_depth u8 + turns u32 LE + label_idx u8 (index
   into content::GHOST_LABELS) + the (trimmed) input log for the rest of the
   file. Header is 28 bytes. Pure builders/parsers only — no I/O here; the
   backends own writing/reading the `.ghost` file (see
   `backend_minifb.rs`/`backend_term.rs`'s `write_ghost`). Phase 1 (this
   task) is substrate only: no ghost RENDERING or playback yet (that's a
   later task), so nothing here is exercised by `--dump`/goldens. */
const GHOST_MAGIC: &[u8; 4] = b"RLG1";
const GHOST_VERSION: u8 = 1;
const GHOST_HEADER_LEN: usize = 28;

pub(crate) const GHOST_DIED_COMBAT: u8 = 0;
pub(crate) const GHOST_DIED_DARK: u8 = 1;
pub(crate) const GHOST_WON: u8 = 2;
pub(crate) const GHOST_ABANDONED: u8 = 3;

/// Phase-1 trim rule (defensive only, not a live code path yet): cap the
/// logged input length. Real "truncate after the input that produced the
/// terminal state" trimming has nothing to do today because every capture
/// point (see the backends) only ever hands `ghost_bytes` the just-ended
/// attempt's log, which already stops at the input that caused death — no
/// frontend pushes anything after that. This cap is pure insurance against
/// a pathologically long log becoming a pathologically large ghost file,
/// not something the current call sites can actually trigger.
const GHOST_MAX_LOG: usize = 6000;

fn trim_ghost_log(inputs: &[u8]) -> &[u8] {
    &inputs[..inputs.len().min(GHOST_MAX_LOG)]
}

/// A parsed ghost file: everything `parse_ghost` extracts from an `RLG1`
/// blob. `inputs` is the (already-trimmed-on-write) log to replay through
/// the same `apply_input`/`replay` machinery the player uses — a ghost adds
/// zero new simulation code, see B-haunted-floppy.md §4.
///
/// `#[allow(dead_code)]`: this task (batch 4 task 2) is Phase 1 substrate
/// only — the roadmap's own words are "no ghost RENDERING yet; that's
/// T3/Phase 4." `parse_ghost`/`Ghost` exist now so the file format is
/// frozen and round-trip-tested (`ghost_bytes_round_trip` in main.rs), but
/// their first real caller (`--watch`/ghost playback) is a later task.
/// `ghost_bytes` (the write side) IS live today via each backend's
/// `write_ghost`, so it needs no such allow.
#[allow(dead_code)]
pub(crate) struct Ghost {
    pub(crate) seed: u64,
    pub(crate) world_hash: u64,
    pub(crate) outcome: u8,
    pub(crate) final_depth: u8,
    pub(crate) turns: u32,
    pub(crate) label_idx: u8,
    pub(crate) inputs: Vec<u8>,
}

/// Build an `RLG1` byte blob. `label_idx` is computed by the caller (see
/// `content::ghost_label_idx` — deterministic from outcome/depth, no new
/// RNG channel). `inputs` is trimmed to `GHOST_MAX_LOG` bytes (see
/// `trim_ghost_log`) before being written.
pub(crate) fn ghost_bytes(
    seed: u64,
    whash: u64,
    outcome: u8,
    final_depth: u8,
    turns: u32,
    label_idx: u8,
    inputs: &[u8],
) -> Vec<u8> {
    let trimmed = trim_ghost_log(inputs);
    let mut out = Vec::with_capacity(GHOST_HEADER_LEN + trimmed.len());
    out.extend_from_slice(GHOST_MAGIC);
    out.push(GHOST_VERSION);
    out.extend_from_slice(&seed.to_le_bytes());
    out.extend_from_slice(&whash.to_le_bytes());
    out.push(outcome);
    out.push(final_depth);
    out.extend_from_slice(&turns.to_le_bytes());
    out.push(label_idx);
    out.extend_from_slice(trimmed);
    out
}

/// Parse an `RLG1` byte blob built by `ghost_bytes`. `None` on a bad magic,
/// unknown version, an unknown outcome byte (must be one of the 4 GHOST_*
/// consts), or a blob shorter than the fixed header.
///
/// `#[allow(dead_code)]`: see `Ghost`'s doc comment — Phase 1 substrate,
/// no live caller until ghost playback lands (later task). Exercised today
/// by `ghost_bytes_round_trip` (main.rs), gated with `#[cfg(test)]` so it
/// doesn't count as the crate's "real" reachability root.
#[allow(dead_code)]
pub(crate) fn parse_ghost(bytes: &[u8]) -> Option<Ghost> {
    if bytes.len() < GHOST_HEADER_LEN || &bytes[..4] != GHOST_MAGIC || bytes[4] != GHOST_VERSION {
        return None;
    }
    let mut s = [0u8; 8];
    s.copy_from_slice(&bytes[5..13]);
    let seed = u64::from_le_bytes(s);
    let mut w = [0u8; 8];
    w.copy_from_slice(&bytes[13..21]);
    let world_hash = u64::from_le_bytes(w);
    let outcome = bytes[21];
    if !matches!(outcome, GHOST_DIED_COMBAT | GHOST_DIED_DARK | GHOST_WON | GHOST_ABANDONED) {
        return None;
    }
    let final_depth = bytes[22];
    let mut t = [0u8; 4];
    t.copy_from_slice(&bytes[23..27]);
    let turns = u32::from_le_bytes(t);
    let label_idx = bytes[27];
    Some(Ghost {
        seed,
        world_hash,
        outcome,
        final_depth,
        turns,
        label_idx,
        inputs: bytes[GHOST_HEADER_LEN..].to_vec(),
    })
}

/// Ghost filename is derived from the world hash, same convention as
/// `save_filename`: latest death per world overwrites (only one ghost is
/// kept per world in Phase 1).
pub(crate) fn ghost_filename(whash: u64) -> String {
    format!("rl144-{:016x}.ghost", whash)
}
