# Batch 7 — the cartridge split + the story's first asks

*Human direction 2026-07-19: "none of the specific game stuff is integral engine stuff. The
engine is all the primitives for every game... this one should be a good show of everything it
can do. Ideally, each other game is a YAML file or equivalent." Interpretation (flagged in
session): literal runtime YAML conflicts with standing doctrine (no serde/new crates; config
files permanently cut), so "or equivalent" = a `GameDef` pure-data structure — one def file per
game, engine consumes GameDef only. The def shape is deliberately deserializer-ready if a
hand-rolled text loader is ever un-cut.*

## Doctrine (new, goes in CLAUDE.md at T3)

- `src/engine/` (or equivalent module set): primitives only — grid/idx/COLS, channels/rng,
  FOV/LOS, turn structure, replay/save/state_hash, worldgen ALGORITHM, portals/multi-world,
  sokoban, talk/regard/receptivity FRAMEWORK, scene/cells surfaces, backends, sim/solve
  harness. Zero references to rats, cheese, amulets, torches-as-fiction, or any string of
  game prose. Grep-clean: no game noun may appear in engine code.
- `src/games/contractor.rs` (name TBD by implementer): ONE `pub const GAME: GameDef` — every
  monster kind (stats, glyphs, talk tables, receptivity bases, thresholds), item kind
  (effects as data: heal amounts, atk bonuses, give-targets), theme, vault, authored floor,
  balance constant (START_LIGHT, VIOLENCE_TAX, spawn tables, HP progression), win-condition
  parameters (objective item, return depth, carry burn), and every player-facing string
  (flavor draft IDs live here).
- GameDef versioning: the def participates in world identity — deriving channels stays
  keyed off the seed exactly as today (the def does not enter the hash mechanically this
  batch; goldens freeze def+algorithm together as they always have).
- A second game = a second def file + a feature/binary selecting it. (No second game ships
  this batch; the SHAPE must make one obviously possible.)

## Tasks

**T1 — the unbraiding (pure refactor, byte-identical).** Restructure so the engine consumes
GameDef for everything currently hardcoded. Acceptance = the gates themselves: dump goldens
byte-identical, frame goldens byte-identical, xhash unchanged (d92834c3cd14adb2), --solve
identical stats, both sims identical lines (greedy 953/15, pacifist 631/16), sizes reported
(small growth from indirection acceptable, ≤ +6 KB packed). Engine grep-clean of game nouns
(document the grep in the report). No behavior change of ANY kind.

**T2 — give/use verb (engine primitive) + cheese/coat/towel (def data), story §9-A.**
Input byte 11 = GIVE (directional, chord g+dir, mirrors talk bytes; save v3 vocabulary grows —
old logs unaffected, test it) and byte 12 = USE (self-apply held consumable, LIFO — §9-A's
verb; A's minimal-inventory: a small held-items list, hashed, no grid UI). Engine: give-verb
framework — giving item I to adjacent entity E consults the def's give-table (regard deltas,
line IDs, transformation effects as data). Def rows: CHEESE (rat regard PENALTY [TUNE -2],
burnable via USE for a light flicker [TUNE +8], works on ONE other monster — mimic [TUNE],
per §12.14 placeholder until human writes the line), COAT (give to the coat-monster… the coat
MONSTER doesn't exist yet — the give-table supports a no-valid-target state gracefully; coat
lands with the D2 cast later; ship the item + narrator line only), TOWEL (same: item + line;
lost guy arrives with §9-J). POTION gains give-ability (biggest regard event [TUNE +3
regard]); SWORD gains set-down via GIVE-at-empty-tile? NO — sword set-down is §9-G, defer.
Sim policies unchanged (bots never emit 11/12 — assert). Bands must not move.

**T3 — docs + status.** CLAUDE.md: cartridge doctrine, vocab 0-12, GameDef map; status entry
with the identity-proof numbers (goldens/xhash unchanged through T1) + T2 additions; ledger.
Also fix the two carried minors (portal-in-sokoban-vault doc note; instantiate_floor hard
error on unknown glyphs) — batch-7 owns those files now.

## Gates & constraints

House process (implementer → reviewer → fix → final whole-branch review); FOREGROUND-only
gate runs (every implementer prompt carries this); full `make check` green after every task
(T1: byte-identical everything; T2: goldens/solve/sims unchanged since gameplay additions are
verb-gated and bots never use them). rustc 1.75; zero new crates; no cfg in core/engine;
story Rule 4 binds (nothing beyond §9-A ships); flavor lines used come from FLAVOR-DRAFT-v0
IDs verbatim where they exist (NAR_032/033/035/036/037 etc.); grounding doctrine on any new
line. Budget ≤ +10 KB packed total.
