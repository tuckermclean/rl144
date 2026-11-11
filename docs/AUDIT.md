# AUDIT — rl144 (`f33a80e`, audited 2026-11-10)

> Committed with the adoption spec (SPEC.md) as the evidence behind its plan, at the owner's request.
> `FACTS.md`, `QUESTION-1.md` and the raw run logs it mentions are the adoption run's working files and are not in the repository.

Unattended audit by noodlebot b8d1bdf. Every verdict comes from reading the code at `f33a80e` and running it in `docker.io/library/rust:1.75-slim` (podman, on a copy of the clone). Docs, commit messages, FACTS.md and QUESTION-1.md were treated as claims to check, not as evidence.

## 1. Summary

**Does the code match the docs?**
- **Engine and doctrine: almost entirely.** Of the 91 inventory rows: 58 done, 8 partial, 5 contradicted, 5 stale, 12 missing (nearly all of those are the documented remaining work), 3 other. Of the 57 engine and doctrine rows (2a–2c), only #25 (`--sim` default) is contradicted.
  - Every gate in `make check` is green under real rustc 1.75.0 (§3).
  - Its numbers match the docs exactly: xhash `1f22670bf826471a`, solve worst case 1476 at seed 82, tactical 33.7% vs diplomat 43.5%, packed size ≈ 15% of the floppy.
- **Status lines are partly stale.** "§9-G not yet briefed" is false: G is done. "§9-J done" is false: J is partial. The "second cheese target slot" doesn't exist; that question was answered in batch 13. `--sim` defaults to 1000, not 5000. The camera-console spec still says "no code written" although the console shipped.
- **The 1.0 plan is about a third built.** Manifest §A:
  - done: items 1 (framework, minus the coat's minigame), 2 (the mimic), 3 (sword custody)
  - partial: 5 (D3 and D5 rooms only) and 7 (3 lesson flags only)
  - missing: 4 (coat split-spawn), 6 (donkey rung 3), 8 (corpse persistence)
  - The ending (§9-I/K) and ship (`SHIP.md`, `make ship`, `make floppy`) are not started.

**Three defects no doc mentions** (all confirmed by an independent reviewer; the first also by a run):
1. **The camera console fails on 32-bit, the 486 target it was built for.**
   - The `ioctl` FFI passes the request as `u64`, so on i686 libc receives a NULL pointer.
   - Run: `ioctl(1, TIOCGWINSZ, NULL) = -1 EFAULT`. The frame then addresses rows up to 30 on a 20-row tty.
   - x86_64 works.
2. **Adding any item breaks replay of old saves.** The set-down bytes are numbered from `17 + items.len()`. The planned donkey-treat would change what every v11 set-down byte means.
3. **"Never bump past v11" conflicts with the save doctrine.** The fields v11 was meant to reserve (custody, exit axes, carrier identity, corpse state) don't exist, so cast batch two needs a v12 bump.

**Also:** there is no CI and no README. The build needs `g++`, which the docs don't mention. The committed release binary dates from v0 and won't start on Debian 12. 198 of the 250 flavor-draft lines aren't wired in (rough count), and the overworld still shows "Placeholder ground…" in-game.

**Remaining work, in a suggested order** (S ≈ a day, M ≈ a few days, L ≈ a batch):
1. **S** Fix the `ioctl` declaration (variadic, `c_ulong`). Add an i686 build+test step.
2. **S** Fix the set-down input-byte base at 27, not `17+len`. Add a replay fixture with literal bytes 17–36 pinned to a hash.
3. **S** CI: GitHub Actions on rust:1.75 + `g++` + upx running `make check` (about 25 min).
4. **S** Doc truth pass: CLAUDE.md status and §9 lines, `--sim` default, `master`→`main`, the spec's status, stale comments, build deps, a short README.
5. **M–L** Cast MAJOR 2 (needs owner sign-off): D2/D4 required rooms plus a `SAVE_VERSION` 12 that really reserves the cast and ending fields. One golden regeneration.
6. **L** The coat: `AnswerSecondVoice`, the first mid-run split-spawn, bot parity.
7. **M** The lost guy plus the towel. **M** The tired ones plus THE STAGE dressing.
8. **L** Donkey rung 3: LOVE tier, descent, ogre becalm, the treat. Lands after step 2.
9. **M–L** §9-J back half: corpse, tombstone and inventory persistence, the wake/drag-marks lines (NAR_060–063), the contract screen, trainer quest flags.
10. **L** Ending (§9-I/K): mantel exam, Judge, three exits, win relocation, donkey re-glyph.
11. **M** Content pass (owner-gated): real overworld screens, the rest of the flavor lines, `[YOURS]` slots.
12. **M** Ship: `SHIP.md`, `make ship`, `make floppy`, a release tag, replace or delete the stale root binary.
13. **M** Feel-revision batch after owner playtests (pre-budgeted in road-to-1.0).

Optional or out of scope unless the owner says otherwise: the fbdev backend (L), and DECISION.md's C-complete items (audio, sprites, wasm; later docs made these post-1.0).

**Second look.** Fresh subagents reviewed the least certain verdicts without being shown mine: #41/§5.2 (byte layout), #65/§5.3 (save version), #68–69 and #86 (§9 status, cheese slot) and #74 (the ioctl). All of them agreed, so nothing is marked disputed. #74 was also confirmed by an strace run.

## 2. Claims inventory

Verdicts: **done** (code does it), **partial**, **missing**, **contradicted** (code or another fact says otherwise), **stale** (doc text out of date, no code impact), **disputed** (a second reviewer disagreed; both views given).
Paths are relative to the repo root at origin/main `f33a80e`. "C:" = CLAUDE.md (AGENTS.md is a symlink to it).

### 2a. Repo shape, tooling, commands

| # | Claim | Source | Verdict | Evidence |
|---|---|---|---|---|
| 1 | Single binary crate with the 10 listed modules | C:17-30 | done | `src/` has exactly rng, content, gamedef, games/{mod,contractor}, game, headless, save, render, backend_minifb, backend_term, main |
| 2 | Deps frozen: minifb + font8x8 only, both optional, behind `backend-minifb` | C:32 | done | `Cargo.toml:9-21` |
| 3 | Exactly one backend enforced by two `compile_error!`s | C:43 | done | `src/main.rs:9-13`; run: `cargo check --no-default-features` and both-features both fail with the compile_error (exit 101) |
| 4 | Core has zero `cfg` blocks except `games/mod.rs` | C:36 | done | grep for `cfg` outside `cfg(test)` in core files finds only comments |
| 5 | `games/mod.rs` is the only file naming a game module | C:24 | done | `src/games/mod.rs:14-15`; other `contractor` hits are comments or `main.rs` tests |
| 6 | Engine/backends grep-clean of game nouns | C:53 | done (manual only) | grep of string literals and identifiers in engine and backend files: no game nouns. No test or make target enforces it |
| 7 | All rendered strings are ASCII | C:116 | done | the only non-ASCII literals are the minifb window title (`backend_minifb.rs:203,205`, OS chrome, not the grid) and stderr text in `headless.rs:1135,1185,1271,1292` |
| 8 | Size flags in Cargo.toml (`opt-level="z"`, lto, codegen-units=1, panic=abort, strip) | C:64 | done | `Cargo.toml:26-31` |
| 9 | Size ballpark ~660 KB stripped / ~220 KB packed (rustc 1.75) | C:59 | done | measured 674,160 B stripped / 221,716 B packed (minifb), 522,608 / 200,024 (term), rustc 1.75.0 + upx 4.2.4 --best --lzma: 15% / 13% of 1,474,560 |
| 10 | Repo-root `rl144-<platform>-upx` files are release artifacts | C:65 | stale | the only one, `rl144-linux-x86_64-upx` (166,108 B), was committed in `2c2b84f` (v0 baseline, 2026-07-18) and never updated; 17 batches old; it needs glibc 2.38 and won't start on Debian 12 (glibc 2.36): `GLIBC_2.38 not found` |
| 11 | `make check` = build, test ×2, goldens, frames, xhash, solve 10000, sim 5000 ×4, flip, size (+msrv) | C:69, Makefile:62 | done | `Makefile:62`; ran green: `make check` exit 0 in 2135 s (§3) |
| 12 | `make targets` prints a size scoreboard | C:69 | done | `Makefile:247-285`; ran, exit 0 (§3) |
| 13 | `make` needs `PATH="$HOME/.cargo/bin:$PATH"` | C:119 | done (env note) | true for rustup installs; not needed in the rust image |
| 14 | Build prerequisites | C:121-124 | partial | docs list rustc/cargo + upx only. The default (minifb) build also needs a C++ compiler: on rust:1.75-slim it fails with `cc1plus: not found` until `g++` is installed (minifb compiles `src/native/posix/scalar.cpp`). Debian 12 has no `upx-ucl` package, contrary to the "upx-ucl from apt" note (C:124) |
| 15 | No CI (implicit: "CI MUST install a real 1.75 toolchain…") | Makefile:100-104 | missing | no `.github/`; `gh api …/actions/workflows` → `total_count: 0`; `gh run list` → no runs |
| 16 | `msrv` target pins 1.75 via rustup | Makefile:107-125 | done | ran with rustup present: `msrv: OK (pinned rustc 1.75, both backends)` |
| 17 | No lint beyond `-D warnings` | (FACTS) | done | no rustfmt/clippy config or target. Informational run: `cargo fmt --check` reports 461 diffs (the code isn't rustfmt-formatted); `cargo clippy` has 1 deny-by-default error (`if_same_then_else`, `render.rs:272`, harmless: both arms return `Facing::S`) plus 5 warnings |
| 18 | README | — | missing | the repo has no README; CLAUDE.md is the only overview |
| 19 | Batches 1–17 "merged to `master`" | C:7 | stale | the default branch is `main`; no `master` on the remote (`840c541` merged master into a feature branch; it was later renamed) |
| 20 | Stable release tag "when asked" | 486 note:257 | missing (pending) | `git tag` and `ls-remote --tags` are empty; `Cargo.toml` version 0.1.0 |
| 21 | LICENSE MIT | `fd37a50`, `776a8f8` | done | `LICENSE`, `Cargo.toml:6` |

### 2b. Headless verification surface

| # | Claim | Source | Verdict | Evidence |
|---|---|---|---|---|
| 22 | `--dump --seed N` all 5 depths | C:71 | done | `main.rs:136-139`, `headless::dump`; `goldens: OK (1 2 3 42 1337)` |
| 23 | `--dump-overworld` (3 screens, seed-independent) | C:71 | done | `main.rs:144-147`; ran, exit 0 |
| 24 | `--solve N` default 10000, `--report` | C:72 | done | `main.rs:97-103` |
| 25 | `--sim N` **default 5000** | C:73 | contradicted | `main.rs:117`: `flag_val("--sim").unwrap_or(1000)`. The Makefile passes 5000 explicitly, so the gate is unaffected |
| 26 | Four policies greedy/pacifist/tactical/tactical-pacifist, each with its own band file | C:73-75 | done | `main.rs:110-115`; `tests/*-band.json`; ran, all four inside band (§3) |
| 27 | `--sim-flip` relational gate; a missing band file is a hard failure | C:76, plan §I | done | `headless.rs:1249-1300` (`exit(1)` at the 1271/1295 region) |
| 28 | `--render-frame --seed N [--ascii]`, term only | C:78 | done | `main.rs:157-161`; `frames: OK (1 42, +ascii)` |
| 29 | `--replay` + `make xhash` cross-backend hash | C:79 | done | `main.rs:162-177`, `Makefile:183-194`; `xhash: OK (1f22670bf826471a)`, the same hash on i686-musl |
| 30 | `--probe-floors` | C:113 | done | `main.rs:151-154`; ran, exit 0 (floor 0 cost 12/value 21, floor 1 cost 14/value 21) |
| 31 | `Game::new` frozen for headless use; `new_overworld` is the interactive front door | C:80 | done | `game.rs:915`; `main.rs:197`; `save.rs` replay |
| 32 | Undocumented CLI: `--daily`, `--load`, string seeds (`--seed swordfish`) | — | undocumented | `main.rs:57-90,180-197`; none appears in CLAUDE.md |
| 33 | Goldens seeds 1/2/3/42/1337; frame goldens 1, 42, 1+ascii | C:86 | done | `Makefile:53-56`, `tests/golden/` |
| 34 | Solver band re-committed after a worldgen MAJOR | C:86, mimic plan T1 | partial | `tests/solver-band.json` was last touched on 2026-07-18 (`5d7a285`); batch 17 says the bands still held and documents the new worst case in a comment instead (`contractor.rs:1162-1176`). The band file's own comment still says "exits in BFS-deepest room", no longer true for D5 |
| 35 | `START_LIGHT` solver-derived, comment kept current | C:86,98 | done | `contractor.rs:1162-1184` (worst 1476 / seed 82, start 2000) |

### 2c. Engine and doctrine invariants

| # | Claim | Source | Verdict | Evidence |
|---|---|---|---|---|
| 36 | Core surface: `render_cells`, `scene()`, `apply_input`, headless entry points | C:38-41 | partial | `render.rs:416`, `game.rs:4482` are live. `scene()` (`render.rs:355`) has `#[allow(dead_code)]` and **no non-test caller**; neither backend consumes it (`render.rs:281-289`) |
| 37 | Presentation-only exclusion set listed in one place (`state_hash` doc) | C:45 | done | `save.rs:157-176` lists the same 8 fields. Also unhashed but not listed: `seen`, `vis`, `rooms`, `room_meta`, `room_visited`, `msgs` (derived or regenerated; `room_visited` gates a flavor draw, `game.rs:2468`) |
| 38 | No `usize` in hashed state | C:47 | done | `save.rs:300-460` hashes explicit-width casts only |
| 39 | Named RNG channels; isolation is tested | C:94 | done | tests `worldgen_isolated_from_combat`, `parley_isolated_from_combat`, `cheese_give_isolated_from_combat_and_ai`, `carry_event_only_touches_flavor_rng_and_the_log` in `main.rs` |
| 40 | Saves = `RL14` + version + seed + input log; `SAVE_VERSION 11`; `parse_save` accepts 1..=11 | C:88,102 | done | `save.rs:71-100` |
| 41 | Input bytes 0–16, `17+kind` use-by-kind, `17+len(items)+kind` set-down-by-kind | C:104 | done, with a hazard | `game.rs:4482-4508`. The set-down range starts at `17+GAME.items.len()`, so adding any item (the planned donkey-treat, manifest §A.6) moves every set-down byte. Old v11 saves would then replay differently. Not documented or tested (see §5) |
| 42 | Held items are a LIFO `Vec<u8>`, hashed; Hold/Consume; give_table; on_use | C:105 | done | `game.rs:` `held`; `save.rs:368-372`; `gamedef.rs:426-440` |
| 43 | Mercy = talk via `receptivity()` on the parley channel; hashed mercy fields | C:106 | done | `game.rs:442`; `save.rs:385-399` |
| 44 | Violence tax, `kill_light_penalty`, `kill_valence` | C:107 | done | `contractor.rs` BALANCE `violence_tax: 1`, `game.rs:2566-2585` |
| 45 | Light as grace: `mood_shine_tiers`, `Game::mood()`, `mcguffin_light()`, `lit_by_mcguffin` | C:98,108 | done | `game.rs:77-84,1577,1785` |
| 46 | McGuffin talks via `CarryEvent`, `carried_lines`, `carried_preamble`, `speech_attempts` | C:109 | done | `gamedef.rs:168`; `game.rs:2448-2457` |
| 47 | Talk-minigame framework: only `PoliteDecline` branches | C:110 | done | `game.rs:2894-2903`; test `talk_at_a_none_minigame_monster_is_unchanged` |
| 48 | Lesson flags `echo_done`/`answer_done`/`endure_done`, hashed | C:110 | done (answer reserved) | `game.rs:710-737`; `save.rs:345-351`; `answer_done` can't be set until a coat row exists |
| 49 | Guaranteed NPC vaults with an exhaustive scan | C:111 | done (D3, D5 only) | `game.rs:1056-1066,1392`; `contractor.rs:1083-1093` (`REQUIRED_VAULTS` = D3 mimic, D5 STAGE; D2/D4 empty) |
| 50 | Sword is Hold; `recompute_atk`; once-per-monster `disarm_regard_paid` | C:112 | done | `contractor.rs:636-660`; `game.rs:3734` |
| 51 | Portals, authored floors, `LightCache`, `max_cache_light_per_run` | C:113 | done | `gamedef.rs:745-754`; `save.rs:199` |
| 52 | Overworld: 3 screens, `=`, `V`, `+`, un-killable trainer/donkey | C:114 | done (placeholder content) | `contractor.rs:1510-1540`. All three screens are 11×5 placeholders whose in-game description reads "Placeholder ground. The real overworld lands in batch 9 T2." |
| 53 | Sokoban: push chain capped at 2, blocks hashed | C:115 | done | `game.rs:196-200`; `save.rs:404-409` |
| 54 | Per-cell light grading (`lit_r`), `PAL_*` palette | C:116 | done | `render.rs`, `content.rs` |
| 55 | `Screen` Title/Play/End is core data; the backend owns the state machine | C:117 | done | `render.rs:47` |
| 56 | Ghost files `RLG1`, auto-written on death, no playback | C:118 | done | `save.rs:458-560`; `backend_term.rs:957-983`, `backend_minifb.rs:527-568` (dead runs only) |
| 57 | Donkey follows when calm (rung 2) | arc doc:150-157 | done | `contractor.rs:523` (`follows_when_calm: true`) |

### 2d. Status and plan claims (batch 17 and after)

| # | Claim | Source | Verdict | Evidence |
|---|---|---|---|---|
| 58 | Batch 17 shipped: NPC-vault MAJOR, minigame framework, sword Hold + set-down, disguise/ambush | C:7, manifest §H.1 | done | commits `4ea78bf`…`03eaa89`; `contractor.rs:544-585` (mimic row: `PoliteDecline`, `starts_disguised`, `climb_reencounter`) |
| 59 | Put-down is kind-routed (amendment A) | manifest §E | done | `game.rs:4502-4506`, `put_down_kind` `game.rs:3315-3371` |
| 60 | Mimic disguises as a chest-like glyph, never `?` (amendment B) | manifest:13-14 | done | glyph `c` (`contractor.rs:547`) |
| 61 | Dump test asserts cast glyphs at canon depths (amendment C) | manifest:14-15 | done | `main.rs:389-403` `golden_dumps_show_the_cast_glyphs` |
| 62 | "`--solve` asserts STAGE + cast rooms exist across full N" | manifest §B | partial | `--solve` only fails if D5 has no objective (`headless.rs:86-88`); the D3 mimic room is asserted only by test `required_cast_vaults_placed_on_every_seed` over seeds 0..2000 (`main.rs:361-385`), not 10000 |
| 63 | ECHO verified expressible, documented | manifest §A.1, default 4 | done | `gamedef.rs:373-377` (Echo == None mechanically); rat row tags `Echo` |
| 64 | Instrument parity: diplomat bots learn polite-decline, answer-second-voice and set-down-for-regard | manifest §D | partial | `headless.rs:438-450` `diplomat_decline_byte` (polite-decline only). No bot ever sets the sword down. `03eaa89` says set-down "was evaluated and deliberately not taught", which departs from §D's "non-negotiable" rule. Answer-second-voice has nothing to teach yet |
| 65 | `SAVE_VERSION 11` reserves every field cast batch two and the ending need ("never re-bumps") | manifest §A.7, §G | partial | only three lesson bools exist (`game.rs:710-737`). No custody-honored flag, run-log exit axes, carrier-identity field (story §10.2) or corpse-persistence state. Cast batch two and the ending will add hashed state, so a `SAVE_VERSION` 12 bump is near-certain. Also `save.rs:66-70` says to extend `state_hash` "under this same 11", which would let a batch-17 binary accept a newer v11 save and silently diverge, the case CLAUDE.md:88 forbids. Second reviewer agrees |
| 66 | Human playtest of the mimic is the gate before cast batch two; "awaiting sign-off" | C:8,164 | contradicted (by history) | `5d9466d` "tune … (human playtest)" shows a playtest happened; no sign-off is recorded anywhere |
| 67 | Known flag: violence 32→40% after batch 17, ~34% after tuning; flip +9.9 | C:9 | done | measured tactical 33.7% (1684/5000), tactical-pacifist 43.5%, flip margin +9.9 (required 3); greedy 0.2%, pacifist 0.1% |
| 68 | §9 checklist "A/B/C/D/E/H/J done; F in progress; G/I/K not yet briefed" | C:10 | contradicted (G), partial (J) | **G is done**: the potion→rat give gives +3 regard and a full heal (`contractor.rs:822-832`), and the sword set-down gives `disarm_regard: 3` (`contractor.rs:657`, `game.rs:3412-3426`). **J is partial**: done are the fixed map (placeholder content), no light clock (`game.rs:1674-1676`), the silent kill counter (`game.rs:2572`) and the donkey talk-target (`contractor.rs:500-504`). Missing are trainer quest flags, the contract screen (`render.rs:47-51` has only Title/Play/End), the NAR_060–063 wake/drag-marks lines, and corpse/tombstone/inventory persistence (`new_overworld` resets `held`). I/K: missing (#84-85). Second reviewer agrees |
| 69 | §12.14 "second cheese give-target … slot marked in contractor.rs" | C:12 | contradicted | story §12.14 (`STORY-COMPILE-v1.md:579`) asks for one cheese target. It was assigned to goblins in batch 13 (`contractor.rs:865-890`; arc doc :234-249); there is also a cheese→rat −2 row (`:804-814`). No unfilled slot is marked; the only open give-targets are COAT/TOWEL (§12.8/§12.10). Second reviewer agrees |
| 70 | Most `[YOURS]` lines in FLAVOR-DRAFT-v0 await a human | C:12 | done (wording off) | FLAVOR-DRAFT has no `[YOURS]` markers; its header calls every line a placeholder. 52 of its 250 lines appear verbatim in `src/` (rough measure, after em-dash→`--`) |
| 71 | SPACES-DRAFT: trainer/donkey dialogue wired; overworld strings placeholders; "Midden/Stage vaults unconsumed" | C:13 | partial / stale | overworld placeholders: true (#52). THE STAGE *is* consumed (`contractor.rs:1088`, a plain 11×5 room, not SPC_STAGE's map). No midden vault exists in the code (`grep -i midden` hits only a comment at `contractor.rs:704`) |
| 72 | Sibling repo `~/Documents/gitrepos/golem`; `.superpowers/sdd/progress.md` | C:14-15 | not checkable | machine-local, not in the repo (`.gitignore:3`) |
| 73 | Camera console design "awaiting human approval… No code has been written" | spec:3-7 | stale | implemented in `f33a80e` (`backend_term.rs:69-100,476-580,746`); the commit says D1a/D2a/D3a approved |
| 74 | Camera console: TIOCGWINSZ, dead-zone viewport, pinned status, `Game::camera_focus`, identity at ≥80×30, goldens unchanged | spec, `f33a80e` | partial (broken on 32-bit) | works on x86_64: the pty run shows `ioctl(1, TIOCGWINSZ, {20,40}) = 0`, and the frame addresses nothing past row 20. **Broken on i686-musl**: `backend_term.rs:80` declares `request: u64` for a variadic `unsigned long`/`int` C parameter, so on cdecl the pointer arrives as NULL. strace shows `ioctl(1, TIOCGWINSZ, NULL) = -1 EFAULT`, and the frame addresses rows up to 30 on a 20-row tty. The camera never engages on the 486 it was built for. Second reviewer agrees independently (high confidence). The 11 unit tests pass because they test only the pure functions. Also, on non-play screens a small console crops the top-left, so the centered title shows only "rl1" at 40 columns |
| 75 | 486 phase 1: i686-musl hash `1f22670bf826471a`, 5 goldens match, solve 82/0, tests 258/258 | 486 note:169-174 | done (count updated) | re-run on rustc 1.75.0 i686-unknown-linux-musl: replay hash `1f22670bf826471a` (same as x86_64), all 5 goldens match, `--solve 2000` 0 unwinnable, `cargo test` **269/269**. Since the note was written, `f33a80e` added 11 tests, which explains 258→269 |
| 76 | termios struct comment corrected | `776a8f8` | done | `backend_term.rs:34-38` |
| 77 | i486 target JSON is reference material, not wired into make | 486 note:140-143 | done | `docs/design/486/i486-unknown-linux-musl.json`; no Makefile reference |
| 78 | Plan checkboxes | docs/superpowers/plans/* | stale | shipped batches still show `- [ ]` (mimic plan: 34 unchecked, 0 checked). Checkboxes say nothing about status; trust commits |

### 2e. Remaining work named by the docs

| # | Item | Source | Verdict | Evidence |
|---|---|---|---|---|
| 79 | Coat + `AnswerSecondVoice` + first mid-run split-spawn (`coat-split` channel) | manifest §A.4, §H.3 | missing | `Minigame::AnswerSecondVoice` is `#[allow(dead_code)]` (`gamedef.rs:383-384`); `COAT` is a dead-code item index (`contractor.rs:80-81`) with no give row; no spawn code (`Monster::spawn` is `#[allow(dead_code)]`, `game.rs:396-397`); no `coat-split` channel |
| 80 | Lost guy (D4) + towel receipt | manifest §A.5, story §4 | missing | `TOWEL` is dead-code (`contractor.rs:82-83`); `REQUIRED_VAULTS` has no D4 entry |
| 81 | Tired ones (D5) + THE STAGE dressing | manifest §A.5 | missing | STAGE is a bare room with a pedestal (`contractor.rs:1083-1087`) |
| 82 | Donkey rung 3: LOVE tier, descends the hole, presence becalms ogres, donkey-treat item | manifest §A.6 | missing | `follows_when_calm` exists; no LOVE tier, no treat item, no descent. Road-to-1.0:51-54 lists rung 3 as post-1.0; CLAUDE.md:8 and manifest §A.6 say IN (later human ruling) |
| 83 | Corpse / tombstone / inventory persistence on retry | manifest §A.8, story §9-J | missing | no tombstone code; `corpse` appears only in a comment (`game.rs:3528`) and a test |
| 84 | Mantel exam + Judge + three exits; win relocation | road-to-1.0, story §9-I | missing | `mantel` appears only in comments; the win is still the root D1 `<` |
| 85 | Donkey re-glyph at buyout | story §9-K | missing | no code |
| 86 | Potion-gift regard event | story §9-G | done | potion→rat give: +3 regard and a full heal (`contractor.rs:822-832`); the goblin enrage variant is the batch-13 mammal-medicine rule |
| 87 | `SHIP.md` + `make ship`; `make floppy` image | road-to-1.0:122-127 | missing | no SHIP.md; no such Makefile targets |
| 88 | Pre-budgeted feel-revision batch | road-to-1.0:116-120 | pending | process item |
| 89 | fbdev backend ("phase two") | 486 note:305-308 | missing (out of scope?) | no fbdev code |
| 90 | DECISION.md "C-complete guarantee": sprite pipeline, animation, particles, full audio, `backend-wasm`, `--dump-audio` | DECISION.md:12-19 | contradicted (by later docs) / mostly missing | only `scene()` (unconsumed) and minifb's hit-flash/squash (`backend_minifb.rs:88-135`) exist. Road-to-1.0:51-54, the manifest and CLAUDE.md:11 make audio, sprites and wasm post-1.0 |
| 91 | Post-1.0 list (ghost playback, Dest::World enrichment, scale/journey B, monomyth) | C:11 | correctly absent | `monomyth-batch-brief.md:1` marked DEFERRED; ghost playback absent |

## 3. Run results

**Environment.**
- podman, `docker.io/library/rust:1.75-slim`: rustc 1.75.0 (82e1608df), cargo 1.75.0, rustup 1.26.0 with the 1.75.0 toolchain as default.
- Mounted a fresh copy of the clone at `f33a80e`, never the clone itself.
- Installed inside the container only: `make`, `g++`, `xz-utils`, `wget` (apt), upx 4.2.4 (the upstream release tarball, since Debian 12 has no `upx-ucl`), and the rustfmt/clippy components for the informational lint run.
- Nothing was installed on the host.

**Two setup failures before the first green build.**
1. The plain image has no `make` and no upx: every target exited 127.
2. With `make` installed, `make build` failed (exit 2): minifb's build script needs `cc1plus`, i.e. `g++`, which no doc mentions.

### x86_64 gate (`make <target>` one by one, then `make check`)

| Command | Exit | Time | Result |
|---|---|---|---|
| `make build` (`-D warnings`, minifb) | 0 | 55 s | clean, no warnings |
| `make test` (minifb) | 0 | 811 s | **254 passed, 0 failed**, 0 ignored |
| `make build-term` | 0 | 10 s | clean |
| `make test-term` | 0 | 532 s | **269 passed, 0 failed**, 0 ignored |
| `make goldens` | 0 | <1 s | `goldens: OK (1 2 3 42 1337)` |
| `make frames` | 0 | <1 s | `frames: OK (1 42, +ascii)` |
| `make xhash` | 0 | 1 s | `xhash: OK (1f22670bf826471a)` |
| `make solve` (10000) | 0 | 136 s | 0 unwinnable; min 537 / p50 885 / p90 1044 / p99 1215 / max 1476 (worst seed 82); band OK |
| `make sim` (5000 ×4) | 0 | 669 s | all four inside band (table below) |
| `make flip` (5000) | 0 | 481 s | diplomat 43.5 vs violent 33.7, margin **+9.9** ≥ 3 |
| `make size` | 0 | 1 s | stripped 674,160 B, packed 221,716 B (budget 1,474,560) |
| `make msrv` | 0 | 18 s | `msrv: OK (pinned rustc 1.75, both backends)` |
| `make targets` | 0 | 22 s | minifb 674,160 / 221,716 (15%); term 522,608 / 200,024 (13%) |
| **`make check`** (all of the above) | **0** | 2135 s | green end to end |
| `cargo check --no-default-features` | 101 | — | the intended `compile_error!` |
| `cargo check --features backend-term` (both) | 101 | — | the intended `compile_error!` |
| `cargo fmt --check` (not a repo gate) | 1 | — | 461 formatting diffs; the repo doesn't use rustfmt |
| `cargo clippy --release --all-targets` (not a repo gate) | 101 | — | 1 deny-level `if_same_then_else` (`render.rs:272`, harmless) + 5 warnings (`game.rs:4262`, `headless.rs:781`, `render.rs:166`, `save.rs:391,405`) |
| `--dump-overworld`, `--probe-floors`, `--solve 200 --report` | 0 | — | outputs as documented |
| unknown flag (`--help`) | 134 | — | no such flag: it launches the minifb window, and with no X11 it panics at `backend_minifb.rs:219` |
| committed `rl144-linux-x86_64-upx --dump --seed 1` | 1 | — | `GLIBC_2.38 not found`: the v0 binary won't start on Debian 12 |

Sim policies (N=5000; band files `tests/*-band.json`):

| Policy | Wins | Win % | Combat deaths | Dark deaths | Stuck |
|---|---|---|---|---|---|
| greedy | 10 | 0.2 | 4986 | 4 | 0 |
| pacifist | 3 | 0.1 | 4992 | 5 | 0 |
| tactical | 1684 | 33.7 | 3037 | 279 | 0 |
| tactical-pacifist | 2177 | 43.5 | 2816 | 7 | 0 |

These match the last recorded numbers (`5d9466d`: 33.7 / 43.5, +9.9) and CLAUDE.md's size ballpark (~660 KB / ~220 KB).

### 32-bit (i686-unknown-linux-musl, rustc 1.75.0 + `gcc-multilib`), checking the 486 note and the camera console

| Command | Exit | Result |
|---|---|---|
| `cargo test --target i686-unknown-linux-musl --no-default-features --features backend-term` | 0 | **269 passed, 0 failed** (964 s) |
| `cargo build --release` (i686, term) | 0 | clean |
| `--replay tests/fixtures/ref.sav` (i686 vs x86_64) | 0 | both `1f22670bf826471a` |
| `--dump --seed {1,2,3,42,1337}` vs goldens | 0 ×5 | byte-identical |
| `--solve 2000` | 0 | 0 unwinnable, band OK |
| interactive run in a 40×20 pty (`podman run -t`, `stty rows 20 cols 40`, strace) — x86_64 | 0 | `ioctl(1, TIOCGWINSZ, {ws_row=20, ws_col=40}) = 0`; highest row addressed 20 → **camera works** |
| same — i686 | 0 | `ioctl(1, TIOCGWINSZ, NULL) = -1 EFAULT`; highest row addressed **30** on a 20-row tty → **camera never engages** |

Raw logs: kept with the adoption run (x86_64; i686, pty, strace), not committed.

Not run: interactive play in a real X11 window or on a real 486 console (no display or hardware here). The camera result above is the closest headless substitute. The `make check` pass was at upx 4.2.4 instead of Debian's `upx-ucl`; packed sizes may differ slightly by upx version.

### CI comparison

`gh run list --limit 10` on the repository returns nothing. The Actions workflows API (`GET /repos/<owner>/<repo>/actions/workflows`) returns `total_count: 0`, and there is no `.github/` directory. The repo has never run CI. The only earlier results are in commit messages, which this run confirms: `03eaa89`/`5d9466d` "tests 254/258" (now 254/269 after the 11 camera tests), xhash `1f22670bf826471a`, term 522,608/200,024.

## 4. Corrections to FACTS.md

- "No Rust toolchain… not run": the audit ran everything above. `make check` is green.
- Build prerequisites: add `g++` (minifb's C++ shim). Debian 12 lacks `upx-ucl`; use the upstream upx release.
- "Human-only content: … cheese give-target §12.14 — don't guess": already resolved (goblin, batch 13). Not open.
- "486 note: 258/258 tests": now 269 (term) / 254 (minifb).
- The camera console isn't just "needs interactive playtest". Its 32-bit FFI is wrong (§5).
- `§9-G "not yet briefed"` per CLAUDE.md: G is done. `§9-J "done"`: J is partial (#68).
- "Inline tests … headless.rs (1)": that hit was a comment. There are 270 `#[test]`s (main 245, backend_term 16, render 8, minifb 1).
- Donkey rung 3: the later signed ruling (manifest, 2026-07-27) puts it IN, overriding road-to-1.0's post-1.0 line. Not a conflict the owner must resolve.

## 5. Undocumented behaviour, latent defects, dead or half-wired code

### Defects and hazards (not in any doc)

1. **The camera console doesn't work on 32-bit, the target it was built for.**
   - `backend_term.rs:80` declares `fn ioctl(fd: i32, request: u64, argp: *mut Winsize)`. The C prototype is variadic: `unsigned long` request in glibc, `int` in musl.
   - On i386 cdecl the `u64` takes two stack slots, so libc reads the zero high half as `argp`. The ioctl then fails (EFAULT), `term_size()` always returns `None` (`:92-96`), and `show()` falls back to 80×30 (`:746`).
   - On the 486's 80×25 console the frame therefore keeps the old wrap/clamp corruption, and the status row is never visible.
   - x86_64 works, because the request travels in a register.
   - Unit tests can't catch this: the pure `viewport` is tested and works. Only the FFI is wrong.
   - Confirmed by an independent reviewer (high confidence) and by the i686 strace run in §3.
   - Fix: a variadic `c_ulong` declaration (size S).
2. **The input-byte layout moves when an item is added.**
   - Set-down-by-kind bytes start at `17 + GAME.items.len()` (`game.rs:4502-4506`).
   - The planned donkey-treat item (manifest §A.6) would turn every v11 set-down byte into a different action. Byte 27, "set down potion", becomes "use the treat", which costs no turn, so replay diverges.
   - The version stays 11, because §G says never re-bump, so `parse_save` can't tell old logs from new.
   - No test pins literal bytes ≥ 27: `v10_save_replays_under_v11_parsing` uses no byte ≥ 17, and `ref.sav` holds only bytes 1, 3 and 4.
   - Confirmed independently (high confidence). It must be fixed before any new item lands (S).
3. **The save-version plan conflicts with the doctrine.**
   - `save.rs:66-70` and manifest §G say later cast and ending state "extends `state_hash` under this same 11".
   - Version 11 has shipped (batch 17). Changing what v11 hashes without a bump is exactly the silent-divergence case CLAUDE.md:88 forbids.
   - The fields the bump was meant to reserve (custody-honored, exit axes, carrier identity, corpse state) don't exist. A v12 bump is needed.
4. `room_visited` is run-affecting (it gates a `flavor_rng` draw, `game.rs:2468-2473`) but isn't hashed or listed in the exclusion set. It is low risk, because the flavor RNG state that *is* hashed captures its effect after the fact.

### Dead or reserved code (`#[allow(dead_code)]`, 24 sites)

- `render::scene()` + `SceneEntity`/`SpriteKind`/`monster_facing`/`anim_phase` (`render.rs:281-355`): the "core surface #2" in CLAUDE.md has no consumer. It is reserved for a post-1.0 sprite backend.
- `save::Ghost`, `parse_ghost` (`save.rs:504,550`): ghost playback is post-1.0. Ghost *writing* is live.
- `Minigame::AnswerSecondVoice` (`gamedef.rs:383`), `Game::answer_done` (never set), `COAT`, `TOWEL` (`contractor.rs:80-83`), `Monster::spawn` (`game.rs:396`): stubs reserved for cast batch two.
- `TRAINER`, `DONKEY`, `MIMIC`, `LIGHT_CACHE` consts: placed by glyph and only read by tests.
- `Game::camera_focus` (`game.rs:3295`): live in term builds only.
- `ItemDef` Consume + AtkBonus arm in `Game::pickup`: unreachable for this cartridge since the sword became Hold (`contractor.rs:636-647` comment).

### Stale code comments

- `contractor.rs:43-55`: MIMIC is called "a PLAIN, minimal MonsterDef row … disguise/ambush … T3/T4's scope". T3/T4 shipped, and the row has `PoliteDecline`, `starts_disguised` and `climb_reencounter`.
- `contractor.rs:588-592`: the potion comment says "same amount (8)", but the row is `Heal(3)` (`:632`).
- `tests/solver-band.json` comment: "exits in BFS-deepest room", no longer true on D5.
- `backend_term.rs:83`: "0x5413 on Linux across archs". True only for x86 and ARM-like arches.

### Undocumented features

- `--daily` (a shared seed of the day plus a log line), `--load <file>`, and string seeds (`--seed swordfish` → hashed): `main.rs:57-90,180-197`.
- F5 save (double-press) and autosave on quit: described only in the batch-3 plan, not in CLAUDE.md.
- The minifb window title shows the daily number or seed (`backend_minifb.rs:203-205`).
- The repo-root binary `rl144-linux-x86_64-upx` is from v0 and needs glibc ≥ 2.38.
