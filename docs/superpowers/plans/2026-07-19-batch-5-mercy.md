# Batch 5 — Phase 2: mercy as talk (the Henson ruling)

*Sign-off: DECISION.md item 3 (mercy + pacifist band, cut-if-band-fails). Design ruling
2026-07-19 (human): mercy is a VERB and the verb is TALK — a monster that answers in its
theme's voice is the face that makes sparing mean something ("ask Jim Henson: if you could
talk to a rat, you could give a rat mercy"). Input-model ruling 2026-07-19: bump stays attack;
ACT is direction-qualified bytes; menus are crust-optional chrome, never engine state.*

## Design (decided)

- **ACT = talk, directional.** Input bytes 7/8/9/10 = ACT-N/S/E... byte order MUST mirror move
  bytes: 7=N, 8=S, 9=W, 10=E. Frontends: `t` (or `a`) then a direction key → one byte.
  apply_input handles 7-10; bytes 5/6 stay frontend-layer. ACT at a wall/empty tile = no-op,
  no turn (like a wall bump). ACT at a monster: costs a normal turn (spend_turn, no tax).
- **Regard.** `Monster.regard: u8` counts ACTs received. Thresholds per kind (tune against the
  gate; start rat 2, goblin 3, ogre 4). At threshold: monster becomes **becalmed**
  (`Monster.calm: bool`), `g.spared += 1` (new hashed field, like kills).
- **The Henson lines.** Each ACT gets an answer: const table in content.rs,
  `TALK_LINES[kind 0-2][stage 0-2]` templates with the theme's mob name / `{A}` adj slots —
  stage = first-ACT / mid / becalmed. 4 themes × 3 kinds × 3 stages ≈ 36 lines, ≤78 chars
  (length test across all fillings). Grounding doctrine binds: a monster may voice want/fear
  about things the engine proves (the dark, your torch, the amulet, its home, its dead) —
  never invented events. Line variety via flavor_rng (per-run stream, replay-safe).
- **The stayed swing (fixes C's free-swing kill of Proposal A).** A monster that received an
  ACT this turn does not attack this turn (it is listening); it may still move. Other monsters
  act normally — crowds stay dangerous. Implement as a transient per-turn mark (index or flag
  cleared in monsters_act's epilogue) — NOT persistent state.
- **Becalmed behavior.** Never attacks, never chases (skipped in monsters_act targeting;
  wanders or stands — pick simplest deterministic option: stands). Bumping a becalmed monster
  SWAPS positions (costs a turn, no tax, no damage — they yield). Still visible in dump as its
  glyph (dump unchanged → goldens hold). Renders calmer: fg tinted via a PAL_CALM treatment.
  ACTing a becalmed monster: one flavor line, no turn cost change, regard capped.
- **No light refund, this batch.** The economy is: violence = tax + damage risk; talk = turns
  (light) + stayed-target-but-live-crowd risk. Refund is a reserve knob if the pacifist band
  can't land without it — use only if needed, document if used.
- **Save format v3.** Vocabulary 0-10. Bump SAVE_VERSION to 3; parse_save accepts 1..=3 (old
  logs contain no 7-10 bytes, replay identically — test). Rationale: an old binary must
  REJECT a v3 save cleanly rather than silently ignoring ACT bytes and diverging.
- **state_hash grows** `spared`, and per-monster `regard`+`calm` bytes (MAJOR — this batch is
  signed off for it). killer/echo/facing/fx_hit exclusion set unchanged.

## Tasks

**T1 — engine core** (game.rs/content.rs/save.rs): everything above except frontends/bot.
Tests: ACT determinism + replay round-trip with 7-10 bytes; stayed-swing (ACTed monster
deals no damage that turn, others do); becalm threshold + swap-on-bump; spared hashed
(state_hash differs when spared differs); v1/v2 back-compat; talk-line length test.
Dump goldens + frame goldens must hold (no worldgen/channel changes; becalmed not in fresh
frames). xhash fixture (v1) must still pass.

**T2 — pacifist bot + gate** (headless.rs, tests/pacifist-band.json, Makefile): `--sim N
--policy pacifist` — greedy bot variant: when the path-blocking or adjacent monster is not
calm, ACT it (correct direction byte); never bump-attacks; otherwise identical loot/route
logic. Measure at 5000, THEN write the band: required shape per DECISION — pacifist runs must
be viable (win_pct band ≥ some floor after honest tuning of regard thresholds/tax within this
brief) and its death mix reported (the inversion claim is measured, not assumed — record
whatever is true). `make sim` runs both policies. If no tuning inside this brief's knobs
lands a defensible band: STOP, report — mercy gets cut per DECISION, do not force it.

**T3 — frontends** (both backends): `t`/`a` + direction → ACT bytes (two-keystroke chord,
one logged byte; lone `t` with no direction within the read = no-op); becalmed tint;
key legend updates (title/end screens ≤78 cols). Term + minifb identical log semantics.
Playtest-flag all feel.

**T4 — docs**: CLAUDE.md (vocab 0-10, v3, regard/calm/spared conventions, stayed-swing rule,
Henson ruling note, pacifist gate in make check), status entry with both policies' measured
numbers, playtest list. Ledger updated.

## Gates & constraints

House process: subagent implementer → reviewer → fix rounds per task; final whole-branch
review; `make check` green after every task (greedy sim numbers must NOT move until T2 tuning
touches shared knobs — if T1 shifts greedy sim, something leaked). Sizes reported (budget
≤ +8 KB packed). rustc 1.75, zero deps, no cfg in core, grounding doctrine on every line.

## Addendum (2026-07-19, human direction): the parley algorithm + rename

**Rename (human ruling):** "ACT" is Undertale menu jargon, not an acronym — the verb is TALK.
Rename identifiers/comments/docs: try_act_player → try_talk_player, act_threshold →
talk_threshold, ACT bytes → talk bytes, chord naming likewise. Bytes 7-10, save v3, bands:
unchanged. "regard"/"becalmed" stay.

**Parley (human direction: "needs to be algorithm'd"):** becalming was a flat counter with a
guaranteed stayed swing — root cause of the pacifist-dominance finding. Replace with a
receptivity roll per talk, integer math, all inputs already tracked:

    receptivity = BASE[kind]            // rat 55, goblin 35, ogre 20   (tunable)
                + 18 * regard           // persistence pays             (tunable)
                + 40 * (maxhp-hp)/maxhp // wounds open ears
                + 6 * (atk - 3)         // visible strength impresses
                - 10 if fov_radius(light) <= 4   // guttering torch
                clamp(5, 95)
    landed (roll < receptivity): +1 regard, monster stayed this turn
    failed: no regard, NOT stayed (monsters_act treats it normally)
    becalm at regard >= talk_threshold (2/3/4 unchanged); talk always costs the turn, no tax.

Rolls from a NEW per-run named channel `parley` (Game field, hashed like combat/ai/flavor) —
never combat_rng, never worldgen. Talk lines: landed vs failed may voice differently (failed =
the monster is unmoved — grounded, no new invented claims; reuse/extend TALK_LINES shape).
Greedy sim must stay exact (726/4266/8/0). Pacifist band: REMEASURE at 5000 and re-baseline
tests/pacifist-band.json (sign-off = this addendum); pacifist policy may need a
retreat-or-persist rule when talks fail — keep it deterministic, document the policy change.
Constants above are starting values; tune within [5,40] win_pct with the same honesty rules
(STOP clause stands).
