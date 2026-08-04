# Boot rl144 on a real 486 — scope discovery + phase-1 proof

**Status:** phase-1 de-risk COMPLETE and empirically proven (2026-08-04). This is a
scoping/handover note, not a build-me-now spec. It draws the line between what
**rl144 owns** and what the **linux-live-iso-factory ("the Monolith")** owns, and it
records what was actually run tonight so the hard question ("can Rust target a real
486?") is answered with measurements, not hope.

Companion file: [`i486-unknown-linux-musl.json`](./i486-unknown-linux-musl.json) — the
custom target spec, ready to hand to the factory. It is **reference material**, not wired
into rl144's Makefile or `make check` (see "Boundary" below — the game's gate never learns
the word *nightly*).

---

## TL;DR

Booting rl144 on a real 486 was three unsolved sub-problems: a toolchain that can target
i486, a userland to run on, and a way to get it onto iron. **The Monolith is two of the
three and the donor organ for the first.** Tonight's experiments confirm the Rust half is
tractable and the remaining gap is exactly the piece the Monolith already supplies (an
i486-linux-musl sysroot + linker via crossdev). This is now assembly, not research.

Two rl144-side deliverables remain, both cleanly ours:
1. A **flexible camera console** in `backend-term` (fit any console size, follow the player).
   Its own feature — needs its own brainstorm/spec. Sized below.
2. **Nothing else in the game repo.** The 486 artifact is built by a separate, explicitly-
   nightly path that lives in the *factory*, consuming our source + the target spec above.

---

## What was actually run tonight (measurements, on rustc 1.75.0 stable unless noted)

### The 32-bit determinism story — PROVEN on a real 32-bit build
Cross-built the terminal backend for **`i686-unknown-linux-musl`** (a stock rustup target,
static musl, no C deps), ran it here (x86_64 executes i686 natively):

| Check | x86_64 (minifb) | x86_64 (term) | **i686-musl (term)** | Verdict |
|---|---|---|---|---|
| `--replay tests/fixtures/ref.sav` state hash | `1f22670bf826471a` | `1f22670bf826471a` | **`1f22670bf826471a`** | identical across arch + backend |
| worldgen goldens (seeds 1/2/3/42/1337) | — | — | **all 5 MATCH** | byte-identical dungeons |
| `--solve` (worstSeed / unwinnable) | 82 / 0 | 82 / 0 | **82 / 0** | identical |
| full test suite | 258/258 | 258/258 | **258 passed, 0 failed** | 32-bit clean |

**This is the determinism doctrine's payoff cashed in.** Seventeen batches of explicit-
width discipline (the `usize` ban on hashed state, `i32` coords, fixed-width hashing) mean
the *same input bytes produce the same world-state hash on a 32-bit target as on the 64-bit
workstation.* The termios FFI struct (`u32` flags, `c_cc[32]`) is already musl-correct on
32-bit — the `// Linux x86_64 layout` comment in `backend_term.rs:34` lies, but the layout
matches; it wants a one-word comment fix, not a rewrite.

### The real-i486 story — codegen PROVEN, link gap is the factory's organ
Built the term backend for a genuine **i486** custom target (nightly 1.99 + `-Zbuild-std`),
per the recipe below. Result:

- **Codegen succeeded.** rustc/LLVM compiled our full crate + `std` for i486.
- **Instruction audit of rl144's own object (93,052 instructions): ZERO forbidden ops.**

  | post-486 instruction class | count in rl144 i486 codegen |
  |---|---|
  | `cmov*` | **0** |
  | `cmpxchg8b` (64-bit atomic — the `max-atomic-width:32` trap) | **0** |
  | SSE (`xmm`/`movss`/`movaps`) | **0** |
  | MMX (`mm0-7`/`padd*`/`pxor`/`punpck`) | **0** |
  | plain 486-era ops (`mov`/`add`/`sub`/`jmp`/`call`/…) | 70,141 |

  The target spec did its job at the silicon level. `max-atomic-width: 32` is load-bearing
  and verified — without it, std lowers 64-bit atomics to `CMPXCHG8B` (a Pentium
  instruction) and the "486" would be i586-cosplay.

- **Link failed — and the failure is the whole point.** `ld` could not find `crt1.o`,
  `crti.o`, `-lc`, `-lunwind`: there is no i486-musl **sysroot** in this sandbox. That is
  precisely the organ the Monolith donates — `i486-gentoo-linux-musl-gcc` + a musl sysroot
  from crossdev. **Not rl144's problem to solve, and deliberately not solved here.**

### 486SX caveat (found tonight)
The spec initially set `+soft-float` (for a 486**SX**, which has no FPU). rustc rejects it:
`soft-float is incompatible with the ABI` — x86-32 has no soft-float ABI variant upstream.
So the current spec targets a **486DX** (FPU present), which is a real 486. Full 486SX
support is a genuinely larger upstream lift (a soft-float x86 ABI); given the game binary
is **integer-only** (all 5 float uses live in `headless.rs`/`main.rs` verification tooling
that never ships and never runs on the 486), a DX target is the pragmatic, honest scope.
Flag for whoever owns the factory: *decide DX-only vs. chasing SX.*

---

## The target spec recipe (the answered "expert question")

Least-painful path = **nightly rustc + custom target JSON + `-Zbuild-std`, with the linker
and libc supplied by the Monolith's crossdev i486-linux-musl toolchain, all inside the
Monolith's Docker image.** Crossdev can't conjure a rustc target that doesn't exist
upstream (hence the custom JSON), but it dissolves the linker/libc pain — the exact gap the
link error above hit.

The spec is `i586-unknown-linux-musl`'s built-in spec with four edits, each closing a real
trap (full file in the companion `.json`):

```
cpu:              "pentium"  ->  "i486"          # 486 baseline
llvm-target:      i586-...   ->  i486-...
max-atomic-width: 64         ->  32              # <- the sneaky one: no CMPXCHG8B
features:         (none)     ->  "-cmov,-mmx,-sse,-sse2"   # 486 predates all of these
```

Build invocation that produced tonight's proof (run inside the factory's builder, with its
crossdev sysroot on the linker path):

```
cargo +nightly -Z build-std=std,panic_abort -Z json-target-spec build --release \
  --no-default-features --features backend-term \
  --target i486-unknown-linux-musl.json
```

(`-Zjson-target-spec` is newly required on nightly ≥1.99. `panic_abort` matches our
`panic="abort"`. Strip UPX entirely — it is a floppy-budget device, irrelevant and
counterproductive inside a squashfs.)

---

## Boundary — who owns what

**rl144 owns (this repo):**
- The source, and the guarantee it stays 32-bit-clean and integer-only in the game core.
- The **camera-console feature** (below) — real game code, our brainstorm/spec/gate.
- The target spec *as reference* (the companion `.json`) and this note.
- A stable release tag the ebuild's `SRC_URI` can point at, when asked.
- **Invariant: the game's `make check` gate stays stable-1.75-only. It never learns
  `nightly`, `build-std`, or `i486`.** The floppy binaries are forever 1.75-stable.

**The Monolith / whoever wires the ISO owns (NOT us):**
- Provisioning the builder image with a Rust nightly + `rust-src` + crossdev
  i486-linux-musl gcc/sysroot (the "needed whatevs").
- The `games-roguelike/rl144` **ebuild** and the `world` entry (the "Amusements" section
  has a roguelike-shaped hole where `nethack` was dropped 2026-08-02 for failing on
  i486-musl — rl144 is pure-std and doesn't have nethack's C-cross problem).
- The **console video mode** so the VT is large enough (a 640×480 fbcon at 8×16 is 80×30;
  but see camera feature — we should make the game not *care*).
- Kernel pin, boot, MOTD, real-hardware bring-up.

---

## Two clocks to respect

1. **Kernel:** mainline dropped 486 support in 2025. The Monolith's **6.12 LTS** pin is
   among the last kernels that boot the target. It should be pinned in `versions.lock` with
   a comment saying *why*, or a routine bump someday bricks the premise. (Factory-side, but
   worth flagging in the handover.)
2. **Silicon:** a 33 MHz 486 is fine for this game — turn-based, integer-only, ~200 short
   raycasts/turn, dirty-cell terminal output — but *only because those disciplines exist.*
   The term backend's dirty-cell diff encoder was, in hindsight, built for exactly this bus.

---

## The one remaining rl144 code deliverable: a flexible camera console

Today `backend-term` hardcodes the engine grid: **80×30** (map rows 0–24, status row 25,
log 26–29), and assumes the console is at least that big. The dungeon map is a fixed
**80×25** (`COLS`×`MAP_H`) — the world *is* the grid, never larger — so "bigger than the
screen" bites specifically when the **console is smaller than 80×30** (a 486 text mode at
80×25, a 40-column mode, etc.). Rather than force a video mode, the game should adapt.

Feature scope (its own brainstorm before any code):
- **Query real terminal size** (TIOCGWINSZ — one more ioctl, same FFI style already in the
  file). Interactive-TTY only.
- **Camera:** focus on the player's cell, compute a viewport rect over the 80×25 map, edge-
  clamp, blit the sub-rect; scroll-follow ("kick around") is a dead-zone design choice.
- **Layout reflow** when the console isn't exactly 30 rows (where status/log go when space
  is tight) — the fiddly real work.
- **Seam decision:** centering needs the player's position, but the backend today only
  consumes the *anonymized* `render_cells` grid. Either a tiny derived core accessor
  (camera-focus = player x,y — a justified small core addition per core/crust doctrine) or
  scrape the grid for the player glyph (hacky). Decide during the brainstorm.
- **Must not break:** `--render-frame` stays a frozen 80×30 golden capture; flexible sizing
  is gated on an actual winsize query, so goldens / xhash / headless are untouched. minifb
  (graphical, X11) is out of ISO scope — the fbdev backend (mmap `/dev/fb0`, rasterize
  through `font8x8`) is the phase-two glow-up, and the factory's `fb640` boot label is
  waiting for it.

---

## Staged plan (cheapest risk first)

1. ~~**Stable 32-bit de-risk** — cross-build i686-musl, run tests + `--replay`.~~ **DONE
   tonight. Hash matched, 258/258 tests, goldens match.** The hard part is proven.
2. **The 486 artifact** — target JSON + build-std in the Monolith container, crossdev-
   linked, static. Codegen proven tonight (clean disasm); only the crossdev link remains.
   Boot in the Monolith's `make test` QEMU with `-cpu 486`.
3. **ISO integration** — rl144 into the rootfs (a static musl binary — it *is* a Monolith
   package). Flourish that earns its keep: **run `--replay ref.sav` at boot as a self-test
   and print the hash.** The same bytes → the same world-state hash on a 1993 CPU as on the
   workstation is the determinism doctrine's graduation ceremony.
4. **Real hardware** — BIOS quirks, RAM floor, IDE/floppy behavior.

---

## Aesthetics (they write themselves)

The Monolith — the black slab that teaches — booting a game about a small light that judges
you, on hardware older than the genre's players. If the 486 build lands under 1.44 MB
packed (plausible: no minifb, no font tables for the term variant), there is a coherent
artifact where the *same physical floppy* is bootable next to the Monolith kernel **and**
readable on a modern machine.
