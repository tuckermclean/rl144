# rl144 — SPACES DRAFT v0 (staged content, ASCII maps + notes)

# Companion to STORY-COMPILE-v1.md (§3.2, §6) and FLAVOR-DRAFT-v0.md. IDs
# are stable: SPC_MIDDEN, SPC_STAGE, SPC_OVR_1..3, SPC_GAG_*. Swap the maps
# freely; keep the IDs so downstream references don't break.
#
# Engine legend as read from src/content.rs at draft time:
#   VAULTS:          '#' wall, '.' floor, '!' potion, ')' sword,
#                     'r'/'g'/'O' rat/goblin/ogre, '^' pit, 'B' push-block,
#                     'x' goal. Rectangular, solid '#' border, center tile
#                     '.' (corridors punch through walls to reach center).
#   AUTHORED_FLOORS:  '#' wall, '.' floor, '<' return portal, '!' potion,
#                     ')' sword, 'r'/'g'/'O' monster. Rectangular, solid
#                     '#' border, exactly one '<', up to 80x25.
# Every glyph below outside those two lists is marked NEEDS-ENGINE-LEGEND
# at first use. A consolidated list is at the bottom of this file.
#
# One mechanical note that shapes several designs below: ordinary
# (non-sokoban) vaults get a corridor punched straight to their center
# tile at generation time, regardless of interior walls (content.rs's own
# vault doc comment: "corridors target the center... will punch through
# walls to reach it"). For an elaborate/maze-shaped vault this would
# normally be a bug to route around. For the sight-gag vaults (§4 below)
# it is instead the punchline, deliberately: the map is authored whole and
# intact; the generator's own straight-line corridor becomes, at runtime,
# the thing that already broke into the fortress before the player did.
# No new system is needed to make this true — it's the existing rule,
# pointed at a joke instead of an inconvenience.

---

## 1. SPC_MIDDEN — THE MIDDEN (D1 vault, story §6.1, "Locked")

Strongest spatial joke: a monument built from an entire species'
worth of cheese wheels, in tidy defensive rings, around one dead man.
The architecture is bigger than the grave it's for.

```
###########
#.o.o.o.o.#
#o.......o#
#.o.....o.#
#..o.T.o..#
#.o.....o.#
#o.......o#
#.o.o.o.o.#
###########
```
11 wide x 9 tall. `T` sits at the exact geometric center (row 4, col 5),
matching the story's "one tombstone in the middle" — a deliberate
exception to the generic-vault convention seen in the existing pool
(guard-post/reliquary/ogre-den all offset their items *off* dead-center;
see content.rs's `VAULTS`). Flag for engineering: confirm the
corridor-punch-to-center rule tolerates a non-`.` walkable tile at
literal center, or relax MIDDEN/STAGE to sit one tile off (as §2 below
already does for the gag vaults, out of caution).

**NEEDS-ENGINE-LEGEND:**
- `o` — cheese wheel. Scenery, not a pickup (cheese has no item glyph
  yet per content.rs's own VAULTS comment — batch 7 territory). Proposed
  as **impassable** (you walk around the monument, not through it), same
  category as a decorative wall segment. Legibility flag: lowercase `o`
  next to uppercase `O` (ogre) is a close pair at font8x8 scale — worth a
  contrast/placement check once rendered.
- `T` — tombstone. Proposed as **walkable, decorative floor** (like `x`
  goal's "ordinary floor for every game-logic purpose except render"),
  so it never blocks the corridor-punch guarantee. Carries `EPI_003`.

**Flavor anchors (existing, no changes):**
- `EPI_003` ("He died believing." — LOCKED, midden stone) on `T`.
- `NAR_004`, `NAR_032`, `NAR_033` (cheese underfoot / pickup / burn) —
  ambient, fire anywhere on D1, especially dense here.
- `RAT_001`–`RAT_005` (aggrieved-stage cheese grievance) — thematically
  anchors near the midden even though rats aren't placed inside it.
- `LOR_S01`–`LOR_S04` (the five-stages-of-cheese graffiti saga) —
  scattered D1 generally; the midden is where the saga's punchline
  stands as architecture instead of handwriting.

**New line-slot:** `[YOURS-CANDIDATE] NAR_090` — first-sight narrator
line for stepping into the midden. Job: one flat observation about scale
("a great many wheels, arranged with care") in the narrator's dry
register — must not state the joke ("this is a monument to a bad
strategy"), must not editorialize, ≤78 chars. The room does the joke;
the line just proves the narrator saw it and moved on.

---

## 2. SPC_STAGE — THE STAGE (D5 vault, story §6.2, "Locked")

Strongest spatial joke: a ring of scrawled-over wall and tally marks
that gets denser the closer you get to a single, small, perfectly
centered, completely unguarded pedestal — four hundred years of
revision compressed into one room, all of it pointing at one object.

```
#############
#...........#
#%%%%%%%%%%%#
#%...,,,...%#
#%...,&,...%#
#%...,,,...%#
#%%%%%%%%%%%#
#...........#
#############
```
13 wide x 9 tall. `&` sits at exact center (row 4, col 6). Density
increases strictly from any entrance inward — plain floor, then scrawled
wall, then tally-marked floor, then the pedestal — which reads as
"thickening toward it" from every direction, not just one, and needed no
directional cheat to say so. Not trapped, not locked (§3.4): every tile
around the pedestal is open floor, nothing blocks the walk-up.

**NEEDS-ENGINE-LEGEND:**
- `%` — densely-scrawled wall (vs. plain `#`). Presentation-only wall
  variant; same solidity/impassability as `#`, distinct render color.
- `,` — tally-mark floor decoration. Walkable, decorative floor, like
  `T` above.
- `&` — amulet/pedestal placement **inside a vault's authored ASCII**.
  This is the one placement question that isn't just a glyph ask:
  currently the amulet is placed programmatically in the BFS-deepest
  room of D5, independent of vault stamping. For STAGE's "centered
  pedestal" to be true on every seed, either (a) STAGE is *guaranteed*
  as D5's deepest room every generation (reading "Locked" in §6 as
  "mandatory, not pool-drawn"), and the existing deepest-room amulet
  placement just needs to target STAGE's `&` cell, or (b) `&` becomes a
  genuinely new, generally-stampable vault glyph. Flagging both readings
  for a placement decision rather than assuming one.

**Flavor anchors (existing, no changes):**
- `TIR_008` ("You want the pedestal? Straight ahead. It's very...
  arranged."), `TIR_009` (not guarding, explicitly).
- `LOR_D16` ("a cleared space. the drafts thicken toward it, then
  stop.") — this line was already describing exactly this map before
  the map existed; treat it as confirmed, not coincidental.
- `LOR_D05`, `LOR_D08` (tally marks; "blocking: pedestal center...")
  anchor directly to the `,` ring and the `&` cell respectively.
- `MCG_001`–`MCG_004` (the six words + interruption) fire on walk-onto.
- `TIR_010` (pickup reaction, "the driest thing in the game").

**New line-slot:** `[YOURS-CANDIDATE] NAR_091` — first-sight narrator
line for entering the stage, before the pickup event fires. Job:
one flat observation, same register as NAR_090, must not foreshadow the
duplicate (Rule 5) and must not state what the room means. Something a
narrator would say about ANY unusually tidy room, nothing that only
makes sense in hindsight.

---

## 3. SPC_OVR_1 / SPC_OVR_2 / SPC_OVR_3 — THE OVERWORLD (~3 screens, §3.2)

Respecting: no light clock up top (ENGINE flag, per story §3.2 and
engine-ask J — just noting placement here, not designing the mechanism);
overworld is the one fixed, hand-built, identical-every-seed stage
(§1.2); the drag-marks resurrection spot sits beside the donkey.

All three screens are drafted at a uniform 21x9 footprint (well inside
the 80x25 authored-floor ceiling) and link edge-to-edge in a straight
line: **OVR_1 → OVR_2 → OVR_3**, with OVR_1's dungeon mouth as the
separate downward link into the real, procedural, light-clocked game.

```
[OVR_1: the posting/the hole] == [OVR_2: the trainer's yard] == [OVR_3: donkey paddock + collector's house]
        |
        V  (the hole — descends into procedural D1, light clock starts)
```

Note on the border rule: both existing authored-content formats (VAULTS,
AUTHORED_FLOORS) require a solid `#` perimeter with exits punched only
at their one defined special tile (a vault's corridor-punch; a floor's
single `<`). These three screens intentionally break that pattern —
each has an edge cell replaced by `=` (or, for OVR_1, `V`) as a border
exit. That's a feature of the new linking mechanism being proposed here,
not an oversight: multi-screen chaining is a genuinely different shape
than either existing format, and is called out as its own engine
question rather than shoehorned into either one.

### 3a. SPC_OVR_1 — THE POSTING / THE HOLE

Strongest spatial joke: the sign-up sheet and the actual hole in the
ground sit a few steps apart in the same small yard — the entire
bureaucracy of the job and the job itself, side by side, to scale.

```
#####################
#...................#
#....?..............#
#...................#
#...................#
#...................#
#...................#
#..........V........=
#####################
```

**NEEDS-ENGINE-LEGEND:**
- `V` — the hole / dungeon mouth. New tile: the transition off the
  fixed, clockless overworld into procedural depth 1, where the light
  clock starts. This is the one tile in the whole draft that needs real
  engine plumbing beyond a glyph (crossing from AUTHORED_FLOORS-style
  fixed content into the seeded generator) — flagged, not designed here.
- `=` — fixed screen-link. A deterministic, named connection between two
  specific authored screens, distinct from batch 6's random `*` portal
  (which rolls a destination). Placed on the east border of this screen,
  linking to OVR_2's west border.

**No new ask:** `?` — reused exactly as the existing lore-item glyph;
the sign-up sheet is diegetically just a readable object, same as any
`?` lore pickup below ground. Free.

**Flavor anchors (existing, no changes):** `POS_001`, `POS_002`,
`POS_004` (notice, crossed-off names, "the hole has never needed to
advertise") anchor to the `?` tile and the yard generally.

**New line-slot:** `[YOURS-CANDIDATE] OVR_ENTER_001` — the line logged
the instant the player steps onto `V` and the light clock begins. Job:
state the threshold flatly and factually (narrator-verifiable: "this is
where it starts"), no dread-mongering, no thesis. Pairs with the
existing "danger only, never the duplicate" foreshadowing rule (§3.2).

### 3b. SPC_OVR_2 — THE TRAINER'S YARD

Strongest spatial joke: the yard has real, live, killable-or-talkable
rats standing around doing nothing, and the entire tutorial is deciding
what to do about that before the game has told you it's a decision.

```
#####################
#...................#
#..Y................#
#...................#
=......r......r.....=
#...................#
#...................#
#...................#
#####################
```

**NEEDS-ENGINE-LEGEND:**
- `Y` — the trainer. Stationary talk-target NPC, same category as the
  donkey (below) and already implied by engine-ask J's "trainer quest
  flags."
- `=` — fixed screen-link, west to OVR_1, east to OVR_3 (same tile kind
  as 3a's).

**No new ask, and a deliberate non-request:** the training rats are
drawn with the plain `r` glyph — ordinary rat entities, fully subject to
the real receptivity/regard/combat systems, not a new decorative-mob
category. This is a conscious choice to avoid re-opening "decor mobs,"
which the roadmap already dropped from batch 6 as not story-priced (see
CLAUDE.md's status log). A training rat is just a rat that happens to
stand in this yard; TRA_005/TRA_006 already cover what happens if you
spare or talk to one instead of the kill-five-rats advice.

**Flavor anchors (existing, no changes):** `TRA_001`–`TRA_008` cover
this screen completely — the rule-one speech, the depth-two boast, the
spare/talk reactions to a training rat, the resurrection line, the
repeat-death line. No gaps found; no new line-slot needed here.

### 3c. SPC_OVR_3 — DONKEY PADDOCK + THE COLLECTOR'S SHUT HOUSE

Strongest spatial joke: the shut house that will eventually decide the
whole game's ending sits directly next to a half-owned donkey with a
dotted line across its middle that the game will never once explain —
the two unresolved objects of the entire plot, sharing a fence.

```
#####################
#...................#
#........+..........#
#...................#
=........D..........#
#...................#
#...................#
#...................#
#####################
```

**NEEDS-ENGINE-LEGEND:**
- `+` — the collector's shut door. Impassable/bump-message tile.
  **Structural finding, not just a glyph:** this is the same door as
  §3.6's "the mantel." Before the amulet is carried, it logs `POS_003`
  ("NOT UNTIL IT'S IN HAND"). Once the player is carrying the amulet and
  approaches `+`, it should open into the `COL_`/`END_` sequence already
  fully drafted in FLAVOR-DRAFT-v0.md — no new lines needed for that
  scene, just this one tile's state-dependent behavior.
- `D` — the donkey. Stationary talk-target NPC with regard stages
  (already an engine ask per J), later re-glyphed at buyout (engine ask
  K, already TRIVIAL-priced — "one beat, no behavior").

**Placement note:** the resurrection point (drag marks) sits immediately
beside `D` — e.g. the floor tile south of the donkey at row 5 — per
story §3.7 and NAR_060/061. No new glyph: resurrection is a spawn
location, not a tile.

**Flavor anchors (existing, no changes):** `DON_001`–`DON_005` (donkey
regard stages), `NAR_060`–`NAR_063` (wake beside the donkey, drag marks,
recovered item, donkey watching the horizon), `POS_003` (the shut door,
physically here even though its ID lives in the POS_ table — cross-
reference only, no duplicate needed). `END_002` ("The donkey is whole
now." — LOCKED) fires on `D`'s re-glyph.

**Reference art only (not a map tile):** the task calls for "the ASCII
donkey with a faint dotted line across its middle." `D` on the grid is
one glyph; below is a small reference sketch for whoever builds the
actual sprite/SpriteKind, showing where the line sits. Never referenced
by any line of dialogue, ever — the point is that nobody in the fiction
comments on it, not that the player can't see it.

```
   ,,___
  /  ..'\.........
 ( .    )         <- the dotted line. never mentioned. ever.
  \____/..........
   |  |
  ='  '=
```

---

## 4. SPC_GAG_* — SIGHT-GAG VAULTS (§6.4, propose-and-strike)

Rule: pure visual joke, no system, must read from the MAP not from
text. All four below use only existing glyphs plus one new one (`o`,
already proposed for the midden — reused, not duplicated).

### TOP PICK 1 — SPC_GAG_MOAT

One-line rationale: an entire fortress of concentric walls, defending
nothing but an ordinary sword — the room is a bigger investment than
the item it protects, visible at a glance, no caption required.

```
#############
#...........#
#.####.####.#
#.#.......#.#
#.#.##.##.#.#
#.#.#)..#.#.#
#.#.##.##.#.#
#.#.......#.#
#.####.####.#
#...........#
#############
```
13x11. Center tile (row 5, col 6) is plain floor; the sword sits one
cell off-center at (row 5, col 5), matching the generic-vault convention
(items offset, not literal-center) out of caution — unlike MIDDEN/STAGE
above, this isn't a "Locked" story set-piece, so it takes the safer
option. Single gaps in each wall ring exist because a real moat has
doors; the corridor-punch will very likely ignore them anyway, which is
itself part of the joke (see the file-header mechanical note).

### TOP PICK 2 — SPC_GAG_THRONE

One-line rationale: a throne-shaped room, narrow tall back widening to
a base, built for someone who never sits in it — the only occupant is a
single cheese wheel, alone on the seat.

```
#################
#####.....#######
#####.....#######
#####.....#######
###.........#####
###.........#####
#...............#
#.......o.......#
#################
```
17x9. Reuses the `o` cheese-wheel glyph already proposed for the midden
(§1) — no new ask.

### PROPOSED, STRUCK — SPC_GAG_LABYRINTH

One-line rationale: the same double-ring maze technique as MOAT, but
the center is bare floor — an elaborate defense that, upon arrival,
turns out to have been guarding literally nothing.

```
#########
#.......#
#.#####.#
#.#...#.#
#.#...#.#
#.#...#.#
#.#####.#
#.......#
#########
```
9x9, fully sealed rings (no doors at all this time — leans harder into
the corridor-punch-as-punchline mechanic). Struck in favor of MOAT/
THRONE: this one needs the corridor-punch joke to actually land to read
as a joke at all (an unreachable maze just looks unreachable), where
MOAT and THRONE both read correctly even if nobody ever thinks about how
the player got in.

### PROPOSED, STRUCK — SPC_GAG_OVAL

One-line rationale: a plain, featureless room-within-a-room, with one
sealed cell dead center holding a potion and nothing else remarkable
about it — the weakest of the four because a rounded inset chamber
doesn't read as any particular THING at vault scale, unlike a throne or
a fortress; it's a shape with a locked box in it, not a shape that IS
the joke.

```
###############
#.............#
#..#########..#
#..#.......#..#
#..#..#!#..#..#
#..#.......#..#
#..#########..#
#.............#
###############
```
15x9, solid rectangular border throughout (an earlier draft tried
rounded corners on the outer wall and broke the engine's "solid `#`
border" rule — fixed here to a plain rectangle with the rounding moved
to an inset second ring instead). Kept for completeness of the
propose-and-strike round, not recommended.

---

## 5. NEEDS-ENGINE-LEGEND — consolidated

| glyph | meaning                          | space(s)                | notes |
|-------|-----------------------------------|--------------------------|-------|
| `o`   | cheese wheel (impassable scenery) | SPC_MIDDEN, SPC_GAG_THRONE | legibility risk next to `O` ogre |
| `T`   | tombstone (walkable, decorative)  | SPC_MIDDEN               | carries EPI_003 |
| `%`   | dense-draft wall (vs plain `#`)   | SPC_STAGE                | presentation-only |
| `,`   | tally-mark floor (walkable)       | SPC_STAGE                | legibility risk at small font |
| `&`   | amulet/pedestal in vault ASCII    | SPC_STAGE                | placement-coupling question, not just a glyph — see §2 |
| `V`   | the hole / dungeon mouth          | SPC_OVR_1                 | real plumbing: fixed-content → procedural handoff, light clock starts |
| `+`   | collector's shut door             | SPC_OVR_3                 | doubles as the §3.6 mantel transition once amulet is held |
| `Y`   | the trainer (NPC)                 | SPC_OVR_2                 | stationary talk-target |
| `D`   | the donkey (NPC)                  | SPC_OVR_3                 | stationary talk-target, re-glyphs at buyout (engine ask K) |
| `=`   | fixed screen-link                 | SPC_OVR_1/2/3 edges       | deterministic, distinct from random `*` portal |

Not a new ask, noted for completeness: `?` (sign-up sheet, reused
exactly), `r` (training rats, reused exactly, deliberately not a new
decor-mob category).
