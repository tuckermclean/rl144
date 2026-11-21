# D03 — design-arc

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | none | M |

**Owns:** `docs/design/2026-*.md`, the dated design docs: the mercy-economy arc and the road-to-1.0 plan of record.

## Purpose
Two design docs that rank below the story canon (D01) and above the batch plans (D07):
- `2026-07-22-mercy-economy-arc.md`, "light as grace, violence as doom": the diagnosis from the batch-9 playtest,
  the north star (violence intuitive but doomed, diplomacy the reliable path that still costs), the two
  doom-engines (HP attrition, and the McGuffin as a second lantern whose light follows its mood, redesigned
  2026-07-24), the measurement reframe (the tactical bots of batch 10), the overworld's two teachers (the trainer,
  the donkey's three-rung kindness ladder), the batch arc 10 to 13 and its ride-alongs (becalm dividend, cheese
  target, per-creature becalm toolkit, goblinoid awe), the sign-off and MAJOR flags, and the **Ratification
  (2026-07-25)**: the thesis is the relational invariant `diplomat_win% >= violent_win% + margin` (margin 3 points
  at N=5000) plus the diplomat band's lower bound (30) as a thesis-bearing floor, both enforced by
  `--sim-flip` in `make check`.
- `2026-07-26-road-to-1.0.md`, the plan of record (`CLAUDE.md`): the order cast, then ending-as-exam, then
  polish and ship; the terminal-mechanics manifest (anything not on it is post-1.0); the NPC-vault worldgen MAJOR
  sign-off package; the gate of this stretch (the human plays the mimic between cast batch one and cast batch
  two); the count (5 to 6 batches, expect 7); and the finish line, a `SHIP.md` acceptance list gated by
  `make ship` and a `make floppy` image. It calls itself "the ratchet, not the authority": each batch's source is
  the story canon.

## Tests
None: no tracked test reads these files. The arc's ratified invariant is enforced by `--sim-flip` against
`tests/tactical-pacifist-band.json` (nodes S02, M03), which holds the margin, N and the floor; this text is not
checked against them.

## Changes
none: imported

## Gate
- **D03-AC-1** — The arc doc's "Ratification (2026-07-25)" section states the thesis as the relational invariant `diplomat_win% >= violent_win% + margin` with margin 3 at N=5000, and the diplomat band's lower bound 30 as a ratified floor. *Verify:* unpinned (prose; the numbers it names are pinned by M03's band file, not this text).
- **D03-AC-2** — The road-to-1.0 doc fixes the order cast, ending-as-exam, polish/ship, lists the terminal mechanics, and makes a human playtest of the mimic the gate before cast batch two. *Verify:* unpinned.

## Risks
- The road-to-1.0 doc is older than the signed-off cast package (`docs/superpowers/plans/2026-07-27-cast-npc-vault-major.md`, D07), whose human rulings changed two of its points: sign-off item 4 (sword custody as a
  stat-flag `disarm`, the sword staying `Consume`) was replaced by ruling ① (sword `Hold` plus set-down-for-regard,
  which batch 17 shipped), and donkey rung 3, listed here as post-1.0, is IN for 1.0 by ruling ② (as `CLAUDE.md`
  says). The doc carries no inline note of either override: the rulings are recorded in the lower-ranked plan,
  and the code follows the rulings. `CLAUDE.md`'s precedence rule asks for such a contradiction to be fixed in
  the lower doc or recorded as an inline human override.
- `SHIP.md`, `make ship` and `make floppy` do not exist yet: they are the planned finish line, not current state.
- The mimic's playtest is still open (owner, answer Q3a), so the road's next step has not started.
- The arc doc's "Status" line still says "design canon (draft)"; its later sections record human ratification.
- Both files contain non-ASCII characters; they are not rendered.
