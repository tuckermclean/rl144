// save.rs — state-is-deltas layer: a save is just the seed plus the input
// log, so persistence is serializing that pair and reconstruction is
// replaying it through Game::apply_input. Also the state hash used to prove
// a replay reproduced the live run exactly.

use crate::game::{Game, Item, Monster, Tile};
use crate::rng::{fnv_bytes, h64};

// ---------- Save / replay: state is deltas (seed + input log) ----------
/* A save is the original seed plus one byte per input; the world is
   reconstructed by replaying. Byte format, no serde:
     "RL14" | version u8 (=1) | seed u64 LE | input bytes...
   Inputs: 0=N 1=S 2=W 3=E 4=wait 5=restart. Tens of bytes per save. */
const SAVE_MAGIC: &[u8; 4] = b"RL14";
const SAVE_VERSION: u8 = 1;
pub(crate) const INPUT_RESTART: u8 = 5;

pub(crate) fn save_bytes(seed0: u64, inputs: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(13 + inputs.len());
    out.extend_from_slice(SAVE_MAGIC);
    out.push(SAVE_VERSION);
    out.extend_from_slice(&seed0.to_le_bytes());
    out.extend_from_slice(inputs);
    out
}

pub(crate) fn parse_save(bytes: &[u8]) -> Option<(u64, Vec<u8>)> {
    if bytes.len() < 13 || &bytes[..4] != SAVE_MAGIC || bytes[4] != SAVE_VERSION {
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
/// bytes re-derive the seed exactly as the R key does.
pub(crate) fn replay(seed0: u64, inputs: &[u8]) -> Game {
    let mut g = Game::new(seed0);
    for &b in inputs {
        if b == INPUT_RESTART {
            g = Game::new(h64(g.seed, &["restart"]));
        } else {
            g.apply_input(b);
        }
    }
    g
}

/// FNV-1a over a canonical serialization of everything that defines the run:
/// player, light, flags, RNG channel states, live level, and every stashed
/// level. If two replays of one save ever hash differently, channel
/// discipline broke somewhere.
///
/// `g.killer` is deliberately NOT hashed: it's presentation-only (the End
/// screen's cause-of-death line), doesn't affect anything replay needs to
/// reproduce, and is fully determined by state that IS hashed anyway (the
/// attack sequence that led to `dead`).
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
