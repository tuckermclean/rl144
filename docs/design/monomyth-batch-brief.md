# [DEFERRED 2026-07-19 — human call: a *next*, not a now. Do not execute until re-prioritized.]
# Batch brief — monomyth instrumentation (lens + --journey + funnel)

*Standalone first batch of the THOUSAND FACES spine (`monomyth.md` §5 items 1–2, sequencing
rule applied). Deliberately writing-free: no register columns, no new flavor lines, no IP-5
mechanic. Instrument first; the funnel data gates the whiplash writing that follows. Slots
cleanly BEFORE Phase 2 (mercy) of DECISION.md — in fact it serves it: the per-beat death
funnel is exactly the diagnostic the pacifist-band tuning will want.*

## Scope

**T1 — `src/monomyth.rs` (core lens).**
- `enum Beat { Threshold, Trials, Ordeal, RoadBack, ReturnThreshold, Elixir }` and
  `enum Register { Comic, Sincere, Plain }`, `pub(crate) fn beat(&Game) -> Beat`,
  `pub(crate) fn register(Beat) -> Register`, exactly as specified in `monomyth.md` §3 —
  including the settled Threshold-duration predicate (`depth == 1 && saved[1].is_none() &&
  !has_amulet`), with its rationale comment. Coarse inputs only (position/boon/terminal/saved
  slot) — nothing else, per §0.2.
- Fully inside the presentation-only exclusion doctrine: no stored state, never hashed/dumped/
  saved; zero platform calls, zero cfg. NO call site changes register selection yet — in this
  batch the lens has exactly two consumers: `--journey` and tests. No player-visible change of
  any kind; both backends untouched.
- Unit tests: beat() total and deterministic; Threshold holds across the whole first floor and
  flips to Trials on first descent (drive a real Game via apply_input); RoadBack/Return/Elixir
  transitions on a scripted winning run; register() mapping table.

**T2 — `--journey` + goldens + funnel.**
- `headless.rs`: `journey_main(path)` — parse a `.sav` (v1 or v2) or `.ghost` (RLG1), replay
  via the existing `replay()`, sample `beat()` per input, print the transition trace one line:
  `threshold:0 trials:41 ordeal:1141 road-back:1181 return-threshold:1744 elixir:1745`
  (beat name at the input index where it first became current; bytes 5/6 reset the trace with a
  `| rebirth:` / `| reroll:` separator). `--journey` flag wired in main.rs (all backends —
  headless mode, no feature gating).
- Golden traces: generate two fixture saves deterministically in-repo (script them via a test
  or a committed fixture like tests/fixtures/ref.sav — one losing run, one winning run for a
  known seed; a winning input log can be produced by recording the sim bot on a seed it wins —
  seeds where it wins are known from --sim, e.g. find one under 100) → commit
  `tests/golden/journey_win.txt` + `journey_loss.txt`, cmp'd by a `journeys` Makefile target in
  `make check` (temp-dir discipline, style-matched to `goldens`/`frames`).
- `#[test] beat_monotone_on_wins`: over any winning replay, beats advance in canonical order
  (skips legal; regression only across a byte-5/6 reset). Drive it with the recorded winning
  log.
- Funnel: `sim_main` counts, per run, the LAST beat reached (which beat the run died/won in)
  and prints them in the JSON (`"died_in":{"threshold":n,"trials":n,"ordeal":n,"road_back":n,
  "return_threshold":n},"elixir":n`). Gate: `tests/journey-band.json` — after measuring, band
  the two structurally-critical numbers only (elixir == wins consistency is a hard assert, and
  road_back+return_threshold deaths ≥ 1 per 5000 — the climb must claim someone, or the Road
  Back is fiction). Do NOT band every beat on first landing; measure, commit the observed
  distribution in the band file's comment, band the minimum.

**T3 — docs.** CLAUDE.md: the lens joins the exclusion-doctrine list; `--journey` joins the
headless modes; `journeys` + funnel join the make check description; status entry with the
measured funnel distribution quoted (this number is the batch's real deliverable — it decides
the whiplash writing that comes next).

## Gates & constraints

Full `make check` green after each task (now incl. `journeys`); dump/frame goldens byte-
identical (nothing here may touch rendering or gameplay — if --sim's win/death numbers move
AT ALL, the lens leaked: stop); sizes reported (budget: ≤ +4 KB packed total); rustc 1.75;
zero new deps; subagent-driven per house process (implementer → reviewer → fix rounds, final
whole-branch review before merge).

## Explicitly out of scope (next batches, gated on this one's funnel data)

Register-column whiplash writing (IP-2) and Call/Ordeal lines (IP-1/3); any register()
call-site wiring; leitmotif-by-beat (rides C's audio batch); IP-5 rescue + martyr policy
(Phase 4); Propp fn_tag (optional MAJOR).
