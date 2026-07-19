# Monomyth for rl144 — narratologist notes
## 1. Stage condensation
Neither Campbell's 17 nor Vogler's 12 survives a 30-60 min permadeath loop
intact — both assume scene budget for Mentor, Temptress, Atonement with
Father that a roguelike run doesn't have. Bespoke cut, six beats, keyed to
what the engine already does:

1. **Threshold** — entrance `<`, depth 1. Collapses Crossing the First
   Threshold *and* Belly of the Whale into one beat: descending IS the
   swallowing. Depth 1→2 is already functionally irreversible mid-run, so no
   separate "point of no return" scene is needed.
2. **Road of Trials** — depths 2-4. Not scripted; *is* ordinary play (combat,
   light spend, lore tiers as Donor-function echoes). This is where the
   mercy/violence economy belongs — it's the trial, not a cutscene about it.
3. **Ordeal + Boon** (compressed) — depth 5, amulet. Campbell separates
   Apotheosis from Ultimate Boon; the engine shouldn't try — pickup-on-
   walkover already fuses transformation and theft-of-the-boon into one input.
4. **Road Back** — the climb, at 2x light cost. The strongest beat in the
   cut because it's mechanically true already, not just asserted. Don't
   narrate it; the doubled number IS the Road Back.
5. **Return Threshold** — re-crossing `<` on depth 1, at the run's lowest
   resources. Treat this, not the amulet pickup, as the emotional climax.
   Crossing the Return Threshold + Resurrection collapse here: the character
   who exits is mechanically thinner (less light) than the one who entered —
   "tested self" made numeric.
6. **Elixir, socialized** — the win feeds the ghost/replay system. Freedom
   to Live isn't a stage the character gets; it's what the *next player*
   gets from watching this run.

**Cut entirely, not collapsed:** Meeting the Goddess, Temptress, Atonement
with the Father — relationship-arc stages needing a persistent NPC with
interiority, the wrong shape for a procedurally generated, seed-scoped cast.
Forcing them in produces the checklist failure in §2. Refusal of the Call and
Rescue from Without are *not* cut — see §4, they're cheap and the parts exist.

## 2. Three failure modes of literal mapping
**a) Checklist-quest feel.** Stages become UI-legible objectives ("Refusal
of the Call: done"). This converts Barthes' hermeneutic code (meaning
withheld, resolved through reading) into his proairetic code (mechanically
legible action-sequence) — myth stops being *recognized*, starts being
*tracked*. Derived structure avoids this only if inference stays off-UI —
expressed through consequence (a lore tier, a light-cost delta), never a
labeled tracker.
**b) Homogenization.** Literal mapping forces every seed/theme through
identical beats regardless of what happened — fatal for "four themes
reskinning one structure," since the joints show. Derived structure fixes
this: the same action (avoiding combat four times) can read as Refusal in
one run's history and Atonement in another's, because the label depends on
accumulated run/ghost data, not a depth counter.
**c) Single-hero myopia.** Campbell assumes one psyche. rl144 has a live
player, a doomed bot, and past-self ghosts in one seed — literal mapping has
no slot for "whose journey is this." Derived structure fixes it by making
the structure cross-run: stage inferred from history-across-attempts, not
position-within-one-run.
**Strongest warning, and where derived structure can make it worse:** if the
inference rule is legible enough to reverse-engineer, players farm the input
that triggers the flavor text they want ("spam mercy for the Atonement
lore"), reconstructing the checklist one level removed, disguised as
emergence. Worse: if the inference is *inaccurate* — "Atonement with the
Father" pinned to a stat pattern produced by bad luck, not choice — it
breaks the sign system's credibility outright, reading as a bug rather than
a myth. Literal mapping's sin is rigidity; derived mapping's failure is
unearned coherence, and that's worse — it spends trust, not just pacing.
## 3. Where death/retry/ghosts sit
Not Refusal (pre-threshold hesitation; death happens well after crossing —
mistimed if applied). Not quite Belly of the Whale either — Belly marks
committing to the unknown at the *start* of a run; death mid-run is a
consequence, not a commitment.
**Best reading: the cyclical cosmogonic round** (Campbell via Eliade's
eternal return) — each run is a world-age; death ends the age; retry-same-
seed is sacred repetition (literally: same cosmos regenerated from the same
hash), new-seed is a genuine new cosmogony. Not a stretch — the engine's own
"retry same world" vs "new world" verb distinction *is* the eternal-return /
new-creation distinction, already built.
**Hook this buys:** ghosts read as ancestor-spirits within a world-age, not
personal-failure markers. The guaranteed doomed-bot ghost becomes a fixed
feature of every cosmos — proof the labyrinth has always claimed a hero,
present before you ever played, stronger myth than "your last death log."
Past-self ghosts on retry-same-seed become the mythic-dead-returning-as-
warning motif (a degraded Supernatural Aid — a mark on the floor, not a
mentor), actionable as a light/waypoint hint at a prior death tile: the
world remembers, rather than "you have a save file."

## 4. Refusal of the Call and Rescue from Without, cheaply
**Refusal** is pre-threshold, so hook it there: at the retry prompt after
death, surface existing per-seed ghost/lore data as unscripted hesitation —
"you have died here before" is free (data exists already), and the real
refusal is the *player's own dwell-time* before pressing retry — diegetic to
behavior, not authored text. Don't animate hesitation; the thumb is it.
**Rescue from Without** — aid arriving after own strength runs out. Map it
onto the return leg specifically: surface another player's replay ghost or
the doomed bot's death-tile data as a waypoint/light marker *only* on the
climb back, never the descent. Cheap — the replay data already exists; it
just needs reading on the outbound/return distinction the engine tracks.
**Verdict on "abandoned runs leave ghosts in the village":** gimmick, if
labeled Refusal — an abandoned run has already crossed the threshold, so by
lapse-time it can't structurally be pre-threshold hesitation; the timing is
wrong, not the flavor. Repurpose as an unresolved-dead / restless-ghost
motif, or better, feed it into Rescue from Without: a later player who finds
the abandoned ghost gleans a small mercy (light, a safe route) from it — aid
from outside their own strength, exactly the function's definition. Same
asset, correctly re-targeted stage.

## 5. What Propp offers that Campbell doesn't
Campbell psychologizes a single hero; Propp is combinatorial — 31 functions
in fixed order but variable *presence*, separated from the 7 spheres of
action (villain, donor, helper, prize, dispatcher, hero, false hero), so
**function is independent of agent**. That's exactly the reskin problem: the
Donor function can be fulfilled by a lore inscription in one theme and a
vault in another without touching structure — Campbell gives no equivalent
decoupling. Propp's Lack/Liquidation-of-Lack pair (VIIIa/XIX) is a cleaner
generative driver for worldgen than "the Ordeal": a slot-fill constraint
(place a lack, place its liquidation) rather than a dramatized beat — what a
generator actually consumes. Division of labor: **Propp for generation
grammar** (tag monsters/items/ghosts by which function they discharge, let
worldgen assign functions to entities per seed), **Campbell/Vogler for the
felt arc** (pacing, the sjuzhet-level presentation of Road Back / Return
Threshold). Propp has no interiority — no want/need/lie — so it can't
replace Campbell for making a run feel personal; it's the grammar under the
myth, not the myth itself.
