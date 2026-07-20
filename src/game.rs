// game.rs — the engine core: grid/game constants, world state (Tile, Monster,
// Item, LevelState, Game) and all Game behavior (worldgen, FOV, turn logic,
// combat, save/restore of visited depths), plus the grid helpers idx/in_map
// and the solver's bfs_dist. apply_input is the client input-vocabulary
// boundary between this engine and any frontend (window loop, replay, sim).
//
// THE UNBRAIDING (batch 7 T1): every game-specific fact (monster/item
// stats, theme flavor, vault layouts, balance numbers, message templates)
// used to live here as literal consts and match arms. It has all moved to
// `crate::gamedef::GameDef` data, authored per-cartridge under `games/`
// (one module per game; see that directory for the one shipped today). This
// module reaches that data through exactly one seam — `crate::games::GAME`,
// re-exported from `games/mod.rs` — and otherwise consumes only `GameDef`
// fields:
// `MKind`/`IKind` are plain indices into `GAME.monsters`/`GAME.items`, not
// named variants; a monster/item's stats, glyph, color, and flavor text are
// all looked up by index, never spelled out here.

use crate::content::theme_for;
use crate::gamedef::ItemEffect;
use crate::games::GAME;
use crate::render::Facing;
use crate::rng::{Rng, channel, h64};
// Layering note (batch 6 T1): `world_hash` is defined in headless.rs (the
// verification-tooling module, which itself depends on game.rs for `Game`)
// but is needed here too, for the walk-over portal describe-line's "world
// hash" component (see `Game::portal_describe`). Rust's module graph has no
// acyclic requirement within one crate — this is a function reference, not
// a type-layout cycle — so the two-way `use` compiles cleanly. Kept as one
// definition (not duplicated) so "what identifies a world" never drifts
// between the two call sites.
use crate::headless::world_hash;

pub(crate) const COLS: usize = 80;
pub(crate) const ROWS: usize = 30;
pub(crate) const MAP_H: usize = 25; // rows [0,25) map; row 25 status; rows [26,30) log

/// Total depth count for the active cartridge (was the engine's own
/// `MAX_DEPTH` constant before the cartridge split — see `WinDef::max_depth`).
pub(crate) fn max_depth() -> u32 {
    GAME.win.max_depth
}

/// Run-wide light pool for the active cartridge, solver-derived — see the
/// active cartridge's own comment on the derivation. `pub(crate)`: render.rs
/// reads this to compute the Torch bar's fill proportion (light /
/// start_light) in the status bar.
pub(crate) fn start_light() -> i32 {
    GAME.balance.start_light
}

/// Monster kind: a plain index into `GAME.monsters`, not a named variant —
/// the engine has no notion of what kind 0 "is," only that it's a row in
/// the active cartridge's monster table. See `Monster::stats`.
pub(crate) type MKind = u8;
/// Item kind: a plain index into `GAME.items`, same convention as `MKind`.
pub(crate) type IKind = u8;

/// Player FOV radius shrinks as the torch burns down. Percent of
/// `start_light()`. Tiers come from `BalanceDef::fov_tiers`, checked in
/// order — see that field's doc comment.
pub(crate) fn fov_radius(light: i32) -> i32 {
    let pct = light * 100 / GAME.balance.start_light;
    for &(threshold, radius) in GAME.balance.fov_tiers {
        if pct > threshold {
            return radius;
        }
    }
    // Unreachable given a well-formed `fov_tiers` table (its last entry's
    // threshold should always match — see the field's doc comment); kept
    // as a safe fallback rather than a panic.
    2
}

// ---------- Map ----------
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Tile {
    Wall,
    Floor,
    Stairs,   // '>' down
    UpStairs, // '<' up; on the return depth it is the way out (win tile, objective in hand)
    /// A door to another world (batch 6 T1, dump/render glyph `*`). Passable
    /// floor for movement/LOS/monster-pathing purposes (`Game::los`'s wall
    /// check, `wall_mask`'s autotile mask, and `monsters_act`'s neighbor
    /// step all key off `Tile::Wall` specifically, so `Portal` falls through
    /// to "not a wall" everywhere for free). Walking ONTO one just logs its
    /// destination (`Game::land_on_tile`'s `Tile::Portal` arm) — it does
    /// NOT transit. Transit is `wait_turn` while STANDING on one (see that
    /// method's doc comment): a deliberate reuse of the existing wait byte
    /// rather than a new input byte, so the input vocabulary stays
    /// unchanged and neither sim bot (which never emits wait) can ever
    /// transit by accident. Destination is cached in `Game::portal`/
    /// `LevelState::portal` at generation time (`Game::gen_level`'s portal-
    /// placement pass) rather than re-derived on demand — see that field's
    /// doc comment for why.
    Portal,
    /// A pit (batch 6 T2, sokoban, dump/render glyph `^`): vault-only this
    /// batch (placed solely by `Game::stamp_vault` via the `^` legend char —
    /// see `GAME.vaults`). Impassable to the player: `try_move_player`
    /// refuses the step with a themed message and no turn, mirroring a wall
    /// bump. Impassable to monsters too: `monsters_act`'s neighbor-step
    /// check excludes it exactly like `Tile::Wall`. LOS passes over it
    /// (waist-deep, not overhead — `los` only ever blocks on `Tile::Wall`,
    /// so this needs no change there). `bfs_dist` treats it as blocking
    /// too: it's the first tile kind that can strand a genuinely-
    /// `Tile::Floor` pocket behind an impassable gap (a push-block bridges
    /// it — see `Game::try_push`), so routing primitives (the solver, the
    /// sim bots' loot sweep) must know the difference, or they'd treat an
    /// unbridged pocket as reachable and either misjudge difficulty or, for
    /// the sim bots, walk straight into a refused-move retry loop. A pushed
    /// block that lands on a pit is destroyed and the pit FILLS: the tile
    /// becomes `Tile::Floor` (the sokoban bridge), so `Pit` is never a
    /// terminal tile kind, just a temporary gate.
    Pit,
    /// A goal tile (batch 6 T2, sokoban, dump/render glyph `x`): a walkable
    /// floor variant (vault-only, placed via the `x` legend char) —
    /// identical to `Tile::Floor` for every game-logic purpose (movement,
    /// LOS, monster pathing, `rand_floor`'s exact-`Tile::Floor` spawn check
    /// deliberately excludes it, same as it already excludes `Pit`/
    /// `Portal`) except: it renders distinctly (`content::PAL_GOAL`), and a
    /// push-chain's farthest member landing here LOCKS instead of merely
    /// advancing. Chosen mechanism (simplest that makes an authorable
    /// puzzle work): the locked block is removed from `Game::blocks`
    /// (absorbed, never pushable again) and this tile becomes ordinary
    /// `Tile::Floor` — see `Game::try_push`.
    Goal,
}

/// Push-chain cap (batch 6 T2, sokoban — ported from golem/topdown-puzzle's
/// `shared/push.js`, `MAX_PUSH_CHAIN`, itself citing KyeScene.js's `if
/// (chain.length > 2) return null`). A 3rd consecutive block in the push
/// direction refuses the WHOLE push (`Game::try_push`'s chain-walk), no
/// turn spent — topdown's "tooLong".
pub(crate) const MAX_PUSH_CHAIN: usize = 2;

/// Outcome of `Game::resolve_push`'s read-only chain-walk (batch 6 T2
/// review fix): `TooLong`/`Blocked` are the two denial cases (see
/// `Game::try_push`'s doc comment for what each means), `Ok` carries the
/// resolved chain (nearest-first) and landing cell for a caller that
/// intends to actually mutate state.
enum PushResolution {
    TooLong,
    Blocked,
    Ok(Vec<(i32, i32)>, (i32, i32)),
}

/// Identifies a world: either a derived dungeon keyed by its own seed, or a
/// hand-authored singular place keyed by its index into
/// `GAME.authored_floors` (batch 6 T1). `Game::world` is the CURRENT
/// world; the root world (the one a fresh `Game::new` starts in, and the
/// only one with a win condition/objective — see `Game::land_on_tile`'s
/// `Tile::UpStairs` arm) is always `Seed(g.seed)`, computed by comparison
/// rather than a separate stored flag, so it can never drift out of sync
/// with `g.seed`. `PartialEq`/`Eq` back the linear scans over `Game::worlds`
/// (fine at this scale — see that field's doc comment); `Copy` because every
/// use is a small value compared/stored by value, never mutated in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldId {
    Seed(u64),
    Floor(u8),
}

/// A portal's destination, rolled once at generation time and cached (see
/// `Tile::Portal`'s doc comment). `World`'s first `u64` is a derived seed
/// (`h64(world_seed, ["portal", depth_tag, index_tag])` — frozen API, see
/// `Game::gen_level`'s portal-placement comment), never `Game::seed`
/// itself. `World`'s second `u64` is `headless::world_hash(seed)`,
/// memoized at the SAME generation site (perf fix, batch 6 review): it's a
/// pure function of the first field, so computing it once here instead of
/// on every walk-onto (`portal_describe` used to call `world_hash` fresh
/// each time — generating all 5 depths of the destination world per step)
/// is free to do and changes no observable output. Being pure-derived from
/// an already-hashed field, it's deliberately EXCLUDED from `state_hash`
/// (see `save::hash_portal`) rather than hashed again. `Floor`'s `u8`
/// indexes `GAME.authored_floors`.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Dest {
    World(u64, u64),
    Floor(u8),
}

/// Snapshot of a world OTHER than the currently active one (batch 6 T1):
/// its own per-depth `saved` stack (same shape as `Game::saved`, which is
/// always the CURRENT world's — see that field's doc comment), the depth
/// the player was at when they left it, and its own provenance (`from`,
/// mirroring `Game::from` — a world reached via a portal chain remembers
/// where IT came from too, so climbing back out the far end of a multi-hop
/// chain unwinds correctly). `Game::worlds` holds one of these per visited
/// non-current world, insertion-ordered by first-LEFT (not first-seen) —
/// see `Game::leave_current_world`'s doc comment.
pub(crate) struct WorldState {
    pub(crate) saved: Vec<Option<LevelState>>,
    pub(crate) depth: u32,
    pub(crate) from: Option<(WorldId, i32, i32)>,
}

#[derive(Clone)]
pub(crate) struct Monster {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) kind: MKind,
    pub(crate) hp: i32,
    /// Talks received so far (batch 5, DECISION.md item 3 — the Henson
    /// ruling: mercy is a verb and the verb is TALK). Counts toward
    /// `Monster::talk_threshold(kind)`; naturally capped there —
    /// `Game::try_talk_player`'s already-calm branch returns before ever
    /// touching this field again. Hashed in `save::state_hash` (mercy is
    /// run-defining state, unlike the presentation-only exclusion set
    /// documented at `state_hash`).
    pub(crate) regard: u8,
    /// Becalmed (batch 5): set true on the talk that crosses
    /// `Monster::talk_threshold`. A calm monster never attacks and never
    /// chases — `Game::monsters_act` skips it outright, every turn,
    /// forever after (the simplest deterministic becalmed behavior: it
    /// stands). Bumping it swaps positions instead of attacking (see
    /// `Game::try_move_player`). Hashed, same rationale as `regard`.
    pub(crate) calm: bool,
}

impl Monster {
    /// This kind's complete definition (stats/glyph/color/talk data),
    /// looked up by index in the active cartridge's monster table. No
    /// engine code names a specific kind — only the cartridge's own data
    /// module does.
    pub(crate) fn stats(kind: MKind) -> &'static crate::gamedef::MonsterDef {
        &GAME.monsters[kind as usize]
    }

    /// Number of talks (batch 5) a monster must receive before it becomes
    /// calm — per-kind, from the cartridge's own table (tuned against the
    /// pacifist gate, `tests/pacifist-band.json`, never retuned by feel
    /// alone).
    pub(crate) fn talk_threshold(kind: MKind) -> u8 {
        GAME.monsters[kind as usize].talk_threshold
    }
}

/// Parley receptivity (batch 5 addendum, human direction: "needs to be
/// algorithm'd" — replaces the old flat counter with a guaranteed stayed
/// swing per talk, which the pacifist-dominance finding in
/// `tests/pacifist-band.json` traced its root cause to). A 0-99 roll
/// against this percentage (`Game::try_talk_player`, via `parley_rng`)
/// decides whether a single talk LANDS (advances `regard`, stays the
/// monster this turn) or FAILS (no regard, no stay — the monster acts
/// normally). Pure integer math over state already tracked; every term's
/// provenance:
///   `def.receptivity_base`: the cartridge's own per-kind starting point —
///   small/skittish kinds read as quicker to listen, large/stubborn kinds
///   as slower.
///   `regard_coeff * m.regard`: persistence pays — each PRIOR landed talk
///   (regard is only incremented on a landed roll, see `try_talk_player`)
///   makes the next one likelier, so a monster mid-being-persuaded trends
///   toward calm rather than resetting every attempt.
///   `wound_coeff * (maxhp - m.hp) / maxhp`: wounds open ears — `maxhp` is
///   the monster's OWN kind ceiling, not the player's; integer division
///   truncates toward 0 (a fresh monster contributes exactly 0 here, a
///   monster one hit from death contributes close to the full coefficient).
///   `atk_coeff * (g.atk - starting_atk)`: visible strength impresses —
///   `starting_atk` is the player's atk at `Game::new`, so this term is 0
///   at the run's start and grows with every attack-boosting pickup.
///   `-torch_penalty if fov_radius(g.light) <= torch_radius_threshold`: a
///   guttering torch reads as weakness, not menace.
///   `clamp(lo, hi)`: never impossible, never certain, regardless of how
///   the terms above sum.
pub(crate) fn receptivity(m: &Monster, g: &Game) -> i32 {
    let def = &GAME.monsters[m.kind as usize];
    let maxhp = def.hp;
    let b = &GAME.balance;
    let torch_penalty =
        if fov_radius(g.light) <= b.receptivity_torch_radius_threshold { b.receptivity_torch_penalty } else { 0 };
    let r = def.receptivity_base + b.receptivity_regard_coeff * m.regard as i32
        + b.receptivity_wound_coeff * (maxhp - m.hp) / maxhp
        + b.receptivity_atk_coeff * (g.atk - b.starting_atk)
        - torch_penalty;
    r.clamp(b.receptivity_clamp.0, b.receptivity_clamp.1)
}

#[derive(Clone)]
pub(crate) struct Item {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) kind: IKind,
}

/// Snapshot of a visited depth, so the climb back out with the objective is
/// through the world you left: same layout, taken items stay taken, dead
/// monsters stay dead, live ones where they last stood.
pub(crate) struct LevelState {
    pub(crate) map: Vec<Tile>,
    pub(crate) seen: Vec<bool>,
    pub(crate) monsters: Vec<Monster>,
    pub(crate) items: Vec<Item>,
    pub(crate) rooms: Vec<(i32, i32, i32, i32)>,
    pub(crate) room_meta: Vec<(u8, u8)>, // (kind, tone) indices
    pub(crate) room_visited: Vec<bool>,
    /// This depth's portal, if it generated one (batch 6 T1): position plus
    /// cached destination, snapshotted/restored alongside everything else
    /// on a stash/restore round trip. See `Game::portal`'s doc comment.
    pub(crate) portal: Option<(i32, i32, Dest)>,
    /// This depth's push-blocks (batch 6 T2), snapshotted/restored like
    /// every other per-level field. See `Game::blocks`' doc comment.
    pub(crate) blocks: Vec<(i32, i32)>,
}

pub(crate) struct Game {
    pub(crate) map: Vec<Tile>,
    pub(crate) seen: Vec<bool>,
    pub(crate) vis: Vec<bool>,
    pub(crate) px: i32,
    pub(crate) py: i32,
    pub(crate) hp: i32,
    pub(crate) maxhp: i32,
    pub(crate) atk: i32,
    pub(crate) depth: u32,
    pub(crate) kills: u32,
    /// Monsters becalmed via talk (batch 5, DECISION.md item 3), incremented
    /// once per monster the instant it crosses `Monster::talk_threshold` —
    /// mercy's counterpart to `kills`. Hashed by `save::state_hash` like
    /// every other run-defining `u32` counter.
    pub(crate) spared: u32,
    pub(crate) light: i32,
    /// Player turns taken: incremented once per `spend_turn` call (move
    /// onto floor, an attack swing, or a wait — see spend_turn). Hashed
    /// into state_hash like every other run-defining field.
    pub(crate) turns: u32,
    /// Themed name of the monster that landed the killing blow, set in
    /// `monsters_act` right before `dead = true`. `None` for a darkness
    /// death (light hits 0 in `spend_turn`) or while alive. Presentation-
    /// only (the End screen's cause-of-death line) — deliberately NOT
    /// hashed by state_hash: it doesn't affect anything replay needs to
    /// reproduce, only what's shown after the run is already over.
    pub(crate) killer: Option<&'static str>,
    /// Where the PREVIOUS attempt ended, if this attempt began via a
    /// same-seed RETRY (input byte 6, save v2 — see `save::replay` and
    /// `save::INPUT_RETRY`): `(px, py, depth)` of the death tile, so a
    /// future renderer (Phase 4, not this task) can mark it. `None` unless
    /// this attempt started from byte 6 immediately after a DEAD ending —
    /// a retry from a win or from mid-run leaves it `None` (see
    /// `save::replay`'s byte-6 arm). Presentation-only, exactly like
    /// `killer`: deliberately NOT hashed by `state_hash` (replay doesn't
    /// need it to reproduce anything), NOT printed by `--dump`, and NOT
    /// itself saved — `save_bytes` only ever serializes seed + input log,
    /// and every replay recomputes `echo` fresh from the state the
    /// PRECEDING Game was in the instant before byte 6 fired.
    pub(crate) echo: Option<(i32, i32, u32)>,
    /// Direction the player last SUCCESSFULLY faced: updated in
    /// `try_move_player` on every branch that actually takes an action (a
    /// landed move onto floor, a landed attack swing, or a becalmed-
    /// monster swap) and in `try_talk_player` on a landed talk (whether the
    /// target monster is calm already or not — both are directed
    /// interactions with a monster) — all from the same `(dx, dy)` those
    /// branches already receive as parameters, so deriving it costs no new
    /// RNG draw and no extra worldgen/spawns state. A wall bump, or a talk
    /// at a wall/empty tile, does NOT update it (those branches return
    /// first). Defaults to
    /// `Facing::S` (`Game::new`). `render::scene()` reads this for the
    /// player's `SceneEntity::facing`; monster facing is derived
    /// separately and needs no stored field (see
    /// `Game::monster_sees_player`/`render::scene`'s `monster_facing`).
    /// Presentation-only: NOT hashed by `state_hash`, NOT printed by
    /// `--dump`, NOT itself saved — see `save::state_hash`'s doc comment
    /// for the shared exclusion-list rationale (`killer`/`echo`/`facing`/
    /// `fx_hit` are all in the same boat).
    pub(crate) facing: Facing,
    /// Grid tile of the last melee impact this attempt: set in
    /// `try_move_player` when the player lands a hit (to the target
    /// monster's tile) and in `monsters_act` when a monster hits the
    /// player (to the player's tile) — every attack in this game always
    /// lands (no miss chance), so this is set unconditionally on either
    /// event. Cleared at the very START of the next player action
    /// (`try_move_player`/`wait_turn`'s first statement, before the
    /// dead/won early return), so it reads `Some` for exactly the frames
    /// between the hit and the next input. `backend_minifb`'s screen-feel
    /// (palette flash + vertical squash) reads this; the term backend
    /// deliberately doesn't (see the note near `backend_term::frame_bytes`).
    /// Presentation-only, same exclusion list as `facing` above.
    pub(crate) fx_hit: Option<(i32, i32)>,
    /// Whether the player currently holds the run's win-condition item
    /// (`ItemEffect::Objective` — see `Game::pickup`). Doubles the per-turn
    /// light burn (`WinDef::carry_burn`, it is heavy) and is the second of
    /// two conditions (with standing on the return depth's up-stairs) that
    /// wins the run.
    pub(crate) has_objective: bool,
    pub(crate) monsters: Vec<Monster>,
    pub(crate) items: Vec<Item>,
    pub(crate) rooms: Vec<(i32, i32, i32, i32)>,
    pub(crate) room_meta: Vec<(u8, u8)>,
    pub(crate) room_visited: Vec<bool>,
    /// The CURRENT world's per-depth stash (batch 6 T1: was unconditionally
    /// "the run's" stash before portals existed — now scoped to whichever
    /// world `Game::world` names). Length `max_depth()` for a `Seed` world,
    /// length 1 for a `Floor` world (always index 0, since `Game::depth` is
    /// pinned to 1 there). Swapped out wholesale on every world transition
    /// (`Game::leave_current_world`/`enter_world_forward`/
    /// `enter_world_return`) the same way `map`/`monsters`/`items` are.
    pub(crate) saved: Vec<Option<LevelState>>,
    /// Every visited world OTHER than the current one (batch 6 T1),
    /// insertion-ordered by first-LEFT (see `Game::leave_current_world`).
    /// The current world's own state lives in the live fields above
    /// (`map`/`saved`/`depth`/`from`), never duplicated here — hashing and
    /// transition code both key off `Game::world` to know which side of
    /// that split a given world is on right now (see `save::state_hash`'s
    /// batch-6 addition and `Game::leave_current_world`/`enter_world_*`).
    /// Linear scan (`Vec`, not a map) is fine at this scale — a run visits
    /// at most a handful of worlds before the light runs out.
    pub(crate) worlds: Vec<(WorldId, WorldState)>,
    /// Where the CURRENT world was entered FROM (batch 6 T1): the source
    /// world plus the exact portal tile there, so walking onto THIS world's
    /// entrance-depth (or a Floor's sole) `<` can transit back to precisely
    /// that tile (`Game::return_to_source`) rather than searching for one.
    /// `None` for the root world (never entered via a portal) and, always,
    /// immediately after `Game::new` — see `WorldId`'s doc comment for why
    /// "root" is a comparison (`world == WorldId::Seed(seed)`) rather than a
    /// field this could get out of sync with.
    pub(crate) from: Option<(WorldId, i32, i32)>,
    /// The CURRENT level's portal, if `Game::gen_level`'s portal-placement
    /// pass rolled one (batch 6 T1) — position plus cached destination.
    /// Cached rather than re-derived on demand: re-deriving would mean
    /// re-running worldgen for this (world seed, depth) up to the placement
    /// point on every lookup, for no benefit over storing the ~9 bytes this
    /// costs; `LevelState::portal` snapshots/restores it exactly like every
    /// other per-level field, and it's hashed in `save::state_hash` (stored
    /// state that changes what a walk-onto/wait does is run-defining).
    /// `None` on a `Floor` world (authored floors place no further portals
    /// this batch — a deliberate scope cut, not a technical limit; see
    /// `AuthoredFloorDef`'s doc comment).
    pub(crate) portal: Option<(i32, i32, Dest)>,
    /// This depth's push-blocks (batch 6 T2, sokoban — ported in spirit from
    /// golem/topdown-puzzle's `shared/push.js`), vault-only this batch
    /// (placed solely by a vault's `B` legend char — see `GAME.vaults` and
    /// `Game::stamp_vault`). Occupy a tile like a monster (block
    /// player/monster movement — `Game::try_push`, `Game::monsters_act`'s
    /// neighbor-step check), but LOS passes over them (waist-high — `los`
    /// never consults this vector) and an item may sit underneath (an item
    /// and a block coexisting at the same `(x, y)` is normal, not a bug:
    /// the item is simply hidden until the block moves off, at which point
    /// walking onto that now-vacated tile picks it up via the ordinary
    /// `pickup` path — no special-casing needed). Push-chains cap at
    /// `MAX_PUSH_CHAIN`; a chain member pushed into a `Tile::Pit` is
    /// destroyed and the pit fills (`Tile::Floor`); a chain member pushed
    /// onto a `Tile::Goal` LOCKS (removed from this vector, the tile
    /// becomes ordinary `Tile::Floor` — see `Game::try_push`). Hashed by
    /// `save::state_hash` (position is run-defining, same rationale as
    /// monster/item position; also "the blocks persist per seed — the next
    /// player on this world finds your fossilized bad idea," per
    /// `docs/story/STORY-COMPILE-v1.md` §6.3).
    pub(crate) blocks: Vec<(i32, i32)>,
    /// Which world is currently active (batch 6 T1). `Seed(seed)` for a
    /// derived dungeon, `Floor(i)` for an authored singular place. See
    /// `WorldId`'s doc comment for the root-world convention.
    pub(crate) world: WorldId,
    pub(crate) msgs: Vec<String>,
    pub(crate) seed: u64,
    pub(crate) combat_rng: Rng,
    pub(crate) ai_rng: Rng,
    pub(crate) flavor_rng: Rng,
    /// Parley rolls (batch 5 addendum, human direction: "needs to be
    /// algorithm'd"): its own per-run channel (`channel(seed, &["parley"])`),
    /// same isolation discipline as `combat_rng`/`ai_rng`/`flavor_rng` — a
    /// parley draw must never perturb worldgen/spawns, nor any other
    /// channel (see `parley_isolated_from_combat` in main.rs). Hashed in
    /// `save::state_hash` alongside the other three run-stream RNG states.
    pub(crate) parley_rng: Rng,
    pub(crate) dead: bool,
    pub(crate) won: bool,
}

pub(crate) fn idx(x: i32, y: i32) -> usize {
    y as usize * COLS + x as usize
}
pub(crate) fn in_map(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < COLS as i32 && y < MAP_H as i32
}

impl Game {
    pub(crate) fn new(seed: u64) -> Self {
        let mut g = Game {
            map: vec![Tile::Wall; COLS * MAP_H],
            seen: vec![false; COLS * MAP_H],
            vis: vec![false; COLS * MAP_H],
            px: 0,
            py: 0,
            hp: GAME.balance.starting_hp,
            maxhp: GAME.balance.starting_hp,
            atk: GAME.balance.starting_atk,
            depth: 1,
            kills: 0,
            spared: 0,
            light: GAME.balance.start_light,
            turns: 0,
            killer: None,
            echo: None,
            facing: Facing::S,
            fx_hit: None,
            has_objective: false,
            monsters: Vec::new(),
            items: Vec::new(),
            rooms: Vec::new(),
            room_meta: Vec::new(),
            room_visited: Vec::new(),
            saved: (0..GAME.win.max_depth).map(|_| None).collect(),
            worlds: Vec::new(),
            from: None,
            portal: None,
            blocks: Vec::new(),
            world: WorldId::Seed(seed),
            msgs: Vec::new(),
            seed,
            combat_rng: channel(seed, &["combat"]),
            ai_rng: channel(seed, &["ai"]),
            flavor_rng: channel(seed, &["flavor"]),
            parley_rng: channel(seed, &["parley"]),
            dead: false,
            won: false,
        };
        g.gen_level();
        g.log(String::from(GAME.strings.intro));
        g
    }

    /// The seed that channel draws (`worldgen`/`spawns`/`vault`/`tone`,
    /// plus theme/lore lookups) should use for the CURRENT world (batch 6
    /// T1). A `Seed` world uses its own seed — a portal world is literally
    /// `Game`-worldgen under a different seed, so `gen_level` needs no
    /// branching of its own, just this one substitution everywhere it used
    /// to say `self.seed`. A `Floor` world has no worldgen (const ASCII,
    /// zero RNG — see `Game::instantiate_floor`) but still wants a theme
    /// for INCIDENTAL flavor (torch-tier warnings, potion/sword adjectives
    /// via `Game::adj`) since those fire regardless of which world you're
    /// in; rather than invent a per-floor theme table, it borrows the root
    /// world's depth-1 theme (`self.seed` at depth 1, which `self.depth`
    /// already IS on a floor — see `Game::depth`'s floor convention) — a
    /// floor's own identity comes from `GAME.authored_floors`' name/
    /// describe line instead, never from this borrowed theme's label.
    fn world_seed(&self) -> u64 {
        match self.world {
            WorldId::Seed(s) => s,
            WorldId::Floor(_) => self.seed,
        }
    }

    pub(crate) fn theme(&self) -> &'static crate::gamedef::ThemeDef {
        theme_for(self.world_seed(), self.depth)
    }

    /// The filled lore line for a tier of the current depth's theme.
    fn lore_line(&self, tier: usize) -> String {
        crate::content::lore_line(self.world_seed(), self.depth, tier)
    }

    fn mob_name(&self, k: MKind) -> &'static str {
        self.theme().mobs[k as usize]
    }

    fn adj(&mut self) -> &'static str {
        let t = self.theme();
        t.adjs[self.flavor_rng.range(0, t.adjs.len() as i32) as usize]
    }

    pub(crate) fn log(&mut self, s: String) {
        self.msgs.push(s);
        if self.msgs.len() > 40 {
            self.msgs.remove(0);
        }
    }

    /* ── WORLDGEN: pure f(seed, depth) via the "worldgen"/"spawns" channels.
       Output is frozen by the golden fixtures in tests/golden/: any diff to
       a golden is a seed-breaking MAJOR change requiring explicit human
       sign-off — never a drive-by. ──────────────────────────────────────── */
    pub(crate) fn gen_level(&mut self) {
        self.map = vec![Tile::Wall; COLS * MAP_H];
        self.seen = vec![false; COLS * MAP_H];
        self.vis = vec![false; COLS * MAP_H];
        self.monsters.clear();
        self.items.clear();
        self.portal = None;
        self.blocks.clear();

        let depth_tag = self.depth.to_string();
        let world_seed = self.world_seed();
        let mut wr = channel(world_seed, &["worldgen", &depth_tag]);
        let mut sr = channel(world_seed, &["spawns", &depth_tag]);

        // rooms
        let mut rooms: Vec<(i32, i32, i32, i32)> = Vec::new(); // x,y,w,h
        for _ in 0..80 {
            if rooms.len() >= 10 {
                break;
            }
            let w = wr.range(5, 12);
            let h = wr.range(4, 8);
            let x = wr.range(1, COLS as i32 - w - 1);
            let y = wr.range(1, MAP_H as i32 - h - 1);
            let clash = rooms.iter().any(|&(rx, ry, rw, rh)| {
                x < rx + rw + 1 && rx < x + w + 1 && y < ry + rh + 1 && ry < y + h + 1
            });
            if clash {
                continue;
            }
            for cy in y..y + h {
                for cx in x..x + w {
                    self.map[idx(cx, cy)] = Tile::Floor;
                }
            }
            rooms.push((x, y, w, h));
        }

        // occasionally stamp one hand-authored vault as an extra room; the
        // corridor pass below connects its center like any other room
        let mut vault_room: Option<usize> = None;
        let mut vr = channel(world_seed, &["vault", &depth_tag]);
        if vr.chance(2, 5) {
            let vi = vr.range(0, GAME.vaults.len() as i32) as usize;
            let rows: Vec<&str> = GAME.vaults[vi].lines().collect();
            let (vw, vh) = (rows[0].len() as i32, rows.len() as i32);
            for _ in 0..40 {
                let x = vr.range(1, COLS as i32 - vw - 1);
                let y = vr.range(1, MAP_H as i32 - vh - 1);
                let clash = rooms.iter().any(|&(rx, ry, rw, rh)| {
                    x < rx + rw + 1 && rx < x + vw + 1 && y < ry + rh + 1 && ry < y + vh + 1
                });
                if clash {
                    continue;
                }
                self.stamp_vault(GAME.vaults[vi], x, y);
                vault_room = Some(rooms.len());
                rooms.push((x, y, vw, vh));
                break;
            }
        }
        // corridors between consecutive room centers (L-shaped)
        let centers: Vec<(i32, i32)> =
            rooms.iter().map(|&(x, y, w, h)| (x + w / 2, y + h / 2)).collect();
        for i in 1..centers.len() {
            let (ax, ay) = centers[i - 1];
            let (bx, by) = centers[i];
            let (mut cx, mut cy) = (ax, ay);
            let horiz_first = wr.chance(1, 2);
            while (cx, cy) != (bx, by) {
                if (horiz_first && cx != bx) || cy == by {
                    cx += (bx - cx).signum();
                } else {
                    cy += (by - cy).signum();
                }
                // A corridor's straight carve punches through anything in
                // its path unconditionally, same as it always has ("the
                // carver breaks in" — ordinary vault doctrine). batch 6 T2
                // review: an earlier version of this carve tried to
                // special-case `Tile::Pit`/`Tile::Goal` (leave them
                // un-overwritten, to stop a corridor from silently
                // "solving" a sokoban gate for free) — reverted: on a rare
                // seed the straight-line carve's OWN target cell can BE
                // that protected tile (two consecutive room centers whose
                // straight path is blocked at the exact cell a vault
                // happened to place its pit/goal on), and refusing to ever
                // paint Floor there could sever that corridor's own
                // connectivity, which is worse (see `--solve`'s
                // reachability proof — this must never regress). A corridor
                // silently trivializing a pit "for free" is accepted, same
                // spirit as walls; what must NOT happen is the exit itself
                // depending on that gate — see the `deepest`-selection
                // exclusion below for how that's actually prevented.
                self.map[idx(cx, cy)] = Tile::Floor;
            }
        }

        let (sx, sy) = centers[0];
        self.px = sx;
        self.py = sy;
        // BFS depth from the entrance is the level's act structure: the exit
        // (down-stairs, or the objective item on the last depth) goes in
        // the DEEPEST reachable room, not the last-generated one — EXCEPT a
        // sokoban vault's own center (batch 6 T2 review fix): if this depth
        // stamped one (`vault_room` is `Some` AND it actually placed
        // blocks — some vaults have neither, so this is a no-op for them),
        // its center is excluded from the candidate pool outright, so the
        // exit can never land inside a sokoban room. Confirmed necessary
        // empirically, not just in theory: an unrelated corridor can (and,
        // on rare seeds, does) trivialize a vault's own pit — "the carver
        // breaks in" is accepted for a pit same as a wall (see the
        // corridor-carve comment above) — after which its blocks are just
        // ordinary movable furniture in an open corridor. A `--sim` greedy
        // bot never deliberately solves puzzles, but its wander-when-stuck
        // fallback (headless.rs) WILL push a block it stumbles into if that
        // push succeeds, and a block pushed hard up against a
        // `Tile::Stairs` tile can never be pushed the rest of the way
        // (landing on Stairs always refuses — `Game::try_push`) — a dead
        // end for a bot that only ever routes, never backtracks. Keeping
        // the exit out of sokoban rooms entirely sidesteps the whole
        // failure class rather than chasing every way a corridor could
        // produce it.
        let dist = bfs_dist(&self.map, (sx, sy));
        let sokoban_vault_center =
            vault_room.filter(|_| !self.blocks.is_empty()).map(|vi| centers[vi]);
        let deepest = centers
            .iter()
            .filter(|&&c| c != (sx, sy))
            .filter(|&&c| Some(c) != sokoban_vault_center)
            .max_by_key(|&&(cx, cy)| dist[idx(cx, cy)])
            .copied()
            .unwrap_or((sx + 1, sy));
        self.map[idx(sx, sy)] = Tile::UpStairs;
        let is_root = self.world == WorldId::Seed(self.seed);
        if self.depth < GAME.win.max_depth {
            self.map[idx(deepest.0, deepest.1)] = Tile::Stairs;
        } else if is_root {
            self.items.push(Item { x: deepest.0, y: deepest.1, kind: GAME.win.objective_item });
        } else {
            /* batch 6 T1: the win-condition item and the win condition both
               live only in the root world (see `Game::land_on_tile`'s
               `UpStairs` arm) — a portal Seed-world's last depth has no
               objective item. Its deepest room still rewards the trip: a
               Potion where the objective would have sat, plus a deep-tier
               lore item placed via the SAME `sr` (spawns) draw every other
               item on this depth uses — existing item machinery, no new
               mechanics. `rand_floor` may occasionally fail to find a
               second free tile in a cramped room; skipping the lore item
               in that rare case is harmless, the Potion alone still marks
               this depth's reward. */
            self.items.push(Item { x: deepest.0, y: deepest.1, kind: GAME.balance.loot_potion_item });
            if let Some((lx, ly)) = self.rand_floor(&mut sr, 4) {
                self.items.push(Item { x: lx, y: ly, kind: GAME.balance.lore_items[2] });
            }
        }

        /* portal (batch 6 T1): chance(1,4) per depth, drawn from the SAME
           `wr` worldgen channel used for every layout draw above (rooms,
           vault placement, corridors) — inserted strictly AFTER all of
           those, so this batch's golden diff was purely additive: existing
           `wr` draws are byte-identical up to this point, this is a tail.
           Room choice excludes the entrance room (index 0 — see
           `centers[0]`) and the exit room (whichever room's center is
           `deepest`, holding Stairs/the objective item/the consolation
           Potion) so a portal never doubles as, or blocks routing to,
           either. Position is a channel-drawn floor tile inside that room,
           retried against collision with anything already placed there (a
           vault-stamped monster/item, or the entrance/exit tile itself).
           Destination kind is rolled ONLY once a valid tile is actually
           found (not inside the retry loop, so retries don't waste draws
           on a roll that might be discarded): chance(1,3) an authored
           Floor, else a derived World whose seed is `h64(world_seed,
           ["portal", depth_tag, index_tag])` — frozen API (like `h64`'s own
           doc comment says of the primitive itself): the whole multiverse
           is a pure function of the root seed. `index_tag` is always "0"
           this batch (at most one portal per depth) — reserved so a future
           "more than one portal per depth" feature can extend the tag
           scheme without colliding with this batch's derivations. */
        if wr.chance(1, 4) {
            let exit_idx = centers.iter().position(|&c| c == deepest);
            let candidates: Vec<usize> =
                (1..rooms.len()).filter(|&i| Some(i) != exit_idx).collect();
            if !candidates.is_empty() {
                let ri = candidates[wr.range(0, candidates.len() as i32) as usize];
                let (rx, ry, rw, rh) = rooms[ri];
                for _ in 0..40 {
                    let px = wr.range(rx, rx + rw);
                    let py = wr.range(ry, ry + rh);
                    if self.map[idx(px, py)] == Tile::Floor
                        && (px, py) != (sx, sy)
                        && (px, py) != deepest
                        && !self.items.iter().any(|it| (it.x, it.y) == (px, py))
                        && !self.monsters.iter().any(|m| (m.x, m.y) == (px, py))
                        && !self.blocks.iter().any(|&b| b == (px, py))
                    {
                        let dest = if wr.chance(1, 3) {
                            Dest::Floor(wr.range(0, GAME.authored_floors.len() as i32) as u8)
                        } else {
                            // world_hash memoized here, once, at generation
                            // time (perf fix, batch 6 review) — see
                            // `Dest::World`'s doc comment.
                            let dseed = h64(world_seed, &["portal", &depth_tag, "0"]);
                            Dest::World(dseed, world_hash(dseed))
                        };
                        self.map[idx(px, py)] = Tile::Portal;
                        self.portal = Some((px, py, dest));
                        break;
                    }
                }
            }
        }

        /* monsters: scale with depth, spawn on floor away from player.
           Sim-derived (batch 3 balance pass, gated by tests/sim-band.json):
           count `spawn_base_count + depth` and roll `d10 + depth` against
           the cartridge's own `monster_roll` threshold table (see
           `BalanceDef::monster_roll`'s doc comment). Tune only against
           `--sim 5000` landing in the band. */
        let count = GAME.balance.spawn_base_count + self.depth as i32;
        for _ in 0..count {
            if let Some((mx, my)) = self.rand_floor(&mut sr, 8) {
                let roll = sr.range(0, 10) + self.depth as i32;
                let kind = GAME
                    .balance
                    .monster_roll
                    .iter()
                    .find(|&&(threshold, _)| roll < threshold)
                    .map(|&(_, k)| k)
                    .unwrap_or(GAME.balance.monster_roll[GAME.balance.monster_roll.len() - 1].1);
                let hp = GAME.monsters[kind as usize].hp;
                self.monsters.push(Monster { x: mx, y: my, kind, hp, regard: 0, calm: false });
            }
        }
        /* items: deep floors are a war of attrition, so supply scales too —
           part of the same sim-gated balance pass as the spawn table. */
        let b = &GAME.balance;
        for _ in 0..sr.range(b.loot_count_lo, b.loot_count_hi) + (self.depth as i32 - 1) * b.loot_count_per_depth {
            if let Some((ix, iy)) = self.rand_floor(&mut sr, 4) {
                let kind =
                    if sr.chance(b.loot_potion_num, b.loot_potion_den) { b.loot_potion_item } else { b.loot_sword_item };
                self.items.push(Item { x: ix, y: iy, kind });
            }
        }
        // story buried by depth: three inscriptions at shallow / mid / deep
        // rooms (by BFS distance from the entrance), read on walk-over.
        // Placement is a pure function of `dist` — zero extra RNG draws.
        let mut by_depth = centers.clone();
        by_depth.sort_by_key(|&(cx, cy)| dist[idx(cx, cy)]);
        let n = by_depth.len();
        let picks = [by_depth[1.min(n - 1)], by_depth[n / 2], by_depth[n.saturating_sub(2)]];
        for (tier, &(cx, cy)) in picks.iter().enumerate() {
            if (cx, cy) == deepest
                || (cx, cy) == (sx, sy)
                || self.map[idx(cx, cy)] != Tile::Floor
                || self.items.iter().any(|it| (it.x, it.y) == (cx, cy))
                || self.monsters.iter().any(|m| (m.x, m.y) == (cx, cy))
            {
                continue;
            }
            let kind = GAME.balance.lore_items[tier];
            self.items.push(Item { x: cx, y: cy, kind });
        }

        // room identity: kind + tone per room from the "tone" channel (its
        // own stream — adds zero draws to worldgen/spawns, so layouts and
        // goldens are untouched). Spawn room counts as already entered.
        // `room_kinds.len() - 1` excludes the reserved LAST kind (vault),
        // never drawn randomly — it's forced onto stamped vault rooms below.
        let kind_roll_max = GAME.room_kinds.len() as i32 - 1;
        let tone_roll_max = GAME.tone_lines.len() as i32;
        let mut tn = channel(world_seed, &["tone", &depth_tag]);
        self.room_meta = rooms
            .iter()
            .map(|_| (tn.range(0, kind_roll_max) as u8, tn.range(0, tone_roll_max) as u8))
            .collect();
        if let Some(vi) = vault_room {
            self.room_meta[vi].0 = (GAME.room_kinds.len() - 1) as u8; // forced "vault"
        }
        self.room_visited = vec![false; rooms.len()];
        self.room_visited[0] = true;
        self.rooms = rooms;

        self.compute_fov();

        // first arrival: name the place; its history is buried in the rooms
        let t = self.theme();
        self.log(GAME.strings.enter_theme.replace("{}", t.label));
    }

    /// Stamp a hand-authored vault's ASCII (a `GAME.vaults`-style legend:
    /// `#` wall — left untouched, `.` floor, `^` pit, `x` goal, `B`
    /// push-block, and every other byte matched against the active
    /// cartridge's item/monster glyphs — see `MonsterDef::glyph`/
    /// `ItemDef::glyph`'s doc comments, a def's glyph doubles as its vault
    /// legend character) into `self.map`/`items`/`monsters`/`blocks` at
    /// world-space origin `(ox, oy)`. Extracted from `gen_level`'s inline
    /// vault-placement loop (batch 6 T2) so it has exactly one
    /// implementation, shared by real worldgen (which calls this once a
    /// collision-free spot is found) and the sokoban solution tests
    /// (main.rs), which stamp a vault directly onto a synthetic map with no
    /// worldgen RNG involved at all — a vault's tested solution can
    /// therefore never silently drift from what actually gets stamped into
    /// a real level.
    pub(crate) fn stamp_vault(&mut self, spec: &str, ox: i32, oy: i32) {
        for (j, row) in spec.lines().enumerate() {
            for (i, c) in row.bytes().enumerate() {
                let (tx, ty) = (ox + i as i32, oy + j as i32);
                match c {
                    b'#' => continue, // already wall
                    b'^' => {
                        self.map[idx(tx, ty)] = Tile::Pit;
                        continue; // a pit never also carries an item/monster
                    }
                    b'x' => self.map[idx(tx, ty)] = Tile::Goal,
                    _ => self.map[idx(tx, ty)] = Tile::Floor,
                }
                if c == b'B' {
                    self.blocks.push((tx, ty));
                } else if let Some(ii) = GAME.items.iter().position(|it| it.glyph == c) {
                    self.items.push(Item { x: tx, y: ty, kind: ii as IKind });
                } else if let Some(ki) = GAME.monsters.iter().position(|m| m.glyph == c) {
                    let hp = GAME.monsters[ki].hp;
                    self.monsters.push(Monster { x: tx, y: ty, kind: ki as MKind, hp, regard: 0, calm: false });
                }
            }
        }
    }

    fn rand_floor(&mut self, rng: &mut Rng, min_player_dist: i32) -> Option<(i32, i32)> {
        for _ in 0..200 {
            let x = rng.range(1, COLS as i32 - 1);
            let y = rng.range(1, MAP_H as i32 - 1);
            if self.map[idx(x, y)] == Tile::Floor
                && (x - self.px).abs() + (y - self.py).abs() >= min_player_dist
                && !self.monsters.iter().any(|m| m.x == x && m.y == y)
                && !self.items.iter().any(|i| i.x == x && i.y == y)
                // batch 6 T2: also skip a sokoban block's tile — Tile::Floor
                // stays Floor under a block (it's a separate entity list,
                // not a Tile variant), so without this a random spawn could
                // otherwise land a monster or item directly on top of one.
                && !self.blocks.iter().any(|&b| b == (x, y))
            {
                return Some((x, y));
            }
        }
        None
    }

    // ---------- FOV: raycast to every tile within radius ----------
    fn compute_fov(&mut self) {
        let r = fov_radius(self.light);
        self.vis.iter_mut().for_each(|v| *v = false);
        self.vis[idx(self.px, self.py)] = true;
        self.seen[idx(self.px, self.py)] = true;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (tx, ty) = (self.px + dx, self.py + dy);
                if !in_map(tx, ty) {
                    continue;
                }
                if self.los(self.px, self.py, tx, ty) {
                    self.vis[idx(tx, ty)] = true;
                    self.seen[idx(tx, ty)] = true;
                }
            }
        }
    }

    /// Bresenham line-of-sight; target tile itself may be a wall (so walls are visible).
    fn los(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = (x1 - x0).signum();
        let sy = (y1 - y0).signum();
        let mut err = dx + dy;
        loop {
            if (x, y) == (x1, y1) {
                return true;
            }
            if (x, y) != (x0, y0) && self.map[idx(x, y)] == Tile::Wall {
                return false;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Whether `m` can currently see (and, in `monsters_act`, therefore
    /// chase or attack) the player: within the cartridge's `monster_sight`
    /// (Chebyshev distance) AND unobstructed line of sight. `pub(crate)` so
    /// `render::scene()` can derive monster facing from the SAME predicate
    /// `monsters_act` uses for chase/attack — one definition, not two that
    /// could drift — without storing any new per-monster state.
    pub(crate) fn monster_sees_player(&self, m: &Monster) -> bool {
        let dist = (self.px - m.x).abs().max((self.py - m.y).abs());
        dist <= GAME.balance.monster_sight && self.los(m.x, m.y, self.px, self.py)
    }

    // ---------- Turn logic ----------
    /// Burn the torch for one player turn: `base_burn` light, or
    /// `WinDef::carry_burn` while carrying the objective (it is heavy),
    /// plus `extra` (the violence tax on attack turns; 0 for
    /// movement/waiting). Light 0 is death in the dark — checked once,
    /// after the combined deduction, before any win condition,
    /// golem-style. Returns false if the player died.
    fn spend_turn(&mut self, extra: i32) -> bool {
        self.turns += 1;
        let before = fov_radius(self.light);
        let base = if self.has_objective { GAME.win.carry_burn } else { GAME.balance.base_burn };
        self.light -= base + extra;
        if self.light <= 0 {
            self.light = 0;
            self.dead = true;
            self.log(String::from(GAME.strings.dark_death));
            self.compute_fov();
            return false;
        }
        let after = fov_radius(self.light);
        if after < before {
            // Index by the radius just crossed into: fov_radius only ever
            // steps through the tiers below the starting radius (see
            // `BalanceDef::fov_tiers`), so this covers every reachable
            // `after` value; `_` is unreachable but kept for exhaustiveness
            // rather than a panic if that ever changes.
            let ti = match after {
                6 => 0,
                5 => 1,
                4 => 2,
                3 => 3,
                _ => 4,
            };
            let a = self.adj();
            self.log(GAME.strings.tier_warnings[ti].replace("{}", a));
        }
        true
    }

    /// Snapshot the current depth (of the CURRENT world — see `Game::saved`'s
    /// doc comment) so it persists exactly as left. Indexes by `self.depth`
    /// unconditionally: a `Floor` world's `depth` is pinned to 1 (see
    /// `Game::instantiate_floor`), so this needs no world-kind branch.
    fn stash_level(&mut self) {
        let d = self.depth as usize - 1;
        self.saved[d] = Some(LevelState {
            map: std::mem::take(&mut self.map),
            seen: std::mem::take(&mut self.seen),
            monsters: std::mem::take(&mut self.monsters),
            items: std::mem::take(&mut self.items),
            rooms: std::mem::take(&mut self.rooms),
            room_meta: std::mem::take(&mut self.room_meta),
            room_visited: std::mem::take(&mut self.room_visited),
            portal: self.portal.take(),
            blocks: std::mem::take(&mut self.blocks),
        });
    }

    /// Restore a previously visited depth and place the player at the first
    /// tile of kind `arrive` found scanning the map (row-major). A monster
    /// that wandered onto the arrival tile is shoved aside. Used by
    /// `descend`/`ascend` (arrive on the stairs) and `enter_world_forward`
    /// (arrive on the entrance `<`), where "the stairs tile" is exactly the
    /// right arrival point and doesn't need to be threaded through as a
    /// coordinate. `return_to_source` wants an EXACT coordinate instead
    /// (the portal tile the player originally stepped through, not "the
    /// first `<`") — see `restore_level_at`, which shares
    /// `apply_restored_level` with this method rather than duplicating the
    /// placement/monster-shove/FOV logic.
    fn restore_level(&mut self, ls: LevelState, arrive: Tile) {
        let pos = (0..COLS as i32 * MAP_H as i32)
            .map(|i| (i % COLS as i32, i / COLS as i32))
            .find(|&(x, y)| ls.map[idx(x, y)] == arrive)
            .unwrap_or((self.px, self.py));
        self.apply_restored_level(ls, pos);
    }

    /// Restore a previously visited depth and place the player at the exact
    /// `(x, y)` given, rather than searching for a tile kind — used by
    /// `return_to_source`, which must land the player back ON the specific
    /// portal tile it left through (see that method's doc comment for why
    /// standing there is safe by construction).
    fn restore_level_at(&mut self, ls: LevelState, x: i32, y: i32) {
        self.apply_restored_level(ls, (x, y));
    }

    /// Shared tail for `restore_level`/`restore_level_at`: install the
    /// snapshot's fields, place the player at `pos`, shove aside any
    /// monster that wandered onto it, and refresh FOV.
    fn apply_restored_level(&mut self, ls: LevelState, pos: (i32, i32)) {
        self.map = ls.map;
        self.seen = ls.seen;
        self.monsters = ls.monsters;
        self.items = ls.items;
        self.rooms = ls.rooms;
        self.room_meta = ls.room_meta;
        self.room_visited = ls.room_visited;
        self.portal = ls.portal;
        self.blocks = ls.blocks;
        self.vis = vec![false; COLS * MAP_H];
        self.px = pos.0;
        self.py = pos.1;
        if let Some(mi) = self.monsters.iter().position(|m| (m.x, m.y) == pos) {
            let (mx, my) = (self.monsters[mi].x, self.monsters[mi].y);
            let spot = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().find_map(|&(dx, dy)| {
                let (tx, ty) = (mx + dx, my + dy);
                // batch 6 T2: a shoved monster must not land on a pit or a
                // block's tile either — same "not a legal step" set
                // monsters_act's own neighbor check uses.
                let free = in_map(tx, ty)
                    && self.map[idx(tx, ty)] != Tile::Wall
                    && self.map[idx(tx, ty)] != Tile::Pit
                    && !self.monsters.iter().any(|m| (m.x, m.y) == (tx, ty))
                    && !self.blocks.iter().any(|&b| b == (tx, ty));
                if free { Some((tx, ty)) } else { None }
            });
            match spot {
                Some((tx, ty)) => {
                    self.monsters[mi].x = tx;
                    self.monsters[mi].y = ty;
                }
                None => {
                    self.monsters.remove(mi);
                }
            }
        }
        self.compute_fov();
    }

    /// Leave the current world for storage in `Game::worlds` (batch 6 T1):
    /// stash its live level, then write its `WorldState` into the vector —
    /// updating an existing slot in place if this world was visited (and
    /// left) before, else appending a new one. This is what makes
    /// `Game::worlds`' order "insertion by first-LEFT": a world's slot,
    /// once created, never moves — a later revisit-then-leave overwrites
    /// the SAME slot rather than reordering it, which is what keeps
    /// `save::state_hash`'s iteration order stable across a replay that
    /// re-enters a world it already left once.
    fn leave_current_world(&mut self) {
        self.stash_level();
        let ws = WorldState {
            saved: std::mem::take(&mut self.saved),
            depth: self.depth,
            from: self.from,
        };
        match self.worlds.iter().position(|(id, _)| *id == self.world) {
            Some(i) => self.worlds[i].1 = ws,
            None => self.worlds.push((self.world, ws)),
        }
    }

    /// Take a world's stored `WorldState` out of `Game::worlds` by id,
    /// leaving a cheap placeholder behind (never read while that world
    /// isn't current — see `Game::worlds`' doc comment: hashing and every
    /// other reader key off `Game::world` to know which side of the
    /// current/stored split a world is on, so a stale placeholder sitting
    /// in a slot between visits is harmless noise, not a correctness risk).
    fn take_world_state(&mut self, wid: WorldId) -> Option<WorldState> {
        let i = self.worlds.iter().position(|(id, _)| *id == wid)?;
        let placeholder = WorldState { saved: Vec::new(), depth: 0, from: None };
        Some(std::mem::replace(&mut self.worlds[i].1, placeholder))
    }

    /// Forward transit into `wid` (batch 6 T1): a portal step
    /// (`Game::transit`), always arriving at that world's entrance — depth
    /// 1 for a `Seed` world, the single level for a `Floor` — regardless of
    /// whatever depth the player happened to be at on a previous visit
    /// (contrast `enter_world_return`, which resumes exactly where a world
    /// was left). If `wid` was visited before, its depth-1 (or sole) stash
    /// is restored as-is (monsters/items exactly as last left); otherwise
    /// it's generated/instantiated fresh. `from` becomes this world's
    /// return provenance, unconditionally overwriting whatever was stored
    /// before: stepping through the SAME door twice always returns via the
    /// door you just used, grounded (you step back out where you came in).
    fn enter_world_forward(&mut self, wid: WorldId, from: (WorldId, i32, i32)) {
        self.world = wid;
        self.from = Some(from);
        self.depth = 1;
        match self.take_world_state(wid) {
            Some(ws) => {
                self.saved = ws.saved;
                match self.saved[0].take() {
                    Some(ls) => self.restore_level(ls, Tile::UpStairs),
                    None => self.regen_current_world(wid), // defensive; should not happen
                }
            }
            None => {
                self.saved = match wid {
                    WorldId::Seed(_) => (0..GAME.win.max_depth as usize).map(|_| None).collect(),
                    WorldId::Floor(_) => vec![None],
                };
                self.regen_current_world(wid);
            }
        }
    }

    /// Return transit into `wid` (batch 6 T1, `Game::return_to_source`):
    /// resumes `wid` EXACTLY as it was left — the depth it was left at, and
    /// the level restored at that depth — placing the player at the exact
    /// `(x, y)` of the portal tile they originally stepped through, not at
    /// a searched-for tile kind. Panics if `wid` has no stored `WorldState`
    /// or that depth was never stashed: both are unreachable given
    /// `return_to_source` only ever calls this with a `from` provenance
    /// that was itself set by a prior `enter_world_forward`/this same
    /// method, which always leaves a matching stash behind.
    fn enter_world_return(&mut self, wid: WorldId, x: i32, y: i32) {
        self.world = wid;
        let ws = self
            .take_world_state(wid)
            .expect("return_to_source: provenance world must have a stored WorldState");
        self.depth = ws.depth;
        self.saved = ws.saved;
        self.from = ws.from;
        let d = self.depth as usize - 1;
        let ls = self.saved[d]
            .take()
            .expect("return_to_source: stashed level must exist for the depth just left");
        self.restore_level_at(ls, x, y);
    }

    /// Generate/instantiate `wid` fresh: `gen_level` for a derived `Seed`
    /// world (reused unchanged — see `Game::world_seed`), `instantiate_floor`
    /// for an authored `Floor`.
    fn regen_current_world(&mut self, wid: WorldId) {
        match wid {
            WorldId::Seed(_) => self.gen_level(),
            WorldId::Floor(i) => self.instantiate_floor(i),
        }
    }

    /// Instantiate an authored floor (batch 6 T1): parse
    /// `GAME.authored_floors[i]`'s const ASCII map directly into
    /// `map`/`items`/`monsters` — zero RNG draws, unlike `gen_level`. A
    /// smaller-than-80x25 map is centered and wall-padded. `rooms` gets one
    /// entry covering the whole authored rect, pre-marked visited, so
    /// `note_room_entry` has somewhere harmless to no-op against (a floor
    /// has no room-tone system — it's one authored space, not a generated
    /// sequence of rooms). Authored floors place no further portals this
    /// batch (see `Game::portal`'s doc comment) and carry no lore items
    /// (see `AuthoredFloorDef`'s doc comment).
    fn instantiate_floor(&mut self, i: u8) {
        self.map = vec![Tile::Wall; COLS * MAP_H];
        self.seen = vec![false; COLS * MAP_H];
        self.vis = vec![false; COLS * MAP_H];
        self.monsters.clear();
        self.items.clear();
        self.portal = None;
        self.blocks.clear(); // authored floors place no sokoban blocks (batch 6 T2 scope)

        let spec = &GAME.authored_floors[i as usize];
        let rows: Vec<&str> = spec.map.lines().collect();
        let (fw, fh) = (rows[0].len() as i32, rows.len() as i32);
        let ox = (COLS as i32 - fw) / 2;
        let oy = (MAP_H as i32 - fh) / 2;
        let mut start = (ox, oy);
        for (j, row) in rows.iter().enumerate() {
            for (col, c) in row.bytes().enumerate() {
                let (tx, ty) = (ox + col as i32, oy + j as i32);
                match c {
                    b'#' => self.map[idx(tx, ty)] = Tile::Wall,
                    b'.' => self.map[idx(tx, ty)] = Tile::Floor,
                    b'<' => {
                        self.map[idx(tx, ty)] = Tile::UpStairs;
                        start = (tx, ty);
                    }
                    _ => {
                        if let Some(ii) = GAME.items.iter().position(|it| it.glyph == c) {
                            self.map[idx(tx, ty)] = Tile::Floor;
                            self.items.push(Item { x: tx, y: ty, kind: ii as IKind });
                        } else if let Some(ki) = GAME.monsters.iter().position(|m| m.glyph == c) {
                            self.map[idx(tx, ty)] = Tile::Floor;
                            let hp = GAME.monsters[ki].hp;
                            self.monsters.push(Monster { x: tx, y: ty, kind: ki as MKind, hp, regard: 0, calm: false });
                        }
                        // else: well-formedness (main.rs) guards the legal-
                        // char set; an unrecognized byte leaves the default
                        // Wall tile untouched.
                    }
                }
            }
        }
        self.px = start.0;
        self.py = start.1;
        self.rooms = vec![(ox, oy, fw, fh)];
        self.room_meta = vec![(0, 0)];
        self.room_visited = vec![true];
        self.compute_fov();
        self.log(GAME.strings.floor_arrive.replace("{}", spec.name));
        self.log(String::from(spec.describe));
    }

    /// Portal transit out of the current world (batch 6 T1,
    /// `Game::wait_turn`): stash+store the current world, then enter the
    /// destination — always at its entrance (`enter_world_forward`), never
    /// resuming mid-depth even on a revisit. Logs its own arrival line
    /// unconditionally (mirroring `descend`/`ascend`'s own
    /// "You descend/climb..." line), on top of whatever `gen_level`/
    /// `instantiate_floor` logs on a FRESH generation — two stacked log
    /// lines on a first visit is the established pattern here, not a bug to
    /// dedupe.
    fn transit(&mut self) {
        let Some((px, py, dest)) = self.portal else {
            return; // defensive: only called while standing on a Portal tile
        };
        let src = self.world;
        self.leave_current_world();
        match dest {
            Dest::World(seed, _) => self.enter_world_forward(WorldId::Seed(seed), (src, px, py)),
            Dest::Floor(i) => self.enter_world_forward(WorldId::Floor(i), (src, px, py)),
        }
        self.log(GAME.strings.portal_arrive.replace("{}", &self.arrival_label(dest)));
    }

    /// Return to the world/tile a portal was entered from (batch 6 T1,
    /// `Game::land_on_tile`'s `UpStairs` arm on a non-root world's entrance
    /// depth, or a `Floor`'s sole `<`): walk-on, like stairs — leaving is
    /// easy. Lands the player standing exactly ON the source portal tile.
    /// This does NOT immediately re-transit even though standing on a
    /// portal tile is normally the trigger condition: transit only fires
    /// from `wait_turn` on an explicit wait input, never as a side effect
    /// of landing on a tile — so arriving here via a move is safe by
    /// construction, no re-entrancy guard needed.
    fn return_to_source(&mut self) {
        let Some((src, x, y)) = self.from else {
            return; // defensive: every non-root world sets `from` on entry
        };
        self.leave_current_world();
        self.enter_world_return(src, x, y);
        self.log(String::from(GAME.strings.portal_return));
    }

    /// The label used for both the walk-over describe-line
    /// (`portal_describe`) and the arrival line (`transit`): a `Seed`
    /// dest's theme label plus its world hash, a `Floor` dest's authored
    /// name.
    fn arrival_label(&self, dest: Dest) -> String {
        match dest {
            Dest::World(seed, _) => theme_for(seed, 1).label.to_string(),
            Dest::Floor(i) => GAME.authored_floors[i as usize].name.to_string(),
        }
    }

    /// Walk-over-a-portal describe line (batch 6 T1, `Game::land_on_tile`'s
    /// `Tile::Portal` arm): grounded — both halves (theme label, world
    /// hash) are DERIVED from the destination seed via existing machinery
    /// (`content::theme_for`, `headless::world_hash`), the engine proving
    /// what's on the other side by generating it, never inventing it. Logs
    /// once per step-on; repeating on a later step-on is fine (same
    /// convention as every other walk-over message in this file).
    ///
    /// Perf (batch 6 review): the world hash used to be recomputed here via
    /// a fresh `world_hash(seed)` call on EVERY walk-onto — regenerating
    /// all 5 depths of the destination world each time a player stepped
    /// onto the portal tile. It's now read from `Dest::World`'s memoized
    /// second field (computed once, at portal-generation time — see that
    /// enum's doc comment); output is unchanged since it's the same pure
    /// function of the same seed, just computed once instead of on every
    /// step-on.
    fn portal_describe(&self, dest: Dest) -> String {
        match dest {
            Dest::World(seed, whash) => GAME
                .strings
                .portal_describe_world
                .replacen("{}", theme_for(seed, 1).label, 1)
                .replacen("{}", &format!("{:016x}", whash), 1),
            Dest::Floor(i) => GAME.strings.portal_describe_floor.replace("{}", GAME.authored_floors[i as usize].name),
        }
    }

    pub(crate) fn descend(&mut self) {
        self.stash_level();
        self.depth += 1;
        let d = self.depth as usize - 1;
        match self.saved[d].take() {
            Some(ls) => self.restore_level(ls, Tile::UpStairs),
            None => {
                self.gen_level();
                /* Sim-derived (batch 3 balance pass): each FIRST descent
                   grants a max-HP/heal bump — hp <= old maxhp, so
                   hp+gain <= new maxhp always; no clamp needed. Without
                   this progression the greedy bot dies to combat 100% of
                   the time (--sim 5000, batch 2); with it plus the
                   softened spawn tables the win rate sits inside the band
                   in tests/sim-band.json. Retune via `--sim 5000`. */
                self.maxhp += GAME.balance.hp_gain_per_depth;
                self.hp += GAME.balance.hp_gain_per_depth;
                self.log(String::from(GAME.strings.descend_first));
            }
        }
        self.log(GAME.strings.descend.replace("{}", &self.depth.to_string()));
    }

    pub(crate) fn ascend(&mut self) {
        self.stash_level();
        self.depth -= 1;
        let d = self.depth as usize - 1;
        match self.saved[d].take() {
            Some(ls) => self.restore_level(ls, Tile::Stairs),
            None => self.gen_level(), // unreachable in play; belt and braces
        }
        self.log(GAME.strings.ascend.replace("{}", &self.depth.to_string()));
    }

    /// First step into a room surfaces its tone line, once per level visit.
    fn note_room_entry(&mut self) {
        let (px, py) = (self.px, self.py);
        let ri = self.rooms.iter().position(|&(rx, ry, rw, rh)| {
            px >= rx && px < rx + rw && py >= ry && py < ry + rh
        });
        if let Some(ri) = ri {
            if !self.room_visited[ri] {
                self.room_visited[ri] = true;
                let (k, t) = self.room_meta[ri];
                let line = GAME.tone_lines[t as usize][self.flavor_rng.range(0, 2) as usize]
                    .replace("{K}", GAME.room_kinds[k as usize]);
                self.log(line);
            }
        }
    }

    pub(crate) fn try_move_player(&mut self, dx: i32, dy: i32) {
        // Screen-feel state (batch 4 task 3): cleared at the START of every
        // player action, before the dead/won early return, so a stale
        // flash/squash never survives past the input that should have
        // cleared it — see `Game::fx_hit`'s doc comment.
        self.fx_hit = None;
        if self.dead || self.won {
            return;
        }
        let (nx, ny) = (self.px + dx, self.py + dy);
        if !in_map(nx, ny) {
            return;
        }
        if let Some(mi) = self.monsters.iter().position(|m| m.x == nx && m.y == ny) {
            self.facing = Facing::from_delta(dx, dy);
            if self.monsters[mi].calm {
                // Mercy's second economy lever (batch 5, DECISION.md item
                // 3): bumping a becalmed monster SWAPS positions instead
                // of attacking — it yields. Costs a turn like any move, no
                // violence tax, no damage. Only `calm == true` swaps; a
                // monster mid-being-persuaded (regard > 0 but not yet
                // calm) still gets attacked below, unchanged.
                let (ox, oy) = (self.px, self.py);
                self.monsters[mi].x = ox;
                self.monsters[mi].y = oy;
                self.px = nx;
                self.py = ny;
                self.land_on_tile(nx, ny, None);
                return;
            }
            let dmg = self.atk + self.combat_rng.range(0, 3);
            let name = self.mob_name(self.monsters[mi].kind);
            self.monsters[mi].hp -= dmg;
            self.fx_hit = Some((nx, ny));
            if self.monsters[mi].hp <= 0 {
                self.monsters.remove(mi);
                self.kills += 1;
                self.log(GAME.strings.slay.replacen("{}", name, 1).replacen("{}", &dmg.to_string(), 1));
            } else {
                self.log(GAME.strings.hit.replacen("{}", name, 1).replacen("{}", &dmg.to_string(), 1));
            }
        } else if self.blocks.contains(&(nx, ny)) {
            // Sokoban (batch 6 T2): walking into a block attempts to push
            // it — see `Game::try_push` for the chain-walk/denial/fill/lock
            // semantics ported from golem/topdown-puzzle's push.js. On
            // success the PLAYER also advances into the now-vacated (nx,
            // ny), same turn, matching topdown's own resolveMove (the
            // block(s) move, then the player's own MOVED event); on a
            // refusal nothing moves and no turn passes, same as a wall
            // bump — `try_push` has already logged the denial.
            self.facing = Facing::from_delta(dx, dy);
            if !self.try_push(nx, ny, dx, dy) {
                return;
            }
            self.px = nx;
            self.py = ny;
            self.land_on_tile(nx, ny, None);
            return;
        } else if self.map[idx(nx, ny)] == Tile::Pit {
            // Sokoban (batch 6 T2): a pit refuses the player exactly like a
            // wall bump — no turn, grounded message, no mutation.
            self.log(String::from(GAME.strings.pit_refuse));
            return;
        } else if self.map[idx(nx, ny)] != Tile::Wall {
            self.facing = Facing::from_delta(dx, dy);
            self.px = nx;
            self.py = ny;
            self.land_on_tile(nx, ny, None);
            return;
        } else {
            return; // bumped a wall: no turn passes
        }
        // attack path: the swing costs a turn too, plus the violence tax
        // (see `BalanceDef::violence_tax`) — folded into one deduction
        // inside spend_turn so the light-0 death check still runs exactly
        // once.
        if !self.spend_turn(GAME.balance.violence_tax) {
            return;
        }
        self.monsters_act(None);
        self.compute_fov();
    }

    /// Attempt to push the block chain starting at `(bx, by)` in direction
    /// `(dx, dy)` — batch 6 T2, sokoban, ported in spirit from golem/
    /// topdown-puzzle's `shared/push.js` (`getPushChain`/`resolveMove`):
    /// chain-walk collects up to `MAX_PUSH_CHAIN` consecutive blocks,
    /// nearest first; the landing cell (one tile past the farthest member)
    /// decides the outcome:
    ///   - out of bounds, `Tile::Wall`, `Tile::Stairs`/`Tile::UpStairs`/
    ///     `Tile::Portal`, or occupied by a monster: the WHOLE push is
    ///     refused — logs a denial, mutates nothing, costs no turn (same as
    ///     `Tile::Pit`'s player-refusal, same as a wall bump).
    ///   - a 3rd consecutive block (chain too long): refused the same way,
    ///     before landing is ever computed — topdown's "tooLong".
    ///   - `Tile::Pit`: the FARTHEST member is destroyed and the pit FILLS
    ///     (`Tile::Floor`); a 2-chain's nearer member survives, advancing
    ///     into the farthest's old slot.
    ///   - `Tile::Floor`: every chain member advances one tile, farthest
    ///     first (mirrors topdown's per-member MOVED ordering).
    ///   - `Tile::Goal`: the farthest member LOCKS there instead of merely
    ///     advancing — removed from `self.blocks` for good, tile becomes
    ///     `Tile::Floor` (see `Tile::Goal`'s doc comment for why that's the
    ///     whole mechanism, no separate reveal step).
    /// Read-only chain-walk + landing-legality check (batch 6 T2 review
    /// fix), factored out of `try_push` so it has exactly one
    /// implementation, shared by two callers: `try_push` itself (which
    /// mutates `self.map`/`self.blocks` once this resolves `Ok`) and
    /// `Game::would_push_succeed` (a pure yes/no predicate, no mutation —
    /// see that method's doc comment for why it exists). Never mutates
    /// `self`.
    fn resolve_push(&self, bx: i32, by: i32, dx: i32, dy: i32) -> PushResolution {
        let mut chain: Vec<(i32, i32)> = Vec::new();
        let (mut cx, mut cy) = (bx, by);
        while self.blocks.contains(&(cx, cy)) {
            chain.push((cx, cy));
            if chain.len() > MAX_PUSH_CHAIN {
                return PushResolution::TooLong;
            }
            cx += dx;
            cy += dy;
        }
        let landing = (cx, cy);
        let blocked = !in_map(landing.0, landing.1)
            || matches!(
                self.map[idx(landing.0, landing.1)],
                Tile::Wall | Tile::Stairs | Tile::UpStairs | Tile::Portal
            )
            || self.monsters.iter().any(|m| (m.x, m.y) == landing);
        if blocked {
            return PushResolution::Blocked;
        }
        PushResolution::Ok(chain, landing)
    }

    /// Would pushing the block chain at `(bx, by)` in direction `(dx, dy)`
    /// succeed RIGHT NOW, without actually doing it? (Batch 6 T2 review
    /// fix.) `headless::sim_seed`'s routing uses this — never
    /// `resolve_push`/`PushResolution` directly, both `pub(crate)` only for
    /// this one cross-module caller — to decide, direction-by-direction,
    /// whether stepping toward a block is a live option this turn: pushing
    /// into `Tile::Pit`/`Tile::Goal`/`Tile::Floor` always succeeds (state
    /// changes — fill, lock, or advance — so even if the SAME direction
    /// gets tried again next turn, the block has moved and the situation
    /// is different), but pushing into a wall/the stairs/a monster/a
    /// too-long chain always refuses (state does NOT change, so blindly
    /// retrying the identical decision forever is the actual sim-bot
    /// deadlock this batch's review caught — see `game::bfs_dist`'s and
    /// `headless::routing_map`'s doc comments for the two-fix history that
    /// led here). Treating EVERY block as flatly impassable for routing
    /// (an earlier version of this fix) was safe but overly conservative —
    /// it also refused pushes that would have succeeded harmlessly, which
    /// cost the sim bot real, winnable routes for no reason; checking the
    /// actual outcome is both correct and precise.
    pub(crate) fn would_push_succeed(&self, bx: i32, by: i32, dx: i32, dy: i32) -> bool {
        matches!(self.resolve_push(bx, by, dx, dy), PushResolution::Ok(..))
    }

    /// Returns `true` on any of the three successful outcomes (caller still
    /// needs to move the PLAYER into `(bx, by)` itself — not this method's
    /// concern, mirroring how topdown keeps push resolution and the
    /// player's own MOVED event as separate steps) and `false` on a
    /// refusal. Draws no RNG — a push's outcome is pure geometry.
    fn try_push(&mut self, bx: i32, by: i32, dx: i32, dy: i32) -> bool {
        let resolved = match self.resolve_push(bx, by, dx, dy) {
            PushResolution::TooLong => {
                self.log(String::from(GAME.strings.push_too_long));
                return false;
            }
            PushResolution::Blocked => {
                self.log(String::from(GAME.strings.push_blocked));
                return false;
            }
            PushResolution::Ok(chain, landing) => (chain, landing),
        };
        let (chain, landing) = resolved;
        let into_pit = self.map[idx(landing.0, landing.1)] == Tile::Pit;
        let onto_goal = self.map[idx(landing.0, landing.1)] == Tile::Goal;
        let farthest = *chain.last().unwrap();
        self.blocks.retain(|b| !chain.contains(b));
        if into_pit {
            self.map[idx(landing.0, landing.1)] = Tile::Floor; // the bridge fills
            if chain.len() == 2 {
                self.blocks.push(farthest); // the nearer member survives, advancing one slot
            }
            self.log(String::from(GAME.strings.push_pit_fill));
        } else {
            // Floor/Goal landing: every member but the nearest advances to
            // the position the member ahead of it just vacated (chain[i]'s
            // new position, for i in 1..len, is exactly chain[i]'s OLD
            // value — see the chain-walk above: chain[i] == chain[i-1] +
            // (dx, dy) by construction). The nearest member's slot (bx, by)
            // is deliberately left out: the player is about to occupy it.
            for &p in &chain[1..] {
                self.blocks.push(p);
            }
            if onto_goal {
                self.map[idx(landing.0, landing.1)] = Tile::Floor; // locked: absorbed for good
                self.log(String::from(GAME.strings.push_goal_lock));
            } else {
                self.blocks.push(landing);
                self.log(String::from(GAME.strings.push_ok));
            }
        }
        true
    }

    /// Talk: the mercy verb (batch 5, DECISION.md item 3 — the Henson
    /// ruling: mercy is a verb and the verb is TALK; addendum, human
    /// direction: the flat guaranteed-stay counter is replaced by a
    /// `receptivity`-rolled parley). Input bytes 7-10 (`apply_input`) map to
    /// N/S/W/E, mirroring the move bytes' 0-3 direction order exactly. A
    /// talk at a wall or empty tile is a no-op, no turn — same as a wall
    /// bump. A talk at a live, non-calm monster ALWAYS costs a normal turn
    /// (`spend_turn(0)`: no violence tax, talk is not violence — landed or
    /// failed, the turn is spent either way) and rolls
    /// `parley_rng.range(0, 100) < receptivity(&monster, self)` (its own
    /// named channel — see `Game::parley_rng`'s doc comment — never
    /// `combat_rng`):
    ///   - LANDED: `regard` advances by one, keyed by the monster's own
    ///     `talk_lines` stage 0/1/2 (first talk / a later one still below
    ///     `Monster::talk_threshold` / the one that crosses it — crossing
    ///     sets `calm` and counts `self.spared`, exactly as `kills` counts
    ///     a kill), and the monster does not get to attack THIS turn (it is
    ///     listening) — passed to `monsters_act` as a plain function
    ///     parameter (`stayed`), not a stored field: it exists only for the
    ///     one `monsters_act` call this method makes and is gone the
    ///     instant that call returns, so it can never leak into a later
    ///     turn or into `state_hash` (unlike `regard`/`calm`, which ARE
    ///     hashed — see `save::state_hash`).
    ///   - FAILED: `regard` is UNCHANGED (a failed talk carries risk, per
    ///     the addendum — persistence only pays when it lands), the
    ///     monster is NOT stayed (`monsters_act` receives `None` for it and
    ///     it acts normally, meaning it may attack this same turn if
    ///     adjacent and seeing the player), and a distinct stage-3 "unmoved"
    ///     line logs instead.
    /// Talking to an already-calm monster logs one more stage-2 line (same
    /// flavor_rng-picked variety, no roll — a calmed monster's answer is
    /// settled) but costs no turn; `regard` is naturally capped since this
    /// branch returns before ever touching it again.
    pub(crate) fn try_talk_player(&mut self, dx: i32, dy: i32) {
        self.fx_hit = None;
        if self.dead || self.won {
            return;
        }
        let (nx, ny) = (self.px + dx, self.py + dy);
        if !in_map(nx, ny) {
            return;
        }
        let Some(mi) = self.monsters.iter().position(|m| m.x == nx && m.y == ny) else {
            return; // talk at a wall/empty tile: no-op, no turn
        };
        self.facing = Facing::from_delta(dx, dy);
        let kind = self.monsters[mi].kind;
        let name = self.mob_name(kind);
        if self.monsters[mi].calm {
            let v = self.flavor_rng.range(0, 2) as usize;
            let line = GAME.monsters[kind as usize].talk_lines[2][v].replace("{M}", name);
            self.log(line);
            return; // no turn cost change; regard stays capped
        }
        let chance = receptivity(&self.monsters[mi], self);
        let landed = self.parley_rng.range(0, 100) < chance;
        let stayed = if landed {
            let threshold = Monster::talk_threshold(kind);
            let before = self.monsters[mi].regard;
            self.monsters[mi].regard = before.saturating_add(1);
            let regard = self.monsters[mi].regard;
            let became_calm = regard >= threshold;
            let stage = if became_calm {
                2
            } else if before == 0 {
                0
            } else {
                1
            };
            if became_calm {
                self.monsters[mi].calm = true;
                self.spared += 1;
            }
            let v = self.flavor_rng.range(0, 2) as usize;
            let line = GAME.monsters[kind as usize].talk_lines[stage][v].replace("{M}", name);
            self.log(line);
            Some(mi)
        } else {
            // Failed roll (addendum): no regard, no stay — the monster
            // acts normally this turn, whether that's an attack or a move.
            let v = self.flavor_rng.range(0, 2) as usize;
            let line = GAME.monsters[kind as usize].talk_lines[3][v].replace("{M}", name);
            self.log(line);
            None
        };
        if !self.spend_turn(0) {
            return; // died in the dark on a talk turn: lose beats anything else
        }
        self.monsters_act(stayed);
        self.compute_fov();
    }

    /// Waiting (byte 4) is also how a portal transits (batch 6 T1): a
    /// deliberate reuse of the existing wait byte rather than a new input
    /// byte, so the input vocabulary stays unchanged and neither sim bot
    /// (which never emits wait — `--sim`'s greedy/pacifist policies only
    /// ever emit move/talk bytes) can ever transit as a side effect of
    /// routing. Light still burns FIRST via `spend_turn`, exactly like a
    /// plain wait — dying in the dark mid-transit is a loss like any other
    /// turn, lose-before-win doctrine holds regardless of which tile you're
    /// standing on.
    pub(crate) fn wait_turn(&mut self) {
        // Same fx_hit-clearing discipline as try_move_player — see
        // `Game::fx_hit`'s doc comment.
        self.fx_hit = None;
        if self.dead || self.won {
            return;
        }
        let transiting = self.map[idx(self.px, self.py)] == Tile::Portal;
        if !self.spend_turn(0) {
            return;
        }
        if transiting {
            self.transit();
            return; // arriving world: monsters don't get a free hit, same courtesy as stairs
        }
        self.monsters_act(None);
        self.compute_fov();
    }

    /// Shared tail for any player action that LANDS the player on
    /// `(nx, ny)` — a normal move onto floor, or a becalmed-monster swap
    /// (batch 5) — both of which spend a turn (no tax), fire room-entry/
    /// pickup/stairs-transition handling, then resume monster turns and
    /// refresh FOV. `stayed` is forwarded to `monsters_act` untouched (see
    /// its doc comment); both call sites here pass `None` since neither a
    /// move nor a swap is a talk.
    fn land_on_tile(&mut self, nx: i32, ny: i32, stayed: Option<usize>) {
        if !self.spend_turn(0) {
            return; // died in the dark: lose beats anything this tile offered
        }
        self.note_room_entry();
        self.pickup();
        match self.map[idx(nx, ny)] {
            Tile::Stairs => {
                self.descend();
                return; // fresh level: monsters don't get a free hit
            }
            Tile::UpStairs => {
                if self.depth > GAME.win.return_depth {
                    self.ascend();
                    return; // same courtesy on arrival upstairs
                }
                // batch 6 T1: the entrance depth (or a Floor's sole level,
                // always depth 1 — see `Game::instantiate_floor`) of a
                // NON-root world is a return portal, not a win check —
                // walking onto it transits back to where this world was
                // entered from (`return_to_source`). The root world's own
                // entrance `<` keeps its original win/refusal semantics
                // untouched.
                if self.world != WorldId::Seed(self.seed) {
                    self.return_to_source();
                    return;
                }
                if self.has_objective {
                    self.won = true;
                    self.log(String::from(GAME.strings.win));
                    return;
                }
                self.log(String::from(GAME.strings.need_objective));
            }
            Tile::Portal => {
                // batch 6 T1: walking ONTO a portal never transits — only
                // `wait_turn` (while standing on one) does. This just logs
                // its destination; see `Game::portal_describe`.
                if let Some((_, _, dest)) = self.portal {
                    self.log(self.portal_describe(dest));
                }
            }
            _ => {}
        }
        self.monsters_act(stayed);
        self.compute_fov();
    }

    fn pickup(&mut self) {
        if let Some(i) = self.items.iter().position(|i| i.x == self.px && i.y == self.py) {
            let kind = self.items[i].kind;
            self.items.remove(i);
            match GAME.items[kind as usize].effect {
                ItemEffect::Heal(amount) => {
                    let heal = amount.min(self.maxhp - self.hp);
                    self.hp += heal;
                    let a = self.adj();
                    self.log(GAME.strings.heal.replacen("{}", a, 1).replacen("{}", &heal.to_string(), 1));
                }
                ItemEffect::AtkBonus(n) => {
                    self.atk += n;
                    let a = self.adj();
                    self.log(GAME.strings.atk_item.replacen("{}", a, 1).replacen("{}", &n.to_string(), 1));
                }
                ItemEffect::Objective => {
                    self.has_objective = true;
                    let name = self.theme().objective_name;
                    self.log(GAME.strings.pickup_objective.replace("{}", name));
                }
                ItemEffect::Lore(tier) => {
                    let line = self.lore_line(tier as usize);
                    self.log(String::from(GAME.strings.lore_prefix));
                    self.log(line);
                }
            }
        }
    }

    /// `stayed`: the index (into `self.monsters` at the moment of the
    /// call) of a monster that received a talk THIS turn and so does not
    /// get to attack this pass — it is listening, though it may still
    /// move (see `Game::try_talk_player`'s doc comment on why this is a
    /// plain parameter and not a stored field). `None` from every call
    /// site except `try_talk_player`'s. Separate from, and secondary to,
    /// `calm`: a becalmed monster is skipped outright below regardless of
    /// `stayed` — `stayed` only matters for a monster mid-being-persuaded
    /// (regard > 0, not yet calm).
    fn monsters_act(&mut self, stayed: Option<usize>) {
        let (px, py) = (self.px, self.py);
        let mut attacks: Vec<(MKind, i32)> = Vec::new();
        for i in 0..self.monsters.len() {
            if self.monsters[i].calm {
                // Becalmed (batch 5): never attacks, never chases — the
                // simplest deterministic option per the batch-5 plan's
                // Design (decided) section is to stand. Skipped every
                // turn, forever, once calm.
                continue;
            }
            let (mx, my) = (self.monsters[i].x, self.monsters[i].y);
            let dist = (px - mx).abs().max((py - my).abs());
            let sees = self.monster_sees_player(&self.monsters[i]);
            if dist == 1 && sees {
                if stayed == Some(i) {
                    // Stayed swing (batch 5): listening this turn, no
                    // attack — falls through to the movement code below
                    // instead (it may still move; other monsters act
                    // normally, so a crowd stays dangerous).
                } else {
                    let atk = Monster::stats(self.monsters[i].kind).atk;
                    let dmg = atk + self.combat_rng.range(0, 2);
                    attacks.push((self.monsters[i].kind, dmg));
                    continue;
                }
            }
            let (dx, dy) = if sees {
                ((px - mx).signum(), (py - my).signum())
            } else if self.ai_rng.chance(1, 3) {
                (self.ai_rng.range(-1, 2), self.ai_rng.range(-1, 2))
            } else {
                (0, 0)
            };
            // try diagonal step, then each axis alone
            for (tx, ty) in [(mx + dx, my + dy), (mx + dx, my), (mx, my + dy)] {
                if (tx, ty) == (mx, my) {
                    continue;
                }
                // batch 6 T2: monsters never path onto a pit, and never
                // wander onto a push-block's tile either (a block occupies
                // space like a monster does) — same exclusion set as the
                // player-facing checks in `try_move_player`/`try_push`.
                if in_map(tx, ty)
                    && self.map[idx(tx, ty)] != Tile::Wall
                    && self.map[idx(tx, ty)] != Tile::Pit
                    && (tx, ty) != (px, py)
                    && !self.monsters.iter().any(|m| m.x == tx && m.y == ty)
                    && !self.blocks.iter().any(|&b| b == (tx, ty))
                {
                    self.monsters[i].x = tx;
                    self.monsters[i].y = ty;
                    break;
                }
            }
        }
        for (kind, dmg) in attacks {
            self.hp -= dmg;
            self.fx_hit = Some((self.px, self.py));
            let name = self.mob_name(kind);
            if self.hp <= 0 {
                self.hp = 0;
                self.dead = true;
                self.killer = Some(name);
                self.log(GAME.strings.killed_by.replace("{}", name));
                return;
            }
            self.log(GAME.strings.hit_by.replacen("{}", name, 1).replacen("{}", &dmg.to_string(), 1));
        }
    }
}

impl Game {
    /// Input-byte vocabulary, 0-10 (save v3, batch 5): 0-4 move/wait (see
    /// below), 5-6 are frontend/reconstruction-layer only (restart/retry —
    /// handled in `save::replay`, never reach here), 7-10 = talk-N/S/W/E,
    /// direction order mirroring the move bytes exactly. Any other byte is
    /// silently ignored (`_ => {}`) — this is the one place old logs (no
    /// bytes 7-10) and this build's own bytes 5-6 both fall through
    /// harmlessly.
    pub(crate) fn apply_input(&mut self, b: u8) {
        match b {
            0 => self.try_move_player(0, -1),
            1 => self.try_move_player(0, 1),
            2 => self.try_move_player(-1, 0),
            3 => self.try_move_player(1, 0),
            4 => self.wait_turn(),
            7 => self.try_talk_player(0, -1),
            8 => self.try_talk_player(0, 1),
            9 => self.try_talk_player(-1, 0),
            10 => self.try_talk_player(1, 0),
            _ => {}
        }
    }
}

/// BFS distances (4-dir) from `from` over a level map. Blocked by
/// `Tile::Wall` and `Tile::Pit` (batch 6 T2: a pit is impassable to the
/// player exactly like a wall is, see `Tile::Pit`'s doc comment) — this is
/// the TRUE physical-floor connectivity, deliberately NOT aware of
/// `Game::blocks` (batch 6 T2 review: an earlier version threaded a
/// `blocks` parameter through here and had every caller pass it, on the
/// reasoning that a block should read as impassable for routing purposes
/// too — reverted, because it isn't safe at this layer). `Game::gen_level`
/// and `headless::solve_seed` both need the map's REAL connectivity
/// (whether a corridor genuinely reaches its target), and in this
/// engine's straight-line-chain worldgen every corridor edge is a graph
/// BRIDGE (room0-room1-...-roomN is a path, not a mesh) — an UNRELATED
/// pair of rooms' corridor can, by sheer placement chance, cut straight
/// through a sokoban vault's interior row (nothing stops it; "the carver
/// breaks in" is the accepted convention for every vault, sokoban or not).
/// If a `Game::blocks` position happens to land on exactly that cell,
/// treating it as bfs-impassable HERE would sever the corridor and, with
/// it, the ENTIRE rest of the level from the entrance — turning a
/// perfectly winnable seed into a false `--solve` failure. (Confirmed by
/// generating seed 4430: every room except the entrance's own read as
/// unreachable the moment blocks were excluded here — the corridor
/// connecting the entrance room to everything else happened to run
/// straight through a vault's row.) The sokoban stuck-prevention this was
/// meant to buy lives instead in `headless::sim_seed`, which builds its
/// OWN blocks-augmented map for its two `bfs_dist` calls (see
/// `headless::routing_map`) — scoped to the bot's routing decisions only,
/// never to the solver's or worldgen's reachability proofs.
pub(crate) fn bfs_dist(map: &[Tile], from: (i32, i32)) -> Vec<i32> {
    let mut dist = vec![-1i32; COLS * MAP_H];
    let mut q = std::collections::VecDeque::new();
    dist[idx(from.0, from.1)] = 0;
    q.push_back(from);
    while let Some((cx, cy)) = q.pop_front() {
        for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
            let (nx, ny) = (cx + dx, cy + dy);
            // `in_map` must short-circuit FIRST: `idx(nx, ny)` is only a
            // valid array index once bounds are known good (the map's
            // border is always Wall in every generation path, so this
            // never actually fires today, but the check order should not
            // rely on that invariant to stay safe).
            if !in_map(nx, ny) {
                continue;
            }
            let passable = !matches!(map[idx(nx, ny)], Tile::Wall | Tile::Pit);
            if passable && dist[idx(nx, ny)] < 0 {
                dist[idx(nx, ny)] = dist[idx(cx, cy)] + 1;
                q.push_back((nx, ny));
            }
        }
    }
    dist
}
