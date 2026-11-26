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
use headless::{Policy, dump, dump_overworld, probe_floors_main, sim_flip_main, sim_main, solve_main};
use rng::h64;
use save::{parse_save, replay, state_hash};

#[cfg(test)]
use content::ghost_label_idx;
#[cfg(test)]
use game::{
    COLS, Dest, Item, MAP_H, MAX_PUSH_CHAIN, MKind, Monster, Tile, WorldId, bfs_dist, fov_radius, idx,
    in_map, map_has_cache_reward, mood_shine_radius, receptivity,
};
#[cfg(test)]
use gamedef::{CarryEvent, ItemEffect, Minigame, PickupBehavior};
#[cfg(test)]
use games::GAME;
#[cfg(test)]
use games::contractor::{
    CHEESE, COAT, DONKEY, GOBLIN, LIGHT_CACHE, MIMIC, OBJECTIVE, OGRE, POTION, RAT, SWORD, TOWEL, TRAINER,
};
#[cfg(test)]
use headless::{band_scalar, diplomat_decline_byte, floor_dive_cost, level_dump, sim_seed, solve_seed};
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
    if args.iter().any(|a| a == "--sim-flip") {
        // batch 16: the relational flip gate — see `headless::sim_flip_main`'s
        // doc comment. Reuses `--sim`'s own default-5000 convention (not
        // `--sim`'s default-1000: the flip margin is calibrated for 5000
        // seeds, per `flip_n` in tests/tactical-pacifist-band.json) since
        // there's no separate `--sim` invocation on this code path to borrow
        // a count from.
        sim_flip_main(flag_val("--sim-flip").unwrap_or(5000));
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
    // batch 14 T2 (portal ROI): reports each authored floor's geometric
    // dive-cost probe next to the derived cache value — see
    // `headless::probe_floors_main`'s doc comment.
    if args.iter().any(|a| a == "--probe-floors") {
        probe_floors_main();
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
#[path = "tests/main.rs"]
mod tests;
