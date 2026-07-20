// content.rs — the authored-content layer: per-depth themes, room kind/tone
// lines, and hand-built vault rooms. Pure data plus the theme_pick channel
// draw; the generator (game.rs) is a librarian over this table.

use crate::rng::channel;

// ---------- Themes ----------
/* Meaning is authored upstream; the generator is a librarian. Each depth
   draws its identity from this table via the per-depth "theme" channel.
   Grounding rule (template edition): flavor text may only restate things
   the engine did — never invent entities, exits, or events. Lore speaks of
   the place's past; it never claims something is present that isn't. */
pub(crate) struct Theme {
    pub(crate) label: &'static str,
    pub(crate) amulet: &'static str,
    pub(crate) mobs: [&'static str; 3], // skins for rat / goblin / ogre stat rows
    pub(crate) adjs: [&'static str; 4],
    pub(crate) lore: [&'static str; 3], // {A} is filled from slots
    pub(crate) slots: [&'static str; 4],
    pub(crate) wall: u32,  // 0xRRGGBB, map render color for Tile::Wall
    pub(crate) floor: u32, // 0xRRGGBB, map render color for Tile::Floor
}

pub(crate) const THEMES: [Theme; 4] = [
    Theme {
        label: "the drowned monastery",
        amulet: "the Quiet Bell",
        mobs: ["cloister rat", "drowned acolyte", "bell-warden"],
        adjs: ["water-stained", "hushed", "candle-blackened", "weeping"],
        lore: [
            "The Order raised these halls over the spring, {A}.",
            "The bell rang for the living, {A}. The water answered.",
            "The lower stairs were sealed {A}. Someone unsealed them.",
        ],
        slots: ["to count the dead hours", "in the wet year", "when the abbot went below", "against all writ"],
        wall: 0x6E96A0,
        floor: 0x3A5560,
    },
    Theme {
        label: "the salt counting-house",
        amulet: "the Final Ledger",
        mobs: ["salt rat", "clerk-thing", "debt-golem"],
        adjs: ["dust-dry", "ink-stained", "ledger-lined", "airless"],
        lore: [
            "The vaults run deep to keep the salt-debts cool, {A}.",
            "The clerks recorded debts before they were owed, {A}.",
            "On the last page is a sum still being paid, {A}.",
        ],
        slots: ["by royal writ", "in the ninth audit", "in the short harvest year", "and sealed twice"],
        wall: 0xA89878,
        floor: 0x5E5442,
    },
    Theme {
        label: "the deep mine",
        amulet: "the First Lode",
        mobs: ["blind rat", "ember wisp", "pit foreman"],
        adjs: ["soot-caked", "cold", "narrow", "groaning"],
        lore: [
            "They followed the seam past the marked depth, {A}.",
            "The singing shaft was sealed, {A}. Digging went on.",
            "The old galleries were abandoned in one shift, {A}.",
        ],
        slots: ["against the surveyor's oath", "in the dry season", "when the birds went quiet", "and told no one above"],
        wall: 0xA07862,
        floor: 0x54423A,
    },
    Theme {
        label: "the hollow library",
        amulet: "the Last Index",
        mobs: ["paper rat", "ink haunt", "shelf-warden"],
        adjs: ["dog-eared", "mould-spotted", "whispering", "unshelved"],
        lore: [
            "The stacks were carved downward when shelves ran out, {A}.",
            "The catalogue was burned by its own librarians, {A}.",
            "Borrowing ended {A}. Returns were still accepted.",
        ],
        slots: ["in the third founding", "one volume at a time", "after the misfiling", "by unanimous silence"],
        wall: 0x9080A8,
        floor: 0x4E4460,
    },
];

/* Room identity: every room gets a kind noun and a tone; the first time
   the player steps into a room, one tone line (with {K} filled) is logged.
   Same grounding rule as lore: atmosphere only, no invented entities.
   KINDS[6] ("vault") is never drawn randomly — it is forced onto stamped
   vault rooms. */
pub(crate) const KINDS: [&str; 7] =
    ["hall", "gallery", "cellar", "stairwell", "chapel", "storeroom", "vault"];
pub(crate) const TONE_LINES: [[&str; 2]; 4] = [
    // ominous
    [
        "Something in this {K} is waiting for you to leave.",
        "This {K} swallows the torchlight a little too eagerly.",
    ],
    // still
    [
        "Dust lies unbroken across this {K}.",
        "This {K} has been holding its breath for years.",
    ],
    // cold
    [
        "The chill of this {K} settles into your bones.",
        "Cold air pools in this {K} like standing water.",
    ],
    // watchful
    [
        "You feel counted as you cross this {K}.",
        "The walls of this {K} seem to lean in and listen.",
    ],
];

/// Per-depth theme identity: theme index plus one slot index per lore tier
/// (templates 0/1/2 = shallow/mid/deep — the story assembles in act order
/// as the player pushes deeper). Its own channel, so flavor can never
/// perturb worldgen or spawns.
pub(crate) fn theme_pick(seed: u64, depth: u32) -> (usize, [usize; 3]) {
    let mut tr = channel(seed, &["theme", &depth.to_string()]);
    let ti = tr.range(0, THEMES.len() as i32) as usize;
    let slots = [
        tr.range(0, 4) as usize,
        tr.range(0, 4) as usize,
        tr.range(0, 4) as usize,
    ];
    (ti, slots)
}

/// The theme for a given seed+depth — pure f(seed, depth), no RNG draws
/// beyond theme_pick's own channel. Free function (rather than a Game
/// method) so both `Game::theme` (live play) and the Title/End screens
/// (render.rs, which may want a depth other than the current run depth —
/// e.g. depth 1 for the title, MAX_DEPTH for the amulet's name) share one
/// implementation.
pub(crate) fn theme_for(seed: u64, depth: u32) -> &'static Theme {
    &THEMES[theme_pick(seed, depth).0]
}

/// The filled lore line for a given seed+depth+tier — pure f(seed, depth,
/// tier), no RNG draws. Shared by `Game::lore_line` (live pickup flavor)
/// and the Title screen (depth-1 tier-0 preview).
pub(crate) fn lore_line(seed: u64, depth: u32, tier: usize) -> String {
    let (ti, slots) = theme_pick(seed, depth);
    let t = &THEMES[ti];
    t.lore[tier].replace("{A}", t.slots[slots[tier]])
}

/* Mercy as talk (batch 5, DECISION.md item 3 — the Henson ruling: "if you
   could talk to a rat, you could give a rat mercy"; addendum, human
   direction: talk is now `game::receptivity`-rolled, not a guaranteed
   stay). Each talk (input bytes 7-10, game.rs `Game::try_talk_player`)
   draws one line here, keyed by `[MKind as usize][stage]` — stage 0 = the
   monster's first LANDED talk, 1 = a later landed talk before it calms,
   2 = the landed talk that crosses `Monster::talk_threshold` (also reused,
   unchanged, for any further talk on an already-calm monster), 3 = a
   FAILED roll (the monster is unmoved — grounded per the same rule below,
   voicing continued wariness, never a new invented event). Two variants
   per cell, picked via `flavor_rng` — same per-run, replay-safe channel
   and the same `TONE_LINES`-style variant-array shape as this file's other
   flavor tables, just with an extra (kind) dimension. `{M}` fills from the
   theme's own mob name for that kind (`Theme::mobs[kind as usize]`) — the
   only slot, so a monster is always named in its own theme's voice.
   Register, low tier to high: rats are simple, small, quick to flinch;
   goblins are wary, visibly weighing you; ogres are slow and heavy, even
   when they yield (or don't). Grounding rule (same as THEMES/TONE_LINES):
   a line may only voice want/fear about things the engine proves — the
   dark, your torch, its patch of the dungeon — never an invented event,
   never a promise the engine can't keep; a failed-stage line may say the
   monster is unmoved/still wary and nothing more. Length-tested
   (`talk_lines_fit_log_row` in main.rs) across every theme's mob-name
   filling. */
pub(crate) const TALK_LINES: [[[&str; 2]; 4]; 3] = [
    // rat
    [
        [
            "The {M} flinches from your torch and does not bite.",
            "The {M} freezes, small and shaking, and does not run.",
        ],
        [
            "The {M} still won't near the torch, but stays put.",
            "The {M} watches you, still too scared to come closer.",
        ],
        [
            "The {M} goes still. It won't cross you again.",
            "The {M} settles, calmer than the dark behind it.",
        ],
        [
            "The {M} bolts sideways, unconvinced, and won't come near.",
            "The {M} keeps its distance, still too wary of your torch.",
        ],
    ],
    // goblin
    [
        [
            "The {M} goes still, watching your torch instead of you.",
            "The {M} pauses mid-step, weighing the light in your hand.",
        ],
        [
            "The {M} is still weighing whether the dark is worse than you.",
            "The {M} keeps its distance, still deciding about you.",
        ],
        [
            "The {M} steps back. It has chosen you over the dark.",
            "The {M} lowers its guard for good. You are not its enemy now.",
        ],
        [
            "The {M} narrows its eyes and does not lower its guard.",
            "The {M} keeps weighing you, and the answer is still no.",
        ],
    ],
    // ogre
    [
        [
            "The {M} stands motionless, breathing slow in the torchlight.",
            "The {M} lowers one fist, unhurried, and studies your torch.",
        ],
        [
            "The {M} stays still, thinking, slowly.",
            "The {M} shifts its weight, in no hurry to fight you now.",
        ],
        [
            "The {M} lowers its fists and turns from the dark behind it.",
            "The {M} settles, heavy and calm, and stands down for good.",
        ],
        [
            "The {M} doesn't blink. Whatever you said, it isn't enough.",
            "The {M} stays wary, heavy and unconvinced, watching you.",
        ],
    ],
];

// ---------- Palette ----------
/* Every color literal used by game.rs/render.rs/the backends lives here as
   a named const, so there is one source of truth for "what color is a
   goblin" or "what color is the HP bar." Values are unchanged from their
   prior inline-literal call sites; only the bar/empty-segment colors are
   new (introduced by the status-bar sub-task). Theme wall/floor colors
   stay on `Theme` — they're per-theme, not global. */
pub(crate) const PAL_PLAYER: u32 = 0xFFFFFF;
pub(crate) const PAL_STAIRS: u32 = 0xFFFF60;
pub(crate) const PAL_POTION: u32 = 0xFF50A0;
pub(crate) const PAL_SWORD: u32 = 0x70B0FF;
pub(crate) const PAL_AMULET: u32 = 0xFFD700;
pub(crate) const PAL_LORE: u32 = 0xC0A0FF;
pub(crate) const PAL_RAT: u32 = 0xB0703A;
pub(crate) const PAL_GOBLIN: u32 = 0x40C040;
pub(crate) const PAL_OGRE: u32 = 0xD05050;
pub(crate) const PAL_STATUS: u32 = 0xE0E0E0;
pub(crate) const PAL_ALERT: u32 = 0xFF5050;
pub(crate) const PAL_LOG_FADE: [u32; 4] = [0x707070, 0x909090, 0xB0B0B0, 0xE0E0E0];
// New (status-bar sub-task): bar fill colors and the shared unfilled-
// segment color (light shade block, 0x2591, dimmed neutral gray).
pub(crate) const PAL_BAR_HP: u32 = 0x50C050;
pub(crate) const PAL_BAR_TORCH: u32 = 0xE0A030;
pub(crate) const PAL_BAR_EMPTY: u32 = 0x404040;
/// Becalmed-monster tint (batch 5 task 3, frontend half of the Henson
/// ruling): a fixed soft blue-gray, chosen over blending with the kind's own
/// color (PAL_RAT/PAL_GOBLIN/PAL_OGRE are all warm — brown/green/red) so a
/// calmed monster reads at a glance as "not a threat" regardless of which
/// kind it was, the same way a status-effect tint works in most roguelikes.
/// `render::render_play` substitutes this wholesale for the kind's stats
/// color (not a blend) when `Monster::calm` is true, then applies the same
/// `scale(_, pct)` light-tier dimming every other monster glyph gets — so at
/// full light the rendered fg is this constant exactly, and it dims exactly
/// like anything else does as the torch burns down.
pub(crate) const PAL_CALM_TINT: u32 = 0x9AB0C0;
/// Portal glyph color (batch 6 T1): a saturated magenta, chosen because it
/// sits outside every theme's earthy/muted wall+floor palette (blue-gray
/// water, tan/brown salt-house, rust-brown mine, muted purple library) —
/// against all four it reads as "not of this place," which is the whole
/// point of a door to somewhere else. Not blended or dimmed differently
/// from any other tile glyph — `render_play` applies the same light-tier
/// `scale(_, pct)` treatment every other glyph gets.
pub(crate) const PAL_PORTAL: u32 = 0xFF40FF;
/// Pit glyph color (batch 6 T2, sokoban): a dull red-black void, reads as
/// hazard without competing with `PAL_ALERT` (HP/torch danger) or any
/// theme's own wall/floor palette. Same light-tier `scale(_, pct)`
/// treatment as every other map glyph.
pub(crate) const PAL_PIT: u32 = 0x502020;
/// Goal-tile glyph color (batch 6 T2, sokoban): a pale gold-green, distinct
/// from the pit's red-black, the portal's magenta, and every theme's
/// earthy wall/floor palette — reads as "aim here."
pub(crate) const PAL_GOAL: u32 = 0xB0D080;
/// Push-block glyph color (batch 6 T2, sokoban): plain stone gray, legible
/// against every theme's floor color and distinct from pit/goal/portal.
pub(crate) const PAL_BLOCK: u32 = 0x9A9A9A;

// ---------- Vaults ----------
/* Hand-authored rooms, stamped whole into a level by the "vault" channel
   (`Game::stamp_vault`). Legend: '#' wall, '.' floor, '!' potion, ')'
   sword, 'r'/'g'/'O' monster of that stat row, and — batch 6 T2, sokoban,
   ported in spirit from golem/topdown-puzzle's shared/push.js — '^' pit,
   'B' push-block, 'x' goal. Rules: rectangular, solid '#' border, center
   tile '.' (corridors target the center and will punch through walls to
   reach it — sealed chambers are opened by the carver, and the solver gate
   proves the exit stays reachable on every CI seed).

   Sokoban vaults (indices 3+) always gate their reward behind a genuine
   Pit crossing, never a bare block in open floor: `game::bfs_dist` blocks
   on `Tile::Pit`, so a reward reachable only by pushing a block past a Pit
   reads as UNREACHABLE via plain floor-walking until the pit is actually
   filled — that's the shape a puzzle needs to exist at all. It is NOT,
   however, what keeps the `--sim` bots safe (batch 6 T2 review finding:
   an earlier assumption that it was turned out false — an unrelated
   corridor can, and on rare seeds does, carve straight through a vault's
   interior and trivialize its pit for free; "the carver breaks in" is
   accepted for a pit exactly like a wall, see `Game::gen_level`'s
   corridor-carve comment). What actually keeps a `--sim` bot from ever
   deadlocking on one of these rooms is `Game::gen_level` excluding a
   sokoban vault's own center from `deepest`-room (exit) selection — the
   game's exit can never depend on solving one — plus `headless::sim_seed`
   routing around a block whenever floor allows it and, as a last resort,
   only ever pushing a block when `Game::would_push_succeed` confirms that
   SPECIFIC push won't be refused. See headless.rs's `routing_map`/
   `would_push_succeed` call site comments for the full mechanism. */
pub(crate) const VAULTS: [&str; 5] = [
    // sealed reliquary: the carver breaks in
    "#########\n\
     #g.....g#\n\
     #.#####.#\n\
     #.#!.)#.#\n\
     #.#####.#\n\
     #O......#\n\
     #########",
    // guard post: loot on the walls, teeth in the corners
    "#######\n\
     #r...r#\n\
     #)...!#\n\
     #r...r#\n\
     #######",
    // ogre den: two big ones, two prizes
    "#########\n\
     #O.....!#\n\
     #.##.##.#\n\
     #!.....O#\n\
     #########",
    // the bridge (batch 6 T2, sokoban): the true pit-crossing puzzle — push
    // the block into the gap to bridge it, then walk to the prize. Reward
    // is a potion, not a sword — per Amendment 2 (the batch-6 plan's
    // story-canon reconciliation): cheese doesn't exist as an item kind
    // yet (batch 7's item table), so this slot is a stand-in.
    // story: swap to cheese when the item table lands (batch 7)
    "##########\n\
     #.....B^!#\n\
     ##########",
    // the goal cell (batch 6 T2, sokoban): a 2-chain push destroys the
    // farthest block into the pit (bridging it), the survivor keeps going
    // and locks onto the goal tile, and ONLY THEN is the corridor clear to
    // the sword beyond — demonstrates chain-of-two, pit-fill, and
    // goal-lock in one authored room, and (per this file's VAULTS doc
    // comment) keeps the whole reward bfs-ungated until actually solved.
    "##################\n\
     #.........BB^.x.)#\n\
     ##################",
];

// ---------- Ghost labels ----------
/* Preset phrases stamped into a ghost file's header (RLG1 format, save.rs).
   Souls-style: no free text, ever — picking from this const table is the
   whole "message" a ghost can leave. Selection is DETERMINISTIC, not a new
   RNG channel: `ghost_label_idx` buckets by outcome (died_combat/died_dark/
   won/abandoned, matching save.rs's GHOST_* outcome consts in that order,
   3 labels per outcome) then picks within the bucket by final_depth (mod
   3), so the same run always produces the same label with zero extra
   draws. Grounding rule, same as THEMES/TONE_LINES: restates what the run
   did, never invents an entity or event. All entries are ASCII and <=16
   bytes (proved by `ghost_labels_fit_16_bytes` in main.rs). */
pub(crate) const GHOST_LABELS: [&str; 12] = [
    // died_combat (outcome 0)
    "fought too much",
    "outmatched",
    "swarmed",
    // died_dark (outcome 1)
    "the dark won",
    "torch ran out",
    "lost the light",
    // won (outcome 2)
    "made it out",
    "amulet in hand",
    "climbed free",
    // abandoned (outcome 3)
    "gave up early",
    "walked away",
    "left it behind",
];

/// Deterministic ghost-label selection: no RNG channel, just outcome and
/// final depth. `outcome` picks a band of `GHOST_LABELS` sized by the 4
/// known outcomes (see save.rs's GHOST_* consts); `final_depth` picks
/// within the band. Sized off `GHOST_LABELS.len()` rather than a hardcoded
/// 3 so the table and the selector can't silently drift apart.
pub(crate) fn ghost_label_idx(outcome: u8, final_depth: u8) -> u8 {
    let per_outcome = (GHOST_LABELS.len() / 4) as u8;
    let band = outcome.min(3) * per_outcome;
    band + final_depth % per_outcome
}

// ---------- Authored floors (batch 6 T1) ----------
/* A portal's destination may be an authored, singular place instead of a
   derived world: hand-built, one level, no RNG at all — `map` is parsed by
   `Game::instantiate_floor` exactly once per visit (fresh) or restored from
   its `LevelState` snapshot (revisit), never regenerated. Same-floor-from-
   different-worlds is the SAME floor (keyed by `WorldId::Floor(index)`, not
   by which portal led there), so its visited state (monsters killed, items
   taken) persists across every door that opens onto it.
   Legend, an extended vault-style subset: '#' wall, '.' floor, '<' the
   return portal (walk-on transits back to the source world — reuses
   `Tile::UpStairs` verbatim, see `Game::land_on_tile`'s UpStairs arm), '!'
   potion, ')' sword, 'r'/'g'/'O' monster of that stat row. Deliberately NO
   lore chars ('?') this batch — a floor has no `Theme` of its own to draw a
   lore template from (see `Game::world_seed`'s doc comment on how floors
   borrow the root theme for INCIDENTAL flavor only; lore items would need
   a real per-floor lore table, future work). Rules, mirroring `VAULTS`:
   rectangular, solid '#' border, exactly one '<', all chars legal
   (`authored_floors_well_formed` in main.rs), full-map size up to 80x25 —
   `Game::instantiate_floor` centers a smaller map and wall-pads the rest.
   `name`/`describe` are grounded (restate only what the fixed map
   guarantees is true) and fit the 78-char log row
   (`authored_floors_flavor_fits_log_row`). Two starter floors per the
   batch-6 brief; the heavy authoring is future NPC-cast-batch work. */
pub(crate) struct AuthoredFloor {
    pub(crate) name: &'static str,
    pub(crate) describe: &'static str,
    pub(crate) map: &'static str,
}

pub(crate) const AUTHORED_FLOORS: [AuthoredFloor; 2] = [
    // a quiet lore shrine: nothing hunts here, two flasks and the portal home
    AuthoredFloor {
        name: "a quiet shrine",
        describe: "Nothing hunts here. Two flasks wait on cold stone, untouched.",
        map: "###########\n\
              #.........#\n\
              #....!....#\n\
              #....<....#\n\
              #....!....#\n\
              #.........#\n\
              ###########",
    },
    // a small hazard/loot room: four guards ring one blade
    AuthoredFloor {
        name: "a cramped loot vault",
        describe: "Four guards ring a single blade. Someone thought it worth that many.",
        map: "#############\n\
              #r.........r#\n\
              #...........#\n\
              #.....).....#\n\
              #...........#\n\
              #g....<....g#\n\
              #############",
    },
];
