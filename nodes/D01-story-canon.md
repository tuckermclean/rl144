# D01 — story-canon

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | none | M |

**Owns:** `docs/story/STORY-COMPILE-v1.md`, `docs/story/FLAVOR-DRAFT-v0.md`, the human-locked story canon: the narrative design and its line pool.

## Purpose
The two files the project treats as its design canon on game content (`CLAUDE.md`, "Design canon"; `SPEC.md`
ranks them above every spec on game content). They are prose and data for people and agents; the binary never
reads them.
- `STORY-COMPILE-v1.md` ("The Contractor, the McGuffin, and Half a Donkey") states the game in sections §0 to
  §13: the three-sentence game and its comic mechanism (§0), the hard rules (§1), the four voices (§2), the beat
  sheet from title screen to death and respawn (§3), the cast floor by floor, D1 rats to D5 tired ones (§4), the
  items (§5), lore tiers and vaults (§6), deaths, ghosts and epitaphs (§7), the line budget (§8), the engine asks
  A to K (§9), horizon-proofing (§10), tuning dials (§11), the master `[YOURS]` index of 20 human-only slots
  (§12) and what died (§13). Its header defines the markers `[YOURS]` (a line only a human writes), `[TUNE]` (a
  number found in playtesting) and `[ENGINE]` (an ask priced in §9).
- `FLAVOR-DRAFT-v0.md` is the line pool: 250 lines of the form `ID | text`, each ID unique, grouped by voice
  (narrator `NAR_`, McGuffin `MCG_`, the cast `RAT_`/`COA_`/`MIM_`/`LOS_`/`TIR_`, the overworld, deaths `DTH_`,
  epitaphs `EPI_`, title, contract and endings). Its header says every line is a placeholder whose text may be
  swapped without touching its ID, that lines are at most 78 characters, and that lines marked `# LOCKED` (9 of
  them) were canonized and are replaced with care.
The cartridge (`src/games/contractor.rs`, node C01) carries the lines it uses as `const` strings and cites their
IDs in comments; `CLAUDE.md` makes a text swap a same-ID change, never a structural one.

## Tests
None: no tracked test or `make check` step reads these files. The cartridge's copies of the lines are checked
only through the goldens and unit tests of the nodes that own the code (C01, M01, M02), not against this text.

## Changes
none: imported

## Gate
- **D01-AC-1** — `STORY-COMPILE-v1.md` has the sections §0 to §13 listed above, with §9 naming the engine asks A to K and §12 listing the human-only `[YOURS]` slots. *Verify:* unpinned (prose).
- **D01-AC-2** — `FLAVOR-DRAFT-v0.md` holds 250 lines of the form `ID | text`, with no ID repeated and no line's text (before a trailing `# LOCKED` comment) longer than 78 characters. *Verify:* unpinned.
- **D01-AC-3** — Canon precedence: on game content `STORY-COMPILE-v1.md` outranks the design docs (`docs/design/`), which outrank the plans and briefs (`docs/superpowers/`); a lower doc that contradicts a higher one is fixed, or carries an inline human override (`CLAUDE.md`, "Process discipline"). *Verify:* unpinned.

## Risks
- Both files contain non-ASCII characters (em-dashes, arrows, curly quotes): 125 lines of the story compile, 58
  of the flavor draft. The renderer is byte-indexed, so a line copied into the cartridge must be normalized to
  ASCII first (`--` for an em-dash), as `CLAUDE.md`'s ASCII rule says; nothing checks the copies against this file.
- `CLAUDE.md` speaks of "most `[YOURS]` lines in `FLAVOR-DRAFT-v0.md`", but the flavor draft has no `[YOURS]`
  marker: its header calls every line a placeholder, and the `[YOURS]` markers (24 lines) are in the story
  compile, indexed by §12.
- §9's checklist status (A/B/C/D/E/H/J done, F in progress, G/I/K not briefed) is tracked in `CLAUDE.md`, not
  here; the compile itself records no progress. The road-to-1.0 plan (D03) warns that §9-F was mis-scoped twice
  from memory: a scope call cites the section, opened.
- The open slot of §12.14 (which monster cheese works on, and its line): `CLAUDE.md` lists a second cheese
  give-target as still open, which C01's Risks find resolved in batch 13; the compile does not say.
