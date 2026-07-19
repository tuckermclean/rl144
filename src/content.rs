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
   could talk to a rat, you could give a rat mercy"). Each ACT (input bytes
   7-10, game.rs `Game::try_act_player`) draws one line here, keyed by
   `[MKind as usize][stage]` — stage 0 = the monster's first ACT, 1 = a
   later ACT before it calms, 2 = the ACT that crosses
   `Monster::act_threshold` (also reused, unchanged, for any further ACT on
   an already-calm monster). Two variants per cell, picked via
   `flavor_rng` — same per-run, replay-safe channel and the same
   `TONE_LINES`-style variant-array shape as this file's other flavor
   tables, just with an extra (kind) dimension. `{M}` fills from the
   theme's own mob name for that kind (`Theme::mobs[kind as usize]`) — the
   only slot, so a monster is always named in its own theme's voice.
   Register, low tier to high: rats are simple, small, quick to flinch;
   goblins are wary, visibly weighing you; ogres are slow and heavy, even
   when they yield. Grounding rule (same as THEMES/TONE_LINES): a line may
   only voice want/fear about things the engine proves — the dark, your
   torch, its patch of the dungeon — never an invented event, never a
   promise the engine can't keep. Length-tested (`talk_lines_fit_log_row`
   in main.rs) across every theme's mob-name filling. */
pub(crate) const TALK_LINES: [[[&str; 2]; 3]; 3] = [
    // rat
    [
        [
            "The {M} flinches from your torch and does not bite.",
            "The {M} freezes, small and shaking, and does not run.",
        ],
        [
            "The {M} still won't near the torch, but stays put.",
            "The {M} watches you, still too scared to bite again.",
        ],
        [
            "The {M} goes still. It won't cross you again.",
            "The {M} settles, calmer than the dark behind it.",
        ],
    ],
    // goblin
    [
        [
            "The {M} lowers its blade, watching your torch, not you.",
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
    ],
    // ogre
    [
        [
            "The {M} stops mid-swing, breathing slow in the torchlight.",
            "The {M} lowers one fist, unhurried, and studies your torch.",
        ],
        [
            "The {M} still hasn't swung again. It is thinking, slowly.",
            "The {M} shifts its weight, in no hurry to fight you now.",
        ],
        [
            "The {M} lowers its fists and turns from the dark behind it.",
            "The {M} settles, heavy and calm, and stands down for good.",
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

// ---------- Vaults ----------
/* Hand-authored rooms, stamped whole into a level by the "vault" channel.
   Legend: '#' wall, '.' floor, '!' potion, ')' sword, 'r'/'g'/'O' monster
   of that stat row. Rules: rectangular, solid '#' border, center tile '.'
   (corridors target the center and will punch through walls to reach it —
   sealed chambers are opened by the carver, and the solver gate proves the
   exit stays reachable on every CI seed). */
pub(crate) const VAULTS: [&str; 3] = [
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
