# D04 — monomyth

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | none | none | S |

**Owns:** `docs/design/monomyth*.md`, the deferred "THOUSAND FACES" monomyth design, its narratologist notes and its first batch brief.

## Purpose
A design that `CLAUDE.md` lists as deferred and explicitly post-1.0 (not to be built without re-prioritization):
- `monomyth.md` (2026-07-18): the claim that the round-trip win is already a monomyth, the six-beat spine
  (Threshold, Trials, Ordeal, Road Back, Return Threshold, Elixir), authored-content insertion points, a
  construction sketch of a derived, presentation-only `beat(&Game)` lens (never hashed, dumped or saved), the
  inventory of reuse, the mission list and the risks.
- `monomyth-notes.md`: the narratologist review whose six-beat cut and warning the design adopts.
- `monomyth-batch-brief.md`: the brief of the first batch (a `src/monomyth.rs` lens, a `--journey` mode, a
  per-beat death funnel), headed `[DEFERRED 2026-07-19 — human call: a *next*, not a now. Do not execute until
  re-prioritized.]`
Nothing of it is built: there is no `src/monomyth.rs` and no `--journey` mode.

## Tests
None: no tracked test reads these files.

## Changes
none: imported

## Gate
- **D04-AC-1** — `monomyth-batch-brief.md` opens with the deferral line above, and `monomyth.md` keeps the lens derived-only (in the presentation-only exclusion set, never hashed, dumped or saved). *Verify:* unpinned (prose).

## Risks
- The brief predates batches 5 to 17: it names `src/monomyth.rs` as a new module (`CLAUDE.md` asks that the module
  count not grow without justification) and assumes the code of 2026-07-19; a re-prioritized batch rewrites it
  against today's code.
- The files contain non-ASCII characters; they are not rendered.
