# D05 — proposals

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | none | M |

**Owns:** `docs/design/proposals/`, the endgame proposals of 2026-07-18 and the human decision record that governs the blended roadmap.

## Purpose
The record of how the endgame direction was chosen:
- `0-brief.md`: the shared brief the three proposals were written against (the commission, the hard constraints
  of the time, the deliverable's shape).
- `A-authored-dungeon.md`, `B-haunted-floppy.md`, `C-impossible-object.md`: three competing end-state proposals
  (authored game content first; ghosts and seed-sharing; presentation: sprites, synth, screen-feel, mobile), each
  ending with its rebuttal round.
- `SYNTHESIS.md`: the controller's adjudication and recommended blend (Phase 1 substrate, Phase 2 mercy-by-gate,
  Phase 3 presentation, Phase 4 ghosts/mobile/net), marked as not signed off.
- `DECISION.md`: the human's decision (2026-07-18) adopting the blend with Proposal C in full, and the five
  sign-offs it grants (violence tax and its band re-baseline; input vocabulary v2 with a `SAVE_VERSION` bump;
  mercy plus the pacifist band; the NPC-vault worldgen MAJOR; the `scene()` core surface). `CLAUDE.md`'s
  roadmap context makes it the governing record: its sign-offs are not re-requested, anything it does not list
  needs fresh sign-off. Code comments cite its items (`DECISION.md item 3`, `item 4`).

## Tests
None: no tracked test reads these files.

## Changes
none: imported

## Gate
- **D05-AC-1** — `DECISION.md` lists exactly five granted sign-offs, numbered 1 to 5 as above, and the standing constraints it leaves unchanged (floppy budget per target, no asset files, dependency freeze, dump-golden doctrine, headless-first gates, MSRV 1.75). *Verify:* unpinned (prose).

## Risks
- `DECISION.md`'s "C-complete guarantee" (sprites, full audio stack, `backend-wasm` mobile) is overtaken by later
  human calls: Phase 3 presentation deferred behind story mechanics (2026-07-19, `CLAUDE.md`) and the road-to-1.0
  plan (D03) putting audio, sprites, wasm/mobile and net post-1.0. The file itself carries no note of this.
- The brief's numbers are of 2026-07-18 (for example packed sizes of 192,144 and 169,724 bytes, a core without
  `gamedef`/`games`); they are history, not current facts.
- The files contain non-ASCII characters; they are not rendered.
