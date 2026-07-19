// rl144 — a roguelike in under 1.44MB. Zero asset files; everything procedural or const.
// Exactly one backend feature is compiled in: backend-minifb (window/pixel
// presentation) or backend-term (ANSI terminal presentation, task 3). Both
// backends consume the same core render::render_cells/save:: surface, so
// every core item is reachable under either feature — no crate-wide
// dead-code allow needed. cfg is confined to this file's backend wiring and
// to backend_minifb.rs/backend_term.rs themselves; the rest of the crate
// stays cfg-free.
#[cfg(not(any(feature = "backend-minifb", feature = "backend-term")))]
compile_error!("exactly one backend feature must be enabled: backend-minifb or backend-term");

#[cfg(all(feature = "backend-minifb", feature = "backend-term"))]
compile_error!("backend features are mutually exclusive");

mod content;
mod game;
mod headless;
mod render;
mod rng;
mod save;

#[cfg(feature = "backend-minifb")]
mod backend_minifb;
#[cfg(feature = "backend-term")]
mod backend_term;

use game::Game;
use headless::{dump, sim_main, solve_main};
use rng::h64;
use save::{parse_save, replay, state_hash};

#[cfg(test)]
use content::{GHOST_LABELS, KINDS, TALK_LINES, THEMES, TONE_LINES, VAULTS, ghost_label_idx};
#[cfg(test)]
use game::{MKind, Monster, TIER_WARNINGS, Tile, idx, in_map};
#[cfg(test)]
use headless::{level_dump, sim_seed, solve_seed};
#[cfg(test)]
use render::scale;
#[cfg(test)]
use rng::channel;
#[cfg(test)]
use save::{INPUT_RESTART, INPUT_RETRY, ghost_bytes, parse_ghost, save_bytes, save_filename};

/// Resolve a `--seed` argument: numeric strings parse directly, anything
/// else is hashed into a u64 so `--seed swordfish` is stable and distinct
/// from any small numeric seed. Only used for `--seed`; `--solve`/`--sim`
/// keep the numeric-only `flag_val`.
fn seed_from_arg(s: &str) -> u64 {
    s.parse::<u64>().unwrap_or_else(|_| h64(0, &["seedstr", s]))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag_val = |name: &str| -> Option<u64> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse().ok())
    };
    let str_val = |name: &str| -> Option<String> {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
    };
    // seed precedence: explicit --seed > --daily (shared seed of the day) >
    // launch-time entropy. The only entropy in the whole program is here.
    // --seed accepts a raw string: numeric parses directly, anything else
    // is hashed (seed_from_arg) so e.g. --seed swordfish is stable.
    let daily = args.iter().any(|a| a == "--daily");
    // --ascii is only meaningful to backend-term (--render-frame and the
    // interactive loop's 7-bit fallback); cfg'd so a backend-minifb build
    // never sees an unused variable.
    #[cfg(feature = "backend-term")]
    let ascii = args.iter().any(|a| a == "--ascii");
    let day = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86400)
        .unwrap_or(0);
    let seed = str_val("--seed").map(|s| seed_from_arg(&s)).unwrap_or_else(|| {
        if daily {
            h64(day, &["daily"])
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0xDEAD_BEEF)
        }
    });

    if args.iter().any(|a| a == "--solve") {
        solve_main(
            flag_val("--solve").unwrap_or(10000),
            args.iter().any(|a| a == "--report"),
        );
        return;
    }
    if args.iter().any(|a| a == "--sim") {
        sim_main(
            flag_val("--sim").unwrap_or(1000),
            args.iter().any(|a| a == "--report"),
        );
        return;
    }
    if args.iter().any(|a| a == "--dump") {
        print!("{}", dump(seed));
        return;
    }
    // --render-frame: render one initial frame straight to stdout and exit,
    // no termios/alt-screen setup at all — safe with stdout redirected to a
    // file (this is the frame-golden capture path). Term-only.
    #[cfg(feature = "backend-term")]
    if args.iter().any(|a| a == "--render-frame") {
        backend_term::render_frame_main(seed, ascii);
        return;
    }
    if let Some(path) = str_val("--replay") {
        match std::fs::read(&path).ok().and_then(|b| parse_save(&b)) {
            Some((s0, inputs)) => {
                let g = replay(s0, &inputs);
                println!(
                    "{{\"hash\":\"{:016x}\",\"seed\":{},\"turns\":{},\"depth\":{},\"hp\":{},\"light\":{},\"dead\":{},\"won\":{}}}",
                    state_hash(&g), s0, inputs.len(), g.depth, g.hp, g.light, g.dead, g.won
                );
            }
            None => {
                eprintln!("bad save file: {}", path);
                std::process::exit(1);
            }
        }
        return;
    }

    // `loaded` distinguishes a `--load`ed save from a fresh seed: it's how
    // the backends decide whether to open on the Title screen or skip
    // straight to Play (see backend_minifb::run / backend_term::run).
    let (seed0, input_log, mut game, loaded) = match str_val("--load") {
        Some(path) => match std::fs::read(&path).ok().and_then(|b| parse_save(&b)) {
            Some((s0, inputs)) => {
                let g = replay(s0, &inputs);
                (s0, inputs, g, true)
            }
            None => {
                eprintln!("bad save file: {}", path);
                std::process::exit(1);
            }
        },
        None => (seed, Vec::new(), Game::new(seed), false),
    };
    if daily && input_log.is_empty() {
        game.log(format!("Daily dungeon #{}. Everyone gets this one today.", day));
    }

    // Backend dispatch: exactly one of these compiles (see the
    // compile_error! guards above). The backend owns everything
    // platform-specific — window/terminal I/O, input polling, save-file
    // writes on quit — and consumes the game purely through the core cell
    // grid (render::render_cells) and the input-byte vocabulary.
    #[cfg(feature = "backend-minifb")]
    backend_minifb::run(seed0, input_log, game, daily, day, loaded);
    #[cfg(feature = "backend-term")]
    backend_term::run(seed0, input_log, game, daily, day, ascii, loaded);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Task-1 acceptance: combat/AI channel draws must not perturb worldgen.
    #[test]
    fn worldgen_isolated_from_combat() {
        let seed = 0xC0FFEE;
        let mut a = Game::new(seed); // generates depth 1
        for _ in 0..1000 {
            a.combat_rng.next();
            a.ai_rng.next();
        }
        a.depth = 2;
        a.gen_level();
        let dirty = level_dump(&a);

        let mut b = Game::new(seed);
        b.depth = 2;
        b.gen_level();
        assert_eq!(dirty, level_dump(&b));
    }

    /// The hash is a public API: these literals were captured by running the
    /// implementation (2026-07-18). A failure here means the PRIMITIVE
    /// changed — every seed and save in the wild breaks (MAJOR), regardless
    /// of whether the worldgen goldens happen to still pass.
    #[test]
    fn hash_vectors() {
        assert_eq!(h64(0, &[]), 0x7bd3_144f_29c0_cc9e);
        assert_eq!(h64(0, &["worldgen", "1"]), 0x34bd_1025_0333_b247);
        assert_eq!(h64(1, &["combat"]), 0xfbeb_6048_ae90_1312);
        assert_eq!(h64(42, &["spawns", "5"]), 0x2973_9991_fe04_caf0);
        assert_eq!(h64(0xDEAD_BEEF, &["a", "b"]), 0x856c_83b2_7284_ba1e);
        let mut c = channel(42, &["worldgen", "1"]);
        assert_eq!(c.next(), 0xeda9_7859_383a_600c);
        assert_eq!(c.next(), 0xf4e6_8a47_e74b_97cc);
        assert_eq!(c.next(), 0x7162_bb71_2f5f_73e3);
    }

    /// Same seed, same full dungeon — twice.
    #[test]
    fn dump_deterministic() {
        assert_eq!(dump(42), dump(42));
    }

    /// Worldgen output is FROZEN by these fixtures. A diff here means a
    /// seed-breaking MAJOR change: get explicit human sign-off, then
    /// regenerate with `--dump --seed N > tests/golden/seed_N.txt`.
    #[test]
    fn golden_dumps() {
        let goldens: [(u64, &str); 5] = [
            (1, include_str!("../tests/golden/seed_1.txt")),
            (2, include_str!("../tests/golden/seed_2.txt")),
            (3, include_str!("../tests/golden/seed_3.txt")),
            (42, include_str!("../tests/golden/seed_42.txt")),
            (1337, include_str!("../tests/golden/seed_1337.txt")),
        ];
        for (seed, want) in goldens {
            assert_eq!(dump(seed), want, "golden drift for seed {}", seed);
        }
    }

    /// Every depth of every small seed must be winnable (exit reachable).
    #[test]
    fn solver_smoke() {
        for seed in 0..50 {
            assert!(solve_seed(seed).is_some(), "seed {} unwinnable", seed);
        }
    }

    /// Vault authoring rules: rectangular, solid border, open center,
    /// legend chars only. Reachability itself is proven by the solver gate.
    #[test]
    fn vaults_well_formed() {
        for (vi, v) in VAULTS.iter().enumerate() {
            let rows: Vec<&str> = v.lines().collect();
            let w = rows[0].len();
            assert!(rows.len() >= 3 && w >= 3, "vault {} too small", vi);
            for (j, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), w, "vault {} row {} ragged", vi, j);
                for (i, c) in row.bytes().enumerate() {
                    assert!(b"#.!)rgO".contains(&c), "vault {} bad char {}", vi, c as char);
                    if j == 0 || j == rows.len() - 1 || i == 0 || i == w - 1 {
                        assert_eq!(c, b'#', "vault {} border open at {},{}", vi, i, j);
                    }
                }
            }
            let (cx, cy) = (w / 2, rows.len() / 2);
            assert_eq!(rows[cy].as_bytes()[cx], b'.', "vault {} center not floor", vi);
        }
    }

    /// Every lore line must fit the 78-char log row for every slot filling,
    /// and so must the fixed-shape flavor messages — including the
    /// tier-crossing torch warnings (batch 3), which weave in a theme
    /// adjective via `self.adj()` and so must fit for EVERY theme's EVERY
    /// adjective, not just one.
    #[test]
    fn theme_lines_fit_log_row() {
        for lines in &TONE_LINES {
            for line in lines {
                for k in &KINDS {
                    assert!(line.replace("{K}", k).len() <= 78);
                }
            }
        }
        for t in &THEMES {
            assert!(format!("You enter {}.", t.label).len() <= 78);
            assert!(format!("You take {}. It is heavy. Climb, before dark!", t.amulet).len() <= 78);
            for lore in &t.lore {
                for slot in &t.slots {
                    let line = lore.replace("{A}", slot);
                    assert!(line.len() <= 78, "too long ({}): {}", line.len(), line);
                }
            }
            for warn in &TIER_WARNINGS {
                for adj in &t.adjs {
                    let line = warn.replace("{}", adj);
                    assert!(line.len() <= 78, "too long ({}): {}", line.len(), line);
                }
            }
        }
    }

    /// Every TALK_LINES template (batch 5, DECISION.md item 3 — the Henson
    /// ruling) must fit the 78-char log row for EVERY theme's mob-name
    /// filling — same length-test discipline as `theme_lines_fit_log_row`
    /// above, just keyed by `[MKind as usize]` into `Theme::mobs` instead
    /// of a flat slot table. `TALK_LINES`'s outer index is kind (rat=0,
    /// goblin=1, ogre=2 — `MKind`'s declared order), matching
    /// `Theme::mobs`'s index exactly (see `Game::mob_name`).
    #[test]
    fn talk_lines_fit_log_row() {
        for (ki, kind_lines) in TALK_LINES.iter().enumerate() {
            for stage_lines in kind_lines {
                for line in stage_lines {
                    for t in &THEMES {
                        let filled = line.replace("{M}", t.mobs[ki]);
                        assert!(filled.len() <= 78, "too long ({}): {}", filled.len(), filled);
                    }
                }
            }
        }
    }

    /// A turn burns 1 light, 2 while carrying the amulet; walls burn nothing.
    #[test]
    fn light_burn_rates() {
        let mut g = Game::new(7);
        let l0 = g.light;
        g.wait_turn();
        assert_eq!(g.light, l0 - 1);
        g.has_amulet = true;
        g.wait_turn();
        assert_eq!(g.light, l0 - 3);
    }

    /// Violence tax (batch 4, DECISION.md item 1): a bump-attack burns the
    /// normal 1-per-turn cost plus VIOLENCE_TAX (1) = 2 total, vs 1 for a
    /// plain wait turn.
    #[test]
    fn violence_tax_burns_extra_light() {
        let mut g = Game::new(7);
        let l0 = g.light;
        g.wait_turn();
        assert_eq!(g.light, l0 - 1, "a wait turn should burn 1 light");
        let l1 = g.light;
        let (px, py) = (g.px, g.py);
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .find(|&(dx, dy)| {
                in_map(px + dx, py + dy) && g.map[idx(px + dx, py + dy)] != Tile::Wall
            })
            .unwrap();
        g.monsters.clear();
        // High hp so the swing doesn't kill it — isolates the tax from the
        // kill path (that's covered by the darkness-death test below).
        g.monsters.push(Monster {
            x: px + dx,
            y: py + dy,
            kind: MKind::Ogre,
            hp: 999,
            regard: 0,
            calm: false,
        });
        g.try_move_player(dx, dy);
        assert_eq!(g.light, l1 - 2, "bump-attack should burn 1 turn + 1 violence tax = 2 light");
    }

    /// Violence tax: if the tax itself lands light at/below 0 on a killing
    /// blow, that's a darkness death — the light-0 check still runs exactly
    /// once (inside spend_turn) and lose-before-win ordering holds. No
    /// `killer` is attributed, since monsters_act never runs on this turn.
    #[test]
    fn violence_tax_kill_can_cause_darkness_death() {
        let mut g = Game::new(7);
        let (px, py) = (g.px, g.py);
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .find(|&(dx, dy)| {
                in_map(px + dx, py + dy) && g.map[idx(px + dx, py + dy)] != Tile::Wall
            })
            .unwrap();
        g.monsters.clear();
        g.monsters.push(Monster {
            x: px + dx,
            y: py + dy,
            kind: MKind::Rat,
            hp: 1, // guaranteed 1-hit kill
            regard: 0,
            calm: false,
        });
        g.light = 2; // 1 (turn) + 1 (tax) lands exactly on 0
        g.try_move_player(dx, dy);
        assert!(g.dead, "should die in the dark from the violence tax");
        assert_eq!(g.light, 0);
        assert!(!g.won, "lose is checked before win");
        assert!(g.killer.is_none(), "a darkness death has no combat killer");
    }

    /// Running out of light on the exit tile is a LOSE, not a win.
    #[test]
    fn lose_beats_win_at_zero_light() {
        let mut g = Game::new(7);
        g.has_amulet = true;
        g.light = 2; // burn of 2 lands exactly on 0
        // step off the entrance and back onto it
        let (ex, ey) = (g.px, g.py);
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .find(|&(dx, dy)| {
                in_map(ex + dx, ey + dy) && g.map[idx(ex + dx, ey + dy)] == Tile::Floor
            })
            .unwrap();
        g.monsters.clear(); // keep the test about light, not combat
        g.light = 3;
        g.try_move_player(dx, dy);
        assert!(!g.dead && !g.won);
        g.try_move_player(-dx, -dy); // back onto '<' with light 1 -> 0
        assert!(g.dead, "should die in the dark");
        assert!(!g.won, "lose is checked before win");
    }

    /// Stayed swing (batch 5, DECISION.md item 3): a monster that received
    /// an ACT this turn does not attack this turn (it is listening) — a
    /// second, un-ACTed monster adjacent and seeing the player attacks
    /// normally the SAME turn, proving the mercy is per-monster, not a
    /// blanket "combat is off" toggle (crowds stay dangerous).
    #[test]
    fn stayed_swing_no_damage_this_turn() {
        let mut g = Game::new(21);
        g.monsters.clear();
        // Find a floor tile with at least two open (non-wall) axis
        // neighbors, so two monsters can flank the player.
        let mut spot = None;
        'outer: for y in 1..(game::MAP_H as i32 - 1) {
            for x in 1..(game::COLS as i32 - 1) {
                if g.map[idx(x, y)] != Tile::Floor {
                    continue;
                }
                let dirs: Vec<(i32, i32)> = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .into_iter()
                    .filter(|&(dx, dy)| {
                        in_map(x + dx, y + dy) && g.map[idx(x + dx, y + dy)] != Tile::Wall
                    })
                    .collect();
                if dirs.len() >= 2 {
                    spot = Some((x, y, dirs));
                    break 'outer;
                }
            }
        }
        let (px, py, dirs) = spot.expect("map should have a floor tile with 2 open neighbors");
        g.px = px;
        g.py = py;
        let (adx, ady) = dirs[0];
        let (bdx, bdy) = dirs[1];
        g.monsters.push(Monster {
            x: px + adx,
            y: py + ady,
            kind: MKind::Rat,
            hp: 3,
            regard: 0,
            calm: false,
        });
        g.monsters.push(Monster {
            x: px + bdx,
            y: py + bdy,
            kind: MKind::Goblin,
            hp: 6,
            regard: 0,
            calm: false,
        });
        let hp0 = g.hp;
        g.try_act_player(adx, ady); // ACT the rat: regard 0->1, threshold 2, not yet calm
        let rat = g.monsters.iter().find(|m| m.kind == MKind::Rat).unwrap();
        assert_eq!(rat.regard, 1, "the ACTed rat's regard should have incremented");
        assert!(!rat.calm, "one ACT (of 2) should not yet calm a rat");
        assert!(
            g.hp < hp0,
            "the un-ACTed adjacent goblin should still attack this same turn"
        );
    }

    /// Becalm threshold + swap-on-bump (batch 5, DECISION.md item 3): a
    /// rat (threshold 2) is not calm after one ACT, becomes calm (and
    /// `spared` increments) on the second, and bumping a calmed monster
    /// swaps positions — no damage, no violence tax — instead of attacking.
    #[test]
    fn becalm_threshold_and_swap_on_bump() {
        let mut g = Game::new(5);
        g.monsters.clear();
        let (px, py) = (g.px, g.py);
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .find(|&(dx, dy)| {
                in_map(px + dx, py + dy) && g.map[idx(px + dx, py + dy)] != Tile::Wall
            })
            .unwrap();
        g.monsters.push(Monster {
            x: px + dx,
            y: py + dy,
            kind: MKind::Rat,
            hp: 3,
            regard: 0,
            calm: false,
        });
        let spared0 = g.spared;

        g.try_act_player(dx, dy); // regard 0->1: below threshold 2
        assert!(!g.monsters[0].calm);
        assert_eq!(g.spared, spared0);

        g.try_act_player(dx, dy); // regard 1->2: threshold reached
        assert!(g.monsters[0].calm, "the rat should be calm after 2 ACTs");
        assert_eq!(g.spared, spared0 + 1, "spared must increment exactly once, on the crossing");

        let hp_before = g.monsters[0].hp;
        let (mx, my) = (g.monsters[0].x, g.monsters[0].y);
        let (old_px, old_py) = (g.px, g.py);
        g.try_move_player(dx, dy); // bump the now-calm rat: swap, not attack
        assert_eq!(g.monsters[0].hp, hp_before, "a swap must not damage the calmed monster");
        assert_eq!((g.px, g.py), (mx, my), "the player takes the monster's old tile");
        assert_eq!(
            (g.monsters[0].x, g.monsters[0].y),
            (old_px, old_py),
            "the monster takes the player's old tile"
        );
    }

    /// `spared` is hashed (batch 5, DECISION.md item 3): two otherwise-
    /// identical games diverge in `state_hash` the instant `spared` differs
    /// — the same pattern `state_hash`'s own doc comment uses to justify
    /// what's IN the hash vs. the presentation-only exclusion set.
    #[test]
    fn spared_is_hashed() {
        let a = Game::new(9);
        let b = Game::new(9);
        assert_eq!(state_hash(&a), state_hash(&b));
        let mut a2 = Game::new(9);
        a2.spared = 1;
        assert_ne!(state_hash(&a2), state_hash(&b), "spared must be part of state_hash");
    }

    /// Per-monster `regard`/`calm` are hashed too (batch 5): mutate either
    /// field on one live monster and the hash must move, proving
    /// `save::state_hash`'s per-monster byte list actually includes them
    /// (not just the top-level `spared` counter).
    #[test]
    fn monster_regard_and_calm_are_hashed() {
        let mut a = Game::new(9);
        let b = Game::new(9);
        assert_eq!(state_hash(&a), state_hash(&b));
        assert!(!a.monsters.is_empty(), "depth 1 of seed 9 should spawn at least one monster");

        a.monsters[0].regard = 1;
        assert_ne!(state_hash(&a), state_hash(&b), "regard must be part of state_hash");

        a.monsters[0].regard = 0;
        assert_eq!(state_hash(&a), state_hash(&b), "hash should return once regard reverts");
        a.monsters[0].calm = true;
        assert_ne!(state_hash(&a), state_hash(&b), "calm must be part of state_hash");
    }

    /// ACT determinism + replay round-trip (batch 5, DECISION.md item 3):
    /// a scripted log mixing move/wait/ACT bytes (0-4, 7-10) replays to an
    /// identical `state_hash` every time — the same determinism proof
    /// `save_replay_roundtrip` makes for the pre-mercy vocabulary, now
    /// covering the new bytes.
    #[test]
    fn act_bytes_replay_deterministic() {
        let seed0 = 88u64;
        let mut script = channel(seed0, &["test", "act_script"]);
        let mut log: Vec<u8> = Vec::new();
        for _ in 0..500 {
            // 0..=4 move/wait, 7..=10 ACT-N/S/W/E — skip 5/6, which are
            // reconstruction-layer bytes handled outside apply_input.
            let roll = script.range(0, 9) as u8;
            let b = if roll < 5 { roll } else { roll + 2 };
            log.push(b);
        }
        let a = replay(seed0, &log);
        let b = replay(seed0, &log);
        assert_eq!(state_hash(&a), state_hash(&b));
    }

    /// Play a scripted run live, save, replay the save: identical hashes.
    /// This is the determinism regression harness — a failure here means
    /// channel discipline broke somewhere.
    #[test]
    fn save_replay_roundtrip() {
        let seed0 = 99;
        let mut live = Game::new(seed0);
        let mut log: Vec<u8> = Vec::new();
        let mut script = channel(seed0, &["test", "script"]);
        for _ in 0..500 {
            let b = script.range(0, 5) as u8;
            log.push(b);
            live.apply_input(b);
        }
        let bytes = save_bytes(seed0, &log);
        let (s2, log2) = parse_save(&bytes).expect("save parses");
        assert_eq!(s2, seed0);
        assert_eq!(log2, log);
        let replayed = replay(s2, &log2);
        assert_eq!(state_hash(&live), state_hash(&replayed));
    }

    /// Restart bytes replay deterministically too.
    #[test]
    fn replay_with_restart_deterministic() {
        let mut log = vec![0u8, 3, 3, 1, 4, INPUT_RESTART, 1, 1, 3, 0, 4, 2];
        let a = replay(5, &log);
        let b = replay(5, &log);
        assert_eq!(state_hash(&a), state_hash(&b));
        // and the hash actually reflects state: one more input changes it
        log.push(1);
        let c = replay(5, &log);
        assert!(state_hash(&c) != state_hash(&a) || (c.px, c.py) == (a.px, a.py));
    }

    /// Save back-compat (DECISION.md sign-off item 2; extended to v3 by
    /// batch 5): a v1-versioned byte blob (which by construction never
    /// contains byte 6 or bytes 7-10 — neither INPUT_RETRY nor ACT existed
    /// yet) parses under today's v3-aware `parse_save` and replays byte-
    /// identically to the same log fed straight to `replay`.
    /// `tests/fixtures/ref.sav` (used by `make xhash`) is itself exactly
    /// such a v1 blob, containing a byte 5 — this test is the unit-level
    /// proof that the case `make xhash` exercises end-to-end still works.
    #[test]
    fn v1_save_replays_under_v3_parsing() {
        let seed0 = 123u64;
        let log = vec![0u8, 1, 2, 3, 4, INPUT_RESTART, 0, 1, 2, 3, 4];
        let mut v1_bytes = Vec::new();
        v1_bytes.extend_from_slice(b"RL14");
        v1_bytes.push(1); // v1
        v1_bytes.extend_from_slice(&seed0.to_le_bytes());
        v1_bytes.extend_from_slice(&log);

        let (s, parsed_log) = parse_save(&v1_bytes).expect("v1 blob must still parse");
        assert_eq!(s, seed0);
        assert_eq!(parsed_log, log);

        let from_v1 = replay(s, &parsed_log);
        let direct = replay(seed0, &log);
        assert_eq!(state_hash(&from_v1), state_hash(&direct));
    }

    /// Save v2 back-compat, same proof as the v1 test above but for a
    /// v2-versioned blob that also carries a byte 6 (INPUT_RETRY, which v2
    /// introduced but a v1 blob could never contain) — a v2 log still
    /// never contains bytes 7-10 (ACT didn't exist until save v3, batch 5),
    /// so it too must replay byte-identically under today's parser.
    #[test]
    fn v2_save_replays_under_v3_parsing() {
        let seed0 = 321u64;
        let log = vec![0u8, 1, INPUT_RETRY, 2, 3, 4];
        let mut v2_bytes = Vec::new();
        v2_bytes.extend_from_slice(b"RL14");
        v2_bytes.push(2); // v2
        v2_bytes.extend_from_slice(&seed0.to_le_bytes());
        v2_bytes.extend_from_slice(&log);

        let (s, parsed_log) = parse_save(&v2_bytes).expect("v2 blob must still parse");
        assert_eq!(s, seed0);
        assert_eq!(parsed_log, log);

        let from_v2 = replay(s, &parsed_log);
        let direct = replay(seed0, &log);
        assert_eq!(state_hash(&from_v2), state_hash(&direct));
    }

    /// `save_bytes` writes the current version (3, batch 5) and a byte-4
    /// version outside 1..=3 is rejected by `parse_save` — the "old binary
    /// must reject a v3 save cleanly" half of the save-v3 rationale
    /// (game.rs's `apply_input` handling ACT bytes 7-10 is the other half).
    #[test]
    fn save_bytes_writes_current_version_and_unknown_versions_are_rejected() {
        let bytes = save_bytes(7, &[0, 1, 2]);
        assert_eq!(bytes[4], 3, "save_bytes must write the current version");
        assert!(parse_save(&bytes).is_some());

        let mut future = bytes.clone();
        future[4] = 4;
        assert!(parse_save(&future).is_none(), "an unknown version must be rejected");

        let mut zero = bytes;
        zero[4] = 0;
        assert!(parse_save(&zero).is_none(), "version 0 must be rejected");
    }

    /// Retry byte (6, save v2): a log with moves, a retry, then more moves
    /// replays deterministically (two replays hash identically), and after
    /// byte 6 the seed is UNCHANGED — unlike byte 5 (restart), which still
    /// rerolls to a new seed exactly as it always did.
    #[test]
    fn retry_byte_replay_deterministic_and_seed_semantics() {
        let seed0 = 55u64;
        let log = vec![0u8, 1, 2, INPUT_RETRY, 3, 0, 1];
        let a = replay(seed0, &log);
        let b = replay(seed0, &log);
        assert_eq!(state_hash(&a), state_hash(&b));
        assert_eq!(a.seed, seed0, "byte 6 (retry) must keep the same seed");

        let reroll_log = vec![0u8, 1, 2, INPUT_RESTART, 3, 0, 1];
        let c = replay(seed0, &reroll_log);
        assert_ne!(c.seed, seed0, "byte 5 (restart) must reroll to a new seed");
    }

    /// Ghost round-trip: `ghost_bytes` -> `parse_ghost` is the inverse.
    #[test]
    fn ghost_bytes_round_trip() {
        let seed = 77u64;
        let whash = 0xABCD_EF01_2345_6789u64;
        let outcome = 0u8; // died_combat
        let final_depth = 3u8;
        let turns = 512u32;
        let label_idx = ghost_label_idx(outcome, final_depth);
        let inputs = vec![0u8, 1, 2, 3, 4, 0, 1];

        let bytes = ghost_bytes(seed, whash, outcome, final_depth, turns, label_idx, &inputs);
        let g = parse_ghost(&bytes).expect("a ghost_bytes blob must parse");
        assert_eq!(g.seed, seed);
        assert_eq!(g.world_hash, whash);
        assert_eq!(g.outcome, outcome);
        assert_eq!(g.final_depth, final_depth);
        assert_eq!(g.turns, turns);
        assert_eq!(g.label_idx, label_idx);
        assert_eq!(g.inputs, inputs);
    }

    /// Echo (batch 4 task 2, save v2 substrate): after replaying a log
    /// whose retry byte (6) followed a death, `echo` equals the death
    /// position/depth, and — proving `echo` is presentation-only and NOT
    /// hashed — a fresh `Game::new(seed)` with `echo` set by hand to the
    /// same value hashes identically to the retried game.
    #[test]
    fn echo_records_death_position_after_retry_byte() {
        let seed0 = 33u64;
        let mut live = Game::new(seed0);
        let mut log: Vec<u8> = Vec::new();
        // Wait repeatedly until the run ends (dark or combat — either way
        // it's a dead ending since the player never moves off the
        // entrance); START_LIGHT (2000) bounds this loop.
        while !live.dead && !live.won {
            log.push(4);
            live.apply_input(4);
        }
        assert!(live.dead, "a waiting-only run must die, not win");
        let (dx, dy, dd) = (live.px, live.py, live.depth);
        log.push(INPUT_RETRY);

        let replayed = replay(seed0, &log);
        assert_eq!(replayed.echo, Some((dx, dy, dd)));
        assert_eq!(replayed.seed, seed0, "retry must not reroll the seed");

        let mut fresh = Game::new(seed0);
        fresh.echo = Some((dx, dy, dd));
        assert_eq!(
            state_hash(&replayed),
            state_hash(&fresh),
            "echo must be unhashed: only its own value should differ"
        );
    }

    /// GHOST_LABELS (content.rs): every preset phrase is ASCII and <=16
    /// bytes, per the RLG1 format's label_idx contract in save.rs.
    #[test]
    fn ghost_labels_fit_16_bytes() {
        for label in &GHOST_LABELS {
            assert!(label.is_ascii(), "non-ASCII ghost label: {}", label);
            assert!(label.len() <= 16, "ghost label too long ({}): {}", label.len(), label);
        }
    }

    /// Visited depths persist: descend and climb back into the same level.
    #[test]
    fn levels_persist_across_stairs() {
        let mut g = Game::new(11);
        let map1: Vec<u8> = g.map.iter().map(|t| *t as u8).collect();
        let items1 = g.items.len();
        g.descend();
        assert_eq!(g.depth, 2);
        g.ascend();
        assert_eq!(g.depth, 1);
        let map1b: Vec<u8> = g.map.iter().map(|t| *t as u8).collect();
        assert_eq!(map1, map1b, "depth 1 layout changed across a round trip");
        assert_eq!(items1, g.items.len(), "items respawned across a round trip");
        // and the player came back out standing on the down-stairs
        assert!(g.map[idx(g.px, g.py)] == Tile::Stairs);
    }

    /// The greedy bot has no RNG of its own: replaying a seed twice through
    /// sim_seed must produce byte-identical outcomes.
    #[test]
    fn sim_deterministic() {
        for seed in [7u64, 42u64] {
            let a = sim_seed(seed);
            let b = sim_seed(seed);
            assert_eq!(a, b, "seed {} was not deterministic", seed);
        }
    }

    /// Over a small seed range the bot should never get stuck (stuck would
    /// mean a policy bug or an unreachable objective, which the solver
    /// already guarantees can't happen), and every run must resolve to
    /// exactly one terminal outcome. Historical finding (batch 2): measured
    /// win_rate was 0.000 over seeds 0..20 (and over samples up to 5000),
    /// 100% combat deaths — combat lethality, not the light budget, was the
    /// wall. The batch-3 balance pass (spawn count, roll table, potion
    /// counts, per-depth HP bonus; see `descend` and `Monster::stats` in
    /// `game.rs`) fixed this and is gated by `--sim`/`tests/sim-band.json`
    /// (`make sim`). Seeds 0..50 now produce 3 deterministic wins, so this
    /// test asserts `wins >= 1` as a floor against regression.
    #[test]
    fn sim_bot_wins_some() {
        let mut wins = 0u64;
        let mut deaths_combat = 0u64;
        let mut deaths_dark = 0u64;
        let mut stuck = 0u64;
        for seed in 0..50u64 {
            let r = sim_seed(seed);
            if r.won {
                wins += 1;
            } else if r.dead_dark {
                deaths_dark += 1;
            } else if r.dead_combat {
                deaths_combat += 1;
            } else if r.stuck {
                stuck += 1;
            }
        }
        assert_eq!(stuck, 0, "bot got stuck on {} of seeds 0..50", stuck);
        assert!(wins >= 1, "expected at least one win over seeds 0..50, got 0");
        assert_eq!(
            wins + deaths_combat + deaths_dark,
            50,
            "every run must resolve to exactly one terminal outcome"
        );
    }

    /// Low-light brightness scaling: per-channel `channel * pct / 100`.
    #[test]
    fn scale_color_channels() {
        assert_eq!(scale(0xFF8040, 50), 0x7F4020);
        assert_eq!(scale(0xFFFFFF, 100), 0xFFFFFF);
    }

    /// Task-3: non-numeric --seed values hash stably and distinctly;
    /// numeric strings parse through unchanged.
    #[test]
    fn string_seeds() {
        assert_eq!(seed_from_arg("swordfish"), seed_from_arg("swordfish"));
        assert_eq!(seed_from_arg("123"), 123);
        assert_ne!(seed_from_arg("swordfish"), seed_from_arg("herring"));
    }

    /// Task-3: save filename format is frozen — F5 and autosave both derive
    /// from this, so a golden-style test here catches drift in either.
    #[test]
    fn save_filename_format() {
        let name = save_filename(0xDEAD_BEEF_1234_5678);
        assert_eq!(name, "rl144-deadbeef12345678.sav");
        assert_eq!(name.len(), 6 + 16 + 4);
        assert!(name.starts_with("rl144-"));
        assert!(name.ends_with(".sav"));
    }
}
