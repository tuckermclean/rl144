# Proposal synthesis — rl144 endgame (2026-07-18)

Three competing end-state proposals were drafted against the shared brief (`0-brief.md`), then
each rebutted the other two. Full texts: `A-authored-dungeon.md`, `B-haunted-floppy.md`,
`C-impossible-object.md` (each ends with its rebuttal round). This document is the controller's
adjudication: what the fight settled, what it left genuinely open, and a recommended blend.

## Settled by unanimity (adopt regardless of which thesis wins)

All three proposals independently converged on these — they are effectively decided:

1. **Mobile = `backend-wasm` via hand-rolled `extern "C"` + a tiny JS host (~50–120 lines),
   no wasm-bindgen, no new crate.** Touch maps to the existing input-byte vocabulary. The JS
   host is platform loader, not asset. (A §3, B §3, C §3 — identical conclusion, reached
   separately.)
2. **The 80×30 grid is never touched for portrait.** Letterbox (A), stacked recomposition of
   the same cell rows (B), or player-centered 40-column viewport crop (C) — all backend-side.
   C's viewport crop is the strongest of the three answers; nothing about it forecloses the
   others.
3. **One scene-layer core surface, C's design:** `scene(&Game) -> Vec<SceneEntity>` with
   derived-only fields (facing, anim_phase — pure functions, never hashed, never saved,
   invisible to `--dump`/goldens). B adopted it wholesale for ghosts; A adopted the
   derived-field pattern for screen-feel. Build it once, in core, before any consumer.
4. **Save v2 with a retry-same-seed input byte (6), auto-capturing the ended attempt's log.**
   A wants it for the memory ledger, B for ghosts, C (shrunk) for a death-tile echo. Same
   primitive. v1 logs (bytes 0–5 only) must replay byte-identical — every proposal keeps this.
5. **The violence tax** (`light -= 1` extra in the attack branch): proposed by A, adopted by B,
   unopposed by C (C attacked ACT/regard, not the tax). One line; makes darkness bite for
   violent play; answers the batch-3 dark-death finding. Requires re-running the sim gate and
   likely re-baselining `tests/sim-band.json` (signed-off balance change).
6. **Audio is stingers-first, tracker-maybe, samples-never.** All three killed the old 20–30KB
   ambient-tracker reservation. C's leitmotif-as-relative-pitch-deltas (one melody, transposed
   per theme/register) is the single most Undertale-shaped audio idea on the table and costs
   ~600B of pattern data over the stinger synth.

## The real disagreement: which risky centerpiece goes first

- **A** bets on a mercy system (ACT verb, regard, pacifist sim-bot gate) + ~30KB of authored
  NPC dialogue. Its own attack surface concedes: two correlated balance gates, thin cast, no
  screenshots.
- **B** bets on the replay substrate (ghosts as parked `Game` instances, corpses as render
  overlays with zero worldgen MAJOR, ghost files ≈ 2KB, live peers = ghosts with a socket).
  Its own attack surface concedes: without population it degrades to your own corpses + the
  sim-bot's, and "MMORPG" honestly means same-seed cohorts.
- **C** bets on presentation (hybrid sprite pipeline: ~10.5KB authored 4bpp geometry + per-theme
  palette ramps + dither/outline passes; synth; screen-feel) at a measured ~33KB packed. Its own
  attack surface concedes: headless CI can't see any of it, and hand-rolled ALSA is
  Linux-only until proven elsewhere.

## Verified kills from the rebuttal round

- **C on A (the round's sharpest point):** `monsters_act()` runs every turn regardless of verb,
  so 3–4 ACT turns adjacent to an ogre are 3–4 free enemy swings — as designed, pacifism may
  INCREASE combat exposure, inverting A's predicted pacifist death distribution. A's own
  pacifist-band gate would catch this, but the mercy design needs rework before it can pass:
  a landed ACT must stay (or redirect) the target's swing that turn, or mercy is a trap.
- **A on B:** B never designs a mercy mechanic — "mercy" appears once, as a stinger name. B is
  a memory/multiplayer proposal wearing the fusion title. Fair hit; B did not contest it.
- **C on B:** `GhostState{source: File | Socket}` elides lockstep's actual hard problem — the
  turn-barrier/ready-check for live peers. B's loopback test proves convergence given ordered
  input, not ordering. Live relay is later-roadmap and feature-flagged, so this wounds B10,
  not B's async core.
- **Controller on C (pre-rebuttal):** C's original gamble rested on the batch-2 "0% win rate"
  finding, already fixed in batch 3 — corrected in C's final text; its residual point (no
  sim-regression check specified for presentation-only changes) is valid and cheap to fix.

## Recommended blend (controller's pick, for human decision — nothing here is signed off)

Sequence by risk-adjusted return, stealing shamelessly:

- **Phase 1 — the unanimous substrate (cheap, low-risk, ~10KB):** save v2 retry byte +
  auto-ghost-on-death; violence tax + sim re-gate; `scene()` core surface; screen-feel
  (hit-squash, palette flash); last-death echo/ghost rendered via `scale()` dimming. Delivers
  "the game remembers this world" and gives the torch teeth, in one batch.
- **Phase 2 — the fusion's load-bearing wall (the bet, ~15KB):** A's mercy system, redesigned
  to answer C's free-swing attack (a landed ACT stays the target's attack that turn), gated by
  the pacifist sim-bot band exactly as A specs. The brief says Undertale fusion; B and C both
  failed to deliver mercy by their own text. If the pacifist band can't be landed after honest
  tuning, mercy gets cut and B's ghost-forward fusion becomes the identity — decide by gate,
  not by taste.
- **Phase 3 — staging (C's craft aimed at Phase 2's meaning, ~35KB):** sprite pipeline, NPC
  dialogue + vault cast (A's batch-6 MAJOR), leitmotif stinger audio. Presentation arrives
  pointed at choices that exist.
- **Phase 4 — the engine win-condition (B's ladder):** ghost exchange polish, daily-seed
  cohorts, `backend-wasm` mobile, and last, the `net`-feature relay with a real turn-barrier
  design. Population needs a game worth haunting; this goes last on purpose.

Projected total: ~290–330KB packed — under 23% of budget with the entire fusion, art, audio,
mobile, and networking aboard.

Open items requiring explicit human sign-off before any phase starts: violence-tax balance
re-baseline (Phase 1), input vocabulary v2 (Phase 1), mercy system + pacifist band (Phase 2),
NPC-vault worldgen MAJOR (Phase 3), `scene()` core-surface addition (Phase 1).
