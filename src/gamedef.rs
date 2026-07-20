// gamedef.rs — the cartridge contract: pure data types describing
// everything a specific game supplies to the engine. Zero logic, zero
// platform calls, zero `cfg`. The engine (game.rs, headless.rs, render.rs,
// save.rs) consumes `GameDef` fields exclusively — it never spells out a
// monster name, an item's flavor, or a balance number as a literal. A
// second game is a second `const GAME: GameDef` shaped like the one under
// `games/` today (the reference cartridge) plus a one-line change to
// `games/mod.rs`'s re-export — that's the whole "swap the game" seam.
//
// Every table here is a `&'static [T]` slice rather than a fixed-size
// array wherever a second cartridge might plausibly want a different
// count (monster kinds, item kinds, themes, vaults, floors) — const fn
// limitations on this project's MSRV (rustc 1.75) are fine with slices and
// nested const structs; only const-time heap allocation is off the table,
// and nothing here needs it. Fixed-size arrays are kept only where the
// SHAPE is structural to the engine's own algorithm, not a per-game count:
// four adjectives/slots per theme (the flavor-template slot count),
// four talk stages x two variants (the receptivity state machine's own
// stage count), six FOV/light tiers (the tier-crossing warning count).

/// One playable "cartridge": every game-specific fact the engine needs.
pub(crate) struct GameDef {
    pub(crate) monsters: &'static [MonsterDef],
    pub(crate) items: &'static [ItemDef],
    pub(crate) themes: &'static [ThemeDef],
    /// Room-identity nouns ("hall", "gallery", ...) that `TONE_LINES`-style
    /// atmosphere lines fill `{K}` with. The engine forces the last index
    /// onto a stamped vault room's kind (`Game::gen_level`) — that convention
    /// is engine behavior, but which noun sits at that final index is data.
    pub(crate) room_kinds: &'static [&'static str],
    /// Room-tone atmosphere lines, two variants each, indexed by a per-room
    /// tone roll. Shared across all themes (unlike lore, which is per-theme).
    pub(crate) tone_lines: &'static [[&'static str; 2]],
    /// Hand-authored vault ASCII blueprints, stamped whole into a level.
    /// Legend: '#' wall, '.' floor, '^' pit, 'x' goal, 'B' push-block, plus
    /// one legend byte per item/monster equal to that def's own `glyph` —
    /// there is no separate vault-legend table, a def's glyph IS its legend
    /// character (see `Game::stamp_vault`).
    pub(crate) vaults: &'static [&'static str],
    /// Hand-authored singular floors a portal may lead to (see
    /// `AuthoredFloorDef`).
    pub(crate) authored_floors: &'static [AuthoredFloorDef],
    /// Death-recording ghost labels (`save.rs`'s RLG1 format): no free text
    /// ever, a run's label is picked deterministically from this table by
    /// outcome/depth (`content::ghost_label_idx`). Length must be divisible
    /// by 4 (one band per `save::GHOST_*` outcome).
    pub(crate) ghost_labels: &'static [&'static str],
    pub(crate) balance: BalanceDef,
    pub(crate) win: WinDef,
    pub(crate) strings: StringsDef,
    /// GIVE-verb reaction table (batch 7 T2, story §5/§9-A): rows keyed by
    /// (item kind, target monster kind — `None` matches any kind), consulted
    /// by `Game::try_give_player`. See `GiveRule`'s doc comment.
    pub(crate) give_table: &'static [GiveRule],
}

/// One monster kind's complete definition. `glyph` doubles as both the
/// render/dump character AND the vault/authored-floor ASCII legend byte for
/// this kind — one field, not two tables that could drift apart.
pub(crate) struct MonsterDef {
    pub(crate) hp: i32,
    pub(crate) atk: i32,
    pub(crate) glyph: u8,
    pub(crate) color: u32,
    /// Talks needed before this kind becalms (`Monster::talk_threshold`).
    pub(crate) talk_threshold: u8,
    /// Base receptivity percentage before any of `receptivity()`'s other
    /// terms (regard/wounds/player-atk/torch) are added — see
    /// `game::receptivity`'s doc comment for the full formula.
    pub(crate) receptivity_base: i32,
    /// Talk-line table, stage x variant: stage 0 = first landed talk, 1 = a
    /// later landed talk before becalming, 2 = the landed talk that crosses
    /// `talk_threshold` (also reused for any further talk once already
    /// calm), 3 = a failed roll. Two variants per stage, picked by
    /// `flavor_rng`. `{M}` fills from the theme's own mob name for this
    /// kind (`ThemeDef::mobs[kind_index]`).
    pub(crate) talk_lines: [[&'static str; 2]; 4],
}

/// One item kind's complete definition. `glyph` doubles as the vault/
/// authored-floor ASCII legend byte, same convention as `MonsterDef::glyph`.
pub(crate) struct ItemDef {
    pub(crate) glyph: u8,
    pub(crate) color: u32,
    pub(crate) effect: ItemEffect,
    /// batch 7 T2 (story §9-A's minimal inventory): whether walking onto
    /// this item applies `effect` immediately (`Consume` — the original v0
    /// walk-over behavior) or instead adds it to `Game.held` (`Hold`, LIFO)
    /// for a later directed GIVE or self-applied USE. A cartridge may move
    /// an existing `Consume` item to `Hold` when it grows a give-ability
    /// (see `on_use`'s doc comment for why: give-ability requires holding
    /// it, so any walk-over effect that item used to apply immediately has
    /// to move to USE instead) — see the active cartridge's own item table
    /// for which rows are which and why.
    pub(crate) on_pickup: PickupBehavior,
    /// The line logged on a `Hold` pickup (ignored for `Consume` rows,
    /// whose pickup message comes from `StringsDef`'s existing templated
    /// fields — `atk_item`/`pickup_objective`/etc. — unchanged). Empty
    /// string on every `Consume` row.
    pub(crate) pickup_line: &'static str,
    /// What USE (input byte 15) does when this is the top of `Game.held`.
    /// `None` for a give-only item with no self-use — USE is a graceful
    /// no-op on it.
    pub(crate) on_use: Option<UseEffect>,
    /// The line logged on a landed USE. Empty string when `on_use` is `None`.
    pub(crate) use_line: &'static str,
}

/// What picking up an item does, as data rather than an engine-side match
/// on a game-specific item name. `pickup()` in game.rs is the one place
/// that interprets this enum; adding a new effect kind is a real engine
/// primitive addition, not a per-cartridge concern.
///
/// batch 7 T2: there is no `Heal` variant here — the potion, this
/// cartridge's only healing item, moved from `Consume`-on-walk-over to
/// `Hold`-plus-`UseEffect::Heal` (see `ItemDef::on_pickup`'s doc comment),
/// so nothing constructs a walk-over heal anymore. Re-add a variant here if
/// a future cartridge item ever wants an immediate walk-over heal again.
#[derive(Clone, Copy)]
pub(crate) enum ItemEffect {
    /// Permanently raise player attack by this much.
    AtkBonus(i32),
    /// The run's win-condition item (only one item kind should carry this
    /// per cartridge — see `WinDef::objective_item`).
    Objective,
    /// A buried-lore inscription at tier 0/1/2 (shallow/mid/deep).
    Lore(u8),
    /// batch 7 T2: no immediate walk-over effect — every `Hold` item (see
    /// `ItemDef::on_pickup`) carries this, since `Game::pickup`'s `Consume`
    /// match is the only place `effect` is ever read and a `Hold` item
    /// never reaches it.
    None,
}

/// Whether a walk-over pickup applies `ItemDef::effect` immediately or adds
/// the item to `Game.held` for later GIVE/USE. See `ItemDef::on_pickup`'s
/// doc comment.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickupBehavior {
    Consume,
    Hold,
}

/// What USE (input byte 15, batch 7 T2) does to the most-recently-held item,
/// self-applied. `Game::use_item` is the one engine site that interprets
/// this enum.
#[derive(Clone, Copy)]
pub(crate) enum UseEffect {
    /// Restore up to this much HP, capped by `maxhp - hp` — same formula
    /// `ItemEffect::Heal` used at walk-over before the potion moved to
    /// `Hold` this batch.
    Heal(i32),
    /// Add this much to the run's light pool — see the active cartridge's
    /// own item table for which item burns and why.
    Light(i32),
}

/// One GIVE-table row (batch 7 T2, story §5/§9-A): what happens when the
/// player offers the top of `Game.held` to an adjacent monster of a given
/// kind. Consulted by `Game::try_give_player`, which scans `GameDef::
/// give_table` for the first row matching (held item, target kind) — exact
/// kind match preferred implicitly by table order (a cartridge author
/// should list specific-kind rows before a wildcard row for the same item,
/// though this batch's table never needs both for one item). No matching
/// row is a graceful no-op (an item with no give-target row yet — see the
/// active cartridge's own item table for which ones): logged via
/// `StringsDef::give_declined`, no turn spent, item stays held.
pub(crate) struct GiveRule {
    pub(crate) item: crate::game::IKind,
    /// `None` matches any monster kind (the potion's universal row).
    pub(crate) monster: Option<crate::game::MKind>,
    /// Applied to the target's `regard` via saturating add/sub — may cross
    /// `Monster::talk_threshold` and becalm it, exactly like a landed talk
    /// (`Game::try_talk_player`'s same crossing check).
    pub(crate) regard_delta: i8,
    /// `Some` for a dedicated reaction line (the potion's NAR_035, verbatim
    /// from FLAVOR-DRAFT-v0); `None` reuses the target's own stage-3
    /// ("unmoved") talk line — a penalty row hooks into the existing
    /// talk/regard machinery instead of inventing new give-specific flavor
    /// text (batch 7 T2 brief; see the active cartridge's own table for
    /// which row this is).
    pub(crate) line: Option<&'static str>,
    /// Heals the target to its kind's full HP (`MonsterDef::hp`) — the
    /// potion-gift's "single biggest regard event," undoing whatever wound
    /// made it listen (story §5).
    pub(crate) heal_full: bool,
    /// Whether a landed give removes the item from `Game.held`.
    pub(crate) consumes: bool,
}

/// One theme's complete authored identity: label, the run's win-condition
/// item's name AS SEEN IN THIS THEME (a themed synonym for the generic
/// objective, e.g. "the Quiet Bell"), per-monster-kind skins (indexed the
/// same as `GameDef::monsters`), flavor adjectives, buried-lore templates,
/// and per-theme render colors.
pub(crate) struct ThemeDef {
    pub(crate) label: &'static str,
    pub(crate) objective_name: &'static str,
    /// Per-monster-kind skin name, indexed exactly like `GameDef::monsters`.
    pub(crate) mobs: &'static [&'static str],
    pub(crate) adjs: [&'static str; 4],
    /// Buried-lore templates, shallow/mid/deep (`{A}` fills from `slots`).
    pub(crate) lore: [&'static str; 3],
    pub(crate) slots: [&'static str; 4],
    pub(crate) wall: u32,
    pub(crate) floor: u32,
}

/// A hand-built, singular, non-random single-level map a portal may lead
/// to. `map`'s legend: '#' wall, '.' floor, '<' the return portal (exactly
/// one required), plus item/monster glyphs same as `GameDef::vaults`.
pub(crate) struct AuthoredFloorDef {
    pub(crate) name: &'static str,
    pub(crate) describe: &'static str,
    pub(crate) map: &'static str,
}

/// Every tunable number the engine's algorithms read instead of a literal.
pub(crate) struct BalanceDef {
    pub(crate) starting_hp: i32,
    pub(crate) starting_atk: i32,
    /// Light burned per turn before any tax/carry surcharge.
    pub(crate) base_burn: i32,
    /// Extra light burned by a bump-attack, on top of `base_burn`.
    pub(crate) violence_tax: i32,
    /// Run-wide light pool at the start of an attempt. Solver-derived for
    /// this cartridge's own worldgen — see the active cartridge's own
    /// comment on the derivation; a different cartridge derives its own.
    pub(crate) start_light: i32,
    /// Chebyshev sight radius a monster can spot the player from.
    pub(crate) monster_sight: i32,
    /// FOV radius tiers: `(pct_threshold_exclusive, radius)`, checked in
    /// order — the first tier whose threshold `light*100/start_light`
    /// exceeds wins. The last entry's threshold should be low enough (or
    /// use `i32::MIN`) to always match, since this is a total function.
    pub(crate) fov_tiers: &'static [(i32, i32)],
    /// Rendered brightness percent for a given FOV radius (render.rs's
    /// `light_pct`): `(radius, pct)` pairs, exact-match lookup with the
    /// last entry as the fallback for any radius not otherwise listed.
    pub(crate) light_tiers: &'static [(i32, u32)],
    /// Per-depth max-HP/heal grant on a level's FIRST descent.
    pub(crate) hp_gain_per_depth: i32,
    /// Base monster spawn count per depth (added to the depth number).
    pub(crate) spawn_base_count: i32,
    /// Depth-scaled kind roll: `(threshold, monster_index)` pairs checked
    /// in order against `d10 + depth`; the first threshold the roll is
    /// strictly less than wins. The last entry's threshold should be high
    /// enough to always match (e.g. `i32::MAX`).
    pub(crate) monster_roll: &'static [(i32, u8)],
    /// Loot count per depth: `range(lo, hi) + (depth-1) * per_depth`.
    pub(crate) loot_count_lo: i32,
    pub(crate) loot_count_hi: i32,
    pub(crate) loot_count_per_depth: i32,
    /// Loot-kind draw: `chance(num, den)` picks `potion_item`, else
    /// `sword_item`.
    pub(crate) loot_potion_num: u64,
    pub(crate) loot_potion_den: u64,
    pub(crate) loot_potion_item: u8,
    pub(crate) loot_sword_item: u8,
    /// batch 7 T2: per-depth `chance(num, den)` for a loot slot to draw
    /// `loot_bonus_item` instead of rolling potion/sword, indexed by
    /// `depth - 1` — an engine primitive for "a bonus item kind weighted
    /// toward shallow depths," whatever a cartridge wants that item to BE
    /// (see the active cartridge's own balance table for what it names and
    /// why). An entry with `num == 0`, or a depth past the end of this
    /// slice, means "never roll it here" and — the important part — draws
    /// NOTHING from the spawns channel for that check, so depths with no
    /// weight cost zero extra RNG draws versus pre-batch-7 worldgen (see
    /// `Game::gen_level`'s loot loop).
    pub(crate) loot_bonus_chance: &'static [(u64, u64)],
    pub(crate) loot_bonus_item: u8,
    /// Buried-lore item indices by tier (shallow/mid/deep).
    pub(crate) lore_items: [u8; 3],
    /// `receptivity()`'s linear-term coefficients — see that function's doc
    /// comment in game.rs for what each multiplies.
    pub(crate) receptivity_regard_coeff: i32,
    pub(crate) receptivity_wound_coeff: i32,
    pub(crate) receptivity_atk_coeff: i32,
    pub(crate) receptivity_torch_penalty: i32,
    /// A torch-tier at or below this FOV radius triggers the torch penalty.
    pub(crate) receptivity_torch_radius_threshold: i32,
    pub(crate) receptivity_clamp: (i32, i32),
}

/// The win condition: which item ends the run, how it's carried, and where
/// it must be returned.
pub(crate) struct WinDef {
    /// Index into `GameDef::items` of the item whose `ItemEffect` must be
    /// `Objective`.
    pub(crate) objective_item: u8,
    /// Total depth count (was the engine's own `MAX_DEPTH` constant before
    /// the cartridge split — a different game may run a different number
    /// of levels).
    pub(crate) max_depth: u32,
    /// The depth whose up-stairs is the win tile once the objective is held.
    pub(crate) return_depth: u32,
    /// Light burned per turn while carrying the objective (replaces the
    /// base burn, doesn't add to it — see `Game::spend_turn`).
    pub(crate) carry_burn: i32,
}

/// Every player-facing message template that lives in the engine's turn
/// logic (game.rs) rather than in a per-entity table (`MonsterDef::
/// talk_lines`, `ThemeDef::lore`, etc., which already carry their own
/// strings). Templates use a positional `{}` placeholder, filled via
/// `str::replacen` in call-site order — see each field's doc comment for
/// what fills it. Fields with no placeholder are logged verbatim.
pub(crate) struct StringsDef {
    pub(crate) intro: &'static str,
    pub(crate) dark_death: &'static str,
    /// Tier-crossing torch warnings, one per FOV-radius step below the
    /// starting radius. `{}` fills from a theme adjective.
    pub(crate) tier_warnings: [&'static str; 5],
    /// `{}` fills from the theme label.
    pub(crate) enter_theme: &'static str,
    pub(crate) descend_first: &'static str,
    /// `{}` fills from the new depth number.
    pub(crate) descend: &'static str,
    /// `{}` fills from the new depth number.
    pub(crate) ascend: &'static str,
    pub(crate) pit_refuse: &'static str,
    pub(crate) push_too_long: &'static str,
    pub(crate) push_blocked: &'static str,
    pub(crate) push_pit_fill: &'static str,
    pub(crate) push_goal_lock: &'static str,
    pub(crate) push_ok: &'static str,
    /// `{}` fill order: monster name, damage dealt.
    pub(crate) slay: &'static str,
    /// `{}` fill order: monster name, damage dealt.
    pub(crate) hit: &'static str,
    /// `{}` fills from the monster name.
    pub(crate) killed_by: &'static str,
    /// `{}` fill order: monster name, damage taken.
    pub(crate) hit_by: &'static str,
    pub(crate) win: &'static str,
    /// `{}` fills from the theme's objective name.
    pub(crate) need_objective: &'static str,
    /// `{}` fills from the theme's objective name.
    pub(crate) pickup_objective: &'static str,
    pub(crate) lore_prefix: &'static str,
    /// `{}` fill order: adjective, attack bonus.
    pub(crate) atk_item: &'static str,
    /// `{}` fills from the destination's arrival label.
    pub(crate) portal_arrive: &'static str,
    pub(crate) portal_return: &'static str,
    /// `{}` fill order: destination theme label, hex world hash (pre-
    /// formatted by the caller — this template holds no format specifier).
    pub(crate) portal_describe_world: &'static str,
    /// `{}` fills from the destination floor's name.
    pub(crate) portal_describe_floor: &'static str,
    /// `{}` fills from the floor's name.
    pub(crate) floor_arrive: &'static str,
    /// batch 7 T2 (GIVE, byte 11-14): no monster stands in the offered
    /// direction. No-op, no turn.
    pub(crate) give_no_target: &'static str,
    /// batch 7 T2: GIVE attempted with nothing in `Game.held`. No-op, no
    /// turn.
    pub(crate) give_empty_hands: &'static str,
    /// batch 7 T2: a monster is present and something is held, but
    /// `GameDef::give_table` has no row for that (item, kind) pair — an
    /// item's graceful no-give-target state. `{}` fills from the monster
    /// name. No-op, no turn.
    pub(crate) give_declined: &'static str,
    /// batch 7 T2 (USE, byte 15): nothing in `Game.held`. No-op, no turn.
    pub(crate) use_empty_hands: &'static str,
    /// batch 7 T2: the top of `Game.held` has no `ItemDef::on_use`. No-op,
    /// no turn.
    pub(crate) use_no_effect: &'static str,
}
