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
use crate::gamedef::{BumpResponse, CarryEvent, ItemEffect, PickupBehavior, UseEffect};
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

/// The McGuffin's own shine radius, tiered off her mood (batch 12 R5,
/// "light as grace") — mirrors `fov_radius`'s threshold-table convention
/// exactly, just against `GameDef::mood_shine_tiers` instead of
/// `fov_tiers`, and against the raw `0..=100` mood value rather than a
/// percent-of-start_light. `0` is a legitimate, load-bearing result (the
/// dark tier — see that field's doc comment for why it must stay `0`), so
/// the safe fallback below is `0`, not a positive radius like
/// `fov_radius`'s torch-floor `2`.
pub(crate) fn mood_shine_radius(mood: i32) -> i32 {
    for &(threshold, radius) in GAME.balance.mood_shine_tiers {
        if mood > threshold {
            return radius;
        }
    }
    0
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
    /// A deterministic screen-to-screen link (batch 9 T1, story §9-J prep,
    /// dump/render glyph `=`) — distinct from `Tile::Portal`'s randomly-
    /// rolled destination: crossing one is instant walk-onto (like
    /// `Tile::Stairs`, never like `Tile::Portal`'s describe-then-wait), and
    /// its destination is always the OTHER of the overworld's two
    /// neighboring screens, never rolled/cached. The `bool` is the
    /// direction, derived once at parse time (`Game::instantiate_overworld_
    /// screen`) from which edge column the `=` glyph sits on: `true` = an
    /// east-edge link (to `depth + 1`), `false` = a west-edge link (to
    /// `depth - 1`) — see `Game::cross_screen_link`. Walkable, presentation
    /// like `Portal`; never appears in a dungeon map.
    ScreenLink(bool),
    /// The overworld's one true engine-plumbing tile (batch 9 T1, dump/
    /// render glyph `V`): walking onto it transits from `WorldId::Overworld`
    /// into the root dungeon (`WorldId::Seed(self.seed)`) — see
    /// `Game::cross_into_dungeon`. Walkable; instant walk-onto, same
    /// convention as `Tile::Stairs`/`ScreenLink`. Never appears in a
    /// dungeon map.
    Hole,
    /// A shut door (batch 9 T1, story §9-J prep, dump/render glyph `+`):
    /// impassable — `Game::try_move_player` refuses a bump against it with
    /// a themed message (`StringsDef::shut_door_refuse`), no turn, mirroring
    /// `Tile::Pit`'s player-refusal. ALWAYS shut this batch, regardless of
    /// `Game::has_objective` — SPACES-DRAFT-v0 flags this tile as
    /// eventually doubling as the story §3.6 mantel/ending transition once
    /// the objective is held, but that state-dependent branch (and the
    /// scripted final encounter behind it, §9-I) is explicitly deferred;
    /// shipping it dumb now rather than half-building the smart version.
    /// Never appears in a dungeon map.
    ShutDoor,
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
    /// The fixed 3-screen overworld (batch 9 T1, story §9-J prep, SIGN-OFF
    /// ASK #1) — a unit variant since there's only ever one (it isn't
    /// keyed by seed or index the way `Seed`/`Floor` are); `Game::depth`
    /// reused as the current screen 1..=3 is what varies, exactly the way
    /// a `Floor` world already pins `depth` to 1 (see `Game::world_seed`'s
    /// doc comment on the shared "borrow the root seed for incidental
    /// flavor" convention both non-`Seed` variants use).
    Overworld,
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
    /// Awe accumulated by "standing tall" (batch 11 T2 — the diplomat's
    /// ogre answer): built by ending a turn cardinally adjacent to this
    /// monster without bump-attacking it, per `Game::resolve_awe`. Resets
    /// to 0 the instant the player either isn't adjacent or attacked it
    /// this turn (fleeing or fighting breaks the stare). At
    /// `>= Monster::stats(kind).awe_threshold` the monster becalms exactly
    /// like a landed talk (`calm = true`, `Game::record_spare()`) — this is
    /// run-defining mercy state, hashed in `save::state_hash` right beside
    /// `regard`/`calm`, NOT the presentation-only exclusion set
    /// (`killer`/`echo`/`facing`/`fx_hit`/`mcguffin_last_line_turn`).
    pub(crate) awe: u8,
    /// The becalm return-trip dividend's once-per-monster farming guard
    /// (batch 13 T3, arc doc §215): set true the first time the player ends
    /// a turn cardinally adjacent to this monster while `calm`, at which
    /// point `Game::resolve_becalm_dividend` refunds `BalanceDef::
    /// becalm_dividend` light and never refunds this monster again. Hashed
    /// in `save::state_hash` right beside `regard`/`calm`/`awe` — it is
    /// run-defining (it changes future light), not the presentation-only
    /// exclusion set.
    pub(crate) dividend_paid: bool,
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

    /// Freshly spawned `Monster` of `kind` at `(x, y)`: full HP from the
    /// cartridge's own table, zeroed `regard`/`awe`, not `calm` — the one
    /// place every construction site's defaults live, so a new hashed
    /// per-monster field only ever needs a single update here (plus
    /// whatever explicit override a caller wants via struct-update `..`
    /// syntax). Batch 11 T2, added because test fixtures had been hand
    /// duplicating this field list at every call site.
    #[allow(dead_code)] // exercised by tests only as of batch 11 T2
    pub(crate) fn spawn(kind: MKind, x: i32, y: i32) -> Monster {
        Monster { x, y, kind, hp: GAME.monsters[kind as usize].hp, regard: 0, calm: false, awe: 0, dividend_paid: false }
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
    /// batch 8 T1 (story §9-B/C/D): the turn number (`Game::turns`) of the
    /// last McGuffin line actually spoken (`Game::carry_event`), used only
    /// to rate-limit chatter to at most one line per `WinDef::
    /// carry_line_rate_limit` turns. Fully determined by the deterministic
    /// turn/event sequence and gates flavor text only — it doesn't change
    /// anything a replay must reproduce beyond which line gets logged, so
    /// it joins the presentation-only exclusion set (`killer`/`echo`/
    /// `facing`/`fx_hit` — see `save::state_hash`'s doc comment) rather
    /// than being hashed.
    pub(crate) mcguffin_last_line_turn: Option<u32>,
    /// batch 12 R5 ("light as grace"): set `true` in the dark-death branch
    /// of `Game::spend_turn` exactly when the McGuffin had a lit radius
    /// SOMEWHERE at the moment of death (`Game::mcguffin_light().is_some()`
    /// — she's yours and her mood tier shines) but it didn't reach the
    /// player's own tile (`lit_by_mcguffin` already failed, or this branch
    /// wouldn't run at all). Lets the End screen distinguish "the dark took
    /// you" from "the dark took you, though her light was somewhere else"
    /// without a new hashed field — it's fully derived from state that's
    /// already hashed (`mood_sum`/`mood_count`/`has_objective`/
    /// `objective_dropped`/`px`/`py`/the item list), same rationale as
    /// `killer`/`echo`/`facing`/`fx_hit`/`mcguffin_last_line_turn` (see
    /// `save::state_hash`'s doc comment for the shared exclusion-list
    /// rationale) — presentation-only, deliberately NOT hashed. Stays at
    /// its default `false` on any non-darkness death or while alive; only
    /// meaningful when `dead && killer.is_none()`.
    pub(crate) died_out_of_her_light: bool,
    /// batch 13 T1 ("the trainer reads your last life"): an echo-shaped,
    /// presentation-only memory of how the PREVIOUS attempt ended, carried
    /// forward across a same-seed RETRY (input byte 6) exactly the way
    /// `echo` (the death position) is — see `save::replay`'s byte-6 arm,
    /// which sets both from the same just-ended `Game` at the same point.
    /// `Some(true)` when that attempt died with `kills > spared` (the SAME
    /// read the McGuffin's pickup register already uses — see
    /// `Game::pickup_register_event`), `Some(false)` for a merciful death
    /// (`kills <= spared`), `None` for anything that isn't a post-death
    /// retry — a fresh reroll (byte 5) or a run's very first attempt, both
    /// of which leave it at `Self::base`'s default. Read by
    /// `Game::try_talk_player` (`MonsterDef::resurrection_lines`) so an
    /// overworld NPC can react to how the just-ended life was played.
    /// Presentation-only: joins the `killer`/`echo`/`facing`/`fx_hit`/
    /// `mcguffin_last_line_turn`/`died_out_of_her_light` exclusion set —
    /// deliberately NOT hashed by `state_hash` (see that function's doc
    /// comment), NOT printed by `--dump`, NOT itself saved (recomputed
    /// fresh by `save::replay` on every replay, exactly like `echo`).
    pub(crate) last_life_bloody: Option<bool>,
    /// batch 13 T1: whether the resurrection greeting above has already
    /// been spoken THIS attempt — a one-shot gate so a `resurrection_
    /// lines`-bearing NPC greets a returning player exactly once per life,
    /// not on every landed talk afterward. Resets to `false` on every fresh
    /// `Self::base` call (a new attempt, retried or rerolled). Same
    /// presentation-only exclusion set as `last_life_bloody` above: it only
    /// gates which extra line gets logged once, nothing replay needs to
    /// reproduce.
    pub(crate) last_life_greeting_spoken: bool,
    /// Whether the player currently holds the run's win-condition item
    /// (`ItemEffect::Objective` — see `Game::pickup`). Doubles the per-turn
    /// light burn (`WinDef::carry_burn`, it is heavy) and is the second of
    /// two conditions (with standing on the return depth's up-stairs) that
    /// wins the run.
    pub(crate) has_objective: bool,
    /// batch 7 T2 (story §9-A's minimal inventory): item kinds picked up via
    /// a `Hold`-behavior `ItemDef` (`Game::pickup`), LIFO — the most
    /// recently picked-up item is `held.last()`, and it's always the one
    /// GIVE (bytes 11-14) or USE (byte 15) act on; a landed GIVE/USE that
    /// `consumes` pops it. No grid UI, deliberately (§9-A: "a small held-
    /// items list, hashed, no grid UI"). Hashed by `save::state_hash` (which
    /// item is held, and in what order, is run-defining, same rationale as
    /// `monster.regard`/`Game::blocks`).
    pub(crate) held: Vec<u8>,
    /// batch 8 T1 (story §9-B/C/D, the McGuffin's voice): count of
    /// `CarryEvent::StairsUp` events fired so far while carrying the
    /// objective BEFORE the current one (`Game::ascend` fires
    /// `CarryEvent::StairsUp` first, reading the pre-increment count, THEN
    /// increments — fix-round correction: incrementing first would skip
    /// index 0, so index 0 = the first carried ascent, index 1 = the
    /// second, etc.) — the climb re-entry ladder's index into that event's
    /// line pool (`Game::carry_event`, `GameDef::carried_lines`). Hashed by
    /// `save::state_hash`: it changes which line a future ascent draws,
    /// which is run-defining once the pool is non-empty (T2), not merely
    /// presentational — same rationale as `held`/`Monster::regard`.
    pub(crate) speech_attempts: u8,
    /// batch 8 T1 (story §9-D, put-down/pick-back-up): true while the
    /// win-condition item is sitting on a floor tile after a put-down
    /// (`Game::put_down`) rather than actually held; false once re-picked-up
    /// (`Game::pickup`'s `ItemEffect::Objective` arm). Distinguishes a
    /// FIRST pickup (`CarryEvent::PickedUpBloody`/`PickedUpMerciful`) from a
    /// re-pickup after abandoning it (`CarryEvent::PickedBackUp`). Hashed: whether the
    /// objective is currently sitting dropped somewhere is run-defining
    /// state (it changes which `CarryEvent` a future pickup fires), not
    /// presentation.
    pub(crate) objective_dropped: bool,
    /// batch 12 R4 (the pickup verdict — "light as grace"): running-average
    /// numerator/denominator for the McGuffin's mood (`Game::mood`). Seeded
    /// ONCE, at the objective's FIRST pickup (`Game::pickup`'s
    /// `ItemEffect::Objective` arm, the `!was_dropped` branch — same guard
    /// that already distinguishes a first pickup from a `PickedBackUp` re-
    /// pickup, so a put-down/re-pickup cannot re-seed the anchor), from the
    /// descent's kill/spare record: `anchor_score = 50` if the player never
    /// fought or talked, else `100 * spared / (kills + spared)` (0 = pure
    /// brute, 100 = pure diplomat). `mood_sum = mood_anchor_weight *
    /// anchor_score; mood_count = mood_anchor_weight` at that moment (see
    /// `BalanceDef::mood_anchor_weight`). Every notable act on the climb
    /// thereafter averages in: a post-pickup kill adds
    /// `MonsterDef::kill_valence` for the kind killed (graduated —
    /// "not every kill is as despicable as every other"), a post-pickup
    /// spare/becalm (`Game::record_spare`) adds `BalanceDef::
    /// mood_spare_valence`. Both are hashed (`save::state_hash`): mood is
    /// run-defining (it will drive the McGuffin's shine radius, a future
    /// task), not presentation. `mood_count == 0` (before any pickup) means
    /// `Game::mood` returns the neutral 50 — her shine doesn't exist until
    /// she's picked up, so an unseeded average is harmless.
    ///
    /// R5+ hook (NOT built this task, per explicit human instruction): "every
    /// time she sees a becalmed monster, she likes you that much more" — a
    /// bounded, once-per-monster mood lift for passing a becalmed monster
    /// within her shine radius on the climb. The natural site is
    /// `Game::monsters_act`'s per-monster loop, but a becalmed monster hits
    /// that loop's `calm || passive` early `continue` before any
    /// distance/sees check runs — the detection has to live ahead of that
    /// `continue`, gated on `has_objective` and a new hashed once-per-
    /// monster "already greeted the McGuffin" flag (farming-guarded, same
    /// spirit as `Monster.awe`'s cap). Left as a comment, not code: this
    /// task is the anchor seed plus the kill/spare valences only.
    pub(crate) mood_sum: i32,
    pub(crate) mood_count: i32,
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
    /// Shared field-init tail for `Game::new`/`Game::new_overworld` (batch 9
    /// T1, SIGN-OFF ASK #3): every field both constructors need identically
    /// (RNG channels, starting stats, every presentation-only default) lives
    /// here, factored verbatim out of the pre-batch-9 inline struct literal
    /// — so `Game::new`'s own behavior stays byte-identical (solve/sim/dump/
    /// goldens/xhash/every pre-batch-9 test still calls it and depends on
    /// exactly this shape). The two constructors diverge only in `world`
    /// (passed in here) and what each does AFTER this returns (`Game::new`
    /// calls `gen_level` + logs the intro; `Game::new_overworld` re-sizes
    /// `saved` to 3 and calls `instantiate_overworld_screen(1)` — neither
    /// tail lives here).
    fn base(seed: u64, world: WorldId) -> Self {
        Game {
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
            mcguffin_last_line_turn: None,
            died_out_of_her_light: false,
            last_life_bloody: None,
            last_life_greeting_spoken: false,
            has_objective: false,
            held: Vec::new(),
            speech_attempts: 0,
            objective_dropped: false,
            mood_sum: 0,
            mood_count: 0,
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
            world,
            msgs: Vec::new(),
            seed,
            combat_rng: channel(seed, &["combat"]),
            ai_rng: channel(seed, &["ai"]),
            flavor_rng: channel(seed, &["flavor"]),
            parley_rng: channel(seed, &["parley"]),
            dead: false,
            won: false,
        }
    }

    pub(crate) fn new(seed: u64) -> Self {
        let mut g = Self::base(seed, WorldId::Seed(seed));
        g.gen_level();
        g.log(String::from(GAME.strings.intro));
        g
    }

    /// The overworld front door (batch 9 T1, story §9-J prep, SIGN-OFF ASK
    /// #3/#4): a fresh attempt begins in the fixed 3-screen overworld
    /// rather than directly in the root dungeon. `Game::new` above is
    /// UNCHANGED — it stays the frozen entry point `solve_seed`/`sim_seed`/
    /// `dump`/the goldens/every pre-batch-9 test depends on (see
    /// `Self::base`'s doc comment for why the refactor is byte-identical).
    /// Nothing in T1 calls `new_overworld` except new tests and
    /// `--dump-overworld`: wiring it into the REAL interactive front door
    /// and `save::replay` is T3's job (batch-9 brief Design §4.1-4.2) — this
    /// constructor existing now, additively, is what makes that later swap
    /// a one-line change instead of a redesign.
    pub(crate) fn new_overworld(seed: u64) -> Self {
        let mut g = Self::base(seed, WorldId::Overworld);
        // Three screens, one stash slot each (batch-9 brief Design §1) —
        // `Game::stash_level`/`restore_level*` already index unconditionally
        // by `self.depth`, so this is the only overworld-specific sizing
        // needed; `Self::base`'s default (`GAME.win.max_depth`-sized, the
        // dungeon's 5) would be the wrong length for this world.
        g.saved = vec![None, None, None];
        g.instantiate_overworld_screen(1);
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
            // batch 9 T1: the overworld has no worldgen/theme of its own
            // either (see `WorldId::Overworld`'s doc comment) — same
            // borrowed-root-seed convention `Floor` already established,
            // needed because talk lines against the trainer/donkey still
            // call `self.mob_name()` -> `self.theme()` -> `world_seed()`.
            WorldId::Overworld => self.seed,
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
                self.monsters.push(Monster { x: mx, y: my, kind, hp, regard: 0, calm: false, awe: 0, dividend_paid: false });
            }
        }
        /* items: deep floors are a war of attrition, so supply scales too —
           part of the same sim-gated balance pass as the spawn table. batch
           7 T2: each slot first checks the bonus-item weight for THIS depth
           (`loot_bonus_chance`, indexed by depth-1) before the existing
           potion/sword roll — a depth with no table entry (or num==0) draws
           nothing extra here, so pre-batch-7 depths (empty in the table
           this batch ships) consume the spawns channel identically to
           before; only the depths with a nonzero entry draw one additional
           `sr.chance` per loot slot, which is why THIS batch's golden diff
           is item-content-only on those depths and untouched everywhere
           else (see the cartridge's `BALANCE.loot_bonus_chance` doc comment
           and this batch's commit message). */
        let b = &GAME.balance;
        for _ in 0..sr.range(b.loot_count_lo, b.loot_count_hi) + (self.depth as i32 - 1) * b.loot_count_per_depth {
            if let Some((ix, iy)) = self.rand_floor(&mut sr, 4) {
                let bonus_weight = b.loot_bonus_chance.get(self.depth as usize - 1).copied();
                let rolled_bonus = match bonus_weight {
                    Some((num, den)) if num > 0 => sr.chance(num, den),
                    _ => false,
                };
                let kind = if rolled_bonus {
                    b.loot_bonus_item
                } else if sr.chance(b.loot_potion_num, b.loot_potion_den) {
                    b.loot_potion_item
                } else {
                    b.loot_sword_item
                };
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
                    self.monsters.push(Monster { x: tx, y: ty, kind: ki as MKind, hp, regard: 0, calm: false, awe: 0, dividend_paid: false });
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
    /// Torch FOV, composed with the McGuffin's own shine (batch 12 R5,
    /// "light as grace") when she's yours and her mood tier actually
    /// shines — a tile is visible if it's within the torch's circle OR
    /// within hers (`Game::mcguffin_light`), each independently raycast
    /// via the shared `light_circle` helper so the two sources use
    /// IDENTICAL distance/LOS semantics. While carried, her circle is
    /// centered on the player's own tile too (a bigger, concentric torch,
    /// walked in); while put down, it's centered wherever she was left —
    /// the "park / scout / return" shuttle is exactly this, no separate
    /// code path.
    fn compute_fov(&mut self) {
        let r = fov_radius(self.light);
        self.vis.iter_mut().for_each(|v| *v = false);
        let (px, py) = (self.px, self.py);
        self.light_circle(px, py, r);
        if let Some(((cx, cy), rr)) = self.mcguffin_light() {
            self.light_circle(cx, cy, rr);
        }
    }

    /// Mark every tile within Euclidean-circle distance `r` of `(cx, cy)`
    /// that also has line-of-sight to `(cx, cy)` as `vis`/`seen` — the one
    /// raycast loop shared by the torch and the McGuffin's own shine (batch
    /// 12 R5), so the two light sources can never silently disagree on
    /// what "within radius" or "unobstructed" means.
    fn light_circle(&mut self, cx: i32, cy: i32, r: i32) {
        self.vis[idx(cx, cy)] = true;
        self.seen[idx(cx, cy)] = true;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (tx, ty) = (cx + dx, cy + dy);
                if !in_map(tx, ty) {
                    continue;
                }
                if self.los(cx, cy, tx, ty) {
                    self.vis[idx(tx, ty)] = true;
                    self.seen[idx(tx, ty)] = true;
                }
            }
        }
    }

    /// Her current position + shine radius, IFF she is currently "yours"
    /// (carried — `has_objective`, at the player's own tile — or put down
    /// after a first pickup — `objective_dropped`, at wherever `Game::
    /// put_down` re-entered her into `self.items`) AND her mood tier
    /// actually shines (`mood_shine_radius(self.mood()) > 0`). `None`
    /// whenever: she hasn't been claimed yet at all (on the floor,
    /// `!has_objective && !objective_dropped` — she isn't "yours" and
    /// doesn't shine for you), OR her mood tier is the darkest band
    /// (radius `0`) — this second case is the CRITICAL nerf-preservation
    /// path: a mood-0 carrier must read as "she does not shine," full
    /// stop, never as "she shines with radius 0," which — because
    /// `light_circle`/`lit_by_mcguffin` both still mark/test her own tile
    /// unconditionally at distance 0 — would otherwise quietly save a
    /// brute a dead-torch death the story's own T1 nerf already priced.
    fn mcguffin_light(&self) -> Option<((i32, i32), i32)> {
        let pos = if self.has_objective {
            Some((self.px, self.py))
        } else if self.objective_dropped {
            self.items.iter().find(|it| it.kind == GAME.win.objective_item).map(|it| (it.x, it.y))
        } else {
            None
        }?;
        let r = mood_shine_radius(self.mood());
        if r > 0 {
            Some((pos, r))
        } else {
            None
        }
    }

    /// The "no light reaches you" half of the death check (batch 12 R5):
    /// whether the PLAYER's own tile currently falls inside the
    /// McGuffin's shine — same Euclidean-circle-distance-AND-LOS test
    /// `light_circle` uses to decide any other tile, so this predicate can
    /// never disagree with what `compute_fov` actually lit the player's
    /// tile with (a Chebyshev or distance-only test here could diverge
    /// from the circle at the boundary, letting the death check and the
    /// render disagree about whether the player's own tile is lit).
    fn lit_by_mcguffin(&self) -> bool {
        let Some(((cx, cy), r)) = self.mcguffin_light() else {
            return false;
        };
        let (dx, dy) = (self.px - cx, self.py - cy);
        dx * dx + dy * dy <= r * r && self.los(cx, cy, self.px, self.py)
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
    /// movement/waiting). Checked once, after the combined deduction,
    /// before any win condition, golem-style.
    ///
    /// Batch 12 R5 ("light as grace") rewrites what "light 0" means: the
    /// torch hitting zero is no longer automatically fatal. Death is now
    /// **no light reaches you** — torch dead (`light <= 0`) AND the
    /// McGuffin's own shine doesn't reach the player's tile either
    /// (`Game::lit_by_mcguffin`, `None` whenever she isn't yours yet or her
    /// mood tier is the dark band). Consequences, all intended: a
    /// max-shine diplomat finishes the climb after the torch dies, walking
    /// in her light; a mood-zero brute dies exactly as before this batch
    /// (her shine is `None` for that carrier, full stop). Lose-before-win
    /// ordering and the single-deduction-then-check shape are both
    /// unchanged; only what counts as "the dark got you" changed. Returns
    /// false if the player died.
    fn spend_turn(&mut self, extra: i32) -> bool {
        self.turns += 1;
        // batch 9 T1 (story §9-J prep): the overworld has no torch clock —
        // the turn still counts (hashed, run-defining) but nothing burns
        // and the dark-death check never runs there. Early return, before
        // any light math, keeps this a one-line, clearly-scoped exemption
        // rather than threading a world-kind branch through the burn/tier-
        // warning logic below.
        if self.world == WorldId::Overworld {
            return true;
        }
        let before = fov_radius(self.light);
        let base = if self.has_objective { GAME.win.carry_burn } else { GAME.balance.base_burn };
        self.light -= base + extra;
        if self.light <= 0 {
            self.light = 0;
            // batch 12 R5: torch-dead alone is no longer fatal — only if
            // her light ALSO doesn't reach the player this turn.
            if !self.lit_by_mcguffin() {
                self.dead = true;
                // Presentation-only (see the field's own doc comment):
                // distinguishes the End screen's "she was shining
                // somewhere, but not here" line from a plain darkness
                // death, without needing a new hashed field — `mcguffin_
                // light().is_some()` is exactly "she has a lit radius
                // right now, it's just not reaching the player," since
                // `lit_by_mcguffin` already just failed above.
                self.died_out_of_her_light = self.mcguffin_light().is_some();
                self.log(String::from(GAME.strings.dark_death));
                self.compute_fov();
                return false;
            }
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
            // batch 8 T1: the McGuffin may have its own opinion of the
            // failing torch, on top of the engine's own tier warning.
            self.carry_event(CarryEvent::TierCrossed);
        }
        true
    }

    /// The single site every becalm path (landed talk, landed give, awe
    /// crossing its threshold) routes through — increments `self.spared`.
    /// Batch 12 R2: the mercy-side light stipend this helper used to apply
    /// (batch 12 T2's now-removed balance field) was stripped — mercy no
    /// longer feeds light on the descent; that reward moves to the
    /// McGuffin's mood/shine at pickup (later tasks). Kept as the one
    /// consolidated spare site regardless, so a future mood/anchor mechanic
    /// has exactly one place to hook — same lesson as batch 11's awe helper.
    ///
    /// Batch 12 R4: that "future mechanic" is now — a post-pickup spare
    /// (`self.mood_count > 0`, i.e. the anchor has already been seeded by
    /// `Game::pickup`) averages `BalanceDef::mood_spare_valence` into the
    /// running mood average (`Game::mood`). A spare BEFORE the objective is
    /// ever picked up only feeds the anchor's `spared` count at the pickup
    /// snapshot — it doesn't touch `mood_sum`/`mood_count` directly, which
    /// is exactly why this guard is `mood_count > 0` and not, say, `self.
    /// has_objective` (a spare landed after a `put_down` still counts,
    /// matching the brief's "post-pickup" wording — chronologically after
    /// the first pickup, not only while currently carrying).
    fn record_spare(&mut self) {
        self.spared += 1;
        if self.mood_count > 0 {
            self.mood_sum += GAME.balance.mood_spare_valence;
            self.mood_count += 1;
        }
    }

    /// The McGuffin's mood (batch 12 R4, "the pickup verdict"): a pure,
    /// deterministic running average, `0..=100`, derived only from hashed
    /// state (`mood_sum`/`mood_count` — see their doc comment on `Game` for
    /// the full seeding/update model). `50` (neutral) whenever
    /// `mood_count == 0` — before the objective's first pickup, there is no
    /// average to report yet, and the McGuffin's shine has no opinion to
    /// shine with (`mcguffin_light` guards on `has_objective`/
    /// `objective_dropped` regardless, so this default is never actually
    /// consulted before a first pickup anyway). Batch 12 R5 gave this its
    /// first non-test caller: `Game::mcguffin_light` feeds it straight into
    /// `mood_shine_radius`.
    pub(crate) fn mood(&self) -> i32 {
        if self.mood_count == 0 { 50 } else { (self.mood_sum / self.mood_count).clamp(0, 100) }
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
        self.shove_monster_off(pos);
        self.compute_fov();
    }

    /// Shove aside any monster occupying `pos` (the player's arrival tile),
    /// searching its four orthogonal neighbors for a legal landing spot and
    /// removing the monster outright if none exists. Shared by
    /// `apply_restored_level` (the restore-from-stash arrival path) and
    /// `cross_screen_link`'s fresh-instantiate arrival path (batch 9 T1
    /// review fix — the fresh-instantiate branch used to hand-roll its own
    /// copy of this check that was missing the `Tile::Pit`/`Game::blocks`
    /// exclusions below, a latent divergence from this one; factoring both
    /// call sites through one function makes that divergence impossible to
    /// reintroduce). "Legal landing spot" mirrors `monsters_act`'s own
    /// neighbor-step legality: not a wall, not a pit, not another monster's
    /// tile, not a sokoban block's tile.
    fn shove_monster_off(&mut self, pos: (i32, i32)) {
        if let Some(mi) = self.monsters.iter().position(|m| (m.x, m.y) == pos) {
            let (mx, my) = (self.monsters[mi].x, self.monsters[mi].y);
            let spot = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().find_map(|&(dx, dy)| {
                let (tx, ty) = (mx + dx, my + dy);
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
    }

    /// Pull a calm, `follows_when_calm` monster out of the current screen's
    /// monster list, if there is one — the piece that lets
    /// `Game::cross_screen_link` carry a followed monster across a screen
    /// boundary (batch 13 T6, the donkey-follow seed's rung 2). Must run
    /// BEFORE `Game::stash_level`: that snapshots whatever is left in
    /// `self.monsters` verbatim into the old screen's `LevelState`, so a
    /// follower has to be removed first or it gets left behind — which is
    /// exactly the desired behavior for a NON-following (not yet calm)
    /// monster, just not for a follower. Overworld-only; a no-op (and
    /// irrelevant) anywhere else, since nothing outside the overworld
    /// cartridge data ever sets the flag.
    fn take_calm_follower(&mut self) -> Option<Monster> {
        if self.world != WorldId::Overworld {
            return None;
        }
        let i = self
            .monsters
            .iter()
            .position(|m| m.calm && Monster::stats(m.kind).follows_when_calm)?;
        Some(self.monsters.remove(i))
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
                    // batch 9 T1: defensive only — nothing in this batch
                    // ever forward-transits INTO the overworld (only OUT of
                    // it, via `Game::cross_into_dungeon`); a future batch
                    // that adds a way back would want this same 3-slot
                    // sizing `Game::new_overworld` already uses.
                    WorldId::Overworld => vec![None, None, None],
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
            // batch 9 T1: defensive only, see the sizing match right above
            // this method's own doc comment — never reached in this batch.
            WorldId::Overworld => self.instantiate_overworld_screen(self.depth as usize),
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
                            self.monsters.push(Monster { x: tx, y: ty, kind: ki as MKind, hp, regard: 0, calm: false, awe: 0, dividend_paid: false });
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

    /// Instantiate one of the overworld's 3 fixed screens (batch 9 T1,
    /// story §9-J prep, SIGN-OFF ASK #1): parses `GAME.overworld.
    /// screens[i-1]`'s const ASCII directly into `map`/`items`/`monsters` —
    /// zero RNG draws, same convention as `Game::instantiate_floor` (see
    /// that method's doc comment), extended with three overworld-only tile
    /// glyphs: `=` (`Tile::ScreenLink`, direction derived from which edge
    /// column it sits on: `col == 0` is the west/prev edge, `col == fw-1`
    /// is the east/next edge — see that variant's doc comment), `V`
    /// (`Tile::Hole`), `+` (`Tile::ShutDoor`).
    ///
    /// Sets a DEFAULT player position (the first `Tile::Floor` tile found
    /// scanning row-major) and logs a fixed arrival line
    /// (`StringsDef::overworld_enter` + the screen's own `describe`,
    /// mirroring `instantiate_floor`'s name+describe pair). For
    /// `Game::new_overworld` (screen 1, run start) this default placement
    /// IS the real one. For `Game::cross_screen_link` calling this on a
    /// never-before-visited screen, both the position and this method's own
    /// log are immediately followed by that caller's own placement/log —
    /// two stacked log lines on a first visit, same established pattern as
    /// `Game::transit`'s own arrival log stacking under a portal
    /// destination's fresh-generation log (see that method's doc comment).
    pub(crate) fn instantiate_overworld_screen(&mut self, i: usize) {
        self.map = vec![Tile::Wall; COLS * MAP_H];
        self.seen = vec![false; COLS * MAP_H];
        self.vis = vec![false; COLS * MAP_H];
        self.monsters.clear();
        self.items.clear();
        self.portal = None;
        self.blocks.clear();

        let screen = &GAME.overworld.screens[i - 1];
        let rows: Vec<&str> = screen.map.lines().collect();
        let (fw, fh) = (rows[0].len() as i32, rows.len() as i32);
        let ox = (COLS as i32 - fw) / 2;
        let oy = (MAP_H as i32 - fh) / 2;
        let mut start = (ox, oy);
        let mut start_found = false;
        for (j, row) in rows.iter().enumerate() {
            for (col, c) in row.bytes().enumerate() {
                let (tx, ty) = (ox + col as i32, oy + j as i32);
                match c {
                    b'#' => self.map[idx(tx, ty)] = Tile::Wall,
                    b'.' => self.map[idx(tx, ty)] = Tile::Floor,
                    b'=' => self.map[idx(tx, ty)] = Tile::ScreenLink(col as i32 == fw - 1),
                    b'V' => self.map[idx(tx, ty)] = Tile::Hole,
                    b'+' => self.map[idx(tx, ty)] = Tile::ShutDoor,
                    _ => {
                        if let Some(ii) = GAME.items.iter().position(|it| it.glyph == c) {
                            self.map[idx(tx, ty)] = Tile::Floor;
                            self.items.push(Item { x: tx, y: ty, kind: ii as IKind });
                        } else if let Some(ki) = GAME.monsters.iter().position(|m| m.glyph == c) {
                            self.map[idx(tx, ty)] = Tile::Floor;
                            let hp = GAME.monsters[ki].hp;
                            self.monsters.push(Monster { x: tx, y: ty, kind: ki as MKind, hp, regard: 0, calm: false, awe: 0, dividend_paid: false });
                        }
                        // else: well-formedness (main.rs) guards the legal-
                        // char set; an unrecognized byte leaves the default
                        // Wall tile untouched.
                    }
                }
                // batch 9 T1 fix: the default start must be a genuinely
                // empty floor tile, not merely "whatever `self.map` ended up
                // holding" — an item/monster glyph also stamps
                // `Tile::Floor` onto its own tile (see the `_` arm above), so
                // checking the map value post-hoc could pick a tile that's
                // occupied by a monster or item (confirmed: OVERWORLD_1's
                // row-major-first floor-like tile is the DONKEY's own `D`
                // glyph, which made the player spawn on top of it and hide
                // it from every render/dump). Matching the literal `.` byte
                // instead guarantees the candidate tile has no authored
                // content on it at all.
                if !start_found && c == b'.' {
                    start = (tx, ty);
                    start_found = true;
                }
            }
        }
        self.px = start.0;
        self.py = start.1;
        self.rooms = vec![(ox, oy, fw, fh)];
        self.room_meta = vec![(0, 0)];
        self.room_visited = vec![true];
        self.compute_fov();
        self.log(GAME.strings.overworld_enter.replace("{}", screen.name));
        self.log(String::from(screen.describe));
    }

    /// Cross a `Tile::ScreenLink` (batch 9 T1, story §9-J prep, SIGN-OFF
    /// ASKS #1/#2): a new sibling of `descend`/`ascend`, modeled closely on
    /// their shape (stash the current level, move to the target screen
    /// index, restore its stash or instantiate it fresh, log an arrival
    /// line) but crossing between the overworld's 3 fixed screens instead
    /// of dungeon depths. `east` is `Tile::ScreenLink`'s own bool (`true` =
    /// the link stood on links to the NEXT screen/east edge, `false` = to
    /// the PREVIOUS screen/west edge). Confirmed instant edge-walk, no
    /// confirming input (SIGN-OFF ASK #2) — called directly from
    /// `Game::land_on_tile`'s `Tile::ScreenLink` arm on walk-onto, exactly
    /// like `Tile::Stairs`/`Tile::UpStairs`.
    ///
    /// The player lands on the SAME `row` they crossed at, one tile in from
    /// the OPPOSITE edge of the destination screen (so they never spawn
    /// exactly on another link tile) — `arrive_x` is pure geometry over
    /// `GAME.overworld`'s own screen width, computed once here and reused
    /// whether the destination is freshly instantiated or restored from a
    /// stash (`Game::restore_level_at` already places at an exact `(x, y)`,
    /// same mechanism `return_to_source` uses for a portal's exact return
    /// tile).
    fn cross_screen_link(&mut self, row: i32, east: bool) {
        // batch 13 T6: pull a calm follower out BEFORE the stash below
        // snapshots the old screen's monster list, so it travels with the
        // player instead of being left behind in that screen's `LevelState`.
        let follower = self.take_calm_follower();
        self.stash_level();
        self.depth = if east { self.depth + 1 } else { self.depth - 1 };
        let d = self.depth as usize - 1;
        let screen = &GAME.overworld.screens[d];
        let fw = screen.map.lines().next().map(|r| r.len()).unwrap_or(0) as i32;
        let ox = (COLS as i32 - fw) / 2;
        let arrive_x = if east { ox + 1 } else { ox + fw - 2 };
        match self.saved[d].take() {
            Some(ls) => self.restore_level_at(ls, arrive_x, row),
            None => {
                self.instantiate_overworld_screen(self.depth as usize);
                // Override the default placement with the exact arrival
                // tile, shoving aside a monster that happens to already
                // occupy it via the SAME `shove_monster_off` helper
                // `apply_restored_level` uses for the restore branch above
                // (batch 9 T1 review fix — this branch used to hand-roll its
                // own copy of the check, missing the `Tile::Pit`/
                // `Game::blocks` exclusions the shared helper has; only ever
                // fires here if a future content pass places a monster
                // directly on a link's counterpart tile — none of this
                // batch's placeholder screens do).
                self.px = arrive_x;
                self.py = row;
                self.shove_monster_off((arrive_x, row));
                self.compute_fov();
            }
        }
        // batch 13 T6: drop the carried follower onto the new screen, at a
        // free cardinal neighbor of the player's arrival tile — never onto
        // the player, a wall, a pit, or another monster (same legality as
        // `shove_monster_off`'s own neighbor search).
        if let Some(mut m) = follower {
            let (fx, fy) = self.free_neighbor_near((arrive_x, row));
            m.x = fx;
            m.y = fy;
            self.monsters.push(m);
        }
        self.log(GAME.strings.overworld_cross.replace("{}", screen.name));
    }

    /// A free cardinal neighbor of `near` (not a wall, not a pit, not the
    /// player's own tile, not another monster's tile) — falls back to
    /// `near` itself if every neighbor is blocked (never expected on this
    /// batch's placeholder screens, which are wide open fields; a future,
    /// denser screen could in principle hit it, and landing exactly on the
    /// link's counterpart tile the player just stood on is a harmless
    /// worst case, not a panic). Used by `cross_screen_link` to place a
    /// carried-across follower beside, not on top of, the player.
    fn free_neighbor_near(&self, near: (i32, i32)) -> (i32, i32) {
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .map(|&(dx, dy)| (near.0 + dx, near.1 + dy))
            .find(|&(tx, ty)| {
                in_map(tx, ty)
                    && self.map[idx(tx, ty)] != Tile::Wall
                    && self.map[idx(tx, ty)] != Tile::Pit
                    && (tx, ty) != (self.px, self.py)
                    && !self.monsters.iter().any(|m| (m.x, m.y) == (tx, ty))
            })
            .unwrap_or(near)
    }

    /// Cross a `Tile::Hole` (batch 9 T1, story §9-J prep): the overworld's
    /// one true engine-plumbing tile, transiting from `WorldId::Overworld`
    /// into the root dungeon (`WorldId::Seed(self.seed)`) via the SAME
    /// forward-transit machinery a portal uses (`Game::transit`), just
    /// without a rolled/cached destination — the destination is always
    /// `self.seed`'s own root dungeon, known statically rather than drawn
    /// from `Game::portal`. Entering `WorldId::Seed(self.seed)` makes
    /// `self.world == WorldId::Seed(self.seed)` true exactly as it always
    /// has been for a `Game::new`-started run (see `WorldId`'s doc comment
    /// on why "root" is a comparison, not a stored flag) — the existing
    /// win/lose machinery in `Game::land_on_tile`'s `Tile::UpStairs` arm
    /// needs ZERO changes to keep working once this fires (batch-9 brief
    /// Design §1, the strongest evidence for reusing the existing
    /// world-transition machinery rather than inventing a parallel one).
    fn cross_into_dungeon(&mut self) {
        let (vx, vy) = (self.px, self.py);
        let src = self.world;
        let seed = self.seed;
        self.leave_current_world();
        self.enter_world_forward(WorldId::Seed(seed), (src, vx, vy));
        // Reuses the same reminder `Game::new` logs on a direct dungeon
        // start — entering the dungeon for the first time via the hole is
        // functionally the same moment a fresh `Game::new` run used to
        // start at.
        self.log(String::from(GAME.strings.intro));
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
        // batch 8 T1 (story §9-B/C/D): the climb re-entry ladder — each
        // ascent made WHILE carrying the objective counts toward
        // `speech_attempts`, which indexes `CarryEvent::StairsUp`'s line
        // pool (`Game::carry_event`) instead of a random draw. Fix-round
        // correction: fire BEFORE incrementing, so the pool is read at the
        // PRE-increment count — the first carried ascent reads index 0 (not
        // 1), the second reads index 1, etc. The final per-run count of
        // `speech_attempts` is unchanged either way (still exactly one
        // increment per carried ascent).
        if self.has_objective {
            self.carry_event(CarryEvent::StairsUp);
            self.speech_attempts = self.speech_attempts.saturating_add(1);
        }
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
        // batch 11 T2 fix round: the player's position BEFORE this turn's
        // own move — `resolve_awe`'s hold-vs-flee decision needs this, since
        // "did the player retreat" is measured against where they started
        // the turn, not (as the pre-fix bug did) against post-chase
        // positions. Captured once, here, before any of this function's
        // branches can mutate `self.px`/`self.py`.
        let prev_player = (self.px, self.py);
        let (nx, ny) = (self.px + dx, self.py + dy);
        if !in_map(nx, ny) {
            return;
        }
        // batch 11 T2: which monster (if any) the player bump-ATTACKED this
        // turn, for `resolve_awe`'s exclusion — declared out here (rather
        // than inside the `if let Some(mi)` block below) so it's still in
        // scope at the fall-through attack-path tail past that block's
        // close. Assigned exactly once, on the only path that ever reaches
        // that tail (every other branch — yield/shove/push/pit/shut-door/
        // plain-move/wall — returns before then; yield/shove/push/plain-
        // move route through `land_on_tile`, which resolves its own
        // `resolve_awe(None)` independently).
        let attacked_idx: Option<usize>;
        if let Some(mi) = self.monsters.iter().position(|m| m.x == nx && m.y == ny) {
            self.facing = Facing::from_delta(dx, dy);
            let bump = Monster::stats(self.monsters[mi].kind).bump;
            if self.monsters[mi].calm || bump == BumpResponse::Yield {
                // Mercy's second economy lever (batch 5, DECISION.md item
                // 3): bumping a becalmed monster SWAPS positions instead
                // of attacking — it yields. Costs a turn like any move, no
                // violence tax, no damage. Only `calm == true` swaps for a
                // pre-batch-9 kind (a monster mid-being-persuaded, regard >
                // 0 but not yet calm, still gets attacked below,
                // unchanged); batch 9 T1's `BumpResponse::Yield` (the
                // TRAINER's shape) takes this SAME path unconditionally,
                // regardless of `calm` — un-killable by construction, no
                // new `invincible` flag needed.
                let (ox, oy) = (self.px, self.py);
                self.monsters[mi].x = ox;
                self.monsters[mi].y = oy;
                self.px = nx;
                self.py = ny;
                self.land_on_tile(nx, ny, None, prev_player);
                return;
            }
            if bump == BumpResponse::Shove {
                // batch 9 T1 (story §9-J prep, SIGN-OFF ASK #6): the
                // DONKEY's shape — push one tile in the bump direction if
                // the destination is plain walkable floor (single-step
                // version of the sokoban push destination-legality
                // convention), else plant and refuse; never damages the
                // target either way. A successful shove also advances the
                // player into the vacated tile, same turn, mirroring
                // `Game::try_push`'s own player-follows-the-block
                // convention.
                let (tx, ty) = (self.monsters[mi].x + dx, self.monsters[mi].y + dy);
                let free = in_map(tx, ty)
                    && self.map[idx(tx, ty)] == Tile::Floor
                    && !self.monsters.iter().any(|m| (m.x, m.y) == (tx, ty))
                    && !self.blocks.iter().any(|&b| b == (tx, ty));
                if !free {
                    let name = self.mob_name(self.monsters[mi].kind);
                    self.log(GAME.strings.shove_refuse.replace("{}", name));
                    return; // stubborn: no move, no damage, no turn
                }
                self.monsters[mi].x = tx;
                self.monsters[mi].y = ty;
                self.px = nx;
                self.py = ny;
                self.land_on_tile(nx, ny, None, prev_player);
                return;
            }
            let dmg = self.atk + self.combat_rng.range(0, 3);
            let name = self.mob_name(self.monsters[mi].kind);
            // batch 11: read before the monster's hp changes (and possibly
            // the monster is removed below) — `retaliation` is a per-kind
            // constant, not per-instance state.
            let retal = Monster::stats(self.monsters[mi].kind).retaliation;
            // batch 12 R4: same read-before-removal discipline as `retal`
            // above — the mood valence for THIS kind, in case this swing
            // kills it.
            let kv = Monster::stats(self.monsters[mi].kind).kill_valence;
            self.monsters[mi].hp -= dmg;
            self.fx_hit = Some((nx, ny));
            let killed = self.monsters[mi].hp <= 0;
            if killed {
                self.monsters.remove(mi);
                self.kills += 1;
                // batch 12 T1 (light-as-grace, the violence half): a kill
                // recoils on the player's own light, on top of the ordinary
                // per-turn burn + violence tax that `spend_turn` applies
                // later this same turn. No separate death check here — the
                // single `light <= 0` dark-death check lives in
                // `spend_turn`, reached below on every path out of this
                // block (the fall-through attack-path tail, or the
                // retal-killed branch's own `spend_turn` call) — confirmed
                // by reading both call sites, not assumed.
                self.light -= GAME.balance.kill_light_penalty;
                // batch 12 R4: a POST-pickup kill (the anchor already
                // seeded, `mood_count > 0`) averages this kind's
                // `kill_valence` into the running mood — see `Game::mood`'s
                // doc comment for the model, `MonsterDef::kill_valence`'s
                // for why this isn't a flat value.
                if self.mood_count > 0 {
                    self.mood_sum += kv;
                    self.mood_count += 1;
                }
                self.log(GAME.strings.slay.replacen("{}", name, 1).replacen("{}", &dmg.to_string(), 1));
                self.carry_event(CarryEvent::KillWitnessed);
            } else {
                self.log(GAME.strings.hit.replacen("{}", name, 1).replacen("{}", &dmg.to_string(), 1));
            }
            // batch 11 T2: a killed monster no longer occupies index `mi`
            // (removed above) — nothing to exclude from `resolve_awe`, so
            // `None`. A surviving one WAS bump-attacked this turn, so it's
            // excluded (attacking resets its awe, per the Design paragraph)
            // regardless of whether it also lands a retaliation hit below.
            attacked_idx = if killed { None } else { Some(mi) };
            // batch 11: the ogre (any kind with retaliation > 0) always lands
            // a hit back the instant you swing — even a killing blow costs
            // you. Separate from its ordinary `monsters_act` turn (which
            // only happens if it survives).
            //
            // batch 11 T1 fix round: a lethal retaliation must still
            // advance `turns`/`light` for this turn exactly like every
            // other death path does (`spend_turn`'s own dark-death branch
            // increments `turns` and burns light before its early return;
            // an ordinary combat death only ever happens from
            // `monsters_act`, which runs AFTER `try_move_player` already
            // called `spend_turn`). The original shape here early-returned
            // BEFORE `spend_turn` ever ran, so an ogre-retaliation kill
            // under-counted `turns` by one and left `light` unburned
            // (neither the base burn nor the violence tax) relative to
            // every other death this engine can produce — both fields are
            // hashed (`state_hash`) and shown on the End screen, so this
            // was a real inconsistency, not cosmetic.
            //
            // Fix: don't early-return here. Record whether the
            // retaliation alone was lethal (`retal_killed`), set `killer`/
            // log/`dead` for that cause now (so it can never be clobbered
            // — `spend_turn`'s dark-death branch never touches `killer`),
            // then fall through into the normal `spend_turn(violence_tax)`
            // call below so `turns`/`light` advance like any other combat
            // turn. If light ALSO hits 0 this same turn, HP-death wins the
            // tie (the retaliation is what actually killed the player);
            // `killer` staying `Some` is what encodes that, since
            // `render_end` and the ghost-outcome backends both key off
            // `killer.is_some()` rather than the log's last line.
            let mut retal_killed = false;
            if retal > 0 {
                self.hp -= retal;
                if self.hp <= 0 {
                    self.hp = 0;
                    self.dead = true;
                    retal_killed = true;
                    self.killer = Some(name);
                    self.log(GAME.strings.killed_by.replace("{}", name));
                } else {
                    self.log(GAME.strings.hit_by.replacen("{}", name, 1).replacen("{}", &retal.to_string(), 1));
                }
            }
            if retal_killed {
                // Advance turns/light for this fatal turn exactly like an
                // ordinary attack does, then stop — no further monster
                // turns for an already-dead player. `spend_turn`'s own
                // light<=0 branch may also fire here (both causes landing
                // the same turn); it never touches `killer`, so the
                // combat attribution set above stands regardless.
                self.spend_turn(GAME.balance.violence_tax);
                self.compute_fov();
                return;
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
            self.land_on_tile(nx, ny, None, prev_player);
            return;
        } else if self.map[idx(nx, ny)] == Tile::Pit {
            // Sokoban (batch 6 T2): a pit refuses the player exactly like a
            // wall bump — no turn, grounded message, no mutation.
            self.log(String::from(GAME.strings.pit_refuse));
            return;
        } else if self.map[idx(nx, ny)] == Tile::ShutDoor {
            // batch 9 T1 (story §9-J prep): always shut this batch,
            // regardless of `has_objective` — see `Tile::ShutDoor`'s doc
            // comment. Refuses exactly like a pit bump: no turn, no
            // mutation.
            self.log(String::from(GAME.strings.shut_door_refuse));
            return;
        } else if self.map[idx(nx, ny)] != Tile::Wall {
            self.facing = Facing::from_delta(dx, dy);
            self.px = nx;
            self.py = ny;
            self.land_on_tile(nx, ny, None, prev_player);
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
        self.monsters_act_and_resolve_awe(None, attacked_idx, prev_player);
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
        // batch 13 T1 ("the trainer reads your last life"): the
        // resurrection greeting fires on the FIRST LANDED talk of a fresh
        // post-death life, once, for whichever kind's cartridge data
        // actually has one (`resurrection_lines` — `None` is a graceful
        // no-op, same invariant as `carry_event`'s empty-pool check).
        // `last_life_bloody`/`last_life_greeting_spoken` are both
        // presentation-only (see their own doc comments) — this can never
        // perturb `regard`/`calm`/anything `spend_turn` or `state_hash`
        // touches, only which extra line gets logged this one time.
        if landed && !self.last_life_greeting_spoken {
            if let Some(bloody) = self.last_life_bloody {
                if let Some(lines) = GAME.monsters[kind as usize].resurrection_lines {
                    self.log(String::from(lines[if bloody { 0 } else { 1 }]));
                    self.last_life_greeting_spoken = true;
                }
            }
        }
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
                self.record_spare();
                self.carry_event(CarryEvent::SpareWitnessed);
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
        // batch 11 T2: talking isn't bump-attacking, so a HOLD-type kind
        // (the ogre) reads a talk turn as building/holding awe exactly like
        // a wait would (independent of `stayed`) — "talk is a no-move
        // action" is deliberately double-duty for that kind (see
        // `talking_at_an_ogre_stands_tall_and_awes`).
        //
        // batch 13 T5: a GIVE-GROUND-type kind (the goblin) is the mirror
        // and must NOT get the same double-duty — talk is its OWN
        // pre-existing mercy mechanic (batch 5's regard/`talk_threshold`
        // path), orthogonal to awe, exactly like a GIVE is (see
        // `try_give_player`'s exclusions above). Without this exclusion, a
        // bot (or player) that persists at repeatedly talking to the same
        // un-becalmed goblin — a legitimate, pre-existing mercy strategy —
        // would ALSO read as "planted, refusing to give ground" every one
        // of those turns and take the punish hit on top of it, turning an
        // ordinary mercy attempt into an accidental death spiral. Measured:
        // this exact gap collapsed `--sim 5000 --policy pacifist` from its
        // 17-win baseline before this exclusion was added. Excluding here
        // only stops awe/punish bookkeeping for THIS monster THIS turn — it
        // never touches `regard`/`calm`/the ordinary talk becalm above.
        let awe_exclude = if Monster::stats(kind).awe_by_giving_ground { Some(mi) } else { None };
        self.monsters_act_and_resolve_awe(stayed, awe_exclude, (self.px, self.py));
    }

    /// GIVE: the mercy verb's counterpart (batch 7 T2, story §5/§9-A).
    /// Input bytes 11-14 (`apply_input`) map to N/S/W/E, mirroring the talk
    /// bytes' 7-10 direction order exactly (which mirrors the move bytes'
    /// 0-3 in turn). Offers the top of `self.held` (the most recently
    /// picked-up `Hold` item — LIFO) to the adjacent monster in that
    /// direction. No monster there, or nothing held, or a monster there but
    /// no `GameDef::give_table` row for (held item, its kind): all three are
    /// a no-op, no turn, one feedback line (`StringsDef::give_no_target`/
    /// `give_empty_hands`/`give_declined`) — a give that doesn't happen
    /// costs nothing, same spirit as a talk at a wall. A LANDED give (a
    /// matching row exists) always costs a normal turn
    /// (`spend_turn(0)`: no violence tax, giving is not violence), applies
    /// the row's `regard_delta` (saturating, may cross `Monster::
    /// talk_threshold` and becalm the target exactly like a landed talk),
    /// heals the target to full if `heal_full`, logs the row's `line` (or,
    /// if `None`, reuses the target's own stage-3 "unmoved" talk line — see
    /// `GiveRule::line`'s doc comment), and pops `held` if `consumes`.
    /// Unlike a landed talk, the target is NOT stayed — GIVE isn't
    /// "listening," it's a delivered object, so `monsters_act` runs with
    /// `None` exactly like a plain wait.
    ///
    /// **Exception, batch 13 T2**: a `GiveRule::stay_and_roll` row (cheese
    /// -> goblin, story §12.14) takes a separate branch entirely — see
    /// `GiveRule::stay_and_roll`'s doc comment for the guaranteed-stay +
    /// gambled-becalm shape, and below for the mechanics.
    ///
    /// **Exception, batch 13 T4**: a `GiveRule::enrage` row (potion ->
    /// goblin/ogre, "potion is mammal-medicine") also takes a separate
    /// branch — see `GiveRule::enrage`'s doc comment for the regard-crash +
    /// un-stayed-swing shape.
    pub(crate) fn try_give_player(&mut self, dx: i32, dy: i32) {
        self.fx_hit = None;
        if self.dead || self.won {
            return;
        }
        let (nx, ny) = (self.px + dx, self.py + dy);
        if !in_map(nx, ny) {
            return;
        }
        let Some(mi) = self.monsters.iter().position(|m| m.x == nx && m.y == ny) else {
            self.log(String::from(GAME.strings.give_no_target));
            return; // nothing to give it to: no-op, no turn
        };
        self.facing = Facing::from_delta(dx, dy);
        let Some(&item) = self.held.last() else {
            self.log(String::from(GAME.strings.give_empty_hands));
            return; // empty hands: no-op, no turn
        };
        let kind = self.monsters[mi].kind;
        let name = self.mob_name(kind);
        let Some(rule) =
            GAME.give_table.iter().find(|r| r.item == item && (r.monster.is_none() || r.monster == Some(kind)))
        else {
            self.log(GAME.strings.give_declined.replace("{}", name));
            return; // no give-table row for this (item, kind): no-op, no turn
        };
        if rule.stay_and_roll {
            // Batch 13 T2 (story §12.14 / arc doc "Cheese has a target"):
            // guaranteed tempo, gambled grace. The target is ALWAYS stayed
            // this turn regardless of roll outcome (`stayed = Some(mi)`
            // below, the same transient per-turn parameter a landed talk
            // uses — never stored, never hashed). Separately, an
            // already-calm target short-circuits to "landed" (no double
            // roll needed); otherwise roll `receptivity` off `parley_rng`
            // ONLY (never combat/ai/worldgen — same channel discipline as
            // `try_talk_player`).
            let already_calm = self.monsters[mi].calm;
            let chance = receptivity(&self.monsters[mi], self);
            let landed = already_calm || self.parley_rng.range(0, 100) < chance;
            if landed && !already_calm {
                self.monsters[mi].calm = true;
                self.record_spare();
                self.carry_event(CarryEvent::SpareWitnessed);
            }
            if let Some(t) = rule.line {
                self.log(t.replace("{M}", name));
            }
            if landed {
                if let Some(t) = rule.line_becalmed {
                    self.log(t.replace("{M}", name));
                }
            }
            if rule.consumes {
                self.held.pop();
            }
            if !self.spend_turn(0) {
                return; // died in the dark on a give turn: lose beats anything else
            }
            // The player never moves during a give; the target is ALWAYS
            // stayed here regardless of landed/failed (batch 13 T2's whole
            // point — the stay is the guaranteed half of the bargain).
            // batch 13 T5: also excluded from `resolve_awe`'s awe/punish
            // bookkeeping (`Some(mi)` as the exclusion index too) — cheese's
            // guaranteed stay is a BRIBE, a separate mechanism from
            // standing-your-ground nerve, and must never incur the goblin's
            // holding-punish on top of it (see `resolve_awe`'s doc comment).
            self.monsters_act_and_resolve_awe(Some(mi), Some(mi), (self.px, self.py));
            return;
        }
        if rule.enrage {
            // Batch 13 T4 (story "potion is mammal-medicine"): the potion
            // sickens a non-mammal instead of healing it. Regard CRASHES
            // (saturating sub, never checked against `talk_threshold` —
            // this branch can never becalm) and NO heal is applied,
            // regardless of `heal_full` (unread here, by construction). The
            // target is left un-stayed (`None` below), so it swings
            // normally in `monsters_act`'s adjacent+sees branch this same
            // turn — the exact failed-talk retaliation path, monster ATK +
            // `combat_rng` only, never touching the player's own ATK.
            let before = self.monsters[mi].regard;
            self.monsters[mi].regard = if rule.regard_delta >= 0 {
                before.saturating_add(rule.regard_delta as u8)
            } else {
                before.saturating_sub((-rule.regard_delta) as u8)
            };
            if let Some(t) = rule.line {
                self.log(t.replace("{M}", name));
            }
            if rule.consumes {
                self.held.pop();
            }
            if !self.spend_turn(0) {
                return; // died in the dark on a give turn: lose beats anything else
            }
            // batch 13 T5: excluded from `resolve_awe`'s awe/punish
            // bookkeeping (`Some(mi)`) — the enrage give is documented to
            // land EXACTLY the failed-talk retaliation's single hit; without
            // this exclusion, a not-yet-fled, not-yet-held goblin/ogre
            // target would ALSO read as "held ground" (the player never
            // moves during a give) and double the hit via the punish path.
            self.monsters_act_and_resolve_awe(None, Some(mi), (self.px, self.py));
            return;
        }
        if rule.heal_full {
            self.monsters[mi].hp = GAME.monsters[kind as usize].hp;
        }
        let before = self.monsters[mi].regard;
        self.monsters[mi].regard = if rule.regard_delta >= 0 {
            before.saturating_add(rule.regard_delta as u8)
        } else {
            before.saturating_sub((-rule.regard_delta) as u8)
        };
        let regard = self.monsters[mi].regard;
        if !self.monsters[mi].calm && regard >= Monster::talk_threshold(kind) {
            self.monsters[mi].calm = true;
            self.record_spare();
            self.carry_event(CarryEvent::SpareWitnessed);
        }
        let line = match rule.line {
            Some(t) => String::from(t),
            None => {
                let v = self.flavor_rng.range(0, 2) as usize;
                GAME.monsters[kind as usize].talk_lines[3][v].replace("{M}", name)
            }
        };
        self.log(line);
        if rule.consumes {
            self.held.pop();
        }
        if !self.spend_turn(0) {
            return; // died in the dark on a give turn: lose beats anything else
        }
        // The player never moves during a give; batch 11 T2 fix round.
        // batch 13 T5: `Some(mi)` also excludes the give target from
        // `resolve_awe`'s awe/punish bookkeeping — no current row targets
        // an awe-able kind through this ordinary branch (RAT isn't
        // awe-able), but a future row that did would hit the same
        // double-hit hazard the cheese/enrage branches above were fixed
        // for, so the exclusion is applied here too, for free.
        self.monsters_act_and_resolve_awe(None, Some(mi), (self.px, self.py)); // giving isn't bump-attacking
    }

    /// USE: self-applies the top of `self.held` (batch 7 T2, story §5/§9-A's
    /// verb; §9-A's minimal inventory — LIFO, no grid UI). Input byte 15.
    /// Nothing held, or the held item's `ItemDef::on_use` is `None`
    /// (give-only items): no-op, no turn, one feedback line. A landed USE
    /// always costs a normal turn (no violence tax — using an item on
    /// yourself is not violence), applies the effect, logs `use_line`, and
    /// pops `held` (every `UseEffect` this batch consumes the item — see
    /// that enum's doc comment if a future non-consuming use is ever
    /// added).
    pub(crate) fn use_item(&mut self) {
        self.fx_hit = None;
        if self.dead || self.won {
            return;
        }
        let Some(&kind) = self.held.last() else {
            self.log(String::from(GAME.strings.use_empty_hands));
            return; // empty hands: no-op, no turn
        };
        let def = &GAME.items[kind as usize];
        let Some(effect) = def.on_use else {
            self.log(String::from(GAME.strings.use_no_effect));
            return; // this held item has no self-use: no-op, no turn
        };
        match effect {
            UseEffect::Heal(amount) => {
                let heal = amount.min(self.maxhp - self.hp);
                self.hp += heal;
            }
            UseEffect::Light(amount) => {
                self.light += amount;
            }
        }
        self.log(String::from(def.use_line));
        self.held.pop();
        if !self.spend_turn(0) {
            return; // died in the dark on a use turn: lose beats anything else
        }
        // The player never moves during a use; batch 11 T2 fix round.
        self.monsters_act_and_resolve_awe(None, None, (self.px, self.py)); // using an item on yourself isn't bump-attacking
    }

    /// PUT DOWN: byte 16 (batch 8 T1, story §9-D). Sets the carried
    /// objective on the player's own tile if held (`has_objective`) and the
    /// tile has no item already sitting on it (no stacking): the objective
    /// re-enters `self.items` at `(self.px, self.py)`, `has_objective`
    /// flips false — which automatically reverts the per-turn light burn to
    /// `BalanceDef::base_burn` (`Game::spend_turn` already branches on
    /// `has_objective`, so no separate code is needed for the burn-rate
    /// flip in either direction) — and `objective_dropped` flips true (so a
    /// later walk-over re-pickup fires `CarryEvent::PickedBackUp`, not
    /// `PickedUpBloody`/`PickedUpMerciful` — see `Game::pickup`). Put-down is legal anywhere the
    /// tile is otherwise empty of items; a run that abandons the objective
    /// and never returns for it simply cannot win (the win check already
    /// requires `has_objective` — see `Game::land_on_tile`'s `Tile::
    /// UpStairs` arm). Not carrying, or a tile that already has an item on
    /// it, is a graceful no-op: one feedback line, no turn.
    ///
    /// `CarryEvent::PutDown` fires BEFORE `has_objective` flips to false —
    /// symmetric with `Game::pickup`'s `ItemEffect::Objective` arm, which
    /// sets `has_objective` true BEFORE calling `carry_event` for the same
    /// reason: `Game::carry_event`'s own guard requires `has_objective` to
    /// currently be true, and at the instant of a put-down the player is,
    /// until this call, still carrying it.
    pub(crate) fn put_down(&mut self) {
        self.fx_hit = None;
        if self.dead || self.won {
            return;
        }
        if !self.has_objective {
            self.log(String::from(GAME.strings.put_down_nothing_carried));
            return; // nothing to set down: no-op, no turn
        }
        if self.items.iter().any(|it| it.x == self.px && it.y == self.py) {
            self.log(String::from(GAME.strings.put_down_occupied));
            return; // no stacking: no-op, no turn
        }
        self.carry_event(CarryEvent::PutDown);
        self.items.push(Item { x: self.px, y: self.py, kind: GAME.win.objective_item });
        self.has_objective = false;
        self.objective_dropped = true;
        self.log(String::from(GAME.strings.put_down_ok));
        if !self.spend_turn(0) {
            return; // died in the dark on a put-down turn: lose beats anything else
        }
        // The player never moves during a put-down; batch 11 T2 fix round.
        self.monsters_act_and_resolve_awe(None, None, (self.px, self.py)); // setting the objective down isn't bump-attacking
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
        // batch 12 R3 ("light as grace" — the grace half): rest, gated on a
        // clear moment. Portal-footing guard: this call site is only
        // reached when `transiting` is false — a wait on a portal tile
        // ALWAYS transits (see above) and returns before ever reaching
        // here, so rest never entangles with transit; that's the clean
        // rule (a transiting turn ends the level/world, so healing it has
        // no meaning). Sequenced before `carry_event`/`monsters_act` so the
        // heal reads the adjacency state the player actually decided to
        // wait in, not a state monsters have since chased into.
        let mended = self.rest_heal();
        // batch 8 T1 / batch 12 R7: a plain wait (not a portal transit) while
        // carrying is the McGuffin's chance to speak. A wait that actually
        // MENDS is her tending you, mood-keyed by her visible shine: a wide
        // ring (radius >= 4, i.e. mood > 50 / 51+) is `RestedBright` ("light to spare");
        // a dim/dark McGuffin is `RestedDim` (she has little glow left, yet
        // rest heals regardless of her shine). A wait that heals nothing
        // (full hp, or a hostile blocking the rest) falls to the plain
        // standing-still `Idle` comment. `carry_event` no-ops unless carrying,
        // so this whole branch is silent before pickup.
        if mended && mood_shine_radius(self.mood()) >= 4 {
            self.carry_event(CarryEvent::RestedBright);
        } else if mended {
            self.carry_event(CarryEvent::RestedDim);
        } else {
            self.carry_event(CarryEvent::Idle);
        }
        // batch 11 T2: this is the standard "stand tall" turn — waiting
        // adjacent to an awe-able monster without attacking it. The player
        // never moves during a wait; batch 11 T2 fix round.
        self.monsters_act_and_resolve_awe(None, None, (self.px, self.py));
    }

    /// Rest (batch 12 R3, "light as grace"): waiting while hurt heals
    /// `BalanceDef::rest_heal` HP, but ONLY when no non-calm monster is
    /// Chebyshev-adjacent (N/S/E/W AND the four diagonals — the same shape
    /// `monsters_act` uses to decide whether a monster attacks, see its
    /// `dist == 1` check). That gate is REQUIRED, not an optimization: it's
    /// what keeps rest and awe-holding (`resolve_awe`, batch 11 T2 — "stand
    /// tall while an ogre pummels you") as two distinct acts. Awe-holding
    /// needs sustained adjacency to an awe-able threat; rest needs the
    /// opposite, a genuinely clear moment, so a player can't earn both from
    /// the same stationary turn. A calm monster never attacks, so it
    /// doesn't block rest. Only ever called from `wait_turn`'s
    /// non-transiting branch — see the portal-footing note there for why a
    /// wait on a portal tile can never reach this call at all.
    ///
    /// batch 12 R3 fix round: this gate originally tested CARDINAL
    /// adjacency only, but `monsters_act`'s attack decision (and the chase
    /// AI that parks a monster next to the player) uses CHEBYSHEV
    /// adjacency, which includes the four diagonals. A monster parked
    /// diagonally adjacent would attack every turn while this cardinal-only
    /// check saw no hostile neighbor and healed anyway — free healing under
    /// a live attacker, defeating the whole point of the gate. Fixed to use
    /// the identical Chebyshev formula `monsters_act` uses.
    /// Returns true iff the rest actually mended hp (batch 12 R7: the caller
    /// in `wait_turn` uses this to decide whether the McGuffin tends you with
    /// a mood-keyed `RestedBright`/`RestedDim` line, vs the plain `Idle`
    /// standing-still comment — a rest that heals nothing earns no tending
    /// line). `rest_heal > 0` and the `hp < maxhp` guard above together mean
    /// a return past both early-outs always raised hp, so `true` is exact.
    fn rest_heal(&mut self) -> bool {
        if self.hp >= self.maxhp {
            return false; // don't heal a corpse... or anyone already topped up
        }
        // `passive` (TRAINER/DONKEY, batch 9 T1) monsters never fight —
        // same exclusion `monsters_act` already applies before its own
        // attack/chase decision — so one standing adjacent isn't a
        // "hostile" for this gate's purposes and doesn't block rest. A
        // `calm` monster is excluded for the same reason (a becalmed
        // monster never attacks either); only a live, non-calm,
        // fight-capable monster in melee range (Chebyshev distance 1)
        // blocks the heal.
        let hostile_adjacent = self.monsters.iter().any(|m| {
            !m.calm
                && !Monster::stats(m.kind).passive
                && (m.x - self.px).abs().max((m.y - self.py).abs()) == 1
        });
        if hostile_adjacent {
            return false;
        }
        self.hp = (self.hp + GAME.balance.rest_heal).min(self.maxhp);
        true
    }

    /// Shared tail for any player action that LANDS the player on
    /// `(nx, ny)` — a normal move onto floor, or a becalmed-monster swap
    /// (batch 5) — both of which spend a turn (no tax), fire room-entry/
    /// pickup/stairs-transition handling, then resume monster turns and
    /// refresh FOV. `stayed` is forwarded to `monsters_act` untouched (see
    /// its doc comment); both call sites here pass `None` since neither a
    /// move nor a swap is a talk. `prev_player` (batch 11 T2 fix round) is
    /// the player's position BEFORE this turn's own move — forwarded
    /// untouched to `resolve_awe` via `monsters_act_and_resolve_awe`, since
    /// by the time this function runs `self.px`/`self.py` are already
    /// `(nx, ny)`.
    fn land_on_tile(&mut self, nx: i32, ny: i32, stayed: Option<usize>, prev_player: (i32, i32)) {
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
            Tile::ScreenLink(east) => {
                // batch 9 T1 (SIGN-OFF ASK #2): instant walk-onto, exactly
                // like Stairs/UpStairs above — never a describe-then-wait
                // like Portal.
                self.cross_screen_link(ny, east);
                return; // fresh screen: monsters don't get a free hit
            }
            Tile::Hole => {
                self.cross_into_dungeon();
                return; // fresh world: monsters don't get a free hit
            }
            _ => {}
        }
        // batch 11 T2: covers a plain move onto floor and a becalmed-yield
        // swap (`Game::try_move_player`'s `BumpResponse::Yield`/`calm`
        // branch) — neither is a bump-attack, so `None`.
        self.monsters_act_and_resolve_awe(stayed, None, prev_player);
    }

    /// The McGuffin's voice (batch 8 T1, story §9-B/C/D): dispatch one
    /// `CarryEvent` against `GAME.carried_lines`. Semantics, in this exact
    /// order (the ordering is what keeps this a provable no-op when the
    /// active cartridge's table is empty, as it is in this batch):
    ///   1. Not currently carrying the objective (`!self.has_objective`):
    ///      return immediately. Events only speak while the objective is
    ///      carried.
    ///   2. No row in `GAME.carried_lines` matches `ev`, or its pool is
    ///      empty: return immediately — BEFORE the rate-limit check, BEFORE
    ///      touching `flavor_rng`, BEFORE mutating any field. This is the
    ///      load-bearing step: this cartridge ships `carried_lines: &[]`
    ///      (T2 fills it), so with today's data EVERY call below returns
    ///      here, drawing no RNG and mutating nothing — the reason
    ///      `--solve`/`--sim`/goldens/frames/xhash all stay byte-identical
    ///      to the pre-batch-8 baseline despite every call site being wired
    ///      live.
    ///   3. Rate limit: except `CarryEvent::PickedUpBloody`/
    ///      `PickedUpMerciful`/`PutDown` (which always speak — picking up or
    ///      setting down the objective is always news), suppress if fewer
    ///      than `WinDef::carry_line_rate_limit` turns have passed since the
    ///      last McGuffin line actually spoken (`self.mcguffin_last_line_turn`;
    ///      `self.turns` is already current at every call site below, since
    ///      each one fires after that turn's own `spend_turn` already ran).
    ///   4. Pick a line: `CarryEvent::StairsUp` indexes the pool by
    ///      `self.speech_attempts` (the climb re-entry ladder), clamped to
    ///      the pool's length; every other event picks via `self.
    ///      flavor_rng` (replay-safe — the same channel the tier-warning/
    ///      adjective picks already use, never combat/ai/parley/worldgen).
    ///   5. Log the chosen line and record this turn as the last one
    ///      spoken.
    pub(crate) fn carry_event(&mut self, ev: CarryEvent) {
        if !self.has_objective {
            return;
        }
        let Some((_, pool)) = GAME.carried_lines.iter().find(|(e, _)| *e == ev) else {
            return;
        };
        if pool.is_empty() {
            return;
        }
        let always_speaks = matches!(
            ev,
            CarryEvent::PickedUpBloody | CarryEvent::PickedUpMerciful | CarryEvent::PutDown
        );
        if !always_speaks {
            if let Some(last) = self.mcguffin_last_line_turn {
                if self.turns.saturating_sub(last) < GAME.win.carry_line_rate_limit {
                    return;
                }
            }
        }
        let i = if ev == CarryEvent::StairsUp {
            (self.speech_attempts as usize).min(pool.len() - 1)
        } else {
            self.flavor_rng.range(0, pool.len() as i32) as usize
        };
        let line = String::from(pool[i]);
        self.log(line);
        self.mcguffin_last_line_turn = Some(self.turns);
    }

    /// batch 7 T2 (story §9-A's minimal inventory): a `Hold`-behavior item
    /// (`ItemDef::on_pickup`) is pushed onto `self.held` and its
    /// `pickup_line` is logged verbatim — no `ItemEffect` is applied at
    /// walk-over time; that waits for a later GIVE (`Game::try_give_player`)
    /// or USE (`Game::use_item`). A `Consume` item keeps the original v0/v1
    /// walk-over behavior unchanged.
    ///
    /// batch 8 T1 fix-round (story §9-C): a FIRST pickup of the win-
    /// condition item (`ItemEffect::Objective`, `!was_dropped`) additionally
    /// logs `GAME.carried_preamble` verbatim in order, then dispatches
    /// `CarryEvent::PickedUpBloody` or `PickedUpMerciful` depending on
    /// whether `self.kills > self.spared` — the pickup register keyed to
    /// the carrier's kill/spare record. A re-pickup after `put_down` never
    /// repeats the preamble or the register (`PickedBackUp` only).
    ///
    /// `pub(crate)` and pulled out as a pure, no-`self`, no-RNG predicate
    /// (batch 8 T1 fix-round) specifically so its selection logic is
    /// unit-testable on its own — `Game::carry_event`'s dispatch is a
    /// provable no-op while `GAME.carried_lines` is empty (this cartridge,
    /// T2 fills it), so a black-box test driving a real pickup can't yet
    /// observe which variant was chosen; this predicate can be checked
    /// directly regardless.
    pub(crate) fn pickup_register_event(kills: u32, spared: u32) -> CarryEvent {
        if kills > spared { CarryEvent::PickedUpBloody } else { CarryEvent::PickedUpMerciful }
    }

    /// batch 12 R4 (the pickup verdict): the FIRST-pickup mood anchor
    /// score, seeded from this same kill/spare record — pure, no-`self`,
    /// no-RNG, pulled out for the same testability reason as
    /// `pickup_register_event` right above. `50` (neutral) if the player
    /// neither fought nor talked before pickup, else `100 * spared /
    /// (kills + spared)` — `0` = pure brute, `100` = pure diplomat. Scaled
    /// by `BalanceDef::mood_anchor_weight` at the call site
    /// (`Game::pickup`), not here — this function is the ratio alone.
    pub(crate) fn anchor_score(kills: u32, spared: u32) -> i32 {
        if kills + spared == 0 {
            50
        } else {
            (100 * spared as i64 / (kills + spared) as i64) as i32
        }
    }

    fn pickup(&mut self) {
        if let Some(i) = self.items.iter().position(|i| i.x == self.px && i.y == self.py) {
            let kind = self.items[i].kind;
            self.items.remove(i);
            let def = &GAME.items[kind as usize];
            if def.on_pickup == PickupBehavior::Hold {
                self.held.push(kind);
                self.log(String::from(def.pickup_line));
                return;
            }
            match def.effect {
                ItemEffect::AtkBonus(n) => {
                    self.atk += n;
                    let a = self.adj();
                    self.log(GAME.strings.atk_item.replacen("{}", a, 1).replacen("{}", &n.to_string(), 1));
                }
                ItemEffect::Objective => {
                    // batch 8 T1: `objective_dropped` is only ever true
                    // after a prior `Game::put_down` — a re-pickup fires
                    // `PickedBackUp` instead of the FIRST-pickup register.
                    // `has_objective` is set BEFORE `carry_event` is called
                    // (below) so its is-currently-carried guard passes.
                    let was_dropped = self.objective_dropped;
                    self.has_objective = true;
                    self.objective_dropped = false;
                    let name = self.theme().objective_name;
                    self.log(GAME.strings.pickup_objective.replace("{}", name));
                    if was_dropped {
                        self.carry_event(CarryEvent::PickedBackUp);
                    } else {
                        // batch 12 R4: seed the McGuffin's mood anchor from
                        // this descent's kill/spare record — this branch
                        // (`!was_dropped`) is the TRUE first pickup, reached
                        // exactly once per run (a re-pickup after `put_down`
                        // takes the `PickedBackUp` arm above instead), so no
                        // extra `mood_count == 0` guard is needed here: this
                        // IS the batch-8 first-pickup guard, reused per the
                        // brief. Sequenced before the preamble/register below
                        // so `Game::mood` is already meaningful the instant
                        // the pickup's own log lines are written.
                        self.mood_sum =
                            GAME.balance.mood_anchor_weight * Self::anchor_score(self.kills, self.spared);
                        self.mood_count = GAME.balance.mood_anchor_weight;
                        // batch 8 T1 fix-round (story §9-C, the pickup
                        // register): the FIRST objective pickup logs the
                        // fixed `carried_preamble` verbatim, in order, no
                        // RNG, no rate-limit — same fixed-string precedent
                        // as `pickup_objective` above, just multi-line —
                        // THEN dispatches the kill/spare-keyed register
                        // event (see `pickup_register_event`'s doc comment).
                        for line in GAME.carried_preamble {
                            self.log(String::from(*line));
                        }
                        self.carry_event(Self::pickup_register_event(self.kills, self.spared));
                    }
                }
                ItemEffect::Lore(tier) => {
                    let line = self.lore_line(tier as usize);
                    self.log(String::from(GAME.strings.lore_prefix));
                    self.log(line);
                }
                // Unreachable in practice: every `Hold` row (the only place
                // `ItemEffect::None` appears) returns above before this
                // match is ever reached. Kept as an explicit arm so this
                // match stays exhaustive against the engine-primitive enum,
                // not silently reliant on a cartridge never doing something
                // unexpected.
                ItemEffect::None => {}
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
            if self.monsters[i].calm || Monster::stats(self.monsters[i].kind).passive {
                // Becalmed (batch 5): never attacks, never chases — the
                // simplest deterministic option per the batch-5 plan's
                // Design (decided) section is to stand. Skipped every
                // turn, forever, once calm. batch 9 T1: a `passive` kind
                // (the TRAINER/DONKEY) gets the identical skip from spawn,
                // unconditionally — never chases, never attacks, regardless
                // of `regard`/`calm` (see `MonsterDef::passive`'s doc
                // comment).
                //
                // R5+ HOOK (batch 12 R4's brief, NOT built this task — see
                // `Game::mood_sum`'s doc comment): "every time she sees a
                // becalmed monster, she likes you that much more" — a
                // bounded, once-per-monster (hashed "already greeted"
                // flag, farming-guarded like `Monster.awe`'s cap) mood lift
                // for passing a becalmed monster within the McGuffin's
                // shine radius while carrying. This is the ONLY place a
                // becalmed monster is visited at all before this early
                // `continue` — the detection has to be gated on
                // `self.has_objective` and land HERE, ahead of this line,
                // since a `calm` monster never reaches the dist/sees check
                // below. Left as a comment: this task ships the anchor seed
                // (`Game::pickup`) and the kill/spare valences
                // (`Game::kills`'s call site, `Game::record_spare`) only.
                continue;
            }
            let (mx, my) = (self.monsters[i].x, self.monsters[i].y);
            let dist = (px - mx).abs().max((py - my).abs());
            let sees = self.monster_sees_player(&self.monsters[i]);
            if dist == 1 && sees {
                // batch 8 T1: the McGuffin may react to a monster now
                // standing right next to you, win-or-lose-of-this-turn
                // aside — fired once per adjacent-and-seeing monster this
                // pass, same as the attack/stay decision right below it.
                self.carry_event(CarryEvent::MonsterAdjacent);
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

    /// Shared tail for every player action that runs monster turns: takes
    /// the pre-chase monster-position snapshot `resolve_awe` needs (batch 11
    /// T2 fix round), runs `monsters_act`, resolves awe from that snapshot,
    /// then refreshes FOV. Every call site that used to pair
    /// `self.monsters_act(..)` with `self.resolve_awe(..)` (plus the
    /// trailing `self.compute_fov()` — all three always ran back-to-back)
    /// now goes through here instead, so a future call site can't
    /// accidentally read post-chase monster positions into the hold-vs-flee
    /// decision the way the original bug did.
    ///
    /// `stayed`/`attacked` are forwarded to `monsters_act`/`resolve_awe`
    /// untouched (see their own doc comments). `prev_player` is the
    /// player's position at the START of this turn, i.e. before this
    /// turn's own move (or unchanged from the current position, for any
    /// action that doesn't move the player) — see `resolve_awe`'s doc
    /// comment for why this must be captured before monster positions get
    /// mutated below.
    fn monsters_act_and_resolve_awe(&mut self, stayed: Option<usize>, attacked: Option<usize>, prev_player: (i32, i32)) {
        // Snapshot every monster's position BEFORE `monsters_act` chases —
        // this is "the ogre's position at the start of the turn" the
        // Design paragraph measures retreat against. Indices stay aligned
        // with `self.monsters` across the `monsters_act` call below:
        // monsters are only ever removed by the player's own attack (which
        // already happened, earlier in the same turn, before this snapshot
        // is taken), never by `monsters_act` itself.
        let pre_chase: Vec<(i32, i32)> = self.monsters.iter().map(|m| (m.x, m.y)).collect();
        self.monsters_act(stayed);
        self.resolve_awe(attacked, prev_player, &pre_chase);
        self.resolve_becalm_dividend();
        self.overworld_follow_step();
        self.compute_fov();
    }

    /// Overworld-only follow step (batch 13 T6, the donkey-follow seed, rung
    /// 2 of the arc doc's three-rung kindness ladder): every monster that is
    /// BOTH `Monster.calm` (the SAME hashed field a talk-becalm already
    /// sets — no new state invented) and `MonsterDef::follows_when_calm`
    /// takes one greedy step toward the player, once per overworld turn.
    /// Deliberately separate from `Game::monsters_act`'s own per-monster
    /// loop (which skips a calm/passive monster outright, unconditionally,
    /// and stays that way — this is a distinct, overworld-only movement
    /// pass, not a change to the existing hostile-AI behavior). A no-op
    /// outside the overworld — irrelevant there anyway, since nothing sets
    /// the flag on a dungeon-only kind. Called from the shared
    /// `monsters_act_and_resolve_awe` tail, so every turn-advancing action
    /// (move/wait/talk/give/use/put-down/screen-link) carries it; never
    /// called on a hole-crossing, since `Game::cross_into_dungeon` doesn't
    /// route through this method at all (the follower is simply never
    /// pulled out of the overworld's monster list — see `WorldId::Overworld`
    /// and this batch's status-log entry for why that's the whole
    /// "won't take the hole" mechanism, not a special case here).
    fn overworld_follow_step(&mut self) {
        if self.world != WorldId::Overworld {
            return;
        }
        let (px, py) = (self.px, self.py);
        for i in 0..self.monsters.len() {
            if !self.monsters[i].calm || !Monster::stats(self.monsters[i].kind).follows_when_calm {
                continue;
            }
            let (mx, my) = (self.monsters[i].x, self.monsters[i].y);
            if (mx, my) == (px, py) {
                continue; // defensive: never expected to already be on the player's tile
            }
            let (dx, dy) = ((px - mx).signum(), (py - my).signum());
            // Diagonal step first, then each axis alone — same fallback
            // order `monsters_act`'s own chase step uses.
            for (tx, ty) in [(mx + dx, my + dy), (mx + dx, my), (mx, my + dy)] {
                if (tx, ty) == (mx, my) || (tx, ty) == (px, py) {
                    continue;
                }
                if in_map(tx, ty)
                    && self.map[idx(tx, ty)] != Tile::Wall
                    && self.map[idx(tx, ty)] != Tile::Pit
                    && !self.monsters.iter().any(|m| m.x == tx && m.y == ty)
                    && !self.blocks.iter().any(|&b| b == (tx, ty))
                {
                    self.monsters[i].x = tx;
                    self.monsters[i].y = ty;
                    break;
                }
            }
        }
    }

    /// Standing tall / giving ground (batch 11 T2 the ogre; batch 13 T5 the
    /// goblin mirror + the paired punish hits — arc doc "Goblinoid awe —
    /// becalm through nerve, not talk"): builds `Monster.awe` for every
    /// awe-able monster (`MonsterDef::awe_threshold > 0`) that's not already
    /// `calm`, once per player action — never called directly; always via
    /// `monsters_act_and_resolve_awe`. `attacked` carries the index of a
    /// monster EXCLUDED from both building awe and the punish hit below —
    /// originally (batch 11) only a monster the player just bump-ATTACKED
    /// this turn; batch 13 T2/T4 widened the same exclusion to a GIVE
    /// target (`try_give_player`'s cheese stay-and-roll and potion-enrage
    /// branches also pass their `mi` here) — a give is its own bribe/trap
    /// mechanism, not a standing-your-ground nerve tactic, and must never
    /// double up with the generic held/gave-ground read below (a give never
    /// moves the player, so without this exclusion the target would always
    /// read as "held ground" — the wrong move for a goblin, punished on top
    /// of whatever the give itself already does). `None` from every other
    /// action — talk/use/put-down/wait/a plain move/a becalmed-yield swap
    /// pass `None` (talk is deliberately NOT excluded — talking at an
    /// awe-able monster IS meant to read as holding ground, per batch 11
    /// T3's "talk is a no-move action").
    ///
    /// **Batch 11 T2 fix round** (review-found bug, preserved exactly):
    /// held-vs-fled must be decided from data that PREDATES `monsters_act`'s
    /// chase — `pre_chase[i]` (the monster's position before `monsters_act`
    /// moved it this turn, supplied by `monsters_act_and_resolve_awe`) and
    /// `prev_player` (the player's own position before THIS turn's move) —
    /// never `self.monsters[i]`'s position AT CALL TIME (post-chase), or a
    /// straight-line retreat at the same speed as a chasing monster
    /// re-establishes adjacency every turn and manufactures "holding ground"
    /// (or, for the goblin below, "never gave ground") for free.
    ///
    /// Two per-turn distance reads off that pre-chase snapshot, generic
    /// across every awe-able kind:
    ///   - `gave_ground`: was cardinally adjacent last turn (`old_dist == 1`)
    ///     and the player's own move strictly INCREASED Manhattan distance
    ///     to `pre_chase[i]` this turn — "stepped away while composed."
    ///   - `held_adjacent`: WAS cardinally adjacent last turn (`old_dist ==
    ///     1`, same precondition as `gave_ground`) AND the player's own
    ///     position is UNCHANGED this turn (two T5 fix rounds — see
    ///     `held_adjacent`'s own let-binding below for why both are
    ///     required, not just `new_dist == 1` alone) — "stood planted." A
    ///     fresh approach, a walk-past that merely happens to stay
    ///     equidistant, or any other ordinary move is neither of these two
    ///     — a neutral non-event, not scored as either move.
    /// Both exclude a monster bump-attacked this turn (`attacked != Some(i)`
    /// — attacking resets awe regardless, same as before batch 13).
    ///
    /// **batch 13 T5, the generic model**: `MonsterDef::awe_by_giving_ground`
    /// picks which of the two IS this kind's awe move
    /// (`did_awe_move`) and which is its mirror-opposite, punishable move
    /// (`did_punished_move`) — `false` (the ogre, unchanged from batch 11):
    /// awe move = `held_adjacent`, punished move = `gave_ground` (fleeing).
    /// `true` (the goblin): awe move = `gave_ground`, punished move =
    /// `held_adjacent` (standing your ground). Neither flag nor this
    /// function names a specific kind — `contractor.rs`'s cartridge data
    /// is what makes one kind "the ogre" and the other "the goblin."
    ///
    /// Doing the awe move: `awe += 1`; crossing `awe_threshold` becalms it
    /// exactly like a landed talk (`calm = true`, `Game::record_spare()`,
    /// batch 12 T2: also feeds the torch) — reusing the existing becalm
    /// state rather than a parallel mechanism, so every downstream mercy
    /// behavior (no chase/attack, yield-on-bump) works unchanged. The log
    /// line reuses the monster's own `talk_lines` stage-2 pool (the
    /// "crosses the threshold" stage already used by a landed talk/give)
    /// rather than inventing new grounded copy. `CarryEvent::SpareWitnessed`
    /// fires too, the same hook every other spare path already fires on
    /// becalming.
    ///
    /// Doing anything else: `awe` resets to 0 (the stare/composure breaks).
    /// If specifically the OPPOSITE, punishable move was made
    /// (`did_punished_move`) and this kind's `MonsterDef::punish_wrong_move`
    /// is set, it lands an explicit punishing hit on the player — the
    /// monster's ordinary swing formula (`atk` + a `combat_rng` draw), never
    /// a new damage path (mirrors the failed-talk/`monsters_act` player-hit
    /// and T4's potion-enrage free swing). This hit is necessary — not
    /// redundant with `monsters_act`'s own adjacent-attack — because the two
    /// fire under different conditions: `monsters_act` only attacks a
    /// monster that ends up BOTH adjacent AND not `stayed` this same turn
    /// (e.g. a bare wait next to a goblin already takes that hit); a
    /// STAYED monster (a landed talk) never attacks via `monsters_act`, so
    /// holding your ground against a goblin via talk would otherwise cost
    /// nothing — this hit closes that gap. A guaranteed-lethal punish is
    /// handled exactly like `monsters_act`'s own attack-death and batch 11's
    /// retaliation-death: `killer` set to this monster, no further monsters
    /// processed this call.
    ///
    /// **Death guard**: if the player is already `dead` (from `monsters_act`
    /// having just killed them earlier in this same
    /// `monsters_act_and_resolve_awe` call), this function is a no-op —
    /// awe/punish bookkeeping must never mutate hp/`killer` after the fatal
    /// blow already landed and was attributed.
    ///
    /// Ordering note (unchanged from batch 11): the actual becalm/punish
    /// still resolves AFTER `monsters_act` runs (via
    /// `monsters_act_and_resolve_awe`'s call order) — a not-yet-calm monster
    /// still gets its own `monsters_act` turn (attack or chase) before this
    /// resolves. Only the DATA the move-read decision uses is pre-chase, not
    /// the call's position in the turn sequence.
    fn resolve_awe(&mut self, attacked: Option<usize>, prev_player: (i32, i32), pre_chase: &[(i32, i32)]) {
        if self.dead {
            return;
        }
        for i in 0..self.monsters.len() {
            let kind = self.monsters[i].kind;
            let stats = Monster::stats(kind);
            let threshold = stats.awe_threshold;
            if threshold == 0 || self.monsters[i].calm {
                continue; // not awe-able, or already calmed some other way
            }
            let (mx, my) = pre_chase[i];
            let old_dist = (prev_player.0 - mx).abs() + (prev_player.1 - my).abs();
            let new_dist = (self.px - mx).abs() + (self.py - my).abs();
            let gave_ground = old_dist == 1 && new_dist > old_dist && attacked != Some(i);
            // batch 13 T5 fix round (found via sim measurement, same
            // discipline as batch 11 T2's own fix round): `held_adjacent`
            // requires `old_dist == 1` too — i.e. the player must have
            // ALREADY been cardinally adjacent last turn, not merely have
            // arrived at adjacency this turn by approaching from farther
            // away. Without this, a single step that closes distance from
            // outside melee range straight to adjacent (`old_dist > 1,
            // new_dist == 1`) satisfies the bare `new_dist <= old_dist`
            // reading — harmless for the ogre (approaching it was already
            // its rewarded move pre-batch-13), but for the goblin
            // (`held_adjacent` is now the PUNISHED move) it meant every
            // first approach to a goblin — an unavoidable, one-time event
            // for any route that passes it — was scored identically to
            // "planting yourself and refusing to give ground," punishing
            // mere proximity before the player ever had a turn to react.
            // Measured effect before this fix: `--sim 5000 --policy
            // tactical-pacifist` collapsed from a healthy diplomat-favors
            // mercy result to 30/5000 wins (0.6%, deaths_combat 4965) —
            // clearly a bug, not "diplomacy is harder now." Requiring
            // `old_dist == 1` makes a fresh approach a neutral non-event
            // (falls through to the `else` branch below with no punish,
            // same as any other non-awe non-punish turn) and restores the
            // symmetric shape with `gave_ground` above (both now require
            // "was already toe-to-toe last turn").
            //
            // **Second fix round finding, also via sim measurement**: even
            // with `old_dist == 1` required, `held_adjacent` still read
            // "STAYED at distance <= 1" purely by the DISTANCE metric,
            // which a player who is genuinely MOVING can satisfy for many
            // consecutive turns — e.g. walking down a corridor that runs
            // exactly one tile from a stationary goblin's alcove keeps
            // Manhattan distance 1 to it turn after turn, even though the
            // player is making real progress, not "planting" anything.
            // `held_adjacent` must mean STANDING STILL ("a bare wait...
            // stand planted," "the planted target" — arc doc), so it now
            // also requires the player's own position to be UNCHANGED this
            // turn (`(self.px, self.py) == prev_player`) — true only for a
            // genuine no-move action (wait/talk/give/use/put-down), never
            // an ordinary move, however the distance metric alone reads.
            // Measured effect before this second fix: `--sim 5000 --policy
            // tactical-pacifist` was still catastrophically down at
            // ~150-190/5000 (versus its 2985/5000 pre-batch-13 baseline)
            // purely from routine corridor traffic near goblins, nothing to
            // do with any bot mistake or the punish mechanic's intended
            // scope at all.
            let held_adjacent =
                old_dist == 1 && new_dist <= old_dist && attacked != Some(i) && (self.px, self.py) == prev_player;
            // batch 13 T5 (mandatory telegraphing, arc doc §315): a legible
            // per-kind cue whenever the player ends this turn cardinally
            // adjacent to a not-yet-calm awe-able monster, regardless of
            // which move they made — so the tell is seen BEFORE a
            // wrong-move death, not only after.
            if new_dist == 1 && !stats.awe_tell.is_empty() {
                self.log(String::from(stats.awe_tell));
            }
            let did_awe_move = if stats.awe_by_giving_ground { gave_ground } else { held_adjacent };
            let did_punished_move = if stats.awe_by_giving_ground { held_adjacent } else { gave_ground };
            if did_awe_move {
                self.monsters[i].awe = self.monsters[i].awe.saturating_add(1);
                if self.monsters[i].awe >= threshold {
                    self.monsters[i].calm = true;
                    self.record_spare();
                    let name = self.mob_name(kind);
                    let v = self.flavor_rng.range(0, 2) as usize;
                    let line = GAME.monsters[kind as usize].talk_lines[2][v].replace("{M}", name);
                    self.log(line);
                    self.carry_event(CarryEvent::SpareWitnessed);
                }
            } else {
                self.monsters[i].awe = 0; // composure breaks
                if did_punished_move && stats.punish_wrong_move {
                    let name = self.mob_name(kind);
                    let dmg = stats.atk + self.combat_rng.range(0, 2);
                    self.hp -= dmg;
                    self.fx_hit = Some((self.px, self.py));
                    if self.hp <= 0 {
                        self.hp = 0;
                        self.dead = true;
                        self.killer = Some(name);
                        self.log(GAME.strings.killed_by.replace("{}", name));
                        return; // mirrors monsters_act's own attack-death early return
                    }
                    self.log(GAME.strings.hit_by.replacen("{}", name, 1).replacen("{}", &dmg.to_string(), 1));
                }
            }
        }
    }

    /// The becalm return-trip dividend (batch 13 T3, arc doc §215/§"Becalm
    /// return-trip dividend"): a becalmed monster (`Monster.calm`) that the
    /// player ends a turn cardinally adjacent to refunds a small, ONE-TIME
    /// light trickle — "it remembers you; it lights the way." Guarded by
    /// the hashed per-`Monster` `dividend_paid` flag so walking back and
    /// forth past the same becalmed monster can't farm light — this is
    /// exactly the lever the arc doc's §215 flags as the one that made
    /// pacifism DOMINANT in batch 5 (the guaranteed stayed swing), so the
    /// per-monster once-only cap is load-bearing, not decorative, and the
    /// `[TUNE]` amount (`BalanceDef::becalm_dividend`) starts small and is
    /// measured, never hand-tuned. Called every turn via
    /// `monsters_act_and_resolve_awe`, right after `resolve_awe` — a calm
    /// monster never moves (`monsters_act` skips it outright), so checking
    /// final post-move positions here is equivalent to checking pre-chase
    /// ones; no snapshot is needed the way `resolve_awe` needs one.
    fn resolve_becalm_dividend(&mut self) {
        for i in 0..self.monsters.len() {
            let (calm, paid, mx, my, kind) = {
                let m = &self.monsters[i];
                (m.calm, m.dividend_paid, m.x, m.y, m.kind)
            };
            if !calm || paid {
                continue;
            }
            let dist = (self.px - mx).abs() + (self.py - my).abs();
            if dist != 1 {
                continue; // not cardinally adjacent this turn
            }
            self.monsters[i].dividend_paid = true;
            self.light += GAME.balance.becalm_dividend;
            let name = self.mob_name(kind);
            let line = String::from(GAME.strings.becalm_dividend).replace("{}", name);
            self.log(line);
        }
    }
}

impl Game {
    /// Input-byte vocabulary, 0-16 (save v5, batch 8 T1): 0-4 move/wait (see
    /// below), 5-6 are frontend/reconstruction-layer only (restart/retry —
    /// handled in `save::replay`, never reach here), 7-10 = talk-N/S/W/E,
    /// 11-14 = give-N/S/W/E, 15 = use, 16 = put-down — every directional
    /// pair's order mirrors the move bytes' 0-3 direction order exactly
    /// (see `Game::try_give_player`'s doc comment; the brief's original
    /// "11-12" numbering is revised to 11-14 give + 15 use so give stays a
    /// 4-byte directional block just like move/talk, rather than colliding
    /// one value short of that shape). 16 (put-down, batch 8 T1, story
    /// §9-D) is self-apply like 15, no direction — see `Game::put_down`.
    /// Any other byte is silently ignored (`_ => {}`) — this is the one
    /// place old logs (v1-v4, no byte 16) and any future build's own
    /// reserved bytes both fall through harmlessly.
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
            11 => self.try_give_player(0, -1),
            12 => self.try_give_player(0, 1),
            13 => self.try_give_player(-1, 0),
            14 => self.try_give_player(1, 0),
            15 => self.use_item(),
            16 => self.put_down(),
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
