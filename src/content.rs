// content.rs — generic content helpers: small algorithms that read the
// active cartridge's data (`crate::games::GAME`, reached only through that
// one seam) but contain no game-specific fact of their own. Everything that
// USED to be authored data here (themes, vaults, talk lines, ghost labels)
// moved to `gamedef.rs`'s types and `games/contractor.rs`'s data — this file
// keeps only the lookups any cartridge would share, plus the engine's own
// UI-chrome palette (tile/status colors that belong to the fixed Tile enum
// and status bar, not to any one game's monsters/items).

use crate::games::GAME;
use crate::rng::channel;

/// Per-depth theme identity: theme index plus one slot index per lore tier
/// (templates 0/1/2 = shallow/mid/deep — the story assembles in act order
/// as the player pushes deeper). Its own channel, so flavor can never
/// perturb worldgen or spawns.
pub(crate) fn theme_pick(seed: u64, depth: u32) -> (usize, [usize; 3]) {
    let mut tr = channel(seed, &["theme", &depth.to_string()]);
    let ti = tr.range(0, GAME.themes.len() as i32) as usize;
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
/// e.g. depth 1 for the title, the max depth for the objective's name)
/// share one implementation.
pub(crate) fn theme_for(seed: u64, depth: u32) -> &'static crate::gamedef::ThemeDef {
    &GAME.themes[theme_pick(seed, depth).0]
}

/// The filled lore line for a given seed+depth+tier — pure f(seed, depth,
/// tier), no RNG draws. Shared by `Game::lore_line` (live pickup flavor)
/// and the Title screen (depth-1 tier-0 preview).
pub(crate) fn lore_line(seed: u64, depth: u32, tier: usize) -> String {
    let (ti, slots) = theme_pick(seed, depth);
    let t = &GAME.themes[ti];
    t.lore[tier].replace("{A}", t.slots[slots[tier]])
}

/// Deterministic ghost-label selection: no RNG channel, just outcome and
/// final depth. `outcome` picks a band of `GAME.ghost_labels` sized by the
/// 4 known outcomes (see save.rs's GHOST_* consts); `final_depth` picks
/// within the band. Sized off `GAME.ghost_labels.len()` rather than a
/// hardcoded number so the table and the selector can't silently drift
/// apart.
pub(crate) fn ghost_label_idx(outcome: u8, final_depth: u8) -> u8 {
    let per_outcome = (GAME.ghost_labels.len() / 4) as u8;
    let band = outcome.min(3) * per_outcome;
    band + final_depth % per_outcome
}

// ---------- Engine UI-chrome palette ----------
/* Colors for tile kinds and status/log chrome the ENGINE itself defines
   (Tile::Stairs/Portal/Pit/Goal, the player glyph, status text, HP/Torch
   bars) rather than any one game's monsters/items — those live on
   `MonsterDef`/`ItemDef` instead (see gamedef.rs), one color per def, no
   separate palette table to drift out of sync. */
pub(crate) const PAL_PLAYER: u32 = 0xFFFFFF;
pub(crate) const PAL_STAIRS: u32 = 0xFFFF60;
pub(crate) const PAL_STATUS: u32 = 0xE0E0E0;
pub(crate) const PAL_ALERT: u32 = 0xFF5050;
/// Narrative/flavor text color (Title/End screen lore preview and win
/// message) — engine UI chrome, independent of any item's own render color
/// (`ItemDef::color`), even where a cartridge's lore items happen to share
/// the same hue.
pub(crate) const PAL_LORE: u32 = 0xC0A0FF;
pub(crate) const PAL_LOG_FADE: [u32; 4] = [0x707070, 0x909090, 0xB0B0B0, 0xE0E0E0];
pub(crate) const PAL_BAR_HP: u32 = 0x50C050;
pub(crate) const PAL_BAR_TORCH: u32 = 0xE0A030;
pub(crate) const PAL_BAR_EMPTY: u32 = 0x404040;
/// Becalmed-monster tint (batch 5 task 3, frontend half of the Henson
/// ruling): a fixed soft blue-gray, chosen over blending with the kind's own
/// color so a calmed monster reads at a glance as "not a threat" regardless
/// of which kind it was, the same way a status-effect tint works in most
/// roguelikes. `render::render_play` substitutes this wholesale for the
/// kind's own color (not a blend) when `Monster::calm` is true, then applies
/// the same `scale(_, pct)` light-tier dimming every other monster glyph
/// gets.
pub(crate) const PAL_CALM_TINT: u32 = 0x9AB0C0;
/// Portal glyph color (batch 6 T1): a saturated magenta, chosen because it
/// sits outside typical earthy/muted wall+floor palettes — reads as "not of
/// this place," the whole point of a door to somewhere else.
pub(crate) const PAL_PORTAL: u32 = 0xFF40FF;
/// Pit glyph color (batch 6 T2, sokoban): a dull red-black void, reads as
/// hazard without competing with `PAL_ALERT`.
pub(crate) const PAL_PIT: u32 = 0x502020;
/// Goal-tile glyph color (batch 6 T2, sokoban): a pale gold-green, distinct
/// from the pit's red-black and the portal's magenta — reads as "aim here."
pub(crate) const PAL_GOAL: u32 = 0xB0D080;
/// Push-block glyph color (batch 6 T2, sokoban): plain stone gray, legible
/// against typical floor colors and distinct from pit/goal/portal.
pub(crate) const PAL_BLOCK: u32 = 0x9A9A9A;
/// Screen-link glyph color (batch 9 T1, story §9-J prep): a cool cyan,
/// distinct from the portal's magenta — reads as "a fixed way through," not
/// a rolled destination.
pub(crate) const PAL_SCREENLINK: u32 = 0x40D0E0;
/// Hole glyph color (batch 9 T1): a deep near-black blue — the one true way
/// down from the overworld into the dungeon, visually heavier than the
/// screen-link's cyan.
pub(crate) const PAL_HOLE: u32 = 0x202840;
/// Shut-door glyph color (batch 9 T1): a dull iron gray-brown, reads as
/// "closed" without competing with the pit's red-black or the alert color.
pub(crate) const PAL_SHUTDOOR: u32 = 0x706050;
