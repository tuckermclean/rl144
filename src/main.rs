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
use headless::{Policy, dump, sim_main, solve_main};
use rng::h64;
use save::{parse_save, replay, state_hash};

#[cfg(test)]
use content::ghost_label_idx;
#[cfg(test)]
use game::{
    COLS, Dest, MAP_H, MAX_PUSH_CHAIN, MKind, Monster, Tile, WorldId, bfs_dist, idx, in_map,
    receptivity,
};
#[cfg(test)]
use games::GAME;
#[cfg(test)]
use games::contractor::{GOBLIN, OGRE, RAT};
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
        // pacifist` selects the mercy bot (batch 5 T2). Any other value
        // (including a typo) falls back to greedy rather than silently
        // matching nothing, same tolerance as the rest of this arg parser.
        let policy = if str_val("--policy").as_deref() == Some("pacifist") {
            Policy::Pacifist
        } else {
            Policy::Greedy
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
        for (vi, v) in GAME.vaults.iter().enumerate() {
            let rows: Vec<&str> = v.lines().collect();
            let w = rows[0].len();
            assert!(rows.len() >= 3 && w >= 3, "vault {} too small", vi);
            for (j, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), w, "vault {} row {} ragged", vi, j);
                for (i, c) in row.bytes().enumerate() {
                    // batch 6 T2: '^' pit, 'B' block, 'x' goal (sokoban).
                    assert!(b"#.!)rgO^Bx".contains(&c), "vault {} bad char {}", vi, c as char);
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
        g.monsters.push(Monster { x: 12, y: 10, kind: RAT, hp: 3, regard: 0, calm: false });
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
    /// pushes a 2-chain into the pit (destroying the farthest member,
    /// filling the gap), keeps pushing the survivor onto the goal tile
    /// (locking it), then walks to the reward.
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
        assert!(!g.items.is_empty(), "fixture: the goal cell vault has a reward item");
        assert_eq!(g.blocks.len(), 2, "fixture: the goal cell vault starts with a 2-chain");
        for _ in 0..7 {
            // East: 2-chain into the pit, survivor onto the goal, walk to the reward.
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
        g.has_objective = true;
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
        });
        g.monsters.push(Monster {
            x: px + bdx,
            y: py + bdy,
            kind: GOBLIN,
            hp: 6,
            regard: 0,
            calm: false,
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
        let fresh_ogre = Monster { x: 0, y: 0, kind: OGRE, hp: 13, regard: 0, calm: false };
        assert_eq!(receptivity(&fresh_ogre, &g), 20, "a fresh ogre should sit at exactly its BASE");

        // Wounded (1 of 13 hp -> wound term 40*(13-1)/13 = 36) plus a
        // strong player (atk 9 -> +6*(9-3) = 36) pushes well past 70:
        // 20 + 0 + 36 + 36 - 0 = 92.
        g.atk = 9;
        let wounded_ogre = Monster { x: 0, y: 0, kind: OGRE, hp: 1, regard: 0, calm: false };
        let r = receptivity(&wounded_ogre, &g);
        assert!(r >= 70, "wounded ogre + high atk should land >= 70-ish, got {}", r);
        assert_eq!(r, 92, "and the exact integer math should hold");

        // Clamp floor: an unnaturally weak "player" (atk 0, below the
        // baseline 3) plus a guttering torch (fov_radius(1) is deep in the
        // bottom tier, well <= 4) drives the raw sum negative
        // (20 + 0 + 0 - 18 - 10 = -8); receptivity must still floor at 5.
        g.atk = 0;
        g.light = 1;
        let floor_ogre = Monster { x: 0, y: 0, kind: OGRE, hp: 13, regard: 0, calm: false };
        assert_eq!(receptivity(&floor_ogre, &g), 5, "receptivity must clamp at the floor of 5");

        // Clamp ceiling: a high-regard, badly wounded rat with a very
        // strong player would compute far past 100; receptivity must cap
        // at 95.
        g.atk = 20;
        g.light = game::start_light();
        let capped_rat = Monster { x: 0, y: 0, kind: RAT, hp: 1, regard: 10, calm: false };
        assert_eq!(receptivity(&capped_rat, &g), 95, "receptivity must clamp at the ceiling of 95");
    }

    /// Landed-vs-failed determinism (batch 5 addendum): two independent
    /// live games from the same seed, talked at the same fresh ogre the
    /// same number of times, produce an identical `state_hash` — whether
    /// each individual roll happened to land or fail. `parley_rng` has no
    /// external entropy, so the exact sequence of landed/failed outcomes
    /// must repeat exactly. Also confirms the scenario actually exercises
    /// both outcomes (an ogre's low BASE receptivity makes at least one
    /// failure near-certain within the attempt cap; regard climbing toward
    /// `Monster::talk_threshold` makes at least one landing certain too).
    #[test]
    fn parley_landed_vs_failed_deterministic() {
        let run = |seed: u64| -> (u64, bool, bool) {
            let mut g = Game::new(seed);
            g.monsters.clear();
            // Headroom: a fresh ogre's failed rolls attack for real damage
            // (it is never stayed on a failed roll — that's the property
            // under test), and this ogre's low BASE receptivity means
            // several fails are likely before the first landing.
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
    /// contains byte 6 or bytes 7-10 — neither INPUT_RETRY nor talk existed
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
    /// never contains bytes 7-10 (talk didn't exist until save v3, batch 5),
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
    /// (game.rs's `apply_input` handling talk bytes 7-10 is the other half).
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

    /// Batch 6 T1: neither sim policy ever emits the wait byte (4) — the
    /// only input that can transit a portal (`game::Game::wait_turn`'s doc
    /// comment) — so a bot run must never leave the root world, regardless
    /// of how many portals its route happens to walk over (walking ONTO one
    /// only logs, per `Game::land_on_tile`'s `Tile::Portal` arm). Checked
    /// over a range wide enough to almost certainly cross at least one
    /// portal-bearing depth (~1/4 chance each, per `Game::gen_level`'s
    /// portal-placement comment) for both policies.
    #[test]
    fn bot_never_transits() {
        for seed in 0..100u64 {
            let (_, world) = sim_seed(seed, Policy::Greedy);
            assert!(world == WorldId::Seed(seed), "greedy bot left the root world on seed {}", seed);
            let (_, world) = sim_seed(seed, Policy::Pacifist);
            assert!(
                world == WorldId::Seed(seed),
                "pacifist bot left the root world on seed {}",
                seed
            );
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
            let (r, _world) = sim_seed(seed, Policy::Greedy);
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
}
