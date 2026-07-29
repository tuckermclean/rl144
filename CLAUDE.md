# CLAUDE.md — rl144

A top-down roguelike in Rust. **Hard constraint: shipped binary + all assets ≤ 1.44MB (1,474,560 bytes) — the capacity of a 3.5" floppy.** Everything else is negotiable; this is not.

## Where this stands (goes stale fast — trust `git log`/`git status` over this)

- **Batches 1–17 merged to `master`.** The road-to-1.0 plan (`docs/design/2026-07-26-road-to-1.0.md`) is the plan of record: **cast → ending-as-exam → polish/ship** (~5–7 batches). Batch 17 (the mimic, cast batch one) shipped the NPC-vault worldgen MAJOR, the talk-minigame framework, sword→Hold + set-down-for-regard, and disguise/ambush.
- **Immediate next action: HUMAN PLAYTESTS THE MIMIC** (road-to-1.0 gate). Only after sign-off (or a tuning follow-up) is cast batch two briefed: the coat (+ first mid-run split-spawn), the lost guy, the tired ones, THE STAGE's dressing, the donkey rung-3 companion (human ruling: IN for 1.0), corpse/tombstone persistence. Frozen manifest: `docs/superpowers/plans/2026-07-27-cast-npc-vault-major.md` §A.
- **Known flag for the human**: batch 17's guaranteed rooms made the game easier (violence 32→40% win rate); the post-merge mimic accept-damage 2→7 tuning pulled it back to ~34% and widened the flip to +9.9, largely undoing the drift — watch on the next playtest. See the batch-17 entry in `.superpowers/sdd/progress.md` / git log.
- **Design canon** (human-locked): `docs/story/STORY-COMPILE-v1.md` + `FLAVOR-DRAFT-v0.md`. §9 checklist: A/B/C/D/E/H/J done; F in progress (mimic done, rest of cast pending); G/I/K not yet briefed — write each brief from the story doc's own §9 entry when its turn comes, don't guess ahead.
- **Explicitly post-1.0** (don't build without re-prioritization): Dest::World multiverse enrichment, scale/journey "B", ghost playback, audio, sprites, wasm/net. `docs/design/monomyth.md` is deferred.
- Open placeholders waiting on a human: story §12.14's second cheese give-target (slot marked in `contractor.rs` — don't guess it); most `[YOURS]` lines in `FLAVOR-DRAFT-v0.md` (replacing one is a same-ID text swap, never structural).
- `SPACES-DRAFT-v0.md`: trainer/donkey dialogue is wired; the 3 overworld screens' map/name/describe strings are still minimal placeholders pending a content-only pass. Midden/Stage vaults unconsumed.
- `~/Documents/gitrepos/golem` (sibling repo) is a source of ported mechanics (sokoban came from it). Check its `games/some-hero` (doors/locks/riddles) and net protocol before designing either from scratch.
- `.superpowers/sdd/progress.md` (gitignored, machine-local) has the task-by-task narrative; this file is the durable git-tracked equivalent.

## Project shape

Single binary crate. Keep this module count; don't fragment further without justification:

- `src/rng.rs` — Rng, h64/channel, fnv (frozen hash primitives)
- `src/content.rs` — generic content helpers + engine UI-chrome palette (`PAL_*` consts)
- `src/gamedef.rs` — the `GameDef` data contract: pure types, zero logic
- `src/games/` — cartridge(s); `games/mod.rs` is the only file allowed to name a game module; `games/contractor.rs` is cartridge #1 (`pub(crate) const GAME: GameDef`)
- `src/game.rs` — engine core: tiles, entities, worldgen, turns, `apply_input`
- `src/headless.rs` — dump/solve/sim/world_hash (verification tooling)
- `src/save.rs` — save format, replay, state_hash
- `src/render.rs` — cell-grid presentation core (80×30 `Cell{ch,fg,bg}`)
- `src/backend_minifb.rs` / `src/backend_term.rs` — the two backends
- `src/main.rs` — arg parsing, backend dispatch, tests

**Dependencies are frozen**: `minifb` + `font8x8` only, both optional, pulled in only by the `backend-minifb` feature (font8x8 uses only the `legacy` module, direct-indexed). Adding a crate requires demonstrating (a) it can't be hand-rolled in <150 lines and (b) the measured size cost. **No engines** (Bevy/ggez/macroquad/SDL disqualified by budget), no `serde`, no `rand`, no `image`. **No asset files** — all content is procedural or `const` data; an `assets/` directory is a design smell.

## Core/crust doctrine

The core (`rng`/`content`/`gamedef`/`games`/`game`/`save`/`headless`/`render`) has **zero platform calls and zero `cfg` blocks** (sole exception: `games/mod.rs` cartridge-selection wiring). Its entire surface to a frontend:

1. `render_cells(&Game, Screen, &mut [Cell])` — the 80×30 cell grid
2. `scene(&Game) -> Vec<SceneEntity>` — derived-only sprite-level data (additive to render_cells)
3. `Game::apply_input(u8)` — the input-byte vocabulary (see Code conventions)
4. The headless entry points (`--dump`/`--solve`/`--sim`/`--replay`/`--render-frame`/…)

Backends may only *consume* that surface. A backend needing a new core API is a real design decision — justify it in the status entry. `cfg` is allowed only in backend modules and `main.rs` dispatch (two `compile_error!`s enforce exactly one of `backend-minifb` (default) / `backend-term`). Terminal build: `cargo build --release --no-default-features --features backend-term --target-dir target/term`.

**Presentation-only exclusion set** (never hashed/dumped/saved): enumerated in ONE place — `state_hash`'s doc comment in `save.rs`. Anything added to the set gets documented there, not scattered. Currently: `killer`, `echo`, `facing`, `fx_hit`, `mcguffin_last_line_turn`, `died_out_of_her_light`, `last_life_bloody`, `last_life_greeting_spoken`, plus the derived render buffer `lit_r`.

Core hashed state stays explicit-width (`i32`/`u32`/`u64`) — no `usize` in anything feeding `state_hash` (32-bit-port prep). `usize` is fine for grid-index/loop plumbing.

## Cartridge doctrine

**The engine is generic; the game is data.** Every game fact (monsters, items, themes, vaults, authored floors, balance, win condition, strings) lives on `GameDef` in `contractor.rs`; engine files reach it only via `crate::games::GAME`.

- **Engine code must stay grep-clean of game nouns.** No `rat`/`goblin`/`ogre`/`cheese`/`sword`/etc. as identifier or string literal in any engine or backend file — checked by grep on every change. Doc comments in plain English are fine; a string that renders or hashes is not. `main.rs`'s test module is exempt. **A leaked game literal in engine code is a real bug** — move it to the appropriate `GameDef` field (usually `StringsDef`).
- A second game = a second `src/games/<name>.rs` shaped like `contractor.rs`; flip the re-export in `games/mod.rs` (cfg permitted there — it's wiring).
- Swapping cartridges is a MAJOR-version event for seeds/saves, same doctrine as a worldgen change.

## Size budget rules

Current ballpark: ~660 KB stripped / ~220 KB UPX-packed under rustc 1.75 (sizes are toolchain-dependent — always report which toolchain measured a number; deltas only compare within one toolchain). The number that counts is the **UPX-packed binary**, per backend, each against the full 1,474,560-byte budget.

After ANY change adding code/data: build release, record stripped size, pack a **copy** with `upx --best --lzma`, report both numbers with delta. Rules:
- >50 KB packed for one feature needs explicit justification.
- Packed >1 MB: stop adding, shrink first (`cargo bloat`).
- Never remove the `Cargo.toml` size flags (`opt-level="z"`, lto, codegen-units=1, panic="abort", strip).
- Never commit packed binaries into `target/`; repo-root `rl144-<platform>-upx` files are release artifacts.

## Verification (headless-first)

No display in CI/containers; the window can't open there. **`make check` is the whole gate**: build, test (both backends), golden cmp, frame cmp, cross-backend `xhash`, `--solve 10000`, `--sim 5000` (all four policies), `--sim-flip`, size. Run it before calling anything done. `make targets` prints the size scoreboard.

- **`--dump --seed N`** — all 5 depths as ASCII; the primary test harness. **`--dump-overworld`** — the 3 authored screens (seed-independent). Legend lives at `headless::level_dump`.
- **`--solve N`** (default 10000) — winnability + difficulty band gate (`tests/solver-band.json`). `--report` = stats-only, the re-baselining flow. Run after ANY worldgen-adjacent change.
- **`--sim N`** (default 5000) — four deterministic bot policies, each gated by its own band file:
  - `greedy` (`tests/sim-band.json`) and `pacifist` (`tests/pacifist-band.json`) — floor-of-competence references (both win <1% now); their bands must not move when other bands change.
  - `tactical` (`tests/tactical-band.json`) and `tactical-pacifist` (`tests/tactical-pacifist-band.json`) — the competent-player instruments. Trust the band files over any prose snapshot of the numbers.
- **`--sim-flip`** (in `make check`) — the mercy arc's ratified central thesis as a relational invariant: `diplomat_win% ≥ violent_win% + margin` (margin/N in `tests/tactical-pacifist-band.json`). It exists because the per-policy bands overlap, so positions alone can't prove the flip. See the arc doc's "Ratification (2026-07-25)".
- **Instrument-parity rule** (named doctrine): when a batch adds a *move to a route* (a new interaction a bot's archetype could take), the bot that models that route must learn the move **in the same batch**, or the post-change band measurement is an artifact. Bot-teaching is priced inside the mechanic's batch, never a follow-on. "The flip is expected to survive — measure it" is the mandatory sign-off line on any band-moving change.
- **`--render-frame --seed N [--ascii]`** (term builds) — frame-golden capture, frozen by `tests/golden/frame_*.bin`.
- **`--replay <file>`** — headless replay + FNV state hash; two replays of one save must hash identically. **`make xhash`** replays `tests/fixtures/ref.sav` through both backends and requires identical hashes — the proof the core/crust seam is real.
- **`Game::new(seed)` vs `Game::new_overworld(seed)`**: `new` is the frozen "start in root dungeon D1" constructor used by every headless surface, golden, and dungeon-direct test. `new_overworld` is the real interactive/replay front door (fresh sessions, `replay()`, backend retry/restart). New call sites: interactive = `new_overworld`; verification = `new`.
- Any new system must be observable via `--dump` or a new headless flag — extend headless modes rather than claiming "it probably works." Non-dumpable logic (combat math, FOV) gets `#[test]`s; tests are free, they don't ship.
- Window-path changes can only be compile-checked headlessly — flag them explicitly as **"needs interactive playtest"**, never report them as verified.

## Seed compatibility (MAJOR-version doctrine)

Worldgen output is a public API, frozen by `tests/golden/` (full dumps, seeds 1/2/3/42/1337). Any change that diffs a golden **map layout** (channel constants, tag scheme, draw order/count on worldgen/spawns/vault/theme channels, room/corridor/placement logic) breaks every seed and save in the wild — that's a **MAJOR bump requiring explicit human sign-off**, never a drive-by. Re-baseline flow after sign-off: regenerate goldens, re-run `--solve 10000`, re-commit `tests/solver-band.json`, re-derive `START_LIGHT` from the new worst-case budget (its comment documents the derivation — keep it current). Dump-format-only diffs aren't seed-breaking but still need goldens regenerated; say so in the commit.

Save-version doctrine (`save.rs`, currently `SAVE_VERSION 11`): bump on **either** input-vocabulary growth **or** hashed-state-shape change. `parse_save` accepts all old versions (each replays byte-identically); the bump's point is that an *old* binary must cleanly reject a *newer* save rather than silently diverge. Don't drop old-version acceptance; don't skip a bump.

## Code conventions

Mechanism details live at their definition sites (doc comments) and in the design docs — reference them, don't duplicate them. The invariants:

- **All randomness flows through named channels** (`channel(seed, &[tags])`). `worldgen`/`spawns`/`vault`/`theme` are per-depth and golden-frozen; `combat`/`ai`/`flavor`/`parley` are per-run streams on `Game`. Channels never leak into each other (there's a test); no system entropy mid-game — the only entropy is the launch seed.
- Coordinates are `i32`; indices via `idx(x,y)`; bounds via `in_map`. One coordinate convention.
- Stats live in const tables on `GameDef`; new content = new table row, not scattered magic numbers.
- Turn structure: player acts (`spend_turn` burns light) → `monsters_act()` → `compute_fov()`. Monsters never act on a level the player just arrived on (the early returns after stair transitions are intentional).
- Light is the run's clock: 1/turn, 2 carrying the objective; 0 = darkness death **unless lit by the McGuffin's shine** (`lit_by_mcguffin`); lose check runs before win check. `START_LIGHT` is solver-derived — don't retune by feel. The overworld is exempt from light burn.
- The win is a round trip: objective on D5, carried to root D1's `<`. Visited levels persist via `LevelState`; never regenerate them.
- Wall bumps cost no turn/light (intentional). Pickup is walk-over.
- Flavor is grounded: lore/adjectives only restate things the engine did — never invent entities, exits, or events.
- Saves are seed + input log (`RL14` header), never serialized world state. Anything making replay diverge from live play is a bug by definition.
- **The 80×30 grid is engine API; the window is presentation.** The grid never follows window size; frontends scale the 640×360 buffer.
- **Input-byte vocabulary** (the platform boundary — any frontend producing these bytes is a valid client): 0–3 move/bump, 4 wait (also portal transit when standing on one), 5 restart-reroll, 6 retry-same-seed (5/6 are reconstruction bytes handled in `replay()`/backends), 7–10 talk-NSWE, 11–14 give-NSWE, 15 use-top, 16 put-down-objective, `17+kind` use-by-kind, `17+len(items)+kind` set-down-by-kind. Chord/selector armed state is frontend-local chrome, never engine state. The title-screen key legend must land in the same commit as any key it documents.
- **Held items** are a LIFO `Vec<u8>` (hashed), no grid UI. `ItemDef::on_pickup` = `Hold` or `Consume` per item. GIVE consults `give_table`; USE applies `on_use`; both are ordinary tax-free turns that no-op gracefully via `StringsDef` lines.
- **Mercy is talk**: talk bytes roll `receptivity()` (the formula + provenance live at its definition in `game.rs` — the one source of truth) on the `parley` channel; becalm at `talk_threshold`; becalmed monsters never attack and yield on bump. `regard`/`calm`/`spared`/`awe`/`yielded`/`struck_player`/`dividend_paid`/`disarm_regard_paid` and friends are hashed — mercy is run-defining. The becalm toolkit is per-creature (ogre: hold ground; goblin: give ground *then talk*, dead once it's struck you; cheese/potion give-rules are read-the-bestiary content) — mechanisms documented at `resolve_awe`/`monsters_act_and_resolve_awe` and in `docs/design/2026-07-22-mercy-economy-arc.md`. Awe reads use the PRE-chase snapshot discipline — don't reintroduce post-chase adjacency reads.
- **Violence is taxed**: `VIOLENCE_TAX` extra light per bump-attack; kills also dim the torch (`kill_light_penalty`) and average `kill_valence` into the McGuffin's mood.
- **Light as grace** (the flip's engine): the carried McGuffin is a second positional light source whose radius = f(mood) via `mood_shine_tiers`; mood is a running average anchored at first pickup from the descent's kill/spare record. Mechanism at `Game::mood()`/`mcguffin_light()` and the batch-12 plan doc.
- **The McGuffin talks** via `CarryEvent` dispatch (`carried_lines` table + `carried_preamble`); empty pool = provable no-op; `StairsUp` is indexed by `speech_attempts`, not drawn. Flavor channel only.
- **Talk-minigame framework** (`MonsterDef::talk_minigame`): `None`/`Echo`/`AnswerSecondVoice` take the unchanged flat roll — only `PoliteDecline` branches (behavior-neutral tagging invariant). The mantel exam (§9-I) reuses this framework + the hashed lesson-state flags (`echo_done`/`answer_done`/`endure_done`).
- **Guaranteed NPC vaults**: `GameDef::required_vaults` + `place_required_vault_room` (exhaustive scan — a bounded retry is NOT a guarantee). Cast depths stamp unconditionally before the optional vault roll; per-depth channels keep non-cast goldens stable.
- **Sword is `Hold`**; ATK derived via `recompute_atk` at every `held` mutation. Set-down-for-regard is guarded once-per-monster (`disarm_regard_paid`) — the anti-farming guard.
- **Portals** (`*`): destination rolled once at gen and cached; walk-on describes (grounded — the engine generates the destination to prove it), wait transits. Light/hp/kills/spared are global across worlds; the win fires only in the root world. **Authored floors** are singular, keyed by `WorldId::Floor(index)`, persistent. Portal-destination floors hold a derived-value `LightCache` reward (derivation via `--probe-floors`, capped per run by `max_cache_light_per_run`).
- **Overworld**: 3 authored screens on `WorldId::Overworld`, reusing depth/stash machinery. `=` screen-link (deterministic, instant), `V` hole into the root dungeon (reuses portal forward-transit), `+` shut door (dumb until §9-I). `passive`/`bump: BumpResponse` make the trainer/donkey un-killable by construction.
- **Sokoban** (vault-only): push chains capped at 2; pit fills, goal locks; blocks hashed; sokoban rooms gate loot only, never the exit (gen excludes their centers from exit selection; the solver proves it).
- Rendering: cells only via `render_cells`; rows `[0,MAP_H)` map / `MAP_H` status / rest log; don't draw outside your band. **Each cell grades by its own light source** (`lit_r`, max-source-wins) — never the frame's torch. Wall autotiling counts only *seen* neighbors. Log lines through `Game::log`, ≤ ~78 chars. **All rendered/authored strings are ASCII-only** — the font is byte-indexed (`put_str`), so non-ASCII corrupts the grid, not just its looks; normalize em-dashes to `--` and drop accents (a same-ID text conform, never structural). Stay disciplined here — the DOS build target is near. Colors are `PAL_*` consts in `content.rs` (theme wall/floor stay on `Theme`).
- `Screen` (Title/Play/End) is core render data; the state machine deciding which is active is backend-owned. Title dismissal logs no input byte.
- **Ghost files** (`RLG1`): pure builders in `save.rs`, backends own I/O; labels only from the const table; auto-written on death; playback is post-1.0 dead code.
- **MSRV 1.75, toolchain is environment-dependent — CHECK it** (`rustup toolchain list`). This container's default rustup toolchain is a real 1.75.0, so `make check` genuinely enforces the floor. Where an env lacks real 1.75, a clean build is NOT proof (newer rustc accepts code 1.75 rejects — E0716 bit three times). `make` recipes run under `/bin/sh` without `~/.cargo/env` — prefix `PATH="$HOME/.cargo/bin:$PATH"` when invoking `make`.

## Platform notes

- Linux: `minifb` with `default-features=false, features=["x11"]`. macOS/Windows need default features — a target-specific flag, not a global change.
- Sandboxed envs: `upx-ucl` and rustc/cargo from Ubuntu apt (rustup's domain often blocked).

## Roadmap context (don't re-litigate)

- **DECISION.md** (`docs/design/proposals/`) governs the blended endgame roadmap; sign-offs already granted are enumerated there — don't re-request them; anything not listed needs fresh sign-off. Phase 3 presentation (sprites/audio) stays deferred behind story mechanics (human, 2026-07-19).
- **Engine direction** (human, 2026-07-18): two win conditions — (a) a ≤1.44MB game, (b) a tiny MMORPG. Networking may return behind a compile flag as lockstep input-sharing over the deterministic replay core (multiplayer = relaying input bytes, never serializing the world). Channel discipline and replay convergence are therefore engine API, not test hygiene. Permanently cut: mod support, config files, localization.
- v0 cuts still queued (rough order): audio synth, procedural sprites, inventory UI, ranged combat/variety.

## Definition of done for any change

1. `cargo build --release` clean (warnings = not clean).
2. `cargo test` green (both feature sets when relevant).
3. `--dump` (and relevant headless modes) eyeballed; `--solve 10000` green after worldgen-adjacent changes; `make check` before calling a batch done.
4. Both size numbers (stripped, packed) reported with delta and toolchain.
5. Anything headlessly unverifiable explicitly flagged for human playtest.
6. If a gameplay change makes a stated invariant in this file false, update this file in the same change.

## Process discipline

- **Subagent dispatch**: Fable/Opus for planning and final whole-branch review; Sonnet for implementer tasks, parallelized when independent. State acceptance numbers up front when knowable; reviewers independently re-run every empirical claim (this has caught real bugs repeatedly). Hitting session limits mid-batch is fine — schedule a wakeup, resume from `git status` + the ledger.
- **New mechanism implied by content work** → write it into the staging doc with an explicit "needs its own sign-off" flag, never quietly build it.
- **Read the canon before pronouncing on it.** A scope call ("post-1.0", "cut", "one batch") is only valid citing the human-locked doc, never memory of it — the road-to-1.0 plan mis-scoped §9-F twice from recollection. Before scoping any §N ask, open §N.
- **Canon precedence auto-resolves upward, silently** (human, 2026-07-27): `STORY-COMPILE-v1.md` > `docs/design/*.md` > plans/briefs. A lower doc contradicting a higher one is a bug in the lower doc — fix it (or record an explicit inline human override).
- **One sign-off package** (human, 2026-07-27): open design questions go inside the single sign-off package as recommendations-with-defaults — never serialized one-question-one-answer pre-rulings.

## History (condensed; full narrative in git log + `.superpowers/sdd/progress.md`)

- **v0.1 / batches 1–3**: channel RNG + goldens, torch clock, round-trip win, save/replay, `--solve`/`--sim` gates, combat balance MAJOR, core/crust split + terminal backend + `xhash`.
- **4–5**: violence tax, save v2 + ghosts, `scene()`, mercy-as-talk (the Henson ruling) + `receptivity()` repricing + pacifist bot gate.
- **6**: variety MAJOR — portals/multi-world/authored floors, sokoban; story canon adopted, non-priced features dropped.
- **7**: cartridge split (the unbraiding), give/use verbs, held-items LIFO.
- **8**: the McGuffin's voice (`CarryEvent`, put-down, climb ladder).
- **9**: overworld skeleton (3 screens, screen-link, hole, trainer/donkey; `new_overworld` front door).
- **10**: tactical/tactical-pacifist bot instruments.
- **11**: the complete ogre (retaliation + stand-tall awe) + heal-scarcity balance MAJOR.
- **12**: light-as-grace (second lantern, mood-driven shine) — THE FLIP: diplomacy above violence, measured.
- **13**: per-creature becalm texture (goblin give-ground, mammal-medicine, cheese, dividend, donkey-follow).
- **14**: portal light-cache ROI + the per-source light-grading render fix.
- **15**: goblin awe = give ground *then talk*, blocked once struck (playtest fix); use-by-kind selector.
- **16**: the flip ratified as a `--sim-flip` relational invariant; diplomat floor ratified; awe_threshold 2→1; real-1.75 toolchain confirmed.
- **17**: the mimic (cast batch one) — NPC-vault MAJOR, talk-minigame framework, sword→Hold + set-down-for-regard, disguise, lesson-state. Flip held, widened to +9.9 after the accept-damage 2→7 playtest tuning; difficulty drift largely addressed. **Awaiting human playtest sign-off before cast batch two.**

New status entries: append a dated one-paragraph entry here (what landed, gate numbers, playtest-pending items); keep the mechanism prose in doc comments and design docs, not in this file.
