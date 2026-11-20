# M03 — bands

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | S02 | none | S |

**Owns:** `tests/*-band.json`, the solver and bot-simulator band files the gate checks measurements against.

## Purpose
The five band files hold the ranges the headless gates accept. Each is a small JSON object read without a JSON
crate (`headless::band_range` reads `"key": [lo, hi]`, `band_scalar` reads `"key": N`), with a long `comment`
string that records each re-baseline's measurements and sign-off.
- `tests/solver-band.json`: `--solve`'s walk-budget statistics over 10,000 seeds, `min` [500, 800], `p50`
  [850, 1100], `p90` [1000, 1250], `p99` [1150, 1450], `max` [1300, 1700].
- `tests/sim-band.json`: the greedy bot over 5,000 seeds, `win_pct` [0, 4] and `deaths_dark` [1, 25].
- `tests/pacifist-band.json`: the pacifist bot, `win_pct` [0, 4].
- `tests/tactical-band.json`: the tactical (violent) bot, `win_pct` [27, 42] and `deaths_dark` [200, 550].
- `tests/tactical-pacifist-band.json`: the tactical diplomat, `win_pct` [30, 50]; and the flip gate's
  `flip_margin` 3 (the diplomat's win percentage must exceed the violent bot's by at least this) and `flip_n`
  5000 (the seed count the margin was calibrated at).
The greedy and pacifist bands are the floor-of-competence references; the tactical pair are the
competent-player instruments, and the 30 floor of the diplomat's band is thesis-bearing (its comment, and the
mercy arc's ratification).

## Tests
The `Makefile` targets that read these files from the repository root through the release binary: `solve`
(`--solve 10000`, `tests/solver-band.json`), `sim` (`--sim 5000` for each of the four policies, each against its
own file) and `flip` (`--sim-flip 5000`, `tests/tactical-pacifist-band.json`'s `flip_margin`). No unit test
reads these files.

## Changes
none: imported

## Gate
- **M03-AC-1** — Over seeds 0 to 9,999, each of the solver's `min`, `p50`, `p90`, `p99` and `max` lies inside its range in `tests/solver-band.json`. *Verify:* `make solve` (target of `Makefile`).
- **M03-AC-2** — Over seeds 0 to 4,999, the greedy bot's `win_pct` and `deaths_dark` lie inside their ranges in `tests/sim-band.json`, and the pacifist bot's `win_pct` inside its range in `tests/pacifist-band.json`. *Verify:* `make sim` (target of `Makefile`).
- **M03-AC-3** — Over seeds 0 to 4,999, the tactical bot's `win_pct` and `deaths_dark` lie inside their ranges in `tests/tactical-band.json`, and the tactical diplomat's `win_pct` inside its range in `tests/tactical-pacifist-band.json`. *Verify:* `make sim` (target of `Makefile`).
- **M03-AC-4** — Over seeds 0 to 4,999, the tactical diplomat's win percentage exceeds the tactical bot's by at least `flip_margin` from `tests/tactical-pacifist-band.json`. *Verify:* `make flip` (target of `Makefile`).
- **M03-AC-5** — The files hold the keys and ranges listed in Purpose, and `flip_n` equals the `Makefile`'s `SIM_SEEDS` (5000). *Verify:* unpinned (a missing or misspelt `[lo, hi]` key skips its check, a missing `flip_margin` falls back to 3, and `flip_n` only prints a note on a mismatch).

## Risks
- A band check runs only when its key is found: `band_range` returns `None` for a missing or renamed key and
  the gate then skips that check and passes (S02's Risks); a missing `flip_margin` falls back to 3
  (`band_scalar(...).unwrap_or(3)` in `sim_flip_main`), and a missing `flip_n` only drops the note. A missing per-policy band file only warns; a missing
  `tests/tactical-pacifist-band.json` fails `make flip`.
- Widening a range always passes the gate, so these files can only catch a narrowing or a measurement drift;
  `CLAUDE.md` makes every re-baseline an owner sign-off, the greedy and pacifist bands must not move when other
  bands change, and the diplomat's 30 floor needs its own sign-off to lower. `SPEC.md`'s open decisions let an
  implementer re-baseline a band only when the node's spec names it, with the flip sign-off line.
- The paths are relative to the working directory, so the gates must run from the repository root.
- The `comment` strings are the measurement history and grow with each re-baseline; they are prose for people,
  never read by the code, and go stale: the solver band's comment quotes the 2026-07-18 stats (min 636, p50
  954, p90 1110, p99 1272, max 1503), while a run of `--solve 10000 --report` on 2026-11-19 (rustc 1.75, release build) gave
  537, 885, 1044, 1215 and 1476.
- `make sim` checks each policy against the band file of the policy it ran, so it cannot tell a policy mix-up
  on the command line (M01's Risks).
