# C01 — contractor

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | E03, E04 | C02, M01 | L |

**Owns:** `src/games/contractor.rs`, cartridge #1 ("The Contractor"): every game fact as `const` data.

## Purpose
`src/games/contractor.rs` is the one cartridge: a single `pub(crate) const GAME: GameDef` (the contract of
E03) built from `const` tables, with no function, no `impl` and no `cfg`. It holds every game fact the engine
reads through `crate::games::GAME`:
- **Named kind indices** for its own tables: monsters `RAT` 0, `GOBLIN` 1, `OGRE` 2, `TRAINER` 3, `DONKEY` 4,
  `MIMIC` 5; items `POTION` 0, `SWORD` 1, `OBJECTIVE` 2, `LORE_A`-`LORE_C` 3-5, `CHEESE` 6, `COAT` 7,
  `TOWEL` 8, `LIGHT_CACHE` 9 (as `crate::game::MKind` and `IKind`). No engine code names them (some
  comments do); `src/main.rs`'s tests import all but `LORE_A`-`LORE_C`, which only `BALANCE.lore_items`
  uses, and the six only tests use carry `#[allow(dead_code)]`.
- **The roster** (`MONSTERS`, six rows): stats, glyph, colour, talk threshold and `receptivity_base`, the
  four-stage, two-variant talk lines, and the per-creature mercy fields: the ogre's retaliation and stand-tall
  awe, the goblin's give-ground awe with its punish and tell, the trainer and donkey as passive `Yield` and
  `Shove` overworld cast (the donkey follows when calm, the trainer has resurrection lines), the rat tagged
  `Minigame::Echo`, and the mimic (`c`) with `PoliteDecline`, disguised at spawn and the one climb
  re-encounter kind.
- **Items** (`ITEMS`, ten rows): pickup and use lines, `Hold` or `Consume`, effects (the sword's `Hold` ATK
  bonus 2 and its disarm regard 3, the potion's heal 3, the cheese's burn for 8 light, the objective, three
  lore items, the coat and towel with no give target yet, the light cache worth 21), and the **give table**
  (`GIVE_TABLE`, five rules: cheese to rat a regard penalty, potion to rat a full heal, potion to goblin or
  ogre an enrage, cheese to goblin a stay and roll).
- **Flavor**: four `THEMES` (label, objective name, mob names per kind, adjectives, lore templates and slots,
  wall and floor colours), `ROOM_KINDS`, `TONE_LINES`, `GHOST_LABELS` (12, one band of three per outcome),
  the McGuffin's `CARRIED_LINES` per `CarryEvent` and `CARRIED_PREAMBLE`, and `STRINGS` (the engine's message
  templates). Lines cite their `FLAVOR-DRAFT-v0.md` IDs where wired verbatim.
- **Places**: five optional `VAULTS` (three combat rooms, two sokoban rooms), the guaranteed
  `REQUIRED_VAULTS` (the mimic's alcove on D3, THE STAGE with the objective on D5), two `AUTHORED_FLOORS`
  (portal destinations, each with a light cache) and the three `OVERWORLD` screens, whose names, descriptions
  and maps are still placeholders.
- **Balance and win** (`BALANCE`, `WIN`): starting HP 20 and ATK 3, `start_light` 2000 (its comment gives the
  solver derivation: worst-case budget 1476 on seed 82 over seeds 0 to 9,999), burn and violence tax, FOV and
  light tiers, spawn and loot rolls, the receptivity coefficients, kill penalty, rest heal, the mood model and
  shine tiers, the becalm dividend, the per-run cache cap 60, the mimic's accept damage 7; the objective is
  carried from depth 5 to depth 1 at burn 2.

The comments carry the provenance of each value (batch, story section, `[TUNE]` marks); the story canon
(`docs/story/STORY-COMPILE-v1.md`, `docs/story/FLAVOR-DRAFT-v0.md`) outranks them on content.

## Tests
No test sits in this file. Inline tests in `src/main.rs` (run with `PATH="$HOME/.cargo/bin:$PATH"
RUSTUP_TOOLCHAIN=1.75 cargo test --quiet <name>`) check its data against the contract's authoring rules and
its own derivations, named in the criteria below. Most engine tests of E04 run on this data; the
whole-engine checks of the gate (`make goldens`, `make solve`, `make sim`, `make flip`) measure the engine
with this cartridge.

## Changes
none: imported

## Gate
- **C01-AC-1** — The cartridge fills every `GameDef` field, builds warning-free under both backends (`make build`, `make build-term`) and checks on rustc 1.75 (`make msrv`). *Verify:* `make build`, `make build-term` and `make msrv` (targets of `Makefile`).
- **C01-AC-2** — The optional vaults are rectangular with a solid `#` border, an open centre and only the listed legend bytes; each guaranteed vault sits on depth 3 or 5 and holds the mimic's glyph (depth 3) or the objective's glyph (depth 5); on each golden seed's dump, the depth-3 section (its title line included) contains the mimic's glyph and the depth-5 section the objective's glyph. *Verify:* `vaults_well_formed`, `required_vaults_well_formed` and `golden_dumps_show_the_cast_glyphs` in `src/main.rs`.
- **C01-AC-3** — The authored floors and overworld screens are well formed (size, border, one `<` per floor, legend, screen links), and each floor's description, and its name in the test's copy of the arrival line (`You arrive at {}.`), fit the 78-column log row. *Verify:* `authored_floors_well_formed`, `overworld_screens_well_formed` and `authored_floors_flavor_fits_log_row` in `src/main.rs`.
- **C01-AC-4** — These fit the 78-column log row: tone lines with every room kind; every theme's label and objective name in the tests' own copies of the entry and pickup templates; lore lines with every slot; tier warnings with every adjective; every monster's talk lines, `give_declined`, `polite_decline_hurt` and `disguise_reveal` with every theme's mob names; the four fixed give and use strings; every item's pickup and use line; and each give rule's `line`, unfilled. The McGuffin's preamble and lines fit and are ASCII. *Verify:* `theme_lines_fit_log_row`, `talk_lines_fit_log_row`, `give_use_strings_fit_log_row`, `polite_decline_hurt_fits_log_row`, `disguise_reveal_fits_log_row` and `mcguffin_lines_ascii_and_fit_log_row` in `src/main.rs`.
- **C01-AC-5** — Every ghost label is ASCII and at most 16 bytes. *Verify:* `ghost_labels_fit_16_bytes` in `src/main.rs`.
- **C01-AC-6** — The authored floors' round-trip dive costs are 12 and 14, and the light cache's value is the worse of them plus half (21). *Verify:* `authored_floor_dive_costs_match_derivation` and `light_cache_value_matches_derivation` in `src/main.rs`.
- **C01-AC-7** — The McGuffin's stairs-up pool has at least four lines, opens with "As I was saying--" and ends with its documented closer. *Verify:* `stairs_up_pool_is_in_documented_order` in `src/main.rs`.
- **C01-AC-8** — The mimic is the only kind with the climb re-encounter, and it spawns disguised. *Verify:* `climb_reencounter_flag_is_true_only_for_the_mimic` and `mimic_starts_disguised_flag_and_default_spawn` in `src/main.rs`.
- **C01-AC-9** — With this data the five golden seeds generate their committed dumps, on seeds 0 to 9,999 every depth's exit is reachable and the walk-budget statistics sit inside `tests/solver-band.json`, the four bot policies stay inside their bands and the diplomat out-wins the violent bot by the ratified margin. *Verify:* `make goldens`, `make solve`, `make sim` and `make flip` (targets of `Makefile`).
- **C01-AC-10** — Every rendered or authored string in the file is ASCII. *Verify:* unpinned (only the McGuffin's lines and the ghost labels are checked).
- **C01-AC-11** — The file holds only data: no `fn`, no `impl`, no `cfg` block and no platform call. *Verify:* unpinned (the file's header and `CLAUDE.md`'s cartridge doctrine, not a tracked check).
- **C01-AC-12** — `GHOST_LABELS` has a length that is a multiple of four (one band per outcome). *Verify:* unpinned.
- **C01-AC-13** — The named kind indices match their rows in `MONSTERS` and `ITEMS`. *Verify:* unpinned (no test checks the full mapping).
- **C01-AC-14** — `STRINGS.enter_theme` and `pickup_objective` filled with every theme's label and objective name, `STRINGS.floor_arrive` with every floor name, each give rule's `line` and `line_becalmed` filled with every theme's mob names, and the sword's `set_down_line` fit the 78-column log row. *Verify:* unpinned (the tests restate the three templates as literals, check give-rule lines unfilled, and read neither `line_becalmed` nor `set_down_line`).

## Risks
- Changing a value that feeds worldgen (themes, `monster_roll`, loot rolls, vaults, required vaults, room
  kinds) diffs the goldens: a MAJOR change that needs the owner's sign-off first. Changing balance moves the
  solver, sim and flip bands; `start_light` is solver-derived and is not retuned by feel. Swapping the
  cartridge is a MAJOR event for seeds and saves.
- Open placeholders wait on the owner and are not to be guessed: the coat's and towel's give
  targets (the comment on `COAT` and `TOWEL` ties them to the `[YOURS]` D2/D4 casts), the `[YOURS]` lines of
  `FLAVOR-DRAFT-v0.md`, and the overworld screens' content (`docs/story/SPACES-DRAFT-v0.md`). Replacing a
  flavor line is a same-ID text swap, never a structural change.
- `CLAUDE.md` still lists story §12.14's second cheese give-target as open, but it was resolved in batch 13
  T2 as the `CHEESE` to `GOBLIN` row of `GIVE_TABLE` (the comment above `CHEESE_GOBLIN_STAY_LINE`, and
  `docs/design/2026-07-22-mercy-economy-arc.md`); that `CLAUDE.md` line is stale.
- `STRINGS.intro`, `STRINGS.need_objective` and the ghost label "amulet in hand" name the Amulet, which none
  of the four themes' `objective_name` uses (Quiet Bell, Final Ledger, First Lode, Last Index; the file's own
  comment holds out the `NAR_050`-`054` lines for this reason).
- The roster grows with cast batch two (`docs/superpowers/plans/2026-07-27-cast-npc-vault-major.md` §A); a
  new move on a route needs the bot that models it taught in the same change (instrument parity).
- `golden_dumps_show_the_cast_glyphs` searches each depth's whole dump section, its title line included. On
  seeds 1 and 1337 depth 3 is "the salt counting-house", whose title contains a `c`, so the depth-3 check
  passes there even with no mimic on the map; it relies on seeds 2, 3 and 42 to catch a missing mimic.
- The data tests live in `src/main.rs`, which only the implementer may write until the tests move to
  `src/tests/*.rs` (`SPEC.md`'s open decisions).
