# rl144 endgame proposal brief (shared context — read fully before writing)

## The commission (from the human, verbatim intent)

Deliver a **serviceable Peasant's Quest / Undertale fusion on a 1.44MB floppy disk**. SVGA pixel-art style, resizable display, mobile portrait AND landscape, "really whatever we can get away with cheaply that will let this feel like a real, amazing game." Not a story pitch — a **project end-state + plan** proposal: what is on the floppy at v1.0, how the bytes are spent, how the architecture carries it, in what order it gets built, and how each step is verified.

## Hard constraints (non-negotiable, from CLAUDE.md — read it in the repo root)

- UPX-packed binary + all assets ≤ 1,474,560 bytes **per target**. Currently: minifb 192,144 B packed, term 169,724 B. Headroom ≈ 1.28MB — large, but "no asset files" stands: all content is code, const tables, or procedural generation.
- Dependency list frozen (minifb, font8x8, both optional behind backend features). New crate = must be impossible to hand-roll in <150 lines AND size-measured. No engines, no serde, no rand, no image.
- Determinism is engine API: named RNG channels, worldgen frozen by dump goldens, saves are seed+input-log, replay convergence gated by `make xhash` across backends. Any golden diff = MAJOR + human sign-off.
- Core/crust doctrine (new in batch 3): core (rng/content/game/save/headless/render) has zero platform calls, zero cfg. Frontend surface: `render_cells(&Game, Screen, &mut [Cell])` (80×30 cells of {ch:u16, fg:u32, bg:u32}), `apply_input(u8)`, headless entry points. Backends behind mutually-exclusive cargo features. **A backend needing a new core API must justify it** — if your presentation plan needs a richer surface than glyph cells (it will, for sprites), DESIGN that surface extension explicitly and defend it.
- The 80×30 cell grid / input-byte vocabulary (0-5) are the platform boundary. Input vocab can grow via save-format v2 (versioned — a deliberate MAJOR), not drive-by.
- MSRV rustc 1.75. `make check` (build×2, tests×2, dump+frame goldens, xhash, solve 10000, sim 5000, size) green after every batch.
- Permanently cut: mod support, config files, localization.

## Current state (post batch 3, 2026-07-18)

Winnable roguelike, 5 depths, round-trip amulet win, light-as-clock (2000, solver-derived), 3 monster kinds w/ themed skins, 4 themes with slot-filled lore at 3 depth tiers, 3 vaults, room kind/tone lines, sim-gated balance (greedy bot wins 14.6%, band [10,25]%), title/end screens, wall autotiling, light-tier color grading, palette consts, status bars, save/load/replay, daily seed, string seeds, world hash. Two backends: minifb window (640×360 px, resizable, aspect-preserving) and ANSI terminal (dirty-cell, 256-color).

## Findings that constrain design (from the batch-3 ledger)

- `R`/restart REROLLS the seed (`h64(seed,["restart"])`) — "the game remembers your resets in this world" needs a retry-same-seed verb (save v2 decision).
- Darkness deaths are structurally rare (~0.1%) for an efficient player: walking dominates light burn. The one untouched lever: make *choices* (violence?) cost light. A mercy economy would need a second sim-bot policy to keep the winnability gate honest.
- Inventory is deliberately deferred (walk-over pickup is doctrine until a signed-off inventory batch). Fetch-quest/mercy-item designs force this decision — sequence it explicitly.
- The direction memo (2026-07-18): rl144 is an ENGINE, win conditions are (a) ≤1.44MB game, (b) tiny MMORPG. Networking may return as lockstep input-sharing over the replay core. State = replay(seed, input_log); multiplayer = relaying input bytes, never world state.

## Presentation problems every proposal MUST answer concretely

1. **SVGA pixel art with zero asset files**: sprites as const tables and/or procedural composition — give the actual byte math (e.g., 16×16 4bpp sprite = 128 B raw ≈ what packed? × how many sprites? palettes?). What does "SVGA style" mean at our scale — target logical resolution, colors on screen, and how the existing cell grid relates to a sprite layer.
2. **Mobile portrait + landscape** with a landscape-locked 80×30 engine grid: camera/viewport? stacked UI recomposition? letterbox? And input: touch vocabulary mapping to the input bytes. Which backend gets us mobile cheaply (wasm/web? SDL-less android?) without violating the dependency doctrine.
3. **Audio**: the old roadmap reserved 20-30KB for a software synth + tracker-style const song data. Commit to a plan or cut it — argue either way.
4. **Resizable already works in minifb; keep it working** in whatever you propose.

## Deliverable format (write to YOUR assigned file, ≤280 lines, tight prose, no filler)

1. **Thesis** — what "success" means; the one-line review you're engineering for.
2. **The floppy at v1.0** — complete feature list + size ledger (KB per system, sum with ≥20% margin under 1,440KB... note the budget is 1,474,560 B).
3. **Presentation architecture** — the SVGA/sprite/mobile/audio answers above, as concrete engineering (new core surface if needed + its justification under the doctrine).
4. **The fusion design** — how Peasant's Quest (verb comedy, authored screens, absurd deaths) and Undertale (mercy, memory, the game knowing you) are DELIVERED mechanically: verbs, save v2 semantics, endings, writing plan (registers, volume of text, who writes it).
5. **Roadmap** — batches 4..N in order, each with its verification-gate extension (how does `make check` grow to hold this feature?), each with a size checkpoint.
6. **Cuts** — what you refuse to build and why the game is better for it.
7. **Attack surface** — the strongest case AGAINST your proposal, stated honestly.

Ground every claim in the actual repo (read the code where it matters). Do NOT commit anything; write only your one file.
