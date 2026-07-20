// headless.rs — verification tooling with no window dependency: the ASCII
// level dump (--dump), the winnability/difficulty solver (--solve), and the
// deterministic greedy-bot playthrough simulator (--sim). This is the test
// harness CLAUDE.md requires to work in an environment with no display.

use crate::content::theme_for;
use crate::game::{COLS, Game, MAP_H, Monster, Tile, WorldId, bfs_dist, idx, in_map, max_depth};
use crate::games::GAME;
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
                Tile::Portal => '*',
                Tile::Pit => '^',
                Tile::Goal => 'x',
            };
            for it in &g.items {
                if (it.x, it.y) == (x, y) {
                    ch = GAME.items[it.kind as usize].glyph as char;
                }
            }
            // push-blocks (batch 6 T2, sokoban): drawn after items so a
            // block covers a hidden item, matching render.rs's same
            // item-then-block layering (see `Game::blocks`' doc comment).
            for &(bx, by) in &g.blocks {
                if (bx, by) == (x, y) {
                    ch = 'B';
                }
            }
            for m in &g.monsters {
                if (m.x, m.y) == (x, y) {
                    ch = Monster::stats(m.kind).glyph as char;
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
/// to the exit (down-stairs, or the win-condition item on the last depth).
/// budget = 3 × total shortest path — walk in ×1, carry the objective out
/// ×2 (it is heavy). Returns None if any exit is unreachable (unwinnable
/// seed).
pub(crate) fn solve_seed(seed: u64) -> Option<i32> {
    let mut g = Game::new(seed);
    let mut total = 0;
    for d in 1..=max_depth() {
        g.depth = d;
        g.gen_level();
        let target = if d < max_depth() {
            let s = (0..COLS as i32 * MAP_H as i32).find_map(|i| {
                let (x, y) = (i % COLS as i32, i / COLS as i32);
                if g.map[idx(x, y)] == Tile::Stairs { Some((x, y)) } else { None }
            });
            s?
        } else {
            let a = g.items.iter().find(|it| it.kind == GAME.win.objective_item)?;
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

/// A copy of `g.map` with every `Game::blocks` position stamped `Tile::Wall`
/// (batch 6 T2, sokoban) — `sim_seed`'s PREFERRED routing view (see the
/// call site's doc comment for why this is a preference, not a
/// reachability proof, and never `game::bfs_dist`'s or `solve_seed`'s own
/// view — see `game::bfs_dist`'s doc comment for why blocks-awareness
/// isn't safe at that layer). Rebuilt fresh each call (`g.map`/`g.blocks`
/// both change turn to turn) — a full-grid clone plus a handful of writes,
/// cheap at this scale (COLS*MAP_H = 2000 cells, same order of magnitude
/// `bfs_dist` itself already walks every call).
fn routing_map(g: &Game) -> Vec<Tile> {
    let mut m = g.map.clone();
    for &(bx, by) in &g.blocks {
        m[idx(bx, by)] = Tile::Wall;
    }
    m
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
/// Without the win-condition item, the bot sweeps the current depth's loot
/// first: if any reachable non-objective item remains, the target is the
/// nearest one (BFS distance from the player, ties broken by smaller
/// idx(x,y)) — this mirrors minimal human play (grab swords/potions before
/// diving) rather than a blind beeline that never gets stronger. Only once
/// the floor is clear does the target become the down-stairs (or the
/// objective item itself on the last depth). Once holding it, it's a pure
/// beeline to the up-stairs — no detours, since carrying it burns light
/// faster.
///
/// `policy` (batch 5 T2) only touches the very last step: see `Policy`'s
/// doc comment. Every BFS/objective/tie-break decision above is identical
/// for both policies — this is deliberate, so a pacifist-band regression
/// can never be a route/loot bug in disguise, only a mercy-vs-combat one.
///
/// Returns the terminal `WorldId` alongside the `SimResult` (batch 6 T1):
/// the bot drives the game exclusively through `apply_input` with move
/// bytes 0-3 (talk bytes 7-10 for `Policy::Pacifist`, and USE byte 15 for
/// both policies once batch 7 T2's half-HP-and-holding-a-potion rule fires
/// — see the doc comment at that check's call site) — it NEVER emits wait
/// (byte 4) or GIVE (11-14, unused by either policy this batch), the only
/// input that can transit a portal (see
/// `game::Game::wait_turn`'s doc comment) — so this should always come back
/// `WorldId::Seed(seed)` (still the root world). `sim_main` ignores it;
/// `bot_never_transits` (main.rs) is the test that actually checks it,
/// which is the whole reason it's threaded out here instead of asserted
/// silently inside this function.
pub(crate) fn sim_seed(seed: u64, policy: Policy) -> (SimResult, WorldId) {
    let mut g = Game::new(seed);
    let mut turns: u32 = 0;
    loop {
        if g.dead {
            let dark = g.light == 0;
            return (
                SimResult {
                    won: false,
                    dead_dark: dark,
                    dead_combat: !dark,
                    stuck: false,
                    turns,
                    light_left: g.light,
                    kills: g.kills,
                    spared: g.spared,
                },
                g.world,
            );
        }
        if g.won {
            return (
                SimResult {
                    won: true,
                    dead_dark: false,
                    dead_combat: false,
                    stuck: false,
                    turns,
                    light_left: g.light,
                    kills: g.kills,
                    spared: g.spared,
                },
                g.world,
            );
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
            return (stuck(turns, g.light, g.kills, g.spared), g.world);
        }
        // batch 7 T2: the potion moved from walk-over Consume to Hold this
        // batch (see `ItemDef::on_pickup`'s doc comment) — a bot that never
        // learned USE would simply never heal again, which is a real
        // behavior change the bands must measure honestly rather than paper
        // over. Both policies apply the same deterministic rule, evaluated
        // every turn before routing: half HP or worse, AND the cartridge's
        // designated potion item (`GAME.balance.loot_potion_item` — the
        // engine names it generically, same convention `gen_level`'s loot
        // table already uses) is on TOP of `held` (`held.last()` — USE
        // always acts on the LIFO top) -> emit USE (byte 15) and skip
        // straight to the next turn. Deliberately `last()`, not `contains()`
        // (review fix): a held item with `on_use: None` sitting on top of a
        // buried potion makes USE a COMPLETE no-op (`Game::use_item` returns
        // before touching any state at all, not even spending a turn), so a
        // `contains()` check would fire the identical no-op every single
        // iteration forever — an infinite loop that only ever ends at
        // `SIM_TURN_CAP`, misreported as "stuck" (confirmed empirically:
        // seeds 14/17/25/30/33/34/38/43/44 of 0..50 hung at turns=6000
        // under `contains()` and are clean under `last()`). If the potion
        // isn't on top, this bot simply doesn't drink it yet — an accepted
        // limitation ("manage your inventory" is not this rule's job), not
        // a bug, and never an infinite loop either way since every OTHER
        // held item this cartridge ships with a real `on_use` still spends
        // a real turn.
        if 2 * g.hp <= g.maxhp && g.held.last() == Some(&GAME.balance.loot_potion_item) {
            g.apply_input(15);
            turns += 1;
            continue;
        }
        // Primary routing view (batch 6 T2 review fix): `Game::blocks`
        // stamped `Tile::Wall` (`routing_map`) — a sim bot never
        // deliberately solves a sokoban puzzle, so its PREFERRED routing
        // should walk AROUND a block whenever ordinary floor offers a way
        // around one (which it very often does — blocks sit in ordinary
        // rooms/corridors, not sealed tunnels), exactly like a human
        // player who doesn't feel like pushing today. This is deliberately
        // NOT the same view `solve_seed`/`game::gen_level` use (see
        // `game::bfs_dist`'s doc comment) — this one is local to routing
        // preference, never reachability proof. The genuine "must push to
        // progress" case is handled by the wander fallback below, not
        // here.
        let rmap = routing_map(&g);
        let objective = if g.has_objective {
            find_tile(&g.map, Tile::UpStairs)
        } else {
            let dist_from_player = bfs_dist(&rmap, (g.px, g.py));
            let loot = g
                .items
                .iter()
                .filter(|it| it.kind != GAME.win.objective_item)
                .filter(|it| in_map(it.x, it.y) && dist_from_player[idx(it.x, it.y)] >= 0)
                .min_by_key(|it| (dist_from_player[idx(it.x, it.y)], idx(it.x, it.y)))
                .map(|it| (it.x, it.y));
            loot.or_else(|| {
                if g.depth < max_depth() {
                    find_tile(&g.map, Tile::Stairs)
                } else {
                    g.items.iter().find(|it| it.kind == GAME.win.objective_item).map(|it| (it.x, it.y))
                }
            })
        };
        let Some(objective) = objective else {
            return (stuck(turns, g.light, g.kills, g.spared), g.world);
        };
        let dist = bfs_dist(&rmap, objective);
        let player_d = dist[idx(g.px, g.py)];
        // batch 6 T2 review fix: a block's tile is a WALL in `rmap`, so
        // `player_d < 0` here means "unreachable while avoiding every
        // block" — not necessarily "unreachable, full stop." Don't declare
        // stuck yet; fall through to the wander fallback below, which is
        // allowed to push. Only genuinely `player_d >= 0` routes take the
        // strict shortest-path branch below; `player_d < 0` skips straight
        // to `step = None`, same as "no live candidate found."
        let step_is_live = |nx: i32, ny: i32, dx: i32, dy: i32| -> bool {
            !g.blocks.contains(&(nx, ny)) || g.would_push_succeed(nx, ny, dx, dy)
        };
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
        let step = if player_d < 0 {
            None
        } else if player_d == 0 {
            SIM_DIRS.iter().enumerate().find_map(|(b, (dx, dy))| {
                let (nx, ny) = (g.px + dx, g.py + dy);
                // batch 6 T2: a pit is exactly as illegal a sidestep target
                // as a wall (see `Tile::Pit`'s doc comment) — this path
                // doesn't go through `bfs_dist`, so it needs the same
                // exclusion made explicit here. `rmap` already makes a
                // block a Wall, so no separate block check is needed on
                // this branch (matches `dist`'s own view).
                let ok = in_map(nx, ny) && !matches!(rmap[idx(nx, ny)], Tile::Wall | Tile::Pit);
                if ok { Some(b as u8) } else { None }
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
                    // `dist` is `rmap`-based (blocks are Wall there), so a
                    // block-occupied neighbor never matches `player_d - 1`
                    // in the first place — no separate block check needed
                    // on this branch.
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
        // Last-resort wander fallback (batch 6 T2 review fix): the strict
        // shortest-path search above can come back empty even when the
        // objective genuinely IS reachable, because it only ever considers
        // the ONE step a plain (block-oblivious) `bfs_dist` calls optimal —
        // if that step happens to be a block whose push doesn't currently
        // succeed (see `step_is_live`), a real player could still make
        // progress by taking a DIFFERENT legal step this turn (a push that
        // does succeed, or ordinary floor) and re-routing next turn once
        // state has changed. Taking any merely-legal neighbor — not
        // necessarily distance-reducing — converts a premature false
        // "stuck" into either eventual progress or, worst case, the SAME
        // outcome (SIM_TURN_CAP) a genuinely-unsolvable-by-this-bot
        // situation would have reached anyway. Never fires outside a
        // sokoban dead end: ordinary reachable floor always has SOME
        // distance-reducing neighbor, so the strict search above never
        // comes back empty for it.
        let step = step.or_else(|| {
            SIM_DIRS.iter().enumerate().find_map(|(b, (dx, dy))| {
                let (nx, ny) = (g.px + dx, g.py + dy);
                let ok = in_map(nx, ny)
                    && !matches!(g.map[idx(nx, ny)], Tile::Wall | Tile::Pit)
                    && step_is_live(nx, ny, *dx, *dy);
                if ok { Some(b as u8) } else { None }
            })
        });
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
            None => return (stuck(turns, g.light, g.kills, g.spared), g.world),
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
///   tests/pacifist-band.json — win_pct in [5,20] (mercy may be harder than
///   violence, must not dominate it; the JSON file is the authority for the
///   exact bound — see its comment for the full measured death-mix and
///   re-baseline history, which is recorded as data, not gated here). No
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
        let (r, _world) = sim_seed(seed, policy);
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
/// Root-world-only (batch 6 T1): `Game::new(seed)` always starts in
/// `WorldId::Seed(seed)` with that same `seed` as `Game::seed`, so this is
/// by definition the ROOT world for the passed-in seed (see
/// `game::WorldId`'s doc comment on how "root" is a comparison, not a
/// stored flag). A portal world is dumpable too — by dumping ITS seed
/// (`Dest::World`'s derived `u64`) directly, which works for free since
/// `dump` never inspects `Game::world` — this function's own portal tiles
/// (`*`, `level_dump`'s `Tile::Portal` arm) just show where such a seed
/// could be reached from, not that this call reaches it.
pub(crate) fn dump(seed: u64) -> String {
    let mut g = Game::new(seed);
    let mut out = format!("seed={}\n", seed);
    for d in 1..=max_depth() {
        g.depth = d;
        g.gen_level();
        let t = theme_for(seed, d);
        out.push_str(&format!("-- depth {} : {} --\n", d, t.label));
        out.push_str(&level_dump(&g));
    }
    out
}
