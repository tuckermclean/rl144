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
