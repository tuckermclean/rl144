# E03 — gamedef

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | E02, E04 | M |

**Owns:** `src/gamedef.rs`, the `GameDef` cartridge contract.

## Purpose
`src/gamedef.rs` is the data contract between the engine and a cartridge: pure types with no function, no
`impl` block, no platform call and no `cfg`. `GameDef` holds every game fact the engine reads: the monster,
item and theme tables (`MonsterDef`, `ItemDef`, `ThemeDef`), room kinds and tone lines, the vault blueprints
and the guaranteed per-depth vaults (`required_vaults`), the authored floors (`AuthoredFloorDef`), the ghost
labels, the balance numbers (`BalanceDef`), the win condition (`WinDef`), the engine's message templates
(`StringsDef`), the give table (`GiveRule`), the McGuffin's lines (`carried_lines` keyed by `CarryEvent`,
`carried_preamble`) and the overworld (`OverworldDef`, `OverworldScreenDef`). The enums `Minigame`,
`BumpResponse`, `ItemEffect`, `PickupBehavior` and `UseEffect` name the per-kind behaviours the engine
dispatches on.

Tables a second cartridge could size differently are `&'static [T]` slices; fixed-size arrays hold shapes the
engine assumes, among them four adjectives, three lore templates and four slots per theme, four talk stages
times two variants, five tier-crossing warnings and three overworld screens. The file's header names "six
FOV/light tiers" among them, but `fov_tiers` and `light_tiers` are slices. Field doc comments carry the
authoring rules, such as the vault legend (a def's `glyph` is its legend byte) and `ghost_labels`' length
being a multiple of four. The one cartridge, `src/games/contractor.rs`, fills it; that file belongs to the
`cartridge` area, still to map.

## Tests
No test exercises this file alone: it has no logic, and the build (`make build`, `make build-term`) checks
that the cartridge fills it. Inline tests in `src/main.rs` (run with `PATH="$HOME/.cargo/bin:$PATH"
RUSTUP_TOOLCHAIN=1.75 cargo test --quiet <name>`) check the cartridge's data against the authoring rules the
contract documents: `vaults_well_formed`, `required_vaults_well_formed`, `authored_floors_well_formed`,
`overworld_screens_well_formed`, `ghost_labels_fit_16_bytes`, `talk_lines_fit_log_row`,
`give_use_strings_fit_log_row`, `mcguffin_lines_ascii_and_fit_log_row`.

## Changes
none: imported

## Gate
- **E03-AC-1** — The contract compiles with the cartridge filling every field, warning-free, under both backends and on rustc 1.75. *Verify:* `make build`, `make build-term` and `make msrv` (targets of `Makefile`).
- **E03-AC-2** — The cartridge's optional vaults are rectangular, with a solid `#` border, an open centre and only the legend bytes the test lists; each guaranteed vault is rectangular with a solid `#` border, holds only `#`, `.`, the mimic's and the objective's glyphs, shows its own depth's glyph (the mimic's on depth 3, the objective's on 5) and sits on depth 3 or 5. *Verify:* `vaults_well_formed` and `required_vaults_well_formed` in `src/main.rs`.
- **E03-AC-3** — Authored floors are rectangular, within 80×25, `#`-bordered, with exactly one `<` and only the listed legend bytes; overworld screens are within 80×25 and `#`-bordered except `=` links on edge columns, screen 1 linking east only, screen 2 both ways and screen 3 west only. *Verify:* `authored_floors_well_formed` and `overworld_screens_well_formed` in `src/main.rs`.
- **E03-AC-4** — Every ghost label is ASCII and at most 16 bytes (the RLG1 `label_idx` contract). *Verify:* `ghost_labels_fit_16_bytes` in `src/main.rs`.
- **E03-AC-5** — Each monster's talk lines, filled with its name in every theme, `give_declined`, filled with every theme's mob names, the four fixed give/use strings, each item's `pickup_line` and `use_line`, each give rule's `line`, and the McGuffin's preamble and lines fit the 78-column log row, and the McGuffin's lines are ASCII; a give rule's `line_becalmed` is not checked. *Verify:* `talk_lines_fit_log_row`, `give_use_strings_fit_log_row` and `mcguffin_lines_ascii_and_fit_log_row` in `src/main.rs`.
- **E03-AC-6** — The file holds no logic (no `fn`, no `impl`), no `cfg` block and no platform call. *Verify:* unpinned (the file's own header and `CLAUDE.md`, not a tracked check).
- **E03-AC-7** — `ghost_labels`' length is a multiple of four. *Verify:* unpinned (documented on the field, not checked).

## Risks
- A field added here must be filled by every cartridge, and read only through `crate::games::GAME`; a game
  fact that leaks into engine code instead of a field here is a bug by the cartridge doctrine.
- Changing a fixed array's size (theme slots or lore templates, talk stages, tier warnings) changes engine
  algorithms. The four slots and three lore templates per theme are also literals in `theme_pick`'s slot draws
  (E02), which fix each seed's lore lines: a MAJOR change that no golden catches.
- `GiveRule` names `crate::game::IKind` and `crate::game::MKind` (`u8` aliases in `src/game.rs`), so the
  contract refers back to E04. The graph cannot list that as a dep (E04 depends on E03), so a change to those
  aliases is a change to this contract too.
- Swapping or reshaping the cartridge's data is a MAJOR event for seeds and saves (`CLAUDE.md`).
