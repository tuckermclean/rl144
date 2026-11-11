# S02 — floppy-image

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | S01 | none | S |

**Owns:** `Makefile` (new `floppy` target), `.github/workflows/release.yml` (the floppy step), `SHIP.md` (the floppy check), `tests/floppy.test.sh` (new), `CLAUDE.md` ("Verification": the `make floppy` command)
**Decisions:** OD4

## Purpose
The road-to-1.0 finish line ends with "`make floppy` writes the ≤1.44MB image: the thesis made
literal" (audit #87). No such target exists. This node writes a standard 1.44 MB FAT12 floppy
image holding the `make ship` artifacts, so the size constraint the whole engine is built
around becomes a shipped object, and publishes it with each release.

## Tests
Keep every existing test. Add `tests/floppy.test.sh <image> <dist>`: the image is exactly
1,474,560 bytes, `minfo` reports a FAT12 1.44 MB geometry, `mdir` lists the OD4 files, and each
file copied out with `mcopy` is byte-identical to its `dist/` source; with a third argument
`--overfull` it runs `make floppy DIST=<tmp>` on a directory holding a 1.5 MB dummy file and expects
it to fail with a message naming the overflow. Red target:
`make floppy && sh tests/floppy.test.sh dist/rl144-floppy.img dist` (non-zero at the base, 0
after).

## Changes
1. Add `make floppy`: from an existing `$(DIST)` directory (default `dist`, built by `make ship`),
   create `$(DIST)/rl144-floppy.img` with mtools (`mformat -f 1440`, `mcopy`), holding the binaries,
   `SHA256SUMS` and a `README.TXT` (ASCII, 8.3-safe names where needed); fail with a message when
   the files do not fit. *S02-AC-1, S02-AC-2, S02-AC-3.*
2. Run `make floppy` and the test in the release workflow and attach the image to the workflow
   artifact and the tag's release. *S02-AC-4.*
3. Add the floppy check to `SHIP.md` and `make floppy` to `CLAUDE.md`'s "Verification" section. *S02-AC-5.*

## Gate
- **S02-AC-1** — `make floppy` writes `dist/rl144-floppy.img` of exactly 1,474,560 bytes with a FAT12 1.44 MB format. *Verify:* `sh tests/floppy.test.sh dist/rl144-floppy.img dist` in the release workflow's dispatch run log.
- **S02-AC-2** — The image holds every `make ship` binary, `SHA256SUMS` and `README.TXT`, and each file copied out is byte-identical to `dist/`. *Verify:* the same script's `mdir` and `cmp` lines in that log.
- **S02-AC-3** — `make floppy` fails with a message naming the overflow when the files exceed the image. *Verify:* `sh tests/floppy.test.sh dist/rl144-floppy.img dist --overfull` in that log.
- **S02-AC-4** — The release workflow builds the image and uploads it with the other artifacts. *Verify:* the dispatch run's artifact list shows `rl144-floppy.img`.
- **S02-AC-5** — `SHIP.md` lists the floppy check and `CLAUDE.md` names `make floppy`. *Verify:* `grep -n 'floppy' SHIP.md CLAUDE.md`.

## Risks
- FAT12 long file names are a VFAT extension; DOS-era readers see 8.3 names, so artifact names
  on the image may need short forms recorded in `README.TXT`.
- mtools must be installed in the release workflow's container.
- Out of scope: a bootable floppy (the ISO factory owns booting).
