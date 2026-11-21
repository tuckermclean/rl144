# D02 — spaces-draft

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | none | S |

**Owns:** `docs/story/SPACES-DRAFT-v0.md`, the staged spatial content: ASCII maps and notes for the vaults and the overworld screens.

## Purpose
The companion to the story canon's §3.2 and §6 (D01): ASCII maps with design notes, keyed by stable IDs whose maps
may be swapped freely. Its sections: §1 `SPC_MIDDEN`, the D1 cheese-ring vault around a tombstone; §2
`SPC_STAGE`, the D5 pickup vault; §3 `SPC_OVR_1` to `SPC_OVR_3`, the three overworld screens (the posting and
the hole `V`, the trainer's yard, the collector's door and the donkey); §4 the `SPC_GAG_*` sight-gag vaults of
the propose-and-strike round; §5 the consolidated table of glyphs that needed an engine legend (`o`, `T`, `%`,
`,`, `&`, `V`, `+`, `Y`, `D`, `=`). Its header records the vault and authored-floor legends the engine had when
it was drafted, and notes that an ordinary vault's corridor is punched straight to its centre, which the gag
vaults use as their punchline.
The overworld of batch 9 adopted its glyphs and layout from here (`src/games/contractor.rs` and
`src/gamedef.rs` cite this file); `CLAUDE.md` records the three screens' strings as minimal placeholders and the
Midden and Stage vaults as not yet consumed.

## Tests
None: no tracked test reads this file. The overworld it shaped is frozen by `--dump-overworld` and the unit tests
of the code's nodes (C01, S02, M01), not against these maps.

## Changes
none: imported

## Gate
- **D02-AC-1** — The file names the space IDs `SPC_MIDDEN`, `SPC_STAGE`, `SPC_OVR_1` to `SPC_OVR_3` and `SPC_GAG_*`, each with an ASCII map with a solid rectangular `#` border, and ends with the consolidated glyph table. *Verify:* unpinned (prose).

## Risks
- The header's legend is "as read from `src/content.rs` at draft time"; since the cartridge split (batch 7) the
  vault and authored-floor tables live on `GameDef` in `src/games/contractor.rs`, and later batches added glyphs
  (the cast, the mimic's disguise), so the legend is a historical snapshot.
- The maps are not compared with the cartridge's: a change to one does not show in the other.
- The file contains non-ASCII characters (83 lines); a string copied into the cartridge is normalized first.
