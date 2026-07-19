# Proposal A — The Authored Dungeon

## 1. Thesis

The floppy succeeds or fails on the GAME, not the frame around it. The review I'm engineering
for: *"it's ugly-charming like Peasant's Quest, then somewhere around depth 3 you realize you
feel bad about the goblin you didn't have to kill, and the game remembers that you spared it."*
Verbs, mercy, memory, and writing are what make a 1.44MB roguelike a *story* instead of a demo.
They are also nearly free: `content.rs` already proves it — `THEMES`, `TONE_LINES`, and `VAULTS`
are ~200 lines of const data driving all of the game's current personality, and the packed
binary is 192,144 B against a 1,474,560 B ceiling (13% used). Text and branching logic are the
cheapest bytes this project can spend. Pixels are the most expensive. I spend accordingly: every
byte on verbs and words before a single byte goes to spectacle.

## 2. The floppy at v1.0

**Unchanged baseline** (current repo, both backends already built, core/crust split already
landed): rng/content/game/headless/save/render core, `backend-minifb` (640×360, resizable,
aspect-stretch) and `backend-term` (ANSI). Packed: minifb 192,144 B, term 169,724 B.

**New at v1.0**, all additive to the existing const-table doctrine:

| System | What | Est. packed delta |
|---|---|---|
| ACT verb + mercy-light economy | new input byte, `regard`/`spared` monster state, violence tax | ~3 KB |
| Pacifist sim-bot policy | `--sim --policy pacifist`, gates the mercy path | ~2 KB |
| USE-ITEM verb (minimal inventory) | deliberate consumable use, no grid UI | ~1 KB |
| NPC entity kind + NPC vaults | new `Npc` struct, new vault legend char, stamped via existing vault channel | ~4 KB |
| Dialogue data model + `Screen::Dialogue` | const `NpcDef`/`DialogueNode` tables, ~8 NPCs × ~15 nodes | ~20–28 KB (raw ~30 KB, text compresses well under LZMA) |
| Endings matrix | pure `fn ending_id(&Game)` + 8 short ending texts | ~3 KB |
| Retry-same-seed verb + ledger file | new input byte, 32-byte persistent ledger, ledger-aware coda lines | ~3 KB |
| Monster "regard" flavor lines | 3 kinds × 4 themes × mercy-stage lines | ~5 KB |

**Sum: ~40–45 KB packed delta.** New total: minifb ≈ 233–237 KB (16%), term ≈ 211–215 KB (14.5%).
That leaves **over 80% of the budget unspent** at v1.0. This is deliberate, not timidity — see
§6. It also clears the brief's 20%-margin bar by a wide margin without trying.

## 3. Presentation architecture

**SVGA pixel art with zero asset files.** I do NOT build a sprite layer for v1.0. The existing
`Cell{ch:u16, fg:u32, bg:u32}` grid (`render.rs`) already carries theme-tinted glyphs, wall
autotiling, and light-tier dimming — that *is* the pixel-art register at this budget: chunky
8×12 glyph cells, a small authored palette per theme (`Theme.wall`/`Theme.floor`, `PAL_*`
consts), rendered at 640×360 today. "SVGA style" reads through glyph density and palette
discipline, not painted sprites — Dwarf Fortress and Cogmind both prove this at far larger
scope. If a backend wants a chunkier "SVGA-ish" canvas, that's a `CW`/`CH` multiplier in
`backend_minifb.rs` alone (e.g. 2× → 1280×720) — zero core change, since the Cell grid stays
the fixed 80×30 abstraction `idx()`/worldgen require.

**The one place I *do* design a new core surface, explicitly, and defer building it:** NPC
dialogue portraits. If signed off post-v1.0 (batch 8, §5), add a `PortraitBitmap` — a
`&'static [u8; 128]` (16×16 px, 4bpp, 16-color palette per theme) referenced by `NpcDef`, and a
new render output alongside cells for `Screen::Dialogue`: `render_portrait(&Game) -> Option<&'static [u8;128]>`.
Pixel backends composite it into a reserved corner region; the term backend degrades to a
box-drawn ASCII face (it already draws box-drawing glyphs for status bars, so this is idiomatic,
not new tech). Byte math: 12 NPCs × 128 B raw = 1,536 B raw, plausibly ~1–1.5 KB packed since
4bpp bitmaps don't compress like text but are tiny in absolute terms. Justification under the
doctrine: this is the *only* place pixel art earns character-per-byte the way text already does
— a face you look at for 15 seconds of dialogue, not a walk cycle you glance past. I do not build
it before batches 4–7 land and are confirmed fun; building it first is exactly the "spectacle
before the talk verb" mistake I'm arguing against.

**Mobile portrait + landscape.** Landscape is already solved: `AspectRatioStretch` letterboxes
the fixed 640×360 buffer into any window, phone or desktop. Portrait is the real problem: 80×30
is an 8:3 grid, landscape-shaped by nature, and `COLS`/`ROWS` are frozen engine API (baked into
`idx()` and worldgen — CLAUDE.md is explicit that the grid must never follow window size, and
resizing the *grid* would be a MAJOR nobody should sign off on). My answer: **letterbox, don't
recompose.** Portrait devices get the same landscape-shaped frame with generous top/bottom bars
— the same tradeoff console emulators and many mobile roguelikes (Brogue included, historically)
accept. It's honest, it's a backend-only change, and it costs nothing. A "true" portrait
recomposition (stacked map/status/log panels reflowing independently) is out of scope: it would
require either a second grid geometry or a richer core surface than Cell, and I'm not spending
engineering time on it before the writing is proven.

Backend for mobile: not minifb (X11/Cocoa/Win32 windowing, not mobile-portable) and not a new
crate. Cheapest credible path is a `backend-web` feature: core compiled to `wasm32-unknown-unknown`
with `#[no_mangle] extern "C"` exports, driven by a hand-rolled ~100-line JS canvas blitter that
ships as a hosted file, not a Rust dependency — zero new Cargo deps, satisfies the "any frontend
producing bytes 0–8 is a valid client" doctrine. Touch input maps to the same vocabulary: a
4-quadrant D-pad for moves, center tap for wait, two buttons for ACT/USE. This is batch 9 (§5),
after the game is good, not before.

**Audio.** I commit to shipping v1.0 **silent**, and I do not treat that as a placeholder — it's
a real design position: a full tracker + song-data system (the old 20–30 KB roadmap reservation)
is spectacle-before-substance at exactly the scale I'm arguing against, and a mediocre chiptune
loop can actively undercut the Undertale-sincere beats I'm banking the whole pitch on. What I
*do* commit to, deferred to batch 8 alongside portraits: a hand-rolled square/triangle-wave
synth (~150–250 lines, no dependency — this clears the "impossible in <150 lines" bar honestly,
it's borderline, so it needs its own size measurement before merge) driving ~8 short procedural
stingers (mercy-spare chime, hit, death, win) — not a song system, not ambient loops. Budget
~5–8 KB, well under the old 20–30 KB reservation, because stingers are what Undertale actually
uses to punctuate mercy/violence choices; full music is not required to land that beat.

**Resizable stays resizable.** Nothing here touches `backend_minifb.rs`'s resize/`AspectRatioStretch`
path; the Cell grid is still the only thing backends rasterize.

## 4. The fusion design

**Verbs.** Input vocabulary grows from 0–5 to 0–8 (save v2, MINOR to the vocabulary, versioned
per doctrine):
- 0–3 move/bump-attack (unchanged — bumping a monster still deals damage, unchanged code path)
- 4 wait (unchanged)
- 5 restart-reroll (unchanged — `h64(seed,["restart"])`)
- **6 restart-same-seed (NEW)** — `Game::new(g.seed)`, no reroll. Answers the batch-3 finding
  directly: dying (or choosing to reset) no longer forces a new world; the player can retry the
  world they just learned, and the ledger (below) makes that retry legible to the NPCs.
- **7 ACT (NEW)** — resolves against the nearest adjacent live `Monster` or `Npc` (deterministic
  tie-break: nearest, then lowest index — no new "facing" state needed, so `state_hash` doesn't
  grow a direction field). On a monster: increments `regard: u8` (0..=REGARD_MAX, tuned per
  `MKind` — rats calm in 1–2 ACTs, ogres take 3–4, matching escalating stakes by depth). At max,
  the monster is removed from `monsters` (same as a kill for exit-reachability purposes) but
  increments `spared: u32` instead of `kills`. Costs a normal turn (1 or 2 light per `spend_turn`,
  same as any action) — no tax. On an `Npc`: opens `Screen::Dialogue` at the tree's current node.
- **8 USE-ITEM (NEW)** — applies the most recently picked-up unused consumable (LIFO, no grid
  UI). Satisfies the roadmap's "deliberate item use" cut item with the cheapest possible verb,
  not a UI subsystem.

**Violence tax.** In `try_move_player`'s attack branch (`src/game.rs:649-659`), after computing
`dmg` and before `spend_turn()`, subtract an additional `VIOLENCE_TAX: i32 = 1` from `self.light`.
This is the whole mechanic: "violence burns light" is one line of code, and it changes the
incentive shape of every fight. It also directly answers the batch-3 finding (100% combat
deaths, ~0.1% dark deaths, sim-band comment confirms greedy-bot walking dominates light burn) —
not by nerfing combat damage (a separate, already-fragile balance surface per `sim-band.json`),
but by giving darkness teeth specifically for players who choose violence repeatedly.

**The pacifist sim-bot gate.** Because mercy (multiple ACT turns per monster) and violence
(tax per swing) both cost light on different curves, this needs its own winnability gate,
exactly as the brief anticipates. New `--sim --policy pacifist` in `headless.rs` (always ACTs
adjacent to monsters instead of bumping; flees when regard is interrupted by an unavoidable
hit) and a new `tests/pacifist-band.json`, same shape as `sim-band.json`. The concrete,
falsifiable claim this batch must demonstrate before merge: **the pacifist bot's death
distribution inverts the greedy bot's** — mostly `deaths_dark`, a minority `deaths_combat` —
proving mercy trades HP-safety for light-risk rather than being strictly dominant or strictly
worse. If it can't hit a defensible band, the tax constant or `REGARD_MAX` values get tuned
before merge, not after.

**NPCs and dialogue as const tables**, extending the exact pattern `THEMES`/`VAULTS` already
establish:

```rust
struct DialogueNode {
    text: &'static str,                          // may contain {K}/{A}-style slots, same
                                                    // template convention as TONE_LINES/Theme.lore
    options: &'static [(&'static str, u8)],       // (label, next node id); empty = terminal
}
struct NpcDef {
    name: &'static str,
    theme_idx: usize,                              // grounds voice in the depth's authored theme
    tree: &'static [DialogueNode],                  // node 0 = entry
    memory_variant: Option<&'static [DialogueNode]>, // alt node 0 if ledger shows a prior meeting
}
const NPCS: [NpcDef; 8] = [ /* 2 per theme × 4 themes */ ];
```

NPCs are stamped via a new `NPC_VAULTS: [&str; N]` table, using the *same* vault-stamping code
path as `VAULTS` (`content.rs:172-200`) with one new legend char for "place `NPCS[k]` here."
This is a worldgen-adjacent change (new vault channel draws) — **MAJOR, human sign-off
required**, goldens regenerate. Flagged loudly in §5.

**Grounding doctrine, extended.** CLAUDE.md's existing rule — "flavor text may only restate
things the engine did, never invent entities, exits, or events" — now covers dialogue: an NPC
may reference `depth`, `theme`, `has_amulet`, `kills`, `spared`, and whether *this* NPC was
already talked to this run or a prior one (via the ledger), and nothing else. This is why
dialogue is written by whoever implements the mechanical system, in the same PR, not handed to
a separate "narrative pass" — a writer without exact knowledge of what state exists will invent
ungrounded fluff, which is precisely what the doctrine forbids.

**Writing registers by depth**, extending the exact shallow/mid/deep tier structure `Theme.lore`
already has: depth 1–2 NPCs run Peasant's Quest register (punny names, absurd-but-grounded
asides, comedy from delivery and juxtaposition, never invented events); depth 4–5 NPCs shift to
Undertale-sincere weight (the mercy stakes read as real, not funny, especially the two NPCs
seeded near the depth-5 amulet room). Volume: ~8 NPCs × ~12–18 nodes × 60–120 chars ≈ 18 KB, plus
~7 KB monster regard lines, ~3 KB endings, ~2 KB ledger callbacks ≈ **~30 KB raw text**, the low
end of the brief's 30–50 KB reservation — deliberately: precision over volume, every line earns
its byte.

**Endings matrix.** Pure function `fn ending_id(g: &Game) -> u8`, same shape as `theme_for`: True
Mercy (won, `kills==0`), Mercy (won, `spared>kills`), Neutral (won, `kills>=spared`), Violent
(won, `spared==0`, `kills` above a threshold), plus non-win variants already partially supported
by `g.dead`/`g.killer` (Dark, Slain). Each ending is 2–4 grounded lines recapping this run's
actual numbers, plus one ledger-aware coda line on any non-first run.

**Save v2 + ledger.** `SAVE_VERSION` bumps to 2 (`save.rs:15`); `parse_save` accepts version ≤
current and `replay()` uses one `apply_input` regardless — v1 saves only ever contain bytes 0–5,
so replaying them under the v2 vocabulary is identical. Separate, tiny **ledger file**
(`rl144.ledger`, one per install, not per world — explicitly NOT serialized world state, just
aggregate counters, same spirit as a highscore file): magic `RLLG`, version `u8`, `runs_total
u32`, `runs_won u32`, `kills_total u32`, `spared_total u32`, `deepest_ever u8`, flags `u8`,
`last_world_hash u64` — 32 bytes. Read at startup, written at run end, feeds ledger-aware
dialogue/ending coda lines. New headless mode `--dump-ledger <file>` prints it for headless
verification.

## 5. Roadmap

Each batch: verification-gate extension + size checkpoint. All batches after the current
baseline (192,144 B minifb / 169,724 B term).

- **Batch 4 — ACT verb + violence tax + regard/spare (monsters only, no NPCs yet).** No new
  worldgen channel draws (regard/spare is pure turn logic) → goldens untouched, MINOR. Gate:
  `--sim --policy pacifist` + `tests/pacifist-band.json` (death-distribution inversion claim,
  above); new `#[test]` for spare-vs-kill `state_hash` determinism. Checkpoint: ≤+5 KB packed
  (target ≈197 KB minifb).
- **Batch 5 — USE-ITEM verb (minimal inventory).** Gate: extend the existing replay-determinism
  test to cover byte 8. Checkpoint: ≤+3 KB (≈200 KB).
- **Batch 6 — NPC entity kind, NPC vaults, dialogue tables, `Screen::Dialogue`.** Worldgen-
  adjacent (new vault channel draws) → **MAJOR, explicit human sign-off required**, `--dump`
  goldens regenerated for seeds 1/2/3/42/1337, `--solve 10000` re-verified. Gate: new `--dump`
  sanity check that NPC glyphs render distinctly and are reachable; dialogue-node graph
  validated (no dangling node ids) by a `#[test]`. Checkpoint: ≤+35 KB (≈235 KB, 16% of budget).
- **Batch 7 — Endings matrix, ledger file, retry-same-seed verb (byte 6).** Gate: `#[test]`
  proving byte 6 replay produces identical `state_hash` to a fresh `Game::new(seed)` (proving it
  isn't secretly rerolling); `--dump-ledger` headless mode. Checkpoint: ≤+5 KB (≈240 KB).
- **Batch 8 (sign-off gated on 4–7 landing AND playtesting as fun) — pixel portraits (§3) +
  minimal audio stingers (§3).** Gate: new frame golden covering `Screen::Dialogue` with a
  portrait; the ~150–250-line synth gets its own size measurement before merge (it's close to
  the "must hand-roll in <150 lines" line and needs to justify going over). Checkpoint: confirm
  total still <25% of budget before landing (target ≈255 KB).
- **Batch 9 (optional, last) — `backend-web` (wasm32 + hand-rolled JS shim), touch input.** Gate:
  wasm32 build compiles clean; flagged explicitly as needing interactive playtest (unverifiable
  headlessly beyond compile, per CLAUDE.md's window-path rule). No size checkpoint against the
  floppy budget (a web build isn't the packed-binary artifact CLAUDE.md gates), but the shipped
  wasm itself should be measured and reported.

## 6. Cuts

- **Full tracker/song system.** Cut for v1.0, redesigned smaller for batch 8 (§3). A silent game
  with real writing beats a game with a mediocre loop and no soul; the old 20–30 KB reservation
  was sized for spectacle I'm not building yet.
- **True pixel-art sprite atlas / animation frames.** Cut permanently at this project's scale.
  Glyphs + theme palettes already carry tone (proven by the existing theme-tinted rendering);
  animated sprites multiply byte cost for zero narrative return. The one pixel-art surface I keep
  (portraits) is designed explicitly *because* it's the highest narrative-return-per-byte pixel
  content available, not as a first domino toward a sprite layer.
- **Grid-driven inventory UI (drag-drop, item combos, equipment slots).** Cut. USE-ITEM satisfies
  the "deliberate use" roadmap item without a UI subsystem eating budget and engineering time.
- **Facing-state / stance-toggle for ACT.** Cut in favor of nearest-adjacent resolution —
  simpler, deterministic, and doesn't grow `state_hash`'s surface for a feature that doesn't need
  directionality to work.
- **Networking / MMORPG win-condition, this proposal's roadmap.** Not scoped into batches 4–9.
  The engine-direction memo's replay-core purity (`state = replay(seed, input_log)`) is fully
  compatible with everything above — nothing here forecloses it — but pursuing it now would
  starve the writing budget of the only scarce resource that actually matters here: engineering
  attention, not bytes.
- **Runtime-generated dialogue (LLM calls, procedural grammar text).** Cut on principle: violates
  the grounding doctrine (can't guarantee "never invent entities") and the "no engines" spirit.
  Every line of dialogue is hand-authored const data, same as `THEMES`.

## 7. Attack surface

The strongest case against this proposal, stated honestly:

- **Screenshots sell roguelikes; prose doesn't screenshot.** Reviewers and wishlist-scrollers
  rate on SVGA pixel art and GIFs in a 5-second scroll. A glyph-forward game, however well
  written, is a harder sell at first glance than a rival's sprite-forward pitch — this proposal
  bets the whole launch on players reading far enough to hit the writing, which is a real risk
  for a floppy-disk novelty project people may only give 30 seconds.
- **I'm stacking a new, untested balance surface on an already-fragile one.** `sim-band.json`'s
  own comment shows the existing gate is tight (win 14.6%, band [10,25]%, 100% of deaths are
  combat). Adding ACT/regard/tax changes that incentive landscape on top of a system batch-3
  already flagged as needing a balance pass — it's plausible both the greedy-bot and
  pacifist-bot gates destabilize together, and chasing two sim-bot bands into a defensible
  correlated state could eat more engineering time than the writing I'm trying to protect.
- **30 KB / 8 NPCs is thin for the "Undertale fusion" claim.** Undertale's power comes from tens
  of hours of character writing across a full cast; 8 NPCs with ~15 nodes each risks reading as
  a tribute gesture rather than an earned fusion — the opposite failure mode from spectacle: all
  mechanics and insufficient narrative meat to justify the pitch.
- **Cutting music entirely for v1.0 is a real tonal risk**, not just a budget footnote — Undertale
  is inseparable from its soundtrack in most players' memory of it, and a silent game explicitly
  pitched as an "Undertale fusion" may get specifically dinged for the absence, independent of
  how good the prose is.
- **Batch 6 is another MAJOR-with-sign-off gate**, the fourth in this project's history
  (`content.rs`'s room-kind/tone and lore-inscription batches were also authorized MAJORs). Each
  one has real process cost (regenerate 5 goldens, re-verify `--solve 10000`, get explicit
  human sign-off) — repeating that pattern risks stalling the roadmap on process overhead rather
  than substance if the NPC-vault reachability proof or dialogue-graph validation takes more than
  one pass to land clean.

## Rebuttal round

**B's most serious flaw.** B is titled around an Undertale fusion but never designs a mercy
mechanic — the word "mercy" appears exactly once in the whole document, as an audio-stinger
trigger: "the events that make an *other* presence legible — ghost sighted, ghost death
replayed, your own retry beginning, **a mercy choice taken**" (§3). Nothing in §4 (their own
"fusion design" section) defines what a mercy choice *is*, what verb triggers it, or what state
it changes. B substitutes "the game remembers you" (ghosts, retry) for "the game watches how you
treat it" (mercy) and calls the result an Undertale fusion. It's a memory/multiplayer proposal
wearing the brief's title, not a moral-choice engine.

**C's most serious flaw.** C cuts mercy explicitly and hands it to a rival: "No new verbs, no
mercy/spare mechanic... it's the natural target for a rival's systems-first thesis — let them
own it" (§6). C's own §7 then names the cost: "Batch 2's finding — 0% bot win rate, 100% combat
deaths, combat lethality... is the wall. Nothing here fixes it; I'm arguing it doesn't need
fixing to ship a good v1.0, which is a defensible but real gamble against the repo's own most
recent finding." That's honest, but it means C spends ~33 KB and five batches making the wall
look better lit without moving it.

**What I'd steal from B.** The auto-ghost-on-death mechanism — RETRY (byte 6) "auto-saves the
just-ended attempt's log as a ghost before resetting" (§4) — is a better memory primitive than
my flat 32-byte ledger counters. I'll fold it in *without networking*: on death, trim-and-stash
the just-ended input log as a local "last-death ghost," and let ledger-aware dialogue/coda lines
quote or reference it directly (a specific spared-then-slain monster, a specific depth) instead
of just a number. Same file format, zero multiplayer dependency, strictly more vivid "the game
remembers" payoff for my endings matrix.

**What I'd steal from C.** The `scene()`/`SceneEntity` derived-field pattern (§3: `facing`,
`anim_phase` computed pure-function, never persisted, never hashed) is a cleaner answer than my
"no facing state" dodge in §4 — it proves facing can be free. I'll adopt derived-only hit-squash
and palette-flash screen-feel (no RNG channel touched, no new save/hash surface) as free
legibility for my violence-tax and ACT/spare moments specifically — a flash color-coded to
tax-paid vs. spared makes the mercy-light economy readable at a glance, for the cost of a pure
function over existing `scale()`.

**Revised final stance.** I concede C's screen-feel is worth taking for near-zero cost, and B's
ghost-of-your-last-death is a better memory primitive than raw counters — both absorbed above
without diluting the thesis. I do not concede the center: neither rival's proposal, by its own
text, delivers the mercy mechanic the brief asks for — B gestures at it once and never designs
it, C deliberately declines it and bets presentation alone repairs the repo's own named combat-
lethality finding. Mine is still the only proposal where "violence burns light" is one line of
code and a gated sim-bot proves the tradeoff is real, not narrated. That's the fusion; I'm
keeping it and enriching its legibility with the two steals above.
