# The Mercy Economy Arc — light as grace, violence as doom

**Status: design canon (draft), converged in the 2026-07-22 brainstorm. Governs batches
10–13, the deferred scale/journey MAJOR, and the donkey-companion reveal (§9-F-adjacent).
This is NOT a batch brief — each batch gets its own brief under `docs/superpowers/plans/`,
written against this spine. If a brief and this doc disagree, the more recent human sign-off
wins; flag the drift.**

## Why this exists (the diagnosis)

Playtest of the batch-9 overworld surfaced seven observations that are all one problem:
**the game has a full set of mechanics — mercy/talk, the McGuffin's conscience, the torch
clock, portals — and no *situations* that force any of them to bite.** Combat is
brute-forceable, so mercy is a strictly *worse* choice, not a hard one; the McGuffin's
kill/spare commentary is noise because killing is free; you zoom through a screen-sized
5-floor dungeon reading nothing; the one real pressure (the torch) shows up only as unfair
dark-death in portal side-worlds; the becalm verb has no payoff; the donkey is furniture.

Difficulty and moral friction are not tuning knobs here — they are the missing ingredient
that gives every existing system a reason to exist. This arc supplies it.

## The north star

**Violence is intuitive but doomed. Diplomacy is the reliable path that still costs.**

- A perfectly-played **brute-force** run tops out at ~**50% wins** — a coin flip at best.
  Not unwinnable; *capped*. The obvious path is the losing path.
- A well-played **diplomacy** run wins **meaningfully above 50%**, but **harder than it is
  today** — the reliable route, never a cakewalk.

The entire difficulty design is those two numbers, expressed as bot-measurable targets (below).

## The economy: two doom-engines, layered

Violence is doomed along two independent axes that compose. They are not redundant: one
makes each *fight* a real decision, the other makes the *strategy* doomed.

### Engine 1 — HP attrition (moment-to-moment fear)
You cannot fight for free.
- **Ogres land a guaranteed hit when you stand up to them** — engaging an ogre always costs
  HP, no roll, no dodge. (Mechanism: a guaranteed retaliation on the player's attack against
  an ogre; exact damage is a tuning target for the batch-11 bots.)
- **Healing gets scarcer** — potions are currently "bloody everywhere, for free." **Cutting
  the potion drop rate is the primary heal-scarcity lever** (the heal-on-descent is a
  secondary knob), so guaranteed attrition can't be trivially refilled. This is the one lever
  that tightens the HP axis without touching light — the two engines stay cleanly separated —
  and it doubles as making the potion→monster give (see "the becalm toolkit has texture") a
  real sacrifice rather than a spammable becalm.
- Effect: each fight becomes *trade / flee / talk-down*, not free bump-through. This is the
  terrain the tactical bot navigates.

### Engine 2 — light as grace (run-level ceiling): §9-E promoted to keystone
§9-E (McGuffin mood → light), previously priced "cheap flavor, no sign-off," becomes the
economic keystone:
- **Killing dims your light** — the McGuffin recoils; your world literally shortens. (On top
  of the existing per-attack `VIOLENCE_TAX`.)
- **Sparing feeds your light** — a becalm is the game's only *renewable* light source.
- **The McGuffin's mood IS the fuel gauge** — its dimming/quieting is not flavor, it is the
  survival readout. The narrator-cannot-lie doctrine holds: mood is engine state, and its
  dimming is a verifiable fact about what you did.
- Effect: a kill-heavy descent can't afford the 2× climb-out burn, so violence caps at ~50%
  **even when you win every fight.** The return trip becomes the scene the mechanics always
  implied — climbing through floors whose light economy you set on the way down.

## The measurement reframe (ships FIRST, batch 10)

The instrument is anchored to a player too dumb to proxy a human. The greedy bot wins ~17%
(849/5000) because it never retreats, heals strategically, or declines a bad trade — so a
game tuned until *that* player wins 1-in-6 is a game a competent human brute-forces while
reading nothing. Fix the instrument before touching a single tuning constant:

- **Add tactical sim policies** — a *violent* tactical policy (retreat at low HP, quaff,
  avoid bad trades, route around ogres) and a smarter *diplomacy* policy. Deterministic,
  channel-disciplined, gate-able exactly like the existing greedy/pacifist bots.
- **Targets (the difficulty spec):** well-played violence → **~50%**; well-played diplomacy
  → **above 50%, harder than today** (the current pacifist bot's 9.3% is *dumb*-play, not the
  target).
- **Re-anchor the bands** — new tactical bands, plus review of `tests/sim-band.json` /
  `tests/pacifist-band.json`. **Greedy stays** as a floor-of-competence reference: its band
  must not move when the tactical bands change (same discipline greedy-vs-pacifist already
  hold — they share the engine, not the gate).
- This batch **ships nothing a player sees.** It de-risks everything after it, and it is a
  *hard dependency* of the economy work: light-as-grace can trivially recreate batch 5's
  *pacifism-strictly-dominant* failure, and only competent bots on *both* routes can prove
  the two paths stay balanced.

## Winnability migration

`--solve` never simulates combat — it is a pure geometry/reachability + walk-budget check, so
its guarantee has *always* implicitly been about the low-violence path. Formalize that:
- **The per-seed guarantee migrates to "a diplomacy / light-positive completion always
  exists"** — grace is always *available*.
- **Violence is a self-imposed gamble the solver never promises to cover.** A high-kill run
  *can* strand you in the dark; that is the design, not a bug.
- `START_LIGHT` is re-derived under the new economy (spare-income and kill-dimming change the
  budget). Requires the balance sign-off (below).
- **Telegraphing is mandatory:** mercy must always be *reachable*, and the light cost of
  killing must be legible, or a new player softlocks into darkness and feels cheated rather
  than judged.

## Two teachers: the overworld as a mercy tutorial

The overworld is where the game teaches its own thesis with the stakes off, through two NPCs
with opposite lessons. The player learns the thesis by feeling the gap between them.

### The trainer — the voice of the doomed instinct
`TRA_001` ("Rule one: kill five rats. Everyone does it.") is not a quest and gives **no
reward** — a reward for killing would pay you to do the doomed thing and break the whole arc.
He is the intuitive, received wisdom the game exists to disprove; the light economy is the
punchline to his setup. Nothing new to price for the setup itself (Rule 4 clean).

**The trainer reads your last life.** Because the hole is one-way, the only way back to the
trainer is *death and resurrection* — so when he acknowledges how you played, it is inherently
ironic (he is reacting to the run that just killed you). On resurrection we carry **one
echo-shaped, presentation-only memory** — "last life was bloody" (`kills > spared` at death,
the same read the McGuffin's pickup register already uses), forwarded into the fresh run
exactly the way `echo` (the death position) already is. The trainer's dialogue reads it:
- **Came back bloody** → he ironically salutes your rat-slaying prowess (the door closing on
  the old wisdom — the very thing he told you to do is why you're back). This is the home for
  the held-out canon line `TRA_007` ("seeing you again").
- **Came back merciful** → a different note; he doesn't quite know what to make of a pacifist.

No new hashed state (the memory is derived and presentation-only, like `echo`); no persistence
subsystem (that stays deferred, §9-J ASK #7). This is a contained echo-shaped carry, nothing
more.

### The donkey — a three-rung kindness ladder, and a secret
The donkey is **secretly invincible** — and already is, in the engine: `passive` + `bump:
Shove` means no code path can damage him. We are not building invincibility; we are building
the **secret** and the **reveal.** The game withholds what the engine already knows; the
narrator never lies.

His arc has three rungs, each a deeper investment of kindness:
1. **Aloof (start).** He ignores you. In character — he is the stubborn `Shove` monster, and
   he is *half a donkey*; instant devotion is wrong for him.
2. **Follows you (schmoozed).** Climb his existing `DONKEY_TALK` regard ladder; the top rung
   `DON_005` — *"The donkey stands beside you now. On purpose."* — flips him to follow-mode
   (zero new content; follow just makes the line true). He follows you around the overworld
   and across screen-links but **will not descend the hole** — not from fear or inability
   (he's super), but because he only goes *down* for someone he **loves**. The hole is a
   threshold of devotion, not a barrier. The player assumes he's scared or stuck; the reveal
   overturns that. **This rung is the batch-12 seed** — a ~20–30-line overworld-only follow.
3. **Loves you → descends → the reveal (later companion batch).** Earn his LOVE (a tier above
   follow), and he walks into the dark with you. The first ogre you meet, instead of the
   guaranteed hit, the donkey ambles up and it **becalms** (reusing the existing `Monster.calm`
   state, triggered by the donkey's presence — donkeys and ogres are best buds). *That* is
   the reveal: he was never afraid, nothing here can touch him, and the aloof half-a-donkey
   you buttered up in a field is the most powerful thing in the game — and he chose you.

Why the secret-super framing is load-bearing: it **dissolves the resurrection tension** (he
can't die, so the anchor always holds) and **simplifies the companion** (not a fragile escort
you protect — an invincible one who protects *you*; no keep-the-NPC-alive fail state). The
donkey-follow is thus the *seed* (rung 2, batch 12); the LOVE-companion + ogre-becalm + reveal
is its *destination* (rung 3, a later companion batch, §9-F-adjacent). The ogre-becalm and the
LOVE tier are **new mechanics — priced and signed off, not assumed** (Rule 4), and ride the
§9-F NPC-vault worldgen MAJOR (DECISION.md item 4). A **giftable donkey-treat** (new item +
`give_table` row) stays **optional, not core** — and would be its *own* treat item, never
cheese (the positive-cheese give-target is now assigned to goblins, story §12.14 → see "Cheese
has a target" below).

## The batch arc

1. **Batch 10 — tactical bots + band re-anchor.** Measurement infra; ships nothing
   player-facing; establishes the real difficulty and the two target numbers. Hard
   dependency of all economy work.
2. **Batch 11 — combat hardening.** Ogre guaranteed hit + heal scarcity; tune violence →
   ~50%, diplomacy → harder, using the batch-10 bots. Combat-balance MAJOR (moves the bands;
   same sign-off class as batches 3/4).
3. **Batch 12 — light as grace (§9-E keystone)** + becalm return-trip dividend + NPC category
   fix + the trainer-reads-your-last-life reaction + the donkey-follow **seed** (rungs 1–2).
   The economic unification; the balance / `START_LIGHT` MAJOR.
4. **Batch 13 — portal ROI.** Loot tables scaled to risk (light caches, unique items,
   otherwise-unreachable lore); the price telegraphed at the threshold in engine facts (the
   portal already knows its destination). Shallow dives +EV for the attentive; deep dives are
   the gamble. The multiverse becomes the pilgrimage light-rich pacifists can afford and
   slaughter-route players cannot.
5. **The donkey-companion reveal (§9-F-adjacent).** Rung 3: LOVE tier → descent →
   ogre-becalm-by-presence → the secretly-super reveal. Rides the §9-F NPC-vault worldgen
   MAJOR. Sequence relative to 13 when it is scoped.
6. **Later — scale & journey (the deferred "B").** Bigger/longer floors so navigation itself
   costs something. Its own worldgen MAJOR; deferred until scarce, morally-charged light makes
   bigger floors *mean* something — expanding before the economy bites just adds rooms to
   zoom through. Let the tactical-bot bands catch what bigger floors do to the budget
   derivation.

## Becalm return-trip dividend (rides batch 12)

Passing a becalmed monster refunds a trickle of light ("it remembers you; it lights the way").
The pacifist descent becomes an investment whose interest is paid on the climb. **Guard:** this
is precisely the lever that made pacifism *dominant* in batch 5 (the guaranteed stayed-swing)
before the parley repricing — tune the dividend against the tactical bots and bands, never by
feel.

## Cheese has a target (rides batch 12 / give-table)

Cheese is not universally good — it teaches the player to read the bestiary. The `give_table`
already prices **cheese → rat as a regard PENALTY** (story §4's subversion: everyone "knows"
rats want cheese; they don't). The human-reserved *positive* cheese give-target (story §12.14)
is now assigned by the human: **cheese → goblin is a major positive** — goblins love cheese.
Under §9-E a cheese-befriended goblin that becalms *feeds your light* (a spare). And because
cheese also has a self-USE (burn it for a light flicker, batch 7), each wheel is a fork in the
light economy: **burn it for light now, or spend it winning over a goblin** whose becalm pays
light now (the spare) and again on the climb (the return-trip dividend). One data row on the
existing give mechanism — Rule-4 cheap.

**How cheese lands on a goblin (resolved):** it *always* **stays** the goblin — stops it
attacking or advancing that turn (reusing the batch-5 stayed mechanic; it's distracted, eating)
— and *rolls* for an **outright becalm** (a `receptivity`-style roll; on a hit it goes fully
`calm`). So cheese is guaranteed **tempo** and gambled **grace**: even a failed roll buys a safe
turn to reposition, line up the awe, or flee; a landed roll is a spare that feeds light (and
pays again on the climb). Cheese scarcity keeps the gamble from becoming a pacifism-dominant
economy. Ogres get **no** cheese route — they are not cheese-lovers; their only becalms are awe
or the donkey.

## The becalm toolkit has texture (talk / cheese / potion)

The three ways to win a hostile over are deliberately priced differently — that texture is what
makes diplomacy a *skill* rather than a spam, and it must be preserved, not "balanced" flat:
- **Talk** — free but risky (a failed `receptivity()` roll buys nothing and invites a swing).
  The everyday tool.
- **Cheese → goblin** — cheap and repeatable-ish, but goblin-specific (and a *penalty* on a
  rat). The read-the-bestiary tool.
- **Potion → biology-coded.** Potion is *human medicine*, which resolves "rats love potion,
  not sure why": it works on **mammals** and poisons everything else. (Per-kind `give_table`
  rows, batch 12, replacing the old blanket potion→any-monster row.)
    - **→ rat (a mammal)** — heals and becalms it. Expensive, and a bad trade *by design* (a
      whole scarce potion for a rat) — an inexplicable-seeming quirk that is really just
      biology; keep it. It is the most self-sacrificing mercy in the game: you spend your
      rarest resource to heal the very thing trying to kill you.
    - **→ goblin / ogre (NOT proper mammals)** — *harmful.* Their biology is different enough
      that the medicine is poison: regard crashes and it lands **like an attack**, a fixed
      effect **not tied to ATK strength** (a weak-combat player can still do it). Potion is
      thus the sharpest read-the-bestiary **trap** in the game — the naive player who potions
      an ogre expecting a becalm instead *provokes* it.
      - **Mechanic (resolved): enrage.** The give lands as a **regard crash + a free swing at
        you** (reusing the failed-talk retaliation path). The potion does **no** damage to the
        goblinoid; the danger is the hit you just invited (and whatever fight follows). No new
        give-as-harm mechanic — Rule-4 cheap.
      - Because it never harms the monster, potion on a goblinoid is **purely a trap** — only
        ever a mistake (a wasted, scarce potion and a free hit on you), never a useful weapon.
        That keeps potion honestly a *mammal-medicine* with **no ranged-violence loophole**:
        the only ways to handle a goblinoid stay fight / awe / (goblins) bribe, so the
        doomed-violence design stays airtight.

The **universal** becalm is **talk**, not potion — talk works on anything, cheese is
goblin-coded, potion is mammal-coded. Do not flatten these into one interchangeable "becalm
cost." The differing prices, and the biology behind them, are the design.

## Goblinoid awe — becalm through nerve, not talk (batch 11 behavior + batch 12 becalm)

**This is the concrete answer to "diplomacy must be harder than it is now."** Mammals (rats)
becalm through ordinary talk (`receptivity()`). **Goblinoids do not — they becalm through AWE**,
and awe is earned by a *movement tactic that is opposite for the two kinds and lethal if you use
the wrong one.* Diplomacy stops being spam-talk and becomes per-enemy nerve under lethal
pressure.

- **Ogre — awe by holding.** Stand tall and do not flinch: hold your ground *through its
  guaranteed hit* (do not retreat). Enduring the blow without fleeing awes it → becalm.
  **Retreating from an ogre gets you killed** (ogres punish flight).
- **Goblin — awe by giving ground.** Walk *backward* while talking — yield space, stay
  composed. That awes it → becalm. **Standing still against a goblin gets you killed** (goblins
  punish the planted target).
- **The moves are opposite and mutually fatal.** Hold vs. an ogre; give ground vs. a goblin. Do
  the ogre move to a goblin, or the goblin move to an ogre, and the other one kills you. Read
  the creature and commit to the correct, opposite response.

**Why this is the keystone of hard diplomacy:** it fuses with engine 1 — the ogre's guaranteed
hit is no longer only attrition, it is the *awe test.* The thing that makes ogres terrifying
(they will hit you) is the same thing that lets you becalm them (stand through it). And it draws
the arc's cleanest HP-vs-light line: **fighting** an ogre trades blows, kills it, and dims your
light (doomed violence); **awe-ing** it costs HP (you eat the hit) but spends no light and
*gains* light on the becalm (grace). Same monster, same blow, opposite economy — decided by
whether you flinch.

**The bestiary's becalm paths, with this in:**
- **Rat (mammal)** — talk (`receptivity`). Potion heals + becalms (a bad trade).
- **Goblin** — awe (walk backward while talking); cheese warms it (reconciliation below).
  Potion *poisons* (trap).
- **Ogre** — awe (stand tall through the hit) **or the donkey's presence** (best buds). **No
  bribe exists** — which is *why the donkey-companion reveal matters*: he trivializes the game's
  single hardest becalm. Potion *poisons.*

**Cheese-vs-awe reconciliation (resolved):** both hold — **awe is the *reliable* goblin becalm**
(skill: walk backward while talking), and **cheese is the goblin-specific *bribe*** that always
*stays* the goblin (stops attack/advance) and *rolls* for an outright becalm (see "Cheese has a
target" for the roll model). Two routes to a calm goblin — nerve or a bribe — and only goblins
get the bribe.

**Scope / sign-off:** a real new mechanic (goblinoid awe-becalm + ogres-punish-retreat +
goblins-punish-holding + the movement encoding), Rule-4 priced, spanning batch 11 (the two
combat behaviors) and batch 12 (the awe-becalm + tells). It also makes the **tactical diplomacy
bot** (batch 10) much smarter — it must read the creature and pick the opposite move — so the
bot either anticipates awe from the start or is extended when awe lands. **Telegraphing is
mandatory:** the ogre/goblin tell must be legible so wrong-move death is *learn-by-death*
(carried by the resurrect → trainer-ribs-you loop), not a gotcha.

## Sign-off / MAJOR flags (the honest price)

- **Light-as-grace is a balance MAJOR**, not "cheap, no sign-off." As the keystone it
  reprices the light economy, re-derives `START_LIGHT`, and reshapes the winnability
  guarantee. Explicit sign-off required (batch 12).
- **Combat hardening (batch 11) moves the sim bands** and the ogre-hit is a combat-math
  change — both re-anchored against the batch-10 tactical instrument. Combat-balance sign-off.
- **The donkey-companion reveal** (LOVE tier + ogre-becalm-by-presence + descent) is new
  mechanics on the §9-F NPC-vault worldgen MAJOR — priced and signed off when scoped, never
  assumed.
- **Portal ROI (batch 13)** touches loot/reward tables and possibly the vault/worldgen
  channel — determine whether it is a worldgen MAJOR when it is scoped.
- Every tuning number in this arc — kill-dim amount, spare-feed amount, ogre-hit damage, heal
  scarcity, becalm dividend, diplomacy friction, the donkey's follow/LOVE thresholds — is **a
  measured quantity, tuned against the tactical bots, never hand-tuned to feel and then
  band-regenerated around the feel.** That discipline is what makes this repo special; do not
  lose it. It is the single most important process constraint in this arc.

## Practice note

Capture played playtest sessions as save/ghost files from here on. "Feel" cannot be goldened,
but a library of real human runs is the sanity-check corpus for the tactical sim policies
(does the bot lose the runs a human loses and win the ones a human wins?), and eventual ghost
content. This is the first entry against the batch-9 playtest-pending list turning into an
asset.

## Open questions deferred to their batches (not blockers here)

- Exact ogre-hit damage, heal-scarcity curve, kill-dim / spare-feed magnitudes, becalm
  dividend size, and "harder diplomacy" friction — all **measured against the batch-10 bots**,
  not decided in this doc.
- The donkey's follow-threshold and LOVE-threshold values, and whether ogre-becalm-by-presence
  is ogres-only (the Shrek joke) or extends to other kinds — resolve in the companion batch.
- "Harder diplomacy" is now primarily answered by **goblinoid awe** (the opposite,
  lethal-if-wrong hold/give-ground tactics) on top of talk-risk and light-management; the exact
  awe input encoding and the tells resolve in batch 12 with the instrument in hand.
- Whether portal ROI is a worldgen MAJOR — resolve when batch 13 is scoped.
