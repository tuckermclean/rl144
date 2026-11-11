# S01 — ship-pipeline

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | T02, B01 | S02 | M |

**Owns:** `SHIP.md` (new), `Makefile` (new `ship` target), `.github/workflows/release.yml` (new), `.gitignore` (`dist/`), `rl144-linux-x86_64-upx` (deleted), `tests/ship.test.sh` (new), `CLAUDE.md` (the release-artifact line of "Size budget rules")
**Decisions:** OD3

## Purpose
The road-to-1.0 plan's finish line is `SHIP.md`, "the acceptance list as a checkable artifact
gated by `make ship`", and a release tag the 486 ISO factory can point at; neither exists (audit
#20, #87). The only release artifact, the committed `rl144-linux-x86_64-upx`, is the v0 build
from `2c2b84f`, 17 batches old, and needs glibc 2.38, so it does not start on Debian 12 (audit
#10). This node builds the engine-side release pipeline (answers Q9a, Q9b): `make ship` runs the
gate and produces checksummed, budget-checked artifacts for the OD3 targets, a workflow publishes
them, and the stale binary is deleted. `SHIP.md` lists the release checks now and keeps a section
for the game's acceptance list, which is later work.

## Tests
Keep every existing test. Add `tests/ship.test.sh dist`, which checks a built `dist/`: the three
OD3 artifacts exist, each is ≤ 1,474,560 bytes, `sha256sum -c SHA256SUMS` passes, each binary's
`--dump --seed 1` matches `tests/golden/seed_1.txt`, and the i686 and x86_64 term binaries replay
`tests/fixtures/ref.sav` to the same hash. Red target: `make ship && sh tests/ship.test.sh dist`
(`make ship` does not exist at the base, so it exits non-zero; 0 after).

## Changes
1. Add `make ship`: run `make check`, then build the OD3 targets, pack copies with
   `upx --best --lzma` into `dist/` as `rl144-<target>-<backend>`, fail if any packed size exceeds
   1,474,560 bytes, and write `dist/SHA256SUMS`. *S01-AC-1, S01-AC-2.*
2. Add `SHIP.md`: each check `make ship` performs, how to cut a release (the owner pushes a `v*`
   tag; the first is `v0.1.0`, matching `Cargo.toml`), and an empty "Game acceptance" section for
   later. *S01-AC-4.*
3. Add `.github/workflows/release.yml`: on `v*` tags and `workflow_dispatch`, in the same container
   setup as the check workflow plus the i686 toolchain, run `make ship` and
   `sh tests/ship.test.sh dist`; upload `dist/` as a workflow artifact; on a tag, create a GitHub
   release with the files attached. *S01-AC-3, S01-AC-5.*
4. Delete `rl144-linux-x86_64-upx`, add `dist/` to `.gitignore`, and change `CLAUDE.md`'s
   "repo-root `rl144-<platform>-upx` files are release artifacts" to point at `dist/` and the
   GitHub release. *S01-AC-6.*

## Gate
- **S01-AC-1** — `make ship` runs `make check` and then writes the three OD3 artifacts to `dist/`, each packed and ≤ 1,474,560 bytes, and fails if any is larger. *Verify:* the release workflow's `workflow_dispatch` run log for the PR head, showing the `check` gate lines and each artifact's size.
- **S01-AC-2** — `dist/SHA256SUMS` covers every artifact and `sha256sum -c` passes. *Verify:* `sh tests/ship.test.sh dist` in that run's log.
- **S01-AC-3** — Each artifact's `--dump --seed 1` matches `tests/golden/seed_1.txt` in the `rust:1.75-slim` (Debian 12) container, and the i686 and x86_64 term artifacts replay `tests/fixtures/ref.sav` to `1f22670bf826471a`. *Verify:* `sh tests/ship.test.sh dist` in that run's log.
- **S01-AC-4** — `SHIP.md` lists every check `make ship` and `tests/ship.test.sh` perform, the tag procedure with `v0.1.0` first, and a "Game acceptance" section marked for later. *Verify:* `grep -n 'v0.1.0\|Game acceptance' SHIP.md`; the reviewer matches each listed check to a Makefile or script step.
- **S01-AC-5** — The release workflow runs on `v*` tags and `workflow_dispatch`, uploads `dist/` as a workflow artifact, and creates a GitHub release only for a tag. *Verify:* the workflow file; the dispatch run's artifact list.
- **S01-AC-6** — `rl144-linux-x86_64-upx` is gone, `dist/` is ignored, and `CLAUDE.md` no longer calls repo-root binaries release artifacts. *Verify:* `test ! -e rl144-linux-x86_64-upx`; `grep -n '^/\?dist' .gitignore`; `grep -c 'repo-root' CLAUDE.md` prints 0.
- **S01-AC-7** — `make check` is green in CI with goldens, frames, `xhash` and bands unchanged. *Verify:* the check run URL; `git diff --stat <base>..HEAD -- tests/golden 'tests/*-band.json'` empty.

## Risks
- The x86_64 glibc artifacts run on glibc ≥ 2.36 (the container's); older systems need the
  i686-musl static build. The minifb artifact needs X11 libraries at run time.
- The i686 artifact needs a P6-class CPU; it is not the real-i486 build, which stays the ISO
  factory's (SPEC.md "Scope").
- Creating releases needs `contents: write` for the workflow token; pushing workflow files needs
  workflow permission (owner pushes if the run's credential lacks it).
- The first tag and release are an owner step after merge; this node never pushes a tag.
- Out of scope: the floppy image (S02), the game's acceptance list (later).
