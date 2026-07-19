// headless.rs — verification tooling with no window dependency: the ASCII
// level dump (--dump), the winnability/difficulty solver (--solve), and the
// deterministic greedy-bot playthrough simulator (--sim). This is the test
// harness CLAUDE.md requires to work in an environment with no display.

use crate::content::{THEMES, theme_pick};
use crate::game::{COLS, Game, IKind, MAP_H, MAX_DEPTH, Monster, Tile, bfs_dist, idx, in_map};
use crate::rng::fnv_bytes;

/// World identity: FNV-1a over the full 5-depth dump. The seed names the
/// input; this names the OUTPUT, so it changes iff a worldgen MAJOR would
/// (same role as the golden fixtures, condensed to 16 hex chars a player
/// can compare over chat).
pub(crate) fn world_hash(seed: u64) -> u64 {
    fnv_bytes(0xcbf2_9ce4_8422_2325, dump(seed).as_bytes())
}

// ---------- Headless dump for testing ----------
pub(crate) fn level_dump(g: &Game) -> String {
    let mut out = String::new();
    for y in 0..MAP_H as i32 {
        for x in 0..COLS as i32 {
            let mut ch = match g.map[idx(x, y)] {
                Tile::Wall => '#',
                Tile::Floor => '.',
                Tile::Stairs => '>',
                Tile::UpStairs => '<',
            };
            for it in &g.items {
                if (it.x, it.y) == (x, y) {
                    ch = match it.kind {
                        IKind::Potion => '!',
                        IKind::Sword => ')',
                        IKind::Amulet => '&',
                        IKind::LoreA | IKind::LoreB | IKind::LoreC => '?',
                    };
                }
            }
            for m in &g.monsters {
                if (m.x, m.y) == (x, y) {
                    ch = Monster::stats(m.kind).2 as char;
                }
            }
            if (g.px, g.py) == (x, y) {
                ch = '@';
            }
            out.push(ch);
        }
        out.push('\n');
    }
    out.push_str(&format!("monsters={} items={}\n", g.monsters.len(), g.items.len()));
    out
}

// ---------- Solver: winnability + walk-budget gate ----------
/// Round-trip walk budget for one seed: for each depth, BFS from the entry
/// to the exit (down-stairs, or the amulet on the last depth). budget =
/// 3 × total shortest path — walk in ×1, carry the amulet out ×2 (it is
/// heavy). Returns None if any exit is unreachable (unwinnable seed).
pub(crate) fn solve_seed(seed: u64) -> Option<i32> {
    let mut g = Game::new(seed);
    let mut total = 0;
    for d in 1..=MAX_DEPTH {
        g.depth = d;
        g.gen_level();
        let target = if d < MAX_DEPTH {
            let s = (0..COLS as i32 * MAP_H as i32).find_map(|i| {
                let (x, y) = (i % COLS as i32, i / COLS as i32);
                if g.map[idx(x, y)] == Tile::Stairs { Some((x, y)) } else { None }
            });
            s?
        } else {
            let a = g.items.iter().find(|it| it.kind == IKind::Amulet)?;
            (a.x, a.y)
        };
        let dd = bfs_dist(&g.map, (g.px, g.py))[idx(target.0, target.1)];
        if dd < 0 {
            return None;
        }
        total += dd;
    }
    Some(3 * total)
}

/// Pull `"key": [lo, hi]` out of the band file without a JSON crate.
pub(crate) fn band_range(json: &str, key: &str) -> Option<(i32, i32)> {
    let k = format!("\"{}\"", key);
    let rest = &json[json.find(&k)? + k.len()..];
    let body = &rest[rest.find('[')? + 1..];
    let body = &body[..body.find(']')?];
    let mut it = body.split(',');
    Some((it.next()?.trim().parse().ok()?, it.next()?.trim().parse().ok()?))
}

/// `--solve N`: winnability + difficulty gate over seeds 0..N. Prints JSON
/// stats; exits nonzero on any unwinnable seed or drift outside the band
/// committed in tests/solver-band.json. With `--report`, prints the stats
/// and exits 0 without gating (for re-baselining after an authorized MAJOR).
pub(crate) fn solve_main(n: u64, report: bool) {
    let mut budgets: Vec<i32> = Vec::new();
    let mut losers: Vec<u64> = Vec::new();
    let (mut worst_seed, mut worst) = (0u64, -1i32);
    for seed in 0..n {
        match solve_seed(seed) {
            Some(b) => {
                if b > worst {
                    worst = b;
                    worst_seed = seed;
                }
                budgets.push(b);
            }
            None => losers.push(seed),
        }
    }
    budgets.sort_unstable();
    let pct = |p: usize| budgets[p * (budgets.len() - 1) / 100];
    let stats = [
        ("min", budgets[0]),
        ("p50", pct(50)),
        ("p90", pct(90)),
        ("p99", pct(99)),
        ("max", worst),
    ];
    println!("{{");
    println!("  \"seeds\": {},", n);
    for (k, v) in stats {
        println!("  \"{}\": {},", k, v);
    }
    println!("  \"worstSeed\": {},", worst_seed);
    println!("  \"unwinnable\": {}", losers.len());
    println!("}}");
    if report {
        return;
    }
    if !losers.is_empty() {
        eprintln!("UNWINNABLE: {}/{} seeds, e.g. {:?}", losers.len(), n, &losers[..losers.len().min(5)]);
        std::process::exit(1);
    }
    match std::fs::read_to_string("tests/solver-band.json") {
        Ok(band) => {
            for (k, v) in stats {
                if let Some((lo, hi)) = band_range(&band, k) {
                    if v < lo || v > hi {
                        eprintln!("difficulty drift: {}={} outside [{},{}]", k, v, lo, hi);
                        std::process::exit(1);
                    }
                }
            }
            println!("solver: {} seeds winnable, difficulty band OK", n);
        }
        Err(_) => {
            eprintln!("warning: tests/solver-band.json not found; drift check skipped");
        }
    }
}

/// Outcome of one `sim_seed` run: exactly one of won/dead_dark/dead_combat/
/// stuck is true (unless the run is still in progress, which never escapes
/// this function). turns is inputs emitted via apply_input; light_left is
/// the light remaining at the end of the run. `kills`/`spared` (batch 5 T2)
/// are `g.kills`/`g.spared` at the terminal state — for the greedy policy
/// `spared` is always 0 (it never emits a talk byte); for the pacifist
/// policy `kills` should always be 0 (`pacifist_never_attacks` asserts it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SimResult {
    pub(crate) won: bool,
    pub(crate) dead_dark: bool,
    pub(crate) dead_combat: bool,
    pub(crate) stuck: bool,
    pub(crate) turns: u32,
    pub(crate) light_left: i32,
    pub(crate) kills: u32,
    pub(crate) spared: u32,
}

/// First tile of kind `t` found scanning the map row-major (deterministic).
fn find_tile(map: &[Tile], t: Tile) -> Option<(i32, i32)> {
    (0..COLS as i32 * MAP_H as i32).find_map(|i| {
        let (x, y) = (i % COLS as i32, i / COLS as i32);
        if map[idx(x, y)] == t { Some((x, y)) } else { None }
    })
}

const SIM_TURN_CAP: u32 = 6000;
/// Fixed neighbor order for tie-breaking the greedy step: N, S, W, E — this
/// is also the apply_input byte order (0..=3), so the index IS the move.
const SIM_DIRS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

/// Sim bot policy (batch 5 T2, DECISION.md item 3 — pacifist band gate).
/// `Greedy` is the original bot (unchanged since batch 3): it bump-attacks
/// whatever is routed through. `Pacifist` shares every byte of the
/// loot/route logic in `sim_seed` and differs in exactly one place: when
/// the step it would take lands on a non-calm monster's tile, it emits the
/// direction-matched talk byte (7-10) instead of the move byte (0-3), so it
/// talks the blocker down (`Monster::talk_threshold`) rather than fighting
/// it. A becalmed monster on that tile is not a blocker — the move byte is
/// used and the engine's swap-on-bump takes over. No RNG of its own either
/// way; policy is a pure function of the byte about to be emitted.
///
/// Failed-talk retreat/persist rule (batch 5 addendum, per its own brief:
/// "pacifist policy may need a retreat-or-persist rule when talks fail —
/// keep it deterministic, document the policy change"): PERSIST — the
/// simplest deterministic option. This required NO code change here: a
/// failed talk (`game::receptivity` roll) does not stay OR move its
/// target (`Game::try_talk_player`'s failed branch), so the same monster
/// is still blocking the same tile next turn, and this policy's per-step
/// re-evaluation emits another talk byte at it automatically — the bot
/// just keeps talking until it lands (or until combat/dark ends the run).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Policy {
    Greedy,
    Pacifist,
}

impl Policy {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Policy::Greedy => "greedy",
            Policy::Pacifist => "pacifist",
        }
    }
}

/// Deterministic greedy bot: play one full seed to a win, death, or the
/// stuck cap, driving the game exclusively through `apply_input`. No RNG of
/// its own — every decision is a function of current Game state. BFS is
/// computed FROM the objective so picking the next step is a single
/// neighbor lookup: the first neighbor (in SIM_DIRS order) whose distance-
/// from-objective is one less than the player's.
///
/// Without the amulet, the bot sweeps the current depth's loot first: if
/// any reachable non-Amulet item remains, the objective is the nearest one
/// (BFS distance from the player, ties broken by smaller idx(x,y)) — this
/// mirrors minimal human play (grab swords/potions before diving) rather
/// than a blind beeline that never gets stronger. Only once the floor is
/// clear does the objective become the down-stairs (or the amulet itself
/// on depth 5). With the amulet, it's a pure beeline to the up-stairs — no
/// detours, since carrying it burns light at 2x.
///
/// `policy` (batch 5 T2) only touches the very last step: see `Policy`'s
/// doc comment. Every BFS/objective/tie-break decision above is identical
/// for both policies — this is deliberate, so a pacifist-band regression
/// can never be a route/loot bug in disguise, only a mercy-vs-combat one.
pub(crate) fn sim_seed(seed: u64, policy: Policy) -> SimResult {
    let mut g = Game::new(seed);
    let mut turns: u32 = 0;
    loop {
        if g.dead {
            let dark = g.light == 0;
            return SimResult {
                won: false,
                dead_dark: dark,
                dead_combat: !dark,
                stuck: false,
                turns,
                light_left: g.light,
                kills: g.kills,
                spared: g.spared,
            };
        }
        if g.won {
            return SimResult {
                won: true,
                dead_dark: false,
                dead_combat: false,
                stuck: false,
                turns,
                light_left: g.light,
                kills: g.kills,
                spared: g.spared,
            };
        }
        let stuck = |turns, light_left, kills, spared| SimResult {
            won: false,
            dead_dark: false,
            dead_combat: false,
            stuck: true,
            turns,
            light_left,
            kills,
            spared,
        };
        if turns >= SIM_TURN_CAP {
            return stuck(turns, g.light, g.kills, g.spared);
        }
        let objective = if g.has_amulet {
            find_tile(&g.map, Tile::UpStairs)
        } else {
            let dist_from_player = bfs_dist(&g.map, (g.px, g.py));
            let loot = g
                .items
                .iter()
                .filter(|it| it.kind != IKind::Amulet)
                .filter(|it| in_map(it.x, it.y) && dist_from_player[idx(it.x, it.y)] >= 0)
                .min_by_key(|it| (dist_from_player[idx(it.x, it.y)], idx(it.x, it.y)))
                .map(|it| (it.x, it.y));
            loot.or_else(|| {
                if g.depth < MAX_DEPTH {
                    find_tile(&g.map, Tile::Stairs)
                } else {
                    g.items.iter().find(|it| it.kind == IKind::Amulet).map(|it| (it.x, it.y))
                }
            })
        };
        let Some(objective) = objective else {
            return stuck(turns, g.light, g.kills, g.spared);
        };
        let dist = bfs_dist(&g.map, objective);
        let player_d = dist[idx(g.px, g.py)];
        if player_d < 0 {
            return stuck(turns, g.light, g.kills, g.spared);
        }
        // Walking onto Stairs always descends, and onto UpStairs always
        // ascends once depth > 1 — both unconditional, regardless of
        // intent (see `try_move_player`/`descend`/`ascend`). ascend()/
        // descend() also reposition the player directly (bypassing the
        // walk-in transition logic), so it's possible to arrive at the top
        // of a turn already standing exactly on this turn's objective
        // (dist 0) with no lower-distance neighbor to step to — handle
        // that first: sidestep to any open neighbor and let the return
        // trip trigger the transition properly next turn.
        let would_transition =
            |t: Tile| t == Tile::Stairs || (t == Tile::UpStairs && g.depth > 1);
        let step = if player_d == 0 {
            SIM_DIRS.iter().enumerate().find_map(|(b, (dx, dy))| {
                let (nx, ny) = (g.px + dx, g.py + dy);
                if in_map(nx, ny) && g.map[idx(nx, ny)] != Tile::Wall { Some(b as u8) } else { None }
            })
        } else {
            // Distances are computed on the real map — a transition tile is
            // ordinary floor for routing purposes, since walling it off
            // would also wall off the player's own tile whenever they're
            // standing on one (true at the top of every level). What we
            // want to avoid is stepping onto it as a mere waypoint while
            // routing toward something else (e.g. dragging the bot up a
            // depth, or down before its loot sweep is done). So prefer any
            // shortest-path neighbor that isn't a transition tile; only
            // fall back to it if it's the sole neighbor on the shortest
            // path (a genuine chokepoint — the room's only way out —
            // rather than an incidental detour), since refusing it outright
            // would be a false "stuck" over unavoidable level geometry.
            let mut transition_fallback: Option<u8> = None;
            SIM_DIRS
                .iter()
                .enumerate()
                .find_map(|(b, (dx, dy))| {
                    let (nx, ny) = (g.px + dx, g.py + dy);
                    if !in_map(nx, ny) || dist[idx(nx, ny)] != player_d - 1 {
                        return None;
                    }
                    if (nx, ny) != objective && would_transition(g.map[idx(nx, ny)]) {
                        transition_fallback.get_or_insert(b as u8);
                        return None;
                    }
                    Some(b as u8)
                })
                .or(transition_fallback)
        };
        match step {
            Some(b) => {
                // Pacifist (batch 5 T2): if the tile this step lands on
                // holds a non-calm monster, talk instead of swing — talk
                // bytes mirror the move bytes' direction order exactly
                // (7-10 = N/S/W/E, see apply_input), so `7 + b` is always
                // the correctly-directed talk. A calm monster on that tile
                // is not a blocker (the engine swaps on the move byte), so
                // it falls through to the normal move below unchanged.
                let (dx, dy) = SIM_DIRS[b as usize];
                let (nx, ny) = (g.px + dx, g.py + dy);
                let blocked = policy == Policy::Pacifist
                    && g.monsters.iter().any(|m| m.x == nx && m.y == ny && !m.calm);
                g.apply_input(if blocked { 7 + b } else { b });
                turns += 1;
            }
            None => return stuck(turns, g.light, g.kills, g.spared),
        }
    }
}

/// `--sim N [--policy greedy|pacifist]`: play N full runs (seeds 0..N) with
/// the deterministic bot for the given `Policy` and print aggregate JSON
/// stats. Proves the actual game loop (combat, light burn, mercy/talk, stair
/// persistence, pickups) is playable end to end, and turns "does the light
/// margin play fair?" / "is mercy viable?" into measured data.
///
/// Since the batch-3 balance pass this is also a GATE (like --solve), one
/// per policy:
/// - `Greedy` (unchanged since batch 3): stats must sit inside
///   tests/sim-band.json — win_pct in [10,25], deaths_dark nonzero but a
///   minority (structural check below, not a JSON band — see the old doc
///   note this comment absorbed: an integer percent band can't encode
///   "nonzero but tiny", ~0.2% of deaths at 5000 seeds).
/// - `Pacifist` (batch 5 T2, DECISION.md item 3): stats must sit inside
///   tests/pacifist-band.json — win_pct in [5,40] (mercy may be harder than
///   violence, must not dominate it; see that file's comment for the full
///   measured death-mix, which is recorded as data, not gated). No
///   dark/combat minority requirement for pacifist — that invariant was
///   never claimed for this policy and is measured honestly instead.
///
/// `stuck` must be 0 for either policy or we exit nonzero (a policy or
/// reachability bug, not a balance question). `--report` prints stats and
/// exits 0 (the re-baselining flow). Bands are calibrated for the default
/// 5000-seed run (`make sim`, which runs both policies); smaller runs may
/// trip a floor spuriously.
pub(crate) fn sim_main(n: u64, report: bool, policy: Policy) {
    let mut wins = 0u64;
    let mut deaths_combat = 0u64;
    let mut deaths_dark = 0u64;
    let mut stuck = 0u64;
    let mut win_turns: Vec<u32> = Vec::new();
    let mut win_light: Vec<i32> = Vec::new();
    let mut kills_total = 0u64;
    let mut spared_total = 0u64;
    for seed in 0..n {
        let r = sim_seed(seed, policy);
        kills_total += r.kills as u64;
        spared_total += r.spared as u64;
        if r.won {
            wins += 1;
            win_turns.push(r.turns);
            win_light.push(r.light_left);
        } else if r.dead_dark {
            deaths_dark += 1;
        } else if r.dead_combat {
            deaths_combat += 1;
        } else if r.stuck {
            stuck += 1;
        }
    }
    win_turns.sort_unstable();
    win_light.sort_unstable();
    let median_u32 = |v: &[u32]| -> u32 {
        if v.is_empty() { 0 } else { v[v.len() / 2] }
    };
    let median_i32 = |v: &[i32]| -> i32 {
        if v.is_empty() { 0 } else { v[v.len() / 2] }
    };
    let win_rate = if n > 0 { wins as f64 / n as f64 } else { 0.0 };
    let p10_light = if win_light.is_empty() { 0 } else { win_light[win_light.len() / 10] };
    println!(
        "{{\"policy\":\"{}\",\"runs\":{},\"wins\":{},\"win_rate\":{:.3},\"deaths_combat\":{},\"deaths_dark\":{},\"stuck\":{},\"median_turns_win\":{},\"median_light_left_win\":{},\"p10_light_left_win\":{},\"min_light_left_win\":{},\"kills_total\":{},\"spared_total\":{}}}",
        policy.name(),
        n,
        wins,
        win_rate,
        deaths_combat,
        deaths_dark,
        stuck,
        median_u32(&win_turns),
        median_i32(&win_light),
        p10_light,
        win_light.first().copied().unwrap_or(0),
        kills_total,
        spared_total,
    );
    if report {
        return;
    }
    if stuck > 0 {
        eprintln!("sim ({}): {} runs stuck — bot policy or reachability bug", policy.name(), stuck);
        std::process::exit(1);
    }
    let win_pct = (wins * 100 / n.max(1)) as i32;
    let band_path = match policy {
        Policy::Greedy => "tests/sim-band.json",
        Policy::Pacifist => "tests/pacifist-band.json",
    };
    match std::fs::read_to_string(band_path) {
        Ok(band) => {
            let checks: &[(&str, i32)] = match policy {
                Policy::Greedy => &[("win_pct", win_pct), ("deaths_dark", deaths_dark as i32)],
                Policy::Pacifist => &[("win_pct", win_pct)],
            };
            for &(k, v) in checks {
                if let Some((lo, hi)) = band_range(&band, k) {
                    if v < lo || v > hi {
                        eprintln!("sim drift ({}): {}={} outside [{},{}]", policy.name(), k, v, lo, hi);
                        std::process::exit(1);
                    }
                }
            }
            // "minority": darkness may claim runs, but combat must claim
            // more. Not a JSON band (see sim_main doc comment): at ~0.1%
            // dark share a percent band floors to 0, so this is a
            // structural code check. Greedy-only — see sim_main doc
            // comment for why pacifist doesn't carry this invariant.
            if policy == Policy::Greedy && deaths_dark >= deaths_combat {
                eprintln!(
                    "sim drift: deaths_dark {} >= deaths_combat {} — the old wall in a new mask",
                    deaths_dark, deaths_combat
                );
                std::process::exit(1);
            }
            println!(
                "sim ({}): {} runs, win_pct {} and dark deaths {} inside band",
                policy.name(),
                n,
                win_pct,
                deaths_dark
            );
        }
        Err(_) => {
            eprintln!("warning: {} not found; sim band check skipped", band_path);
        }
    }
}

/// Full-run dump: every depth of the seed's dungeon, generated directly.
pub(crate) fn dump(seed: u64) -> String {
    let mut g = Game::new(seed);
    let mut out = format!("seed={}\n", seed);
    for d in 1..=MAX_DEPTH {
        g.depth = d;
        g.gen_level();
        let t = &THEMES[theme_pick(seed, d).0];
        out.push_str(&format!("-- depth {} : {} --\n", d, t.label));
        out.push_str(&level_dump(&g));
    }
    out
}
