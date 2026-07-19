# THOUSAND FACES — the monomyth as a derived structure on the rl144 engine

*Design + construction sketch, 2026-07-18. Stage theory pressure-tested by a narratologist
review (committed as `monomyth-notes.md`); its six-beat cut and its warning are adopted
wholesale. Named for Campbell's thesis: every hero's journey is one journey wearing a thousand
faces. A deterministic engine makes that literal — every run is one function,
`replay(seed, input_log)`, wearing a thousand seeds. The daily seed is the face everyone wears
today; the ghosts are the faces that came before yours.*

## 0. The claim

rl144's win condition is already a monomyth: **descend through trials, seize the boon at the
bottom, return transformed at double cost.** The amulet's 2×-light burn IS "the Road Back is
harder than the Road of Trials." Retry-same-seed IS the eternal return — the same cosmos
regenerated from the same hash. Ghosts ARE the thousand faces. This document names the
structure the engine already computes, marks every **insertion point** where authored content
plugs in, and lists what's missing.

**Two governing principles, both architectural:**

1. **Derived, not stored.** The monomyth layer is a pure function `stage(&Game) -> Beat` in
   the presentation-only exclusion set (like `scene()`/`killer`/`echo`/`fx_hit`): never hashed,
   dumped, or saved. The game never *puts* you in a stage; it *recognizes* the one you're in
   from facts it already tracks.
2. **Off-UI, and conservative.** (The narratologist's warning.) The lens is expressed ONLY
   through consequence — lore register, music transposition, a light delta — never a label, a
   tracker, or stage names in any player-facing surface. And where inference is weak it stays
   silent (Plain register): a mislabeled beat is worse than an unnamed one, because *unearned
   coherence spends trust, not just pacing*. Legible inference gets farmed ("spam mercy for
   the Atonement lore") — so the mapping stays coarse (position + boon + rebirth), never
   stat-pattern cleverness.

## 1. The spine — six beats (the narratologist's cut)

Campbell's 17 and Vogler's 12 assume scene budget a 30–60-minute permadeath run doesn't have.
Six beats, each keyed to something the engine already does. Relationship-arc stages (Goddess,
Temptress, Atonement) are **cut entirely, not collapsed** — they need persistent NPC
interiority, the wrong shape for a seed-scoped cast.

| # | Beat | Campbell folded in | Engine trigger (pure predicate) | Carried by |
|---|------|--------------------|--------------------------------|-----------|
| 1 | Threshold | First Threshold + Belly of the Whale | first arrival, depth 1 | the `<` entrance; descending IS the swallowing |
| 2 | Road of Trials | Trials; Donor-function echoes | `1 <= depth <= 4 && !has_amulet` | ordinary play: combat/mercy economy, light spend, lore tiers |
| 3 | Ordeal + Boon (fused) | Ordeal, Apotheosis, Ultimate Boon | `depth == MAX_DEPTH`, then amulet pickup | pickup-on-walk-over fuses transformation and theft-of-the-boon into one input — deliberately not separated |
| 4 | Road Back | Magic Flight | `has_amulet && depth > 1` | the 2× burn. Don't narrate it; the doubled number IS the beat |
| 5 | Return Threshold | Return Threshold + Resurrection | `has_amulet && depth == 1` | **the emotional climax — not the pickup.** The self that exits is mechanically thinner than the one that entered |
| 6 | Elixir, socialized | Master of Two Worlds, Freedom to Live | `won` | the win feeds the ghost system: freedom-to-live is what the NEXT face gets from watching you |

**Pre- and trans-run lenses** (not beats; they live across attempts):
- **The Call** — the title screen (exists: theme label, lore line, the seed as the world's
  name). "Any key" is answering.
- **Refusal of the Call** — pre-threshold only. The retry prompt after death surfaces what the
  engine already knows ("you have died here before" — echo/ghost data); the *player's own
  dwell-time at that prompt* is the refusal. Diegetic to behavior; zero authored hesitation.
- **The Cosmic Round** — death/retry is neither Refusal nor Belly (both mistimed): it is
  Eliade's eternal return. Byte 6 = the same cosmos regenerated (sacred repetition); byte 5 =
  a genuinely new cosmogony. The engine's own retry/reroll verb split *is* this distinction,
  already built. Ghosts therefore read as **ancestor-spirits of a world-age**, and the
  bot-ghost as the hero the labyrinth had already claimed before you ever arrived — present in
  every cosmos, stronger myth than "your save file."

## 2. Insertion points (authored-content sockets, well-noted)

Same pattern as `THEMES`/`VAULTS`/lore tiers: const tables the generator librarians over.

- **IP-1 The Call** — `Theme` gains `call: [&str; 2]`: one line on the title screen, one at
  first sight of the `>`. Slot-filled, grounded. *Verify:* length tests. Log-only; no goldens.
- **IP-2 Trials registers** — the register shift bound to the spine instead of the map:
  `TONE_LINES` and lore templates gain a **second column** (sincere voicing, same `{K}`/`{A}`
  slots). Beat 2 uses column 0 (comic); beats 4–5 re-voice *revisited* rooms through column 1 —
  the room you joked about on the way down answers gravely on the way back up, boon in hand.
  This is the Peasant's-Quest→Undertale whiplash, mechanized. *Verify:* length tests × both
  columns; goldens untouched (log-only).
- **IP-3 Ordeal framing** — `Theme.ordeal: [&str; 2]`: depth-5 arrival, amulet-room approach.
- **IP-4 The mentor's corpse** — the bot-ghost's death site + one const note per outcome
  ("here lies one who walked only in straight lines"). Derived data; no channel draws.
- **IP-5 Rescue from Without** *(the narratologist's re-target — Phase 4)* — aid arriving after
  your own strength runs out, so: **return leg only, never the descent.** A same-seed ghost's
  death tile (including *abandoned* ghosts — an unresolved dead, not a "refusal" gimmick)
  yields a one-time small light stipend or a revealed safe tile on the climb. Grounded: the
  engine can prove a run ended there. *Balance-gated: enters solver/sim models; sign-off.*
- **IP-6 The Judge** — Return Threshold reads the log: kills, spared, lore read, retries vs
  rerolls, ghosts witnessed, light left. Every line an engine fact (adopted from Proposal A).
- **IP-7 Leitmotif spine** — C's pitch-delta melody keyed by *beat*, not depth: plain at the
  Call; staccato-comic through Trials; minor and half-speed at the Ordeal; transposed up and
  urgent on the Road Back; resolved at the Elixir. One melody, five renderings — the monomyth
  audible, never labeled. *Verify:* `--dump-audio` golden per beat.
- **IP-8 Propp grammar layer** (generation-side; see §5 of the notes) — Campbell for the felt
  arc, **Propp for the generator**: a data-only `fn_tag` column on spawn/item/vault tables
  naming which Proppian function an entity can discharge (donor, lack, liquidation-of-lack,
  helper, false-hero…). Worldgen assigns functions per seed; the same function is a lore
  inscription in one theme and a vault in another without touching structure. First shipment is
  tags + one rule (every depth places a Lack whose Liquidation is deeper) — a constraint the
  generator consumes, not a dramatized beat.

## 3. Construction (pseudocode against the real crate)

```rust
// src/monomyth.rs — CORE module: zero platform calls, zero cfg, zero stored state.
// Entirely inside the presentation-only exclusion doctrine (see state_hash docs).

pub(crate) enum Beat { Threshold, Trials, Ordeal, RoadBack, ReturnThreshold, Elixir }
pub(crate) enum Register { Comic, Sincere, Plain }

/// Derived, total, COARSE on purpose (see §0.2): position + boon + terminal
/// state only. No stat-pattern inference, ever — that way lies farming and
/// unearned coherence.
pub(crate) fn beat(g: &Game) -> Beat {
    if g.won                          { return Beat::Elixir; }
    if g.has_amulet && g.depth == 1   { return Beat::ReturnThreshold; }
    if g.has_amulet                   { return Beat::RoadBack; }
    if g.depth == MAX_DEPTH           { return Beat::Ordeal; }
    if g.turns == 0                   { return Beat::Threshold; }
    Beat::Trials
}

pub(crate) fn register(b: Beat) -> Register {
    match b {
        Beat::Trials | Beat::Threshold          => Register::Comic,
        Beat::RoadBack | Beat::ReturnThreshold  => Register::Sincere,
        _                                       => Register::Plain, // silent when unsure
    }
}

// Call sites: flavor lookups (note_room_entry, lore_line, tier warnings) take
// register(beat(g)) to pick the column; the audio sequencer takes beat(g) to
// pick the leitmotif transposition. NO other surface sees the beat. No UI
// label, no tracker, no journal. render_cells signature unchanged.
```

```rust
// headless.rs — the spine becomes INSTRUMENTED: the monomyth as a funnel.

/// --journey <save|ghost>: replay, sampling beat() per turn, print transitions:
///   "threshold:0 trials:1 ordeal:1141 boon:1180 road-back:1181
///    return-threshold:1744 elixir:1745"
pub(crate) fn journey_main(path: &str) { /* replay() + beat() diff per turn */ }

// Gates, extending standing machinery:
// - tests/golden/journey_seed_*.txt — golden beat traces for fixture runs.
// - #[test] beat_monotone_on_wins: over any winning replay, beats advance in
//   canonical order (skips legal; regression only via a byte-6 world reset).
//   This is the tripwire for a broken lens — it names the exact turn.
// - sim_main grows per-beat death counts (where does the myth eat greedy
//   runs: Trials vs Ordeal vs Road Back?) → tests/journey-band.json. Balance
//   stops being one win-rate number and becomes a funnel with named beats.
```

```rust
// Register plumbing (content.rs) — the only content-schema change:
pub(crate) const TONE_LINES: [[[&str; 2]; 2]; 4]; // [tone][register][variant]
// Theme.lore gains a sincere column the same way. Slots and channels are
// UNTOUCHED: theme_pick draws exactly as today, so worldgen goldens hold.
// Room re-voicing rule: note_room_entry logs column 1 iff register()==Sincere
// AND room_visited was already true this level — "the room answers back."
```

```rust
// Rescue from Without (Phase 4, IP-5) — sketch, balance-gated:
// on entering a tile where a loaded same-seed ghost ended (parse_ghost →
// final position via replay), if beat(g) == RoadBack && !g.rescued_here:
//   g.light += RESCUE_STIPEND; log("{their} torch, still warm."); // grounded
// rescued_here: per-run presentation-adjacent set — but light IS hashed
// state, so unlike the lens this is a REAL mechanic: sim/solve must model
// it, and it ships only behind its own band check.
```

## 4. What exists already (inventory of reuse)

Every load-bearing piece: the round-trip win and 2× boon burn (beats 3–5 mechanically true
today); light-as-clock; retry/reroll verb split (the cosmic round, verbatim); echo + RLG1
ghosts incl. the abandoned outcome (ancestor-spirits, restless dead); the deterministic
bot-run per seed (the always-already-claimed hero); themes/slots/register precedent and the
grounding doctrine; title screen as the Call; the endings matrix direction (the Judge);
`scene()`; the exclusion doctrine; replay/xhash; and the gate apparatus that turns "does the
myth hold?" into CI. Notably absent from the needs list after the narratologist pass: **no
village level, no new depth, no win-predicate move, no save v3** — the six-beat cut needs
none of them.

## 5. What we'd need (the mission list)

1. **`monomyth.rs`** — beat/register lens + monotone test. Small, zero-risk, no sign-off. ~1 KB.
2. **`--journey` + journey goldens + funnel bands in sim.** ~2 KB. The verification spine.
3. **Second register column** (IP-2) + Call/Ordeal lines (IP-1/3) + corpse notes (IP-4) —
   writing, ~10–15 KB, grounding doctrine binds all of it. The whiplash table (IP-2) is where
   the design lives or dies: **prototype it first.**
4. **Phase-2 mercy** (already decided) — the Trials need it to mean anything.
5. **Phase-4 ghost playback** (mentor watching, IP-5 rescue — the one new *mechanic*, gated).
6. **Leitmotif-by-beat** — retargets C's committed audio system; ~0 KB new.
7. **Propp `fn_tag` column + the Lack/Liquidation placement rule** (IP-8) — data + one worldgen
   constraint; the constraint is a **worldgen MAJOR** when it lands. Optional for v1 of this.

Rough ledger: **~18–20 KB packed** on top of the decided roadmap. One gated mechanic (IP-5),
one optional MAJOR (IP-8), zero new dependencies, zero new backends, zero format bumps.

## 6. Risks, named

- **Unearned coherence** (the narratologist's warning, now principle §0.2): the lens stays
  coarse and off-UI; the monotone test plus journey goldens catch misfires mechanically, but
  only writing discipline catches a line that *claims* more than the run earned. Review IP-2
  copy against the grounding doctrine like all flavor.
- **Farming**: inference inputs are position/boon/terminal only — nothing a player can grind.
  Keep it that way; every future "wouldn't it be clever if the lens noticed X" is how the
  checklist rebuilds itself one level removed.
- **Register whiplash misfiring**: a comic column-0 line surfacing during the Road Back would
  puncture the climb. The register() gate is centralized precisely so there is one switch to
  audit; test it per-beat.
