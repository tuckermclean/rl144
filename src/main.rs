// rl144 — a roguelike in under 1.44MB. Zero asset files; everything procedural or const.
mod content;
mod game;
mod headless;
mod render;
mod rng;
mod save;

use game::Game;
use headless::{dump, sim_main, solve_main, world_hash};
use minifb::{Key, KeyRepeat, ScaleMode, Window, WindowOptions};
use render::{HEIGHT, WIDTH, render};
use rng::h64;
use save::{INPUT_RESTART, parse_save, replay, save_bytes, save_filename, state_hash};

#[cfg(test)]
use content::{KINDS, THEMES, TONE_LINES, VAULTS};
#[cfg(test)]
use game::{Tile, idx, in_map};
#[cfg(test)]
use headless::{level_dump, sim_seed, solve_seed};
#[cfg(test)]
use render::scale;
#[cfg(test)]
use rng::channel;

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

    let (seed0, mut input_log, mut game) = match str_val("--load") {
        Some(path) => match std::fs::read(&path).ok().and_then(|b| parse_save(&b)) {
            Some((s0, inputs)) => {
                let g = replay(s0, &inputs);
                (s0, inputs, g)
            }
            None => {
                eprintln!("bad save file: {}", path);
                std::process::exit(1);
            }
        },
        None => (seed, Vec::new(), Game::new(seed)),
    };
    if daily && input_log.is_empty() {
        game.log(format!("Daily dungeon #{}. Everyone gets this one today.", day));
    }
    // The 80x30 cell grid (640x360 logical pixels) is engine API — COLS and
    // MAP_H are baked into worldgen, so the grid can never follow the
    // window. The window is presentation: minifb scales the fixed buffer,
    // preserving aspect. A DOS or mobile frontend swaps this block, not the
    // grid.
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

    while window.is_open() && !window.is_key_down(Key::Escape) {
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
        if (game.dead || game.won) && window.is_key_pressed(Key::R, KeyRepeat::No) {
            input_log.push(INPUT_RESTART);
            let s = h64(game.seed, &["restart"]);
            game = Game::new(s);
            whash = world_hash(s);
            window.set_title(&title(s));
            confirm_armed = false;
        }
        // F1: identify the world. Log-only — consumes no turn, no input byte,
        // and touches no RNG channel, so replay is unaffected.
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
        render(&game, &mut buf);
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
    /// and so must the fixed-shape flavor messages.
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
