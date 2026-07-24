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
    AuthoredFloorDef, BalanceDef, BumpResponse, CarryEvent, GameDef, GiveRule, ItemDef, ItemEffect,
    MonsterDef, OverworldDef, OverworldScreenDef, PickupBehavior, StringsDef, ThemeDef, UseEffect,
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
/* batch 9 T1 (story §9-J prep, SIGN-OFF ASK #6): the minimal overworld
   cast — TRAINER (`passive: true, bump: Yield`) and DONKEY (`passive: true,
   bump: Shove`). Both are ordinary `MonsterDef` rows, placed by
   `Game::instantiate_overworld_screen` from the `Y`/`D` glyphs exactly the
   way `Game::instantiate_floor` already places monsters from an
   `AuthoredFloorDef`'s ASCII — no Tile enum change, no new entity-kind
   engine surface. Like COAT/TOWEL above, neither has a non-test production
   reference yet (they're placed by glyph in `Game::instantiate_overworld_
   screen`, never referenced by index in engine code) — `#[allow(dead_code)]`
   for the same reason. */
#[allow(dead_code)]
pub(crate) const TRAINER: crate::game::MKind = 3;
#[allow(dead_code)]
pub(crate) const DONKEY: crate::game::MKind = 4;

pub(crate) const POTION: crate::game::IKind = 0;
pub(crate) const SWORD: crate::game::IKind = 1;
pub(crate) const OBJECTIVE: crate::game::IKind = 2;
pub(crate) const LORE_A: crate::game::IKind = 3;
pub(crate) const LORE_B: crate::game::IKind = 4;
pub(crate) const LORE_C: crate::game::IKind = 5;
/* batch 7 T2 (story §5/§9-A): the three items the story asks for, plus the
   GIVE/USE verbs (`gamedef::GiveRule`/`UseEffect`) their behaviors ride on.
   All three are `PickupBehavior::Hold` (see `ItemDef::on_pickup`'s doc
   comment) — walking over one adds it to `Game.held` and logs its find/take
   line; nothing happens until a later GIVE or USE. */
pub(crate) const CHEESE: crate::game::IKind = 6;
// `#[allow(dead_code)]`: unlike CHEESE (used below in `GIVE_TABLE`/
// `BALANCE.loot_bonus_item`), neither COAT nor TOWEL has a non-test
// production-code reference yet — both are vault-find-only this batch with
// no give-target (see `GIVE_TABLE`'s trailing comment and the `[YOURS]`
// D2/D4 monster casts §12.14/the towel receipt still need). They exist as
// named indices for main.rs's tests (which `cfg(test)`-gated code doesn't
// count toward this crate's own dead-code analysis) and for whichever
// future def row wires their give-targets in.
#[allow(dead_code)]
pub(crate) const COAT: crate::game::IKind = 7;
#[allow(dead_code)]
pub(crate) const TOWEL: crate::game::IKind = 8;

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

/* batch 9 T2 (story §9-J, SIGN-OFF ASK #8): the trainer/donkey talk-line
   content, replacing T1's placeholders. Same 4-stage x 2-variant shape and
   dispatch as every other kind (`try_talk_player`'s `stage` computation is
   unchanged by `passive`/`bump` — the dialogue ladder climbs exactly the
   same way for these two as for rat/goblin/ogre): 0 = first landed talk,
   1 = a later landed talk still below threshold, 2 = the landed talk that
   crosses `talk_threshold` (also reused forever once calm), 3 = a failed
   roll. Neither table uses `{M}` — the trainer and the donkey are named,
   singular NPCs (unlike the generic per-theme `RAT`/`GOBLIN`/`OGRE`), so
   their lines are direct, unsubstituted text; `try_talk_player`'s
   `.replace("{M}", name)` is a harmless no-op on a line with no `{M}` in it,
   same mechanism `talk_lines_fit_log_row` already exercises across every
   theme (the fill is a no-op there too, so the test still covers these
   lines' raw length).

   DONKEY_TALK wires `DON_001`-`DON_005` (FLAVOR-DRAFT-v0.md, "donkey regard
   stages") in verbatim, same same-ID-text-conform discipline as every prior
   content batch — no em-dashes in the source, so no ASCII normalization was
   needed this time. `DON_001` ("The donkey regards you.") is the pun: the
   line names the `regard` mechanic firing on a first landed talk. Fit:
   DON_001 -> stage 0 (first landed) paired with a new grounded variant;
   DON_002/DON_003 -> stage 1 (mid-landed, both read as "still warming up");
   DON_004/DON_005 -> stage 2 (crosses threshold / already calm forever, both
   already describe the settled state). No canon line covers a FAILED talk,
   so stage 3 is new-authored, grounded in the donkey's established
   stubbornness (its own `BumpResponse::Shove` characterization above) and
   nothing else. This spends all 5 of the story's "~5 regard stages" lines
   across the engine's real 4-stage x 2-variant shape per the brief's own
   framing: content-fit, not a table-shape change.

   TRAINER_TALK wires `TRA_001`-`TRA_004` (the rule-one/cheese/depth-two/
   talk-skepticism lines) as the four canon variant-0 slots, one per stage,
   in escalating-conversation order; `TRA_005`-`TRA_008` are NOT reused here
   — they're anchored to other triggers (sparing/talking to a training rat,
   resurrection, repeat deaths), not to "the player landed a talk with the
   trainer himself," so folding them into this stage table would misattribute
   what fired them; they're held out for whatever future hook actually
   fires on those events (same "held out, not dropped" treatment batch 8 T2
   gave NAR_050-054/060/062/063). New grounded variant-1 lines pair each
   canon line in the trainer's same terse, dry-coach register.

   Fix round: `TRAINER`'s `talk_threshold` was T1's placeholder value of 2,
   which made stage 1 ("mid-landed") mechanically unreachable in play — a
   first landed talk brings `regard` to 1, so the very next landed talk
   would cross the threshold straight to stage 2, never landing on stage 1,
   which meant TRA_002 (the canon cheese line, stage 1's variant-0 slot)
   could never actually display. Bumped `talk_threshold` to 3 (see the
   `TRAINER` `MonsterDef` row below) so all four authored stages are
   reachable: stage 0 at regard 0->1, stage 1 at regard 1->2, stage 2 at
   regard 2->3 (crosses threshold, becalms), same rung-by-rung climb every
   other kind's table gets. `receptivity_base: 90` was already high enough
   that this costs at most one extra landed talk in practice — the trainer
   is a passive, un-killable, unlimited-attempts NPC, so there's no failure
   cost to a slower ladder, unlike a hostile monster's threshold tuning.

   batch 12 R7 (the depth-5 telegraphing pass): stage-0 variant-1 was revised
   to carry the pickup-verdict warning — "What's below keeps a tally" grounds
   to the mood anchor `Game::pickup` reads from the descent's kill/spare
   record (see `BalanceDef::mood_anchor_weight` and `Game::mood`), and the
   trainer has "been down there. Twice" (TRA_002), so he plausibly knows.
   This is the batch's guaranteed pre-pickup warning that she will judge how
   you came down; the FULL trainer-memory (reading your last life) is the
   deferred batch-13 hook, but the warning itself lands here.

   batch 13 T1 (the deferred hook above, now built): TRA_007 ("resurrection")
   was held out of TRAINER_TALK because it isn't anchored to "landed a talk
   with the trainer" like the four stage rows above it — it's anchored to
   "the player is back from a death," a different trigger entirely
   (`MonsterDef::resurrection_lines`, fired once by `Game::try_talk_player`
   on the first landed talk of a fresh post-death life). Wired verbatim,
   unmodified, as the bloody-return slot (index 0): "Back already? Happens.
   I don't ask. You don't ask." reads as an ironic salute once the player
   already knows their own last life killed more than it spared — the line
   itself states no such thing, the irony is entirely the player's own
   knowledge, same "grounded, invent nothing" discipline as every other
   register here. The merciful-return counterpart (index 1) is new-authored
   in the same terse, dry-coach register, restating only the complementary
   fact (spared >= kills) without claiming a number or inventing a reason he'd
   know it: "Back already? Quiet trip, from what I can tell. Don't get many
   of those." Every other kind's `resurrection_lines` stays `None` — this
   batch's memory is the trainer's alone; a future kind can opt in the same
   way. */
const TRAINER_TALK: [[&str; 2]; 4] = [
    [
        "Rule one: kill five rats. Warms up the sword arm. Everyone does it.",
        "Opinions are free in the yard. What's below keeps a tally, though. Mind it.",
    ],
    [
        "Bring cheese. Rats love cheese. I've been down there. Twice.",
        "Still talking. Fine. The rats aren't going anywhere either.",
    ],
    [
        "Depth two, both careers. You don't forget your first depth two.",
        "Nothing left to teach. You'll figure the rest out down there.",
    ],
    [
        "Talking to monsters. Heard of it. Never saw the percentage in it.",
        "Save the conversation. The dark doesn't listen either.",
    ],
];
const DONKEY_TALK: [[&str; 2]; 4] = [
    [
        "The donkey regards you.",
        "The donkey's ears swivel toward you. That much, at least, is new.",
    ],
    [
        "The donkey permits an ear scratch. Your half or theirs. Unclear.",
        "The donkey shifts its weight toward you. Diplomatically.",
    ],
    [
        "The donkey has becalmed. It was never not calm. Still: official.",
        "The donkey stands beside you now. On purpose.",
    ],
    [
        "The donkey does not look up. Whatever that was, it wasn't enough.",
        "The donkey stays put, chewing, unmoved by any of it.",
    ],
];
// batch 13 T1 ("the trainer reads your last life"): TRA_007 (verbatim,
// FLAVOR-DRAFT-v0.md's `(resurrection)` slot) for a bloody return, a new
// grounded counterpart for a merciful one — see `TRAINER_TALK`'s own doc
// comment above for the full grounding rationale.
const TRAINER_RESURRECTION: [&str; 2] = [
    "Back already? Happens. I don't ask. You don't ask.",
    "Back already? Quiet trip, from what I can tell. Don't get many of those.",
];

const MONSTERS: [MonsterDef; 5] = [
    MonsterDef {
        hp: 3,
        atk: 1,
        glyph: b'r',
        color: 0xB0703A,
        talk_threshold: 2,
        receptivity_base: 55,
        talk_lines: RAT_TALK,
        resurrection_lines: None,
        passive: false,
        bump: BumpResponse::Fight,
        retaliation: 0,
        awe_threshold: 0,
        // batch 12 R4 [TUNE]: cruelty to the near-harmless — the most
        // despicable kill in the roster.
        kill_valence: 0,
    },
    MonsterDef {
        hp: 6,
        atk: 2,
        glyph: b'g',
        color: 0x40C040,
        talk_threshold: 3,
        receptivity_base: 35,
        talk_lines: GOBLIN_TALK,
        resurrection_lines: None,
        passive: false,
        bump: BumpResponse::Fight,
        retaliation: 0,
        awe_threshold: 0,
        // batch 12 R4 [TUNE]: a middling threat — more forgivable than the
        // rat, still below self-defense.
        kill_valence: 15,
    },
    MonsterDef {
        hp: 13,
        atk: 4,
        glyph: b'O',
        color: 0xD05050,
        talk_threshold: 4,
        receptivity_base: 20,
        talk_lines: OGRE_TALK,
        resurrection_lines: None,
        passive: false,
        bump: BumpResponse::Fight,
        // [TUNE] batch 11: guaranteed retaliation — bump-attacking an ogre
        // always costs 3 HP, even on a killing blow (task 5 tunes this).
        retaliation: 6,
        // [TUNE] batch 11 T2: 3 cardinal-adjacent turns of not swinging
        // becalms it via awe — the diplomat's ogre answer.
        awe_threshold: 3,
        // batch 12 R4 [TUNE]: a guaranteed-hitter (see `retaliation` above)
        // — killing it reads closest to self-defense, the least despicable
        // of the three, though still below the neutral midpoint.
        kill_valence: 30,
    },
    // TRAINER (batch 9 T1, story §9-J prep, SIGN-OFF ASK #6): un-killable by
    // construction — `passive` keeps it out of `monsters_act` entirely, and
    // `bump: Yield` means a bump swaps position (never an attack) whether or
    // not it's ever been talked to. `hp`/`atk` are inert (never read by
    // combat once bump can't route to Fight) but kept sane rather than
    // zeroed.
    MonsterDef {
        hp: 10,
        atk: 0,
        glyph: b'Y',
        color: 0xC0A050,
        talk_threshold: 3,
        receptivity_base: 90,
        talk_lines: TRAINER_TALK,
        resurrection_lines: Some(TRAINER_RESURRECTION),
        passive: true,
        bump: BumpResponse::Yield,
        retaliation: 0,
        awe_threshold: 0,
        // batch 12 R4: un-killable by construction (see the comment above
        // this table) — irrelevant, kept sane rather than implying a
        // meaning.
        kill_valence: 0,
    },
    // DONKEY (batch 9 T1, story §9-J prep, SIGN-OFF ASK #6): stubborn —
    // `bump: Shove` pushes it one tile if the destination is plain floor,
    // else it plants; never damaged, never dies, "your own resurrection
    // point." `talk_threshold`/`receptivity_base` and `DONKEY_TALK` are
    // placeholders; T2 fits the story's "~5 regard stages" content into
    // this same 4-stage x 2-variant shape (see `gamedef::MonsterDef::
    // talk_lines`'s doc comment on why "~5 lines" doesn't mean 5 states).
    MonsterDef {
        hp: 12,
        atk: 0,
        glyph: b'D',
        color: 0x8A6A4A,
        talk_threshold: 4,
        receptivity_base: 30,
        talk_lines: DONKEY_TALK,
        resurrection_lines: None,
        passive: true,
        bump: BumpResponse::Shove,
        retaliation: 0,
        awe_threshold: 0,
        // batch 12 R4: un-killable by construction — irrelevant, same as
        // TRAINER above.
        kill_valence: 0,
    },
];

/* batch 7 T2: the potion moves from `Consume` (old walk-over heal) to
   `Hold` — give-ability requires holding it (story §5: "Potion, given...");
   the walk-over heal becomes a USE effect instead (`on_use`), same amount
   (8) and same eventual log line (NAR_031, moved verbatim from the old
   pickup path to the new use path) — see `ItemDef::on_pickup`'s doc
   comment. `potion_pickup_line` is new-authored (not a FLAVOR-DRAFT-v0 ID:
   the draft never anticipated the potion becoming a Hold item) — flagged in
   this batch's report for a human swap if a numbered line is ever assigned. */
const POTION_PICKUP_LINE: &str = "You take a potion. Best saved, or spent.";
const POTION_USE_LINE: &str = "You drink the potion. Your wounds close. The potion is gone forever."; // NAR_031, verbatim
const CHEESE_PICKUP_LINE: &str = "You take the cheese. It is historically significant."; // NAR_032, verbatim
const CHEESE_USE_LINE: &str = "You burn the cheese. Cheese is, technically, fuel."; // NAR_033, verbatim
const POTION_GIVE_LINE: &str = "You offer the potion. This is not what potions are for. It works."; // NAR_035, verbatim
const COAT_PICKUP_LINE: &str = "You find a coat. It is a normal coat for one normal monster."; // NAR_036, verbatim
const TOWEL_PICKUP_LINE: &str = "You find a towel. You cannot imagine who needs it. You will."; // NAR_037, verbatim

const ITEMS: [ItemDef; 9] = [
    ItemDef {
        glyph: b'!',
        color: 0xFF50A0,
        effect: ItemEffect::None,
        on_pickup: PickupBehavior::Hold,
        pickup_line: POTION_PICKUP_LINE,
        on_use: Some(UseEffect::Heal(3)),
        use_line: POTION_USE_LINE,
    },
    ItemDef {
        glyph: b')',
        color: 0x70B0FF,
        effect: ItemEffect::AtkBonus(2),
        on_pickup: PickupBehavior::Consume,
        pickup_line: "",
        on_use: None,
        use_line: "",
    },
    ItemDef {
        glyph: b'&',
        color: 0xFFD700,
        effect: ItemEffect::Objective,
        on_pickup: PickupBehavior::Consume,
        pickup_line: "",
        on_use: None,
        use_line: "",
    },
    ItemDef {
        glyph: b'?',
        color: 0xC0A0FF,
        effect: ItemEffect::Lore(0),
        on_pickup: PickupBehavior::Consume,
        pickup_line: "",
        on_use: None,
        use_line: "",
    },
    ItemDef {
        glyph: b'?',
        color: 0xC0A0FF,
        effect: ItemEffect::Lore(1),
        on_pickup: PickupBehavior::Consume,
        pickup_line: "",
        on_use: None,
        use_line: "",
    },
    ItemDef {
        glyph: b'?',
        color: 0xC0A0FF,
        effect: ItemEffect::Lore(2),
        on_pickup: PickupBehavior::Consume,
        pickup_line: "",
        on_use: None,
        use_line: "",
    },
    // CHEESE (batch 7 T2, story §4/§5 D1): the midden's own bait —
    // `o`, adopted verbatim from docs/story/SPACES-DRAFT-v0.md's legend
    // proposal for the cheese-wheel glyph. Held; burns for a light flicker
    // on USE [TUNE +8]; given to a rat, a measured regard PENALTY (see
    // `GIVE_TABLE` below) instead of the bait the genre promised.
    ItemDef {
        glyph: b'o',
        color: 0xE8C840,
        effect: ItemEffect::None,
        on_pickup: PickupBehavior::Hold,
        pickup_line: CHEESE_PICKUP_LINE,
        on_use: Some(UseEffect::Light(8)),
        use_line: CHEESE_USE_LINE,
    },
    // COAT (batch 7 T2, story §4 D2 / §5): vault-find only this batch, no
    // give-target yet (the coat-monster doesn't exist — `GIVE_TABLE` has no
    // row for this item, so GIVE gracefully no-ops per `StringsDef::
    // give_declined`). Glyph `[` — non-colliding with every existing tile/
    // item/monster/vault-legend character.
    ItemDef {
        glyph: b'[',
        color: 0xA0785C,
        effect: ItemEffect::None,
        on_pickup: PickupBehavior::Hold,
        pickup_line: COAT_PICKUP_LINE,
        on_use: None,
        use_line: "",
    },
    // TOWEL (batch 7 T2, story §4 D4 / §5): same shape as the coat — vault
    // find, no give-target yet (the lost guy isn't a `GiveRule` target this
    // batch either). Glyph `~`.
    ItemDef {
        glyph: b'~',
        color: 0x60C0D0,
        effect: ItemEffect::None,
        on_pickup: PickupBehavior::Hold,
        pickup_line: TOWEL_PICKUP_LINE,
        on_use: None,
        use_line: "",
    },
];

/* GIVE table (batch 7 T2, story §5/§9-A). Row order doesn't matter for
   correctness (`Game::try_give_player` scans for the first match and this
   batch never lists two rows for the same item), but is kept
   cheese-then-potion to match the items table above. */
const GIVE_TABLE: [GiveRule; 2] = [
    // cheese -> rat: measured regard PENALTY [TUNE -2] (story §4: "everyone
    // knows rats want cheese... offering cheese: measured regard PENALTY").
    // `line: None` reuses the rat's own stage-3 ("unmoved") talk line via
    // `Game::try_give_player`'s generic fallback rather than inventing new
    // give-specific flavor text — batch 7 T2 brief: "hooks into the
    // existing talk/regard machinery, not new lines."
    GiveRule { item: CHEESE, monster: Some(RAT), regard_delta: -2, line: None, heal_full: false, consumes: true },
    // potion -> any monster: the single biggest regard event in the game
    // [TUNE +3] (story §5: "Potion, given... undoing the wound after it
    // made them listen"). `heal_full` undoes whatever wound made the
    // target listen; NAR_035 verbatim.
    GiveRule {
        item: POTION,
        monster: None,
        regard_delta: 3,
        line: Some(POTION_GIVE_LINE),
        heal_full: true,
        consumes: true,
    },
    // [YOURS/TUNE] story §5/§12.14: cheese also "works on ONE other
    // monster" — a positive give-target, human-picked (the story is
    // explicit that this is not a guessing job: goblin/ogre would both be
    // wrong without the human's line). Deliberately left OUT of this table
    // until that pick lands; GIVE-ing cheese at anything but a rat falls
    // through to the generic `give_declined` no-op in the meantime.
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
        // batch 9 T1: indices 3/4 (trainer/donkey) are constant across every
        // theme, unlike rat/goblin/ogre's per-theme reskins — they're two
        // specific recurring characters, not a monster-kind archetype that
        // gets a new name per dungeon.
        mobs: &["cloister rat", "drowned acolyte", "bell-warden", "trainer", "donkey"],
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
        mobs: &["salt rat", "clerk-thing", "debt-golem", "trainer", "donkey"],
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
        mobs: &["blind rat", "ember wisp", "pit foreman", "trainer", "donkey"],
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
        mobs: &["paper rat", "ink haunt", "shelf-warden", "trainer", "donkey"],
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
   sword, 'r'/'g'/'O' monster of that stat row, — batch 6 T2, sokoban,
   ported in spirit from golem/topdown-puzzle's shared/push.js — '^' pit,
   'B' push-block, 'x' goal, and — batch 7 T2, story §5/§6.3 — 'o' cheese,
   '[' coat, '~' towel. Rules: rectangular, solid '#' border, center tile
   '.' (corridors target the center and will punch through walls to reach
   it — sealed chambers are opened by the carver, and the solver gate
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
    // the block into the gap to bridge it, then walk to the prize. batch 7
    // T2: this is the "cheese doesn't exist yet, use a potion, note the
    // swap-slot" placeholder from the batch-6 plan — the prize is now the
    // cheese itself ('o'), not a potion.
    "##########\n\
     #.....B^o#\n\
     ##########",
    // the goal cell (batch 6 T2, sokoban): a 2-chain push destroys the
    // farthest block into the pit (bridging it), the survivor keeps going
    // and locks onto the goal tile, and ONLY THEN is the corridor clear to
    // the sword beyond. batch 7 T2: two of the corridor's free floor tiles,
    // immediately west of the player's start (which is unchanged), now
    // hold the coat and towel — reachable without solving the puzzle at
    // all, same room's ONE reward slot each per the batch-7 T2 brief;
    // layout (walls/blocks/pit/goal positions, room width) is byte-for-byte
    // unchanged from batch 6, only two floor cells' item content moved.
    "##################\n\
     #......[~.BB^.x.)#\n\
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
    hp_gain_per_depth: 3,
    spawn_base_count: 3,
    monster_roll: &[(9, RAT), (13, GOBLIN), (i32::MAX, OGRE)],
    loot_count_lo: 2,
    loot_count_hi: 4,
    loot_count_per_depth: 1,
    loot_potion_num: 3,
    loot_potion_den: 4,
    loot_potion_item: POTION,
    loot_sword_item: SWORD,
    // batch 7 T2 (story §5/§11: cheese "mostly on depth 1-2" [TUNE]): this
    // cartridge's bonus item IS the cheese (see `CHEESE`/`GIVE_TABLE`
    // above) — a loot slot on depth 1 rolls it 2/5 of the time, depth 2
    // 1/5, depth 3+ never (no entry — see `BalanceDef::loot_bonus_chance`'s
    // doc comment for why a missing/zero entry costs zero extra
    // spawns-channel draws on those depths).
    loot_bonus_chance: &[(2, 5), (1, 5)],
    loot_bonus_item: CHEESE,
    lore_items: [LORE_A, LORE_B, LORE_C],
    receptivity_regard_coeff: 18,
    receptivity_wound_coeff: 40,
    receptivity_atk_coeff: 6,
    receptivity_torch_penalty: 10,
    receptivity_torch_radius_threshold: 4,
    receptivity_clamp: (5, 95),
    // batch 12 T1 (story "light as grace" — the violence half): [TUNE]
    // starting value, priced later against the balance sim re-baseline.
    kill_light_penalty: 8,
    // batch 12 R3 (story "light as grace" — the grace half): [TUNE]
    // starting value, priced later against the tactical-band re-baseline.
    rest_heal: 1,
    // batch 12 R4 (the pickup verdict): [TUNE] starting values, per the
    // human-authored mood model — see `gamedef::BalanceDef::
    // mood_anchor_weight`'s doc comment.
    mood_anchor_weight: 20,
    mood_spare_valence: 100,
    // batch 12 R5 ("light as grace"): [TUNE] starting bands. mood 76-100 ->
    // radius 6 (a max-shine diplomat's light matches the torch's own top
    // tier); 51-75 -> 4; 26-50 -> 2; 0-25 -> radius 0, the DARK tier — a
    // mood-0 brute gets no shine from her at all, preserving the pre-batch
    // "torch dead = death" outcome exactly for that carrier.
    mood_shine_tiers: &[(75, 6), (50, 4), (25, 2), (i32::MIN, 0)],
};

const WIN: WinDef = WinDef {
    objective_item: OBJECTIVE,
    max_depth: 5,
    return_depth: 1,
    carry_burn: 2,
    carry_line_rate_limit: 6,
};

/* batch 8 T2 (story §9-B/C/D, "the McGuffin's voice"): FLAVOR-DRAFT-v0's
   MCG_ lines, wired verbatim by stable ID except for one across-the-board
   ASCII normalization — `render.rs`'s `put_str` maps each BYTE of a log
   string to one grid cell, so a multi-byte UTF-8 char (the draft's em-dash)
   would render as garbage cells and break the 78-char row budget measured
   in bytes. Every em-dash (`--` below) is FLAVOR-DRAFT's own "--" character,
   two ASCII hyphens, standing in for the same cut-off-mid-word beat; no
   other text, spacing, or capitalization changed. This is a same-ID text
   conform, permitted by that file's own header ("replace any line's text
   without touching its ID and nothing downstream breaks").

   Held out of this batch (not wired anywhere): MCG_063 (asserts a spared
   "one with the coat" — the coat-as-monster doesn't exist yet, only the
   coat-as-item does; see status log/DECISION.md item 4) and MCG_062 (reads
   as promising the §9-E mood-to-light free-step effect, not implemented
   this batch — reviewer's call to hold per the T2 wiring spec). NAR_050-054
   are held entirely: all five hardcode "the amulet," which is true in none
   of this cartridge's four themes (`objective_name` is "the Quiet Bell" /
   "the Final Ledger" / "the First Lode" / "the Last Index"), and
   `Game::carry_event` does no template substitution to fix that at
   dispatch time. `MonsterAdjacent`/`TierCrossed`/`Idle` ship with no row at
   all — no authored lines exist for any of the three yet (future-content
   gap, not an oversight). */
const MCG_PICKED_UP_BLOODY: [&str; 4] = [
    "Unhand-- no. Hand. Keep handing. We'll workshop the rescue.", // MCG_010
    "Seized by a brute! It's exactly like the books. The good ones.", // MCG_011
    "The rats told me about you. Well. The survivors did.", // MCG_012
    "I shall resist you by growing fond of you. It's chapter four.", // MCG_013
];
const MCG_PICKED_UP_MERCIFUL: [&str; 4] = [
    "You came. I sensed a gentleness. The rats speak highly of you.", // MCG_020
    "Fated. Foretold. Foreshadowed, at minimum.", // MCG_021
    "Four hundred years, and my hero walks in mid-monologue. Poetic.", // MCG_022
    "Don't speak. Or do. One of us should, and I have material.", // MCG_023
];
const MCG_PICKED_BACK_UP: [&str; 2] = [
    "You're back. I knew it. I rehearsed knowing it.", // MCG_074
    "While you were gone, nothing happened. I narrated it anyway.", // MCG_075
];
// `StairsUp` is indexed by `Game::speech_attempts` (the climb re-entry
// ladder), NOT drawn at random — `Game::carry_event` clamps the index to
// `pool.len() - 1`, so order here is load-bearing: this is the one true
// sequence, not a shuffleable pool. Rungs 0-3 are the short "ladder" (each
// attempt gets ~one word further into the same opening line); rungs 4-9
// are the "running commentary" lines folded into the SAME pool per the T2
// wiring spec (a second `(StairsUp, ...)` row would never be reached —
// `carried_lines` is looked up by first match). Climb 10+ repeats rung 9
// forever (MCG_045 reads fine as a steady-state line).
const MCG_STAIRS_UP: [&str; 10] = [
    "As I was saying--", // MCG_030
    "From the top, then: In the--", // MCG_031
    "Where was I. The beginning. In the beginning, there--", // MCG_032
    "I'll just hold my place. I'm good at holding a place.", // MCG_033
    "And here the hero bears me up the-- stairs. Yes. As rehearsed.", // MCG_040
    "Note the walls. I've had four centuries with these walls. Skippable.", // MCG_041
    "Is that a rat you know? You know a rat. My legs know a rat.", // MCG_042
    "This part wasn't in the drafts. Improvising. I'm thriving.", // MCG_043
    "Slower on the stairs. Not for me. For the drama. Fine, for me.", // MCG_044
    "You breathe loudly for a legendary figure. It's humanizing. Keep it.", // MCG_045
];
const MCG_KILL_WITNESSED: [&str; 4] = [
    "I saw that.", // MCG_050
    "The light seems heavier tonight. Unrelated, I'm sure.", // MCG_051
    "In the ballads, the sword is a last resort. This is a lot of resorts.", // MCG_052
    "It had a face, is all. I knew the face. Carry on.", // MCG_053
];
// MCG_060 ("There. A little of mine...") was held on the batch-8 T2
// grounding review because it reads as bestowing the §9-E mood->light gift,
// unwired then. Batch 12's second-lantern design IS that mechanic (her shine
// radius = f(mood)) — but MCG_060 STAYS HELD anyway (batch 12 R7 grounding
// review caught this): "a little of mine" asserts a nonzero shine reaching
// the player, and her radius hits 0 at mood<25 (a pure brute's dead lantern),
// where the claim is simply false. It needs a context with a GUARANTEED
// small-but-nonzero ring; no current trigger provides one (RestedDim spans
// radius 0 too, see its own comment). Revive it there, not here. MCG_062
// ("Take the next step on me.") stays held: it is grounded by the same shine,
// but it is a STEPPING line, a mismatch for a rest-in-place beat — it awaits
// a future climb/shine hook, not this rest pool. MCG_063 (the coat-monster
// claim) stays held because that monster still does not exist.
const MCG_SPARE_WITNESSED: [&str; 1] = [
    "I've decided your footsteps have a rhythm. I've named it.", // MCG_061
];
const MCG_PUT_DOWN: [&str; 4] = [
    "Setting me down. Bold. Dramatic, even. I'll allow it.", // MCG_070
    "A pause in the narrative. Very modern. Hurry back.", // MCG_071
    "Hello? Legs? ...Anyone with legs?", // MCG_072
    "(to the floor) He'll return. He's the returning type. I cast him.", // MCG_073
];
// batch 12 R7: the McGuffin tends a rest that mends (see `CarryEvent::
// RestedBright`/`RestedDim` and `Game::wait_turn`'s dispatch). BRIGHT fires
// when her shine radius >= 4 (mood >= 50) — she has light to spare and keeps
// a wide ring against the dark; both lines are grounded in that visible ring
// plus the fact that a wait heals. Register: tending, not economy.
const MCG_RESTED_BRIGHT: [&str; 2] = [
    "I've light to spare. Rest; I'll keep the dark off you.",
    "Lean here. I'll hold a bright ring around you a while.",
];
// DIM fires when she has gone dim (radius < 4): rest still heals — the
// mending doesn't need her shine — so both lines stay true across the WHOLE
// sub-50 range, INCLUDING the mood<25 / radius-0 tier where she casts NO
// light at all. That radius-0 edge is exactly why MCG_060 ("...a little of
// mine...") is NOT revived here despite §9-E landing: it asserts a nonzero
// shine, false when her ring is 0 (see the held-note above
// MCG_SPARE_WITNESSED). "gone dim" and "dimmed near to nothing" both cover
// radius 2 AND radius 0 honestly; "isn't mine to give"/"doesn't need me"
// both restate that rest_heal never consults her shine.
const MCG_RESTED_DIM: [&str; 2] = [
    "I've gone dim. Rest anyway; the mending doesn't need me.",
    "Dimmed near to nothing. Rest -- the mending isn't mine to give.",
];

const CARRIED_LINES: [(CarryEvent, &[&str]); 9] = [
    (CarryEvent::PickedUpBloody, &MCG_PICKED_UP_BLOODY),
    (CarryEvent::PickedUpMerciful, &MCG_PICKED_UP_MERCIFUL),
    (CarryEvent::PickedBackUp, &MCG_PICKED_BACK_UP),
    (CarryEvent::StairsUp, &MCG_STAIRS_UP),
    (CarryEvent::KillWitnessed, &MCG_KILL_WITNESSED),
    (CarryEvent::SpareWitnessed, &MCG_SPARE_WITNESSED),
    (CarryEvent::PutDown, &MCG_PUT_DOWN),
    (CarryEvent::RestedBright, &MCG_RESTED_BRIGHT),
    (CarryEvent::RestedDim, &MCG_RESTED_DIM),
];

/* batch 8 T2 (story §9-C, the pickup register): the fixed, always-fires
   preamble shown once at the FIRST objective pickup, printed in order
   before the kill/spare-keyed `PickedUpBloody`/`PickedUpMerciful` dispatch.
   MCG_001-004 verbatim (ASCII-normalized em-dash in MCG_001, see the block
   comment above `CARRIED_LINES`). */
const CARRIED_PREAMBLE: [&str; 4] = [
    "IN THE BEGINNING, THERE WAS ME--", // MCG_001
    "...You could have knocked.", // MCG_002
    "No matter. No matter! We'll call that the cold open.", // MCG_003
    "Onward, legs. Destiny dislikes a dawdler. I read that. I wrote that.", // MCG_004
];

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
    atk_item: "A {} blade, still sharp! (+{} ATK)",
    portal_arrive: "You step through into {}.",
    portal_return: "You step back through, right where you left.",
    portal_describe_world: "Beyond it: {} ({}).",
    portal_describe_floor: "Beyond it: {}.",
    floor_arrive: "You arrive at {}.",
    // batch 7 T2 (give/use no-op feedback): dry, restates the engine fact,
    // never invents an event — same grounding rule as every other line here.
    give_no_target: "There is nothing there to give it to.",
    give_empty_hands: "Your hands are empty.",
    give_declined: "The {} has no use for that.",
    use_empty_hands: "Your hands are empty.",
    use_no_effect: "That isn't something you can use like that.",
    resource_label: "Torch",
    // batch 8 T1 (PUT DOWN, byte 16, story §9-D): new-authored, not a
    // FLAVOR-DRAFT-v0 ID (the draft's own D-section lines are T2's job, via
    // `CarryEvent::PutDown`'s pool in `CARRIED_LINES` — this is the plain
    // mechanical feedback line, distinct from the McGuffin's own voice).
    put_down_ok: "You set your burden down.",
    put_down_occupied: "There is no room here.",
    put_down_nothing_carried: "You are carrying nothing to set down.",
    // batch 9 T1 (story §9-J prep): the shut door is dumb this batch,
    // always this line regardless of `has_objective` (POS_003, per the
    // batch-9 brief's own citation) — the smarter version is deferred
    // future work, see `Tile::ShutDoor`'s doc comment.
    shut_door_refuse: "Not until it's in hand.",
    shove_refuse: "The {} plants its feet and will not budge.",
    overworld_enter: "You arrive at {}.",
    overworld_cross: "You cross into {}.",
};

/* The overworld's 3 fixed screens (batch 9 T1, story §9-J prep, SIGN-OFF
   ASK #1): MINIMAL VALID PLACEHOLDER content only (bordered, legal legend
   chars, correctly linked edges) — sufficient to compile/test/dump; T2
   replaces name/describe/map with the real content drafted in
   `docs/story/SPACES-DRAFT-v0.md`. Legend, on top of the AuthoredFloorDef-
   style base ('#' wall, '.' floor, plus item/monster glyphs): '=' a
   deterministic screen-link (direction derived from which edge column it
   sits on — see `Tile::ScreenLink`'s doc comment), 'V' the hole down into
   the dungeon, '+' a dumb, always-shut door, 'Y'/'D' the TRAINER/DONKEY
   cast. All 3 screens share the same 11x5 footprint for this placeholder;
   nothing about the engine requires that (T2's real screens need not
   match). */
const OVERWORLD_1: OverworldScreenDef = OverworldScreenDef {
    // T2: placeholder
    name: "the waking field",
    describe: "Placeholder ground. The real overworld lands in batch 9 T2.",
    map: "###########\n\
          #D...Y....=\n\
          #....V....#\n\
          #.........#\n\
          ###########",
};
const OVERWORLD_2: OverworldScreenDef = OverworldScreenDef {
    // T2: placeholder
    name: "the long walk",
    describe: "Placeholder ground. The real overworld lands in batch 9 T2.",
    map: "###########\n\
          =.........=\n\
          #.........#\n\
          #.........#\n\
          ###########",
};
const OVERWORLD_3: OverworldScreenDef = OverworldScreenDef {
    // T2: placeholder
    name: "the shut gate",
    describe: "Placeholder ground. The real overworld lands in batch 9 T2.",
    map: "###########\n\
          =....+....#\n\
          #.........#\n\
          #.........#\n\
          ###########",
};

const OVERWORLD: OverworldDef = OverworldDef { screens: [OVERWORLD_1, OVERWORLD_2, OVERWORLD_3] };

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
    give_table: &GIVE_TABLE,
    carried_lines: &CARRIED_LINES,
    carried_preamble: &CARRIED_PREAMBLE,
    overworld: OVERWORLD,
};
