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
mod gamedef;
mod games;
mod headless;
mod render;
mod rng;
mod save;

#[cfg(feature = "backend-minifb")]
mod backend_minifb;
#[cfg(feature = "backend-term")]
mod backend_term;

use game::Game;
use headless::{Policy, dump, dump_overworld, sim_main, solve_main};
use rng::h64;
use save::{parse_save, replay, state_hash};

#[cfg(test)]
use content::ghost_label_idx;
#[cfg(test)]
use game::{
    COLS, Dest, Item, MAP_H, MAX_PUSH_CHAIN, MKind, Monster, Tile, WorldId, bfs_dist, fov_radius, idx,
    in_map, mood_shine_radius, receptivity,
};
#[cfg(test)]
use gamedef::CarryEvent;
#[cfg(test)]
use games::GAME;
#[cfg(test)]
use games::contractor::{CHEESE, COAT, DONKEY, GOBLIN, OGRE, POTION, RAT, TOWEL, TRAINER};
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
        // Default policy is greedy (unchanged CLI surface); `--policy
        // pacifist` selects the mercy bot (batch 5 T2), `--policy tactical`
        // selects the route-around-fights bot (batch 10 T2). Any other value
        // (including a typo) falls back to greedy rather than silently
        // matching nothing, same tolerance as the rest of this arg parser.
        let policy = match str_val("--policy").as_deref() {
            Some("pacifist") => Policy::Pacifist,
            Some("tactical") => Policy::Tactical,
            Some("tactical-pacifist") => Policy::TacticalPacifist,
            _ => Policy::Greedy,
        };
        sim_main(
            flag_val("--sim").unwrap_or(1000),
            args.iter().any(|a| a == "--report"),
            policy,
        );
        return;
    }
    if args.iter().any(|a| a == "--dump") {
        print!("{}", dump(seed));
        return;
    }
    // batch 9 T1: seed-independent (fixed authored ASCII, zero RNG) — see
    // `headless::dump_overworld`'s doc comment. Checked before `--dump`'s
    // sibling handling above only in file order; both return immediately so
    // order between them doesn't matter.
    if args.iter().any(|a| a == "--dump-overworld") {
        print!("{}", dump_overworld());
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
        // Batch 9 T3 (SIGN-OFF ASKS #3/#4): a fresh interactive session
        // starts in the overworld, not directly in the dungeon — `Game::new`
        // itself is unchanged and stays the frozen constructor every
        // headless verification surface (dump/solve/sim/render-frame) uses.
        None => (seed, Vec::new(), Game::new_overworld(seed), false),
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
        for (vi, v) in GAME.vaults.iter().enumerate() {
            let rows: Vec<&str> = v.lines().collect();
            let w = rows[0].len();
            assert!(rows.len() >= 3 && w >= 3, "vault {} too small", vi);
            for (j, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), w, "vault {} row {} ragged", vi, j);
                for (i, c) in row.bytes().enumerate() {
                    // batch 6 T2: '^' pit, 'B' block, 'x' goal (sokoban).
                    // batch 7 T2: 'o' cheese, '[' coat, '~' towel.
                    assert!(b"#.!)rgO^Bxo[~".contains(&c), "vault {} bad char {}", vi, c as char);
                    if j == 0 || j == rows.len() - 1 || i == 0 || i == w - 1 {
                        assert_eq!(c, b'#', "vault {} border open at {},{}", vi, i, j);
                    }
                }
            }
            let (cx, cy) = (w / 2, rows.len() / 2);
            assert_eq!(rows[cy].as_bytes()[cx], b'.', "vault {} center not floor", vi);
        }
    }

    // ---------- Sokoban (batch 6 T2) ----------

    /// Helper for the push-mechanics unit tests below: a real generated
    /// `Game` (so seed/theme/RNG machinery is realistic) with monsters/
    /// items/blocks cleared and a clean 10x10 floor patch stamped in,
    /// player centered — isolates push semantics from worldgen entirely.
    fn blank_room(seed: u64) -> Game {
        let mut g = Game::new(seed);
        g.monsters.clear();
        g.items.clear();
        g.blocks.clear();
        for y in 5..15 {
            for x in 5..15 {
                g.map[idx(x, y)] = Tile::Floor;
            }
        }
        g.px = 10;
        g.py = 10;
        g.turns = 0;
        g
    }

    #[test]
    fn max_push_chain_is_two() {
        assert_eq!(MAX_PUSH_CHAIN, 2, "topdown-puzzle's cap; the batch brief pins this value");
    }

    #[test]
    fn push_into_floor_advances_block_and_player() {
        let mut g = blank_room(1);
        g.blocks.push((11, 10));
        let before_turns = g.turns;
        g.try_move_player(1, 0);
        assert!(g.blocks.contains(&(12, 10)), "block should advance one tile");
        assert!(!g.blocks.contains(&(11, 10)));
        assert_eq!((g.px, g.py), (11, 10), "player advances into the vacated cell");
        assert_eq!(g.turns, before_turns + 1, "a successful push costs a turn");
    }

    #[test]
    fn push_into_wall_refuses_no_turn() {
        let mut g = blank_room(1);
        g.blocks.push((11, 10));
        g.map[idx(12, 10)] = Tile::Wall;
        let before_turns = g.turns;
        let before_light = g.light;
        g.try_move_player(1, 0);
        assert!(g.blocks.contains(&(11, 10)), "block must not move");
        assert_eq!((g.px, g.py), (10, 10), "player must not move");
        assert_eq!(g.turns, before_turns, "a refused push costs no turn");
        assert_eq!(g.light, before_light, "a refused push burns no light");
    }

    #[test]
    fn push_into_monster_refuses() {
        let mut g = blank_room(1);
        g.blocks.push((11, 10));
        g.monsters.push(Monster { x: 12, y: 10, kind: RAT, hp: 3, regard: 0, calm: false, awe: 0 });
        g.try_move_player(1, 0);
        assert!(g.blocks.contains(&(11, 10)), "block must not move");
        assert_eq!((g.px, g.py), (10, 10));
    }

    #[test]
    fn push_into_stairs_refuses() {
        let mut g = blank_room(1);
        g.blocks.push((11, 10));
        g.map[idx(12, 10)] = Tile::Stairs;
        g.try_move_player(1, 0);
        assert!(g.blocks.contains(&(11, 10)));
        assert_eq!((g.px, g.py), (10, 10));
    }

    #[test]
    fn push_into_pit_destroys_block_and_fills() {
        let mut g = blank_room(1);
        g.blocks.push((11, 10));
        g.map[idx(12, 10)] = Tile::Pit;
        g.try_move_player(1, 0);
        assert!(g.blocks.is_empty(), "the block is destroyed");
        assert_eq!(g.map[idx(12, 10)], Tile::Floor, "the pit fills");
        assert_eq!((g.px, g.py), (11, 10));
    }

    #[test]
    fn push_onto_goal_locks_block() {
        let mut g = blank_room(1);
        g.blocks.push((11, 10));
        g.map[idx(12, 10)] = Tile::Goal;
        g.try_move_player(1, 0);
        assert!(g.blocks.is_empty(), "the block is absorbed, no longer a live entity");
        assert_eq!(g.map[idx(12, 10)], Tile::Floor, "the locked tile becomes ordinary floor");
        assert_eq!((g.px, g.py), (11, 10));
    }

    #[test]
    fn push_chain_of_two_works() {
        let mut g = blank_room(1);
        g.blocks.push((11, 10));
        g.blocks.push((12, 10));
        g.try_move_player(1, 0);
        assert!(g.blocks.contains(&(12, 10)), "nearer block advances into the farther's old slot");
        assert!(g.blocks.contains(&(13, 10)), "farther block advances into the landing cell");
        assert_eq!(g.blocks.len(), 2);
        assert_eq!((g.px, g.py), (11, 10));
    }

    #[test]
    fn push_chain_of_three_refuses() {
        let mut g = blank_room(1);
        g.blocks.push((11, 10));
        g.blocks.push((12, 10));
        g.blocks.push((13, 10));
        let before_turns = g.turns;
        g.try_move_player(1, 0);
        let mut want = vec![(11, 10), (12, 10), (13, 10)];
        let mut got = g.blocks.clone();
        want.sort();
        got.sort();
        assert_eq!(got, want, "a 3-chain must not move at all");
        assert_eq!((g.px, g.py), (10, 10), "player must not move on a refused push");
        assert_eq!(g.turns, before_turns);
    }

    #[test]
    fn push_chain_of_two_into_pit_destroys_farthest_and_advances_survivor() {
        let mut g = blank_room(1);
        g.blocks.push((11, 10));
        g.blocks.push((12, 10));
        g.map[idx(13, 10)] = Tile::Pit;
        g.try_move_player(1, 0);
        assert_eq!(g.blocks, vec![(12, 10)], "the nearer block survives, advancing one slot");
        assert_eq!(g.map[idx(13, 10)], Tile::Floor, "the pit fills");
        assert_eq!((g.px, g.py), (11, 10));
    }

    /// Blocks are hashed state (batch 6 T2): two otherwise-identical games
    /// differing only in a block's position must hash differently.
    #[test]
    fn blocks_are_hashed() {
        let mut a = blank_room(1);
        a.blocks.push((11, 10));
        let mut b = blank_room(1);
        b.blocks.push((12, 10));
        assert_ne!(state_hash(&a), state_hash(&b), "differing block positions must hash differently");
    }

    /// Persistence round trip: push a block, leave the level, return — the
    /// block stayed (the story's "fossilized bad idea", per
    /// docs/story/STORY-COMPILE-v1.md §6.3).
    #[test]
    fn block_persists_across_descend_and_ascend() {
        let mut g = Game::new(1);
        let (px, py) = (g.px, g.py);
        let spot = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .map(|&(dx, dy)| (px + dx, py + dy))
            .find(|&(x, y)| g.map[idx(x, y)] == Tile::Floor)
            .expect("fixture: spawn room is larger than 1x1, so it has a floor neighbor");
        g.blocks = vec![spot];
        g.descend();
        assert_eq!(g.depth, 2);
        g.ascend();
        assert_eq!(g.depth, 1);
        assert_eq!(g.blocks, vec![spot], "block position must survive a stash/restore round trip");
    }

    /// Replay determinism with pushes: driving the SAME push through
    /// `apply_input` twice from a fresh `Game::new` must land on an
    /// identical `state_hash` — a push draws no RNG, so channel discipline
    /// is trivially preserved, but this is the explicit regression gate.
    #[test]
    fn push_replay_is_deterministic() {
        let dirs = [((1, 0), 3u8), ((-1, 0), 2u8), ((0, 1), 1u8), ((0, -1), 0u8)];
        let build = |seed: u64| {
            let mut g = Game::new(seed);
            let (px, py) = (g.px, g.py);
            let (spot, byte) = dirs
                .iter()
                .map(|&((dx, dy), b)| ((px + dx, py + dy), b))
                .find(|&((x, y), _)| g.map[idx(x, y)] == Tile::Floor)
                .expect("fixture: spawn has a floor neighbor");
            g.blocks = vec![spot];
            g.apply_input(byte);
            g
        };
        let a = build(1);
        let b = build(1);
        assert_eq!(state_hash(&a), state_hash(&b), "two replays of the same push must hash identically");
        assert_eq!(a.blocks, b.blocks);
    }

    #[test]
    fn sokoban_vaults_present() {
        assert!(GAME.vaults.len() >= 5, "expected the batch-6 T2 sokoban vaults at indices 3+");
        let uses = |v: &str, c: u8| v.bytes().any(|b| b == c);
        assert!(GAME.vaults[3..].iter().any(|v| uses(v, b'^')), "expected a true pit/bridge puzzle");
        assert!(GAME.vaults[3..].iter().any(|v| uses(v, b'x')), "expected a goal-tile room");
    }

    /// SOLUTION TEST (batch 6 T2, ported discipline from golem/
    /// topdown-puzzle's tests/solutions/*.moves.json): "the bridge" vault
    /// ships with its proof of solvability — a checked-in move sequence
    /// that pushes the block into the pit gap and walks to the reward.
    #[test]
    fn sokoban_bridge_vault_is_solvable() {
        let vi = 3;
        let mut g = Game::new(1);
        g.map = vec![Tile::Wall; COLS * MAP_H];
        g.monsters.clear();
        g.items.clear();
        g.blocks.clear();
        let rows: Vec<&str> = GAME.vaults[vi].lines().collect();
        let (vw, vh) = (rows[0].len() as i32, rows.len() as i32);
        let (ox, oy) = (5, 5);
        g.stamp_vault(GAME.vaults[vi], ox, oy);
        g.px = ox + vw / 2;
        g.py = oy + vh / 2;
        assert!(!g.items.is_empty(), "fixture: the bridge vault has a reward item");
        for _ in 0..3 {
            g.apply_input(3); // East: push the block into the pit, then walk to the potion
        }
        assert!(g.items.is_empty(), "the reward must have been reached and picked up");
    }

    /// SOLUTION TEST: "the goal cell" — a checked-in move sequence that
    /// walks west to collect the towel and coat (batch 7 T2: two free
    /// floor cells immediately west of the start, no puzzle required),
    /// walks back, then pushes a 2-chain into the pit (destroying the
    /// farthest member, filling the gap), keeps pushing the survivor onto
    /// the goal tile (locking it), then walks to the sword reward.
    #[test]
    fn sokoban_goal_cell_vault_is_solvable() {
        let vi = 4;
        let mut g = Game::new(2);
        g.map = vec![Tile::Wall; COLS * MAP_H];
        g.monsters.clear();
        g.items.clear();
        g.blocks.clear();
        let rows: Vec<&str> = GAME.vaults[vi].lines().collect();
        let (vw, vh) = (rows[0].len() as i32, rows.len() as i32);
        let (ox, oy) = (5, 5);
        g.stamp_vault(GAME.vaults[vi], ox, oy);
        g.px = ox + vw / 2;
        g.py = oy + vh / 2;
        assert_eq!(g.items.len(), 3, "fixture: towel, coat, and the sword reward");
        assert_eq!(g.blocks.len(), 2, "fixture: the goal cell vault starts with a 2-chain");
        for _ in 0..2 {
            // West: pick up the towel, then the coat.
            g.apply_input(2);
        }
        assert_eq!(g.held, vec![TOWEL, COAT], "LIFO: coat picked up last, so it's on top (held.last())");
        for _ in 0..9 {
            // East: 2 to return to start, then 7 to solve the push puzzle
            // exactly as before (2-chain into the pit, survivor onto the
            // goal, walk to the reward).
            g.apply_input(3);
        }
        assert!(g.items.is_empty(), "the reward must have been reached and picked up");
        assert!(g.blocks.is_empty(), "the surviving block must have locked onto the goal");
    }

    #[test]
    fn sokoban_messages_fit_log_row() {
        let msgs = [
            "The floor drops away underfoot. You cannot cross.",
            "That row will not budge. Too many to push.",
            "There is nowhere for it to go.",
            "The block tips into the pit and is gone. The gap fills.",
            "The block settles into the goal. Something gives way.",
            "You shove the block.",
        ];
        for m in msgs {
            assert!(m.len() <= 78, "too long ({}): {}", m.len(), m);
        }
    }

    /// Every lore line must fit the 78-char log row for every slot filling,
    /// and so must the fixed-shape flavor messages — including the
    /// tier-crossing torch warnings (batch 3), which weave in a theme
    /// adjective via `self.adj()` and so must fit for EVERY theme's EVERY
    /// adjective, not just one.
    #[test]
    fn theme_lines_fit_log_row() {
        for lines in GAME.tone_lines {
            for line in lines {
                for k in GAME.room_kinds {
                    assert!(line.replace("{K}", k).len() <= 78);
                }
            }
        }
        for t in GAME.themes {
            assert!(format!("You enter {}.", t.label).len() <= 78);
            assert!(
                format!("You take {}. It is heavy. Climb, before dark!", t.objective_name).len() <= 78
            );
            for lore in &t.lore {
                for slot in &t.slots {
                    let line = lore.replace("{A}", slot);
                    assert!(line.len() <= 78, "too long ({}): {}", line.len(), line);
                }
            }
            for warn in &GAME.strings.tier_warnings {
                for adj in &t.adjs {
                    let line = warn.replace("{}", adj);
                    assert!(line.len() <= 78, "too long ({}): {}", line.len(), line);
                }
            }
        }
    }

    /// Every monster's talk-line template (batch 5, DECISION.md item 3 —
    /// the Henson ruling) must fit the 78-char log row for EVERY theme's
    /// mob-name filling — same length-test discipline as
    /// `theme_lines_fit_log_row` above, just keyed by monster index into
    /// `ThemeDef::mobs` instead of a flat slot table.
    #[test]
    fn talk_lines_fit_log_row() {
        for (ki, monster) in GAME.monsters.iter().enumerate() {
            for stage_lines in monster.talk_lines {
                for line in stage_lines {
                    for t in GAME.themes {
                        let filled = line.replace("{M}", t.mobs[ki]);
                        assert!(filled.len() <= 78, "too long ({}): {}", filled.len(), filled);
                    }
                }
            }
        }
    }

    /// Every GIVE/USE feedback and content string (batch 7 T2) must fit the
    /// 78-char log row, including `give_declined`'s `{}` filled by every
    /// theme's every mob name (same discipline as `talk_lines_fit_log_row`),
    /// and every item's `pickup_line`/`use_line` and `GiveRule::line`.
    #[test]
    fn give_use_strings_fit_log_row() {
        let statics = [
            GAME.strings.give_no_target,
            GAME.strings.give_empty_hands,
            GAME.strings.use_empty_hands,
            GAME.strings.use_no_effect,
        ];
        for s in statics {
            assert!(s.len() <= 78, "too long ({}): {}", s.len(), s);
        }
        for t in GAME.themes {
            for name in t.mobs {
                let filled = GAME.strings.give_declined.replace("{}", name);
                assert!(filled.len() <= 78, "too long ({}): {}", filled.len(), filled);
            }
        }
        for item in GAME.items {
            assert!(item.pickup_line.len() <= 78, "pickup_line too long: {}", item.pickup_line);
            assert!(item.use_line.len() <= 78, "use_line too long: {}", item.use_line);
        }
        for row in GAME.give_table {
            if let Some(line) = row.line {
                assert!(line.len() <= 78, "give_table line too long: {}", line);
            }
        }
    }

    /// Every McGuffin voice line (batch 8 T2: `GAME.carried_preamble` +
    /// every pool in `GAME.carried_lines`) must be pure ASCII — `render.rs`'s
    /// `put_str` maps each BYTE of a log string to one grid cell, so a
    /// stray multi-byte UTF-8 char (the source draft's em-dash, ASCII-
    /// normalized to `--` when this table was wired) would render as
    /// garbage cells and desync the 78-char row budget, which is itself
    /// measured in bytes, not chars. Same discipline as `talk_lines_fit_log_row`
    /// / `give_use_strings_fit_log_row` above, extended with an explicit
    /// ASCII check since these lines came from a prose draft, not a
    /// hand-typed template.
    #[test]
    fn mcguffin_lines_ascii_and_fit_log_row() {
        for line in GAME.carried_preamble {
            assert!(line.is_ascii(), "non-ASCII McGuffin preamble line: {}", line);
            assert!(line.len() <= 78, "too long ({}): {}", line.len(), line);
        }
        for (_, pool) in GAME.carried_lines {
            for line in *pool {
                assert!(line.is_ascii(), "non-ASCII McGuffin line: {}", line);
                assert!(line.len() <= 78, "too long ({}): {}", line.len(), line);
            }
        }
    }

    /// `CarryEvent::StairsUp`'s pool is indexed by `Game::speech_attempts`
    /// (the climb re-entry ladder), NOT drawn at random — order is
    /// load-bearing content, not an incidental array layout. Guard against a
    /// future accidental reorder: at least 4 entries (the short ladder),
    /// and the first/last rungs match the documented sequence (MCG_030
    /// "As I was saying--" opens it, MCG_045's steady-state closer is last).
    #[test]
    fn stairs_up_pool_is_in_documented_order() {
        let (_, pool) = GAME
            .carried_lines
            .iter()
            .find(|(e, _)| *e == crate::gamedef::CarryEvent::StairsUp)
            .expect("StairsUp pool must exist");
        assert!(pool.len() >= 4, "StairsUp pool too short: {}", pool.len());
        assert_eq!(pool[0], "As I was saying--");
        assert_eq!(*pool.last().unwrap(), "You breathe loudly for a legendary figure. It's humanizing. Keep it.");
    }

    /// A turn burns 1 light, 2 while carrying the amulet; walls burn nothing.
    #[test]
    fn light_burn_rates() {
        let mut g = Game::new(7);
        let l0 = g.light;
        g.wait_turn();
        assert_eq!(g.light, l0 - 1);
        g.has_objective = true;
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
            kind: OGRE,
            hp: 999,
            regard: 0,
            calm: false,
            awe: 0,
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
            kind: RAT,
            hp: 1, // guaranteed 1-hit kill
            regard: 0,
            calm: false,
            awe: 0,
        });
        g.light = 2; // 1 (turn) + 1 (tax) lands exactly on 0
        g.try_move_player(dx, dy);
        assert!(g.dead, "should die in the dark from the violence tax");
        assert_eq!(g.light, 0);
        assert!(!g.won, "lose is checked before win");
        assert!(g.killer.is_none(), "a darkness death has no combat killer");
    }

    /// Light-as-grace (batch 12 T1, the violence half): killing a monster
    /// dims the player's light by `kill_light_penalty`, on top of the
    /// ordinary attack-turn burn (base_burn + violence_tax) `spend_turn`
    /// applies afterward.
    #[test]
    fn killing_dims_light() {
        let mut g = Game::new(7);
        let (ox, oy) = (g.px + 1, g.py);
        g.monsters.clear();
        g.monsters.push(crate::game::Monster { kind: RAT, x: ox, y: oy, hp: 1, ..crate::game::Monster::spawn(RAT, ox, oy) });
        let light_before = g.light;
        g.apply_input(3); // bump East, kill the 1-hp rat
        assert!(g.monsters.iter().all(|m| !(m.x == ox && m.y == oy)), "rat should be dead");
        let ordinary = GAME.balance.base_burn + GAME.balance.violence_tax;
        assert_eq!(g.light, light_before - ordinary - GAME.balance.kill_light_penalty,
            "a kill must dim light by kill_light_penalty on top of the ordinary attack-turn burn");
    }

    /// Batch 12 R2: `Game::record_spare` no longer feeds light (the T2
    /// stipend was stripped — mercy pays at the top now, via the McGuffin's
    /// mood/shine at pickup in later tasks), but it stays the one
    /// consolidated becalm site, and every becalm path must still count
    /// toward `self.spared`. Driven via the deterministic awe path
    /// (`standing tall`, batch 11 T2) rather than the randomized
    /// talk-receptivity roll, exactly like the (now-removed) light-gain
    /// tests this replaces.
    #[test]
    fn becalm_still_counts_spared() {
        let mut g = Game::new(11);
        let (ox, oy) = (g.px + 1, g.py);
        g.monsters.clear();
        g.monsters.push(crate::game::Monster {
            kind: OGRE,
            x: ox,
            y: oy,
            hp: 99,
            ..crate::game::Monster::spawn(OGRE, ox, oy)
        });
        let thr = crate::game::Monster::stats(OGRE).awe_threshold as usize;
        let spared_before = g.spared;
        for _ in 0..thr {
            g.apply_input(4); // WAIT adjacent = stand tall
        }
        assert!(g.monsters.iter().any(|m| m.x == ox && m.y == oy && m.calm), "ogre becalmed via awe");
        assert_eq!(g.spared, spared_before + 1, "a becalm must still increment spared even with no light gain");
    }

    /// Running out of light on the exit tile is a LOSE, not a win.
    #[test]
    fn lose_beats_win_at_zero_light() {
        let mut g = Game::new(7);
        g.has_objective = true;
        // batch 12 R5 ("light as grace"): `has_objective = true` here is a
        // test shortcut that bypasses `Game::pickup` — in real play that
        // path always seeds the mood anchor in the same branch, so
        // `mood_count` is never 0 while `has_objective` is true. Pin a
        // genuine dark-tier (pure-brute) mood explicitly so this test
        // still exercises "she does not shine" rather than accidentally
        // reading `Game::mood`'s `mood_count == 0` neutral-50 fallback,
        // which would land in a non-dark shine tier and survive the
        // darkness this test means to prove is still fatal for a brute.
        g.mood_sum = 0;
        g.mood_count = 1;
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

    /// Talk until the (single, by construction) monster of `kind` registers
    /// a landed roll (`regard` advances) — parley is a `receptivity` ROLL
    /// now (batch 5 addendum), so a single `try_talk_player` call is no
    /// longer guaranteed to land; tests that need a landed talk retry
    /// against the fixed seed's deterministic `parley_rng` stream rather
    /// than assuming a single call always succeeds. A failed attempt still
    /// costs a turn and runs `monsters_act` normally (see
    /// `Game::try_talk_player`), so callers that also care about combat
    /// fallout from those interim turns get it for free. Panics if 200
    /// attempts never land (would indicate a receptivity or RNG-isolation
    /// regression, not ordinary variance).
    fn talk_until_landed(g: &mut Game, dx: i32, dy: i32, kind: MKind) -> u32 {
        for attempt in 1..=200u32 {
            let before = g.monsters.iter().find(|m| m.kind == kind).unwrap().regard;
            g.try_talk_player(dx, dy);
            let after = g.monsters.iter().find(|m| m.kind == kind).unwrap().regard;
            if after > before {
                return attempt;
            }
            assert!(!g.dead, "player died before a talk ever landed (attempt {})", attempt);
        }
        panic!("talk did not land within 200 attempts — receptivity or RNG isolation regression?");
    }

    /// Stayed swing (batch 5, DECISION.md item 3; addendum: only a LANDED
    /// talk stays its target): the turn a talk lands, the talked-to monster
    /// does not attack that turn (it is listening) — a second, un-talked-to
    /// monster adjacent and seeing the player attacks normally regardless,
    /// proving the mercy is per-monster, not a blanket "combat is off"
    /// toggle (crowds stay dangerous). The rat's hp is set near its kind
    /// max (1 of 3) purely to keep `receptivity` high so the retry loop
    /// below lands quickly and deterministically; the test's claim is about
    /// stayed-vs-not, not about the wound term.
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
            kind: RAT,
            hp: 1,
            regard: 0,
            calm: false,
            awe: 0,
        });
        g.monsters.push(Monster {
            x: px + bdx,
            y: py + bdy,
            kind: GOBLIN,
            hp: 6,
            regard: 0,
            calm: false,
            awe: 0,
        });
        let hp0 = g.hp;
        talk_until_landed(&mut g, adx, ady, RAT); // regard 0->1, threshold 2, not yet calm
        let rat = g.monsters.iter().find(|m| m.kind == RAT).unwrap();
        assert_eq!(rat.regard, 1, "the talked-to rat's regard should have incremented exactly once");
        assert!(!rat.calm, "one landed talk (of 2) should not yet calm a rat");
        assert!(
            g.hp < hp0,
            "the un-talked-to adjacent goblin should attack across the attempt(s) above"
        );
    }

    /// Becalm threshold + swap-on-bump (batch 5, DECISION.md item 3;
    /// addendum: threshold-crossing now requires two LANDED talks, not two
    /// calls): a rat (threshold 2) is not calm after one landed talk,
    /// becomes calm (and `spared` increments) on the second, and bumping a
    /// calmed monster swaps positions — no damage, no violence tax —
    /// instead of attacking.
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
            kind: RAT,
            hp: 1, // near its kind max wound term — keeps receptivity high
            regard: 0,
            calm: false,
            awe: 0,
        });
        let spared0 = g.spared;

        talk_until_landed(&mut g, dx, dy, RAT); // regard 0->1: below threshold 2
        assert!(!g.monsters[0].calm);
        assert_eq!(g.spared, spared0);

        talk_until_landed(&mut g, dx, dy, RAT); // regard 1->2: threshold reached
        assert!(g.monsters[0].calm, "the rat should be calm after 2 landed talks");
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

    /// Receptivity math vectors (batch 5 addendum): pins `game::receptivity`'s
    /// integer arithmetic to hand-computed values, independent of any RNG
    /// roll (the function takes no RNG — it's the PROBABILITY the roll is
    /// checked against, computed in `Game::try_talk_player`).
    #[test]
    fn receptivity_math_vectors() {
        let mut g = Game::new(1);
        g.monsters.clear();
        g.atk = 3; // Game::new's default; +6*(atk-3) term is 0
        let fresh_ogre = Monster { x: 0, y: 0, kind: OGRE, hp: 13, regard: 0, calm: false, awe: 0 };
        assert_eq!(receptivity(&fresh_ogre, &g), 20, "a fresh ogre should sit at exactly its BASE");

        // Wounded (1 of 13 hp -> wound term 40*(13-1)/13 = 36) plus a
        // strong player (atk 9 -> +6*(9-3) = 36) pushes well past 70:
        // 20 + 0 + 36 + 36 - 0 = 92.
        g.atk = 9;
        let wounded_ogre = Monster { x: 0, y: 0, kind: OGRE, hp: 1, regard: 0, calm: false, awe: 0 };
        let r = receptivity(&wounded_ogre, &g);
        assert!(r >= 70, "wounded ogre + high atk should land >= 70-ish, got {}", r);
        assert_eq!(r, 92, "and the exact integer math should hold");

        // Clamp floor: an unnaturally weak "player" (atk 0, below the
        // baseline 3) plus a guttering torch (fov_radius(1) is deep in the
        // bottom tier, well <= 4) drives the raw sum negative
        // (20 + 0 + 0 - 18 - 10 = -8); receptivity must still floor at 5.
        g.atk = 0;
        g.light = 1;
        let floor_ogre = Monster { x: 0, y: 0, kind: OGRE, hp: 13, regard: 0, calm: false, awe: 0 };
        assert_eq!(receptivity(&floor_ogre, &g), 5, "receptivity must clamp at the floor of 5");

        // Clamp ceiling: a high-regard, badly wounded rat with a very
        // strong player would compute far past 100; receptivity must cap
        // at 95.
        g.atk = 20;
        g.light = game::start_light();
        let capped_rat = Monster { x: 0, y: 0, kind: RAT, hp: 1, regard: 10, calm: false, awe: 0 };
        assert_eq!(receptivity(&capped_rat, &g), 95, "receptivity must clamp at the ceiling of 95");
    }

    /// batch 11 T1: bump-attacking an ogre always costs the player HP, even
    /// on a killing blow — the guaranteed-retaliation tax (`MonsterDef::
    /// retaliation`), separate from the ogre's ordinary `monsters_act` turn
    /// (which never gets to happen here, since the blow kills it).
    #[test]
    fn attacking_an_ogre_always_costs_hp() {
        let mut g = Game::new(7);
        // place a 1-hp ogre cardinally adjacent to the player, nothing else
        // in the way
        let (ox, oy) = (g.px + 1, g.py);
        g.monsters.clear();
        g.monsters.push(Monster { x: ox, y: oy, kind: OGRE, hp: 1, regard: 0, calm: false, awe: 0 });
        let hp_before = g.hp;
        g.apply_input(3); // move/bump East onto the ogre
        assert!(
            g.monsters.iter().all(|m| !(m.x == ox && m.y == oy)),
            "the 1-hp ogre should be dead"
        );
        assert!(g.hp < hp_before, "killing an ogre must still cost the player HP (guaranteed retaliation)");
    }

    /// batch 11 T1 fix round: a LETHAL ogre retaliation must advance
    /// `turns`/`light` for that turn exactly like every other death path
    /// does (`spend_turn`'s own dark-death branch increments `turns` and
    /// burns light before its early return; an ordinary combat death only
    /// ever happens from `monsters_act`, which runs after `spend_turn`
    /// already did). The original shape early-returned before `spend_turn`
    /// ever ran, so an ogre-retaliation kill under-counted `turns` by one
    /// and left `light` unburned — both fields are hashed (`state_hash`)
    /// and shown on the End screen.
    #[test]
    fn lethal_ogre_retaliation_advances_turns_and_light() {
        let mut g = Game::new(7);
        // A durable ogre (won't die to the player's hit) cardinally
        // adjacent to a player too frail to survive its guaranteed 3-point
        // retaliation (`OGRE`'s `retaliation` in the contractor cartridge).
        let (ox, oy) = (g.px + 1, g.py);
        g.monsters.clear();
        g.monsters.push(Monster { x: ox, y: oy, kind: OGRE, hp: 100, regard: 0, calm: false, awe: 0 });
        g.hp = 2;
        let ogre_name = g.theme().mobs[OGRE as usize];
        let turns_before = g.turns;
        let light_before = g.light;
        g.apply_input(3); // move/bump East onto the ogre
        assert!(g.dead, "the player should be dead from the guaranteed retaliation");
        assert_eq!(g.hp, 0, "hp should floor at 0, not go negative");
        assert_eq!(g.turns, turns_before + 1, "the fatal turn must still count like any other turn");
        let expected_burn = GAME.balance.base_burn + GAME.balance.violence_tax;
        assert_eq!(
            g.light,
            light_before - expected_burn,
            "the fatal turn must still burn light (base + violence tax), same as any other bump-attack turn"
        );
        assert_eq!(
            g.killer,
            Some(ogre_name),
            "a lethal retaliation is a combat death by the ogre, not a dark death"
        );
    }

    /// batch 11 T2: "standing tall" — waiting cardinally adjacent to an
    /// ogre for `awe_threshold` turns, WITHOUT ever attacking it, builds
    /// enough `Monster.awe` to becalm it via `Game::resolve_awe`, exactly
    /// like a landed talk would (`calm = true`, `Game::spared += 1`) — no
    /// violence involved, so `kills` stays 0.
    #[test]
    fn standing_tall_awes_an_ogre_into_calm() {
        let mut g = Game::new(11);
        let (ox, oy) = (g.px + 1, g.py);
        g.monsters.clear();
        g.monsters.push(crate::game::Monster {
            kind: OGRE,
            x: ox,
            y: oy,
            hp: 99,
            ..crate::game::Monster::spawn(OGRE, ox, oy)
        });
        let thr = crate::game::Monster::stats(OGRE).awe_threshold as usize;
        assert!(thr > 0, "ogre must be awe-able");
        for _ in 0..thr {
            g.apply_input(4); // WAIT adjacent = stand tall
        }
        assert!(
            g.monsters.iter().any(|m| m.x == ox && m.y == oy && m.calm),
            "ogre should becalm via awe"
        );
        assert_eq!(g.kills, 0, "standing tall is not violence — no kill");
    }

    /// batch 11 T3: the tactical-diplomat's ANSWER to an ogre is its existing
    /// cornered-talk — talk is a no-move action, so talking at an adjacent ogre
    /// holds ground and endures its hit exactly like a bare `wait`, and awes it
    /// into a becalm within `awe_threshold` turns regardless of the (low-odds
    /// for an ogre) talk roll. This is why T3 needed NO new bot branch: an
    /// explicit "wait to stand tall" would be redundant with talk AND worse (a
    /// bare wait risks portal-transit, the invariant batch 10 guarded). Proves
    /// the composition: talk-adjacent == stand tall.
    #[test]
    fn talking_at_an_ogre_stands_tall_and_awes() {
        let mut g = Game::new(11);
        let (ox, oy) = (g.px + 1, g.py); // ogre to the East
        g.monsters.clear();
        g.monsters.push(crate::game::Monster {
            kind: OGRE,
            x: ox,
            y: oy,
            hp: 99,
            ..crate::game::Monster::spawn(OGRE, ox, oy)
        });
        let thr = crate::game::Monster::stats(OGRE).awe_threshold as usize;
        for _ in 0..thr {
            g.apply_input(10); // talk East = 7 + dir(E=3): a no-move hold, not a swing
        }
        assert!(
            g.monsters.iter().any(|m| m.x == ox && m.y == oy && m.calm),
            "talking at an ogre (holding adjacent) should awe it into calm"
        );
        assert_eq!(g.kills, 0, "the diplomat never kills — awe is not violence");
    }

    /// batch 11 T2: stepping away from an awe-in-progress ogre resets
    /// `Monster.awe` to 0 (fleeing breaks the stare) — a monster that isn't
    /// cardinally adjacent this turn never becalms via awe regardless of
    /// how much awe it had built up before.
    #[test]
    fn fleeing_resets_ogre_awe() {
        let mut g = Game::new(11);
        let (ox, oy) = (g.px + 1, g.py);
        g.monsters.clear();
        g.monsters.push(crate::game::Monster {
            kind: OGRE,
            x: ox,
            y: oy,
            hp: 99,
            ..crate::game::Monster::spawn(OGRE, ox, oy)
        });
        let thr = crate::game::Monster::stats(OGRE).awe_threshold as usize;
        assert!(thr > 1, "test needs room to step away before the threshold is reached");
        // Build awe part-way (thr - 1 turns), staying adjacent throughout.
        for _ in 0..(thr - 1) {
            g.apply_input(4);
        }
        let ogre = g.monsters.iter().find(|m| m.x == ox && m.y == oy).expect("ogre still present");
        assert!(ogre.awe > 0, "awe should have built up while standing tall");
        assert!(!ogre.calm, "should not yet be calm — one turn short of threshold");
        // Step away one turn (west, back toward where the player started —
        // any direction off the ogre's tile breaks adjacency).
        let (dx, dy) = [(0, 1), (0, -1), (-1, 0)]
            .into_iter()
            .find(|&(dx, dy)| {
                let (nx, ny) = (g.px + dx, g.py + dy);
                in_map(nx, ny) && g.map[idx(nx, ny)] != Tile::Wall && (nx, ny) != (ox, oy)
            })
            .expect("fixture: at least one non-wall, non-ogre neighbor must exist");
        g.apply_input(match (dx, dy) {
            (0, -1) => 0,
            (0, 1) => 1,
            (-1, 0) => 2,
            (1, 0) => 3,
            _ => unreachable!(),
        });
        let ogre = g.monsters.iter().find(|m| m.kind == OGRE).expect("ogre still present");
        assert_eq!(ogre.awe, 0, "stepping away should reset awe to 0");
        assert!(!ogre.calm, "should not have becalmed");
    }

    /// batch 11 T2 fix round (review-found bug): a STRAIGHT-LINE retreat
    /// must never awe-becalm an ogre, even though the ogre chases at the
    /// same speed and re-establishes cardinal adjacency every turn. Before
    /// the fix, `Game::resolve_awe` measured adjacency AFTER `monsters_act`
    /// had already chased the ogre back next to the player, so a player who
    /// did nothing but walk away in a straight line still built awe and
    /// becalmed the ogre — unharmed, without ever standing tall. The ogre
    /// starts cardinally adjacent to the east; the player steps directly
    /// away (west) `awe_threshold` times. Since the player's OWN move
    /// increases distance to the ogre's start-of-turn position every single
    /// turn, this must never count as "holding ground."
    #[test]
    fn straight_retreat_from_ogre_does_not_awe() {
        let mut g = Game::new(11);
        let (ox, oy) = (g.px + 1, g.py); // ogre starts cardinally adjacent, to the east
        g.monsters.clear();
        g.monsters.push(crate::game::Monster {
            kind: OGRE,
            x: ox,
            y: oy,
            hp: 99,
            ..crate::game::Monster::spawn(OGRE, ox, oy)
        });
        let thr = crate::game::Monster::stats(OGRE).awe_threshold as usize;
        assert!(thr > 0, "ogre must be awe-able");
        // Step directly away (west, byte 2) every turn — a straight-line
        // retreat, never toward the ogre, never a wait/talk/attack.
        for _ in 0..thr {
            g.apply_input(2); // WEST
        }
        assert!(
            !g.monsters.iter().any(|m| m.kind == OGRE && m.calm),
            "a straight-line retreat must not awe-becalm the ogre"
        );
        assert_eq!(g.spared, 0, "retreat is not standing tall — no spare should be recorded");
    }

    /// Batch 12 R3 ("light as grace" — the grace half): a plain wait while
    /// hurt, with no non-calm hostile cardinally adjacent, heals
    /// `BalanceDef::rest_heal` HP (capped at `maxhp`). HP was the
    /// diplomat's real bottleneck, not light — this is the fix.
    #[test]
    fn resting_heals_when_no_hostile_adjacent() {
        let mut g = Game::new(1);
        g.monsters.clear();
        assert!(
            g.map[idx(g.px, g.py)] != Tile::Portal,
            "fixture: player must not start standing on a portal"
        );
        g.hp = g.maxhp - 5;
        let hp_before = g.hp;
        g.apply_input(4); // WAIT
        assert_eq!(
            g.hp,
            (hp_before + GAME.balance.rest_heal).min(g.maxhp),
            "resting with no hostile adjacent should heal rest_heal HP"
        );
    }

    /// Batch 12 R3: resting is capped at `maxhp` — waiting at full HP is a
    /// no-op for `Game::rest_heal` (the `self.hp >= self.maxhp` guard),
    /// same "don't heal a corpse [or the already-topped-up]" spirit as the
    /// dead/won early return at the top of `wait_turn`.
    #[test]
    fn resting_does_not_overheal_past_maxhp() {
        let mut g = Game::new(1);
        g.monsters.clear();
        assert_eq!(g.hp, g.maxhp, "fixture: a fresh game starts at full HP");
        g.apply_input(4); // WAIT
        assert_eq!(g.hp, g.maxhp, "resting at full HP must never exceed maxhp");
    }

    /// Batch 12 R3: the REQUIRED gate — waiting cardinally adjacent to a
    /// live, non-calm, fight-capable monster must NOT heal, so rest and
    /// awe-holding (`resolve_awe`, batch 11 T2 — "stand tall while an ogre
    /// pummels you") stay two distinct acts. The adjacent monster still
    /// gets its ordinary `monsters_act` attack this same turn (waiting
    /// isn't a talk, so it's never `stayed`) — the fixture precomputes that
    /// exact combat roll from the SAME `combat` channel `Game::new` seeds
    /// (the first-ever `combat_rng` draw in this run, since nothing else
    /// touches it before this turn), so the assertion proves the heal did
    /// NOT land on top of the hit, not merely that the net change is
    /// negative (which a partially-offset heal could still satisfy).
    #[test]
    fn resting_does_not_heal_adjacent_to_hostile() {
        let seed = 1;
        let mut g = Game::new(seed);
        let (rx, ry) = (g.px + 1, g.py);
        assert!(
            in_map(rx, ry) && g.map[idx(rx, ry)] != Tile::Wall,
            "fixture: east of the player must be open floor"
        );
        g.monsters.clear();
        g.monsters.push(Monster { x: rx, y: ry, kind: RAT, hp: 99, regard: 0, calm: false, awe: 0 });
        g.hp = g.maxhp - 5;
        let hp_before = g.hp;
        let mut crng = channel(seed, &["combat"]);
        let expected_dmg = crate::game::Monster::stats(RAT).atk + crng.range(0, 2);
        g.apply_input(4); // WAIT
        assert!(!g.dead, "fixture: the player must survive the rat's hit");
        assert_eq!(
            g.hp,
            hp_before - expected_dmg,
            "an adjacent live hostile still attacks on a wait, but rest must not also heal that same turn"
        );
    }

    /// Batch 12 R3 fix round: the reviewer's exact repro — `Game::rest_heal`
    /// originally tested CARDINAL adjacency only, but `monsters_act`'s
    /// attack decision (and the chase AI that parks a monster next to the
    /// player) uses CHEBYSHEV adjacency, which includes the diagonals. A
    /// live, non-calm, fight-capable monster parked DIAGONALLY adjacent
    /// still attacks every turn — so the old cardinal-only gate saw no
    /// hostile neighbor and healed anyway, landing a heal on top of the
    /// same turn's hit (free healing under a live attacker). This test must
    /// FAIL on the old cardinal-only code and PASS once the gate matches
    /// `monsters_act`'s Chebyshev formula. Same precise-math discipline as
    /// `resting_does_not_heal_adjacent_to_hostile` above: the fixture
    /// precomputes the exact combat roll from the same first-ever
    /// `combat_rng` draw, so the assertion proves the heal did NOT land on
    /// top of the hit, not merely that the net change is negative.
    #[test]
    fn resting_does_not_heal_diagonally_adjacent_to_hostile() {
        let seed = 1;
        let mut g = Game::new(seed);
        let (rx, ry) = (g.px + 1, g.py + 1);
        assert!(
            in_map(rx, ry) && g.map[idx(rx, ry)] != Tile::Wall,
            "fixture: the tile diagonally adjacent (SE) to the player must be open floor"
        );
        g.monsters.clear();
        g.monsters.push(Monster { x: rx, y: ry, kind: RAT, hp: 99, regard: 0, calm: false, awe: 0 });
        g.hp = g.maxhp - 5;
        let hp_before = g.hp;
        let mut crng = channel(seed, &["combat"]);
        let expected_dmg = crate::game::Monster::stats(RAT).atk + crng.range(0, 2);
        g.apply_input(4); // WAIT
        assert!(!g.dead, "fixture: the player must survive the rat's hit");
        assert_eq!(
            g.hp,
            hp_before - expected_dmg,
            "a diagonally-adjacent live hostile still attacks on a wait (Chebyshev), \
             so rest must not also heal that same turn"
        );
    }

    /// Batch 12 R3: `Game::wait_turn`'s portal-footing guard — rest is only
    /// ever attempted from the non-transiting branch, so a wait while
    /// standing on a portal must transit exactly as before (batch 6) AND
    /// never apply a heal on that same turn (the clean rule chosen over
    /// entangling the two: a transiting turn ends the level/world, so
    /// healing it has no meaning).
    #[test]
    fn resting_does_not_fire_on_a_transiting_wait() {
        let mut g = Game::new(1);
        g.depth = 2;
        g.gen_level();
        let (px, py, _dest) = g.portal.expect("fixture: seed 1 depth 2 has a portal");
        g.monsters.clear();
        g.px = px;
        g.py = py;
        g.hp = g.maxhp - 5; // hurt, so an (incorrect) rest would be observable
        let hp_before = g.hp;
        let world_before = g.world;
        g.wait_turn();
        assert!(g.world != world_before, "waiting while standing on a portal must still transit");
        assert_eq!(
            g.hp,
            hp_before,
            "rest must never fire on a transiting wait — the portal-footing guard"
        );
    }

    /// Batch 12 R7: the end-to-end dispatch — a wait that MENDS while carrying
    /// speaks a mood-keyed rest line, `RestedBright` when her ring is wide
    /// (radius >= 4, mood >= 50) and `RestedDim` otherwise. This is the
    /// "does the feature actually fire in play" guard (this repo has shipped
    /// features that were computed but silently never surfaced), proving the
    /// `wait_turn` branch reaches the right pool, not just that the pools
    /// exist. A wait that heals nothing must NOT speak a rest line.
    #[test]
    fn resting_speaks_mood_keyed_line_while_carrying() {
        let pool_for = |ev: CarryEvent| {
            GAME.carried_lines
                .iter()
                .find(|(e, _)| *e == ev)
                .expect("rest pool must exist")
                .1
        };
        let spoke_from = |g: &Game, before: usize, pool: &[&str]| {
            g.msgs[before..].iter().any(|m| pool.contains(&m.as_str()))
        };

        // BRIGHT: mood 100 -> radius 6 -> a wide ring.
        let mut g = Game::new(1);
        g.monsters.clear();
        assert!(g.map[idx(g.px, g.py)] != Tile::Portal, "fixture: not on a portal");
        g.has_objective = true;
        g.hp = g.maxhp - 5;
        g.mood_sum = 100;
        g.mood_count = 1;
        let before = g.msgs.len();
        g.apply_input(4); // WAIT
        assert!(g.hp > g.maxhp - 5, "fixture: the wait must actually mend");
        assert!(
            spoke_from(&g, before, pool_for(CarryEvent::RestedBright)),
            "a mending wait at a bright mood must speak a RestedBright line"
        );
        assert!(
            !spoke_from(&g, before, pool_for(CarryEvent::RestedDim)),
            "must not also speak the dim line"
        );

        // DIM: mood 0 -> radius 0 -> gone dark, but rest still heals.
        let mut g = Game::new(1);
        g.monsters.clear();
        assert!(g.map[idx(g.px, g.py)] != Tile::Portal, "fixture: not on a portal");
        g.has_objective = true;
        g.hp = g.maxhp - 5;
        g.mood_sum = 0;
        g.mood_count = 1;
        let before = g.msgs.len();
        g.apply_input(4); // WAIT
        assert!(g.hp > g.maxhp - 5, "fixture: the wait must actually mend");
        assert!(
            spoke_from(&g, before, pool_for(CarryEvent::RestedDim)),
            "a mending wait at a dim mood must speak a RestedDim line"
        );

        // A wait that heals NOTHING (already full HP) speaks no rest line.
        let mut g = Game::new(1);
        g.monsters.clear();
        assert!(g.map[idx(g.px, g.py)] != Tile::Portal, "fixture: not on a portal");
        g.has_objective = true;
        assert_eq!(g.hp, g.maxhp, "fixture: fresh game is full HP");
        let before = g.msgs.len();
        g.apply_input(4); // WAIT
        assert!(
            !spoke_from(&g, before, pool_for(CarryEvent::RestedBright))
                && !spoke_from(&g, before, pool_for(CarryEvent::RestedDim)),
            "a wait that mends nothing must speak no rest line"
        );
    }

    /// Landed-vs-failed determinism (batch 5 addendum): two independent
    /// live games from the same seed, talked at the same fresh goblin the
    /// same number of times, produce an identical `state_hash` — whether
    /// each individual roll happened to land or fail. `parley_rng` has no
    /// external entropy, so the exact sequence of landed/failed outcomes
    /// must repeat exactly. Also confirms the scenario actually exercises
    /// both outcomes (this kind's BASE receptivity makes at least one
    /// failure near-certain within the attempt cap; regard climbing toward
    /// `Monster::talk_threshold` makes at least one landing certain too).
    ///
    /// batch 11 T2: switched from OGRE to GOBLIN. The ogre is now
    /// awe-able (`MonsterDef::awe_threshold` 3) — repeatedly talking to an
    /// adjacent, non-attacked monster ALSO builds `Monster.awe`
    /// (`Game::resolve_awe` doesn't distinguish "why" the player didn't
    /// attack this turn), so a loop of pure talk against a fresh ogre now
    /// always becalms it via awe by the 3rd attempt regardless of any
    /// talk landing — talk_threshold 4 can never be reached first. The
    /// goblin (awe_threshold 0) isn't awe-able at all, so it isolates the
    /// parley-determinism property this test is actually about from the
    /// new stand-tall mechanic.
    #[test]
    fn parley_landed_vs_failed_deterministic() {
        let run = |seed: u64| -> (u64, bool, bool) {
            let mut g = Game::new(seed);
            g.monsters.clear();
            // Headroom: a fresh goblin's failed rolls attack for real damage
            // (it is never stayed on a failed roll — that's the property
            // under test), and this kind's BASE receptivity means several
            // fails are likely before the first landing.
            g.hp = 5000;
            g.maxhp = 5000;
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
                kind: GOBLIN,
                hp: 6,
                regard: 0,
                calm: false,
                awe: 0,
            });
            let (mut saw_landed, mut saw_failed) = (false, false);
            for _ in 0..50 {
                if g.monsters[0].calm || g.dead {
                    break;
                }
                let before = g.monsters[0].regard;
                g.try_talk_player(dx, dy);
                if g.monsters[0].regard > before {
                    saw_landed = true;
                } else {
                    saw_failed = true;
                }
            }
            (state_hash(&g), saw_landed, saw_failed)
        };
        let (h1, landed1, failed1) = run(13);
        let (h2, landed2, failed2) = run(13);
        assert_eq!(h1, h2, "two identical live runs must hash identically");
        assert!(landed1, "expected at least one landed talk within 50 attempts");
        assert!(failed1, "expected at least one failed talk within 50 attempts");
        assert_eq!((landed1, failed1), (landed2, failed2), "both runs must see the same outcomes");
    }

    /// A failed talk does NOT stay its target (batch 5 addendum): the first
    /// FAILED roll against a fresh, low-receptivity ogre still costs the
    /// player hp that same call, proving `stayed` is `None` on the failed
    /// branch of `Game::try_talk_player` so the un-stayed monster attacks
    /// normally in `monsters_act`.
    #[test]
    fn failed_talk_does_not_stay_monster() {
        let mut g = Game::new(3);
        g.monsters.clear();
        // Headroom so several failed rounds' worth of ogre damage can't
        // kill the player before a failed roll is observed.
        g.hp = 5000;
        g.maxhp = 5000;
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
            kind: OGRE,
            hp: 13,
            regard: 0,
            calm: false,
            awe: 0,
        });
        let mut found = false;
        for _ in 0..200 {
            if g.monsters[0].calm {
                break; // ran out of failed attempts before becalming
            }
            let before_regard = g.monsters[0].regard;
            let hp0 = g.hp;
            g.try_talk_player(dx, dy);
            if g.monsters[0].regard == before_regard {
                assert!(g.hp < hp0, "an un-stayed ogre should attack the turn its talk failed");
                found = true;
                break;
            }
        }
        assert!(found, "expected at least one failed talk within 200 attempts before becalming");
    }

    /// Parley channel isolation (batch 5 addendum): burning `parley_rng`
    /// must never perturb `combat_rng` — same discipline
    /// `worldgen_isolated_from_combat` proves for combat/ai draws vs
    /// worldgen, checked here against a live combat sequence instead of a
    /// level dump.
    #[test]
    fn parley_isolated_from_combat() {
        let seed = 0xFEEDu64;
        let mut a = Game::new(seed);
        for _ in 0..1000 {
            a.parley_rng.next();
        }
        let mut b = Game::new(seed);
        // Bump-attack the same scripted number of times on both games and
        // compare the surviving monster's hp: if parley_rng draws had
        // leaked into combat_rng, the damage rolls (and so this hp) would
        // diverge between the two.
        let fight = |g: &mut Game| -> i32 {
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
                kind: OGRE,
                hp: 999,
                regard: 0,
                calm: false,
                awe: 0,
            });
            for _ in 0..10 {
                g.try_move_player(dx, dy);
            }
            g.monsters[0].hp
        };
        let hp_a = fight(&mut a);
        let hp_b = fight(&mut b);
        assert_eq!(hp_a, hp_b, "parley_rng draws must not perturb combat_rng's sequence");
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

    /// Talk determinism + replay round-trip (batch 5, DECISION.md item 3):
    /// a scripted log mixing move/wait/talk bytes (0-4, 7-10) replays to an
    /// identical `state_hash` every time — the same determinism proof
    /// `save_replay_roundtrip` makes for the pre-mercy vocabulary, now
    /// covering the new bytes.
    #[test]
    fn talk_bytes_replay_deterministic() {
        let seed0 = 88u64;
        let mut script = channel(seed0, &["test", "talk_script"]);
        let mut log: Vec<u8> = Vec::new();
        for _ in 0..500 {
            // 0..=4 move/wait, 7..=10 talk-N/S/W/E — skip 5/6, which are
            // reconstruction-layer bytes handled outside apply_input.
            let roll = script.range(0, 9) as u8;
            let b = if roll < 5 { roll } else { roll + 2 };
            log.push(b);
        }
        let a = replay(seed0, &log);
        let b = replay(seed0, &log);
        assert_eq!(state_hash(&a), state_hash(&b));
    }

    // ---------- GIVE/USE (batch 7 T2, story §5/§9-A) ----------

    /// Same determinism proof as `talk_bytes_replay_deterministic`, extended
    /// to the full save-v4 vocabulary (0-4, 7-15 — give bytes 11-14 and use
    /// byte 15 alongside move/wait/talk).
    #[test]
    fn give_use_bytes_replay_deterministic() {
        let seed0 = 89u64;
        let mut script = channel(seed0, &["test", "give_use_script"]);
        let mut log: Vec<u8> = Vec::new();
        for _ in 0..500 {
            // 0..=4 move/wait, 7..=15 talk/give/use — skip 5/6, the
            // reconstruction-layer bytes handled outside apply_input.
            let roll = script.range(0, 14) as u8;
            let b = if roll < 5 { roll } else { roll + 2 };
            log.push(b);
        }
        let a = replay(seed0, &log);
        let b = replay(seed0, &log);
        assert_eq!(state_hash(&a), state_hash(&b));
    }

    /// `Game.held` (LIFO) is hashed, order and all: two otherwise-identical
    /// games differing only in held ORDER must hash differently, and two
    /// games given the identical held vector must hash identically.
    #[test]
    fn held_is_hashed() {
        let mut a = Game::new(9);
        let mut b = Game::new(9);
        assert_eq!(state_hash(&a), state_hash(&b));

        a.held = vec![POTION, CHEESE];
        b.held = vec![CHEESE, POTION];
        assert_ne!(state_hash(&a), state_hash(&b), "held order must be part of state_hash");

        let mut c = Game::new(9);
        c.held = vec![POTION, CHEESE];
        assert_eq!(state_hash(&a), state_hash(&c), "identical held vectors must hash identically");
    }

    /// GIVE at an empty tile: no monster there, no-op, no turn, held
    /// untouched.
    #[test]
    fn give_at_empty_tile_is_noop() {
        let mut g = blank_room(1);
        g.held = vec![CHEESE];
        let before_turns = g.turns;
        g.apply_input(11); // give-N: nothing north of a freshly-cleared room
        assert_eq!(g.turns, before_turns, "an empty-tile give must cost no turn");
        assert_eq!(g.held, vec![CHEESE], "an empty-tile give must not touch held");
    }

    /// GIVE with empty hands: a monster IS adjacent, but nothing is held —
    /// no-op, no turn.
    #[test]
    fn give_with_empty_hands_is_noop() {
        let mut g = blank_room(1);
        g.monsters.push(Monster { x: g.px, y: g.py - 1, kind: RAT, hp: 3, regard: 0, calm: false, awe: 0 });
        let before_turns = g.turns;
        assert!(g.held.is_empty(), "fixture: nothing held");
        g.apply_input(11); // give-N
        assert_eq!(g.turns, before_turns, "an empty-handed give must cost no turn");
    }

    /// GIVE with a monster present and something held, but no give-table
    /// row for that (item, kind) pair (coat has no give-target this batch):
    /// graceful no-op, no turn, item stays held.
    #[test]
    fn give_declined_when_no_matching_rule() {
        let mut g = blank_room(1);
        g.monsters.push(Monster { x: g.px, y: g.py - 1, kind: RAT, hp: 3, regard: 0, calm: false, awe: 0 });
        g.held = vec![COAT];
        let before_turns = g.turns;
        g.apply_input(11); // give-N
        assert_eq!(g.turns, before_turns, "a declined give must cost no turn");
        assert_eq!(g.held, vec![COAT], "a declined give must not consume the item");
    }

    /// Give cheese to a rat: the story's D1 grievance — a measured regard
    /// PENALTY, applied through the give path (not new flavor text — see
    /// `GiveRule::line`'s doc comment on reusing the rat's own stage-3
    /// "unmoved" talk line).
    #[test]
    fn cheese_to_rat_is_a_regard_penalty() {
        let mut g = blank_room(1);
        g.monsters.push(Monster { x: g.px, y: g.py - 1, kind: RAT, hp: 3, regard: 1, calm: false, awe: 0 });
        g.held = vec![CHEESE];
        let before_turns = g.turns;
        g.apply_input(11); // give-N
        assert_eq!(g.monsters[0].regard, 0, "cheese-to-rat must apply the -2 [TUNE] penalty (saturating)");
        assert!(g.held.is_empty(), "cheese must be consumed by a landed give");
        assert_eq!(g.turns, before_turns + 1, "a landed give costs a turn");
    }

    /// Give the potion to a wounded monster: heals it to full AND raises
    /// regard (story §5: "the single biggest regard event in the game").
    #[test]
    fn potion_given_heals_full_and_raises_regard() {
        let mut g = blank_room(1);
        g.monsters.push(Monster { x: g.px, y: g.py - 1, kind: RAT, hp: 1, regard: 0, calm: false, awe: 0 });
        g.held = vec![POTION];
        g.apply_input(11); // give-N
        let maxhp = Monster::stats(RAT).hp;
        assert_eq!(g.monsters[0].hp, maxhp, "potion-gift must heal the target to full");
        assert_eq!(g.monsters[0].regard, 3, "potion-gift must apply the +3 [TUNE] regard bonus");
        assert!(g.held.is_empty(), "the potion must be consumed by a landed give");
    }

    /// USE with nothing held: no-op, no turn.
    #[test]
    fn use_with_empty_hands_is_noop() {
        let mut g = blank_room(1);
        let before_turns = g.turns;
        g.apply_input(15);
        assert_eq!(g.turns, before_turns);
    }

    /// USE on a held item with no `on_use` (coat, towel): graceful no-op,
    /// no turn, item stays held.
    #[test]
    fn use_on_no_effect_item_is_noop() {
        let mut g = blank_room(1);
        g.held = vec![TOWEL];
        let before_turns = g.turns;
        g.apply_input(15);
        assert_eq!(g.turns, before_turns);
        assert_eq!(g.held, vec![TOWEL], "a no-effect use must not consume the item");
    }

    /// USE-ing a potion applies exactly the potion's AUTHORED heal amount
    /// (`UseEffect::Heal(n)` in the cartridge), capped by maxhp - hp. Reads `n`
    /// from GAME rather than hardcoding it, so a balance re-tune of the heal
    /// (batch 11's heal-scarcity pass cut it 8 -> 3) can't silently break the
    /// "USE applies the authored heal" invariant — the test tracks the value.
    #[test]
    fn potion_use_applies_authored_heal() {
        let mut g = blank_room(1);
        g.hp = 1;
        let hp_before = g.hp;
        g.held = vec![POTION];
        g.apply_input(15);
        let heal = match GAME.items[POTION as usize].on_use {
            Some(crate::gamedef::UseEffect::Heal(n)) => n,
            _ => panic!("potion should have a Heal on_use"),
        };
        let expected = hp_before + heal.min(g.maxhp - hp_before);
        assert_eq!(g.hp, expected, "USE-ing a potion must apply the authored heal amount");
        assert!(g.held.is_empty(), "a landed use must consume the potion");
    }

    /// USE-ing cheese burns it for +8 [TUNE] light, minus the ordinary
    /// per-turn burn `spend_turn` still applies on top (USE is a turn like
    /// any other).
    #[test]
    fn cheese_use_burns_for_light() {
        let mut g = blank_room(1);
        let light_before = g.light;
        g.held = vec![CHEESE];
        g.apply_input(15);
        assert_eq!(
            g.light,
            light_before + 8 - GAME.balance.base_burn,
            "cheese USE must add 8 [TUNE] light, then pay the ordinary per-turn burn"
        );
        assert!(g.held.is_empty(), "cheese must be consumed by a landed use");
    }

    /// The sim bots' deterministic USE rule (headless.rs `sim_seed`): once
    /// hp drops to half of maxhp or below and a potion is held, USE fires.
    /// This exercises the same condition/action pair directly (not through
    /// a full sim run, which `sim_deterministic`/`pacifist_policy_
    /// deterministic` already cover end to end).
    #[test]
    fn bot_use_rule_heals_when_wounded_and_holding_potion() {
        let mut g = blank_room(1);
        g.hp = g.maxhp / 2; // exactly at the threshold: 2*hp <= maxhp
        let hp_before = g.hp;
        g.held = vec![POTION];
        assert!(2 * g.hp <= g.maxhp, "fixture: at or under half HP");
        g.apply_input(15);
        assert!(g.hp > hp_before, "USE-ing the held potion must heal");
        assert!(g.held.is_empty());
    }

    // ---------- PUT DOWN / CarryEvent (batch 8 T1, story §9-B/C/D) ----------

    /// PUT DOWN (byte 16) while not carrying the objective: graceful no-op,
    /// no turn, no item pushed.
    #[test]
    fn put_down_noop_when_not_carrying() {
        let mut g = blank_room(3);
        let before_light = g.light;
        let before_items = g.items.len();
        g.put_down();
        assert!(!g.has_objective);
        assert_eq!(g.light, before_light, "a no-op put-down must not spend a turn");
        assert_eq!(g.items.len(), before_items);
    }

    /// PUT DOWN refuses to stack onto a tile that already holds an item:
    /// graceful no-op, no turn, objective stays carried.
    #[test]
    fn put_down_noop_when_tile_occupied() {
        let mut g = blank_room(3);
        g.has_objective = true;
        g.items.push(Item { x: g.px, y: g.py, kind: GAME.win.objective_item });
        let before_light = g.light;
        let before_len = g.items.len();
        g.put_down();
        assert!(g.has_objective, "an occupied tile must refuse the put-down");
        assert_eq!(g.light, before_light, "a refused put-down must not spend a turn");
        assert_eq!(g.items.len(), before_len, "a refused put-down must not push a duplicate item");
    }

    /// The carry-burn flip is automatic (`Game::spend_turn` already branches
    /// on `has_objective`): carrying burns `WinDef::carry_burn`, dropping it
    /// reverts to `BalanceDef::base_burn`, and walking back onto the
    /// dropped objective (re-pickup) restores `carry_burn` again — proven
    /// both directions in one scripted round trip.
    #[test]
    fn put_down_and_pickup_flip_light_burn_both_ways() {
        let mut g = blank_room(3);
        let (px0, py0) = (g.px, g.py);
        g.has_objective = true;

        let before = g.light;
        g.wait_turn();
        assert_eq!(before - g.light, GAME.win.carry_burn, "carrying: a turn burns carry_burn");

        g.put_down();
        assert!(!g.has_objective, "put_down must clear has_objective");
        assert!(g.objective_dropped, "put_down must set objective_dropped");
        assert!(
            g.items.iter().any(|it| (it.x, it.y) == (px0, py0) && it.kind == GAME.win.objective_item),
            "the objective must re-enter the item list at the player's tile"
        );

        let before2 = g.light;
        g.wait_turn();
        assert_eq!(before2 - g.light, GAME.balance.base_burn, "after put-down: a turn reverts to base_burn");

        g.try_move_player(1, 0); // step off the dropped objective's tile
        assert_eq!((g.px, g.py), (px0 + 1, py0), "setup: must have actually moved east");
        g.try_move_player(-1, 0); // step back onto it: re-pickup
        assert_eq!((g.px, g.py), (px0, py0));
        assert!(g.has_objective, "walking back onto the dropped objective re-picks it up");
        assert!(!g.objective_dropped, "objective_dropped must clear on re-pickup");

        let before3 = g.light;
        g.wait_turn();
        assert_eq!(before3 - g.light, GAME.win.carry_burn, "after re-pickup: carry_burn resumes");
    }

    /// Byte 16 (`apply_input`) dispatches to `Game::put_down`.
    #[test]
    fn apply_input_byte_16_triggers_put_down() {
        let mut g = blank_room(3);
        g.has_objective = true;
        let before_len = g.items.len();
        g.apply_input(16);
        assert!(!g.has_objective);
        assert_eq!(g.items.len(), before_len + 1);
    }

    /// batch 8 T2: with the active cartridge's `carried_lines` table now
    /// FILLED (story content wired), `Game::carry_event` must (a) log
    /// exactly one line and draw exactly one `flavor_rng` value for every
    /// event that HAS a populated pool, and (b) remain a provable no-op —
    /// no RNG draw, no log line — for the three events this batch still
    /// ships with no row at all (`MonsterAdjacent`/`TierCrossed`/`Idle`,
    /// future-content gaps). Either way, `combat_rng`/`ai_rng`/`parley_rng`
    /// must never move: the McGuffin's voice only ever touches the log and
    /// `flavor_rng`, never a channel that feeds combat/movement/death/
    /// kill/spare/stuck — this is what keeps goldens/solve/sim/xhash
    /// byte-identical to the pre-batch-8 baseline despite real content now
    /// being spoken in play.
    #[test]
    fn carry_event_only_touches_flavor_rng_and_the_log() {
        assert!(!GAME.carried_lines.is_empty(), "T2 fills carried_lines with story content");
        assert!(!GAME.carried_preamble.is_empty(), "T2 fills carried_preamble with story content");
        let populated: Vec<CarryEvent> = GAME.carried_lines.iter().map(|(e, _)| *e).collect();
        for ev in [
            CarryEvent::PickedUpBloody,
            CarryEvent::PickedUpMerciful,
            CarryEvent::PutDown,
            CarryEvent::PickedBackUp,
            CarryEvent::StairsUp,
            CarryEvent::MonsterAdjacent,
            CarryEvent::KillWitnessed,
            CarryEvent::SpareWitnessed,
            CarryEvent::TierCrossed,
            CarryEvent::Idle,
            CarryEvent::RestedBright,
            CarryEvent::RestedDim,
        ] {
            let mut g = blank_room(1);
            g.has_objective = true;
            let before_combat = g.combat_rng.0;
            let before_ai = g.ai_rng.0;
            let before_parley = g.parley_rng.0;
            let before_msgs = g.msgs.len();
            g.carry_event(ev);
            assert_eq!(g.combat_rng.0, before_combat, "carry_event must never draw combat_rng ({:?})", ev);
            assert_eq!(g.ai_rng.0, before_ai, "carry_event must never draw ai_rng ({:?})", ev);
            assert_eq!(g.parley_rng.0, before_parley, "carry_event must never draw parley_rng ({:?})", ev);
            if populated.contains(&ev) {
                assert_eq!(g.msgs.len(), before_msgs + 1, "a populated pool must log exactly one line ({:?})", ev);
            } else {
                assert_eq!(g.msgs.len(), before_msgs, "an event with no row must log nothing ({:?})", ev);
            }
        }
    }

    /// batch 8 T2: a real FIRST walk-over pickup of the win-condition item
    /// logs the fixed `carried_preamble` (in order, unconditionally), then
    /// exactly one `PickedUpBloody`/`PickedUpMerciful` line chosen by
    /// `flavor_rng`, alongside the pre-existing `pickup_objective` line —
    /// and none of it touches `combat_rng`/`ai_rng`/`parley_rng`.
    #[test]
    fn first_objective_pickup_speaks_preamble_and_register_line() {
        assert!(!GAME.carried_preamble.is_empty());
        assert!(!GAME.carried_lines.is_empty());
        let mut g = blank_room(1);
        g.items.push(Item { x: g.px + 1, y: g.py, kind: GAME.win.objective_item });
        let before_combat = g.combat_rng.0;
        let before_ai = g.ai_rng.0;
        let before_parley = g.parley_rng.0;
        g.try_move_player(1, 0);
        assert!(g.has_objective, "walking onto the objective item must pick it up");
        assert_eq!(g.combat_rng.0, before_combat, "pickup must never draw combat_rng");
        assert_eq!(g.ai_rng.0, before_ai, "pickup must never draw ai_rng");
        assert_eq!(g.parley_rng.0, before_parley, "pickup must never draw parley_rng");
        assert!(
            g.msgs.iter().any(|m| m.contains("heavy")),
            "the pre-existing pickup_objective line must still appear verbatim"
        );
        for line in GAME.carried_preamble {
            assert!(g.msgs.iter().any(|m| m == line), "preamble line must be logged verbatim: {}", line);
        }
        let bloody = GAME.carried_lines.iter().find(|(e, _)| *e == CarryEvent::PickedUpBloody).unwrap().1;
        let merciful = GAME.carried_lines.iter().find(|(e, _)| *e == CarryEvent::PickedUpMerciful).unwrap().1;
        assert!(
            g.msgs.iter().any(|m| bloody.contains(&m.as_str()) || merciful.contains(&m.as_str())),
            "exactly one register line (bloody or merciful) must be logged"
        );
    }

    /// The climb re-entry ladder: `speech_attempts` increments exactly once
    /// per `Game::ascend` call made WHILE carrying the objective, and not at
    /// all when ascending without it.
    #[test]
    fn speech_attempts_only_increments_on_ascend_while_carrying() {
        let mut g = Game::new(2);
        g.depth = 2;
        g.gen_level();
        assert_eq!(g.speech_attempts, 0);
        g.ascend();
        assert_eq!(g.speech_attempts, 0, "ascend while NOT carrying must not increment speech_attempts");

        g.depth = 2;
        g.gen_level();
        g.has_objective = true;
        g.ascend();
        assert_eq!(g.speech_attempts, 1, "ascend while carrying must increment speech_attempts exactly once");
    }

    /// Fix-round: `speech_attempts` counts exactly N after N carried
    /// ascents in a row — proves Fix B's reordering (fire `StairsUp` before
    /// incrementing) didn't perturb the final per-run count, only which
    /// pre-increment value a given ascent's `carry_event` call reads.
    #[test]
    fn speech_attempts_counts_n_carried_ascents_in_a_row() {
        let mut g = Game::new(7);
        g.has_objective = true;
        for n in 1..=4u8 {
            g.depth = 2;
            g.gen_level();
            g.ascend();
            assert_eq!(g.speech_attempts, n, "after {n} carried ascents in a row, speech_attempts must equal {n}");
        }
    }

    /// Fix-round (StairsUp ladder off-by-one): `Game::ascend` must fire
    /// `CarryEvent::StairsUp` BEFORE incrementing `speech_attempts`, so the
    /// FIRST carried ascent reads pre-increment index 0. Observed indirectly
    /// via `mcguffin_last_line_turn`/RNG being untouched is not possible
    /// with an empty `carried_lines` table (the whole call is a no-op), so
    /// this test instead pins the ordering directly: `speech_attempts` must
    /// still read 0 at the instant `carry_event(StairsUp)` would consult it
    /// (i.e. immediately before the post-call increment lands), which is
    /// exactly what `speech_attempts_only_increments_on_ascend_while_carrying`
    /// and the N-in-a-row test above jointly prove (0 before the first
    /// ascent's bump, 1 after it, matching "index 0 = first carried
    /// ascent"). This test names that invariant explicitly for the record.
    #[test]
    fn first_carried_ascent_reads_pre_increment_index_zero() {
        let mut g = Game::new(11);
        g.depth = 2;
        g.gen_level();
        g.has_objective = true;
        assert_eq!(g.speech_attempts, 0, "before any carried ascent, the ladder index is 0");
        g.ascend();
        assert_eq!(
            g.speech_attempts, 1,
            "after the FIRST carried ascent, speech_attempts becomes 1 — but StairsUp fired while it was still 0, \
             i.e. the first carried ascent reads ladder index 0, not 1"
        );
    }

    /// Fix-round (§9-C pickup register): `Game::pickup_register_event` is a
    /// pure predicate pulled out of `Game::pickup`'s `ItemEffect::Objective`
    /// arm specifically so its branch selection is unit-testable without
    /// T2's `carried_preamble`/`carried_lines` data (both empty this
    /// batch, so a black-box pickup can't yet observe which `CarryEvent`
    /// variant fired). `kills > spared` reads bloody; a tie — including
    /// 0/0 — reads merciful.
    #[test]
    fn pickup_register_event_selects_bloody_only_when_kills_exceed_spared() {
        assert_eq!(Game::pickup_register_event(3, 1), CarryEvent::PickedUpBloody, "more kills than spares: bloody");
        assert_eq!(Game::pickup_register_event(1, 3), CarryEvent::PickedUpMerciful, "more spares than kills: merciful");
        assert_eq!(Game::pickup_register_event(0, 0), CarryEvent::PickedUpMerciful, "a 0/0 tie reads merciful");
        assert_eq!(Game::pickup_register_event(2, 2), CarryEvent::PickedUpMerciful, "an equal tie reads merciful");
    }

    // ---------- Batch 12 R4: the pickup verdict (mood anchor + running average) ----------

    /// `Game::anchor_score` is the pure ratio behind the mood anchor: `50`
    /// (neutral) if the player neither fought nor talked before pickup,
    /// else `100 * spared / (kills + spared)` — `0` for a pure brute, `100`
    /// for a pure diplomat.
    #[test]
    fn anchor_score_pure_diplomat_brute_and_neutral() {
        assert_eq!(Game::anchor_score(0, 0), 50, "no kills or spares: neutral");
        assert_eq!(Game::anchor_score(0, 5), 100, "pure diplomat: max");
        assert_eq!(Game::anchor_score(5, 0), 0, "pure brute: min");
        assert_eq!(Game::anchor_score(2, 2), 50, "an equal split reads neutral, same as doing nothing");
    }

    /// Before the objective is ever picked up, `mood_count == 0` and
    /// `Game::mood` reports the neutral midpoint — the McGuffin has no
    /// opinion to report yet.
    #[test]
    fn mood_is_neutral_before_any_pickup() {
        let g = Game::new(1);
        assert_eq!(g.mood_count, 0);
        assert_eq!(g.mood(), 50);
    }

    /// The anchor seeds correctly at the objective's FIRST real walk-over
    /// pickup, from whatever kill/spare record the descent left behind: a
    /// pure-diplomat run (kills 0) anchors mood to 100, a pure-brute run
    /// (spared 0) anchors it to 0, and a run that neither fought nor talked
    /// anchors it to the neutral 50 — mirroring `anchor_score_*` above but
    /// through the real `Game::pickup` path, not the pure predicate alone.
    #[test]
    fn mood_anchors_at_first_pickup_from_kill_spare_record() {
        let mut diplomat = blank_room(101);
        diplomat.spared = 5;
        diplomat.items.push(Item { x: diplomat.px + 1, y: diplomat.py, kind: GAME.win.objective_item });
        diplomat.try_move_player(1, 0);
        assert!(diplomat.has_objective, "setup: pickup must land");
        assert_eq!(diplomat.mood(), 100, "a pure-diplomat record (0 kills) anchors mood to 100");

        let mut brute = blank_room(102);
        brute.kills = 5;
        brute.items.push(Item { x: brute.px + 1, y: brute.py, kind: GAME.win.objective_item });
        brute.try_move_player(1, 0);
        assert_eq!(brute.mood(), 0, "a pure-brute record (0 spares) anchors mood to 0");

        let mut neither = blank_room(103);
        neither.items.push(Item { x: neither.px + 1, y: neither.py, kind: GAME.win.objective_item });
        neither.try_move_player(1, 0);
        assert_eq!(neither.mood(), 50, "no kills or spares anchors mood to the neutral midpoint");
    }

    /// A re-pickup after `put_down` (`CarryEvent::PickedBackUp`, not a
    /// FIRST pickup) must NOT re-seed the anchor: changing the kill/spare
    /// record between the put-down and the re-pickup must not move
    /// `Game::mood` at all.
    #[test]
    fn repickup_after_put_down_does_not_reseed_anchor() {
        let mut g = blank_room(104);
        g.kills = 1;
        g.spared = 3; // anchor_score(1, 3) = 100*3/4 = 75
        let (px0, py0) = (g.px, g.py);
        g.items.push(Item { x: px0 + 1, y: py0, kind: GAME.win.objective_item });
        g.try_move_player(1, 0);
        assert!(g.has_objective);
        assert_eq!(g.mood(), 75, "setup: anchor seeded from the 1-kill/3-spare record");
        let mood_after_first_pickup = g.mood();

        g.put_down();
        assert!(g.objective_dropped);

        // Change the record after the anchor was seeded — a re-pickup must
        // not react to this at all.
        g.kills += 10;
        g.spared = 0;

        g.try_move_player(-1, 0); // step off the dropped objective's tile
        assert!(!g.has_objective);
        g.try_move_player(1, 0); // step back onto it: re-pickup (PickedBackUp)
        assert!(g.has_objective);
        assert!(!g.objective_dropped);
        assert_eq!(
            g.mood(),
            mood_after_first_pickup,
            "a re-pickup after put-down must not re-seed the anchor from the new record"
        );
    }

    /// The running average: a post-pickup KILL averages in below the
    /// neutral anchor (dimming mood), and a post-pickup SPARE/becalm
    /// averages in above it (brightening mood) — the diplomat/brute halves
    /// of "the pickup verdict."
    #[test]
    fn post_pickup_kill_lowers_mood_and_spare_raises_it() {
        let mut g = blank_room(105);
        g.items.push(Item { x: g.px + 1, y: g.py, kind: GAME.win.objective_item });
        g.try_move_player(1, 0);
        assert_eq!(g.mood(), 50, "setup: neutral anchor");
        let (px1, py1) = (g.px, g.py);

        // A 1-hp OGRE east of the (now-stationary, attacks don't move the
        // player) carrier: a guaranteed kill.
        let (ox, oy) = (px1 + 1, py1);
        g.monsters.push(Monster { kind: OGRE, x: ox, y: oy, hp: 1, ..Monster::spawn(OGRE, ox, oy) });
        let mood_before_kill = g.mood();
        g.try_move_player(1, 0);
        assert_eq!(g.kills, 1, "setup: the ogre must actually die");
        assert!(g.mood() < mood_before_kill, "a post-pickup kill must lower mood");

        // A becalmed rat (threshold 2) east of the carrier: a spare.
        let (rx, ry) = (px1 + 1, py1);
        g.monsters.push(Monster { kind: RAT, x: rx, y: ry, hp: 1, regard: 0, calm: false, awe: 0 });
        let mood_before_spare = g.mood();
        talk_until_landed(&mut g, 1, 0, RAT);
        talk_until_landed(&mut g, 1, 0, RAT); // crosses threshold 2: becalms
        assert!(g.monsters.iter().any(|m| m.kind == RAT && m.calm), "setup: the rat must actually becalm");
        assert!(g.mood() > mood_before_spare, "a post-pickup spare must raise mood");
    }

    /// Human spec refinement (mid-batch 12 R4): kill valence is graduated
    /// by kind, not flat — killing the near-harmless RAT is more
    /// despicable (lower valence) than killing the guaranteed-hitter OGRE
    /// (closer to self-defense), so the same one-kill script must lower
    /// mood MORE for a rat than for an ogre.
    #[test]
    fn killing_a_rat_lowers_mood_more_than_killing_an_ogre() {
        assert!(
            Monster::stats(RAT).kill_valence < Monster::stats(OGRE).kill_valence,
            "the rat's kill_valence must be lower (more despicable) than the ogre's"
        );
        let mood_after_one_kill = |seed: u64, kind: MKind| -> i32 {
            let mut g = blank_room(seed);
            g.items.push(Item { x: g.px + 1, y: g.py, kind: GAME.win.objective_item });
            g.try_move_player(1, 0);
            assert_eq!(g.mood(), 50, "setup: neutral anchor");
            let (ox, oy) = (g.px + 1, g.py);
            g.monsters.push(Monster { kind, x: ox, y: oy, hp: 1, ..Monster::spawn(kind, ox, oy) });
            g.try_move_player(1, 0);
            assert_eq!(g.kills, 1, "setup: the monster must actually die");
            g.mood()
        };
        let rat_mood = mood_after_one_kill(201, RAT);
        let ogre_mood = mood_after_one_kill(202, OGRE);
        assert!(
            rat_mood < ogre_mood,
            "killing a rat ({rat_mood}) must lower mood more than killing an ogre ({ogre_mood})"
        );
    }

    /// `Game::mood` is a pure function of hashed state: the same script run
    /// twice must land on the exact same mood (and the same state hash).
    #[test]
    fn mood_is_deterministic_given_the_same_script() {
        let run = |seed: u64| -> Game {
            let mut g = blank_room(seed);
            g.kills = 2;
            g.spared = 6;
            g.items.push(Item { x: g.px + 1, y: g.py, kind: GAME.win.objective_item });
            g.try_move_player(1, 0);
            g
        };
        let a = run(55);
        let b = run(55);
        assert_eq!(a.mood(), b.mood(), "the same script must produce the same mood");
        assert_eq!(state_hash(&a), state_hash(&b));
    }

    /// `mood_sum`/`mood_count` are run-defining, hashed state (they will
    /// drive the McGuffin's shine radius, a near-future task) — mirrors
    /// `state_hash_covers_speech_attempts_and_objective_dropped` below.
    #[test]
    fn state_hash_covers_mood_sum_and_mood_count() {
        let mut a = Game::new(9);
        let b = Game::new(9);
        assert_eq!(state_hash(&a), state_hash(&b));

        a.mood_sum = 37;
        assert_ne!(state_hash(&a), state_hash(&b), "mood_sum must be hashed");
        a.mood_sum = 0;
        assert_eq!(state_hash(&a), state_hash(&b));

        a.mood_count = 5;
        assert_ne!(state_hash(&a), state_hash(&b), "mood_count must be hashed");
    }

    // ---------- Batch 12 R5: "light as grace" — her shine + the death rewrite ----------

    /// `mood_shine_radius`'s own tier table: the CRITICAL invariant is that
    /// the dark tier (mood 0-25) reads as radius 0 — "she does not shine,"
    /// full stop — while every band above it shines with a positive radius,
    /// topping out at the torch's own top-tier radius for a max-mood
    /// diplomat.
    #[test]
    fn mood_shine_radius_tiers() {
        assert_eq!(mood_shine_radius(0), 0, "dark tier: no shine at all");
        assert_eq!(mood_shine_radius(25), 0, "dark tier's own upper edge");
        assert_eq!(mood_shine_radius(26), 2);
        assert_eq!(mood_shine_radius(50), 2, "this band's own upper edge");
        assert_eq!(mood_shine_radius(51), 4);
        assert_eq!(mood_shine_radius(75), 4, "this band's own upper edge");
        assert_eq!(mood_shine_radius(76), 6);
        assert_eq!(mood_shine_radius(100), 6, "max mood: brightest tier");
    }

    /// A max-shine McGuffin carries the run through a torch that hits
    /// exactly 0 — "the flip": a diplomat's carrier walks in her light once
    /// the torch itself is spent.
    #[test]
    fn carried_bright_mcguffin_survives_dead_torch() {
        let mut g = blank_room(401);
        g.has_objective = true;
        g.mood_sum = 100;
        g.mood_count = 1; // mood() == 100 -> bright tier, radius 6
        g.light = 1; // any turn's carry_burn (>=1) lands this at/below 0
        g.wait_turn();
        assert_eq!(g.light, 0, "setup: the torch itself must actually hit zero");
        assert!(!g.dead, "her bright shine must carry the run through a dead torch");
    }

    /// A mood-0 (dark tier, radius 0) carrier is nerfed exactly like a
    /// pre-batch-12-R5 attempt: her shine is `None` for this carrier, so a
    /// dead torch still kills — the T1 nerf this batch does not undo.
    #[test]
    fn dark_tier_mcguffin_dies_on_dead_torch() {
        let mut g = blank_room(402);
        g.has_objective = true;
        g.mood_sum = 0;
        g.mood_count = 1; // mood() == 0 -> dark tier, radius 0: no shine
        g.light = 1;
        g.wait_turn();
        assert_eq!(g.light, 0, "setup: the torch itself must actually hit zero");
        assert!(g.dead, "a mood-0 brute must still die in the dark, unchanged from before this batch");
    }

    /// Before the objective is ever claimed (not carried, not put down),
    /// she isn't "yours" and doesn't shine for you at all — a dead torch
    /// kills exactly as it always has.
    #[test]
    fn no_mcguffin_dies_on_dead_torch() {
        let mut g = blank_room(403);
        assert!(!g.has_objective && !g.objective_dropped, "setup: she hasn't been claimed yet");
        g.light = 1;
        g.wait_turn();
        assert_eq!(g.light, 0, "setup: the torch itself must actually hit zero");
        assert!(g.dead, "with no claim on her at all, a dead torch is still fatal");
    }

    /// The put-down shuttle (batch 8 byte 16, now genuine strategy): her
    /// light stays exactly where she was set down. Standing within her
    /// radius with a dead torch survives; straying beyond it with a dead
    /// torch is fatal — "park / scout / return" is intended play, and this
    /// is exactly the self-limiting risk that makes it a choice.
    #[test]
    fn put_down_then_stray_dies_in_dark() {
        // Radius-2 (mood 26-50) shine, parked at the drop tile.
        let mut within = blank_room(404);
        within.has_objective = true;
        within.mood_sum = 50;
        within.mood_count = 1; // mood() == 50 -> radius 2
        let (dropx, dropy) = (within.px, within.py);
        within.put_down();
        assert!(within.objective_dropped, "setup: she must actually be on the ground");
        within.try_move_player(1, 0);
        within.try_move_player(1, 0); // now 2 tiles east: exactly at the radius edge
        assert_eq!((within.px - dropx, within.py - dropy), (2, 0));
        within.light = 1;
        within.wait_turn();
        assert_eq!(within.light, 0, "setup: the torch itself must actually hit zero");
        assert!(!within.dead, "standing at the edge of her parked light with a dead torch must survive");

        let mut beyond = blank_room(405);
        beyond.has_objective = true;
        beyond.mood_sum = 50;
        beyond.mood_count = 1; // mood() == 50 -> radius 2
        let (dropx, dropy) = (beyond.px, beyond.py);
        beyond.put_down();
        assert!(beyond.objective_dropped, "setup: she must actually be on the ground");
        beyond.try_move_player(1, 0);
        beyond.try_move_player(1, 0);
        beyond.try_move_player(1, 0); // now 3 tiles east: past the radius-2 edge
        assert_eq!((beyond.px - dropx, beyond.py - dropy), (3, 0));
        beyond.light = 1;
        beyond.wait_turn();
        assert_eq!(beyond.light, 0, "setup: the torch itself must actually hit zero");
        assert!(beyond.dead, "straying beyond her parked light with a dead torch must be fatal");
    }

    /// The composition half of R5: her shine actually extends `vis`/`seen`
    /// in `compute_fov`, not just the death check — a tile the torch alone
    /// can't reach becomes visible once her (brighter) radius covers it.
    #[test]
    fn her_shine_extends_fov_past_the_torch_alone() {
        let mut g = blank_room(406);
        g.has_objective = true;
        g.mood_sum = 100;
        g.mood_count = 1; // mood() == 100 -> radius 6
        g.light = 40; // deep in the torch's own lowest tier
        let torch_r = fov_radius(g.light);
        let her_r = mood_shine_radius(g.mood());
        assert!(torch_r < 4 && her_r >= 4, "setup: the torch alone must not reach a tile 4 out, her shine must");
        let (fx, fy) = (g.px + 4, g.py); // 4 tiles east, still inside blank_room's open floor
        assert_eq!(g.map[idx(fx, fy)], Tile::Floor, "setup: the probed tile must be open floor");
        g.wait_turn();
        assert_eq!(fov_radius(g.light), torch_r, "setup: still in the torch's lowest tier after the burn");
        assert!(g.vis[idx(fx, fy)], "a tile past the torch's own radius must be lit by her shine");
        assert!(g.seen[idx(fx, fy)]);
    }

    /// `speech_attempts`/`objective_dropped` are run-defining, hashed state.
    #[test]
    fn state_hash_covers_speech_attempts_and_objective_dropped() {
        let mut a = Game::new(9);
        let b = Game::new(9);
        assert_eq!(state_hash(&a), state_hash(&b));

        a.speech_attempts = 3;
        assert_ne!(state_hash(&a), state_hash(&b), "speech_attempts must be hashed");
        a.speech_attempts = 0;
        assert_eq!(state_hash(&a), state_hash(&b));

        a.objective_dropped = true;
        assert_ne!(state_hash(&a), state_hash(&b), "objective_dropped must be hashed");
    }

    /// `mcguffin_last_line_turn` is presentation-only (same exclusion set as
    /// `killer`/`echo`/`facing`/`fx_hit`) and must NOT be hashed.
    #[test]
    fn mcguffin_last_line_turn_is_not_hashed() {
        let mut a = Game::new(9);
        let b = Game::new(9);
        a.mcguffin_last_line_turn = Some(123);
        assert_eq!(state_hash(&a), state_hash(&b), "the presentation-only rate-limit tracker must not be hashed");
    }

    /// The dropped objective is an ordinary `Item` in `self.items`, so its
    /// position is already covered by `state_hash`'s existing per-item hash
    /// — this proves it, rather than assuming it.
    #[test]
    fn state_hash_covers_dropped_objective_position() {
        let mut a = blank_room(11);
        a.has_objective = true;
        a.put_down();

        let mut b = blank_room(11);
        b.has_objective = true;
        b.put_down();
        assert_eq!(state_hash(&a), state_hash(&b), "sanity: identical put-downs must hash identically");

        if let Some(it) = b.items.iter_mut().find(|it| it.kind == GAME.win.objective_item) {
            it.x += 1;
        }
        assert_ne!(state_hash(&a), state_hash(&b), "the dropped objective's position must be hashed");
    }

    /// Play a scripted run live, save, replay the save: identical hashes.
    /// This is the determinism regression harness — a failure here means
    /// channel discipline broke somewhere.
    #[test]
    fn save_replay_roundtrip() {
        let seed0 = 99;
        // Batch 9 T3: `live` must start the same way `replay()` does now
        // (the overworld front door) so applying the identical script
        // directly reproduces exactly what save->parse->replay reconstructs.
        let mut live = Game::new_overworld(seed0);
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
    /// batch 5, to v4 by batch 7 T2, to v5 by batch 8 T1, to v6 by batch 12
    /// R4): a v1-versioned byte blob (which by construction never contains
    /// byte 6 or bytes 7-16 — none of INPUT_RETRY/talk/give/use/put-down
    /// existed yet) parses under today's v6-aware `parse_save` and replays
    /// byte-identically to the same log fed straight to `replay`.
    /// `tests/fixtures/ref.sav` (used by `make xhash`) is itself exactly
    /// such a v1 blob, containing a byte 5 — this test is the unit-level
    /// proof that the case `make xhash` exercises end-to-end still works.
    #[test]
    fn v1_save_replays_under_v6_parsing() {
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
    /// never contains bytes 7-16 (talk/give/use/put-down didn't exist until
    /// save v3/v4/v5), so it too must replay byte-identically under today's
    /// parser.
    #[test]
    fn v2_save_replays_under_v6_parsing() {
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

    /// Save v3 back-compat (batch 7 T2): a v3-versioned blob may carry talk
    /// bytes (7-10) but never give/use/put-down (11-16, save v4/v5) — it too
    /// must replay byte-identically under today's parser.
    #[test]
    fn v3_save_replays_under_v6_parsing() {
        let seed0 = 654u64;
        let log = vec![0u8, 7, 1, 8, 2, 9, 3, 10, 4];
        let mut v3_bytes = Vec::new();
        v3_bytes.extend_from_slice(b"RL14");
        v3_bytes.push(3); // v3
        v3_bytes.extend_from_slice(&seed0.to_le_bytes());
        v3_bytes.extend_from_slice(&log);

        let (s, parsed_log) = parse_save(&v3_bytes).expect("v3 blob must still parse");
        assert_eq!(s, seed0);
        assert_eq!(parsed_log, log);

        let from_v3 = replay(s, &parsed_log);
        let direct = replay(seed0, &log);
        assert_eq!(state_hash(&from_v3), state_hash(&direct));
    }

    /// Save v4 back-compat (batch 8 T1): a v4-versioned blob may carry
    /// give/use bytes (11-15) but never put-down (16, save v5) — it too
    /// must replay byte-identically under today's parser.
    #[test]
    fn v4_save_replays_under_v6_parsing() {
        let seed0 = 987u64;
        let log = vec![0u8, 1, 11, 2, 12, 3, 15, 4];
        let mut v4_bytes = Vec::new();
        v4_bytes.extend_from_slice(b"RL14");
        v4_bytes.push(4); // v4
        v4_bytes.extend_from_slice(&seed0.to_le_bytes());
        v4_bytes.extend_from_slice(&log);

        let (s, parsed_log) = parse_save(&v4_bytes).expect("v4 blob must still parse");
        assert_eq!(s, seed0);
        assert_eq!(parsed_log, log);

        let from_v4 = replay(s, &parsed_log);
        let direct = replay(seed0, &log);
        assert_eq!(state_hash(&from_v4), state_hash(&direct));
    }

    /// Save v5 back-compat (batch 8 T1's own version, now one behind
    /// current): a v5-versioned blob may carry put-down (16) but nothing
    /// past it (save v6, batch 12 R4, added no new input byte at all — see
    /// this module's header comment — so there's nothing a v5 log could
    /// contain that v6 parsing wouldn't already handle identically).
    #[test]
    fn v5_save_replays_under_v6_parsing() {
        let seed0 = 741u64;
        let log = vec![0u8, 1, 16, 2, 16, 3, 4];
        let mut v5_bytes = Vec::new();
        v5_bytes.extend_from_slice(b"RL14");
        v5_bytes.push(5); // v5
        v5_bytes.extend_from_slice(&seed0.to_le_bytes());
        v5_bytes.extend_from_slice(&log);

        let (s, parsed_log) = parse_save(&v5_bytes).expect("v5 blob must still parse");
        assert_eq!(s, seed0);
        assert_eq!(parsed_log, log);

        let from_v5 = replay(s, &parsed_log);
        let direct = replay(seed0, &log);
        assert_eq!(state_hash(&from_v5), state_hash(&direct));
    }

    /// Put-down byte (16, batch 8 T1) round-trips through save -> parse ->
    /// replay identically, same proof shape as the version back-compat
    /// tests above: a log containing byte 16 survives `save_bytes` (which
    /// now writes v6) -> `parse_save` -> `replay` producing the exact same
    /// state as replaying the original log directly.
    #[test]
    fn put_down_byte_round_trips_through_save_parse_replay() {
        let seed0 = 246u64;
        let log = vec![0u8, 1, 16, 2, 3, 16, 4];
        let bytes = save_bytes(seed0, &log);
        assert_eq!(bytes[4], 6, "save_bytes must write the current version (6)");

        let (s, parsed_log) = parse_save(&bytes).expect("v6 blob must parse");
        assert_eq!(s, seed0);
        assert_eq!(parsed_log, log);

        let from_saved = replay(s, &parsed_log);
        let direct = replay(seed0, &log);
        assert_eq!(state_hash(&from_saved), state_hash(&direct));
    }

    /// `save_bytes` writes the current version (6, batch 12 R4) and a
    /// version outside 1..=6 is rejected by `parse_save` — the "old binary
    /// must reject a newer save cleanly" half of every save-version bump's
    /// rationale (this bump's other half is simply keeping the version
    /// label in lockstep with the hashed-state addition — see this
    /// module's header comment on `SAVE_VERSION`).
    #[test]
    fn save_bytes_writes_current_version_and_unknown_versions_are_rejected() {
        let bytes = save_bytes(7, &[0, 1, 2]);
        assert_eq!(bytes[4], 6, "save_bytes must write the current version");
        assert!(parse_save(&bytes).is_some());

        let mut future = bytes.clone();
        future[4] = 7;
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

    /// batch 7 T3 fix: `parse_ghost` must reject a `label_idx` that can't
    /// index `GAME.ghost_labels` (untrusted, from a ghost file on disk) —
    /// in bounds at `len()-1` parses fine, out of bounds at `len()` fails.
    #[test]
    fn parse_ghost_rejects_out_of_bounds_label_idx() {
        let seed = 77u64;
        let whash = 0xABCD_EF01_2345_6789u64;
        let outcome = 0u8; // died_combat
        let final_depth = 3u8;
        let turns = 512u32;
        let inputs = vec![0u8, 1, 2, 3];
        let n = GAME.ghost_labels.len();

        let in_bounds = ghost_bytes(seed, whash, outcome, final_depth, turns, (n - 1) as u8, &inputs);
        assert!(
            parse_ghost(&in_bounds).is_some(),
            "label_idx == len()-1 is the last valid index and must parse"
        );

        let out_of_bounds = ghost_bytes(seed, whash, outcome, final_depth, turns, n as u8, &inputs);
        assert!(
            parse_ghost(&out_of_bounds).is_none(),
            "label_idx == len() is one past the last valid index and must fail to parse"
        );
    }

    /// Echo (batch 4 task 2, save v2 substrate): after replaying a log
    /// whose retry byte (6) followed a death, `echo` equals the death
    /// position/depth, and — proving `echo` is presentation-only and NOT
    /// hashed — a fresh `Game::new_overworld(seed)` with `echo` set by hand
    /// to the same value hashes identically to the retried game.
    ///
    /// Batch 9 T3 update: the front door is now the overworld, and the
    /// overworld's own torch-clock exemption (`Game::spend_turn`) means
    /// waiting there can never kill you — so this test must actually walk
    /// into the dungeon (the same short, seed-independent screen-1
    /// crossing `tests/fixtures/ref.sav` uses: S then E,E,E onto the hole,
    /// see that fixture's regeneration note) before the wait-to-death loop
    /// can do its job. `live` and `fresh` both start via
    /// `Game::new_overworld` to match `replay()`'s own front door.
    #[test]
    fn echo_records_death_position_after_retry_byte() {
        let seed0 = 33u64;
        let mut live = Game::new_overworld(seed0);
        let mut log: Vec<u8> = Vec::new();
        // Screen 1's fixed, seed-independent path from spawn onto the hole
        // (`V`): one step south, then three east — see
        // `instantiate_overworld_screen`'s doc comment on the default start
        // tile and `OVERWORLD_1`'s map in contractor.rs.
        for &b in &[1u8, 3, 3, 3] {
            log.push(b);
            live.apply_input(b);
        }
        assert_eq!(live.world, WorldId::Seed(seed0), "fixture: must have crossed into the root dungeon");
        // Wait repeatedly until the run ends (dark or combat — either way
        // it's a dead ending since the player never moves off the
        // dungeon entrance); START_LIGHT (2000) bounds this loop.
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

        let mut fresh = Game::new_overworld(seed0);
        fresh.echo = Some((dx, dy, dd));
        assert_eq!(
            state_hash(&replayed),
            state_hash(&fresh),
            "echo must be unhashed: only its own value should differ"
        );
    }

    /// GAME.ghost_labels (content.rs): every preset phrase is ASCII and <=16
    /// bytes, per the RLG1 format's label_idx contract in save.rs.
    #[test]
    fn ghost_labels_fit_16_bytes() {
        for label in GAME.ghost_labels {
            assert!(label.is_ascii(), "non-ASCII ghost label: {}", label);
            assert!(label.len() <= 16, "ghost label too long ({}): {}", label.len(), label);
        }
    }

    /// Visited depths persist: descend and climb back into the same level.
    #[test]
    fn levels_persist_across_stairs() {
        let mut g = Game::new(11);
        // batch 9 T1: `Tile::ScreenLink(bool)` carries a field, so `Tile` can
        // no longer be `as u8`-cast (only fieldless enums support that) —
        // compare the `Tile` vectors directly instead (still `Clone`/
        // `PartialEq`/`Debug`, so `assert_eq!` works unchanged).
        let map1: Vec<Tile> = g.map.clone();
        let items1 = g.items.len();
        g.descend();
        assert_eq!(g.depth, 2);
        g.ascend();
        assert_eq!(g.depth, 1);
        assert_eq!(map1, g.map, "depth 1 layout changed across a round trip");
        assert_eq!(items1, g.items.len(), "items respawned across a round trip");
        // and the player came back out standing on the down-stairs
        assert!(g.map[idx(g.px, g.py)] == Tile::Stairs);
    }

    /// The greedy bot has no RNG of its own: replaying a seed twice through
    /// sim_seed must produce byte-identical outcomes.
    #[test]
    fn sim_deterministic() {
        for seed in [7u64, 42u64] {
            let a = sim_seed(seed, Policy::Greedy);
            let b = sim_seed(seed, Policy::Greedy);
            assert_eq!(a, b, "seed {} was not deterministic", seed);
        }
    }

    /// Same determinism guarantee, pacifist policy (batch 5 T2): the talk
    /// detour is still a pure function of Game state, no RNG of its own.
    #[test]
    fn pacifist_policy_deterministic() {
        for seed in [7u64, 42u64] {
            let a = sim_seed(seed, Policy::Pacifist);
            let b = sim_seed(seed, Policy::Pacifist);
            assert_eq!(a, b, "seed {} was not deterministic under Policy::Pacifist", seed);
        }
    }

    /// The pacifist bot's one behavioral rule: never bump-attack. Over a
    /// seed range it must land zero kills (spared may be > 0 — that's the
    /// whole point of the policy).
    #[test]
    fn pacifist_never_attacks() {
        let mut total_spared = 0u64;
        for seed in 0..200u64 {
            let (r, _world) = sim_seed(seed, Policy::Pacifist);
            assert_eq!(r.kills, 0, "pacifist bot landed a kill on seed {}", seed);
            total_spared += r.spared as u64;
        }
        assert!(total_spared > 0, "expected at least one becalmed monster over seeds 0..200");
    }

    /// batch 10: the tactical-violent bot is deterministic (two runs
    /// identical). It never EMITS a talk byte — `sim_seed`'s `talks` gate is
    /// only ever set for `Policy::Pacifist`/`TacticalPacifist`, never plain
    /// `Tactical` — so it never spares via the talk/give path.
    ///
    /// batch 11 T2 note: `spared` is no longer proof of that by itself.
    /// "Standing tall" (`Game::resolve_awe`) can now becalm an awe-able
    /// monster (the ogre) purely from ending several turns cardinally
    /// adjacent without attacking it — which can happen incidentally to a
    /// violent bot too, e.g. while its BFS route runs alongside a stationary
    /// ogre it hasn't targeted yet. That's a real, intended side effect of
    /// the new mechanic (any player/bot who merely doesn't swing at an ogre
    /// it's beside earns the same mercy a talker would), not a bug in this
    /// policy — so this test no longer asserts `spared == 0`.
    ///
    /// batch 11 T2 fix round: `resolve_awe` was rewritten so this can no
    /// longer happen via a RETREAT (a monster the bot is fleeing from, that
    /// then chases back into adjacency, no longer builds awe — see that
    /// method's doc comment). Re-measured after the fix,
    /// `./target/release/rl144 --sim 300 --policy tactical --report` still
    /// reports `spared_total: 60` — nonzero, confirming this is a genuine
    /// sustained hold (the bot's own route keeping it beside a stationary
    /// ogre it isn't currently routing to fight, without ever increasing
    /// distance to it), not the retreat artifact the fix removed. So the
    /// dropped `spared == 0` assertion stays dropped, on firmer footing
    /// than before: it's now confirmed to reflect an intended emergent
    /// mercy rather than a bug this fix should have closed off.
    #[test]
    fn tactical_bot_deterministic_and_never_talks() {
        use headless::{sim_seed, Policy};
        for seed in [1u64, 7, 42, 100, 1337] {
            let (a, _) = sim_seed(seed, Policy::Tactical);
            let (b, _) = sim_seed(seed, Policy::Tactical);
            assert_eq!(
                (a.won, a.dead_dark, a.dead_combat, a.stuck, a.turns, a.light_left, a.kills, a.spared),
                (b.won, b.dead_dark, b.dead_combat, b.stuck, b.turns, b.light_left, b.kills, b.spared),
                "tactical sim must be deterministic for seed {seed}"
            );
            assert!(!a.stuck, "tactical bot must not get stuck, seed {seed}");
        }
    }

    /// batch 10, the whole point: a competent violent bot wins MORE than greedy on
    /// the same seeds — the current game is easy for a player who routes around
    /// fights. (A weak inequality over a small sample; the full measured delta is
    /// captured in the band file, not asserted here.)
    #[test]
    fn tactical_bot_wins_at_least_as_often_as_greedy() {
        use headless::{sim_seed, Policy};
        let mut greedy = 0u32;
        let mut tactical = 0u32;
        for seed in 0u64..300 {
            if sim_seed(seed, Policy::Greedy).0.won { greedy += 1; }
            if sim_seed(seed, Policy::Tactical).0.won { tactical += 1; }
        }
        assert!(tactical >= greedy, "tactical {tactical} should win >= greedy {greedy} over 300 seeds");
    }

    /// batch 10: the tactical-diplomat never attacks (kills stay 0) and is
    /// deterministic.
    #[test]
    fn tactical_pacifist_never_attacks_and_deterministic() {
        use headless::{sim_seed, Policy};
        for seed in [1u64, 7, 42, 100, 1337] {
            let (a, _) = sim_seed(seed, Policy::TacticalPacifist);
            let (b, _) = sim_seed(seed, Policy::TacticalPacifist);
            assert_eq!(a.kills, 0, "tactical-diplomat must never kill, seed {seed}");
            assert_eq!(
                (a.won, a.turns, a.light_left, a.spared),
                (b.won, b.turns, b.light_left, b.spared),
                "tactical-diplomat must be deterministic, seed {seed}"
            );
            assert!(!a.stuck, "must not get stuck, seed {seed}");
        }
    }

    /// batch 12 stuck fix regression: before `headless::STALL_LIMIT` (plus
    /// the `MAX_REST_TURNS_PER_RUN` hygiene cap alongside it), `--sim 5000
    /// --policy tactical-pacifist` produced exactly 5 STUCK runs (seeds
    /// 817, 1074, 3133, 3191, 4552 — every one timing out at
    /// `SIM_TURN_CAP` with `light_left == 0`). Traced root cause (seed
    /// 817): rest propped the run through a near-death combat moment it
    /// would otherwise have lost, the objective-carrying diplomat's maxed
    /// mood then made her carried light immune to darkness death, and the
    /// run walked into a separate, pre-existing `tactical_routing_map`
    /// hazard — a patrolling monster's tile alternately opening/closing a
    /// shortcut, entraining the fully-myopic per-turn replan into undoing
    /// its own step forever, with no death left to end it. Pin all five by
    /// name so a future rest/routing change that reintroduces the stall
    /// fails loudly here instead of only showing up as a nonzero `stuck`
    /// count in a full 5000-seed `--sim` run.
    #[test]
    fn tactical_pacifist_previously_stuck_seeds_now_terminate() {
        use headless::{sim_seed, Policy};
        for seed in [817u64, 1074, 3133, 3191, 4552] {
            let (r, _) = sim_seed(seed, Policy::TacticalPacifist);
            assert!(!r.stuck, "seed {seed} must not get stuck (tactical-pacifist)");
        }
    }

    /// A diplomat who routes around fights and only talks when forced wins at least
    /// as often as the blunt pacifist that talks its way through everything.
    #[test]
    fn tactical_pacifist_wins_at_least_as_often_as_pacifist() {
        use headless::{sim_seed, Policy};
        let mut base = 0u32;
        let mut tac = 0u32;
        for seed in 0u64..300 {
            if sim_seed(seed, Policy::Pacifist).0.won { base += 1; }
            if sim_seed(seed, Policy::TacticalPacifist).0.won { tac += 1; }
        }
        assert!(tac >= base, "tactical-diplomat {tac} should win >= pacifist {base} over 300 seeds");
    }

    /// Batch 6 T1 (extended batch 12 R3): the only input that can transit a
    /// portal is wait (byte 4, `game::Game::wait_turn`'s doc comment) — so a
    /// bot run must never leave the root world, regardless of how many
    /// portals its route happens to walk over (walking ONTO one only logs,
    /// per `Game::land_on_tile`'s `Tile::Portal` arm). Greedy/Pacifist never
    /// emit wait at all (unchanged since batch 6). Batch 12 R3 gave BOTH
    /// tactical policies a rest branch that DOES emit wait — its own
    /// explicit `Tile::Portal` guard (`headless::sim_seed`) is what this
    /// test actually proves holds, for all four policies now, not just the
    /// two that structurally can't transit by construction. Checked over a
    /// range wide enough to almost certainly cross at least one
    /// portal-bearing depth (~1/4 chance each, per `Game::gen_level`'s
    /// portal-placement comment).
    #[test]
    fn bot_never_transits() {
        for seed in 0..100u64 {
            for policy in
                [Policy::Greedy, Policy::Pacifist, Policy::Tactical, Policy::TacticalPacifist]
            {
                let (_, world) = sim_seed(seed, policy);
                assert!(
                    world == WorldId::Seed(seed),
                    "{} bot left the root world on seed {}",
                    policy.name(),
                    seed
                );
            }
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
    /// (`make sim`). Batch 11's heal-scarcity combat MAJOR collapsed the greedy
    /// bot to <1% wins (it's the too-dumb reference now), so this "the engine
    /// can still produce a win" floor switched from greedy to the TACTICAL
    /// (competent-play) bot, which wins ~47% — the honest proxy for "winnable
    /// by a skilled player." Seeds 0..50 produce several deterministic tactical
    /// wins; asserts `wins >= 1` as a floor against a return of the batch-2
    /// unwinnable wall.
    #[test]
    fn sim_bot_wins_some() {
        let mut wins = 0u64;
        let mut deaths_combat = 0u64;
        let mut deaths_dark = 0u64;
        let mut stuck = 0u64;
        for seed in 0..50u64 {
            let (r, _world) = sim_seed(seed, Policy::Tactical);
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

    // ---------- Batch 6 T1: portals + multi-world state + authored floors ----------

    /// Greedy BFS walk from the player's CURRENT position to `(tx, ty)`,
    /// appending each move byte to `inputs` and applying it — test support
    /// for scripting a REAL (not teleported) approach to a tile, needed by
    /// `multiworld_replay_is_deterministic` since replay only ever sees an
    /// actual input-byte log. Attacks through a monster blocking the
    /// shortest path rather than detouring (same as a human bumping
    /// through) — that still converges, just costs extra bytes/turns.
    fn walk_to(inputs: &mut Vec<u8>, g: &mut Game, tx: i32, ty: i32) {
        let dirs: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
        for _ in 0..2000 {
            if (g.px, g.py) == (tx, ty) || g.dead || g.won {
                return;
            }
            let dist = bfs_dist(&g.map, (tx, ty));
            let player_d = dist[idx(g.px, g.py)];
            assert!(
                player_d >= 0,
                "walk_to: ({},{}) unreachable from ({},{})",
                tx,
                ty,
                g.px,
                g.py
            );
            let b = dirs
                .iter()
                .enumerate()
                .find_map(|(b, &(dx, dy))| {
                    let (nx, ny) = (g.px + dx, g.py + dy);
                    if in_map(nx, ny) && dist[idx(nx, ny)] == player_d - 1 {
                        Some(b as u8)
                    } else {
                        None
                    }
                })
                .expect("walk_to: no step decreases distance to target");
            inputs.push(b);
            g.apply_input(b);
        }
        panic!("walk_to: exceeded step cap approaching ({}, {})", tx, ty);
    }

    /// Move the player onto `(tx, ty)` via a single step from a walkable
    /// neighbor — test support for triggering `try_move_player`'s landing
    /// logic (pickup, stairs/portal handling) on a specific tile without
    /// scripting a full path there. Teleports the player to the neighbor
    /// first (fine for unit-testing a specific mechanic in isolation — same
    /// direct-field-poke convention `lose_beats_win_at_zero_light` and
    /// friends already use elsewhere in this file).
    fn step_onto(g: &mut Game, tx: i32, ty: i32) {
        let deltas: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        let (adj, delta) = deltas
            .iter()
            .map(|&(dx, dy)| ((tx - dx, ty - dy), (dx, dy)))
            .find(|&((ax, ay), _)| {
                in_map(ax, ay)
                    && g.map[idx(ax, ay)] != Tile::Wall
                    && !g.monsters.iter().any(|m| (m.x, m.y) == (ax, ay))
            })
            .expect("step_onto: target has no walkable, unoccupied neighbor");
        g.px = adj.0;
        g.py = adj.1;
        g.try_move_player(delta.0, delta.1);
    }

    /// Fixture assumption shared by several tests below: seed 1's depth 2
    /// rolls a portal (found by scanning `--dump --seed 1`'s depth-2 block
    /// for `*`). Isolated here as its own smoke test so a future worldgen
    /// re-baseline (T4) gets a clear, specific failure instead of a
    /// confusing one three tests downstream.
    #[test]
    fn fixture_seed1_depth2_has_a_portal() {
        let mut g = Game::new(1);
        g.depth = 2;
        g.gen_level();
        assert!(g.portal.is_some(), "fixture assumption broke: seed 1 depth 2 no longer rolls a portal");
    }

    /// A portal's destination is a pure function of (world seed, depth):
    /// regenerating the identical (seed, depth) twice must land the SAME
    /// portal position and the SAME destination (both the kind — World vs
    /// Floor — and the specific seed/index).
    #[test]
    fn portal_destination_is_deterministic() {
        let mut a = Game::new(1);
        a.depth = 2;
        a.gen_level();
        let mut b = Game::new(1);
        b.depth = 2;
        b.gen_level();
        let (pa, pb) = (a.portal, b.portal);
        match (pa, pb) {
            (Some((ax, ay, adest)), Some((bx, by, bdest))) => {
                assert_eq!((ax, ay), (bx, by), "portal position must be deterministic");
                match (adest, bdest) {
                    (Dest::World(sa, wa), Dest::World(sb, wb)) => {
                        assert_eq!(sa, sb, "derived destination seed must be deterministic");
                        assert_eq!(wa, wb, "memoized world hash must be deterministic too");
                    }
                    (Dest::Floor(ia), Dest::Floor(ib)) => {
                        assert_eq!(ia, ib, "destination floor index must be deterministic")
                    }
                    _ => panic!("destination KIND differed between two identical generations"),
                }
            }
            _ => panic!("portal presence differed between two identical generations"),
        }
    }

    /// Walking ONTO a portal only logs its destination and does not
    /// transit; standing on it and pressing wait (byte 4) does.
    #[test]
    fn portal_transit_only_via_wait() {
        let mut g = Game::new(1);
        g.depth = 2;
        g.gen_level();
        let (px, py, _dest) = g.portal.expect("fixture: seed 1 depth 2 has a portal");

        step_onto(&mut g, px, py);
        assert!((g.px, g.py) == (px, py), "should have landed on the portal tile");
        assert!(g.world == WorldId::Seed(1), "walking onto a portal must not transit");
        // Not necessarily the LAST message: a monster adjacent to the
        // portal may attack in the same turn's monsters_act, logging
        // AFTER the describe line — search the whole log instead.
        assert!(
            g.msgs.iter().any(|m| m.starts_with("Beyond it:")),
            "walk-over must log the describe line, got: {:?}",
            g.msgs
        );

        g.wait_turn();
        assert!(g.world != WorldId::Seed(1), "waiting while standing on a portal must transit");
    }

    /// Leaving through a portal and returning restores the SOURCE level
    /// exactly — map, monsters, and items unchanged (state_hash equality
    /// against a never-left control is NOT expected: light differs, since
    /// time still passed in the destination — see the batch-6 T1 test
    /// list's own note on this).
    #[test]
    fn portal_round_trip_restores_source_exactly() {
        let mut g = Game::new(1);
        g.depth = 2;
        g.gen_level();
        let (px, py, _dest) = g.portal.expect("fixture: seed 1 depth 2 has a portal");

        let map_before = g.map.clone();
        let mons_before: Vec<(i32, i32, MKind, i32, u8, bool)> =
            g.monsters.iter().map(|m| (m.x, m.y, m.kind, m.hp, m.regard, m.calm)).collect();
        let items_before: Vec<(i32, i32, u8)> =
            g.items.iter().map(|it| (it.x, it.y, it.kind as u8)).collect();

        g.px = px;
        g.py = py;
        g.wait_turn(); // transit
        assert!(g.world != WorldId::Seed(1), "fixture assumption: transit happened");

        // Step off the destination's entrance and back onto it to trigger
        // return_to_source (arriving already stands ON the entrance, which
        // doesn't by itself fire land_on_tile).
        let (ex, ey) = (g.px, g.py);
        let off = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .find(|&(dx, dy)| {
                in_map(ex + dx, ey + dy)
                    && g.map[idx(ex + dx, ey + dy)] != Tile::Wall
                    && !g.monsters.iter().any(|m| (m.x, m.y) == (ex + dx, ey + dy))
            })
            .expect("destination entrance has a free neighbor");
        g.try_move_player(off.0, off.1);
        g.try_move_player(-off.0, -off.1);
        assert!(g.world == WorldId::Seed(1), "should be back in the source world");
        assert!((g.px, g.py) == (px, py), "must land exactly on the source portal tile");

        assert!(g.map == map_before, "source map must be byte-identical after the round trip");
        let mons_after: Vec<(i32, i32, MKind, i32, u8, bool)> =
            g.monsters.iter().map(|m| (m.x, m.y, m.kind, m.hp, m.regard, m.calm)).collect();
        assert!(mons_after == mons_before, "source monsters must be unchanged");
        let items_after: Vec<(i32, i32, u8)> =
            g.items.iter().map(|it| (it.x, it.y, it.kind as u8)).collect();
        assert_eq!(items_after, items_before, "source items must be unchanged");
    }

    /// Globals (light burn aside, which is expected — see `spend_turn`)
    /// and the per-run RNG streams (combat/ai/flavor/parley) are NOT reset
    /// or perturbed by a transit: they're per-RUN state, not per-world.
    #[test]
    fn globals_persist_across_transit() {
        let mut g = Game::new(1);
        g.depth = 2;
        g.gen_level();
        let (px, py, _dest) = g.portal.expect("fixture: seed 1 depth 2 has a portal");

        g.atk += 2; // simulate a sword pickup's lasting effect
        let atk_before = g.atk;
        let kills_before = g.kills;
        let spared_before = g.spared;

        // Prove combat_rng's stream isn't perturbed: draw one value now,
        // transit, draw the next — it must match a channel that was simply
        // left running, not reset.
        let mut shadow = channel(1, &["combat"]);
        let first = g.combat_rng.next();
        assert_eq!(first, shadow.next(), "sanity: combat_rng starts matching a fresh channel");

        g.px = px;
        g.py = py;
        g.wait_turn(); // transit
        assert!(g.world != WorldId::Seed(1), "fixture assumption: transit happened");

        assert_eq!(g.atk, atk_before, "atk (a run-global) must survive a transit");
        assert_eq!(g.kills, kills_before, "kills (a run-global) must survive a transit");
        assert_eq!(g.spared, spared_before, "spared (a run-global) must survive a transit");
        let second = g.combat_rng.next();
        assert_eq!(second, shadow.next(), "combat_rng must be exactly where it left off");
    }

    /// Script an input log that crosses TWO worlds (root -> a portal ->
    /// waits in the destination), then replay it twice from scratch and
    /// require an identical state hash.
    #[test]
    fn multiworld_replay_is_deterministic() {
        let seed = 3;
        let mut g = Game::new(seed);
        let (tx, ty, _dest) = g.portal.expect("fixture: seed 3 depth 1 has a portal");
        let mut inputs: Vec<u8> = Vec::new();
        walk_to(&mut inputs, &mut g, tx, ty);
        assert!((g.px, g.py) == (tx, ty), "fixture assumption: walk_to reached the portal");

        inputs.push(4);
        g.apply_input(4); // transit
        assert!(g.world != WorldId::Seed(seed), "fixture assumption: transit happened");
        inputs.push(4);
        g.apply_input(4);
        inputs.push(4);
        g.apply_input(4);

        let r1 = replay(seed, &inputs);
        let r2 = replay(seed, &inputs);
        assert_eq!(state_hash(&r1), state_hash(&r2), "multi-world replay must be deterministic");
        assert!(r1.world != WorldId::Seed(seed), "replay should also have crossed worlds");
    }

    /// The root world's depth-1 `<` win check is untouched by the
    /// non-root-world return-portal branch added alongside it.
    #[test]
    fn root_win_unaffected_by_portals() {
        let mut g = Game::new(7);
        g.has_objective = true;
        g.monsters.clear(); // keep the test about the win check, not combat
        let (ex, ey) = (g.px, g.py);
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .find(|&(dx, dy)| {
                in_map(ex + dx, ey + dy) && g.map[idx(ex + dx, ey + dy)] == Tile::Floor
            })
            .unwrap();
        g.try_move_player(dx, dy);
        assert!(!g.dead && !g.won);
        g.try_move_player(-dx, -dy); // back onto '<' with the amulet in hand
        assert!(g.won, "root world's depth-1 <, with the amulet, must still win");
    }

    /// Mirrors `vaults_well_formed`: bordered by `#`, exactly one return
    /// portal `<`, only legal legend chars, within the 80x25 grid.
    #[test]
    fn authored_floors_well_formed() {
        for (fi, f) in GAME.authored_floors.iter().enumerate() {
            let rows: Vec<&str> = f.map.lines().collect();
            let w = rows[0].len();
            assert!(rows.len() >= 3 && w >= 3, "floor {} too small", fi);
            assert!(rows.len() <= MAP_H && w <= COLS, "floor {} exceeds 80x25", fi);
            let mut lt_count = 0;
            for (j, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), w, "floor {} row {} ragged", fi, j);
                for (i, c) in row.bytes().enumerate() {
                    assert!(b"#.<!)rgO".contains(&c), "floor {} bad char {}", fi, c as char);
                    if c == b'<' {
                        lt_count += 1;
                    }
                    if j == 0 || j == rows.len() - 1 || i == 0 || i == w - 1 {
                        assert_eq!(c, b'#', "floor {} border open at {},{}", fi, i, j);
                    }
                }
            }
            assert_eq!(lt_count, 1, "floor {} must have exactly one return portal", fi);
        }
    }

    /// Authored-floor flavor (name + describe line, and the "You arrive
    /// at..." line it feeds) must fit the 78-char log row — same
    /// discipline as `theme_lines_fit_log_row`/`talk_lines_fit_log_row`.
    #[test]
    fn authored_floors_flavor_fits_log_row() {
        for f in GAME.authored_floors {
            assert!(f.describe.len() <= 78, "describe too long ({}): {}", f.describe.len(), f.describe);
            let arrival = format!("You arrive at {}.", f.name);
            assert!(arrival.len() <= 78, "arrival line too long ({}): {}", arrival.len(), arrival);
        }
    }

    /// Same-floor-from-different-worlds is the SAME floor: `WorldId::Floor`
    /// alone keys the persisted state, not which portal/tile led there.
    /// Constructed directly (per the batch-6 T1 test list's own allowance)
    /// since natural worldgen doesn't guarantee two distinct real portals
    /// converge on the same floor within a small seed sample.
    #[test]
    fn floor_is_singular_across_visits() {
        let mut g = Game::new(1);
        let enter_floor0 = |g: &mut Game| {
            let (px, py) = (g.px, g.py);
            g.map[idx(px, py)] = Tile::Portal;
            g.portal = Some((px, py, Dest::Floor(0)));
            g.wait_turn();
        };

        enter_floor0(&mut g);
        assert!(g.world == WorldId::Floor(0), "fixture: should have transited into Floor(0)");
        let items_on_arrival = g.items.len();
        assert!(items_on_arrival > 0, "fixture: floor 0 has pickupable items");

        let (ix, iy) = (g.items[0].x, g.items[0].y);
        step_onto(&mut g, ix, iy);
        assert_eq!(g.items.len(), items_on_arrival - 1, "item should have been picked up");

        let ret = (0..COLS as i32 * MAP_H as i32)
            .map(|i| (i % COLS as i32, i / COLS as i32))
            .find(|&(x, y)| g.map[idx(x, y)] == Tile::UpStairs)
            .expect("floor has a return <");
        step_onto(&mut g, ret.0, ret.1);
        assert!(g.world == WorldId::Seed(1), "should be back in root after leaving Floor(0)");

        // Re-enter Floor(0) from a DIFFERENT tile in the source world —
        // what's under test is that WorldId::Floor(0) alone determines
        // which stashed state comes back, regardless of provenance.
        g.px = 5;
        g.py = 5;
        enter_floor0(&mut g);
        assert!(g.world == WorldId::Floor(0));
        assert_eq!(
            g.items.len(),
            items_on_arrival - 1,
            "revisiting the SAME floor must restore its persisted state"
        );
    }

    // ---------- batch 9 T1: overworld skeleton (story §9-J prep) ----------

    /// Mirrors `authored_floors_well_formed`: bordered by `#` (except where
    /// a legal `=` link legitimately replaces the border), only legal
    /// legend chars, within the 80x25 grid, and the exact link-count/edge
    /// shape SIGN-OFF ASK #1 specifies (screen 0 east-only, screen 1 both
    /// edges, screen 2 west-only — a straight chain, never a loop).
    #[test]
    fn overworld_screens_well_formed() {
        for (si, screen) in GAME.overworld.screens.iter().enumerate() {
            let rows: Vec<&str> = screen.map.lines().collect();
            let w = rows[0].len();
            assert!(rows.len() >= 3 && w >= 3, "overworld screen {} too small", si);
            assert!(rows.len() <= MAP_H && w <= COLS, "overworld screen {} exceeds 80x25", si);
            let mut west_links = 0;
            let mut east_links = 0;
            for (j, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), w, "overworld screen {} row {} ragged", si, j);
                for (i, c) in row.bytes().enumerate() {
                    let known = b"#.V+".contains(&c)
                        || c == b'='
                        || GAME.items.iter().any(|it| it.glyph == c)
                        || GAME.monsters.iter().any(|m| m.glyph == c);
                    assert!(known, "overworld screen {} bad char {}", si, c as char);
                    let on_border = j == 0 || j == rows.len() - 1 || i == 0 || i == w - 1;
                    if c == b'=' {
                        assert!(
                            i == 0 || i == w - 1,
                            "overworld screen {} link not on an edge column at {},{}",
                            si,
                            i,
                            j
                        );
                        if i == 0 {
                            west_links += 1;
                        } else {
                            east_links += 1;
                        }
                    } else if on_border {
                        assert_eq!(c, b'#', "overworld screen {} border open at {},{}", si, i, j);
                    }
                }
            }
            let (want_west, want_east) = match si {
                0 => (0, 1),
                1 => (1, 1),
                2 => (1, 0),
                _ => panic!("unexpected screen count"),
            };
            assert_eq!(west_links, want_west, "overworld screen {} west-link count", si);
            assert_eq!(east_links, want_east, "overworld screen {} east-link count", si);
        }
    }

    /// `Game::new` must stay byte-for-byte the frozen root-dungeon
    /// constructor (SIGN-OFF ASK #3) even after being refactored to share
    /// `Game::base` with `Game::new_overworld` — checked directly here
    /// (goldens/solve/sim/xhash prove it too, but this is the one test that
    /// names the invariant explicitly).
    #[test]
    fn game_new_unchanged_by_the_base_refactor() {
        let g = Game::new(99);
        assert_eq!(g.world, WorldId::Seed(99));
        assert_eq!(g.depth, 1);
        assert_eq!(g.hp, GAME.balance.starting_hp);
        assert_eq!(g.maxhp, GAME.balance.starting_hp);
        assert_eq!(g.atk, GAME.balance.starting_atk);
        assert_eq!(g.light, GAME.balance.start_light);
        assert_eq!(g.saved.len(), GAME.win.max_depth as usize);
        assert_eq!(g.turns, 0);
        assert!(!g.dead && !g.won);
    }

    /// `Game::new_overworld` starts in `WorldId::Overworld` at screen 1, with
    /// a 3-slot stash, and the overworld's no-torch-clock exemption
    /// (`Game::spend_turn`) means a plain wait burns no light while still
    /// counting the turn.
    #[test]
    fn new_overworld_starts_in_overworld_and_burns_no_light() {
        let mut g = Game::new_overworld(1);
        assert_eq!(g.world, WorldId::Overworld);
        assert_eq!(g.depth, 1);
        assert_eq!(g.saved.len(), 3);
        let (light_before, turns_before) = (g.light, g.turns);
        g.wait_turn();
        assert_eq!(g.light, light_before, "the overworld has no torch clock");
        assert_eq!(g.turns, turns_before + 1, "turns still count as hashed, run-defining state");
        assert!(!g.dead, "the overworld can never kill you in the dark");
    }

    /// Crossing a `Tile::ScreenLink` moves between overworld screens
    /// instantly on walk-onto (SIGN-OFF ASK #2), in both directions, purely
    /// via `Game::depth` (the current-screen convention SIGN-OFF ASK #1
    /// reuses from `Floor`'s own `depth`-pinning).
    #[test]
    fn overworld_screen_link_crossing_moves_between_screens() {
        let mut g = Game::new_overworld(5);
        assert_eq!(g.depth, 1);
        let find_link = |g: &Game, want_east: Option<bool>| {
            (0..COLS as i32 * MAP_H as i32)
                .map(|i| (i % COLS as i32, i / COLS as i32))
                .find(|&(x, y)| match (g.map[idx(x, y)], want_east) {
                    (Tile::ScreenLink(e), Some(w)) => e == w,
                    (Tile::ScreenLink(_), None) => true,
                    _ => false,
                })
                .expect("expected screen-link tile not found")
        };
        let (lx, ly) = find_link(&g, Some(true)); // screen 1's only link is east
        step_onto(&mut g, lx, ly);
        assert_eq!(g.depth, 2, "crossing the east link moves to screen 2");
        assert_eq!(g.world, WorldId::Overworld);

        let (lx2, ly2) = find_link(&g, Some(false)); // cross back via screen 2's west link
        step_onto(&mut g, lx2, ly2);
        assert_eq!(g.depth, 1, "crossing the west link returns to screen 1");
    }

    /// Crossing a `Tile::Hole` transits from the overworld into the ROOT
    /// dungeon (`WorldId::Seed(seed)`), and the resulting depth-1 map is
    /// byte-identical to what `Game::new(seed)` generates directly — proof
    /// that reaching the dungeon via the hole is not a second, divergent
    /// worldgen path (batch-9 brief Design §1's central claim).
    #[test]
    fn overworld_hole_crossing_enters_root_dungeon_unchanged() {
        let mut g = Game::new_overworld(7);
        let (hx, hy) = (0..COLS as i32 * MAP_H as i32)
            .map(|i| (i % COLS as i32, i / COLS as i32))
            .find(|&(x, y)| g.map[idx(x, y)] == Tile::Hole)
            .expect("screen 1 must have a hole");
        step_onto(&mut g, hx, hy);
        assert_eq!(g.world, WorldId::Seed(7), "crossing the hole enters the root dungeon");
        assert_eq!(g.depth, 1);
        let direct = Game::new(7);
        assert_eq!(level_dump(&g), level_dump(&direct), "hole-entered dungeon must match Game::new's directly");
    }

    /// `instantiate_overworld_screen`'s DEFAULT player-start scan (the
    /// first-`Tile::Floor`-found-row-major rule, used both by
    /// `Game::new_overworld`'s screen-1 entry and by
    /// `cross_screen_link`'s own fresh-instantiate fallback before it
    /// overrides the placement) must never land the player on a tile
    /// occupied by a monster or an item — a real bug (found by review, then
    /// confirmed empirically) where OVERWORLD_1's row-major-first floor-like
    /// tile was the DONKEY's own `D` glyph: the map-value check
    /// `self.map[idx(tx,ty)] == Tile::Floor` is also true on an
    /// item/monster tile (both stamp `Tile::Floor` onto their own cell), so
    /// the player spawned exactly on top of the donkey and hid it from
    /// every render/dump (`--dump-overworld` showed `monsters=2` but only
    /// one visible glyph). Checked directly via `instantiate_overworld_screen`
    /// for every screen 1..=3, not just screen 1, so a future content pass
    /// can't silently reintroduce the same class of collision on screen 2/3.
    #[test]
    fn overworld_default_start_never_collides_with_monster_or_item() {
        for i in 1..=3usize {
            let mut g = Game::new(1); // any root-dungeon Game; only used as a scratch receiver
            g.instantiate_overworld_screen(i);
            let start = (g.px, g.py);
            for m in &g.monsters {
                assert_ne!(start, (m.x, m.y), "screen {} start collides with a monster", i);
            }
            for it in &g.items {
                assert_ne!(start, (it.x, it.y), "screen {} start collides with an item", i);
            }
        }
    }

    /// A `passive` monster (TRAINER/DONKEY) never chases or attacks, from
    /// spawn, regardless of `regard`/`calm` — `Game::monsters_act`'s new
    /// skip condition.
    #[test]
    fn passive_monster_never_chases_or_attacks() {
        let mut g = blank_room(1);
        g.monsters.push(Monster { x: 12, y: 11, kind: DONKEY, hp: GAME.monsters[DONKEY as usize].hp, regard: 0, calm: false, awe: 0 });
        let hp_before = g.hp;
        for _ in 0..20 {
            g.wait_turn();
            if g.dead {
                break;
            }
        }
        assert_eq!(g.hp, hp_before, "a passive monster never attacks even when adjacent");
        assert_eq!((g.monsters[0].x, g.monsters[0].y), (12, 11), "a passive monster never chases either");
    }

    /// `BumpResponse::Fight` (every pre-batch-9 kind) is unchanged: a bump
    /// attacks, the player doesn't move.
    #[test]
    fn bump_fight_kind_attacks_unchanged() {
        let mut g = blank_room(1);
        g.monsters.push(Monster { x: 11, y: 10, kind: RAT, hp: 3, regard: 0, calm: false, awe: 0 });
        g.try_move_player(1, 0);
        assert_eq!((g.px, g.py), (10, 10), "attacking doesn't move the player");
        assert!(g.monsters.is_empty() || g.monsters[0].hp < 3, "the rat takes damage or dies");
    }

    /// `BumpResponse::Yield` (the TRAINER's shape): a bump swaps position,
    /// never damages, exactly like a becalmed monster's yield — but
    /// unconditionally, with no talk required first.
    #[test]
    fn bump_yield_kind_swaps_without_damage() {
        let mut g = blank_room(1);
        let full_hp = GAME.monsters[TRAINER as usize].hp;
        g.monsters.push(Monster { x: 11, y: 10, kind: TRAINER, hp: full_hp, regard: 0, calm: false, awe: 0 });
        g.try_move_player(1, 0);
        assert_eq!((g.px, g.py), (11, 10), "player swaps into the trainer's tile");
        assert_eq!((g.monsters[0].x, g.monsters[0].y), (10, 10), "trainer swaps back to the player's old tile");
        assert_eq!(g.monsters[0].hp, full_hp, "never damaged");
    }

    /// `BumpResponse::Shove` (the DONKEY's shape): a bump onto plain floor
    /// pushes the donkey one tile and the player follows, never damaging it.
    #[test]
    fn bump_shove_kind_pushes_onto_floor() {
        let mut g = blank_room(1);
        let full_hp = GAME.monsters[DONKEY as usize].hp;
        g.monsters.push(Monster { x: 11, y: 10, kind: DONKEY, hp: full_hp, regard: 0, calm: false, awe: 0 });
        g.try_move_player(1, 0);
        assert_eq!((g.monsters[0].x, g.monsters[0].y), (12, 10), "donkey shoved one tile");
        assert_eq!((g.px, g.py), (11, 10), "player advances into the vacated tile");
        assert_eq!(g.monsters[0].hp, full_hp, "never damaged");
    }

    /// `BumpResponse::Shove` refuses (plants, no move, no turn, no damage)
    /// when the destination isn't plain walkable floor.
    #[test]
    fn bump_shove_kind_refuses_against_wall() {
        let mut g = blank_room(1);
        g.map[idx(12, 10)] = Tile::Wall;
        let full_hp = GAME.monsters[DONKEY as usize].hp;
        g.monsters.push(Monster { x: 11, y: 10, kind: DONKEY, hp: full_hp, regard: 0, calm: false, awe: 0 });
        let before_turns = g.turns;
        g.try_move_player(1, 0);
        assert_eq!((g.monsters[0].x, g.monsters[0].y), (11, 10), "donkey plants, does not move");
        assert_eq!((g.px, g.py), (10, 10), "player does not move");
        assert_eq!(g.turns, before_turns, "a refused shove costs no turn");
        assert_eq!(g.monsters[0].hp, full_hp, "never damaged");
    }

    /// `--dump-overworld` must always work headlessly and never open a
    /// window: sane output for all 3 screens, deterministic (seed-
    /// independent, zero RNG).
    #[test]
    fn dump_overworld_deterministic_and_sane() {
        let out = dump_overworld();
        assert_eq!(out, dump_overworld(), "dump_overworld must be deterministic");
        assert_eq!(out.matches("-- screen ").count(), 3, "must print exactly 3 screens");
        assert!(out.contains('='), "must show at least one screen-link glyph");
        assert!(out.contains('V'), "must show the hole glyph");
    }

    /// batch 10: the tactical routing view stamps a live monster's tile as Wall
    /// (so the bot prefers to route around it), but leaves a becalmed monster
    /// walkable (the engine swaps on that tile, no fight).
    #[test]
    fn tactical_routing_map_walls_live_monsters_only() {
        use headless::tactical_routing_map;
        let mut g = Game::new(7);
        // find a live monster on depth 1
        let m = g.monsters.iter().find(|m| !m.calm).expect("a live monster on d1");
        let (mx, my) = (m.x, m.y);
        let view = tactical_routing_map(&g);
        assert_eq!(view[idx(mx, my)], Tile::Wall, "live monster tile must be walled in the tactical view");

        // becalm every monster; now none should be stamped
        for mon in g.monsters.iter_mut() { mon.calm = true; }
        let view2 = tactical_routing_map(&g);
        assert_ne!(view2[idx(mx, my)], Tile::Wall, "a becalmed monster tile must stay walkable");
    }
}
