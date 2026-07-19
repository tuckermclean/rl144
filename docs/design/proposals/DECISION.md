# Decision record — endgame direction (2026-07-18)

Human review of `A-authored-dungeon.md`, `B-haunted-floppy.md`, `C-impossible-object.md`, and
`SYNTHESIS.md`: **approved as a whole, with the explicit directive that Proposal C is adopted in
full** — no line-item of C's ledger may be trimmed by the blend. Verbatim intent: "It all looks
good. Make sure C gets everything he wants."

## What this settles

- The blended roadmap in SYNTHESIS.md is adopted (Phase 1 substrate → Phase 2 mercy-by-gate →
  Phase 3 presentation → Phase 4 ghosts/mobile/net).
- **C-complete guarantee**: every item in C's proposal ships — hybrid sprite pipeline (authored
  4bpp base geometry + palette ramps + Bayer dither + outline pass), `scene()`/`SceneEntity`
  core surface, animation (walk bob/hit squash/idle breathe) and deterministic particles/
  screen-feel, full audio stack (synth core + ALSA shim + tracker pattern tables + the
  leitmotif-as-pitch-deltas transposition system — not just stingers), `backend-wasm` mobile
  with the player-centered portrait viewport crop and touch→byte mapping, C's `--dump-audio`
  headless gate, and C's batch-9 richness pass, which is hereby **committed** (still
  budget-gated by the floppy ceiling, no longer "optional").
- Where C's cuts conflict with A/B items the synthesis kept (mercy system, NPC vaults, ghost
  substrate), the synthesis governs: those still ship in their phases. C's cuts bound C's
  *own* scope, not the project's.

## Sign-offs granted by this decision

1. Violence tax (+1 light per attack) and the resulting `tests/sim-band.json` re-baseline.
2. Input vocabulary v2 (retry-same-seed byte 6; later ACT/USE bytes 7-8 in Phase 2) with
   `SAVE_VERSION` bump; v1 logs must replay byte-identical.
3. Mercy system + pacifist sim-bot band (Phase 2), contingent on the gate landing — if the
   pacifist band cannot be landed after honest tuning, mercy is cut and the ghost-forward
   identity governs (decide by gate, not taste).
4. NPC-vault worldgen MAJOR (Phase 3) — goldens regenerate under the standing re-baseline flow.
5. `scene()` core-surface addition (Phase 1) per C's spec: derived-only fields, never hashed,
   never saved, invisible to `--dump`.

## Standing constraints unchanged

Floppy budget per target, no asset files, dependency freeze (hand-rolled FFI/JS-host paths as
proposed — no wasm-bindgen, no cpal), dump-golden doctrine, headless-first gates, MSRV 1.75.
