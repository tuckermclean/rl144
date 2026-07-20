// games/contractor.rs — cartridge #1: "The Contractor" (rl144's original
// and, through batch 6, only game). ALL game-specific facts live here: every
// monster/item stat and flavor line, every theme, every vault, every
// authored floor, every balance constant, the win condition, and every
// player-facing string template the engine's turn logic needs. The engine
// (game.rs/headless.rs/render.rs/save.rs) never spells any of this out — it
// consumes `GAME`'s fields through the one seam in `games/mod.rs`.
//
// ANOTHER GAME = ANOTHER FILE SHAPED LIKE THIS ONE: a second cartridge is a
// second `pub(crate) const GAME: GameDef` module (its own monster/item/theme
// tables, its own balance numbers, its own strings) plus a one-line change
// to `games/mod.rs`'s re-export to point at it instead. Nothing here is
// engine code — it's all data, so it stays fully replaceable.

use crate::gamedef::{
    AuthoredFloorDef, BalanceDef, GameDef, ItemDef, ItemEffect, MonsterDef, StringsDef, ThemeDef,
    WinDef,
};

// ---------- Monster/item kind indices ----------
// `MKind`/`IKind` are plain indices (`u8`) into `GAME.monsters`/`GAME.items`
// — the engine has no notion of "a rat" or "a potion," only "kind 0." These
// named consts are this cartridge's own vocabulary for its own tables and
// tests; nothing in game.rs/headless.rs/render.rs/save.rs references them by
// name.
pub(crate) const RAT: crate::game::MKind = 0;
pub(crate) const GOBLIN: crate::game::MKind = 1;
pub(crate) const OGRE: crate::game::MKind = 2;

pub(crate) const POTION: crate::game::IKind = 0;
pub(crate) const SWORD: crate::game::IKind = 1;
pub(crate) const OBJECTIVE: crate::game::IKind = 2;
pub(crate) const LORE_A: crate::game::IKind = 3;
pub(crate) const LORE_B: crate::game::IKind = 4;
pub(crate) const LORE_C: crate::game::IKind = 5;

/* Mercy as talk (batch 5, DECISION.md item 3 — the Henson ruling: "if you
   could talk to a rat, you could give a rat mercy"; addendum, human
   direction: talk is `game::receptivity`-rolled, not a guaranteed stay).
   Each landed/failed talk (game.rs `Game::try_talk_player`) draws one line
   from a kind's `talk_lines`, keyed by stage: 0 = the monster's first
   LANDED talk, 1 = a later landed talk before it calms, 2 = the landed talk
   that crosses `Monster::talk_threshold` (also reused, unchanged, for any
   further talk on an already-calm monster), 3 = a FAILED roll (the monster
   is unmoved — grounded per the rule below, voicing continued wariness,
   never a new invented event). Two variants per stage. `{M}` fills from the
   theme's own mob name for that kind (`ThemeDef::mobs[kind_index]`) — the
   only slot, so a monster is always named in its own theme's voice.
   Register, low tier to high: rats are simple, small, quick to flinch;
   goblins are wary, visibly weighing you; ogres are slow and heavy, even
   when they yield (or don't). Grounding rule (same as THEMES/TONE_LINES): a
   line may only voice want/fear about things the engine proves — the dark,
   your torch, its patch of the dungeon — never an invented event, never a
   promise the engine can't keep; a failed-stage line may say the monster is
   unmoved/still wary and nothing more. Length-tested
   (`talk_lines_fit_log_row` in main.rs) across every theme's mob-name
   filling. */
const RAT_TALK: [[&str; 2]; 4] = [
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
];
const GOBLIN_TALK: [[&str; 2]; 4] = [
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
];
const OGRE_TALK: [[&str; 2]; 4] = [
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
];

const MONSTERS: [MonsterDef; 3] = [
    MonsterDef {
        hp: 3,
        atk: 1,
        glyph: b'r',
        color: 0xB0703A,
        talk_threshold: 2,
        receptivity_base: 55,
        talk_lines: RAT_TALK,
    },
    MonsterDef {
        hp: 6,
        atk: 2,
        glyph: b'g',
        color: 0x40C040,
        talk_threshold: 3,
        receptivity_base: 35,
        talk_lines: GOBLIN_TALK,
    },
    MonsterDef {
        hp: 13,
        atk: 4,
        glyph: b'O',
        color: 0xD05050,
        talk_threshold: 4,
        receptivity_base: 20,
        talk_lines: OGRE_TALK,
    },
];

const ITEMS: [ItemDef; 6] = [
    ItemDef { glyph: b'!', color: 0xFF50A0, effect: ItemEffect::Heal(8) },
    ItemDef { glyph: b')', color: 0x70B0FF, effect: ItemEffect::AtkBonus(2) },
    ItemDef { glyph: b'&', color: 0xFFD700, effect: ItemEffect::Objective },
    ItemDef { glyph: b'?', color: 0xC0A0FF, effect: ItemEffect::Lore(0) },
    ItemDef { glyph: b'?', color: 0xC0A0FF, effect: ItemEffect::Lore(1) },
    ItemDef { glyph: b'?', color: 0xC0A0FF, effect: ItemEffect::Lore(2) },
];

/* Meaning is authored upstream; the generator is a librarian. Each depth
   draws its identity from this table via the per-depth "theme" channel.
   Grounding rule (template edition): flavor text may only restate things
   the engine did — never invent entities, exits, or events. Lore speaks of
   the place's past; it never claims something is present that isn't. */
const THEMES: [ThemeDef; 4] = [
    ThemeDef {
        label: "the drowned monastery",
        objective_name: "the Quiet Bell",
        mobs: &["cloister rat", "drowned acolyte", "bell-warden"],
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
    ThemeDef {
        label: "the salt counting-house",
        objective_name: "the Final Ledger",
        mobs: &["salt rat", "clerk-thing", "debt-golem"],
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
    ThemeDef {
        label: "the deep mine",
        objective_name: "the First Lode",
        mobs: &["blind rat", "ember wisp", "pit foreman"],
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
    ThemeDef {
        label: "the hollow library",
        objective_name: "the Last Index",
        mobs: &["paper rat", "ink haunt", "shelf-warden"],
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

/// Room identity: every room gets a kind noun and a tone; the first time
/// the player steps into a room, one tone line (with `{K}` filled) is
/// logged. Same grounding rule as lore: atmosphere only, no invented
/// entities. The LAST entry ("vault") is never drawn randomly — the engine
/// forces it onto stamped vault rooms (see `Game::gen_level`'s doc comment
/// on `room_kinds`).
const ROOM_KINDS: [&str; 7] = ["hall", "gallery", "cellar", "stairwell", "chapel", "storeroom", "vault"];

const TONE_LINES: [[&str; 2]; 4] = [
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

/* Hand-authored rooms, stamped whole into a level by the "vault" channel
   (`Game::stamp_vault`). Legend: '#' wall, '.' floor, '!' potion, ')'
   sword, 'r'/'g'/'O' monster of that stat row, and — batch 6 T2, sokoban,
   ported in spirit from golem/topdown-puzzle's shared/push.js — '^' pit,
   'B' push-block, 'x' goal. Rules: rectangular, solid '#' border, center
   tile '.' (corridors target the center and will punch through walls to
   reach it — sealed chambers are opened by the carver, and the solver gate
   proves the exit stays reachable on every CI seed).

   Sokoban vaults (indices 3+) always gate their reward behind a genuine
   Pit crossing — see `game::bfs_dist`'s and `Game::gen_level`'s doc
   comments for the full mechanism that keeps `--sim` bots safe around
   these rooms. */
const VAULTS: [&str; 5] = [
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
    // the block into the gap to bridge it, then walk to the prize.
    "##########\n\
     #.....B^!#\n\
     ##########",
    // the goal cell (batch 6 T2, sokoban): a 2-chain push destroys the
    // farthest block into the pit (bridging it), the survivor keeps going
    // and locks onto the goal tile, and ONLY THEN is the corridor clear to
    // the sword beyond.
    "##################\n\
     #.........BB^.x.)#\n\
     ##################",
];

/* A portal's destination may be an authored, singular place instead of a
   derived world: hand-built, one level, no RNG at all. Legend, an extended
   vault-style subset: '#' wall, '.' floor, '<' the return portal, '!'
   potion, ')' sword, 'r'/'g'/'O' monster of that stat row. See
   `gamedef::AuthoredFloorDef`'s doc comment for the engine-side contract. */
const AUTHORED_FLOORS: [AuthoredFloorDef; 2] = [
    // a quiet lore shrine: nothing hunts here, two flasks and the portal home
    AuthoredFloorDef {
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
    AuthoredFloorDef {
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

/* Preset phrases stamped into a ghost file's header (RLG1 format, save.rs).
   Souls-style: no free text, ever — picking from this const table is the
   whole "message" a ghost can leave. Selection is DETERMINISTIC
   (`content::ghost_label_idx`): buckets by outcome (died_combat/died_dark/
   won/abandoned, matching save.rs's GHOST_* outcome consts in that order,
   one band per outcome) then picks within the bucket by final_depth, so the
   same run always produces the same label with zero extra draws. Grounding
   rule, same as THEMES/TONE_LINES: restates what the run did, never invents
   an entity or event. All entries are ASCII and <=16 bytes (proved by
   `ghost_labels_fit_16_bytes` in main.rs). Length must stay divisible by 4
   (one band per outcome). */
const GHOST_LABELS: [&str; 12] = [
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

/* Solver-derived: worst-case round-trip walk budget over the 10K CI seed
   set is 1494 (--solve 10000, worstSeed 2108; budget = descend ×1 + climb
   out ×2 per step, see headless::solve_seed). Start light 2000 leaves
   margin for combat, detours and loot runs. Changing this re-tunes every
   run: rerun --solve and re-commit tests/solver-band.json alongside it. */
const BALANCE: BalanceDef = BalanceDef {
    starting_hp: 20,
    starting_atk: 3,
    base_burn: 1,
    violence_tax: 1,
    start_light: 2000,
    monster_sight: 8,
    fov_tiers: &[(50, 8), (30, 6), (18, 5), (10, 4), (4, 3), (i32::MIN, 2)],
    light_tiers: &[(8, 100), (6, 85), (5, 70), (4, 55), (3, 40), (2, 28)],
    hp_gain_per_depth: 4,
    spawn_base_count: 3,
    monster_roll: &[(9, RAT), (13, GOBLIN), (i32::MAX, OGRE)],
    loot_count_lo: 2,
    loot_count_hi: 4,
    loot_count_per_depth: 2,
    loot_potion_num: 3,
    loot_potion_den: 4,
    loot_potion_item: POTION,
    loot_sword_item: SWORD,
    lore_items: [LORE_A, LORE_B, LORE_C],
    receptivity_regard_coeff: 18,
    receptivity_wound_coeff: 40,
    receptivity_atk_coeff: 6,
    receptivity_torch_penalty: 10,
    receptivity_torch_radius_threshold: 4,
    receptivity_clamp: (5, 95),
};

const WIN: WinDef = WinDef { objective_item: OBJECTIVE, max_depth: 5, return_depth: 1, carry_burn: 2 };

const STRINGS: StringsDef = StringsDef {
    intro: "Fetch the Amulet from depth 5 and climb back before dark!",
    dark_death: "Your torch dies. The darkness takes you. [R] to restart",
    tier_warnings: [
        "The {} shadows edge closer as your torch burns low.",
        "The flame gutters; the {} dark takes what light remains.",
        "Darkness presses in, {} and patient, around your failing light.",
        "Your torch is nearly spent. The {} dark waits. Hurry.",
        "The last embers gutter. The {} dark is almost total.",
    ],
    enter_theme: "You enter {}.",
    descend_first: "Deeper, and harder. You feel tougher. (+4 HP)",
    descend: "You descend to depth {}.",
    ascend: "You climb back to depth {}.",
    pit_refuse: "The floor drops away underfoot. You cannot cross.",
    push_too_long: "That row will not budge. Too many to push.",
    push_blocked: "There is nowhere for it to go.",
    push_pit_fill: "The block tips into the pit and is gone. The gap fills.",
    push_goal_lock: "The block settles into the goal. Something gives way.",
    push_ok: "You shove the block.",
    slay: "You slay the {}! ({} dmg)",
    hit: "You hit the {} for {}.",
    killed_by: "The {} kills you... [R] to restart",
    hit_by: "The {} hits you for {}.",
    win: "You climb into daylight. You made it! [R] new run",
    need_objective: "The way out. You won't leave without the Amulet.",
    pickup_objective: "You take {}. It is heavy. Climb, before dark!",
    lore_prefix: "A carved inscription:",
    heal: "You quaff a {} draught. (+{} HP)",
    atk_item: "A {} blade, still sharp! (+{} ATK)",
    portal_arrive: "You step through into {}.",
    portal_return: "You step back through, right where you left.",
    portal_describe_world: "Beyond it: {} ({}).",
    portal_describe_floor: "Beyond it: {}.",
    floor_arrive: "You arrive at {}.",
};

pub(crate) const GAME: GameDef = GameDef {
    monsters: &MONSTERS,
    items: &ITEMS,
    themes: &THEMES,
    room_kinds: &ROOM_KINDS,
    tone_lines: &TONE_LINES,
    vaults: &VAULTS,
    authored_floors: &AUTHORED_FLOORS,
    ghost_labels: &GHOST_LABELS,
    balance: BALANCE,
    win: WIN,
    strings: STRINGS,
};
