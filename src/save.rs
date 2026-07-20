// save.rs — state-is-deltas layer: a save is just the seed plus the input
// log, so persistence is serializing that pair and reconstruction is
// replaying it through Game::apply_input. Also the state hash used to prove
// a replay reproduced the live run exactly. Batch 4 task 2 (save v2, per
// DECISION.md sign-off item 2) adds the ghost file format (RLG1) here too:
// like a save, a ghost is just bytes describing a run, built by pure
// functions with zero I/O — the backends/main own writing the file.

use crate::game::{Dest, Game, Item, Monster, Tile, WorldId};
use crate::rng::{fnv_bytes, h64};

// ---------- Save / replay: state is deltas (seed + input log) ----------
/* A save is the original seed plus one byte per input; the world is
   reconstructed by replaying. Byte format, no serde:
     "RL14" | version u8 (=1, 2, 3, or 4) | seed u64 LE | input bytes...
   Inputs: 0=N 1=S 2=W 3=E 4=wait 5=restart(reroll to a new seed)
   6=retry(same seed, save v2 — see INPUT_RETRY) 7=talk-N 8=talk-S 9=talk-W
   10=talk-E (save v3, batch 5, DECISION.md item 3 — the Henson ruling;
   direction order mirrors move bytes 0-3 exactly) 11=give-N 12=give-S
   13=give-W 14=give-E 15=use (save v4, batch 7 T2, story §5/§9-A; give's
   direction order mirrors talk's/move's exactly, see
   `game::Game::apply_input`). Tens of bytes per save. `save_bytes` always
   writes v4; `parse_save` accepts v1, v2, v3, or v4 — a v1/v2/v3 log never
   contains bytes 11-15 (give/use didn't exist yet), so it replays byte-
   identical under v4 parsing either way (see the
   `v1_save_replays_under_v4_parsing`/`v2_save_replays_under_v4_parsing`/
   `v3_save_replays_under_v4_parsing` tests in main.rs, and `make xhash`,
   whose fixture is a v1 blob). This is a version bump rather than a silent
   superset precisely so an OLD binary (whose own `parse_save` only ever
   accepted up to v3) REJECTS a v4 save cleanly instead of silently
   ignoring give/use bytes and diverging from what was actually played —
   same discipline the v2->v3 talk bump established. */
const SAVE_MAGIC: &[u8; 4] = b"RL14";
const SAVE_VERSION: u8 = 4;
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
    if bytes.len() < 13 || &bytes[..4] != SAVE_MAGIC || !(1..=4).contains(&bytes[4]) {
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
/// `WorldId` as bytes, shared by every place `state_hash` needs to fold one
/// in (batch 6 T1): current world, provenance, and every stored
/// `WorldState`'s own id.
fn hash_world_id(h: u64, w: WorldId) -> u64 {
    match w {
        WorldId::Seed(s) => fnv_bytes(fnv_bytes(h, &[0]), &s.to_le_bytes()),
        WorldId::Floor(i) => fnv_bytes(fnv_bytes(h, &[1]), &[i]),
    }
}

/// Provenance (`Game::from`/`WorldState::from`) as bytes (batch 6 T1): which
/// world a portal was entered from, plus the exact tile there, so a
/// multi-hop portal chain's return path is hashed, not just guessed at from
/// the worlds it passes through.
fn hash_provenance(h: u64, from: Option<(WorldId, i32, i32)>) -> u64 {
    match from {
        Some((id, x, y)) => {
            let h = fnv_bytes(h, &[1]);
            let h = hash_world_id(h, id);
            let h = fnv_bytes(h, &x.to_le_bytes());
            fnv_bytes(h, &y.to_le_bytes())
        }
        None => fnv_bytes(h, &[0]),
    }
}

/// A level's portal (`Game::portal`/`LevelState::portal`) as bytes (batch 6
/// T1): the tile itself is already covered by the per-tile map hash below
/// (`Tile::Portal` is a distinct discriminant), but its DESTINATION isn't —
/// that's cached state, not derivable from the tile alone, so it's hashed
/// here explicitly (see `Game::portal`'s doc comment on why it's cached).
///
/// `Dest::World`'s second field (the memoized `world_hash`, batch 6 review
/// perf fix) is deliberately NOT hashed here: it's a pure function of the
/// seed already being hashed on the line below, so it carries no
/// information the seed doesn't already — hashing it too would be
/// redundant, not more correct. See `Dest::World`'s doc comment in
/// game.rs.
fn hash_portal(h: u64, portal: Option<(i32, i32, Dest)>) -> u64 {
    match portal {
        Some((x, y, dest)) => {
            let h = fnv_bytes(h, &[1]);
            let h = fnv_bytes(h, &x.to_le_bytes());
            let h = fnv_bytes(h, &y.to_le_bytes());
            match dest {
                Dest::World(seed, _) => fnv_bytes(fnv_bytes(h, &[0]), &seed.to_le_bytes()),
                Dest::Floor(i) => fnv_bytes(fnv_bytes(h, &[1]), &[i]),
            }
        }
        None => fnv_bytes(h, &[0]),
    }
}

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
        g.has_objective as u64,
        g.dead as u64,
        g.won as u64,
        g.spared as u64,
        g.combat_rng.0,
        g.ai_rng.0,
        g.flavor_rng.0,
        g.parley_rng.0,
    ] {
        h = fnv_bytes(h, &v.to_le_bytes());
    }
    // batch 7 T2: `Game.held` (the LIFO held-items list) — order matters
    // (GIVE/USE always act on `held.last()`), so it's hashed as a
    // length-prefixed byte sequence rather than folded in unordered.
    h = fnv_bytes(h, &(g.held.len() as u64).to_le_bytes());
    for &k in &g.held {
        h = fnv_bytes(h, &[k]);
    }
    // batch 6 T1: which world is current, and where it was entered from.
    h = hash_world_id(h, g.world);
    h = hash_provenance(h, g.from);
    let level = |h: u64, map: &[Tile], monsters: &[Monster], items: &[Item], blocks: &[(i32, i32)]| -> u64 {
        let mut h = h;
        for t in map {
            h = fnv_bytes(h, &[*t as u8]);
        }
        for m in monsters {
            // regard/calm (batch 5, DECISION.md item 3) are hashed — mercy
            // is run-defining state, not presentation-only like the
            // killer/echo/facing/fx_hit exclusion set below.
            h = fnv_bytes(
                h,
                &[m.x as u8, m.y as u8, m.kind as u8, m.hp as u8, m.regard, m.calm as u8],
            );
        }
        for it in items {
            h = fnv_bytes(h, &[it.x as u8, it.y as u8, it.kind as u8]);
        }
        // Push-blocks (batch 6 T2, sokoban): position is the whole of a
        // block's state — see `Game::blocks`' doc comment on why this is
        // run-defining ("the next player on this world finds your
        // fossilized bad idea").
        for &(bx, by) in blocks {
            h = fnv_bytes(h, &[bx as u8, by as u8]);
        }
        h
    };
    h = level(h, &g.map, &g.monsters, &g.items, &g.blocks);
    h = hash_portal(h, g.portal);
    // The CURRENT world's own per-depth stash (batch 6 T1: `g.saved` is
    // scoped to whichever world `g.world` names — see that field's doc
    // comment).
    for s in &g.saved {
        match s {
            Some(ls) => {
                h = fnv_bytes(h, &[1]);
                h = level(h, &ls.map, &ls.monsters, &ls.items, &ls.blocks);
                h = hash_portal(h, ls.portal);
            }
            None => h = fnv_bytes(h, &[0]),
        }
    }
    // Every OTHER visited world, insertion order (batch 6 T1). The current
    // world's own entry in `g.worlds`, if any, is stale (see
    // `Game::take_world_state`'s doc comment) and deliberately skipped —
    // its real state is whatever was just hashed above via the live
    // fields, never this vector.
    for (id, ws) in &g.worlds {
        if *id == g.world {
            continue;
        }
        h = fnv_bytes(h, &[2]); // marker byte, distinct from the per-depth 0/1 markers above
        h = hash_world_id(h, *id);
        h = fnv_bytes(h, &ws.depth.to_le_bytes());
        h = hash_provenance(h, ws.from);
        for s in &ws.saved {
            match s {
                Some(ls) => {
                    h = fnv_bytes(h, &[1]);
                    h = level(h, &ls.map, &ls.monsters, &ls.items, &ls.blocks);
                    h = hash_portal(h, ls.portal);
                }
                None => h = fnv_bytes(h, &[0]),
            }
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
